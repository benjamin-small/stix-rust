use stix_matcher::match_bundle;
use stix_model::Bundle;
use stix_pattern::parse;

fn bundle() -> Bundle {
    Bundle::from_json_str(include_str!("fixtures/bundle.json")).unwrap()
}

#[test]
fn matches_simple_value() {
    let b = bundle();
    let p = parse("[ipv4-addr:value = '198.51.100.5']").unwrap();
    assert!(match_bundle(&p, &b).unwrap().is_match());
}

#[test]
fn non_match_returns_false() {
    let b = bundle();
    let p = parse("[ipv4-addr:value = '203.0.113.9']").unwrap();
    assert!(!match_bundle(&p, &b).unwrap().is_match());
}

#[test]
fn matches_across_object_types_in_one_observation() {
    let b = bundle();
    let p =
        parse("[ipv4-addr:value = '198.51.100.5' AND domain-name:value = 'evil.example']").unwrap();
    assert!(match_bundle(&p, &b).unwrap().is_match());
}

#[test]
fn matches_through_reference_deref() {
    let b = bundle();
    // network-traffic:src_ref -> ipv4-addr--m1, whose value is 198.51.100.5
    let p = parse("[network-traffic:src_ref.value = '198.51.100.5']").unwrap();
    assert!(match_bundle(&p, &b).unwrap().is_match());
}

#[test]
fn matches_issubset_cidr() {
    let b = bundle();
    let p = parse("[ipv4-addr:value ISSUBSET '198.51.100.0/24']").unwrap();
    assert!(match_bundle(&p, &b).unwrap().is_match());
}

#[test]
fn matches_like_wildcard() {
    let b = bundle();
    let p = parse("[domain-name:value LIKE '%.example']").unwrap();
    assert!(match_bundle(&p, &b).unwrap().is_match());
}

#[test]
fn followedby_is_unsupported_end_to_end() {
    let b = bundle();
    let p =
        parse("[ipv4-addr:value = '198.51.100.5'] FOLLOWEDBY [domain-name:value = 'evil.example']")
            .unwrap();
    assert!(match_bundle(&p, &b).is_err());
}
