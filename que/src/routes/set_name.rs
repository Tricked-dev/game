use axum::{
    debug_handler,
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

use crate::{pool_extractor::DatabaseConnection, UserCreateError};

pub async fn set_name(
    DatabaseConnection(conn): DatabaseConnection,
    Json(body): Json<UserUpdate>,
) -> Result<String, UserCreateError> {
    let signature = signature_from_string(&body.signature);
    let pub_key = verifying_key_from_string(&body.pub_key);
    if let (Some(signature), Some(pub_key)) = (signature, pub_key) {
        pub_key.verify_strict(body.name.as_bytes(), &signature)?;
        conn.query(
            /* language=postgresql */
            "UPDATE players SET name = $1 WHERE public_key = $2",
            &[&body.name, &STANDARD_NO_PAD.decode(&body.pub_key)?],
        )
        .await?;
        Ok("Ok".to_string())
    } else {
        Err(UserCreateError::InvalidSignature)
    }
}
