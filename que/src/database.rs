use ed25519_dalek::{PUBLIC_KEY_LENGTH, SECRET_KEY_LENGTH};
use uuid::Uuid;

pub struct Player {
    // PK
    player_id: Uuid,
    public_key: String,
    secret_key: String,
    name: String,
}
pub struct Match {
    // PK
    match_id: Uuid,
    seed: u64,
    time: u64,
    player1: Uuid,
    player2: Uuid,
    winner: Option<Uuid>,
    result: ResultType,
}

pub enum ResultType {
    Win,
    Forfeit,
    Tie,
}
