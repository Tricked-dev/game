use std::{sync::Arc, time::SystemTime};

use axum::{
    extract::{
        ws::{Message, WebSocket},
        WebSocketUpgrade,
    },
    Extension,
};
use base64::{engine::general_purpose::STANDARD_NO_PAD, prelude::Engine};
use ed25519_dalek::{Signature, Signer};
use futures::{SinkExt, StreamExt};
use lib_knuckle::{signature_from_string, verifying_key_from_string};
use rand_core::OsRng;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    pool_extractor::{Conn, DatabaseConnection},
    AppState, User, UserCreateError,
};

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(state): Extension<AppState>,
    DatabaseConnection(conn): DatabaseConnection,
) -> impl axum::response::IntoResponse {
    tracing::debug!("Got WS connection");
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

    tracing::debug!("{:?}", &state.all_users);
    let secret = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    state.all_users.insert(user_id, user);
    tracing::debug!("Sending Verify");
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

    let mut queue = Arc::new(RwLock::new(Uuid::nil()));

    let state_clone = state.clone();
    let tx_clone = tx.clone();
    let queue_clone = queue.clone();
    let data_handler = || async move {
        let state = state_clone;
        let tx = tx_clone;
        let queue = queue_clone.clone();
        while let Some(Ok(message)) = receiver.next().await {
            if let Message::Text(text) = message {
                let data: serde_json::Value = serde_json::from_str(&text).unwrap();
                match data["type"].as_str() {
                    Some("join") => {
                        tracing::debug!("Getting all users");
                        tracing::debug!("Getting a user");
                        if !state.all_users.contains_key(&user_id) {
                            Err(UserCreateError::Internal(
                                "User not in all_users something broke lol".to_owned(),
                            ))?;
                        }

                        let signature = data["signature"].as_str().ok_or_else(|| {
                            UserCreateError::Internal("Missing signature".to_owned())
                        })?;
                        let pub_key = data["pub_key"].as_str().ok_or_else(|| {
                            UserCreateError::Internal("Missing pub_key".to_owned())
                        })?;

                        // default to public queue
                        let queue_name = match data["queue"].as_str() {
                            Some(queue_name) => Uuid::parse_str(queue_name)?,
                            None => Uuid::nil(),
                        };

                        *queue.write().await = queue_name;

                        {
                            let mut user =
                                state.all_users.get_mut(&user_id).ok_or_else(|| {
                                    UserCreateError::Internal(
                                        "User not in all_users something broke lol"
                                            .to_owned(),
                                    )
                                })?;

                            tracing::debug!("{:?} {:?}", &data, &text);

                            let verify_key = verifying_key_from_string(pub_key)
                                .ok_or_else(|| {
                                    UserCreateError::BadRequest(
                                        "Invalid verify key".to_owned(),
                                    )
                                })?;
                            let signature =
                                signature_from_string(signature).ok_or_else(|| {
                                    UserCreateError::BadRequest(
                                        "Invalid signature".to_owned(),
                                    )
                                })?;

                            verify_key.verify_strict(
                                secret.to_string().as_bytes(),
                                &signature,
                            )?;
                            user.pub_key = Some(pub_key.to_owned());
                        }

                        let data = conn
                            .query_one(
                                "SELECT player_id FROM players WHERE public_key = $1",
                                &[&STANDARD_NO_PAD.decode(pub_key).unwrap()],
                            )
                            .await?;
                        tracing::debug!("Getting user again");
                        {
                            let mut user =
                                state.all_users.get_mut(&user_id).ok_or_else(|| {
                                    UserCreateError::Internal(
                                        "User not in all_users something broke lol"
                                            .to_owned(),
                                    )
                                })?;
                            tracing::debug!("Success!");
                            let id: Uuid = data.get(0);
                            user.player_id = Some(id);
                        }

                        tracing::debug!("Locking queues");

                        let mut waiting_users =
                            state.queues.get_mut(&queue_name).ok_or_else(|| {
                                UserCreateError::Internal(
                                    "Queue not in queues something broke lol".to_owned(),
                                )
                            })?;

                        if let Some(partner_user_id) = waiting_users.pop() {
                            tracing::debug!("Partner found!");
                            let partner_option = state.all_users.get(&partner_user_id);
                            if let Some(partner_user) = partner_option.map(|v| v.clone())
                            {
                                {
                                    let mut user =
                                        state.all_users.get_mut(&user_id).ok_or_else(|| {
                                            UserCreateError::Internal(
                                                "User not in all_users something broke lol"
                                                    .to_owned(),
                                            )
                                        })?;

                                    tracing::debug!("{:?} {:?}", &user, &partner_user);
                                    let seed = rand_core::RngCore::next_u32(&mut csprng);
                                    let time = std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)?
                                        .as_secs();
                                    let user_pub_key =
                                        user.pub_key.clone().ok_or_else(|| {
                                            UserCreateError::Internal(
                                                "User pub_key not set".to_owned(),
                                            )
                                        })?;
                                    let partner_pub_key = partner_user
                                        .pub_key
                                        .clone()
                                        .ok_or_else(|| {
                                            UserCreateError::Internal(
                                                "Partner pub_key not set".to_owned(),
                                            )
                                        })?;

                                    let signature =
                                        state.dice_seed_signing_keys.lock().await.sign(
                                            format!(
                                                "{seed}:{time}:{}:{}",
                                                user_pub_key, partner_pub_key
                                            )
                                            .as_bytes(),
                                        );
                                    tracing::debug!("Sending Paired");
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
                                    ))?;
                                    tracing::debug!("Sending User Text");
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
                                    ))?;
                                    tracing::debug!("Done sending user text");
                                    user.partner_id = Some(partner_user_id);
                                }

                                state
                                    .all_users
                                    .get_mut(&partner_user_id)
                                    .ok_or_else(|| {
                                        UserCreateError::Internal(
                                            "Partner not in all_users something broke lol"
                                                .to_owned(),
                                        )
                                    })?
                                    .partner_id = Some(user_id);
                            }
                        } else {
                            tracing::debug!("Partner not found!");
                            waiting_users.push(user_id);
                        }
                    }
                    Some("ice-candidate") | Some("offer") | Some("answer") => {
                        let partner_id = state
                            .all_users
                            .get(&user_id)
                            .ok_or_else(|| {
                                UserCreateError::Internal(
                                    "User not in all_users something broke lol"
                                        .to_owned(),
                                )
                            })?
                            .partner_id;
                        if let Some(partner_user_id) = partner_id {
                            if let Some(partner_user) =
                                state.all_users.get(&partner_user_id)
                            {
                                partner_user.sender.send(Message::Text(text))?;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    };

    let out: Result<(), UserCreateError> = data_handler().await;
    if let Err(e) = out {
        tracing::debug!("User disconnected: {e:?}");
        tx.send(Message::Text(
            serde_json::json!({"type": "disconnect", "reason": e.to_string()})
                .to_string(),
        ))
        .unwrap();
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
    if let Some(mut q) = state.queues.get_mut(&*queue.read().await) {
        q.retain(|&id| id != user_id);
    }
    state.all_users.remove(&user_id);
}
