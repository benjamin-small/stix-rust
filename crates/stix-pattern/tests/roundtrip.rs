use stix_pattern::{parse, Pattern};

#[test]
fn parse_then_serde_round_trip() {
    let src = "[ipv4-addr:value = '1.2.3.4'] FOLLOWEDBY [domain-name:value = 'evil.example'] WITHIN 300 SECONDS";
    let pattern = parse(src).expect("should parse");
    let json = serde_json::to_string(&pattern).expect("serialize");
    let back: Pattern = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(pattern, back);
}

#[test]
fn doc_example_in_lib_is_valid() {
    // Mirrors the crate-level doc example; guards against doc rot.
    let p = parse("[file:hashes.'SHA-256' = 'abc']").unwrap();
    let _ = serde_json::to_string(&p).unwrap();
}
