use stix_ffi::{Engine, ErrorCode};

fn bundle_json() -> &'static str {
    r#"{"type":"bundle","id":"bundle--1","objects":[
        {"type":"ipv4-addr","id":"ipv4-addr--1","value":"198.51.100.5"},
        {"type":"observed-data","id":"observed-data--1",
         "first_observed":"2020-01-01T00:00:00Z","last_observed":"2020-01-01T00:00:00Z",
         "number_observed":1,"object_refs":["ipv4-addr--1"]}
    ]}"#
}

#[test]
fn full_surface_round_trip() {
    let engine = Engine::new();

    // Pattern handle -> AST JSON.
    let pattern = engine.parse_pattern("[ipv4-addr:value = '198.51.100.5']").unwrap();
    let ast = pattern.to_json();
    assert!(ast.contains("ipv4-addr"));

    // Bundle handle -> object access.
    let bundle = engine.parse_bundle(bundle_json()).unwrap();
    assert_eq!(bundle.object_count(), 2);
    assert!(bundle.object_json(0).unwrap().contains("ipv4-addr--1"));

    // Match -> outcome.
    let outcome = engine.match_bundle(&pattern, &bundle).unwrap();
    assert!(outcome.matched);
    assert!(!outcome.observations.is_empty());
}

#[test]
fn error_codes_surface() {
    let engine = Engine::new();
    assert_eq!(engine.parse_pattern("[bad").unwrap_err().code, ErrorCode::Parse);
    assert_eq!(
        engine.parse_bundle("not json").unwrap_err().code,
        ErrorCode::Model
    );
}
