//! FFI-friendly facade over the stix toolkit.
//!
//! Pure Rust (no FFI macros). The language bindings each wrap this surface.

pub mod error;

pub use error::{ErrorCode, FfiError};
