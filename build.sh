#!/bin/bash
cargo build --lib -p lib_knuckle --target wasm32-unknown-unknown
wasm-bindgen ./target/wasm32-unknown-unknown/debug/lib_knuckle.wasm --target bundler --out-dir src/lib/wasmdev

RUSTFLAGS="-Zlocation-detail=none" cargo +nightly build -Z build-std=std,panic_abort -Z build-std-features=panic_immediate_abort \
     --target wasm32-unknown-unknown --profile release-wasm \
     --lib -p lib_knuckle

wasm-bindgen ./target/wasm32-unknown-unknown/release/lib_knuckle.wasm --target bundler --out-dir src/lib/wasmprd
wasm-opt -Oz --optimize-for-js -o ./src/lib/wasmprd/lib_knuckle_bg.wasm ./src/lib/wasmprd/lib_knuckle_bg.wasm
