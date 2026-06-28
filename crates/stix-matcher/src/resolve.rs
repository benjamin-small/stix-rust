//! Resolve a pattern `ObjectPath` to the set of values it selects on an object.

use stix_model::{ObjectStore, ObjectView, StixValue};
use stix_pattern::ast::{ObjectPath, PathStep};

/// Resolve `path` against `obj`, returning every value the path selects.
///
/// Returns an empty vec if the object's type does not match the path root, or the
/// path leads nowhere. A `[*]` step or a `_refs` list can produce several values.
/// When a step descends *into* a string value, that string is treated as a STIX id
/// and dereferenced through `store` (if provided) — this implements `_ref`/`_refs`
/// traversal such as `network-traffic:src_ref.value`.
pub fn resolve_path(
    obj: &dyn ObjectView,
    path: &ObjectPath,
    store: Option<&ObjectStore>,
) -> Vec<StixValue> {
    if obj.type_() != Some(path.object_type.as_str()) {
        return Vec::new();
    }
    let mut steps = path.steps.iter();

    // The first step is always a key looked up on the object itself.
    let first = match steps.next() {
        Some(PathStep::Key(k)) => k,
        // A path with no steps, or a leading index, selects nothing meaningful.
        _ => return Vec::new(),
    };
    let mut current: Vec<StixValue> = match obj.property(first) {
        Some(v) => vec![v],
        None => Vec::new(),
    };

    for step in steps {
        let mut next = Vec::new();
        for value in current.drain(..) {
            apply_step(value, step, store, &mut next);
        }
        current = next;
    }
    current
}

/// Apply one path step to one value, pushing any resulting values into `out`.
fn apply_step(
    value: StixValue,
    step: &PathStep,
    store: Option<&ObjectStore>,
    out: &mut Vec<StixValue>,
) {
    match step {
        PathStep::Key(key) => match value {
            // Descend into a nested object.
            StixValue::Object(map) => {
                if let Some(v) = map.get(key) {
                    out.push(v.clone());
                }
            }
            // Descend into a referenced object: treat the string as an id.
            StixValue::String(id) => {
                if let Some(store) = store {
                    if let Some(referenced) = store.get(&id) {
                        if let Some(v) = referenced.property(key) {
                            out.push(v);
                        }
                    }
                }
            }
            _ => {}
        },
        PathStep::Index(i) => {
            if let StixValue::List(items) = value {
                if let Some(v) = items.into_iter().nth(*i as usize) {
                    out.push(v);
                }
            }
        }
        PathStep::AnyIndex => {
            if let StixValue::List(items) = value {
                out.extend(items);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stix_model::{Bundle, ObjectStore, StixObject, StixValue};
    use stix_pattern::ast::{ObjectPath, PathStep};

    fn obj(json: serde_json::Value) -> StixObject {
        StixObject::from_json(json).unwrap()
    }

    fn path(object_type: &str, steps: Vec<PathStep>) -> ObjectPath {
        ObjectPath {
            object_type: object_type.to_string(),
            steps,
        }
    }

    #[test]
    fn resolves_top_level_property() {
        let o = obj(serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.2.3.4"}));
        let p = path("ipv4-addr", vec![PathStep::Key("value".into())]);
        assert_eq!(
            resolve_path(&o, &p, None),
            vec![StixValue::String("1.2.3.4".into())]
        );
    }

    #[test]
    fn type_mismatch_yields_nothing() {
        let o = obj(serde_json::json!({"type": "domain-name", "id": "domain-name--1", "value": "x"}));
        let p = path("ipv4-addr", vec![PathStep::Key("value".into())]);
        assert!(resolve_path(&o, &p, None).is_empty());
    }

    #[test]
    fn resolves_nested_key() {
        let o = obj(serde_json::json!({
            "type": "file", "id": "file--1",
            "hashes": {"SHA-256": "abc"}
        }));
        let p = path(
            "file",
            vec![
                PathStep::Key("hashes".into()),
                PathStep::Key("SHA-256".into()),
            ],
        );
        assert_eq!(resolve_path(&o, &p, None), vec![StixValue::String("abc".into())]);
    }

    #[test]
    fn resolves_index_and_any_index() {
        let o = obj(serde_json::json!({
            "type": "network-traffic", "id": "network-traffic--1",
            "protocols": ["ipv4", "tcp"]
        }));
        let idx = path(
            "network-traffic",
            vec![PathStep::Key("protocols".into()), PathStep::Index(1)],
        );
        assert_eq!(resolve_path(&o, &idx, None), vec![StixValue::String("tcp".into())]);

        let any = path(
            "network-traffic",
            vec![PathStep::Key("protocols".into()), PathStep::AnyIndex],
        );
        assert_eq!(
            resolve_path(&o, &any, None),
            vec![
                StixValue::String("ipv4".into()),
                StixValue::String("tcp".into())
            ]
        );
    }

    #[test]
    fn dereferences_ref_through_store() {
        let bundle = Bundle::from_json_str(
            r#"{"type":"bundle","id":"bundle--1","objects":[
                {"type":"ipv4-addr","id":"ipv4-addr--1","value":"1.2.3.4"},
                {"type":"network-traffic","id":"network-traffic--1","src_ref":"ipv4-addr--1"}
            ]}"#,
        )
        .unwrap();
        let store = ObjectStore::from_bundle(&bundle);
        let nt = obj(serde_json::json!({
            "type": "network-traffic", "id": "network-traffic--1", "src_ref": "ipv4-addr--1"
        }));
        let p = path(
            "network-traffic",
            vec![
                PathStep::Key("src_ref".into()),
                PathStep::Key("value".into()),
            ],
        );
        assert_eq!(
            resolve_path(&nt, &p, Some(&store)),
            vec![StixValue::String("1.2.3.4".into())]
        );
    }

    #[test]
    fn missing_property_yields_nothing() {
        let o = obj(serde_json::json!({"type": "file", "id": "file--1", "name": "x"}));
        let p = path("file", vec![PathStep::Key("size".into())]);
        assert!(resolve_path(&o, &p, None).is_empty());
    }
}
