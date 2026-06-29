//! `ModelRegistry`: register consumer-supplied handling for STIX object types.

use std::collections::HashMap;

use serde_json::Value;

use crate::bundle::Bundle;
use crate::error::{ModelError, Result};
use crate::object::StixObject;
use crate::view::GenericObject;

/// A handler that turns a raw JSON object of a registered type into a `StixObject`.
type TypeHandler = Box<dyn Fn(Value) -> Result<StixObject> + Send + Sync>;

/// Maps a STIX `type` to a consumer-supplied handler. Registered handlers take
/// precedence over the built-in dispatch in [`StixObject::from_json`].
///
/// Two registration forms:
/// - [`register_handler`](ModelRegistry::register_handler): a data-level
///   `Value -> Result<Value>` validate/normalize hook (the form bindings bridge a
///   host callable onto). The result is stored as a generic object.
/// - [`register`](ModelRegistry::register): a typed Rust convenience that stores a
///   `StixObject::Custom`.
#[derive(Default)]
pub struct ModelRegistry {
    handlers: HashMap<String, TypeHandler>,
}

impl ModelRegistry {
    /// An empty registry (parsing behaves like the built-in dispatch).
    pub fn new() -> Self {
        ModelRegistry::default()
    }

    /// Register a data-level validate/normalize hook for `type_name`. The hook may
    /// reject the object (return `Err`) or return an enriched object (e.g. with a
    /// computed property). The result is stored as a [`StixObject::Generic`].
    pub fn register_handler<F>(&mut self, type_name: impl Into<String>, hook: F)
    where
        F: Fn(Value) -> Result<Value> + Send + Sync + 'static,
    {
        self.handlers.insert(
            type_name.into(),
            Box::new(move |value| {
                let normalized = hook(value)?;
                Ok(StixObject::Generic(GenericObject::from_json(normalized)?))
            }),
        );
    }

    /// Parse a single JSON object, dispatching on its `type`: a registered handler
    /// wins; otherwise the built-in dispatch applies.
    pub fn parse_object(&self, value: Value) -> Result<StixObject> {
        let type_ = value
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| ModelError::InvalidObject("missing 'type' property".to_string()))?
            .to_string();

        match self.handlers.get(&type_) {
            Some(handler) => handler(value),
            None => StixObject::from_json(value),
        }
    }

    /// Parse a bundle, routing every object through [`parse_object`](Self::parse_object).
    pub fn parse_bundle(&self, json: &str) -> Result<Bundle> {
        let value: Value = serde_json::from_str(json)?;
        let type_ = value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if type_ != "bundle" {
            return Err(ModelError::NotABundle(format!("type was '{type_}'")));
        }
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .map(|s| s.to_string());
        let objects = match value.get("objects") {
            Some(Value::Array(arr)) => arr
                .iter()
                .map(|o| self.parse_object(o.clone()))
                .collect::<Result<Vec<_>>>()?,
            _ => Vec::new(),
        };
        Ok(Bundle {
            type_,
            id,
            objects,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ModelError;
    use crate::object::{StixObject, TypedObject};
    use crate::view::ObjectView;

    #[test]
    fn handler_validates_and_rejects() {
        let mut reg = ModelRegistry::new();
        reg.register_handler("x-thing", |obj| {
            if obj.get("risk_score").is_none() {
                return Err(ModelError::InvalidObject("missing risk_score".into()));
            }
            Ok(obj)
        });
        let ok = reg.parse_object(serde_json::json!({"type":"x-thing","id":"x--1","risk_score":1}));
        assert!(ok.is_ok());
        let bad = reg.parse_object(serde_json::json!({"type":"x-thing","id":"x--1"}));
        assert!(matches!(bad, Err(ModelError::InvalidObject(_))));
    }

    #[test]
    fn handler_adds_computed_property() {
        let mut reg = ModelRegistry::new();
        reg.register_handler("x-thing", |mut obj| {
            let score = obj.get("risk_score").and_then(|v| v.as_i64()).unwrap_or(0);
            obj["risk_band"] = serde_json::json!(if score > 80 { "high" } else { "low" });
            Ok(obj)
        });
        let parsed = reg
            .parse_object(serde_json::json!({"type":"x-thing","id":"x--1","risk_score":90}))
            .unwrap();
        // The enriched object is stored as data (a Generic object).
        assert!(matches!(parsed, StixObject::Generic(_)));
        assert_eq!(
            parsed.property("risk_band"),
            Some(crate::value::StixValue::String("high".into()))
        );
    }

    #[test]
    fn unregistered_types_use_builtin_dispatch() {
        let reg = ModelRegistry::new();
        // Built-in observed-data dispatch still works.
        let od = reg
            .parse_object(serde_json::json!({
                "type":"observed-data","id":"observed-data--1",
                "first_observed":"2020-01-01T00:00:00Z","last_observed":"2020-01-01T00:00:00Z",
                "number_observed":1,"object_refs":[]
            }))
            .unwrap();
        assert!(matches!(od, StixObject::Typed(TypedObject::ObservedData(_))));
        // Unknown types fall back to Generic.
        let g = reg
            .parse_object(
                serde_json::json!({"type":"ipv4-addr","id":"ipv4-addr--1","value":"1.2.3.4"}),
            )
            .unwrap();
        assert!(matches!(g, StixObject::Generic(_)));
    }

    #[test]
    fn parse_bundle_routes_objects_through_handlers() {
        let mut reg = ModelRegistry::new();
        reg.register_handler("x-thing", |mut obj| {
            obj["seen"] = serde_json::json!(true);
            Ok(obj)
        });
        let bundle = reg
            .parse_bundle(
                r#"{"type":"bundle","id":"bundle--1","objects":[
                    {"type":"x-thing","id":"x--1"},
                    {"type":"ipv4-addr","id":"ipv4-addr--1","value":"1.2.3.4"}
                ]}"#,
            )
            .unwrap();
        assert_eq!(bundle.objects.len(), 2);
        assert_eq!(
            bundle.objects[0].property("seen"),
            Some(crate::value::StixValue::Bool(true))
        );
    }

    #[test]
    fn parse_bundle_rejects_non_bundle() {
        let reg = ModelRegistry::new();
        let err = reg
            .parse_bundle(r#"{"type":"ipv4-addr","id":"x--1"}"#)
            .unwrap_err();
        assert!(matches!(err, ModelError::NotABundle(_)));
    }
}
