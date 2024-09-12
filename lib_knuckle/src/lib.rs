use ed25519::signature::SignerMut;
use ed25519::Signature;
use ed25519_dalek::Verifier;
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::RngCore;
use rand::{rngs::StdRng, SeedableRng};
use serde::{Deserialize, Serialize};
use shift_columns::{shift_column_values, FloatDirection};
#[allow(unused)]
use std::time::{SystemTime, UNIX_EPOCH};
use utils::knucklebones_points::calculate_knucklebones_points;
#[cfg(any(test, target_arch = "wasm32", feature = "wasm"))]
use wasm_bindgen::prelude::wasm_bindgen;

mod shift_columns;
#[cfg(any(test, target_arch = "wasm32", feature = "wasm"))]
mod wasm;
#[cfg(any(test, target_arch = "wasm32", feature = "wasm"))]
pub use wasm::*;
#[cfg(test)]
mod tests;

mod utils;

pub use utils::signing_helpers::*;

use utils::now_impl::now;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoryItem {
    seq: u32,
    now: u64,
    x: usize,
    signature: Vec<u8>,
}

#[derive(Debug)]
pub struct ServerGameInfo {
    seed: u64,
    starting: bool,
}

pub struct Dice {
    next_dice: u8,
    rng: StdRng,
}

impl Dice {
    pub fn new(seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        let next_dice = (rng.next_u32() % 6) as u8 + 1;
        Dice { next_dice, rng }
    }

    pub fn roll(&mut self) -> usize {
        let num = self.next_dice;
        self.next_dice = (self.rng.next_u32() % 6) as u8 + 1;
        num as usize
    }
    pub fn peek(&self) -> usize {
        self.next_dice as usize
    }

    #[cfg(test)]
    pub(crate) fn set_next(&mut self, num: u8) {
        self.next_dice = num
    }
}

#[cfg_attr(any(test, target_arch = "wasm32", feature = "wasm"), wasm_bindgen)]
pub struct Game {
    history: Vec<HistoryItem>,
    deck: Vec<u32>,
    other_deck: Vec<u32>,
    seq: u32,
    dice: Dice,
    my_keys: SigningKey,
    other_keys: VerifyingKey,
    deck_size: (usize, usize),
    info: ServerGameInfo,
    verify: bool,
}

impl Game {
    pub fn new(
        my_keys: SigningKey,
        other_keys: VerifyingKey,
        deck_size: (usize, usize),
        info: ServerGameInfo,
    ) -> Self {
        let deck = Self::create_deck(deck_size);
        let other_deck = Self::create_deck(deck_size);
        let dice = Dice::new(info.seed);

        Game {
            history: Vec::new(),
            deck,
            other_deck,
            seq: 0,
            dice,
            my_keys,
            other_keys,
            deck_size,
            info,
            verify: true,
        }
    }

    pub fn disable_verify(&mut self) {
        self.verify = false;
    }

    fn encode_history_item(item: &HistoryItem) -> Vec<u8> {
        format!("{}:{}:{}", item.seq, item.now, item.x).into_bytes()
    }

    fn create_deck(desk_size: (usize, usize)) -> Vec<u32> {
        vec![0; desk_size.0 * desk_size.1]
    }

    pub fn add_opponent_move(&mut self, data: HistoryItem) -> Result<(), String> {
        self.seq += 1;
        self.history.push(data.clone());
        self.play_move(data)
    }

    pub fn place(&mut self, x: usize) -> Result<HistoryItem, String> {
        let signed_item = self.create_history_for_placing(x)?;

        self.seq += 1;
        self.history.push(signed_item.clone());
        self.play_move(signed_item.clone())?;
        Ok(signed_item)
    }

    pub fn test_place(&mut self, x: usize) -> Result<(), String> {
        let signed_item = self.create_history_for_placing(x)?;
        self.seq += 1;
        self.validate_move(&signed_item)?;
        self.seq -= 1;
        Ok(())
    }

    fn create_history_for_placing(&mut self, x: usize) -> Result<HistoryItem, String> {
        let now = now();

        let data = HistoryItem {
            seq: self.seq + 1,
            now,
            x,
            signature: vec![],
        };

        let to_sign = Game::encode_history_item(&data);
        let signature = self.my_keys.sign(&to_sign);
        let mut signed_item = data.clone();
        signed_item.signature = signature.to_bytes().to_vec();
        Ok(signed_item)
    }

    fn my_turn(&self) -> bool {
        let player = self.seq % 2;
        let me_first = self.info.starting;
        (me_first && player == 1) || (!me_first && player == 0)
    }

    fn validate_move(&self, item: &HistoryItem) -> Result<(usize, usize), String> {
        if self.is_completed() {
            return Err("Game is already completed".to_string());
        }

        let (public_key, deck) = if self.my_turn() {
            (&self.my_keys.verifying_key(), &self.deck)
        } else {
            (&self.other_keys, &self.other_deck)
        };

        if self.verify {
            let to_verify = Game::encode_history_item(item);
            let signature = Signature::from_bytes(&item.signature.clone().try_into().unwrap());

            public_key
                .verify(&to_verify, &signature)
                .map_err(|_| "Invalid signature".to_string())?;
        }

        let mut item_y = 0;
        for i in 0..self.deck_size.0 {
            if deck[item.x + i * self.deck_size.1] == 0 {
                item_y = i;
                break;
            }
        }

        let pos = item.x + item_y * self.deck_size.1;
        if deck[pos] != 0 {
            return Err(format!(
                "Collision deck at {},{} already has a {}, seq {}",
                item.x, item_y, deck[pos], self.seq
            ));
        }

        Ok((item.x, pos))
    }

    fn play_move(&mut self, item: HistoryItem) -> Result<(), String> {
        let (item_x, pos) = self.validate_move(&item)?;

        let (deck, other_deck) = if self.my_turn() {
            (&mut self.deck, &mut self.other_deck)
        } else {
            (&mut self.other_deck, &mut self.deck)
        };

        let num = self.dice.roll() as u32;
        deck[pos] = num;

        let width = self.deck_size.1;
        let col_idx = item_x;
        for row_idx in 0..other_deck.len() / width {
            let idx = row_idx * width + col_idx;
            if other_deck[idx] == num {
                other_deck[idx] = 0;
            }
        }

        shift_column_values(&mut self.other_deck, self.deck_size.0, FloatDirection::Up);
        shift_column_values(&mut self.deck, self.deck_size.0, FloatDirection::Down);
        Ok(())
    }

    fn is_completed(&self) -> bool {
        self.deck.iter().all(|c| *c != 0) || self.other_deck.iter().all(|c| *c != 0)
    }

    pub fn get_board_data(&self) -> BoardData {
        let player = self.seq % 2;
        let me_first = self.info.starting;
        BoardData {
            points: Points {
                me: calculate_knucklebones_points(&self.deck, self.deck_size.0),
                other: calculate_knucklebones_points(&self.other_deck, self.deck_size.0),
            },
            decks: Decks {
                me: self.deck.clone(),
                other: self.other_deck.clone(),
            },
            history: self.history.clone(),
            seq: self.seq,
            deck_size: self.deck_size,
            next_dice: self.dice.peek() as u8,
            your_turn: !((me_first && player == 1) || (!me_first && player == 0)),
            is_completed: self.is_completed(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoardData {
    points: Points,
    decks: Decks,
    history: Vec<HistoryItem>,
    seq: u32,
    deck_size: (usize, usize),
    next_dice: u8,
    your_turn: bool,
    is_completed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Points {
    me: Vec<u32>,
    other: Vec<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Decks {
    me: Vec<u32>,
    other: Vec<u32>,
}
