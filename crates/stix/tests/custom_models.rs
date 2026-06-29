use serde::{Deserialize, Serialize};
use stix::matcher::match_bundle;
use stix::model::{ModelRegistry, ObjectView, StixValue};
use stix::parse;

#[derive(Debug, Serialize, Deserialize)]
struct AcmeWidget {
    #[serde(rename = "type")]
    type_: String,
    id: String,
    risk_score: i64,
}

impl ObjectView for AcmeWidget {
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
            "risk_score" => Some(StixValue::Integer(self.risk_score)),
            // A computed property, synthesized on demand.
            "risk_band" => Some(StixValue::String(
                if self.risk_score > 80 { "high" } else { "low" }.to_string(),
            )),
            _ => None,
        }
    }
}

fn bundle_json() -> &'static str {
    r#"{
      "type": "bundle",
      "id": "bundle--1",
      "objects": [
        {"type": "x-acme-widget", "id": "x-acme-widget--1", "risk_score": 90},
        {"type": "observed-data", "id": "observed-data--1",
         "first_observed": "2020-01-01T00:00:00Z", "last_observed": "2020-01-01T00:00:00Z",
         "number_observed": 1, "object_refs": ["x-acme-widget--1"]}
      ]
    }"#
}

#[test]
fn matches_custom_typed_property() {
    let mut registry = ModelRegistry::new();
    registry.register::<AcmeWidget>("x-acme-widget");
    let bundle = registry.parse_bundle(bundle_json()).unwrap();

    let pattern = parse("[x-acme-widget:risk_score = 90]").unwrap();
    assert!(match_bundle(&pattern, &bundle).unwrap().is_match());
}

#[test]
fn matches_custom_computed_property() {
    let mut registry = ModelRegistry::new();
    registry.register::<AcmeWidget>("x-acme-widget");
    let bundle = registry.parse_bundle(bundle_json()).unwrap();

    // `risk_band` is computed by the ObjectView impl, not present in the JSON.
    let pattern = parse("[x-acme-widget:risk_band = 'high']").unwrap();
    assert!(match_bundle(&pattern, &bundle).unwrap().is_match());
}

#[test]
fn typed_downcast_after_parse() {
    let mut registry = ModelRegistry::new();
    registry.register::<AcmeWidget>("x-acme-widget");
    let bundle = registry.parse_bundle(bundle_json()).unwrap();

    let widget = bundle
        .objects
        .iter()
        .find_map(|o| o.downcast_ref::<AcmeWidget>())
        .expect("widget present");
    assert_eq!(widget.risk_score, 90);
}
