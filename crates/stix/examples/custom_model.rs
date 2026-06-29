//! Run with: `cargo run -p stix --example custom_model`
//!
//! Demonstrates registering a consumer-defined custom STIX object type, getting
//! typed access to it after parsing, and matching a pattern against a computed
//! property.

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
            // Computed property — synthesized, not stored in the JSON.
            "risk_band" => Some(StixValue::String(
                if self.risk_score > 80 { "high" } else { "low" }.to_string(),
            )),
            _ => None,
        }
    }
}

fn main() {
    let mut registry = ModelRegistry::new();
    registry.register::<AcmeWidget>("x-acme-widget");

    let json = r#"{
      "type": "bundle",
      "id": "bundle--1",
      "objects": [
        {"type": "x-acme-widget", "id": "x-acme-widget--1", "risk_score": 90},
        {"type": "observed-data", "id": "observed-data--1",
         "first_observed": "2020-01-01T00:00:00Z", "last_observed": "2020-01-01T00:00:00Z",
         "number_observed": 1, "object_refs": ["x-acme-widget--1"]}
      ]
    }"#;

    let bundle = registry.parse_bundle(json).expect("parse bundle");

    // Typed access via downcast.
    for obj in &bundle.objects {
        if let Some(w) = obj.downcast_ref::<AcmeWidget>() {
            println!(
                "typed access -> widget {} (risk_score={})",
                w.id, w.risk_score
            );
        }
    }

    // Match against a computed property the matcher resolves through ObjectView.
    let pattern = parse("[x-acme-widget:risk_band = 'high']").expect("parse pattern");
    let result = match_bundle(&pattern, &bundle).expect("match");
    println!(
        "pattern [x-acme-widget:risk_band = 'high'] matched: {}",
        result.is_match()
    );
    assert!(result.is_match());
}
