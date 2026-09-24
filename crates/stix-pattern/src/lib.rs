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
//!
//! Patterns can also be lowered to a three-address [`ir::Program`] and rendered
//! back to canonical pattern text:
//!
//! ```
//! use stix_pattern::{ir, parse};
//!
//! let pattern = parse("[file:size>1024]").unwrap();
//! let program = ir::lower(&pattern);
//! assert_eq!(ir::render(&program), "[file:size > 1024]");
//! ```

#![warn(missing_docs)]

pub mod ast;
pub mod error;
pub mod ir;
pub mod lexer;
pub mod parser;

pub use ast::*;
pub use error::{ParseError, Span};
pub use parser::parse;
