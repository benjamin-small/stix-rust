//! The `ObjectView` trait and the generic value-backed object.

use std::any::Any;
use std::collections::BTreeMap;

use serde::Serialize;

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

/// A consumer-supplied object type that the matcher can view uniformly.
///
/// Blanket-implemented for any [`ObjectView`] that is also `Serialize`, so a
/// consumer only writes an `ObjectView` impl. `as_json` backs serialization and
/// equality; `as_any` enables downcasting back to the concrete type.
pub trait CustomObject: ObjectView + std::fmt::Debug + Send + Sync {
    fn as_json(&self) -> serde_json::Value;
    fn as_any(&self) -> &dyn Any;
}

impl<T> CustomObject for T
where
    T: ObjectView + Serialize + std::fmt::Debug + Send + Sync + 'static,
{
    fn as_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
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

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    struct TestWidget {
        #[serde(rename = "type")]
        type_: String,
        id: String,
        risk: i64,
    }

    impl ObjectView for TestWidget {
        fn id(&self) -> Option<&str> {
            Some(&self.id)
        }
        fn type_(&self) -> Option<&str> {
            Some(&self.type_)
        }
        fn property(&self, name: &str) -> Option<StixValue> {
            match name {
                "risk" => Some(StixValue::Integer(self.risk)),
                _ => None,
            }
        }
    }

    #[test]
    fn custom_object_blanket_impl_provides_json_and_any() {
        let w = TestWidget {
            type_: "x-widget".into(),
            id: "x-widget--1".into(),
            risk: 90,
        };
        // Blanket impl gives `as_json` and `as_any` for free.
        let json = CustomObject::as_json(&w);
        assert_eq!(json["risk"], serde_json::json!(90));
        assert!(w.as_any().downcast_ref::<TestWidget>().is_some());
    }
}
