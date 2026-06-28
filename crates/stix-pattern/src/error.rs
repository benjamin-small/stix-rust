//! Error and source-span types for parsing.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A half-open byte range `[start, end)` into the original pattern string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Span { start, end }
    }
}

/// An error produced while lexing or parsing a STIX pattern.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("parse error at bytes {}..{}: {message}", .span.start, .span.end)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl ParseError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        ParseError {
            message: message.into(),
            span,
        }
    }
}

/// Convenience alias for results in this crate.
pub type Result<T> = std::result::Result<T, ParseError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_records_offsets() {
        let span = Span::new(3, 7);
        assert_eq!(span.start, 3);
        assert_eq!(span.end, 7);
    }

    #[test]
    fn parse_error_displays_with_span() {
        let err = ParseError::new("unexpected token", Span::new(5, 6));
        let msg = format!("{err}");
        assert!(msg.contains("unexpected token"), "got: {msg}");
        assert!(msg.contains('5'), "span start should appear: {msg}");
    }
}
