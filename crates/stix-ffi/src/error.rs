//! The facade's flat error type, mappable onto host-language exceptions.

/// A coarse category each binding maps to its own exception type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    Parse,
    Model,
    Match,
    Validation,
}

/// A flat, FFI-friendly error: a category plus a human-readable message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfiError {
    pub code: ErrorCode,
    pub message: String,
}

impl FfiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        FfiError {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for FfiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for FfiError {}

impl From<stix::pattern::ParseError> for FfiError {
    fn from(e: stix::pattern::ParseError) -> Self {
        FfiError::new(ErrorCode::Parse, e.to_string())
    }
}

impl From<stix::model::ModelError> for FfiError {
    fn from(e: stix::model::ModelError) -> Self {
        FfiError::new(ErrorCode::Model, e.to_string())
    }
}

impl From<stix::matcher::MatchError> for FfiError {
    fn from(e: stix::matcher::MatchError) -> Self {
        FfiError::new(ErrorCode::Match, e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_error_maps_to_parse_code() {
        let e = stix::parse("[bad").unwrap_err();
        let f: FfiError = e.into();
        assert_eq!(f.code, ErrorCode::Parse);
        assert!(!f.message.is_empty());
    }

    #[test]
    fn display_includes_code_and_message() {
        let f = FfiError::new(ErrorCode::Validation, "missing field");
        let s = format!("{f}");
        assert!(s.contains("Validation"));
        assert!(s.contains("missing field"));
    }
}
