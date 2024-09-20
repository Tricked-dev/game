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
use serde::{Deserialize, Serialize};
use uuid::{NoContext, Timestamp, Uuid};

use crate::{
    ice_servers::{IceServerData, IceServers},
    pool_extractor::{Conn, DatabaseConnection},
    AppState, User, UserCreateError,
};

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(state): Extension<AppState>,
    Extension(ice_server_provider): Extension<IceServerData>,
    DatabaseConnection(conn): DatabaseConnection,
) -> impl axum::response::IntoResponse {
    tracing::debug!("Got WS connection");
    ws.on_upgrade(|socket| async {
        if let Err(e) = handle_socket(socket, state, conn, ice_server_provider).await {
            tracing::error!("Error in websocket: {e:?}");
        }
    })
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum SendMessages {
    #[serde(rename = "verify")]
    Verify { verify_time: String },
    #[serde(rename = "paired")]
    Paired {
        public_key: String,
        partner_key: String,
        initiator: bool,
        seed: u32,
        signature: String,
        time: u64,
        ice_servers: IceServers,
    },
    #[serde(rename = "partner-left")]
    PartnerLeft,
    #[serde(rename = "disconnected")]
    Disconnected { reason: String, name: String },
}

impl SendMessages {
    fn to_text_message(&self) -> Result<Message, serde_json::Error> {
        Ok(Message::Text(serde_json::to_string(self)?))
    }
}

trait TrickedShenanigans<T> {
    fn ok_or_badrequest(self, error: &str) -> Result<T, UserCreateError>;
    fn ok_or_internal(self, error: &str) -> Result<T, UserCreateError>;
}

impl<T> TrickedShenanigans<T> for Option<T> {
    fn ok_or_badrequest(self, error: &str) -> Result<T, UserCreateError> {
        self.ok_or_else(|| UserCreateError::BadRequest(error.to_string()))
    }
    fn ok_or_internal(self, error: &str) -> Result<T, UserCreateError> {
        self.ok_or_else(|| UserCreateError::Internal(error.to_string()))
    }
}

fn verify_signature(
    signature: &str,
    pub_key: &str,
    secret: &str,
) -> Result<(), UserCreateError> {
    let verify_key =
        verifying_key_from_string(pub_key).ok_or_badrequest("Invalid verify key")?;

    let signature =
        signature_from_string(signature).ok_or_badrequest("Invalid signature")?;

    verify_key.verify_strict(secret.as_bytes(), &signature)?;

    Ok(())
}

async fn resolve_user_name(conn: &Conn, pub_key: &str) -> Result<Uuid, UserCreateError> {
    let data = conn
        .query_one(
            "SELECT player_id FROM players WHERE public_key = $1",
            &[&STANDARD_NO_PAD.decode(pub_key).unwrap()],
        )
        .await
        .map_err(|e| UserCreateError::UserDoesNotExist)?;
    let id: Uuid = data.get(0);
    Ok(id)
}

pub async fn handle_socket(
    socket: WebSocket,
    state: AppState,
    conn: Conn,
    ice_server_provider: IceServerData,
) -> Result<(), UserCreateError> {
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
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs();
    state.all_users.insert(user_id, user);
    tracing::debug!("Sending Verify");

    sender
        .send(
            SendMessages::Verify {
                verify_time: secret.to_string(),
            }
            .to_text_message()?,
        )
        .await
        .map_err(|_| UserCreateError::Internal("Failed Sending".to_owned()))?;

    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if sender.send(message).await.is_err() {
                break;
            }
        }
    });

    let mut queue_name = Uuid::nil();

    let data_handler = async {
        while let Some(Ok(message)) = receiver.next().await {
            if let Message::Text(text) = message {
                let data: serde_json::Value = serde_json::from_str(&text)?;
                match data["type"].as_str() {
                    Some("join") => {
                        let signature = data["signature"]
                            .as_str()
                            .ok_or_badrequest("Missing signature")?;
                        let pub_key = data["pub_key"]
                            .as_str()
                            .ok_or_badrequest("Missing pub_key")?;

                        verify_signature(signature, pub_key, &secret.to_string())?;

                        tracing::debug!("Setting pub_key and player_id");

                        state
                            .all_users
                            .get_mut(&user_id)
                            .ok_or_internal("User not in all_users something broke lol")?
                            .set_pub_key(pub_key.to_owned())
                            .set_player_id(resolve_user_name(&conn, pub_key).await?);

                        tracing::debug!("Locking queues");

                        // default to public queue
                        queue_name = match data["queue"].as_str() {
                            Some(queue_name) => Uuid::parse_str(queue_name)?,
                            None => Uuid::nil(),
                        };

                        let mut waiting_users =
                            state.queues.get_mut(&queue_name).ok_or_internal(
                                "User not in all_users something broke lol",
                            )?;

                        if let Some(partner_user_id) = waiting_users.pop() {
                            tracing::debug!("Partner found!");
                            let partner_user =
                                state.get_user_clone(&partner_user_id).ok_or_internal(
                                    "Partner not in all_users something broke lol",
                                )?;

                            let user = state.get_user_clone(&user_id).ok_or_internal(
                                "User not in all_users something broke lol",
                            )?;

                            tracing::debug!("{:?} {:?}", &user, &partner_user);
                            let seed = rand_core::RngCore::next_u32(&mut csprng);
                            let time = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)?
                                .as_secs();

                            let user_pub_key = pub_key.to_owned();
                            let partner_pub_key = partner_user
                                .pub_key
                                .clone()
                                .ok_or_internal("Partner pub_key not set")?;

                            let game_signature = STANDARD_NO_PAD.encode(
                                state
                                    .dice_seed_signing_keys
                                    .lock()
                                    .await
                                    .sign(
                                        format!(
                                            "{seed}:{time}:{}:{}",
                                            user_pub_key, partner_pub_key
                                        )
                                        .as_bytes(),
                                    )
                                    .to_bytes(),
                            );

                            conn.execute(
                                /* language=postgresql */
                                "INSERT INTO started_matches (match_id, time, seed, player1, player2) VALUES ($1, $2, $3, $4, $5)",
                            &[&Uuid::new_v7(Timestamp::now(NoContext)), &(time as i64),  &(seed as i64), &user_id, &partner_user_id])
                            .await?;

                            tracing::debug!("Sending Paired");

                            let ice_servers =
                                ice_server_provider.get_ice_servers().await?;

                            partner_user.sender.send(
                                SendMessages::Paired {
                                    public_key: partner_pub_key.clone(),
                                    partner_key: user_pub_key.clone(),
                                    initiator: false,
                                    seed,
                                    signature: game_signature.clone(),
                                    ice_servers: ice_servers.clone(),
                                    time,
                                }
                                .to_text_message()?,
                            )?;
                            tracing::debug!("Sending User Text");

                            tx.send(
                                SendMessages::Paired {
                                    public_key: user_pub_key,
                                    partner_key: partner_pub_key,
                                    initiator: true,
                                    seed,
                                    signature: game_signature,
                                    ice_servers: ice_servers,
                                    time,
                                }
                                .to_text_message()?,
                            )?;
                            tracing::debug!("Done sending user text");

                            state
                                .all_users
                                .get_mut(&user_id)
                                .ok_or_internal(
                                    "Partner not in all_users something broke lol",
                                )?
                                .set_partner_id(partner_user_id);
                            state
                                .all_users
                                .get_mut(&partner_user_id)
                                .ok_or_internal(
                                    "Partner not in all_users something broke lol",
                                )?
                                .set_partner_id(user_id);
                        } else {
                            tracing::debug!("Partner not found!");
                            waiting_users.push(user_id);
                        }
                    }
                    Some("ice-candidate")
                    | Some("offer")
                    | Some("answer")
                    | Some("candidate") => {
                        let partner_id = state
                            .all_users
                            .get(&user_id)
                            .ok_or_internal("User not in all_users something broke lol")?
                            .partner_id;
                        if let Some(partner_user_id) = partner_id {
                            if let Some(partner_user) =
                                state.all_users.get(&partner_user_id)
                            {
                                partner_user.sender.send(Message::Text(text))?;
                            }
                        }
                    }
                    option => {
                        dbg!(&option);
                    }
                }
            }
        }
        Ok(())
    };

    let out: Result<(), UserCreateError> = data_handler.await;
    if let Err(e) = out {
        tracing::debug!("User disconnected: {e:?}");
        tx.send(
            SendMessages::Disconnected {
                name: e.get_name().to_owned(),
                reason: e.to_string(),
            }
            .to_text_message()?,
        )?;
    }
    tracing::info!("User {:?} disconnected", user_id);
    let partner_id = state.all_users.get(&user_id).unwrap().partner_id;
    if let Some(partner_user_id) = partner_id {
        if let Some(partner_user) = state.all_users.get(&partner_user_id) {
            partner_user
                .sender
                .send(SendMessages::PartnerLeft.to_text_message()?)?;
        }
    }
    if let Some(mut q) = state.queues.get_mut(&queue_name) {
        q.retain(|&id| id != user_id);
    }
    state.all_users.remove(&user_id);
    Ok(())
}
