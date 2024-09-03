use ed25519::signature::Keypair;
use ed25519::Signature;
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::RngCore;
use rand::{rngs::StdRng, SeedableRng};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
struct HistoryItem {
    seq: u32,
    now: u64,
    x: usize,
    signature: Vec<u8>,
}

#[derive(Debug)]
struct ServerGameInfo {
    seed: u64,
    starter: u32,
    signature: Vec<u8>,
}

struct Game {
    history: Vec<HistoryItem>,
    rng: StdRng,
    deck: Vec<u32>,
    other_deck: Vec<u32>,
    seq: u32,
    next_dice: u32,
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
        let mut rng = StdRng::seed_from_u64(info.seed);
        let deck = vec![0; deck_size.0 * deck_size.1];
        let other_deck = vec![0; deck_size.0 * deck_size.1];
        let next_dice = rng.next_u32();

        Game {
            history: Vec::new(),
            rng,
            deck,
            other_deck,
            seq: 0,
            next_dice,
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

    pub async fn add_opponent_move(&mut self, data: HistoryItem) {
        self.seq += 1;
        self.history.push(data.clone());
        self.play_move(data).await;
    }

    pub async fn place(&mut self, x: usize) -> HistoryItem {
        self.seq += 1;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let data = HistoryItem {
            seq: self.seq,
            now,
            x,
            signature: Vec::new(),
        };

        let to_sign = Game::encode_history_item(&data);
        let signature = self.my_keys.sign(&to_sign);
        let mut signed_item = data.clone();
        signed_item.signature = signature.to_bytes().to_vec();

        self.history.push(signed_item.clone());
        self.play_move(signed_item.clone()).await;
        signed_item
    }

    async fn play_move(&mut self, item: HistoryItem) {
        let player = self.seq % 2;
        let me_first = self.info.starter == self.id;

        let (public_key, deck, other_deck) =
            if (me_first && player == 1) || (!me_first && player == 0) {
                (&self.my_keys.public, &mut self.deck, &mut self.other_deck)
            } else {
                (&self.other_keys, &mut self.other_deck, &mut self.deck)
            };

        let to_verify = Game::encode_history_item(&item);
        let signature = Signature::from_bytes(&item.signature).unwrap();

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

        let num = self.next_dice;
        deck[pos] = num;

        for i in 0..self.deck_size.0 {
            let pos = item.x + i * self.deck_size.1;
            if other_deck[pos] == num {
                other_deck[pos] = 0;
                break;
            }
        }
        self.next_dice = self.rng.next_u32();
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
            next_dice: self.next_dice,
        }
    }
}

struct BoardData {
    points: Points,
    decks: Decks,
    history: Vec<HistoryItem>,
    seq: u32,
    deck_size: (usize, usize),
    next_dice: u32,
}

struct Points {
    me: Vec<u32>,
    other: Vec<u32>,
}

struct Decks {
    me: Vec<u32>,
    other: Vec<u32>,
}

fn calculate_knucklebones_points(board: &[u32], height: usize) -> Vec<u32> {
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
            total += multiplication_table[key as usize - 1][value as usize - 1];
        }

        results.push(total);
    }

    results
}

fn count_occurrences(arr: &[u32]) -> std::collections::HashMap<u32, u32> {
    let mut map = std::collections::HashMap::new();
    for &item in arr {
        *map.entry(item).or_insert(0) += 1;
    }
    map
}
