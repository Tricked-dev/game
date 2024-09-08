use base64::prelude::BASE64_STANDARD_NO_PAD;
use base64::Engine;
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
#[cfg(any(test, target_arch = "wasm32", feature = "wasm"))]
use wasm_bindgen::prelude::wasm_bindgen;

mod shift_columns;
#[cfg(any(test, target_arch = "wasm32", feature = "wasm"))]
mod wasm;

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

cfg_if::cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        fn now() -> u64 {
            use wasm::now_wasm;
            now_wasm() as u64
        }
    } else {
        fn now() -> u64 {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
        }
    }
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

    pub fn add_opponent_move(&mut self, data: HistoryItem) {
        self.seq += 1;
        self.history.push(data.clone());
        self.play_move(data);
    }

    pub fn place(&mut self, x: usize) -> HistoryItem {
        self.seq += 1;

        let now = now();

        let data = HistoryItem {
            seq: self.seq,
            now,
            x,
            signature: vec![],
        };

        let to_sign = Game::encode_history_item(&data);
        let signature = self.my_keys.sign(&to_sign);
        let mut signed_item = data.clone();
        signed_item.signature = signature.to_bytes().to_vec();

        self.history.push(signed_item.clone());
        self.play_move(signed_item.clone());
        signed_item
    }

    fn play_move(&mut self, item: HistoryItem) {
        if self.is_completed() {
            panic!("Game is already completed");
        }
        let player = self.seq % 2;
        let me_first = self.info.starting;

        let (public_key, deck, other_deck) =
            if (me_first && player == 1) || (!me_first && player == 0) {
                (
                    &self.my_keys.verifying_key(),
                    &mut self.deck,
                    &mut self.other_deck,
                )
            } else {
                (&self.other_keys, &mut self.other_deck, &mut self.deck)
            };

        if self.verify {
            let to_verify = Game::encode_history_item(&item);
            let signature = Signature::from_bytes(&item.signature.clone().try_into().unwrap());

            public_key
                .verify(&to_verify, &signature)
                .unwrap_or_else(|_| {
                    panic!("Invalid signature");
                });
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
            panic!(
                "Collision deck at {},{} already has a {}, player {}",
                item.x, item_y, deck[pos], player
            );
        }

        let num = self.dice.roll() as u32;
        deck[pos] = num;

        let width = self.deck_size.1;
        let col_idx = item.x;
        for row_idx in 0..other_deck.len() / width {
            let idx = row_idx * width + col_idx;
            if other_deck[idx] == num {
                other_deck[idx] = 0;
            }
        }

        shift_column_values(&mut self.other_deck, self.deck_size.0, FloatDirection::Up);
        shift_column_values(&mut self.deck, self.deck_size.0, FloatDirection::Down);
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

pub(crate) fn calculate_knucklebones_points(board: &[u32], width: usize) -> Vec<u32> {
    let multiplication_table = [
        [1, 4, 9],
        [2, 8, 18],
        [3, 12, 27],
        [4, 16, 36],
        [5, 20, 45],
        [6, 24, 54],
    ];

    let mut columns = vec![vec![]; width];
    for (i, &value) in board.iter().enumerate() {
        columns[i % width].push(value);
    }

    let mut results = Vec::new();

    for column in columns {
        let mut total = 0;
        let occ = count_occurrences(&column);

        for (&key, &value) in occ.iter() {
            if key == 0 {
                continue;
            }
            if key > 6 {
                return vec![];
            }
            total += multiplication_table[key as usize - 1]
                .get(value as usize - 1)
                .unwrap_or(&0);
        }

        results.push(total);
    }

    results
}

pub(crate) fn count_occurrences(arr: &[u32]) -> std::collections::HashMap<u32, u32> {
    let mut map = std::collections::HashMap::new();
    for &item in arr {
        *map.entry(item).or_insert(0) += 1;
    }
    map
}

#[cfg(test)]
mod tests {
    use rand_core::OsRng;

    use super::*;

    impl Game {
        fn mock_move(&mut self, number: u8, x: usize) {
            self.dice.set_next(number);
            self.play_move(HistoryItem {
                seq: self.seq,
                now: 0,
                x,
                signature: vec![],
            });
            self.seq += 1;
        }

        fn mock_move_nodice(&mut self, x: usize) {
            self.play_move(HistoryItem {
                seq: self.seq,
                now: 0,
                x,
                signature: vec![],
            });
            self.seq += 1;
        }

        fn debug_print_board(&self) {
            // return;
            let board = self.get_board_data();
            println!("{} mine -------", self.seq);
            let mut output = String::new();
            for (i, &x) in board.decks.me.iter().enumerate() {
                if i % board.deck_size.0 == 0 {
                    output.push('\n');
                }
                output.push_str(&x.to_string());
                output.push(' ');
            }

            println!("{}", output.trim());
            println!("{} other -------", self.seq);
            let mut output = String::new();

            for (i, &x) in board.decks.other.iter().enumerate() {
                if i % board.deck_size.0 == 0 {
                    output.push('\n');
                }
                output.push_str(&x.to_string());
                output.push(' ');
            }

            println!("{}", output.trim());
        }
    }

    #[test]
    fn test_count_occurrences() {
        let arr = vec![1, 2, 3, 4, 1, 2, 3, 4];
        let map = count_occurrences(&arr);
        assert_eq!(map[&1], 2);
        assert_eq!(map[&2], 2);
        assert_eq!(map[&3], 2);
        assert_eq!(map[&4], 2);
    }
    // Test only 3x3 since thats what we do in the game
    #[test]
    fn test_knucklebones() {
        let points = calculate_knucklebones_points(&[1, 0, 0, 0, 0, 0, 0, 0, 0], 3);
        assert_eq!(points, vec![1, 0, 0]);
        let points = calculate_knucklebones_points(&[6, 1, 2, 6, 0, 0, 6, 0, 0], 3);
        assert_eq!(points, vec![54, 1, 2]);
        let points = calculate_knucklebones_points(&[0, 0, 0, 0, 0, 0, 0, 0, 0], 3);
        assert_eq!(points, vec![0, 0, 0]);
    }

    // The comment above was a lie i want some tests for it
    #[test]
    fn test_invalid_knucklebones_inputs() {
        let points = calculate_knucklebones_points(&[1, 1, 1, 1], 1);
        assert_eq!(points, vec![0]);
        let points = calculate_knucklebones_points(&[0, 0, 0, 0, 0, 0, 0], 2);
        assert_eq!(points, vec![0, 0]);
        let points = calculate_knucklebones_points(&[7], 1);
        assert_eq!(points, vec![]);
    }
    #[test]
    fn test_is_completed() {
        let arr = vec![1].into_iter().cycle().take(9).collect::<Vec<u32>>();
        let mut csprng = OsRng;
        let my_keys = SigningKey::generate(&mut csprng);
        let other_keys = SigningKey::generate(&mut csprng);
        let deck_size = (3, 3);
        let mut game = Game::new(
            my_keys,
            other_keys.verifying_key(),
            deck_size,
            ServerGameInfo {
                seed: 0,
                starting: true,
            },
        );
        assert!(!game.get_board_data().is_completed);
        game.deck = arr.clone();
        assert!(game.get_board_data().is_completed);
        game.other_deck = arr.clone();
        assert!(game.get_board_data().is_completed);
    }

    #[test]
    fn test_game() {
        let mut csprng = OsRng;
        let my_keys = SigningKey::generate(&mut csprng);
        let other_keys = SigningKey::generate(&mut csprng);
        let deck_size = (3, 3);
        let info = ServerGameInfo {
            seed: 0,
            starting: true,
        };
        let mut game = Game::new(my_keys.clone(), other_keys.verifying_key(), deck_size, info);
        let info = game.get_board_data();
        assert_eq!(info.next_dice, 2);
        assert_eq!(info.deck_size, (3, 3));
        assert_eq!(info.seq, 0);
        assert_eq!(info.history.len(), 0);
        let mv = game.place(2);
        let item = {
            let deck_size = (3, 3);
            let info = ServerGameInfo {
                seed: 0,
                starting: false,
            };
            let mut game = Game::new(other_keys, my_keys.verifying_key(), deck_size, info);
            game.add_opponent_move(mv);
            game.place(1)
        };

        game.add_opponent_move(item);

        let info = game.get_board_data();
        let next = info.next_dice;
        assert_eq!(info.points.me, vec![0, 0, 2]);
        assert_eq!(info.points.other, vec![0, 3, 0]);

        game.place(0);
        let info = game.get_board_data();
        assert_eq!(info.points.me[0], next as u32);
    }

    #[test]
    fn test_cascading_logic() {
        let mut csprng = OsRng;
        let my_keys = SigningKey::generate(&mut csprng);
        let other_keys = SigningKey::generate(&mut csprng);
        let deck_size = (3, 3);
        let info = ServerGameInfo {
            seed: 0,
            starting: true,
        };
        let mut game = Game {
            deck: vec![0, 1, 1, 1, 1, 1, 1, 1, 1],
            other_deck: vec![1, 1, 1, 1, 1, 1, 1, 1, 1],
            deck_size,
            history: vec![],
            seq: 1,
            dice: Dice::new(0),
            my_keys,
            other_keys: other_keys.verifying_key(),
            info,
            verify: false,
        };

        game.mock_move(1, 0);
        let info = game.get_board_data();
        assert_eq!(info.decks.other[0], 0);
        assert_eq!(info.decks.other[3], 0);
        assert_eq!(info.decks.other[6], 0);

        game.mock_move(1, 0);

        let info = game.get_board_data();
        assert_eq!(info.decks.me[0], 0);
        assert_eq!(info.decks.me[3], 0);
        assert_eq!(info.decks.me[6], 0);

        game.other_deck[0] = 1;
        game.other_deck[3] = 2;
        game.other_deck[6] = 1;

        game.mock_move(1, 0);

        assert_eq!(game.other_deck[0], 2);

        game.deck = vec![];
        game.other_deck = vec![];
        let info = game.get_board_data();
        assert!(info.is_completed);
    }

    #[test]
    fn test_removing_numbers_logic() {
        let mut csprng = OsRng;
        let my_keys = SigningKey::generate(&mut csprng);
        let other_keys = SigningKey::generate(&mut csprng);
        let deck_size = (3, 3);
        let mut game = Game::new(
            my_keys,
            other_keys.verifying_key(),
            deck_size,
            ServerGameInfo {
                seed: 0,
                starting: true,
            },
        );

        game.disable_verify();

        game.mock_move(1, 2);
        game.mock_move(6, 0);
        game.mock_move(1, 1);
        game.mock_move(2, 0);
        game.mock_move(2, 1);
        game.mock_move(6, 0);
        game.mock_move(6, 0);
        let info = game.get_board_data();
        // is on 6 cause it got floated
        assert_eq!(info.decks.me[6], 2);
    }

    #[test]
    fn test_removing_numbers_logic2() {
        let mut csprng = OsRng;
        let my_keys = SigningKey::generate(&mut csprng);
        let other_keys = SigningKey::generate(&mut csprng);
        let deck_size = (3, 3);
        let mut game = Game::new(
            my_keys,
            other_keys.verifying_key(),
            deck_size,
            ServerGameInfo {
                seed: 0,
                starting: true,
            },
        );

        game.disable_verify();
        game.mock_move(1, 0);
        game.mock_move(3, 1);
        game.debug_print_board();
        game.mock_move(1, 0);
        game.mock_move(2, 1);
        game.debug_print_board();
        game.mock_move(1, 0);
        game.mock_move(3, 1);
        game.debug_print_board();
        game.mock_move(3, 1);

        let info = game.get_board_data();
        assert_eq!(info.decks.me[1], 0);
        assert_eq!(info.decks.me[4], 0);
        assert_eq!(info.decks.me[7], 2);
    }

    #[test]
    fn test_real_game() {
        //427896094
        let mut csprng = OsRng;
        let my_keys = SigningKey::generate(&mut csprng);
        let other_keys = SigningKey::generate(&mut csprng);
        let deck_size = (3, 3);
        let info = ServerGameInfo {
            seed: 427896094,
            starting: true,
        };
        let mut game = Game::new(my_keys, other_keys.verifying_key(), deck_size, info);
        game.disable_verify();

        game.mock_move_nodice(0);
        game.mock_move_nodice(0);
        game.mock_move_nodice(0);
        game.mock_move_nodice(0);
        game.mock_move_nodice(0);
        game.mock_move_nodice(0);
        game.mock_move_nodice(0);
        game.mock_move_nodice(0);
        game.mock_move_nodice(1);
        game.mock_move_nodice(1);
        game.mock_move_nodice(1);
        game.mock_move_nodice(1);
        game.mock_move_nodice(1);
        game.mock_move_nodice(2);
        game.mock_move_nodice(2);
        game.mock_move_nodice(1);
        game.mock_move_nodice(1);
    }

    #[test]
    fn test_real_game2() {
        let mut csprng = OsRng;
        let my_keys = SigningKey::generate(&mut csprng);
        let other_keys = SigningKey::generate(&mut csprng);
        let deck_size = (3, 3);
        let info = ServerGameInfo {
            seed: 1282226401,
            starting: true,
        };
        let mut game = Game::new(my_keys, other_keys.verifying_key(), deck_size, info);
        game.disable_verify();

        game.mock_move_nodice(0);
        game.mock_move_nodice(0);
        game.mock_move_nodice(1);
        game.mock_move_nodice(1);
        game.mock_move_nodice(0);
        game.mock_move_nodice(0);
    }
}
