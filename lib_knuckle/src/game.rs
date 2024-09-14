use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
#[cfg(any(test, target_arch = "wasm32", feature = "wasm"))]
use wasm_bindgen::prelude::wasm_bindgen;

use crate::{
    dice::Dice,
    keys::Keys,
    shift_columns::{shift_column_values, FloatDirection},
    utils::{knucklebones_points::calculate_knucklebones_points, now_impl::now},
};

#[cfg_attr(any(test, target_arch = "wasm32", feature = "wasm"), wasm_bindgen)]
pub struct Game {
    history: Vec<HistoryItem>,
    deck: Vec<u32>,
    other_deck: Vec<u32>,
    seq: u32,
    dice: Dice,
    deck_size: (usize, usize),
    info: ServerGameInfo,
    verify: bool,
    keys: Keys,
}

impl Game {
    pub fn new(keys: Keys, deck_size: (usize, usize), info: ServerGameInfo) -> Self {
        let deck = Self::create_deck(deck_size);
        let other_deck = Self::create_deck(deck_size);
        let dice = Dice::new(info.seed);

        Game {
            history: Vec::new(),
            deck,
            other_deck,
            seq: 0,
            dice,
            keys,
            deck_size,
            info,
            verify: true,
        }
    }
    pub fn validate_entire_game(
        my_keys: VerifyingKey,
        other_keys: VerifyingKey,
        deck_size: (usize, usize),
        info: ServerGameInfo,
        history: Vec<HistoryItem>,
    ) -> Result<(), String> {
        let mut game = Game::new(
            Keys::VerifyOnly {
                my_keys,
                other_keys,
            },
            deck_size,
            info,
        );
        for item in history {
            game.add_opponent_move(item)?;
        }
        Ok(())
    }

    pub fn disable_verify(&mut self) {
        self.verify = false;
    }

    pub(crate) fn encode_history_item(item: &HistoryItem) -> Vec<u8> {
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

    pub fn place(&mut self, x: u16) -> Result<HistoryItem, String> {
        let signed_item = self.create_history_for_placing(x)?;

        self.seq += 1;
        self.history.push(signed_item.clone());
        self.play_move(signed_item.clone())?;
        Ok(signed_item)
    }

    pub fn test_place(&mut self, x: u16) -> Result<(), String> {
        let signed_item = self.create_history_for_placing(x)?;
        self.seq += 1;
        self.validate_move(&signed_item)?;
        self.seq -= 1;
        Ok(())
    }

    fn create_history_for_placing(&mut self, x: u16) -> Result<HistoryItem, String> {
        let now = now();

        let data = HistoryItem {
            seq: self.seq + 1,
            now,
            x,
            signature: vec![],
        };

        let to_sign = Game::encode_history_item(&data);
        let key = self.keys.my_sign().unwrap();
        let signature = key.sign(&to_sign);
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
            (&self.keys.my_verify(), &self.deck)
        } else {
            (&self.keys.other_verify(), &self.other_deck)
        };

        if self.verify {
            let to_verify = Game::encode_history_item(item);
            let signature =
                Signature::from_bytes(&item.signature.clone().try_into().unwrap());

            public_key
                .verify(&to_verify, &signature)
                .map_err(|_| "Invalid signature".to_string())?;
        }

        let mut item_y = 0;
        let item_x = item.x as usize;
        for i in 0..self.deck_size.0 {
            if deck[item_x + i * self.deck_size.1] == 0 {
                item_y = i;
                break;
            }
        }

        let pos = item_x + item_y * self.deck_size.1;
        if deck[pos] != 0 {
            return Err(format!(
                "Collision deck at {},{} already has a {}, seq {}",
                item_x, item_y, deck[pos], self.seq
            ));
        }

        Ok((item_x, pos))
    }
    pub fn forfeit(&mut self) -> HistoryItem {
        let signed_item = self.create_history_for_placing(u16::MAX).unwrap();
        self.seq += 1;
        self.history.push(signed_item.clone());
        signed_item
    }

    fn is_valid_signature(&self, item: &HistoryItem) -> Result<(), String> {
        let signature = item.signature.clone();
        let signature = signature.try_into().unwrap();
        let signature = Signature::from_bytes(&signature);
        let key = self.keys.my_verify();
        let encoded = Self::encode_history_item(item);
        match key.verify_strict(&encoded, &signature) {
            Ok(_) => Ok(()),
            Err(_) => {
                match self.keys.other_verify().verify_strict(&encoded, &signature) {
                    Ok(_) => Ok(()),
                    Err(_) => Err("Invalid signature".to_string()),
                }
            }
        }
    }

    fn play_move(&mut self, item: HistoryItem) -> Result<(), String> {
        if item.is_forfeit() {
            if self.verify {
                self.is_valid_signature(&item)?
            }
            return Ok(());
        }
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
        match self.history.last() {
            Some(item) if item.is_forfeit() => return true,
            _ => {}
        }
        self.deck.iter().all(|c| *c != 0) || self.other_deck.iter().all(|c| *c != 0)
    }

    pub fn get_board_data(&self) -> BoardData {
        let player = self.seq % 2;
        let me_first = self.info.starting;
        let me_points = calculate_knucklebones_points(&self.deck, self.deck_size.0);
        let other_points =
            calculate_knucklebones_points(&self.other_deck, self.deck_size.0);
        let your_turn = !((me_first && player == 1) || (!me_first && player == 0));
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
            your_turn,
            is_completed: self.is_completed(),
            winner: match self.history.last() {
                Some(item) if item.is_forfeit() => {
                    let to_verify = Game::encode_history_item(item);
                    let signature = Signature::from_bytes(
                        &item.signature.clone().try_into().unwrap(),
                    );

                    let is_from_me = self.keys.my_verify().verify(&to_verify, &signature);
                    dbg!(&is_from_me);
                    GameEnd {
                        win_by_tie: false,
                        win_by_forfeit: true,
                        winner: is_from_me.is_err(),
                    }
                }
                _ => match (me_points, other_points) {
                    (me, other) if me > other => GameEnd {
                        win_by_tie: false,
                        win_by_forfeit: false,
                        winner: true,
                    },
                    (other, me) if me < other => GameEnd {
                        win_by_tie: false,
                        win_by_forfeit: false,
                        winner: false,
                    },
                    _ => GameEnd {
                        win_by_tie: true,
                        win_by_forfeit: false,
                        winner: false,
                    },
                },
            },
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(
    any(test, target_arch = "wasm32", feature = "wasm"),
    derive(tsify::Tsify)
)]
#[cfg_attr(
    any(test, target_arch = "wasm32", feature = "wasm"),
    tsify(into_wasm_abi, from_wasm_abi)
)]
struct GameEnd {
    win_by_tie: bool,
    win_by_forfeit: bool,
    winner: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(
    any(test, target_arch = "wasm32", feature = "wasm"),
    derive(tsify::Tsify)
)]
#[cfg_attr(
    any(test, target_arch = "wasm32", feature = "wasm"),
    tsify(into_wasm_abi, from_wasm_abi)
)]
pub struct BoardData {
    points: Points,
    decks: Decks,
    history: Vec<HistoryItem>,
    seq: u32,
    deck_size: (usize, usize),
    next_dice: u8,
    your_turn: bool,
    is_completed: bool,
    winner: GameEnd,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(
    any(test, target_arch = "wasm32", feature = "wasm"),
    derive(tsify::Tsify)
)]
#[cfg_attr(
    any(test, target_arch = "wasm32", feature = "wasm"),
    tsify(into_wasm_abi, from_wasm_abi)
)]
pub struct Points {
    me: Vec<u32>,
    other: Vec<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(
    any(test, target_arch = "wasm32", feature = "wasm"),
    derive(tsify::Tsify)
)]
#[cfg_attr(
    any(test, target_arch = "wasm32", feature = "wasm"),
    tsify(into_wasm_abi, from_wasm_abi)
)]
pub struct Decks {
    me: Vec<u32>,
    other: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(
    any(test, target_arch = "wasm32", feature = "wasm"),
    derive(tsify::Tsify)
)]
#[cfg_attr(
    any(test, target_arch = "wasm32", feature = "wasm"),
    tsify(into_wasm_abi, from_wasm_abi)
)]
pub struct HistoryItem {
    seq: u32,
    now: u64,
    x: u16,
    pub(crate) signature: Vec<u8>,
}

impl HistoryItem {
    pub fn is_forfeit(&self) -> bool {
        self.x == u16::MAX
    }
}

#[derive(Debug)]
pub struct ServerGameInfo {
    pub(crate) seed: u64,
    pub(crate) starting: bool,
}

#[cfg(test)]
mod tests {
    use rand_core::OsRng;

    use super::*;

    impl Game {
        fn mock_move(&mut self, number: u8, x: u16) {
            self.dice.set_next(number);
            self.play_move(HistoryItem {
                seq: self.seq,
                now: 0,
                x,
                signature: vec![],
            })
            .unwrap();
            self.seq += 1;
        }

        fn mock_move_nodice(&mut self, x: u16) {
            self.play_move(HistoryItem {
                seq: self.seq,
                now: 0,
                x,
                signature: vec![],
            })
            .unwrap();
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

    fn create_test_game(seed: u64) -> Game {
        let mut csprng = OsRng;
        let my_keys = SigningKey::generate(&mut csprng);
        let other_keys = SigningKey::generate(&mut csprng);
        let deck_size = (3, 3);
        Game::new(
            Keys::Sign {
                my_keys,
                other_keys: other_keys.verifying_key(),
            },
            deck_size,
            ServerGameInfo {
                seed,
                starting: true,
            },
        )
    }

    #[test]
    fn test_is_completed() {
        let mut game = create_test_game(0);
        let arr = vec![1].into_iter().cycle().take(9).collect::<Vec<u32>>();

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
        let mut game = Game::new(
            Keys::Sign {
                my_keys: my_keys.clone(),
                other_keys: other_keys.verifying_key(),
            },
            deck_size,
            info,
        );
        let info = game.get_board_data();
        assert_eq!(info.next_dice, 2);
        assert_eq!(info.deck_size, (3, 3));
        assert_eq!(info.seq, 0);
        assert_eq!(info.history.len(), 0);
        let mv = game.place(2).unwrap();
        let item = {
            let deck_size = (3, 3);
            let info = ServerGameInfo {
                seed: 0,
                starting: false,
            };
            let mut game = Game::new(
                Keys::Sign {
                    my_keys: other_keys,
                    other_keys: my_keys.verifying_key(),
                },
                deck_size,
                info,
            );
            game.add_opponent_move(mv).unwrap();
            game.place(1).unwrap()
        };

        game.add_opponent_move(item).unwrap();

        let info = game.get_board_data();
        let next = info.next_dice;
        assert_eq!(info.points.me, vec![0, 0, 2]);
        assert_eq!(info.points.other, vec![0, 3, 0]);

        game.place(0).unwrap();
        let info = game.get_board_data();
        assert_eq!(info.points.me[0], next as u32);
    }

    #[test]
    fn test_cascading_logic() {
        let mut game = create_test_game(0);
        game.disable_verify();

        //TODO: in some future or slomething remove this if possible!
        #[cfg_attr(rustfmt, rustfmt_skip)]
        {
            game.deck = vec![
                1, 1, 1,
                1, 1, 1,
                1, 1, 0];
            game.other_deck = vec![
                0, 1, 1,
                1, 1, 1,
                1, 0, 0];
        }

        game.mock_move(1, 0);
        let info = game.get_board_data();
        assert_eq!(info.decks.me[0], 0);
        assert_eq!(info.decks.me[3], 0);
        assert_eq!(info.decks.me[6], 0);

        game.mock_move(1, 0);

        let info = game.get_board_data();
        assert_eq!(info.decks.other[0], 0);
        assert_eq!(info.decks.other[3], 0);
        assert_eq!(info.decks.other[6], 0);

        game.deck[0] = 1;
        game.deck[3] = 2;
        game.deck[6] = 1;

        game.mock_move(1, 0);

        //Floated upwards!
        assert_eq!(game.deck[6], 2);

        game.deck = vec![];
        game.other_deck = vec![];
        let info = game.get_board_data();
        assert!(info.is_completed);
    }

    #[test]
    fn test_removing_numbers_logic() {
        let mut game = create_test_game(0);
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
        let mut game = create_test_game(0);
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
        let mut game = create_test_game(427896094);
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
        let mut game = create_test_game(1282226401);
        game.disable_verify();

        game.mock_move_nodice(0);
        game.mock_move_nodice(0);
        game.mock_move_nodice(1);
        game.mock_move_nodice(1);
        game.mock_move_nodice(0);
        game.mock_move_nodice(0);
    }

    #[test]
    fn test_forfeit() {
        let mut game = create_test_game(0);
        game.disable_verify();
        let item = game.forfeit();
        assert_eq!(game.history.len(), 1);
        assert!(game.history[0].is_forfeit());
        assert!(!game.get_board_data().winner);
        assert!(game.get_board_data().is_completed);
        assert!(!game.get_board_data().your_turn);
        let mut game2 = create_test_game(0);
        game2.info.starting = false;
        game2.disable_verify();
        game2.add_opponent_move(item).unwrap();
        assert!(game2.get_board_data().winner);
        assert!(game2.get_board_data().is_completed);
    }
}
