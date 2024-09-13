use ed25519::{signature::SignerMut, Signature};
use ed25519_dalek::{SigningKey, Verifier, VerifyingKey};
use rand::{rngs::StdRng, RngCore, SeedableRng};
use serde::{Deserialize, Serialize};
use shift_columns::{shift_column_values, FloatDirection};
use std::time::{SystemTime, UNIX_EPOCH};
use utils::knucklebones_points::calculate_knucklebones_points;
#[cfg(any(test, target_arch = "wasm32", feature = "wasm"))]
use wasm_bindgen::prelude::wasm_bindgen;

mod shift_columns;
#[cfg(any(test, target_arch = "wasm32", feature = "wasm"))]
mod wasm;
#[cfg(any(test, target_arch = "wasm32", feature = "wasm"))]
pub use wasm::*;

pub mod game;
mod utils;

mod dice;
mod keys;

pub use utils::signing_helpers::*;
