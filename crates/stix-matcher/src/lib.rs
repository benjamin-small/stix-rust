//! Match STIX 2.1 patterns against observed STIX objects.

pub mod error;
pub mod result;

pub use error::MatchError;
pub use result::MatchResult;
