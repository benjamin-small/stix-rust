# stix-matcher + stix umbrella Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `stix-matcher` crate — the engine that evaluates a parsed pattern AST against observed STIX objects — and the `stix` umbrella crate that re-exports the whole toolkit with high-level entry points.

**Architecture:** `stix-matcher` depends on `stix-pattern` (AST) and `stix-model` (objects). It normalizes every input to a list of `Observation`s (a set of SCOs + temporal metadata), resolves pattern object-paths against `ObjectView`s (dereferencing `_ref`/`_refs` through an `ObjectStore`), evaluates comparison operators, and combines comparisons via a binding-enumeration model (each referenced object-type binds to one object per observation expression). Single-observation matching and observation-level `AND`/`OR` are implemented; `FOLLOWEDBY` and qualifiers parse but return an explicit `Unsupported` error rather than silently passing.

**Tech Stack:** Rust (edition 2021), `stix-pattern`, `stix-model`, `serde`/`serde_json`, `thiserror`, `regex` (for `LIKE`/`MATCHES`).

---

## Matching semantics (read before starting)

- **Observation:** one observation = a set of cyber-observable objects observed together, plus optional `first_observed`/`last_observed` and a `number_observed` count. Each `observed-data` SDO is one observation (MITRE-compatible).
- **Object-path resolution** returns the *set* of values a path selects (a path with `[*]` or through a `_refs` list yields several). A reference value (a string id) is dereferenced through the `ObjectStore` when the path continues past it.
- **Comparison (leaf):** matches an object if the object's type equals the path's root type and at least one resolved value satisfies the operator. `EXISTS` matches if the path resolves to any value. A leading `NOT` negates the leaf's result.
- **Comparison expression (inside `[ ]`):** evaluated against an observation by **binding enumeration** — for each distinct object-type referenced, choose one object of that type from the observation (or none); the expression matches if some binding makes the boolean tree true. This gives correct "same object" semantics for `AND` within an observation while keeping the search tiny (observations are small).
- **Observation expression:** an `[ ... ]` matches if any observation in the set satisfies it. `AND`/`OR` combine observation expressions (possibly satisfied by different observations). `FOLLOWEDBY` and any `Qualified { .. }` (WITHIN/REPEATS/START..STOP) return `MatchError::Unsupported` — parsed, honestly not-yet-matched.

### Recap of the types this crate consumes (already built)

- `stix_pattern`: `parse(&str) -> Result<Pattern, ParseError>`; `Pattern { expression: ObservationExpression }`;
  `ObservationExpression::{Observation(Box<ComparisonExpression>), And(Box<_>,Box<_>), Or(..), FollowedBy(..), Qualified{expression,qualifier}}`;
  `ComparisonExpression::{Test(Comparison), And(..), Or(..)}`;
  `Comparison { path: ObjectPath, operator: ComparisonOperator, negated: bool, value: ComparisonOperand }`;
  `ComparisonOperand::{Literal(Literal), Set(Vec<Literal>)}`;
  `ComparisonOperator::{Equal,NotEqual,GreaterThan,GreaterThanOrEqual,LessThan,LessThanOrEqual,In,Like,Matches,IsSubset,IsSuperset,Exists}`;
  `ObjectPath { object_type: String, steps: Vec<PathStep> }`; `PathStep::{Key(String),Index(u64),AnyIndex}`;
  `Literal::{String,Integer(i64),Float(f64),Boolean(bool),Timestamp(String),Binary(String),Hex(String)}`.
- `stix_model`: `StixValue::{Null,Bool,Integer(i64),Float(f64),String,List(Vec),Object(BTreeMap)}` with `as_str/as_i64/as_f64/as_bool/as_list/as_object/is_null`;
  `trait ObjectView { fn id(&self)->Option<&str>; fn type_(&self)->Option<&str>; fn property(&self,name:&str)->Option<StixValue>; }`;
  `StixObject` (impls `ObjectView`); `ObservedData { first_observed, last_observed, number_observed, object_refs, .. }` with `sco_ids()->Vec<&str>`;
  `Bundle { objects: Vec<StixObject>, .. }`; `ObjectStore::{from_bundle, from_objects, get}`.

---

## File Structure

- `Cargo.toml` (root) — add `regex` to `[workspace.dependencies]`; add both crates to `members`.
- `crates/stix-matcher/Cargo.toml` — manifest.
- `crates/stix-matcher/src/lib.rs` — module wiring + re-exports + entry points.
- `crates/stix-matcher/src/error.rs` — `MatchError`.
- `crates/stix-matcher/src/result.rs` — `MatchResult`.
- `crates/stix-matcher/src/observation.rs` — `Observation`.
- `crates/stix-matcher/src/resolve.rs` — object-path resolution (incl. `_ref` deref).
- `crates/stix-matcher/src/compare.rs` — scalar equality/ordering + `IN`.
- `crates/stix-matcher/src/pattern_ops.rs` — `LIKE` + `MATCHES`.
- `crates/stix-matcher/src/subset.rs` — `ISSUBSET`/`ISSUPERSET` (CIDR).
- `crates/stix-matcher/src/eval.rs` — leaf + comparison-expression + observation-expression evaluation; entry points.
- `crates/stix-matcher/tests/fixtures/bundle.json` — matcher integration fixture.
- `crates/stix-matcher/tests/integration.rs` — end-to-end matching tests.
- `crates/stix/Cargo.toml` + `crates/stix/src/lib.rs` — umbrella crate.

---

## Task 1: Scaffold stix-matcher crate

**Files:**
- Modify: `Cargo.toml` (root)
- Create: `crates/stix-matcher/Cargo.toml`
- Create: `crates/stix-matcher/src/lib.rs`

- [ ] **Step 1: Add `regex` to workspace deps and register the crate**

In the root `Cargo.toml`, update `members` and add `regex`:

```toml
members = ["crates/stix-pattern", "crates/stix-model", "crates/stix-matcher", "crates/stix"]
```

Add under `[workspace.dependencies]`:

```toml
regex = "1"
stix-pattern = { path = "crates/stix-pattern" }
stix-model = { path = "crates/stix-model" }
stix-matcher = { path = "crates/stix-matcher" }
```

> The `stix` umbrella crate (Task 12) is listed in `members` now; create a stub for it in Task 1 Step 4 so the workspace resolves.

- [ ] **Step 2: Create the matcher manifest**

Create `crates/stix-matcher/Cargo.toml`:

```toml
[package]
name = "stix-matcher"
version = "0.0.1"
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "Match STIX 2.1 patterns against observed STIX objects."

[dependencies]
stix-pattern = { workspace = true }
stix-model = { workspace = true }
regex = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
serde_json = { workspace = true }
```

- [ ] **Step 3: Create a placeholder matcher lib.rs**

Create `crates/stix-matcher/src/lib.rs`:

```rust
//! Match STIX 2.1 patterns against observed STIX objects.

#[cfg(test)]
mod smoke {
    #[test]
    fn crate_builds() {
        assert_eq!(2 + 2, 4);
    }
}
```

- [ ] **Step 4: Create a stub umbrella crate so the workspace resolves**

Create `crates/stix/Cargo.toml`:

```toml
[package]
name = "stix"
version = "0.0.1"
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "Umbrella crate for the stix-rust toolkit."

[dependencies]
stix-pattern = { workspace = true }
stix-model = { workspace = true }
stix-matcher = { workspace = true }
```

Create `crates/stix/src/lib.rs`:

```rust
//! Umbrella crate for the stix-rust toolkit. (Re-exports added in a later task.)
```

- [ ] **Step 5: Verify the workspace builds**

Run: `cargo test -p stix-matcher`
Expected: compiles; `smoke::crate_builds` passes.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/stix-matcher/Cargo.toml crates/stix-matcher/src/lib.rs crates/stix/Cargo.toml crates/stix/src/lib.rs
git commit -m "feat(matcher): scaffold stix-matcher and stix umbrella crates"
```

---

## Task 2: MatchError

**Files:**
- Create: `crates/stix-matcher/src/error.rs`
- Modify: `crates/stix-matcher/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/stix-matcher/src/error.rs`:

```rust
//! Error type for the matcher.

use thiserror::Error;

/// Errors produced while matching a pattern against observations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MatchError {
    /// A pattern feature is parsed but not yet supported by the matcher
    /// (e.g. `FOLLOWEDBY` sequencing or temporal qualifiers).
    #[error("unsupported pattern feature: {0}")]
    Unsupported(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_displays_feature() {
        let e = MatchError::Unsupported("FOLLOWEDBY".to_string());
        assert!(format!("{e}").contains("FOLLOWEDBY"));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-matcher`
Expected: FAIL — `error` module not declared.

- [ ] **Step 3: Wire the module**

Set `crates/stix-matcher/src/lib.rs` to:

```rust
//! Match STIX 2.1 patterns against observed STIX objects.

pub mod error;

pub use error::MatchError;
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p stix-matcher`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-matcher/src/error.rs crates/stix-matcher/src/lib.rs
git commit -m "feat(matcher): add MatchError"
```

---

## Task 3: MatchResult

**Files:**
- Create: `crates/stix-matcher/src/result.rs`
- Modify: `crates/stix-matcher/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/stix-matcher/src/result.rs`:

```rust
//! The outcome of a match.

/// The result of evaluating a pattern against a set of observations.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MatchResult {
    matched: bool,
    matched_observations: Vec<usize>,
}

impl MatchResult {
    /// A non-match (no observations).
    pub fn no_match() -> Self {
        MatchResult {
            matched: false,
            matched_observations: Vec::new(),
        }
    }

    /// A match, recording the indices of the observations that satisfied the pattern.
    pub fn matched(observations: Vec<usize>) -> Self {
        MatchResult {
            matched: true,
            matched_observations: observations,
        }
    }

    /// Whether the pattern matched.
    pub fn is_match(&self) -> bool {
        self.matched
    }

    /// Indices (into the input observation list) that participated in the match.
    pub fn observations(&self) -> &[usize] {
        &self.matched_observations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_match_is_false() {
        let r = MatchResult::no_match();
        assert!(!r.is_match());
        assert!(r.observations().is_empty());
    }

    #[test]
    fn matched_records_observations() {
        let r = MatchResult::matched(vec![0, 2]);
        assert!(r.is_match());
        assert_eq!(r.observations(), &[0, 2]);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-matcher`
Expected: FAIL — `result` module not declared.

- [ ] **Step 3: Wire the module**

Update `crates/stix-matcher/src/lib.rs`:

```rust
//! Match STIX 2.1 patterns against observed STIX objects.

pub mod error;
pub mod result;

pub use error::MatchError;
pub use result::MatchResult;
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p stix-matcher`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-matcher/src/result.rs crates/stix-matcher/src/lib.rs
git commit -m "feat(matcher): add MatchResult"
```

---

## Task 4: Observation

**Files:**
- Create: `crates/stix-matcher/src/observation.rs`
- Modify: `crates/stix-matcher/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/stix-matcher/src/observation.rs`:

```rust
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-matcher`
Expected: FAIL — `Observation` not found.

- [ ] **Step 3: Implement Observation**

At the top of `crates/stix-matcher/src/observation.rs` (above the test module):

```rust
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
```

In `crates/stix-matcher/src/lib.rs`, add the module and a `serde_json` dev-dependency note (already in dev-deps). Update:

```rust
pub mod error;
pub mod observation;
pub mod result;

pub use error::MatchError;
pub use observation::Observation;
pub use result::MatchResult;
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p stix-matcher`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-matcher/src/observation.rs crates/stix-matcher/src/lib.rs
git commit -m "feat(matcher): add Observation type"
```

---

## Task 5: Object-path resolution

**Files:**
- Create: `crates/stix-matcher/src/resolve.rs`
- Modify: `crates/stix-matcher/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/stix-matcher/src/resolve.rs` with the test module first:

```rust
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
        let p = path("file", vec![PathStep::Key("hashes".into()), PathStep::Key("SHA-256".into())]);
        assert_eq!(resolve_path(&o, &p, None), vec![StixValue::String("abc".into())]);
    }

    #[test]
    fn resolves_index_and_any_index() {
        let o = obj(serde_json::json!({
            "type": "network-traffic", "id": "network-traffic--1",
            "protocols": ["ipv4", "tcp"]
        }));
        let idx = path("network-traffic", vec![PathStep::Key("protocols".into()), PathStep::Index(1)]);
        assert_eq!(resolve_path(&o, &idx, None), vec![StixValue::String("tcp".into())]);

        let any = path("network-traffic", vec![PathStep::Key("protocols".into()), PathStep::AnyIndex]);
        assert_eq!(
            resolve_path(&o, &any, None),
            vec![StixValue::String("ipv4".into()), StixValue::String("tcp".into())]
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
        let p = path("network-traffic", vec![PathStep::Key("src_ref".into()), PathStep::Key("value".into())]);
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-matcher`
Expected: FAIL — `resolve_path` not found.

- [ ] **Step 3: Implement path resolution**

At the top of `crates/stix-matcher/src/resolve.rs` (above the test module):

```rust
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
pub fn resolve_path(obj: &dyn ObjectView, path: &ObjectPath, store: Option<&ObjectStore>) -> Vec<StixValue> {
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
fn apply_step(value: StixValue, step: &PathStep, store: Option<&ObjectStore>, out: &mut Vec<StixValue>) {
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
```

In `crates/stix-matcher/src/lib.rs`, add (keep modules sorted):

```rust
pub mod resolve;
```

(Place `pub mod resolve;` after `pub mod result;`. No re-export needed — it's an internal helper, but keep the module public for testing/tooling.)

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p stix-matcher resolve`
Expected: PASS — all six resolve tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-matcher/src/resolve.rs crates/stix-matcher/src/lib.rs
git commit -m "feat(matcher): add object-path resolution with reference deref"
```

---

## Task 6: Scalar comparison (equality, ordering, IN)

**Files:**
- Create: `crates/stix-matcher/src/compare.rs`
- Modify: `crates/stix-matcher/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/stix-matcher/src/compare.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use stix_model::StixValue;
    use stix_pattern::ast::Literal;

    #[test]
    fn string_equality() {
        assert!(value_eq_literal(&StixValue::String("a".into()), &Literal::String("a".into())));
        assert!(!value_eq_literal(&StixValue::String("a".into()), &Literal::String("b".into())));
    }

    #[test]
    fn numeric_equality_crosses_int_float() {
        assert!(value_eq_literal(&StixValue::Integer(3), &Literal::Integer(3)));
        assert!(value_eq_literal(&StixValue::Integer(3), &Literal::Float(3.0)));
        assert!(value_eq_literal(&StixValue::Float(3.0), &Literal::Integer(3)));
        assert!(!value_eq_literal(&StixValue::Integer(3), &Literal::Integer(4)));
    }

    #[test]
    fn typed_literals_compare_as_strings() {
        assert!(value_eq_literal(
            &StixValue::String("2020-01-01T00:00:00Z".into()),
            &Literal::Timestamp("2020-01-01T00:00:00Z".into())
        ));
        assert!(value_eq_literal(&StixValue::String("cafe".into()), &Literal::Hex("cafe".into())));
    }

    #[test]
    fn ordering() {
        use std::cmp::Ordering;
        assert_eq!(value_cmp_literal(&StixValue::Integer(2), &Literal::Integer(5)), Some(Ordering::Less));
        assert_eq!(value_cmp_literal(&StixValue::String("b".into()), &Literal::String("a".into())), Some(Ordering::Greater));
        assert_eq!(value_cmp_literal(&StixValue::Bool(true), &Literal::Integer(1)), None);
    }

    #[test]
    fn membership() {
        let set = vec![Literal::Integer(1), Literal::Integer(2)];
        assert!(value_in_set(&StixValue::Integer(2), &set));
        assert!(!value_in_set(&StixValue::Integer(3), &set));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-matcher`
Expected: FAIL — `value_eq_literal` not found.

- [ ] **Step 3: Implement scalar comparison**

At the top of `crates/stix-matcher/src/compare.rs` (above the test module):

```rust
//! Scalar comparison between a resolved `StixValue` and a pattern `Literal`.

use std::cmp::Ordering;

use stix_model::StixValue;
use stix_pattern::ast::Literal;

/// The string contents of a string-like literal, if any.
fn literal_str(lit: &Literal) -> Option<&str> {
    match lit {
        Literal::String(s)
        | Literal::Timestamp(s)
        | Literal::Binary(s)
        | Literal::Hex(s) => Some(s),
        _ => None,
    }
}

/// The numeric value of a numeric literal, if any.
fn literal_f64(lit: &Literal) -> Option<f64> {
    match lit {
        Literal::Integer(n) => Some(*n as f64),
        Literal::Float(f) => Some(*f),
        _ => None,
    }
}

/// Equality between a value and a literal, with int/float promotion and
/// string-typed-literal comparison.
pub fn value_eq_literal(value: &StixValue, lit: &Literal) -> bool {
    match (value, lit) {
        (StixValue::Bool(b), Literal::Boolean(l)) => b == l,
        _ => {
            if let (Some(v), Some(l)) = (value.as_str(), literal_str(lit)) {
                return v == l;
            }
            if let (Some(v), Some(l)) = (value.as_f64(), literal_f64(lit)) {
                return v == l;
            }
            false
        }
    }
}

/// Ordering between a value and a literal (numeric or string), or `None` if the
/// two are not comparable.
pub fn value_cmp_literal(value: &StixValue, lit: &Literal) -> Option<Ordering> {
    if let (Some(v), Some(l)) = (value.as_f64(), literal_f64(lit)) {
        return v.partial_cmp(&l);
    }
    if let (Some(v), Some(l)) = (value.as_str(), literal_str(lit)) {
        return Some(v.cmp(l));
    }
    None
}

/// Whether a value equals any member of a set literal (`IN`).
pub fn value_in_set(value: &StixValue, set: &[Literal]) -> bool {
    set.iter().any(|lit| value_eq_literal(value, lit))
}
```

In `crates/stix-matcher/src/lib.rs`, add `pub mod compare;` (after `pub mod compare`'s alphabetical spot — place it right after `pub mod error;`... to keep it simple, list modules in this order):

```rust
pub mod compare;
pub mod error;
pub mod observation;
pub mod resolve;
pub mod result;
```

(Keep the existing `pub use` lines.)

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p stix-matcher compare`
Expected: PASS — all five comparison tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-matcher/src/compare.rs crates/stix-matcher/src/lib.rs
git commit -m "feat(matcher): add scalar equality, ordering, and IN comparison"
```

---

## Task 7: LIKE and MATCHES

**Files:**
- Create: `crates/stix-matcher/src/pattern_ops.rs`
- Modify: `crates/stix-matcher/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/stix-matcher/src/pattern_ops.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn like_percent_matches_any_run() {
        assert!(like_matches("foobar.evil.example", "%.evil.example"));
        assert!(!like_matches("foobar.good.example", "%.evil.example"));
    }

    #[test]
    fn like_underscore_matches_single_char() {
        assert!(like_matches("cat", "c_t"));
        assert!(!like_matches("coat", "c_t"));
    }

    #[test]
    fn like_escapes_regex_metachars() {
        // '.' in the pattern is a literal dot, not "any char".
        assert!(like_matches("a.b", "a.b"));
        assert!(!like_matches("axb", "a.b"));
    }

    #[test]
    fn matches_uses_regex() {
        assert!(regex_matches("invoice12", "invoice[0-9]+"));
        assert!(!regex_matches("invoice", "invoice[0-9]+"));
    }

    #[test]
    fn invalid_regex_does_not_match() {
        assert!(!regex_matches("anything", "("));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-matcher`
Expected: FAIL — `like_matches` not found.

- [ ] **Step 3: Implement LIKE and MATCHES**

At the top of `crates/stix-matcher/src/pattern_ops.rs` (above the test module):

```rust
//! The `LIKE` (SQL wildcard) and `MATCHES` (regex) operators.

use regex::Regex;

/// STIX `LIKE`: `%` matches any run of characters, `_` matches exactly one. All
/// other characters match literally. Implemented by translating to an anchored
/// regex with every non-wildcard character escaped.
pub fn like_matches(value: &str, pattern: &str) -> bool {
    let mut regex = String::with_capacity(pattern.len() * 2 + 2);
    regex.push('^');
    for ch in pattern.chars() {
        match ch {
            '%' => regex.push_str(".*"),
            '_' => regex.push('.'),
            other => regex.push_str(&regex::escape(&other.to_string())),
        }
    }
    regex.push('$');
    match Regex::new(&regex) {
        Ok(re) => re.is_match(value),
        Err(_) => false,
    }
}

/// STIX `MATCHES`: PCRE-style regular-expression match (unanchored, like the
/// reference implementation). An invalid regex never matches.
pub fn regex_matches(value: &str, pattern: &str) -> bool {
    match Regex::new(pattern) {
        Ok(re) => re.is_match(value),
        Err(_) => false,
    }
}
```

In `crates/stix-matcher/src/lib.rs`, add `pub mod pattern_ops;` (after `pub mod observation;`).

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p stix-matcher pattern_ops`
Expected: PASS — all five tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-matcher/src/pattern_ops.rs crates/stix-matcher/src/lib.rs
git commit -m "feat(matcher): add LIKE and MATCHES operators"
```

---

## Task 8: ISSUBSET / ISSUPERSET (CIDR)

**Files:**
- Create: `crates/stix-matcher/src/subset.rs`
- Modify: `crates/stix-matcher/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/stix-matcher/src/subset.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv4_address_within_cidr() {
        assert!(is_subset("198.51.100.5", "198.51.100.0/24"));
        assert!(!is_subset("198.51.101.5", "198.51.100.0/24"));
    }

    #[test]
    fn ipv4_cidr_within_cidr() {
        assert!(is_subset("198.51.100.0/25", "198.51.100.0/24"));
        assert!(!is_subset("198.51.100.0/23", "198.51.100.0/24"));
    }

    #[test]
    fn ipv6_within_cidr() {
        assert!(is_subset("2001:db8::1", "2001:db8::/32"));
        assert!(!is_subset("2001:dead::1", "2001:db8::/32"));
    }

    #[test]
    fn mismatched_family_is_not_subset() {
        assert!(!is_subset("198.51.100.5", "2001:db8::/32"));
    }

    #[test]
    fn garbage_is_not_subset() {
        assert!(!is_subset("not-an-ip", "198.51.100.0/24"));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-matcher`
Expected: FAIL — `is_subset` not found.

- [ ] **Step 3: Implement CIDR subset**

At the top of `crates/stix-matcher/src/subset.rs` (above the test module):

```rust
//! `ISSUBSET` / `ISSUPERSET` for IPv4/IPv6 addresses and CIDR ranges.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// A parsed network: an address widened to 128 bits, a prefix length, and whether
/// it is IPv6 (so families are never mixed).
struct Network {
    bits: u128,
    prefix: u32,
    is_v6: bool,
}

/// Parse `"addr"` or `"addr/prefix"` into a `Network`. Bare addresses use the full
/// prefix length (32 for v4, 128 for v6).
fn parse_network(s: &str) -> Option<Network> {
    let (addr_part, prefix_part) = match s.split_once('/') {
        Some((a, p)) => (a, Some(p)),
        None => (s, None),
    };
    let addr: IpAddr = addr_part.parse().ok()?;
    match addr {
        IpAddr::V4(v4) => {
            let prefix = match prefix_part {
                Some(p) => p.parse::<u32>().ok().filter(|p| *p <= 32)?,
                None => 32,
            };
            Some(Network {
                bits: u128::from(u32::from(v4)),
                prefix: prefix + 96, // align v4 into the low 32 bits of a 128-bit space
                is_v6: false,
            })
        }
        IpAddr::V6(v6) => {
            let prefix = match prefix_part {
                Some(p) => p.parse::<u32>().ok().filter(|p| *p <= 128)?,
                None => 128,
            };
            Some(Network {
                bits: u128::from(v6),
                prefix,
                is_v6: true,
            })
        }
    }
}

/// Mask `bits` to its top `prefix` bits (out of 128).
fn masked(bits: u128, prefix: u32) -> u128 {
    if prefix == 0 {
        0
    } else if prefix >= 128 {
        bits
    } else {
        let mask = u128::MAX << (128 - prefix);
        bits & mask
    }
}

/// Whether network `a` is entirely contained within network `b`.
fn network_subset(a: &Network, b: &Network) -> bool {
    if a.is_v6 != b.is_v6 {
        return false;
    }
    // `a` is inside `b` only if it is at least as specific and shares b's prefix.
    a.prefix >= b.prefix && masked(a.bits, b.prefix) == masked(b.bits, b.prefix)
}

/// STIX `ISSUBSET`: is the address/range `value` a subset of `range`?
pub fn is_subset(value: &str, range: &str) -> bool {
    match (parse_network(value), parse_network(range)) {
        (Some(a), Some(b)) => network_subset(&a, &b),
        _ => false,
    }
}

/// STIX `ISSUPERSET`: is `value` a superset of `range`? (i.e. `range` ⊆ `value`)
pub fn is_superset(value: &str, range: &str) -> bool {
    is_subset(range, value)
}

// Silence "field never read" — `is_v6` is read in `network_subset`; this import
// guards against accidental unused warnings if the IP types change.
#[allow(unused_imports)]
use std::net::IpAddr as _IpAddrAlias;
type _Unused = (Ipv4Addr, Ipv6Addr);
```

> Note: the two trailing lines exist only to ensure `Ipv4Addr`/`Ipv6Addr` are
> referenced; if clippy complains about them in Task 13, delete them — they are not
> load-bearing. Cleaner alternative: remove the `Ipv4Addr, Ipv6Addr` from the `use`
> and the `_Unused` line entirely, keeping only `IpAddr`. Prefer that if it compiles.

In `crates/stix-matcher/src/lib.rs`, add `pub mod subset;` (after `pub mod result;`).

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p stix-matcher subset`
Expected: PASS — all five tests pass.

- [ ] **Step 5: Simplify imports and re-run**

Replace the `use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};` line with `use std::net::IpAddr;`
and delete the trailing `#[allow(unused_imports)]` line and the `type _Unused` line.

Run: `cargo test -p stix-matcher subset`
Expected: PASS — still passes, now with no unused-import scaffolding.

- [ ] **Step 6: Commit**

```bash
git add crates/stix-matcher/src/subset.rs crates/stix-matcher/src/lib.rs
git commit -m "feat(matcher): add ISSUBSET/ISSUPERSET CIDR operators"
```

---

## Task 9: Leaf comparison evaluation

**Files:**
- Create: `crates/stix-matcher/src/eval.rs`
- Modify: `crates/stix-matcher/src/lib.rs`

This task implements evaluating a single `Comparison` against a single object. The
comparison-expression and observation-expression layers come in Tasks 10–11.

- [ ] **Step 1: Write the failing test**

Create `crates/stix-matcher/src/eval.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use stix_model::StixObject;
    use stix_pattern::ast::{
        Comparison, ComparisonOperand, ComparisonOperator, Literal, ObjectPath, PathStep,
    };

    fn obj(json: serde_json::Value) -> StixObject {
        StixObject::from_json(json).unwrap()
    }

    fn cmp(
        object_type: &str,
        key: &str,
        operator: ComparisonOperator,
        negated: bool,
        value: ComparisonOperand,
    ) -> Comparison {
        Comparison {
            path: ObjectPath {
                object_type: object_type.to_string(),
                steps: vec![PathStep::Key(key.to_string())],
            },
            operator,
            negated,
            value,
        }
    }

    fn lit(s: &str) -> ComparisonOperand {
        ComparisonOperand::Literal(Literal::String(s.to_string()))
    }

    #[test]
    fn equality_against_object() {
        let o = obj(serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.2.3.4"}));
        let c = cmp("ipv4-addr", "value", ComparisonOperator::Equal, false, lit("1.2.3.4"));
        assert!(eval_comparison(&o, &c, None));

        let c2 = cmp("ipv4-addr", "value", ComparisonOperator::Equal, false, lit("9.9.9.9"));
        assert!(!eval_comparison(&o, &c2, None));
    }

    #[test]
    fn negation_inverts() {
        let o = obj(serde_json::json!({"type": "file", "id": "file--1", "name": "evil.exe"}));
        let c = cmp("file", "name", ComparisonOperator::Equal, true, lit("evil.exe"));
        assert!(!eval_comparison(&o, &c, None));
    }

    #[test]
    fn exists_checks_presence() {
        let o = obj(serde_json::json!({"type": "file", "id": "file--1", "name": "x"}));
        let present = cmp("file", "name", ComparisonOperator::Exists, false, lit("ignored"));
        assert!(eval_comparison(&o, &present, None));
        let absent = cmp("file", "size", ComparisonOperator::Exists, false, lit("ignored"));
        assert!(!eval_comparison(&o, &absent, None));
    }

    #[test]
    fn in_set_against_object() {
        let o = obj(serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "8.8.8.8"}));
        let set = ComparisonOperand::Set(vec![
            Literal::String("1.1.1.1".into()),
            Literal::String("8.8.8.8".into()),
        ]);
        let c = cmp("ipv4-addr", "value", ComparisonOperator::In, false, set);
        assert!(eval_comparison(&o, &c, None));
    }

    #[test]
    fn issubset_against_object() {
        let o = obj(serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "198.51.100.5"}));
        let c = cmp(
            "ipv4-addr",
            "value",
            ComparisonOperator::IsSubset,
            false,
            ComparisonOperand::Literal(Literal::String("198.51.100.0/24".into())),
        );
        assert!(eval_comparison(&o, &c, None));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-matcher`
Expected: FAIL — `eval_comparison` not found.

- [ ] **Step 3: Implement leaf evaluation**

At the top of `crates/stix-matcher/src/eval.rs` (above the test module):

```rust
//! Evaluation: leaf comparisons, comparison expressions, and observation expressions.

use stix_model::{ObjectStore, ObjectView, StixValue};
use stix_pattern::ast::{Comparison, ComparisonOperand, ComparisonOperator, Literal};

use crate::compare::{value_cmp_literal, value_eq_literal, value_in_set};
use crate::pattern_ops::{like_matches, regex_matches};
use crate::resolve::resolve_path;
use crate::subset::{is_subset, is_superset};

/// Evaluate a single `Comparison` against a single object, dereferencing through
/// `store` where the path requires it. Honors the leaf's `negated` flag.
pub fn eval_comparison(obj: &dyn ObjectView, c: &Comparison, store: Option<&ObjectStore>) -> bool {
    let values = resolve_path(obj, &c.path, store);

    let base = if c.operator == ComparisonOperator::Exists {
        !values.is_empty()
    } else {
        values.iter().any(|v| operator_holds(v, c.operator, &c.value))
    };

    base ^ c.negated
}

/// Whether a single resolved value satisfies a (non-EXISTS) operator + operand.
fn operator_holds(value: &StixValue, op: ComparisonOperator, operand: &ComparisonOperand) -> bool {
    use std::cmp::Ordering;

    // `IN` is the only operator that takes a set operand.
    if op == ComparisonOperator::In {
        return match operand {
            ComparisonOperand::Set(set) => value_in_set(value, set),
            ComparisonOperand::Literal(lit) => value_in_set(value, std::slice::from_ref(lit)),
        };
    }

    let lit = match operand {
        ComparisonOperand::Literal(l) => l,
        // A non-IN operator with a set operand is ill-formed; never matches.
        ComparisonOperand::Set(_) => return false,
    };

    match op {
        ComparisonOperator::Equal => value_eq_literal(value, lit),
        ComparisonOperator::NotEqual => !value_eq_literal(value, lit),
        ComparisonOperator::GreaterThan => value_cmp_literal(value, lit) == Some(Ordering::Greater),
        ComparisonOperator::GreaterThanOrEqual => {
            matches!(value_cmp_literal(value, lit), Some(Ordering::Greater | Ordering::Equal))
        }
        ComparisonOperator::LessThan => value_cmp_literal(value, lit) == Some(Ordering::Less),
        ComparisonOperator::LessThanOrEqual => {
            matches!(value_cmp_literal(value, lit), Some(Ordering::Less | Ordering::Equal))
        }
        ComparisonOperator::Like => string_op(value, lit, like_matches),
        ComparisonOperator::Matches => string_op(value, lit, regex_matches),
        ComparisonOperator::IsSubset => string_op(value, lit, is_subset),
        ComparisonOperator::IsSuperset => string_op(value, lit, is_superset),
        // Handled above / not reachable here.
        ComparisonOperator::In | ComparisonOperator::Exists => false,
    }
}

/// Apply a `(value_str, literal_str) -> bool` operator, requiring both sides to be
/// strings.
fn string_op(value: &StixValue, lit: &Literal, f: impl Fn(&str, &str) -> bool) -> bool {
    let v = match value.as_str() {
        Some(s) => s,
        None => return false,
    };
    let l = match lit {
        Literal::String(s) | Literal::Timestamp(s) | Literal::Binary(s) | Literal::Hex(s) => s,
        _ => return false,
    };
    f(v, l)
}
```

In `crates/stix-matcher/src/lib.rs`, add `pub mod eval;` (after `pub mod error;`).

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p stix-matcher eval`
Expected: PASS — all five leaf-evaluation tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-matcher/src/eval.rs crates/stix-matcher/src/lib.rs
git commit -m "feat(matcher): evaluate leaf comparisons (all operators, EXISTS, NOT)"
```

---

## Task 10: Comparison-expression evaluation (binding enumeration)

**Files:**
- Modify: `crates/stix-matcher/src/eval.rs`

- [ ] **Step 1: Write the failing test**

Add these tests inside the existing `mod tests` in `crates/stix-matcher/src/eval.rs`:

```rust
    use crate::observation::Observation;
    use stix_pattern::ast::ComparisonExpression;

    fn observation(objs: Vec<serde_json::Value>) -> Observation {
        Observation::new(objs.into_iter().map(obj).collect())
    }

    fn test_expr(c: Comparison) -> ComparisonExpression {
        ComparisonExpression::Test(c)
    }

    #[test]
    fn single_test_matches_some_object() {
        let o = observation(vec![
            serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.2.3.4"}),
            serde_json::json!({"type": "domain-name", "id": "domain-name--1", "value": "evil.example"}),
        ]);
        let expr = test_expr(cmp("domain-name", "value", ComparisonOperator::Equal, false, lit("evil.example")));
        assert!(eval_comparison_expression(&expr, &o, None));
    }

    #[test]
    fn and_requires_same_object_binding() {
        // Two constraints on `file` must be satisfied by ONE file object.
        let matching = observation(vec![
            serde_json::json!({"type": "file", "id": "file--1", "name": "evil.exe", "size": 10}),
        ]);
        let split = observation(vec![
            serde_json::json!({"type": "file", "id": "file--1", "name": "evil.exe", "size": 99}),
            serde_json::json!({"type": "file", "id": "file--2", "name": "ok.txt", "size": 10}),
        ]);
        let expr = ComparisonExpression::And(
            Box::new(test_expr(cmp("file", "name", ComparisonOperator::Equal, false, lit("evil.exe")))),
            Box::new(test_expr(cmp(
                "file",
                "size",
                ComparisonOperator::Equal,
                false,
                ComparisonOperand::Literal(Literal::Integer(10)),
            ))),
        );
        assert!(eval_comparison_expression(&expr, &matching, None));
        // No single file is both name=evil.exe AND size=10, so this must not match.
        assert!(!eval_comparison_expression(&expr, &split, None));
    }

    #[test]
    fn or_matches_either_branch() {
        let o = observation(vec![
            serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.2.3.4"}),
        ]);
        let expr = ComparisonExpression::Or(
            Box::new(test_expr(cmp("ipv4-addr", "value", ComparisonOperator::Equal, false, lit("9.9.9.9")))),
            Box::new(test_expr(cmp("ipv4-addr", "value", ComparisonOperator::Equal, false, lit("1.2.3.4")))),
        );
        assert!(eval_comparison_expression(&expr, &o, None));
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-matcher eval`
Expected: FAIL — `eval_comparison_expression` not found.

- [ ] **Step 3: Implement binding enumeration**

Append these items to `crates/stix-matcher/src/eval.rs` (after the `string_op` function, before the test module). Add to the existing `use stix_pattern::ast::{...}` import: `ComparisonExpression`. Add `use crate::observation::Observation;` and `use std::collections::BTreeMap;` near the top imports.

```rust
/// Evaluate a comparison expression against one observation using binding
/// enumeration: each distinct referenced object-type is bound to one object of
/// that type from the observation (or none); the expression matches if some
/// binding makes the boolean tree true. This gives correct "same object" semantics
/// for `AND` within an observation while staying cheap (observations are small).
pub fn eval_comparison_expression(
    expr: &ComparisonExpression,
    observation: &Observation,
    store: Option<&ObjectStore>,
) -> bool {
    // Distinct object types referenced anywhere in the expression.
    let mut types: Vec<String> = Vec::new();
    collect_types(expr, &mut types);

    // Candidate objects per referenced type (indices into observation.objects).
    let candidates: Vec<Vec<usize>> = types
        .iter()
        .map(|t| {
            observation
                .objects
                .iter()
                .enumerate()
                .filter(|(_, o)| o.type_() == Some(t.as_str()))
                .map(|(i, _)| i)
                .collect()
        })
        .collect();

    // Enumerate one choice per type (or `None` when a type has no candidate).
    let mut binding: BTreeMap<String, usize> = BTreeMap::new();
    enumerate_bindings(&types, &candidates, 0, &mut binding, &|binding| {
        eval_tree(expr, observation, binding, store)
    })
}

/// Recursively collect distinct object types referenced by an expression's leaves.
fn collect_types(expr: &ComparisonExpression, out: &mut Vec<String>) {
    match expr {
        ComparisonExpression::Test(c) => {
            if !out.contains(&c.path.object_type) {
                out.push(c.path.object_type.clone());
            }
        }
        ComparisonExpression::And(a, b) | ComparisonExpression::Or(a, b) => {
            collect_types(a, out);
            collect_types(b, out);
        }
    }
}

/// Try every assignment of one candidate object per type; return true as soon as
/// `predicate` accepts a binding. Types with no candidates are simply absent from
/// the binding map (their leaves evaluate to false).
fn enumerate_bindings(
    types: &[String],
    candidates: &[Vec<usize>],
    idx: usize,
    binding: &mut BTreeMap<String, usize>,
    predicate: &dyn Fn(&BTreeMap<String, usize>) -> bool,
) -> bool {
    if idx == types.len() {
        return predicate(binding);
    }
    if candidates[idx].is_empty() {
        // No object of this type; leave it unbound and continue.
        return enumerate_bindings(types, candidates, idx + 1, binding, predicate);
    }
    for &obj_idx in &candidates[idx] {
        binding.insert(types[idx].clone(), obj_idx);
        if enumerate_bindings(types, candidates, idx + 1, binding, predicate) {
            binding.remove(&types[idx]);
            return true;
        }
    }
    binding.remove(&types[idx]);
    false
}

/// Evaluate the boolean tree under a fixed binding.
fn eval_tree(
    expr: &ComparisonExpression,
    observation: &Observation,
    binding: &BTreeMap<String, usize>,
    store: Option<&ObjectStore>,
) -> bool {
    match expr {
        ComparisonExpression::Test(c) => match binding.get(&c.path.object_type) {
            Some(&obj_idx) => eval_comparison(&observation.objects[obj_idx], c, store),
            None => false,
        },
        ComparisonExpression::And(a, b) => {
            eval_tree(a, observation, binding, store) && eval_tree(b, observation, binding, store)
        }
        ComparisonExpression::Or(a, b) => {
            eval_tree(a, observation, binding, store) || eval_tree(b, observation, binding, store)
        }
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p stix-matcher eval`
Expected: PASS — including the `and_requires_same_object_binding` case.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-matcher/src/eval.rs
git commit -m "feat(matcher): evaluate comparison expressions via binding enumeration"
```

---

## Task 11: Observation-expression evaluation + entry points

**Files:**
- Modify: `crates/stix-matcher/src/eval.rs`
- Modify: `crates/stix-matcher/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Add these tests inside the existing `mod tests` in `crates/stix-matcher/src/eval.rs`:

```rust
    use stix_pattern::parse;

    #[test]
    fn single_observation_matches_across_set() {
        let observations = vec![
            observation(vec![serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.1.1.1"})]),
            observation(vec![serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--2", "value": "1.2.3.4"})]),
        ];
        let pattern = parse("[ipv4-addr:value = '1.2.3.4']").unwrap();
        let result = eval_pattern(&pattern, &observations, None).unwrap();
        assert!(result.is_match());
        assert_eq!(result.observations(), &[1]);
    }

    #[test]
    fn observation_and_needs_both() {
        let observations = vec![
            observation(vec![serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.1.1.1"})]),
            observation(vec![serde_json::json!({"type": "domain-name", "id": "domain-name--1", "value": "evil.example"})]),
        ];
        let yes = parse("[ipv4-addr:value = '1.1.1.1'] AND [domain-name:value = 'evil.example']").unwrap();
        assert!(eval_pattern(&yes, &observations, None).unwrap().is_match());

        let no = parse("[ipv4-addr:value = '1.1.1.1'] AND [domain-name:value = 'good.example']").unwrap();
        assert!(!eval_pattern(&no, &observations, None).unwrap().is_match());
    }

    #[test]
    fn followedby_is_unsupported() {
        let observations =
            vec![observation(vec![serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.1.1.1"})])];
        let pattern = parse("[ipv4-addr:value = '1.1.1.1'] FOLLOWEDBY [ipv4-addr:value = '2.2.2.2']").unwrap();
        let err = eval_pattern(&pattern, &observations, None).unwrap_err();
        assert!(matches!(err, crate::error::MatchError::Unsupported(_)));
    }

    #[test]
    fn qualifier_is_unsupported() {
        let observations =
            vec![observation(vec![serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.1.1.1"})])];
        let pattern = parse("[ipv4-addr:value = '1.1.1.1'] REPEATS 2 TIMES").unwrap();
        let err = eval_pattern(&pattern, &observations, None).unwrap_err();
        assert!(matches!(err, crate::error::MatchError::Unsupported(_)));
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-matcher eval`
Expected: FAIL — `eval_pattern` not found.

- [ ] **Step 3: Implement observation-expression evaluation**

Append to `crates/stix-matcher/src/eval.rs` (after `eval_tree`, before the test module). Add to imports: `use stix_pattern::ast::{ObservationExpression, Pattern};`, `use crate::error::MatchError;`, `use crate::result::MatchResult;`.

```rust
/// Evaluate a whole pattern against a list of observations.
///
/// Phase 1: single observations and observation-level `AND`/`OR`. `FOLLOWEDBY` and
/// qualifiers (`WITHIN`/`REPEATS`/`START..STOP`) are parsed but return
/// `MatchError::Unsupported` rather than silently passing.
pub fn eval_pattern(
    pattern: &Pattern,
    observations: &[Observation],
    store: Option<&ObjectStore>,
) -> Result<MatchResult, MatchError> {
    let mut matched = Vec::new();
    let is_match = eval_observation_expression(&pattern.expression, observations, store, &mut matched)?;
    if is_match {
        matched.sort_unstable();
        matched.dedup();
        Ok(MatchResult::matched(matched))
    } else {
        Ok(MatchResult::no_match())
    }
}

/// Returns whether the observation expression matches, accumulating the indices of
/// observations that satisfied any `[ ... ]` leaf into `matched`.
fn eval_observation_expression(
    expr: &ObservationExpression,
    observations: &[Observation],
    store: Option<&ObjectStore>,
    matched: &mut Vec<usize>,
) -> Result<bool, MatchError> {
    match expr {
        ObservationExpression::Observation(comparison) => {
            let mut any = false;
            for (i, obs) in observations.iter().enumerate() {
                if eval_comparison_expression(comparison, obs, store) {
                    matched.push(i);
                    any = true;
                }
            }
            Ok(any)
        }
        ObservationExpression::And(a, b) => {
            let left = eval_observation_expression(a, observations, store, matched)?;
            let right = eval_observation_expression(b, observations, store, matched)?;
            Ok(left && right)
        }
        ObservationExpression::Or(a, b) => {
            let left = eval_observation_expression(a, observations, store, matched)?;
            let right = eval_observation_expression(b, observations, store, matched)?;
            Ok(left || right)
        }
        ObservationExpression::FollowedBy(_, _) => Err(MatchError::Unsupported(
            "FOLLOWEDBY sequencing is not yet implemented".to_string(),
        )),
        ObservationExpression::Qualified { .. } => Err(MatchError::Unsupported(
            "observation qualifiers (WITHIN/REPEATS/START..STOP) are not yet implemented".to_string(),
        )),
    }
}
```

- [ ] **Step 4: Add the public entry points to lib.rs**

Replace `crates/stix-matcher/src/lib.rs` with (module list + the four entry points):

```rust
//! Match STIX 2.1 patterns against observed STIX objects.
//!
//! # Example
//!
//! ```
//! use stix_matcher::{match_scos};
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
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p stix-matcher`
Expected: PASS — all eval tests, including the FOLLOWEDBY/qualifier `Unsupported` cases and the doc test.

- [ ] **Step 6: Commit**

```bash
git add crates/stix-matcher/src/eval.rs crates/stix-matcher/src/lib.rs
git commit -m "feat(matcher): observation-expression eval + four entry points"
```

---

## Task 12: Integration tests

**Files:**
- Create: `crates/stix-matcher/tests/fixtures/bundle.json`
- Create: `crates/stix-matcher/tests/integration.rs`

- [ ] **Step 1: Create the fixture**

Create `crates/stix-matcher/tests/fixtures/bundle.json`:

```json
{
  "type": "bundle",
  "id": "bundle--m1",
  "objects": [
    { "type": "ipv4-addr", "id": "ipv4-addr--m1", "value": "198.51.100.5" },
    { "type": "domain-name", "id": "domain-name--m1", "value": "evil.example" },
    { "type": "network-traffic", "id": "network-traffic--m1", "src_ref": "ipv4-addr--m1" },
    {
      "type": "observed-data",
      "id": "observed-data--m1",
      "first_observed": "2020-03-01T12:00:00Z",
      "last_observed": "2020-03-01T12:10:00Z",
      "number_observed": 1,
      "object_refs": ["ipv4-addr--m1", "domain-name--m1", "network-traffic--m1"]
    }
  ]
}
```

- [ ] **Step 2: Write the integration tests**

Create `crates/stix-matcher/tests/integration.rs`:

```rust
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
    let p = parse("[ipv4-addr:value = '198.51.100.5' AND domain-name:value = 'evil.example']").unwrap();
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
    let p = parse("[ipv4-addr:value = '198.51.100.5'] FOLLOWEDBY [domain-name:value = 'evil.example']").unwrap();
    assert!(match_bundle(&p, &b).is_err());
}
```

- [ ] **Step 3: Run the integration tests**

Run: `cargo test -p stix-matcher --test integration`
Expected: PASS — all seven end-to-end tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/stix-matcher/tests/
git commit -m "test(matcher): add end-to-end matching integration tests"
```

---

## Task 13: stix umbrella crate

**Files:**
- Modify: `crates/stix/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Replace `crates/stix/src/lib.rs` with:

```rust
//! Umbrella crate for the **stix-rust** toolkit.
//!
//! Re-exports the parser ([`pattern`]), object model ([`model`]), and matcher
//! ([`matcher`]) so downstream code can depend on a single crate.
//!
//! # Example
//!
//! ```
//! use stix::parse;
//! use stix::matcher::match_scos;
//! use stix::model::StixObject;
//!
//! let pattern = parse("[ipv4-addr:value = '198.51.100.1']").unwrap();
//! let sco = StixObject::from_json(serde_json::json!({
//!     "type": "ipv4-addr", "id": "ipv4-addr--1", "value": "198.51.100.1"
//! })).unwrap();
//! assert!(match_scos(&pattern, &[sco]).unwrap().is_match());
//! ```

pub use stix_matcher as matcher;
pub use stix_model as model;
pub use stix_pattern as pattern;

/// Parse a STIX pattern string into an AST (re-export of [`stix_pattern::parse`]).
pub use stix_pattern::parse;

/// The high-level matching entry points.
pub use stix_matcher::{match_bundle, match_observations, match_observed_data, match_scos};
```

> The doc test uses `serde_json::json!`; add `serde_json` to the umbrella crate's
> dev-dependencies in the next step so the doc test compiles.

- [ ] **Step 2: Add serde_json dev-dependency to the umbrella crate**

Append to `crates/stix/Cargo.toml`:

```toml
[dev-dependencies]
serde_json = { workspace = true }
```

- [ ] **Step 3: Run the umbrella tests**

Run: `cargo test -p stix`
Expected: PASS — the doc test compiles and matches.

- [ ] **Step 4: Commit**

```bash
git add crates/stix/src/lib.rs crates/stix/Cargo.toml
git commit -m "feat(stix): umbrella crate re-exporting parser, model, and matcher"
```

---

## Task 14: Lint, format, final verification, README update

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Format**

Run: `cargo fmt --all`
Then review: `git diff`.

- [ ] **Step 2: Clippy across the workspace (warnings as errors)**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: no warnings. If clippy flags the `subset.rs` IP scaffolding, ensure Task 8
Step 5's simplification was applied (only `use std::net::IpAddr;`). Fix any other
lints inline.

- [ ] **Step 3: Full workspace test run**

Run: `cargo test`
Expected: all four crates' suites PASS (`stix-pattern`, `stix-model`, `stix-matcher`, `stix`).

- [ ] **Step 4: Flip the matcher to ✅ in the README**

In `README.md`, update the status table row for `stix-matcher` from
`| 🚧 In progress |` to `| ✅ Available  |`, and the `stix` row from
`| 🚧 Planned    |` to `| ✅ Available  |`. In the Roadmap section, check the two
boxes:

```markdown
- [x] **`stix-matcher`** — object-path resolution, all comparison operators, boolean logic, multiple entry points (observations / observed-data / bundle / SCO list)
- [x] **`stix`** — umbrella crate with high-level entry points
```

Also update the "Matching *(coming with `stix-matcher`)*" subsection: change the
heading to "### Matching" and the fenced block from ```rust,ignore``` to ```rust```
is **not** required (the README isn't compiled), but update the example imports to
the real API:

```rust
use stix::{parse, matcher::match_bundle};

let pattern = parse("[ipv4-addr:value = '198.51.100.5']").unwrap();
let result = match_bundle(&pattern, &bundle).unwrap();
assert!(result.is_match());
```

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore(matcher): fmt + clippy clean; mark matcher available in README"
```

---

## Self-Review Notes (already applied)

- **Spec coverage:** normalized `Observation` input (Task 4); object-path resolution
  incl. list/`*`/reference deref (Task 5); all comparison operators —
  `=`/`!=`/`<`/`<=`/`>`/`>=` (Task 6), `LIKE`/`MATCHES` (Task 7),
  `ISSUBSET`/`ISSUPERSET` (Task 8), `IN`/`EXISTS`/`NOT` (Tasks 6/9); boolean logic
  with binding (Task 10); four entry points and the typed `Unsupported` result for
  FOLLOWEDBY/qualifiers (Task 11); `stix` umbrella (Task 13). All `stix-matcher`
  spec bullets map to a task.
- **Deliberate deviation:** comparison-expression evaluation uses single-object-
  per-type binding enumeration (documented in Task 10). This is correct for the
  dominant patterns and far simpler than the reference's full binding-set engine;
  more exotic multi-binding semantics remain future work alongside FOLLOWEDBY.
- **Type consistency:** entry points `match_observations`/`match_observed_data`/
  `match_bundle`/`match_scos`, `Observation { objects, first_observed,
  last_observed, number_observed }`, `MatchResult::{no_match, matched, is_match,
  observations}`, `MatchError::Unsupported`, and the eval helpers
  (`resolve_path`, `eval_comparison`, `eval_comparison_expression`, `eval_pattern`)
  are used consistently across tasks and tests. AST/model type and field names match
  the crates as built (verified against `stix-pattern`/`stix-model` source).
- **No placeholders:** every code step contains complete, compilable code. The one
  scaffolding wart in Task 8 is explicitly removed in Task 8 Step 5.
```
