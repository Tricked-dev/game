use axum::{Extension, Json};
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine};
use ed25519_dalek::SigningKey;
use rand_core::OsRng;
use uuid::{Timestamp, Uuid};

use crate::{pool_extractor::DatabaseConnection, SharedContextV7};

pub async fn signup(
    DatabaseConnection(conn): DatabaseConnection,
    Extension(clock): Extension<SharedContextV7>,
) -> impl axum::response::IntoResponse {
    let mut rng = OsRng;

    let priv_key = SigningKey::generate(&mut rng);
    conn.query(
        /* language=postgresql */
        "INSERT INTO players (player_id, public_key, secret_key, name) VALUES ($1, $2, $3, $4)",
        &[
            &Uuid::new_v7(Timestamp::now(&*clock.lock().await)),
            &priv_key.verifying_key().to_bytes().to_vec(),
            &priv_key.to_bytes().to_vec(),
            &String::from("Player"),
        ],
    )
    .await
    .unwrap();

    Json(serde_json::json!({
        "pub_key": STANDARD_NO_PAD.encode(priv_key.verifying_key().to_bytes()),
        "priv_key": STANDARD_NO_PAD.encode(priv_key.to_bytes())
    }))
}
