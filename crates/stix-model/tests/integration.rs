use stix_model::{Bundle, ObjectStore, ObjectView, StixObject, StixValue, TypedObject};

#[test]
fn bundle_to_store_resolves_observation_refs() {
    let raw = include_str!("fixtures/bundle.json");
    let bundle = Bundle::from_json_str(raw).expect("parse bundle");
    let store = ObjectStore::from_bundle(&bundle);

    // Find the observed-data SDO and confirm it deserialized to the typed variant.
    let observed = bundle
        .objects
        .iter()
        .find_map(|o| match o {
            StixObject::Typed(TypedObject::ObservedData(od)) => Some(od),
            _ => None,
        })
        .expect("bundle has an observed-data object");

    assert_eq!(observed.number_observed, 5);
    assert_eq!(observed.sco_ids(), vec!["ipv4-addr--a1", "domain-name--a1"]);

    // Resolve each referenced SCO through the store and read a property.
    let ipv4 = store.get("ipv4-addr--a1").expect("ipv4 resolved");
    assert_eq!(ipv4.type_(), Some("ipv4-addr"));
    assert_eq!(
        ipv4.property("value"),
        Some(StixValue::String("198.51.100.5".into()))
    );

    let domain = store.get(observed.sco_ids()[1]).expect("domain resolved");
    assert_eq!(
        domain.property("value").unwrap().as_str(),
        Some("evil.example")
    );
}
