use axum::{Extension, Json};
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine};
use lib_knuckle::{
    api_interfaces::GameBody,
    game::{Game, GameEnd, ServerGameInfo},
    keys::Keys,
    signature_from_string, verifying_key_from_string,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio_postgres::types::ToSql;
use uuid::Uuid;

use crate::{pool_extractor::DatabaseConnection, AppState, UserCreateError};

fn unix_timestamp_to_system_time(timestamp: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(timestamp)
}

pub async fn submit_game(
    DatabaseConnection(conn): DatabaseConnection,
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

    tracing::debug!("User {:?} Partner {:?}", user_id, partner_id);

    let game = Game::validate_entire_game(
        Keys::VerifyOnly {
            my_keys: verify_your,
            other_keys: verify_other,
        },
        (user_id, partner_id),
        (3, 3),
        ServerGameInfo::new(body.seed, body.starting),
        body.moves,
    );

    if let Err(e) = game {
        return Err(UserCreateError::BadRequest(e.to_string()));
    };

    let (board_data, sql_history) = game.map_err(UserCreateError::BadRequest)?;

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
        .map_err(|_e| UserCreateError::BadRequest("Cant find match".to_owned()))?;

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

    let mut query = String::from(
        "INSERT INTO moves (match_id, player_id, number, x, seq, created_at) VALUES ",
    );
    let mut params: Vec<&(dyn ToSql + Sync)> = Vec::new();

    for (i, item) in sql_history.iter().enumerate() {
        if i != 0 {
            query.push_str(", ");
        }
        query.push_str(&format!(
            "(${}, ${}, ${}, ${}, ${}, ${})",
            i * 6 + 1,
            i * 6 + 2,
            i * 6 + 3,
            i * 6 + 4,
            i * 6 + 5,
            i * 6 + 6,
        ));
        params.push(&match_id);
        params.push(&item.player);
        params.push(Box::leak(Box::new(item.number as i16)));
        params.push(Box::leak(Box::new(item.x as i16)));
        params.push(Box::leak(Box::new(item.seq as i32)));
        params.push(Box::leak(Box::new(unix_timestamp_to_system_time(item.now))));
    }

    conn.execute(&query, params.as_slice()).await?;

    println!("signature is valid");

    Ok("Ok".to_owned())
}
