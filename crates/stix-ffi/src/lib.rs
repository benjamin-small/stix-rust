//! FFI-friendly facade over the stix toolkit.
//!
//! Pure Rust (no FFI macros). The language bindings each wrap this surface:
//! an [`Engine`] parses patterns and bundles into opaque [`Pattern`]/[`Bundle`]
//! handles and runs matches, returning a [`MatchOutcome`]; deep structure (the AST,
//! object properties) crosses as JSON.
//!
//! ```
//! use stix_ffi::Engine;
//!
//! let engine = Engine::new();
//! let pattern = engine.parse_pattern("[ipv4-addr:value = '198.51.100.5']").unwrap();
//! let bundle = engine.parse_bundle(r#"{"type":"bundle","objects":[
//!     {"type":"ipv4-addr","id":"ipv4-addr--1","value":"198.51.100.5"},
//!     {"type":"observed-data","id":"observed-data--1",
//!      "first_observed":"2020-01-01T00:00:00Z","last_observed":"2020-01-01T00:00:00Z",
//!      "number_observed":1,"object_refs":["ipv4-addr--1"]}
//! ]}"#).unwrap();
//! assert!(engine.match_bundle(&pattern, &bundle).unwrap().matched);
//! ```

pub mod engine;
pub mod error;
pub mod handles;

pub use engine::Engine;
pub use error::{ErrorCode, FfiError};
pub use handles::{Bundle, MatchOutcome, Pattern};
