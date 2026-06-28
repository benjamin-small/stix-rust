//! Typed STIX Domain Objects. Phase 1 implements `observed-data`; more are additive.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::value::StixValue;
use crate::view::ObjectView;

/// The STIX `observed-data` SDO.
///
/// Carries the temporal fields and `object_refs` the matcher needs. Unknown or
/// custom properties are retained in `additional` (via `#[serde(flatten)]`) so the
/// `ObjectView` still exposes them. Tolerates STIX 2.0 inline `objects` as well as
/// 2.1 `object_refs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservedData {
    #[serde(rename = "type")]
    pub type_: String,
    pub id: String,
    pub first_observed: String,
    pub last_observed: String,
    pub number_observed: u64,
    #[serde(default)]
    pub object_refs: Vec<String>,
    /// STIX 2.0 inline observed objects (`objects`), if present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objects: Option<BTreeMap<String, StixValue>>,
    /// Any other properties, retained for `ObjectView` and round-tripping.
    #[serde(flatten)]
    pub additional: BTreeMap<String, StixValue>,
}

impl ObservedData {
    /// The referenced SCO ids. Prefers 2.1 `object_refs`; otherwise empty.
    /// (2.0 inline `objects` are keyed locally, not by id, so they yield no refs.)
    pub fn sco_ids(&self) -> Vec<&str> {
        self.object_refs.iter().map(String::as_str).collect()
    }
}

impl ObjectView for ObservedData {
    fn id(&self) -> Option<&str> {
        Some(&self.id)
    }

    fn type_(&self) -> Option<&str> {
        Some(&self.type_)
    }

    fn property(&self, name: &str) -> Option<StixValue> {
        match name {
            "type" => Some(StixValue::String(self.type_.clone())),
            "id" => Some(StixValue::String(self.id.clone())),
            "first_observed" => Some(StixValue::String(self.first_observed.clone())),
            "last_observed" => Some(StixValue::String(self.last_observed.clone())),
            "number_observed" => Some(StixValue::Integer(self.number_observed as i64)),
            "object_refs" => Some(StixValue::List(
                self.object_refs
                    .iter()
                    .map(|s| StixValue::String(s.clone()))
                    .collect(),
            )),
            "objects" => self.objects.clone().map(StixValue::Object),
            other => self.additional.get(other).cloned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::StixValue;
    use crate::view::ObjectView;

    fn sample_json() -> serde_json::Value {
        serde_json::json!({
            "type": "observed-data",
            "id": "observed-data--1",
            "first_observed": "2020-01-01T00:00:00Z",
            "last_observed": "2020-01-01T00:05:00Z",
            "number_observed": 3,
            "object_refs": ["ipv4-addr--1", "domain-name--1"],
            "x_custom": "keep-me"
        })
    }

    #[test]
    fn deserializes_typed_fields() {
        let od: ObservedData = serde_json::from_value(sample_json()).unwrap();
        assert_eq!(od.id, "observed-data--1");
        assert_eq!(od.number_observed, 3);
        assert_eq!(od.first_observed, "2020-01-01T00:00:00Z");
        assert_eq!(od.object_refs, vec!["ipv4-addr--1", "domain-name--1"]);
    }

    #[test]
    fn object_view_exposes_typed_and_custom_props() {
        let od: ObservedData = serde_json::from_value(sample_json()).unwrap();
        assert_eq!(od.type_(), Some("observed-data"));
        assert_eq!(od.id(), Some("observed-data--1"));
        assert_eq!(od.property("number_observed"), Some(StixValue::Integer(3)));
        assert_eq!(
            od.property("first_observed"),
            Some(StixValue::String("2020-01-01T00:00:00Z".into()))
        );
        // custom property preserved via `additional`
        assert_eq!(
            od.property("x_custom"),
            Some(StixValue::String("keep-me".into()))
        );
        assert_eq!(od.property("nope"), None);
    }

    #[test]
    fn sco_ids_prefers_object_refs() {
        let od: ObservedData = serde_json::from_value(sample_json()).unwrap();
        assert_eq!(od.sco_ids(), vec!["ipv4-addr--1", "domain-name--1"]);
    }
}
