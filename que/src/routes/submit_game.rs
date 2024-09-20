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
use std::time::SystemTime;
use thiserror::Error;
use tokio::{fs, signal, sync::Mutex};
use tokio_postgres::NoTls;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};
use uuid::{ContextV7, Timestamp, Uuid};

use crate::{
    pool_extractor::DatabaseConnection, AppState, SharedContextV7, UserCreateError,
};

pub async fn submit_game(
    DatabaseConnection(conn): DatabaseConnection,
    Extension(clock): Extension<SharedContextV7>,
    Extension(state): Extension<AppState>,
    Json(body): Json<GameBody>,
) -> Result<String, UserCreateError> {
    if body.your_key == body.opponent_key {
        return Err(UserCreateError::BadRequest(
            "Good luck playing against yourself :)".to_owned(),
        ));
    }
    let signature_to_check = match signature_from_string(&body.signature) {
        Some(signature) => signature,
        None => return Err(UserCreateError::InvalidSignature),
    };
    let keys = match body.starting {
        true => (body.your_key.clone(), body.opponent_key.clone()),
        false => (body.opponent_key.clone(), body.your_key.clone()),
    };
    let data_to_check = format!("{}:{}:{}:{}", body.seed, body.time, keys.0, keys.1);
    let is_valid = state
        .dice_seed_signing_keys
        .lock()
        .await
        .verify(data_to_check.as_bytes(), &signature_to_check);
    if is_valid.is_err() {
        return Err(UserCreateError::InvalidSignature);
    };

    let (verify_your, verify_other) = match (
        verifying_key_from_string(&body.your_key),
        verifying_key_from_string(&body.opponent_key),
    ) {
        (Some(your_key), Some(opponent_key)) => (your_key, opponent_key),
        _ => {
            return Err(UserCreateError::InvalidSignature);
        }
    };

    let game = Game::validate_entire_game(
        Keys::VerifyOnly {
            my_keys: verify_your,
            other_keys: verify_other,
        },
        (3, 3),
        ServerGameInfo::new(body.seed, body.starting),
        body.moves,
    );

    if let Err(e) = game {
        return Err(UserCreateError::BadRequest(e.to_string()));
    };

    let (board_data, sql_history) = game.map_err(UserCreateError::BadRequest)?;

    let user_id: Uuid = conn
        .query_one(
            /* language=postgresql */
            "SELECT player_id FROM players WHERE public_key = $1",
            &[&STANDARD_NO_PAD.decode(&body.your_key)?],
        )
        .await?
        .get(0);
    let partner_id: Uuid = conn
        .query_one(
            /* language=postgresql */
            "SELECT player_id FROM players WHERE public_key = $1",
            &[&STANDARD_NO_PAD.decode(&body.opponent_key)?],
        )
        .await?
        .get(0);

    let (winner, result) = match board_data.winner {
        GameEnd {
            winner: true,
            win_by_tie: false,
            win_by_forfeit: false,
        } => (Some(user_id), "win".to_string()),
        GameEnd {
            winner: false,
            win_by_tie: false,
            win_by_forfeit: false,
        } => (Some(partner_id), "win".to_string()),
        GameEnd {
            win_by_tie: true, ..
        } => (None, "tie".to_string()),
        GameEnd {
            winner: true,
            win_by_forfeit: true,
            ..
        } => (Some(user_id), "forfeit".to_string()),
        GameEnd {
            winner: false,
            win_by_forfeit: true,
            ..
        } => (Some(partner_id), "forfeit".to_string()),
    };

    let existing_match = conn
        .query_one(
            /* language=postgresql */
            "SELECT match_id, created_at FROM started_matches WHERE seed = $1 AND time = $2",
            &[&(body.seed as i64), &(body.time as i64)],
        )
        .await
        .map_err(|e| UserCreateError::BadRequest("Cant find match".to_owned()))?;

    let match_id: Uuid = existing_match.get(0);
    let created_at: SystemTime = existing_match.get(1);

    conn.query(
        /* language=postgresql */
        "INSERT INTO matches(
        match_id,
        seed,
        time,
        player1,
        player2,
        winner,
        result,
        points_p1,
        points_p2,
        started_at
    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        &[
            &match_id,
            &(body.seed as i64),
            &(body.time as i64),
            &user_id,
            &partner_id,
            &winner,
            &result,
            &(board_data.points.me.iter().sum::<u32>() as i16),
            &(board_data.points.other.iter().sum::<u32>() as i16),
            &created_at,
        ],
    )
    .await?;

    println!("signature is valid");

    Ok("Ok".to_owned())
}
