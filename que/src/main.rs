use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    routing::get,
    Router,
};
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine};
use ed25519::signature::SignerMut;
use ed25519_dalek::SigningKey;
use embed::static_handler;
use futures::{SinkExt, StreamExt};
use rand_core::OsRng;
use std::{collections::HashMap, sync::Arc};
use tokio::{fs, sync::Mutex};
use uuid::Uuid;

mod embed;

struct User {
    partner_id: Option<Uuid>,
    sender: tokio::sync::mpsc::UnboundedSender<Message>,
    pub_key: String,
    priv_key: String,
}
#[derive(Clone)]
struct AppState {
    waiting_users: Arc<Mutex<Vec<Uuid>>>,
    all_users: Arc<Mutex<HashMap<Uuid, User>>>,
    dice_seed_signing_keys: Arc<Mutex<SigningKey>>,
}

#[tokio::main]
async fn main() {
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
        waiting_users: Arc::new(Mutex::new(Vec::new())),
        all_users: Arc::new(Mutex::new(HashMap::new())),
        dice_seed_signing_keys: Arc::new(Mutex::new(dice_seed_signing_keys)),
    };

    let app = Router::new()
        .route("/_astro/*file", get(static_handler))
        .route("/", get(static_handler))
        .route("/index.html", get(static_handler))
        .route("/ws", get(ws_handler))
        .with_state(app_state);
    println!("Starting at localhost:8083");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8083").await.unwrap();
    axum::serve(listener, app).await.unwrap();
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
    let signing_key: SigningKey = SigningKey::generate(&mut csprng);

    let user_id = Uuid::new_v4();
    let user = User {
        partner_id: None,
        sender: tx.clone(),
        priv_key: STANDARD_NO_PAD.encode(signing_key.to_bytes()),
        pub_key: STANDARD_NO_PAD.encode(signing_key.verifying_key().to_bytes()),
    };
    let pub_key = user.pub_key.clone();
    let priv_key = user.priv_key.clone();
    state.all_users.lock().await.insert(user_id, user);

    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if sender.send(message).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(message)) = receiver.next().await {
        if let Message::Text(text) = message {
            let data: serde_json::Value = serde_json::from_str(&text).unwrap();
            match data["type"].as_str() {
                Some("join") => {
                    let mut waiting_users = state.waiting_users.lock().await;
                    if let Some(partner_user_id) = waiting_users.pop() {
                        let mut all_users = state.all_users.lock().await;
                        if let Some(partner_user) = all_users.get(&partner_user_id) {
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
                                        "private_key": partner_user.priv_key,
                                        "partner_key": pub_key,
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
                                     "public_key": pub_key,
                                     "private_key": priv_key,
                                     "partner_key": partner_user.pub_key,
                                     "initiator": true,
                                     "seed": seed,
                                     "signature": STANDARD_NO_PAD.encode(signature.to_bytes()),
                                     "time": time
                                })
                                .to_string(),
                            ))
                            .unwrap();
                            all_users.get_mut(&partner_user_id).unwrap().partner_id = Some(user_id);
                            all_users.get_mut(&user_id).unwrap().partner_id = Some(partner_user_id);
                        }
                    } else {
                        waiting_users.push(user_id);
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

    state.waiting_users.lock().await.retain(|&id| id != user_id);
    state.all_users.lock().await.remove(&user_id);
}
