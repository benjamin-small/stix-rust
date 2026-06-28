//! STIX specification version seam.

use serde::{Deserialize, Serialize};

/// The STIX spec version a document conforms to. Phase 1 targets 2.1; the enum
/// exists so version-specific behavior can be added without API churn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SpecVersion {
    #[default]
    #[serde(rename = "2.1")]
    V2_1,
}

impl SpecVersion {
    /// The canonical version string (e.g. `"2.1"`).
    pub fn as_str(&self) -> &'static str {
        match self {
            SpecVersion::V2_1 => "2.1",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_2_1() {
        assert_eq!(SpecVersion::default(), SpecVersion::V2_1);
        assert_eq!(SpecVersion::default().as_str(), "2.1");
    }
}
