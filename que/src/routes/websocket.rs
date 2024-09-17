use std::time::SystemTime;

use axum::{
    extract::{
        ws::{Message, WebSocket},
        WebSocketUpgrade,
    },
    Extension,
};
use base64::{engine::general_purpose::STANDARD_NO_PAD, prelude::Engine};
use ed25519_dalek::Signer;
use futures::{SinkExt, StreamExt};
use lib_knuckle::{signature_from_string, verifying_key_from_string};
use rand_core::OsRng;
use uuid::Uuid;

use crate::{
    pool_extractor::{Conn, DatabaseConnection},
    AppState, User,
};

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(state): Extension<AppState>,
    DatabaseConnection(conn): DatabaseConnection,
) -> impl axum::response::IntoResponse {
    dbg!("Got WS connection");
    ws.on_upgrade(|socket| handle_socket(socket, state, conn))
}

pub async fn handle_socket(socket: WebSocket, state: AppState, conn: Conn) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    let mut csprng = OsRng;

    let user_id = Uuid::new_v4();
    let user = User {
        partner_id: None,
        sender: tx.clone(),
        pub_key: None,
        player_id: None,
    };

    dbg!(&state.all_users);
    let secret = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    state.all_users.insert(user_id, user);
    dbg!("Sending Verify");
    sender
        .send(Message::Text(
            serde_json::to_string(&serde_json::json!({
                "type": "verify",
                "verify_time": secret.to_string()
            }))
            .unwrap(),
        ))
        .await
        .unwrap();

    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if sender.send(message).await.is_err() {
                break;
            }
        }
    });

    let q = Uuid::nil();

    if !state.queues.contains_key(&q) {
        state.queues.insert(q, Vec::new());
    }

    'outer: while let Some(Ok(message)) = receiver.next().await {
        if let Message::Text(text) = message {
            let data: serde_json::Value = serde_json::from_str(&text).unwrap();
            match data["type"].as_str() {
                Some("join") => {
                    dbg!("Getting all users");
                    dbg!("Getting a user");
                    if state.all_users.contains_key(&user_id) {
                        let signature = data["signature"].as_str().unwrap();
                        let pub_key = data["pub_key"].as_str().unwrap();
                        {
                            let mut user = state.all_users.get_mut(&user_id).unwrap();

                            dbg!(&data, &text);

                            let verify_key = verifying_key_from_string(pub_key);
                            let signature = signature_from_string(signature);

                            let (verify_key, signature) = match (verify_key, signature) {
                                (Some(verify_key), Some(signature)) => {
                                    (verify_key, signature)
                                }
                                _ => {
                                    tx.send(Message::Text(
                                        serde_json::json!({"type": "disconnect", "reason": "Missing Data"})
                                            .to_string(),
                                    ))
                                    .unwrap();
                                    break 'outer;
                                }
                            };

                            //TODO: check if in redis
                            if verify_key
                                .verify_strict(secret.to_string().as_bytes(), &signature)
                                .is_ok()
                            {
                                user.pub_key = Some(pub_key.to_owned());
                            } else {
                                tx.send(Message::Text(
                                    serde_json::json!({"type": "disconnect", "reason": "Invalid Signature"}).to_string(),
                                ))
                                .unwrap();
                                continue 'outer;
                            }
                        }

                        let data = conn
                            .query_one(
                                "SELECT player_id FROM players WHERE public_key = $1",
                                &[&STANDARD_NO_PAD.decode(&pub_key).unwrap()],
                            )
                            .await
                            .unwrap();
                        dbg!("Getting user again");
                        {
                            let mut user = state.all_users.get_mut(&user_id).unwrap();
                            dbg!("Success!");
                            let id: Uuid = data.get(0);
                            user.player_id = Some(id);
                        }

                        dbg!("Locking queues");

                        let waiting_users = state.queues.get_mut(&q);
                        let mut waiting_users = match waiting_users {
                            Some(waiting_users) => waiting_users,
                            None => {
                                tx.send(Message::Text(
                                    serde_json::json!({"type": "disconnect", "reason": "Missing Data"})
                                        .to_string(),
                                )).unwrap();
                                tracing::error!("Somehow waiting user queue is missing");
                                break 'outer;
                            }
                        };

                        if let Some(partner_user_id) = waiting_users.pop() {
                            dbg!("Partner found!");
                            let partner_option = state.all_users.get(&partner_user_id);
                            if let Some(partner_user) = partner_option.map(|v| v.clone())
                            {
                                {
                                    let mut user =
                                        state.all_users.get_mut(&user_id).unwrap();
                                    dbg!(&user, &partner_user);
                                    let seed = rand_core::RngCore::next_u32(&mut csprng);
                                    let time = std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap()
                                        .as_secs();
                                    let user_pub_key = user.pub_key.clone().unwrap();
                                    let partner_pub_key =
                                        partner_user.pub_key.clone().unwrap();

                                    let signature =
                                        state.dice_seed_signing_keys.lock().await.sign(
                                            format!(
                                                "{seed}:{time}:{}:{}",
                                                user_pub_key, partner_pub_key
                                            )
                                            .as_bytes(),
                                        );
                                    dbg!("Sending Paired");
                                    partner_user
                                    .sender
                                    .send(Message::Text(
                                        serde_json::json!({
                                            "type": "paired",
                                            "public_key": partner_user.pub_key,
                                            "partner_key": user.pub_key,
                                            "initiator": false,
                                            "seed": seed,
                                            "signature": STANDARD_NO_PAD.encode(signature.to_bytes()),
                                            "time": time
                                        })
                                        .to_string(),
                                    ))
                                    .unwrap();
                                    dbg!("Sending User Text");
                                    tx.send(Message::Text(
                                    serde_json::json!({"type": "paired",
                                         "public_key": user.pub_key,
                                         "partner_key": partner_user.pub_key,
                                         "initiator": true,
                                         "seed": seed,
                                         "signature": STANDARD_NO_PAD.encode(signature.to_bytes()),
                                         "time": time
                                    })
                                    .to_string(),
                                ))
                                .unwrap();
                                    dbg!("Done sending user text");
                                    user.partner_id = Some(partner_user_id);
                                }

                                state
                                    .all_users
                                    .get_mut(&partner_user_id)
                                    .unwrap()
                                    .partner_id = Some(user_id);
                            }
                        } else {
                            dbg!("Partner not found!");
                            waiting_users.push(user_id);
                        }
                    }
                }
                Some("ice-candidate") | Some("offer") | Some("answer") => {
                    let partner_id = state.all_users.get(&user_id).unwrap().partner_id;
                    if let Some(partner_user_id) = partner_id {
                        if let Some(partner_user) = state.all_users.get(&partner_user_id)
                        {
                            partner_user.sender.send(Message::Text(text)).unwrap();
                        }
                    }
                }
                _ => {}
            }
        }
    }
    println!("User {:?} disconnected", user_id);
    let partner_id = state.all_users.get(&user_id).unwrap().partner_id;
    if let Some(partner_user_id) = partner_id {
        if let Some(partner_user) = state.all_users.get(&partner_user_id) {
            partner_user
                .sender
                .send(Message::Text(
                    serde_json::json!({"type": "disconnected"}).to_string(),
                ))
                .unwrap();
        }
    }
    if let Some(mut q) = state.queues.get_mut(&q) {
        q.retain(|&id| id != user_id);
    }
    state.all_users.remove(&user_id);
}
