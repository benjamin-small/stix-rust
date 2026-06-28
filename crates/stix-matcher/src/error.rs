//! Error type for the matcher.

use thiserror::Error;

/// Errors produced while matching a pattern against observations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MatchError {
    /// A pattern feature is parsed but not yet supported by the matcher
    /// (e.g. `FOLLOWEDBY` sequencing or temporal qualifiers).
    #[error("unsupported pattern feature: {0}")]
    Unsupported(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_displays_feature() {
        let e = MatchError::Unsupported("FOLLOWEDBY".to_string());
        assert!(format!("{e}").contains("FOLLOWEDBY"));
    }
}
