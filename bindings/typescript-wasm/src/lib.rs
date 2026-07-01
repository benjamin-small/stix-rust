//! WebAssembly bindings for the stix-rust toolkit (raw wasm-bindgen layer).
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn _healthcheck() -> bool {
    true
}
