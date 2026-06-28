//! `ObjectStore`: an id-indexed collection for resolving references.

use std::collections::HashMap;

use crate::bundle::Bundle;
use crate::object::StixObject;
use crate::view::ObjectView;

/// An id → object index built from a bundle or a list of objects. Used by the
/// matcher to resolve `object_refs` and reference properties (e.g. `src_ref`).
#[derive(Debug, Clone, Default)]
pub struct ObjectStore {
    by_id: HashMap<String, StixObject>,
}

impl ObjectStore {
    /// Build a store from a slice of objects. Objects without an `id` are skipped.
    pub fn from_objects(objects: &[StixObject]) -> Self {
        let mut by_id = HashMap::new();
        for obj in objects {
            if let Some(id) = obj.id() {
                by_id.insert(id.to_string(), obj.clone());
            }
        }
        ObjectStore { by_id }
    }

    /// Build a store from a bundle's objects.
    pub fn from_bundle(bundle: &Bundle) -> Self {
        ObjectStore::from_objects(&bundle.objects)
    }

    /// Resolve an object by id.
    pub fn get(&self, id: &str) -> Option<&StixObject> {
        self.by_id.get(id)
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::Bundle;
    use crate::view::ObjectView;

    fn store() -> ObjectStore {
        let b = Bundle::from_json_str(
            r#"{
                "type": "bundle",
                "id": "bundle--1",
                "objects": [
                    {"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.2.3.4"},
                    {"type": "domain-name", "id": "domain-name--1", "value": "evil.example"}
                ]
            }"#,
        )
        .unwrap();
        ObjectStore::from_bundle(&b)
    }

    #[test]
    fn resolves_by_id() {
        let s = store();
        let o = s.get("ipv4-addr--1").expect("should be present");
        assert_eq!(o.property("value").unwrap().as_str(), Some("1.2.3.4"));
    }

    #[test]
    fn missing_id_returns_none() {
        let s = store();
        assert!(s.get("nope--1").is_none());
    }

    #[test]
    fn len_counts_objects() {
        let s = store();
        assert_eq!(s.len(), 2);
        assert!(!s.is_empty());
    }
}
