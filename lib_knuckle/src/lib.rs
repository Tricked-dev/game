use ed25519::signature::{Keypair, SignerMut};
use ed25519::Signature;
use ed25519_dalek::Verifier;
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::RngCore;
use rand::{rngs::StdRng, SeedableRng};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
struct HistoryItem {
    seq: u32,
    now: u64,
    x: usize,
    signature: [u8; 64],
}

#[derive(Debug)]
struct ServerGameInfo {
    seed: u64,
    starter: u32,
    signature: Vec<u8>,
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
}

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
    id: u32,
}

impl Game {
    pub fn new(
        my_keys: SigningKey,
        other_keys: VerifyingKey,
        deck_size: (usize, usize),
        info: ServerGameInfo,
        id: u32,
    ) -> Self {
        let deck = vec![0; deck_size.0 * deck_size.1];
        let other_deck = vec![0; deck_size.0 * deck_size.1];
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
            id,
        }
    }

    fn encode_history_item(item: &HistoryItem) -> Vec<u8> {
        format!("{}:{}:{}", item.seq, item.now, item.x).into_bytes()
    }

    fn create_deck(&self) -> Vec<u32> {
        vec![0; self.deck_size.0 * self.deck_size.1]
    }

    pub fn add_opponent_move(&mut self, data: HistoryItem) {
        self.seq += 1;
        self.history.push(data.clone());
        self.play_move(data);
    }

    pub fn place(&mut self, x: usize) -> HistoryItem {
        self.seq += 1;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let data = HistoryItem {
            seq: self.seq,
            now,
            x,
            signature: [0; 64],
        };

        let to_sign = Game::encode_history_item(&data);
        let signature = self.my_keys.sign(&to_sign);
        let mut signed_item = data.clone();
        signed_item.signature = signature.to_bytes();

        self.history.push(signed_item.clone());
        self.play_move(signed_item.clone());
        signed_item
    }

    pub fn play_move(&mut self, item: HistoryItem) {
        let player = self.seq % 2;
        let me_first = self.info.starter == self.id;

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

        let to_verify = Game::encode_history_item(&item);
        let signature = Signature::from_bytes(&item.signature);

        public_key
            .verify(&to_verify, &signature)
            .unwrap_or_else(|_| {
                panic!("Invalid signature");
            });

        self.history.push(item.clone());

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

        for i in 0..self.deck_size.0 {
            let pos = item.x + i * self.deck_size.1;
            if other_deck[pos] == num {
                other_deck[pos] = 0;
                break;
            }
        }
    }

    pub fn get_board_data(&self) -> BoardData {
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
        }
    }
}

#[derive(Clone, Debug)]
struct BoardData {
    points: Points,
    decks: Decks,
    history: Vec<HistoryItem>,
    seq: u32,
    deck_size: (usize, usize),
    next_dice: u8,
}

#[derive(Clone, Debug)]
struct Points {
    me: Vec<u32>,
    other: Vec<u32>,
}

#[derive(Clone, Debug)]
struct Decks {
    me: Vec<u32>,
    other: Vec<u32>,
}

pub(crate) fn calculate_knucklebones_points(board: &[u32], height: usize) -> Vec<u32> {
    let multiplication_table = [
        [1, 4, 9],
        [2, 8, 18],
        [3, 12, 27],
        [4, 16, 36],
        [5, 20, 45],
        [6, 24, 54],
    ];

    let mut columns = vec![vec![]; height];
    for (i, &value) in board.iter().enumerate() {
        columns[i % height].push(value);
    }

    let mut results = Vec::new();

    for column in columns {
        let mut total = 0;
        let occ = count_occurrences(&column);

        for (&key, &value) in occ.iter() {
            if key == 0 {
                continue;
            }
            total += multiplication_table[key as usize - 1][value as usize - 1];
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
    #[test]
    fn test_game() {
        let mut csprng = OsRng;
        let my_keys = SigningKey::generate(&mut csprng);
        let other_keys = SigningKey::generate(&mut csprng);
        let deck_size = (3, 3);
        let info = ServerGameInfo {
            seed: 0,
            starter: 0,
            signature: vec![],
        };
        let mut game = Game::new(
            my_keys.clone(),
            other_keys.verifying_key(),
            deck_size,
            info,
            0,
        );
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
                starter: 0,
                signature: vec![],
            };
            let mut game = Game::new(other_keys, my_keys.verifying_key(), deck_size, info, 1);
            game.add_opponent_move(mv);
            game.place(1)
        };

        game.add_opponent_move(item);

        let info = game.get_board_data();
        assert_eq!(info.points.me, vec![0, 0, 2]);
        assert_eq!(info.points.other, vec![0, 3, 0]);
        dbg!(info);
    }
}
