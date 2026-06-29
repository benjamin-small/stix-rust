//! The `Engine` handle: owns a registry, parses patterns/bundles, runs matches.

use stix::model::ModelRegistry;

use crate::error::ErrorCode;
use crate::error::FfiError;
use crate::handles::{Bundle, MatchOutcome, Pattern};

/// The stateful facade handle. Holds the custom-model registry used by
/// `parse_bundle`.
#[derive(Default)]
pub struct Engine {
    registry: ModelRegistry,
}

impl Engine {
    /// A new engine with an empty registry.
    pub fn new() -> Self {
        Engine::default()
    }

    /// Register a custom object type. `hook` validates and/or normalizes a raw JSON
    /// object of `type_name`; returning `Err(message)` rejects it (surfaced as a
    /// `Validation` error from `parse_bundle`). Returning an enriched object adds
    /// computed properties, stored as data. The hook runs only at parse time.
    pub fn register_type(
        &mut self,
        type_name: &str,
        hook: Box<dyn Fn(serde_json::Value) -> Result<serde_json::Value, String> + Send + Sync>,
    ) {
        self.registry.register_handler(type_name, move |value| {
            hook(value).map_err(|message| {
                stix::model::ModelError::InvalidObject(format!("[Validation] {message}"))
            })
        });
    }

    /// Parse a STIX pattern string into a [`Pattern`] handle.
    pub fn parse_pattern(&self, src: &str) -> Result<Pattern, FfiError> {
        let inner = stix::parse(src)?;
        Ok(Pattern::new(inner))
    }

    /// Parse a STIX bundle (consulting registered custom types) into a [`Bundle`].
    pub fn parse_bundle(&self, json: &str) -> Result<Bundle, FfiError> {
        match self.registry.parse_bundle(json) {
            Ok(inner) => Ok(Bundle::new(inner)),
            Err(stix::model::ModelError::InvalidObject(m)) if m.starts_with("[Validation] ") => {
                Err(FfiError::new(
                    ErrorCode::Validation,
                    m.trim_start_matches("[Validation] ").to_string(),
                ))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Match a pattern against a bundle.
    pub fn match_bundle(
        &self,
        pattern: &Pattern,
        bundle: &Bundle,
    ) -> Result<MatchOutcome, FfiError> {
        let result = stix::matcher::match_bundle(pattern.inner(), bundle.inner())?;
        Ok(MatchOutcome {
            matched: result.is_match(),
            observations: result.observations().iter().map(|&i| i as u64).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use crate::error::ErrorCode as Code;

    fn custom_bundle_json() -> &'static str {
        r#"{"type":"bundle","id":"bundle--1","objects":[
            {"type":"x-acme-widget","id":"x-acme-widget--1","risk_score":90},
            {"type":"observed-data","id":"observed-data--1",
             "first_observed":"2020-01-01T00:00:00Z","last_observed":"2020-01-01T00:00:00Z",
             "number_observed":1,"object_refs":["x-acme-widget--1"]}
        ]}"#
    }

    #[test]
    fn register_type_adds_computed_property_and_matches() {
        let mut engine = Engine::new();
        engine.register_type(
            "x-acme-widget",
            Box::new(|mut obj| {
                let score = obj.get("risk_score").and_then(|v| v.as_i64()).unwrap_or(0);
                obj["risk_band"] = serde_json::json!(if score > 80 { "high" } else { "low" });
                Ok(obj)
            }),
        );
        let bundle = engine.parse_bundle(custom_bundle_json()).unwrap();
        let pattern = engine.parse_pattern("[x-acme-widget:risk_band = 'high']").unwrap();
        assert!(engine.match_bundle(&pattern, &bundle).unwrap().matched);
    }

    #[test]
    fn register_type_rejection_is_validation_error() {
        let mut engine = Engine::new();
        engine.register_type(
            "x-acme-widget",
            Box::new(|obj| {
                if obj.get("risk_score").is_none() {
                    return Err("missing risk_score".to_string());
                }
                Ok(obj)
            }),
        );
        let err = engine
            .parse_bundle(
                r#"{"type":"bundle","objects":[{"type":"x-acme-widget","id":"x--1"}]}"#,
            )
            .unwrap_err();
        assert_eq!(err.code, Code::Validation);
        assert!(err.message.contains("missing risk_score"));
    }

    fn bundle_json() -> &'static str {
        r#"{"type":"bundle","id":"bundle--1","objects":[
            {"type":"ipv4-addr","id":"ipv4-addr--1","value":"198.51.100.5"},
            {"type":"observed-data","id":"observed-data--1",
             "first_observed":"2020-01-01T00:00:00Z","last_observed":"2020-01-01T00:00:00Z",
             "number_observed":1,"object_refs":["ipv4-addr--1"]}
        ]}"#
    }

    #[test]
    fn parse_pattern_ok_and_err() {
        let engine = Engine::new();
        assert!(engine.parse_pattern("[ipv4-addr:value = '1.2.3.4']").is_ok());
        let err = engine.parse_pattern("[bad").unwrap_err();
        assert_eq!(err.code, ErrorCode::Parse);
    }

    #[test]
    fn parse_bundle_ok_and_non_bundle_err() {
        let engine = Engine::new();
        assert!(engine.parse_bundle(bundle_json()).is_ok());
        let err = engine
            .parse_bundle(r#"{"type":"ipv4-addr","id":"x--1"}"#)
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::Model);
    }

    #[test]
    fn match_bundle_match_and_non_match() {
        let engine = Engine::new();
        let bundle = engine.parse_bundle(bundle_json()).unwrap();

        let hit = engine.parse_pattern("[ipv4-addr:value = '198.51.100.5']").unwrap();
        let outcome = engine.match_bundle(&hit, &bundle).unwrap();
        assert!(outcome.matched);

        let miss = engine.parse_pattern("[ipv4-addr:value = '203.0.113.9']").unwrap();
        assert!(!engine.match_bundle(&miss, &bundle).unwrap().matched);
    }
}
