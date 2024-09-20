use axum::{
    extract::ws::Message,
    routing::{get, post},
    Extension, Json, Router,
};
use axum_thiserror::ErrorStatus;
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine};
use bb8::Pool;
use bb8_postgres::PostgresConnectionManager;
use clap::Parser;
use dashmap::DashMap;
use ed25519_dalek::SigningKey;
use http::{
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    HeaderValue, StatusCode,
};
use lib_knuckle::{
    api_interfaces::{GameBody, LeaderBoard, LeaderBoardEntry, UserUpdate},
    game::{Game, GameEnd, ServerGameInfo},
    keys::Keys,
    signature_from_string, verifying_key_from_string,
};
use rand_core::OsRng;
use std::sync::Arc;
use thiserror::Error;
use tokio::{fs, signal, sync::Mutex};
use tokio_postgres::NoTls;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};
use uuid::{ContextV7, Timestamp, Uuid};

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
