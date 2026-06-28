//! `StixObject`: the hybrid typed-or-generic object, with deserialization dispatch.

use serde::de::{Deserialize, Deserializer, Error as DeError};
use serde::Serialize;

use crate::error::{ModelError, Result};
use crate::sdo::ObservedData;
use crate::value::StixValue;
use crate::view::{GenericObject, ObjectView};

/// A STIX object: either a recognized typed object or a generic value bag.
#[derive(Debug, Clone, PartialEq)]
pub enum StixObject {
    Typed(TypedObject),
    Generic(GenericObject),
}

/// The set of types with dedicated typed structs. Additive: new variants slot in
/// here and in [`StixObject::from_json`]'s dispatch.
#[derive(Debug, Clone, PartialEq)]
pub enum TypedObject {
    ObservedData(ObservedData),
}

impl StixObject {
    /// Build a `StixObject` from a JSON value, dispatching on its `type` property.
    pub fn from_json(value: serde_json::Value) -> Result<Self> {
        let type_ = value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| ModelError::InvalidObject("missing 'type' property".to_string()))?
            .to_string();

        match type_.as_str() {
            "observed-data" => {
                let od: ObservedData = serde_json::from_value(value)?;
                Ok(StixObject::Typed(TypedObject::ObservedData(od)))
            }
            _ => Ok(StixObject::Generic(GenericObject::from_json(value)?)),
        }
    }
}

impl ObjectView for StixObject {
    fn id(&self) -> Option<&str> {
        match self {
            StixObject::Typed(t) => t.id(),
            StixObject::Generic(g) => g.id(),
        }
    }

    fn type_(&self) -> Option<&str> {
        match self {
            StixObject::Typed(t) => t.type_(),
            StixObject::Generic(g) => g.type_(),
        }
    }

    fn property(&self, name: &str) -> Option<StixValue> {
        match self {
            StixObject::Typed(t) => t.property(name),
            StixObject::Generic(g) => g.property(name),
        }
    }
}

impl ObjectView for TypedObject {
    fn id(&self) -> Option<&str> {
        match self {
            TypedObject::ObservedData(od) => od.id(),
        }
    }

    fn type_(&self) -> Option<&str> {
        match self {
            TypedObject::ObservedData(od) => od.type_(),
        }
    }

    fn property(&self, name: &str) -> Option<StixValue> {
        match self {
            TypedObject::ObservedData(od) => od.property(name),
        }
    }
}

impl Serialize for StixObject {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            StixObject::Typed(TypedObject::ObservedData(od)) => od.serialize(serializer),
            StixObject::Generic(g) => g.properties().serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for StixObject {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        StixObject::from_json(value).map_err(DeError::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::StixValue;
    use crate::view::ObjectView;

    #[test]
    fn observed_data_becomes_typed() {
        let v = serde_json::json!({
            "type": "observed-data",
            "id": "observed-data--1",
            "first_observed": "2020-01-01T00:00:00Z",
            "last_observed": "2020-01-01T00:05:00Z",
            "number_observed": 1,
            "object_refs": ["file--1"]
        });
        let obj = StixObject::from_json(v).unwrap();
        assert!(matches!(
            obj,
            StixObject::Typed(TypedObject::ObservedData(_))
        ));
        assert_eq!(obj.type_(), Some("observed-data"));
    }

    #[test]
    fn unknown_type_becomes_generic() {
        let v = serde_json::json!({
            "type": "ipv4-addr",
            "id": "ipv4-addr--1",
            "value": "198.51.100.1"
        });
        let obj = StixObject::from_json(v).unwrap();
        assert!(matches!(obj, StixObject::Generic(_)));
        assert_eq!(obj.type_(), Some("ipv4-addr"));
        assert_eq!(
            obj.property("value"),
            Some(StixValue::String("198.51.100.1".into()))
        );
    }

    #[test]
    fn missing_type_is_an_error() {
        let v = serde_json::json!({"id": "x--1"});
        assert!(StixObject::from_json(v).is_err());
    }

    #[test]
    fn deserializes_via_serde() {
        // serde path (used by Bundle) routes through the same dispatch.
        let v = serde_json::json!({
            "type": "ipv4-addr",
            "id": "ipv4-addr--2",
            "value": "203.0.113.5"
        });
        let obj: StixObject = serde_json::from_value(v).unwrap();
        assert_eq!(obj.id(), Some("ipv4-addr--2"));
    }
}
