//! The `Engine` handle: owns a registry, parses patterns/bundles, runs matches.

use stix::model::ModelRegistry;

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

    /// Parse a STIX pattern string into a [`Pattern`] handle.
    pub fn parse_pattern(&self, src: &str) -> Result<Pattern, FfiError> {
        let inner = stix::parse(src)?;
        Ok(Pattern::new(inner))
    }

    /// Parse a STIX bundle (consulting registered custom types) into a [`Bundle`].
    pub fn parse_bundle(&self, json: &str) -> Result<Bundle, FfiError> {
        let inner = self.registry.parse_bundle(json)?;
        Ok(Bundle::new(inner))
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
