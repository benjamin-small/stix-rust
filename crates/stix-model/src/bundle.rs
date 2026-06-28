//! The STIX `bundle` container.

use serde::{Deserialize, Serialize};

use crate::error::{ModelError, Result};
use crate::object::StixObject;

/// A STIX bundle: a `type: "bundle"` envelope around a list of objects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    pub objects: Vec<StixObject>,
}

impl Bundle {
    /// Parse a bundle from a JSON string, validating the `type` is `bundle`.
    pub fn from_json_str(s: &str) -> Result<Self> {
        let bundle: Bundle = serde_json::from_str(s)?;
        if bundle.type_ != "bundle" {
            return Err(ModelError::NotABundle(format!(
                "type was '{}'",
                bundle.type_
            )));
        }
        Ok(bundle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::ObjectView;

    fn bundle_json() -> &'static str {
        r#"{
            "type": "bundle",
            "id": "bundle--1",
            "objects": [
                {"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.2.3.4"},
                {"type": "observed-data", "id": "observed-data--1",
                 "first_observed": "2020-01-01T00:00:00Z",
                 "last_observed": "2020-01-01T00:00:00Z",
                 "number_observed": 1, "object_refs": ["ipv4-addr--1"]}
            ]
        }"#
    }

    #[test]
    fn parses_bundle() {
        let b = Bundle::from_json_str(bundle_json()).unwrap();
        assert_eq!(b.id.as_deref(), Some("bundle--1"));
        assert_eq!(b.objects.len(), 2);
        assert_eq!(b.objects[0].type_(), Some("ipv4-addr"));
        assert_eq!(b.objects[1].type_(), Some("observed-data"));
    }

    #[test]
    fn rejects_non_bundle() {
        let err = Bundle::from_json_str(r#"{"type": "ipv4-addr", "id": "x--1"}"#).unwrap_err();
        assert!(matches!(err, crate::error::ModelError::NotABundle(_)));
    }

    #[test]
    fn round_trips() {
        let b = Bundle::from_json_str(bundle_json()).unwrap();
        let s = serde_json::to_string(&b).unwrap();
        let b2 = Bundle::from_json_str(&s).unwrap();
        assert_eq!(b.objects.len(), b2.objects.len());
    }
}
