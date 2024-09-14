use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    routing::get,
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine};
use bb8_redis::redis::AsyncCommands;
use ed25519::{signature::SignerMut, Signature};
use ed25519_dalek::{SigningKey, VerifyingKey};
use embed::static_handler;
use futures::{SinkExt, StreamExt};
use rand_core::OsRng;
use std::{collections::HashMap, sync::Arc, time::SystemTime};
use tokio::{fs, sync::Mutex};
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::layer::SubscriberExt;
use uuid::Uuid;

mod embed;
mod pool_extractor;
struct User {
    partner_id: Option<Uuid>,
    sender: tokio::sync::mpsc::UnboundedSender<Message>,
    pub_key: Option<String>,
}
#[derive(Clone)]
struct AppState {
    queues: Arc<Mutex<HashMap<Uuid, Vec<Uuid>>>>,
    all_users: Arc<Mutex<HashMap<Uuid, User>>>,
    dice_seed_signing_keys: Arc<Mutex<SigningKey>>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer());
    tracing::debug!("connecting to redis");

    // let manager = RedisConnectionManager::new("redis://localhost").unwrap();
    // let pool = bb8::Pool::builder().build(manager).await.unwrap();

    // {
    //     // ping the database before starting
    //     let mut conn = pool.get().await.unwrap();
    //     // conn.set::<&str, &str, ()>("foo", "bar").await.unwrap();
    //     let result: String = conn.get("foo").await.unwrap();
    //     assert_eq!(result, "bar");
    // }
    tracing::debug!("successfully connected to redis and pinged it");

    let dice_seed_signing_keys = match fs::read("server_seed").await {
        Ok(data) => SigningKey::from_bytes(&data.try_into().unwrap()),
        Err(_) => {
            let mut csprng = OsRng;
            let priv_key = SigningKey::generate(&mut csprng);
            fs::write("server_seed", priv_key.to_bytes()).await.unwrap();
            priv_key
        }
    };

    let app_state = AppState {
        queues: Arc::new(Mutex::new(HashMap::new())),
        all_users: Arc::new(Mutex::new(HashMap::new())),
        dice_seed_signing_keys: Arc::new(Mutex::new(dice_seed_signing_keys)),
    };

    let app = Router::new()
        .route("/_astro/*file", get(static_handler))
        .route("/assets/*file", get(static_handler))
        .route("/", get(static_handler))
        .route("/index.html", get(static_handler))
        .route("/signup", get(signup))
        .route("/ws", get(ws_handler))
        .with_state(app_state)
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any));
    println!("Starting at localhost:8083");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8083").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn signup() -> impl axum::response::IntoResponse {
    let mut rng = OsRng;

    let priv_key = SigningKey::generate(&mut rng);
    Json(serde_json::json!({
        "pub_key": STANDARD_NO_PAD.encode(priv_key.verifying_key().to_bytes()),
        "priv_key": STANDARD_NO_PAD.encode(priv_key.to_bytes())
    }))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    let mut csprng = OsRng;

    let user_id = Uuid::new_v4();
    let user = User {
        partner_id: None,
        sender: tx.clone(),
        pub_key: None,
    };
    let secret = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    state.all_users.lock().await.insert(user_id, user);

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

    while let Some(Ok(message)) = receiver.next().await {
        if let Message::Text(text) = message {
            let data: serde_json::Value = serde_json::from_str(&text).unwrap();
            match data["type"].as_str() {
                Some("join") => {
                    let mut all_users = state.all_users.lock().await;

                    if let Some(user) = all_users.get_mut(&user_id) {
                        let signature = data["signature"].as_str().unwrap();
                        let pub_key = data["pub_key"].as_str().unwrap();

                        dbg!(&data, &text);

                        let verify_key = VerifyingKey::from_bytes(
                            STANDARD_NO_PAD
                                .decode(pub_key)
                                .unwrap()
                                .as_slice()
                                .try_into()
                                .unwrap(),
                        )
                        .unwrap();
                        let signature = Signature::from_bytes(
                            STANDARD_NO_PAD
                                .decode(signature)
                                .unwrap()
                                .as_slice()
                                .try_into()
                                .unwrap(),
                        );
                        //TODO: check if in redis
                        if verify_key
                            .verify_strict(secret.to_string().as_bytes(), &signature)
                            .is_ok()
                        {
                            user.pub_key = Some(pub_key.to_owned());
                        } else {
                            tx.send(Message::Text(
                                serde_json::json!({"type": "verified_failed"})
                                    .to_string(),
                            ))
                            .unwrap();
                            continue;
                        }

                        let mut queues = state.queues.lock().await;

                        let mut waiting_users = queues.get_mut(&q);
                        let waiting_users = match waiting_users {
                            Some(waiting_users) => waiting_users,
                            None => {
                                queues.insert(q, Vec::new());
                                waiting_users = queues.get_mut(&q);
                                waiting_users.unwrap()
                            }
                        };

                        if let Some(partner_user_id) = waiting_users.pop() {
                            if let Some(partner_user) = all_users.get(&partner_user_id) {
                                let user = all_users.get(&user_id).unwrap();
                                let seed = rand_core::RngCore::next_u32(&mut csprng);
                                let time = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs();
                                let signature = state
                                    .dice_seed_signing_keys
                                    .lock()
                                    .await
                                    .sign(format!("{seed}:{time}").as_bytes());
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
                                all_users.get_mut(&partner_user_id).unwrap().partner_id =
                                    Some(user_id);
                                all_users.get_mut(&user_id).unwrap().partner_id =
                                    Some(partner_user_id);
                            }
                        } else {
                            waiting_users.push(user_id);
                        }
                    }
                }
                Some("ice-candidate") | Some("offer") | Some("answer") => {
                    let partner_id = state
                        .all_users
                        .lock()
                        .await
                        .get(&user_id)
                        .unwrap()
                        .partner_id;
                    if let Some(partner_user_id) = partner_id {
                        if let Some(partner_user) =
                            state.all_users.lock().await.get(&partner_user_id)
                        {
                            partner_user.sender.send(Message::Text(text)).unwrap();
                        }
                    }
                }
                _ => {}
            }
        }
    }
    let partner_id = state
        .all_users
        .lock()
        .await
        .get(&user_id)
        .unwrap()
        .partner_id;
    if let Some(partner_user_id) = partner_id {
        if let Some(partner_user) = state.all_users.lock().await.get(&partner_user_id) {
            partner_user
                .sender
                .send(Message::Text(
                    serde_json::json!({"type": "disconnected"}).to_string(),
                ))
                .unwrap();
        }
    }
    let mut queues = state.queues.lock().await;
    let q = queues.get_mut(&q).unwrap();
    q.retain(|&id| id != user_id);
    state.all_users.lock().await.remove(&user_id);
}
