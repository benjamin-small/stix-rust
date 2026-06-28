//! Error type for the object model.

use thiserror::Error;

/// Errors produced while importing or interpreting STIX objects.
#[derive(Debug, Error)]
pub enum ModelError {
    /// The JSON could not be parsed.
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),

    /// A required property was missing or had the wrong type.
    #[error("invalid STIX object: {0}")]
    InvalidObject(String),

    /// The input was not a STIX bundle.
    #[error("not a STIX bundle: {0}")]
    NotABundle(String),
}

/// Convenience alias for results in this crate.
pub type Result<T> = std::result::Result<T, ModelError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_object_displays_message() {
        let e = ModelError::InvalidObject("missing id".to_string());
        assert!(format!("{e}").contains("missing id"));
    }

    #[test]
    fn json_error_converts() {
        let json_err = serde_json::from_str::<serde_json::Value>("{bad").unwrap_err();
        let e: ModelError = json_err.into();
        assert!(matches!(e, ModelError::Json(_)));
    }
}
