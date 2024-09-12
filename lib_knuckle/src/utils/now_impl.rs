use std::time::{SystemTime, UNIX_EPOCH};

cfg_if::cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        pub fn now() -> u64 {
            use wasm::now_wasm;
            now_wasm() as u64
        }
    } else {
        pub fn now() -> u64 {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
        }
    }
}
