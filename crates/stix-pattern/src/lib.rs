//! Lexer and parser for the STIX 2.1 patterning language.
//!
//! # Example
//!
//! ```
//! use stix_pattern::parse;
//!
//! let pattern = parse("[file:hashes.'SHA-256' = 'abc']").unwrap();
//! let json = serde_json::to_string(&pattern).unwrap();
//! assert!(json.contains("SHA-256"));
//! ```

pub mod ast;
pub mod error;
pub mod lexer;
pub mod parser;

pub use ast::*;
pub use error::{ParseError, Span};
pub use parser::parse;
