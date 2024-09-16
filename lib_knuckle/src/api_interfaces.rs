use serde::{Deserialize, Serialize};
// TODO: possibly split up crate into game and utils for interop
use crate::game::HistoryItem;

#[derive(Clone, Deserialize, Serialize)]
#[cfg_attr(
    any(test, target_arch = "wasm32", feature = "wasm"),
    derive(tsify::Tsify)
)]
#[cfg_attr(
    any(test, target_arch = "wasm32", feature = "wasm"),
    tsify(into_wasm_abi, from_wasm_abi)
)]
pub struct GameBody {
    // required for signature
    pub seed: u64,
    // required for signature
    pub time: u64,
    // required for signature
    pub your_key: String,
    // required for signature
    pub opponent_key: String,
    // decides wether his key will go first in check
    pub starting: bool,
    pub signature: String,
    pub moves: Vec<HistoryItem>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(
    any(test, target_arch = "wasm32", feature = "wasm"),
    derive(tsify::Tsify)
)]
#[cfg_attr(
    any(test, target_arch = "wasm32", feature = "wasm"),
    tsify(into_wasm_abi, from_wasm_abi)
)]
pub struct LeaderBoard {
    pub total: u32,
    pub entries: Vec<LeaderBoardEntry>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(
    any(test, target_arch = "wasm32", feature = "wasm"),
    derive(tsify::Tsify)
)]
#[cfg_attr(
    any(test, target_arch = "wasm32", feature = "wasm"),
    tsify(into_wasm_abi, from_wasm_abi)
)]
pub struct LeaderBoardEntry {
    pub name: String,
    pub total_points: u32,
    pub total_games: u32,
    pub total_wins: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(
    any(test, target_arch = "wasm32", feature = "wasm"),
    derive(tsify::Tsify)
)]
#[cfg_attr(
    any(test, target_arch = "wasm32", feature = "wasm"),
    tsify(into_wasm_abi, from_wasm_abi)
)]
pub struct UserUpdate {
    pub name: String,
    pub pub_key: String,
    pub signature: String,
}
