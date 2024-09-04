cargo build --lib -p lib_knuckle
wasm-bindgen ./target/wasm32-unknown-unknown/debug/lib_knuckle.wasm --target bundler --out-dir src/lib/wasm