//! An observation: a set of cyber-observable objects plus temporal metadata.

use stix_model::StixObject;

/// A set of objects observed together. Each STIX `observed-data` SDO maps to one
/// `Observation`; `match_scos` treats a flat list as a single observation.
#[derive(Debug, Clone)]
pub struct Observation {
    pub objects: Vec<StixObject>,
    pub first_observed: Option<String>,
    pub last_observed: Option<String>,
    pub number_observed: u64,
}

impl Observation {
    /// A single observation of the given objects (`number_observed` = 1, no times).
    pub fn new(objects: Vec<StixObject>) -> Self {
        Observation {
            objects,
            first_observed: None,
            last_observed: None,
            number_observed: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stix_model::StixObject;

    fn sco(json: serde_json::Value) -> StixObject {
        StixObject::from_json(json).unwrap()
    }

    #[test]
    fn new_defaults_number_observed_to_one() {
        let o = Observation::new(vec![sco(serde_json::json!({
            "type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.2.3.4"
        }))]);
        assert_eq!(o.objects.len(), 1);
        assert_eq!(o.number_observed, 1);
        assert!(o.first_observed.is_none());
    }
}
