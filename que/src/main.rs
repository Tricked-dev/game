use axum::{
    extract::ws::Message,
    routing::{get, post},
    Extension, Router,
};
use axum_thiserror::ErrorStatus;
use bb8::Pool;
use bb8_postgres::PostgresConnectionManager;
use clap::Parser;
use dashmap::DashMap;
use database::init_db;
use ed25519_dalek::SigningKey;
use embed::static_handler;
use http::{
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    HeaderValue, StatusCode,
};
use ice_servers::{
    CloudflareIceServerProvider, GoogleIceServerProvider, IceServerProvider,
};
use rand_core::OsRng;
use routes::{leader_board, set_name, signup, submit_game, ws_handler};
use std::{sync::Arc, time::Instant};
use strum::EnumMessage;
use thiserror::Error;
use tokio::{fs, signal, sync::Mutex};
use tokio_postgres::NoTls;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::{ContextV7, Uuid};

pub mod database;
pub mod embed;
pub mod ice_servers;
pub mod pool_extractor;
pub mod routes;

#[derive(Error, Debug, ErrorStatus, strum_macros::EnumMessage)]
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
    #[error("Reqwest Error: {0}")]
    #[status(StatusCode::INTERNAL_SERVER_ERROR)]
    ReqwestError(#[from] reqwest::Error),
    #[error("User does not exist")]
    #[status(StatusCode::BAD_REQUEST)]
    UserDoesNotExist,
}

impl UserCreateError {
    pub fn get_name(&self) -> &'static str {
        self.get_serializations()[0]
    }
}

#[derive(Clone, Debug)]
pub struct User {
    partner_id: Option<Uuid>,
    sender: tokio::sync::mpsc::UnboundedSender<Message>,
    pub_key: Option<String>,
    player_id: Option<Uuid>,
    in_queue_since: Instant,
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

pub type AllUsers = Arc<DashMap<Uuid, User>>;

#[derive(Clone)]
pub struct AppState {
    queues: Arc<DashMap<Uuid, Vec<Uuid>>>,
    all_users: AllUsers,
    dice_seed_signing_keys: Arc<Mutex<SigningKey>>,
}

impl AppState {
    fn get_user_clone(&self, user_id: &Uuid) -> Option<User> {
        self.all_users.get(user_id).map(|v| v.clone())
    }
}

pub type SharedContextV7 = Arc<Mutex<ContextV7>>;

#[derive(Parser, Debug)]
struct Args {
    #[clap(
        long,
        env = "DATABASE_STRINGLIKE",
        default_value = "host=localhost user=postgres password=postgres"
    )]
    database_stringlike: String,
    #[clap(long, env = "TURN_TOKEN_ID")]
    turn_token_id: Option<String>,
    #[clap(long, env = "API_TOKEN")]
    api_token: Option<String>,
}

#[tokio::main]
async fn main() {
    dotenv_rs::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("error,{}=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    tracing::debug!("connecting to Postgresql");

    let args = Args::parse();

    let ice_server_provider: IceServerProvider =
        if let (Some(turn_token_id), Some(api_token)) =
            (args.turn_token_id, args.api_token)
        {
            IceServerProvider::Cloudflare(CloudflareIceServerProvider::new(
                turn_token_id,
                api_token,
            ))
        } else {
            IceServerProvider::Google(GoogleIceServerProvider)
        };

    let manager =
        PostgresConnectionManager::new_from_stringlike(args.database_stringlike, NoTls)
            .unwrap();
    let pool = Pool::builder().build(manager).await.unwrap();

    init_db(pool.get_owned().await.unwrap()).await.unwrap();

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
        .route("/og/*file", get(static_handler))
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
        .layer(Extension(Arc::new(ice_server_provider)))
        .layer(
            CorsLayer::new()
                .allow_origin(AllowOrigin::predicate(
                    |_origin: &HeaderValue, _request_parts: &http::request::Parts| true,
                ))
                .allow_headers([AUTHORIZATION, ACCEPT, CONTENT_TYPE])
                .allow_private_network(true)
                .allow_methods(Any),
        );

    tracing::info!("Starting at localhost:8083");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8083").await.unwrap();
    let terminate_signal = signal::ctrl_c();

    use parking_lot::deadlock;
    use std::{thread, time::Duration};
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(10));
        let deadlocks = deadlock::check_deadlock();
        if deadlocks.is_empty() {
            continue;
        }

        println!("{} deadlocks detected", deadlocks.len());
        for (i, threads) in deadlocks.iter().enumerate() {
            println!("Deadlock #{}", i);
            for t in threads {
                println!("Thread Id {:#?}", t.thread_id());
                println!("{:#?}", t.backtrace());
            }
        }
    });

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

/// Utility function for mapping any error into a `500 Internal Server Error`
/// response.
fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}
