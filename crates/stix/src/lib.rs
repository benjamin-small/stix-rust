//! Umbrella crate for the **stix-rust** toolkit.
//!
//! Re-exports the parser ([`pattern`]), object model ([`model`]), and matcher
//! ([`matcher`]) so downstream code can depend on a single crate.
//!
//! # Example
//!
//! ```
//! use stix::parse;
//! use stix::matcher::match_scos;
//! use stix::model::StixObject;
//!
//! let pattern = parse("[ipv4-addr:value = '198.51.100.1']").unwrap();
//! let sco = StixObject::from_json(serde_json::json!({
//!     "type": "ipv4-addr", "id": "ipv4-addr--1", "value": "198.51.100.1"
//! })).unwrap();
//! assert!(match_scos(&pattern, &[sco]).unwrap().is_match());
//! ```

pub use stix_matcher as matcher;
pub use stix_model as model;
pub use stix_pattern as pattern;

/// Parse a STIX pattern string into an AST (re-export of [`stix_pattern::parse`]).
pub use stix_pattern::parse;

/// The high-level matching entry points.
pub use stix_matcher::{match_bundle, match_observations, match_observed_data, match_scos};
