use base64::prelude::BASE64_STANDARD_NO_PAD;
use base64::Engine;
use ed25519::signature::SignerMut;
use ed25519_dalek::{SigningKey, VerifyingKey};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

use crate::game::{Game, HistoryItem, ServerGameInfo};
use crate::signing_key_from_string;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
    #[wasm_bindgen(js_namespace = Date, js_name = now)]
    pub fn now_wasm() -> u32;
}

#[allow(unused)]
macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
pub fn sign_message(key: String, message: String) -> String {
    #[cfg(feature = "debug")]
    console_error_panic_hook::set_once();

    let mut my_keys = signing_key_from_string(key).unwrap();
    let signature = my_keys.sign(message.as_bytes());
    let signature = signature.to_vec();
    BASE64_STANDARD_NO_PAD.encode(signature)
}

#[wasm_bindgen]
impl Game {
    #[wasm_bindgen(constructor)]
    pub fn w_new(
        my_key_pub: String,
        my_key_priv: String,
        other_key_pub: String,
        deck_x: usize,
        deck_y: usize,
        starting: bool,
        seed: u64,
    ) -> Self {
        #[cfg(feature = "debug")]
        console_error_panic_hook::set_once();

        let my_keys = SigningKey::from_bytes(
            &BASE64_STANDARD_NO_PAD
                .decode(my_key_priv)
                .unwrap()
                .try_into()
                .unwrap(),
        );

        assert_eq!(
            my_keys.verifying_key(),
            VerifyingKey::from_bytes(
                &BASE64_STANDARD_NO_PAD
                    .decode(my_key_pub)
                    .unwrap()
                    .try_into()
                    .unwrap()
            )
            .unwrap()
        );

        let other_keys = VerifyingKey::from_bytes(
            &BASE64_STANDARD_NO_PAD
                .decode(other_key_pub)
                .unwrap()
                .try_into()
                .unwrap(),
        )
        .unwrap();

        Self::new(
            my_keys,
            other_keys,
            (deck_x, deck_y),
            ServerGameInfo { seed, starting },
        )
    }

    pub fn w_add_opponent_move(&mut self, data: Vec<u8>) -> Option<String> {
        let item = match bincode::deserialize::<HistoryItem>(&data) {
            Ok(item) => item,
            Err(e) => return Some(e.to_string()),
        };

        match self.add_opponent_move(item) {
            Err(e) => Some(e),
            Ok(_) => None,
        }
    }

    pub fn w_test_place(&mut self, x: usize) -> Option<String> {
        let item = self.test_place(x);
        match item {
            Err(e) => Some(e),
            Ok(_) => None,
        }
    }

    pub fn w_place(&mut self, x: usize) -> Vec<u8> {
        let item = self.place(x).unwrap();

        let wasm = bincode::serialize(&item).unwrap();
        console_log!("Sending Bytes {:?}", wasm);
        console_log!("Sending Length {:?}", wasm.len());
        console_log!(
            "Reparse result: {:?}",
            bincode::deserialize::<HistoryItem>(&wasm)
        );
        wasm
    }

    pub fn w_get_board_data(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&self.get_board_data()).unwrap()
    }
}
