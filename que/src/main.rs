use axum::{
    extract::ws::Message,
    routing::{get, post},
    Extension, Json, Router,
};
use axum_thiserror::ErrorStatus;
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine};
use bb8::Pool;
use bb8_postgres::PostgresConnectionManager;
use dashmap::DashMap;
use ed25519_dalek::SigningKey;
use embed::static_handler;
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
use pool_extractor::DatabaseConnection;
use rand_core::OsRng;
use routes::websocket::ws_handler;
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc};
use thiserror::Error;
use tokio::{fs, signal, sync::Mutex};
use tokio_postgres::NoTls;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tracing_subscriber::layer::SubscriberExt;
use uuid::{ContextV7, Timestamp, Uuid};

mod database;
mod embed;
mod pool_extractor;
mod routes;

#[derive(Error, Debug, ErrorStatus)]
pub enum UserCreateError {
    #[error("User {0} already exists")]
    #[status(StatusCode::CONFLICT)]
    UserAlreadyExists(String),
    #[error("Invalid signature")]
    #[status(StatusCode::BAD_REQUEST)]
    InvalidSignature,
    #[error("{0}")]
    #[status(StatusCode::INTERNAL_SERVER_ERROR)]
    Internal(String),
    #[error("{0}")]
    #[status(StatusCode::BAD_REQUEST)]
    BadRequest(String),
    #[error("Internal Database Error: {0}")]
    #[status(StatusCode::INTERNAL_SERVER_ERROR)]
    DatabaseError(#[from] tokio_postgres::Error),
    #[error("Base64 Decode Error: {0}")]
    #[status(StatusCode::BAD_REQUEST)]
    Base64DecodeError(#[from] base64::DecodeError),
    #[error("Pool Error: {0}")]
    #[status(StatusCode::INTERNAL_SERVER_ERROR)]
    PoolError(#[from] bb8::RunError<tokio_postgres::Error>),
    #[error("Signature Error: {0}")]
    #[status(StatusCode::BAD_REQUEST)]
    SignatureError(#[from] ed25519_dalek::SignatureError),
    #[error("UUID Parse Error: {0}")]
    #[status(StatusCode::BAD_REQUEST)]
    UuidParseError(#[from] uuid::Error),
    #[error("Systemtime: {0}")]
    #[status(StatusCode::INTERNAL_SERVER_ERROR)]
    SystemTimeError(#[from] std::time::SystemTimeError),
    #[error("Send Error: {0}")]
    #[status(StatusCode::INTERNAL_SERVER_ERROR)]
    SendError(#[from] tokio::sync::mpsc::error::SendError<Message>),
    #[error("Json Error: {0}")]
    #[status(StatusCode::INTERNAL_SERVER_ERROR)]
    JsonError(#[from] serde_json::Error),
}

#[derive(Clone, Debug)]
struct User {
    partner_id: Option<Uuid>,
    sender: tokio::sync::mpsc::UnboundedSender<Message>,
    pub_key: Option<String>,
    player_id: Option<Uuid>,
}

impl User {
    fn set_pub_key(&mut self, pub_key: String) -> &mut Self {
        self.pub_key = Some(pub_key);
        self
    }
    fn set_player_id(&mut self, player_id: Uuid) -> &mut Self {
        self.player_id = Some(player_id);
        self
    }
    fn set_partner_id(&mut self, partner_id: Uuid) -> &mut Self {
        self.partner_id = Some(partner_id);
        self
    }
}

#[derive(Clone)]
struct AppState {
    queues: Arc<DashMap<Uuid, Vec<Uuid>>>,
    all_users: Arc<DashMap<Uuid, User>>,
    dice_seed_signing_keys: Arc<Mutex<SigningKey>>,
}

impl AppState {
    fn get_user_clone(&self, user_id: &Uuid) -> Option<User> {
        self.all_users.get(user_id).map(|v| v.clone())
    }
}

pub type SharedContextV7 = Arc<Mutex<ContextV7>>;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer());
    tracing::debug!("connecting to redis");

    let manager = PostgresConnectionManager::new_from_stringlike(
        "host=localhost user=postgres password=postgres",
        NoTls,
    )
    .unwrap();
    let pool = Pool::builder().build(manager).await.unwrap();

    pool.get_owned()
        .await
        .unwrap()
        .simple_query(
            "
      CREATE TABLE players (
            player_id UUID PRIMARY KEY,
            public_key BYTEA NOT NULL,
            secret_key BYTEA NOT NULL,
            name TEXT NOT NULL
        );


    ",
        )
        .await
        .ok();
    pool.get_owned()
        .await
        .unwrap()
        .simple_query(
            "
  CREATE TABLE matches (
            match_id UUID PRIMARY KEY,
            seed BIGINT NOT NULL,
            time BIGINT NOT NULL,
            player1 UUID NOT NULL,
            player2 UUID NOT NULL,
            winner UUID,
            result TEXT NOT NULL,
            points_p1 SMALLINT NOT NULL,
            points_p2 SMALLINT NOT NULL,
            FOREIGN KEY (player1) REFERENCES players(player_id),
            FOREIGN KEY (player2) REFERENCES players(player_id),
            FOREIGN KEY (winner) REFERENCES players(player_id),
            UNIQUE (seed, time)
        );
    ",
        )
        .await
        .ok();

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
        queues: Arc::new(DashMap::new()),
        all_users: Arc::new(DashMap::new()),
        dice_seed_signing_keys: Arc::new(Mutex::new(dice_seed_signing_keys)),
    };

    app_state.queues.insert(Uuid::nil(), Vec::new());

    let uuid_clock = ContextV7::new();

    let app = Router::new()
        .route("/_astro/*file", get(static_handler))
        .route("/assets/*file", get(static_handler))
        .route("/fonts/*file", get(static_handler))
        .route("/", get(static_handler))
        .route("/index.html", get(static_handler))
        .route("/signup", get(signup))
        .route("/ws", get(ws_handler))
        .route("/submit_game", post(submit_game))
        .route("/leaderboard", get(leader_board))
        .route("/set_name", post(set_name))
        .with_state(pool)
        .layer(Extension(app_state))
        .layer(Extension(Arc::new(Mutex::new(uuid_clock))))
        .layer(
            CorsLayer::new()
                .allow_origin(AllowOrigin::predicate(
                    |_origin: &HeaderValue, _request_parts: &http::request::Parts| true,
                ))
                .allow_headers([AUTHORIZATION, ACCEPT, CONTENT_TYPE])
                .allow_private_network(true)
                .allow_methods(Any),
        );

    println!("Starting at localhost:8083");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8083").await.unwrap();
    let terminate_signal = signal::ctrl_c();

    // Use select to run the server and listen for termination signal
    tokio::select! {
        _ = axum::serve(listener, app) => {
            println!("Server finished execution");
        },
        _ = terminate_signal => {
            println!("Received termination signal, shutting down...");
        }
    }
}

async fn set_name(
    DatabaseConnection(conn): DatabaseConnection,
    Json(body): Json<UserUpdate>,
) -> Result<String, UserCreateError> {
    let signature = signature_from_string(&body.signature);
    let pub_key = verifying_key_from_string(&body.pub_key);
    if let (Some(signature), Some(pub_key)) = (signature, pub_key) {
        pub_key.verify_strict(body.name.as_bytes(), &signature)?;
        conn.query(
            "UPDATE players SET name = $1 WHERE public_key = $2",
            &[&body.name, &STANDARD_NO_PAD.decode(&body.pub_key)?],
        )
        .await?;
        Ok("Ok".to_string())
    } else {
        Err(UserCreateError::InvalidSignature)
    }
}

async fn signup(
    DatabaseConnection(conn): DatabaseConnection,
    Extension(clock): Extension<SharedContextV7>,
) -> impl axum::response::IntoResponse {
    let mut rng = OsRng;

    let priv_key = SigningKey::generate(&mut rng);
    conn.query(
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

async fn leader_board(
    DatabaseConnection(conn): DatabaseConnection,
) -> Result<axum::Json<LeaderBoard>, UserCreateError> {
    let leader_board = conn
        .query(
            "
SELECT
    p.name,
    COALESCE(SUM(
        CASE
            WHEN m.player1 = p.player_id THEN m.points_p1
            WHEN m.player2 = p.player_id THEN m.points_p2
        END
    ), 0) AS total_points,
    COUNT(DISTINCT m.match_id) AS total_games,
    COUNT(CASE WHEN m.winner = p.player_id THEN 1 END) AS total_wins
FROM
    players p
LEFT JOIN
    matches m ON p.player_id IN (m.player1, m.player2)
GROUP BY
    p.player_id
ORDER BY
    total_points DESC, total_wins DESC, total_games DESC, p.name;
            ",
            &[],
        )
        .await?;
    let leader_board = leader_board
        .iter()
        .map(|row| {
            let name: &str = row.get(0);
            let total_points: i64 = row.get(1);
            let total_games: i64 = row.get(2);
            let total_wins: i64 = row.get(3);
            LeaderBoardEntry {
                name: name.to_owned(),
                total_points: total_points as u32,
                total_games: total_games as u32,
                total_wins: total_wins as u32,
            }
        })
        .collect::<Vec<_>>();

    Ok(Json(LeaderBoard {
        total: leader_board.len() as u32,
        entries: leader_board,
    }))
}

async fn submit_game(
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
            "SELECT player_id FROM players WHERE public_key = $1",
            &[&STANDARD_NO_PAD.decode(&body.your_key)?],
        )
        .await?
        .get(0);
    let partner_id: Uuid = conn
        .query_one(
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

    conn.query(
        "INSERT INTO matches(
        match_id,
        seed,
        time,
        player1,
        player2,
        winner,
        result,
        points_p1,
        points_p2
    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        &[
            &Uuid::new_v7(Timestamp::now(&*clock.lock().await)),
            &(body.seed as i64),
            &(body.time as i64),
            &user_id,
            &partner_id,
            &winner,
            &result,
            &(board_data.points.me.iter().sum::<u32>() as i16),
            &(board_data.points.other.iter().sum::<u32>() as i16),
        ],
    )
    .await?;

    println!("signature is valid");

    Ok("Ok".to_owned())
}

/// Utility function for mapping any error into a `500 Internal Server Error`
/// response.
fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}
