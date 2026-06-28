//! Match STIX 2.1 patterns against observed STIX objects.

pub mod compare;
pub mod error;
pub mod eval;
pub mod observation;
pub mod pattern_ops;
pub mod resolve;
pub mod result;
pub mod subset;

pub use error::MatchError;
pub use observation::Observation;
pub use result::MatchResult;
