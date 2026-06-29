//! `StixObject`: the hybrid typed-or-generic object, with deserialization dispatch.

use serde::de::{Deserialize, Deserializer, Error as DeError};
use serde::Serialize;

use std::sync::Arc;

use crate::error::{ModelError, Result};
use crate::sdo::ObservedData;
use crate::value::StixValue;
use crate::view::{CustomObject, GenericObject, ObjectView};

/// A STIX object: a recognized typed object, a generic value bag, or a
/// consumer-registered custom object.
#[derive(Debug, Clone)]
pub enum StixObject {
    Typed(TypedObject),
    Generic(GenericObject),
    Custom(Arc<dyn CustomObject>),
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

    /// If this is a registered custom object of concrete type `T`, borrow it.
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        match self {
            StixObject::Custom(c) => c.as_any().downcast_ref::<T>(),
            _ => None,
        }
    }
}

impl PartialEq for StixObject {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (StixObject::Typed(a), StixObject::Typed(b)) => a == b,
            (StixObject::Generic(a), StixObject::Generic(b)) => a == b,
            (StixObject::Custom(a), StixObject::Custom(b)) => a.as_json() == b.as_json(),
            _ => false,
        }
    }
}

impl ObjectView for StixObject {
    fn id(&self) -> Option<&str> {
        match self {
            StixObject::Typed(t) => t.id(),
            StixObject::Generic(g) => g.id(),
            StixObject::Custom(c) => c.id(),
        }
    }

    fn type_(&self) -> Option<&str> {
        match self {
            StixObject::Typed(t) => t.type_(),
            StixObject::Generic(g) => g.type_(),
            StixObject::Custom(c) => c.type_(),
        }
    }

    fn property(&self, name: &str) -> Option<StixValue> {
        match self {
            StixObject::Typed(t) => t.property(name),
            StixObject::Generic(g) => g.property(name),
            StixObject::Custom(c) => c.property(name),
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
            StixObject::Custom(c) => c.as_json().serialize(serializer),
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

    use std::sync::Arc;

    #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Widget {
        #[serde(rename = "type")]
        type_: String,
        id: String,
        risk: i64,
    }

    impl ObjectView for Widget {
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
                "risk" => Some(StixValue::Integer(self.risk)),
                _ => None,
            }
        }
    }

    fn widget(risk: i64) -> StixObject {
        StixObject::Custom(Arc::new(Widget {
            type_: "x-widget".into(),
            id: "x-widget--1".into(),
            risk,
        }))
    }

    #[test]
    fn custom_exposes_object_view() {
        let o = widget(90);
        assert_eq!(o.type_(), Some("x-widget"));
        assert_eq!(o.id(), Some("x-widget--1"));
        assert_eq!(o.property("risk"), Some(StixValue::Integer(90)));
        assert_eq!(o.property("missing"), None);
    }

    #[test]
    fn custom_downcasts_to_concrete_type() {
        let o = widget(90);
        let w = o.downcast_ref::<Widget>().expect("downcast");
        assert_eq!(w.risk, 90);
        // Wrong target type yields None.
        assert!(o.downcast_ref::<String>().is_none());
        // Non-custom objects yield None.
        let generic = StixObject::from_json(serde_json::json!({"type":"x","id":"x--1"})).unwrap();
        assert!(generic.downcast_ref::<Widget>().is_none());
    }

    #[test]
    fn custom_clone_and_eq_and_serialize() {
        assert_eq!(widget(90), widget(90));
        assert_ne!(widget(90), widget(10));
        let json = serde_json::to_value(widget(90)).unwrap();
        assert_eq!(json["risk"], serde_json::json!(90));
    }
}
