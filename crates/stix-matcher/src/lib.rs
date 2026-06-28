//! Match STIX 2.1 patterns against observed STIX objects.
//!
//! # Example
//!
//! ```
//! use stix_matcher::match_scos;
//! use stix_pattern::parse;
//! use stix_model::StixObject;
//!
//! let pattern = parse("[ipv4-addr:value = '198.51.100.1']").unwrap();
//! let sco = StixObject::from_json(serde_json::json!({
//!     "type": "ipv4-addr", "id": "ipv4-addr--1", "value": "198.51.100.1"
//! })).unwrap();
//!
//! let result = match_scos(&pattern, &[sco]).unwrap();
//! assert!(result.is_match());
//! ```

pub mod compare;
pub mod error;
pub mod eval;
pub mod observation;
pub mod pattern_ops;
pub mod resolve;
pub mod result;
pub mod subset;

pub use error::MatchError;
pub use observation::Observation;
pub use result::MatchResult;

use stix_model::{Bundle, ObjectStore, StixObject, TypedObject};
use stix_pattern::ast::Pattern;

/// Match a pattern against a list of pre-built observations.
pub fn match_observations(
    pattern: &Pattern,
    observations: &[Observation],
) -> Result<MatchResult, MatchError> {
    eval::eval_pattern(pattern, observations, None)
}

/// Match a pattern against `observed-data` SDOs, resolving their `object_refs`
/// through `store` (MITRE-compatible entry point).
pub fn match_observed_data(
    pattern: &Pattern,
    observed: &[stix_model::ObservedData],
    store: &ObjectStore,
) -> Result<MatchResult, MatchError> {
    let observations: Vec<Observation> = observed
        .iter()
        .map(|od| {
            let objects = od
                .sco_ids()
                .iter()
                .filter_map(|id| store.get(id).cloned())
                .collect();
            Observation {
                objects,
                first_observed: Some(od.first_observed.clone()),
                last_observed: Some(od.last_observed.clone()),
                number_observed: od.number_observed,
            }
        })
        .collect();
    eval::eval_pattern(pattern, &observations, Some(store))
}

/// Match a pattern against a whole bundle, deriving observations from its
/// `observed-data` SDOs and resolving references through the bundle's objects.
pub fn match_bundle(pattern: &Pattern, bundle: &Bundle) -> Result<MatchResult, MatchError> {
    let store = ObjectStore::from_bundle(bundle);
    let observed: Vec<stix_model::ObservedData> = bundle
        .objects
        .iter()
        .filter_map(|o| match o {
            StixObject::Typed(TypedObject::ObservedData(od)) => Some(od.clone()),
            _ => None,
        })
        .collect();
    match_observed_data(pattern, &observed, &store)
}

/// Match a pattern against a flat list of cyber-observable objects, treated as a
/// single observation.
pub fn match_scos(pattern: &Pattern, scos: &[StixObject]) -> Result<MatchResult, MatchError> {
    let store = ObjectStore::from_objects(scos);
    let observation = Observation::new(scos.to_vec());
    eval::eval_pattern(pattern, std::slice::from_ref(&observation), Some(&store))
}
