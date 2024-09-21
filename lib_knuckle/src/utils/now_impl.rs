cfg_if::cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        use wasm_bindgen::prelude::wasm_bindgen;
        mod wasm {
            use wasm_bindgen::prelude::wasm_bindgen;
            #[wasm_bindgen]
            extern "C" {
                #[wasm_bindgen(js_namespace = Date)]
                pub fn now() -> f64;
            }
        }
        pub fn now() -> u64 {
            wasm::now() as u64
        }
    } else {
        use std::time::{SystemTime, UNIX_EPOCH};
        pub fn now() -> u64 {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_now() {
        let now = now();
        println!("now: {}", now);
        assert!(now > 0);
    }
}
