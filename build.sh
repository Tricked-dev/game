cd lib_knuckle
cargo build --target wasm32-unknown-unknown
cd ..
wasm-bindgen ./target/wasm32-unknown-unknown/debug/lib_knuckle.wasm --target bundler --out-dir src/lib/wasm