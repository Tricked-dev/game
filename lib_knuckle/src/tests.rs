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
        })
        .unwrap();
        self.seq += 1;
    }

    fn mock_move_nodice(&mut self, x: usize) {
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
    let mv = game.place(2).unwrap();
    let item = {
        let deck_size = (3, 3);
        let info = ServerGameInfo {
            seed: 0,
            starting: false,
        };
        let mut game = Game::new(other_keys, my_keys.verifying_key(), deck_size, info);
        game.add_opponent_move(mv);
        game.place(1).unwrap()
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
    // let mut csprng = OsRng;
    // let my_keys = SigningKey::generate(&mut csprng);
    // let other_keys = SigningKey::generate(&mut csprng);
    // let deck_size = (3, 3);
    // let info = ServerGameInfo {
    //     seed: 0,
    //     starting: true,
    // };
    // let mut game = Game {
    //     deck: vec![0, 1, 1, 1, 1, 1, 1, 1, 1],
    //     other_deck: vec![1, 1, 1, 1, 1, 1, 1, 1, 1],
    //     deck_size,
    //     history: vec![],
    //     seq: 1,
    //     dice: Dice::new(0),
    //     my_keys,
    //     other_keys: other_keys.verifying_key(),
    //     info,
    //     verify: false,
    // };

    // game.mock_move(1, 0);
    // let info = game.get_board_data();
    // assert_eq!(info.decks.other[0], 0);
    // assert_eq!(info.decks.other[3], 0);
    // assert_eq!(info.decks.other[6], 0);

    // game.mock_move(1, 0);

    // let info = game.get_board_data();
    // assert_eq!(info.decks.me[0], 0);
    // assert_eq!(info.decks.me[3], 0);
    // assert_eq!(info.decks.me[6], 0);

    // game.other_deck[0] = 1;
    // game.other_deck[3] = 2;
    // game.other_deck[6] = 1;

    // game.mock_move(1, 0);

    // assert_eq!(game.other_deck[0], 2);

    // game.deck = vec![];
    // game.other_deck = vec![];
    // let info = game.get_board_data();
    // assert!(info.is_completed);
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
