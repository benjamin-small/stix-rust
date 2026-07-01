//! Native Node bindings for the stix-rust toolkit (raw napi layer).
#![deny(clippy::all)]

use napi_derive::napi;

#[napi]
pub fn _healthcheck() -> bool {
    true
}
