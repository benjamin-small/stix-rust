//! FFI-friendly facade over the stix toolkit.
//!
//! Pure Rust (no FFI macros). The language bindings each wrap this surface.

pub mod engine;
pub mod error;
pub mod handles;

pub use engine::Engine;
pub use error::{ErrorCode, FfiError};
pub use handles::{Bundle, MatchOutcome, Pattern};
