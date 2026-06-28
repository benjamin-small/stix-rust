//! The `ObjectView` trait and the generic value-backed object.

use std::collections::BTreeMap;

use crate::error::{ModelError, Result};
use crate::value::StixValue;

/// A read-only, type-agnostic view over a STIX object.
///
/// `property` returns an *owned* [`StixValue`] so typed objects can synthesize
/// values on demand without storing every field twice. The matcher consumes only
/// this trait, so it never needs to branch on typed vs. generic objects.
pub trait ObjectView {
    fn id(&self) -> Option<&str>;
    fn type_(&self) -> Option<&str>;
    fn property(&self, name: &str) -> Option<StixValue>;
}

/// A STIX object stored as a flat property map. Used for any object type without
/// a dedicated typed struct, and retains all properties (including custom ones).
#[derive(Debug, Clone, PartialEq)]
pub struct GenericObject {
    properties: BTreeMap<String, StixValue>,
}

impl GenericObject {
    /// Build a generic object from a JSON value. The value must be a JSON object.
    pub fn from_json(value: serde_json::Value) -> Result<Self> {
        match StixValue::from(value) {
            StixValue::Object(properties) => Ok(GenericObject { properties }),
            _ => Err(ModelError::InvalidObject(
                "expected a JSON object".to_string(),
            )),
        }
    }

    /// The full property map.
    pub fn properties(&self) -> &BTreeMap<String, StixValue> {
        &self.properties
    }
}

impl ObjectView for GenericObject {
    fn id(&self) -> Option<&str> {
        self.properties.get("id").and_then(StixValue::as_str)
    }

    fn type_(&self) -> Option<&str> {
        self.properties.get("type").and_then(StixValue::as_str)
    }

    fn property(&self, name: &str) -> Option<StixValue> {
        self.properties.get(name).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::StixValue;

    fn sample() -> GenericObject {
        let v = serde_json::json!({
            "type": "ipv4-addr",
            "id": "ipv4-addr--1",
            "value": "198.51.100.1"
        });
        GenericObject::from_json(v).unwrap()
    }

    #[test]
    fn exposes_id_and_type() {
        let o = sample();
        assert_eq!(o.type_(), Some("ipv4-addr"));
        assert_eq!(o.id(), Some("ipv4-addr--1"));
    }

    #[test]
    fn property_returns_owned_value() {
        let o = sample();
        assert_eq!(
            o.property("value"),
            Some(StixValue::String("198.51.100.1".into()))
        );
        assert_eq!(o.property("missing"), None);
    }

    #[test]
    fn rejects_non_object_json() {
        let err = GenericObject::from_json(serde_json::json!([1, 2, 3])).unwrap_err();
        assert!(matches!(err, crate::error::ModelError::InvalidObject(_)));
    }
}
