# stix-model Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `stix-model` crate — the STIX object model: a uniform dynamic `StixValue`, a `ObjectView` trait, a generic value-backed object plus a typed `ObservedData` SDO (the hybrid "typed core + generic fallback"), a `Bundle` parser, an `ObjectStore` for id/reference resolution, and a `SpecVersion` seam.

**Architecture:** A new workspace crate, independent of `stix-pattern` (depends only on serde/serde_json/thiserror). Objects deserialize by peeking their `type`: known types (`observed-data`) become typed structs; everything else becomes a `GenericObject` value bag. Both implement `ObjectView`, whose `property(name) -> Option<StixValue>` returns owned values so typed structs can synthesize values without dual storage. Path-walking over a pattern `ObjectPath` is intentionally *not* here — it belongs to `stix-matcher` (Plan 3), the only crate depending on both. This keeps `stix-model` reusable on its own.

**Tech Stack:** Rust (edition 2021), `serde` + `serde_json` (object/bundle (de)serialization), `thiserror` (errors).

---

## Design decisions (read before starting)

- **`ObjectView::property` returns an owned `StixValue`.** Typed structs (e.g. `ObservedData`) synthesize the value (`StixValue::String(self.id.clone())`); the generic object clones from its map. This avoids storing every typed field twice and gives the matcher one uniform accessor.
- **`StixValue` is JSON-shaped** (null/bool/int/float/string/list/object). STIX timestamps, hex, and binary are carried as strings at the value layer; the matcher compares them against pattern literals. Integers and floats are distinguished so numeric comparisons work.
- **Hybrid model:** `StixObject::Typed(TypedObject)` | `StixObject::Generic(GenericObject)`. Phase 1 implements exactly one typed variant, `ObservedData` (it carries the temporal + `object_refs` fields the matcher needs). Adding more typed SDO/SCO structs later is additive — a new `TypedObject` variant + a new arm in the deserialization dispatch.
- **Version seam:** `SpecVersion` enum (only `V2_1` for now). `ObservedData` tolerates both 2.1 `object_refs` and 2.0 inline `objects` so the seam is real but unobtrusive.
- **No `stix-pattern` dependency.** `ObjectView` deliberately does not take an `ObjectPath`.

## File Structure

- `Cargo.toml` (workspace root) — add `crates/stix-model` to `members`.
- `crates/stix-model/Cargo.toml` — crate manifest.
- `crates/stix-model/src/lib.rs` — module wiring + re-exports.
- `crates/stix-model/src/error.rs` — `ModelError`.
- `crates/stix-model/src/value.rs` — `StixValue` + `From<serde_json::Value>` + accessors.
- `crates/stix-model/src/view.rs` — `ObjectView` trait + `GenericObject`.
- `crates/stix-model/src/sdo.rs` — typed `ObservedData` SDO + its `ObjectView` impl.
- `crates/stix-model/src/object.rs` — `StixObject` / `TypedObject` enums + deserialization dispatch.
- `crates/stix-model/src/bundle.rs` — `Bundle`.
- `crates/stix-model/src/store.rs` — `ObjectStore`.
- `crates/stix-model/src/version.rs` — `SpecVersion`.
- `crates/stix-model/tests/fixtures/bundle.json` — a real STIX 2.1 bundle.
- `crates/stix-model/tests/integration.rs` — end-to-end bundle → store → view test.

---

## Task 1: Crate scaffold

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Create: `crates/stix-model/Cargo.toml`
- Create: `crates/stix-model/src/lib.rs`

- [ ] **Step 1: Add the crate to the workspace members**

In the root `Cargo.toml`, change the `members` line to:

```toml
members = ["crates/stix-pattern", "crates/stix-model"]
```

- [ ] **Step 2: Create the crate manifest**

Create `crates/stix-model/Cargo.toml`:

```toml
[package]
name = "stix-model"
version = "0.0.1"
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "STIX 2.1 object model: values, objects, bundles, and an object store."

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
```

- [ ] **Step 3: Create a placeholder lib.rs**

Create `crates/stix-model/src/lib.rs`:

```rust
//! STIX 2.1 object model: values, objects, bundles, and an object store.

#[cfg(test)]
mod smoke {
    #[test]
    fn crate_builds() {
        assert_eq!(2 + 2, 4);
    }
}
```

- [ ] **Step 4: Verify the workspace builds**

Run: `cargo test -p stix-model`
Expected: compiles; `smoke::crate_builds` passes.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/stix-model/Cargo.toml crates/stix-model/src/lib.rs
git commit -m "feat(model): scaffold stix-model crate"
```

---

## Task 2: Error type

**Files:**
- Create: `crates/stix-model/src/error.rs`
- Modify: `crates/stix-model/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/stix-model/src/error.rs`:

```rust
//! Error type for the object model.

use thiserror::Error;

/// Errors produced while importing or interpreting STIX objects.
#[derive(Debug, Error)]
pub enum ModelError {
    /// The JSON could not be parsed.
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),

    /// A required property was missing or had the wrong type.
    #[error("invalid STIX object: {0}")]
    InvalidObject(String),

    /// The input was not a STIX bundle.
    #[error("not a STIX bundle: {0}")]
    NotABundle(String),
}

/// Convenience alias for results in this crate.
pub type Result<T> = std::result::Result<T, ModelError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_object_displays_message() {
        let e = ModelError::InvalidObject("missing id".to_string());
        assert!(format!("{e}").contains("missing id"));
    }

    #[test]
    fn json_error_converts() {
        let json_err = serde_json::from_str::<serde_json::Value>("{bad").unwrap_err();
        let e: ModelError = json_err.into();
        assert!(matches!(e, ModelError::Json(_)));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-model`
Expected: FAIL — `error` module not declared.

- [ ] **Step 3: Wire the module**

Set `crates/stix-model/src/lib.rs` to:

```rust
//! STIX 2.1 object model: values, objects, bundles, and an object store.

pub mod error;

pub use error::{ModelError, Result};
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p stix-model`
Expected: PASS — both error tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-model/src/error.rs crates/stix-model/src/lib.rs
git commit -m "feat(model): add ModelError"
```

---

## Task 3: StixValue

**Files:**
- Create: `crates/stix-model/src/value.rs`
- Modify: `crates/stix-model/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/stix-model/src/value.rs` with the test module first (implementation added in Step 3):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_json_scalars() {
        assert_eq!(StixValue::from(serde_json::json!(null)), StixValue::Null);
        assert_eq!(StixValue::from(serde_json::json!(true)), StixValue::Bool(true));
        assert_eq!(StixValue::from(serde_json::json!(42)), StixValue::Integer(42));
        assert_eq!(StixValue::from(serde_json::json!(-7)), StixValue::Integer(-7));
        assert_eq!(StixValue::from(serde_json::json!(2.5)), StixValue::Float(2.5));
        assert_eq!(
            StixValue::from(serde_json::json!("hi")),
            StixValue::String("hi".to_string())
        );
    }

    #[test]
    fn from_json_nested() {
        let v = StixValue::from(serde_json::json!({"a": [1, "x"], "b": true}));
        match v {
            StixValue::Object(map) => {
                assert_eq!(
                    map.get("a"),
                    Some(&StixValue::List(vec![
                        StixValue::Integer(1),
                        StixValue::String("x".to_string())
                    ]))
                );
                assert_eq!(map.get("b"), Some(&StixValue::Bool(true)));
            }
            _ => panic!("expected object"),
        }
    }

    #[test]
    fn accessors() {
        assert_eq!(StixValue::String("s".into()).as_str(), Some("s"));
        assert_eq!(StixValue::Integer(3).as_i64(), Some(3));
        assert_eq!(StixValue::Float(1.5).as_f64(), Some(1.5));
        assert_eq!(StixValue::Integer(3).as_f64(), Some(3.0));
        assert_eq!(StixValue::Bool(true).as_bool(), Some(true));
        assert!(StixValue::Null.is_null());
        assert_eq!(StixValue::String("s".into()).as_i64(), None);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-model`
Expected: FAIL — `StixValue` not found.

- [ ] **Step 3: Implement StixValue**

At the top of `crates/stix-model/src/value.rs` (above the test module):

```rust
//! `StixValue`: a uniform, JSON-shaped value the matcher can walk.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A dynamic STIX property value.
///
/// STIX timestamps, hex, and binary are carried as [`StixValue::String`] at this
/// layer; higher layers interpret them. Integers and floats are kept distinct so
/// numeric comparisons behave correctly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StixValue {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    List(Vec<StixValue>),
    Object(BTreeMap<String, StixValue>),
}

impl StixValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            StixValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            StixValue::Integer(n) => Some(*n),
            _ => None,
        }
    }

    /// Returns the value as `f64`, promoting integers.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            StixValue::Float(f) => Some(*f),
            StixValue::Integer(n) => Some(*n as f64),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            StixValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[StixValue]> {
        match self {
            StixValue::List(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&BTreeMap<String, StixValue>> {
        match self {
            StixValue::Object(map) => Some(map),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, StixValue::Null)
    }
}

impl From<serde_json::Value> for StixValue {
    fn from(v: serde_json::Value) -> Self {
        use serde_json::Value;
        match v {
            Value::Null => StixValue::Null,
            Value::Bool(b) => StixValue::Bool(b),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    StixValue::Integer(i)
                } else {
                    // Falls back to float for u64-too-big or fractional numbers.
                    StixValue::Float(n.as_f64().unwrap_or(0.0))
                }
            }
            Value::String(s) => StixValue::String(s),
            Value::Array(arr) => StixValue::List(arr.into_iter().map(StixValue::from).collect()),
            Value::Object(obj) => {
                StixValue::Object(obj.into_iter().map(|(k, v)| (k, StixValue::from(v))).collect())
            }
        }
    }
}
```

In `crates/stix-model/src/lib.rs`, add:

```rust
pub mod value;

pub use value::StixValue;
```

(Keep the existing `error` module and its re-exports.)

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p stix-model`
Expected: PASS — all `value` tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-model/src/value.rs crates/stix-model/src/lib.rs
git commit -m "feat(model): add StixValue with JSON conversion and accessors"
```

---

## Task 4: ObjectView trait + GenericObject

**Files:**
- Create: `crates/stix-model/src/view.rs`
- Modify: `crates/stix-model/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/stix-model/src/view.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::StixValue;

    fn sample() -> GenericObject {
        let v = serde_json::json!({
            "type": "ipv4-addr",
            "id": "ipv4-addr--1",
            "value": "198.51.100.1"
        });
        GenericObject::from_json(v).unwrap()
    }

    #[test]
    fn exposes_id_and_type() {
        let o = sample();
        assert_eq!(o.type_(), Some("ipv4-addr"));
        assert_eq!(o.id(), Some("ipv4-addr--1"));
    }

    #[test]
    fn property_returns_owned_value() {
        let o = sample();
        assert_eq!(o.property("value"), Some(StixValue::String("198.51.100.1".into())));
        assert_eq!(o.property("missing"), None);
    }

    #[test]
    fn rejects_non_object_json() {
        let err = GenericObject::from_json(serde_json::json!([1, 2, 3])).unwrap_err();
        assert!(matches!(err, crate::error::ModelError::InvalidObject(_)));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-model`
Expected: FAIL — `ObjectView` / `GenericObject` not found.

- [ ] **Step 3: Implement the trait and generic object**

At the top of `crates/stix-model/src/view.rs` (above the test module):

```rust
//! The `ObjectView` trait and the generic value-backed object.

use std::collections::BTreeMap;

use crate::error::{ModelError, Result};
use crate::value::StixValue;

/// A read-only, type-agnostic view over a STIX object.
///
/// `property` returns an *owned* [`StixValue`] so typed objects can synthesize
/// values on demand without storing every field twice. The matcher consumes only
/// this trait, so it never needs to branch on typed vs. generic objects.
pub trait ObjectView {
    fn id(&self) -> Option<&str>;
    fn type_(&self) -> Option<&str>;
    fn property(&self, name: &str) -> Option<StixValue>;
}

/// A STIX object stored as a flat property map. Used for any object type without
/// a dedicated typed struct, and retains all properties (including custom ones).
#[derive(Debug, Clone, PartialEq)]
pub struct GenericObject {
    properties: BTreeMap<String, StixValue>,
}

impl GenericObject {
    /// Build a generic object from a JSON value. The value must be a JSON object.
    pub fn from_json(value: serde_json::Value) -> Result<Self> {
        match StixValue::from(value) {
            StixValue::Object(properties) => Ok(GenericObject { properties }),
            _ => Err(ModelError::InvalidObject(
                "expected a JSON object".to_string(),
            )),
        }
    }

    /// The full property map.
    pub fn properties(&self) -> &BTreeMap<String, StixValue> {
        &self.properties
    }
}

impl ObjectView for GenericObject {
    fn id(&self) -> Option<&str> {
        self.properties.get("id").and_then(StixValue::as_str)
    }

    fn type_(&self) -> Option<&str> {
        self.properties.get("type").and_then(StixValue::as_str)
    }

    fn property(&self, name: &str) -> Option<StixValue> {
        self.properties.get(name).cloned()
    }
}
```

In `crates/stix-model/src/lib.rs`, add:

```rust
pub mod view;

pub use view::{GenericObject, ObjectView};
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p stix-model`
Expected: PASS — all `view` tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-model/src/view.rs crates/stix-model/src/lib.rs
git commit -m "feat(model): add ObjectView trait and GenericObject"
```

---

## Task 5: Typed ObservedData SDO

**Files:**
- Create: `crates/stix-model/src/sdo.rs`
- Modify: `crates/stix-model/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/stix-model/src/sdo.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::StixValue;
    use crate::view::ObjectView;

    fn sample_json() -> serde_json::Value {
        serde_json::json!({
            "type": "observed-data",
            "id": "observed-data--1",
            "first_observed": "2020-01-01T00:00:00Z",
            "last_observed": "2020-01-01T00:05:00Z",
            "number_observed": 3,
            "object_refs": ["ipv4-addr--1", "domain-name--1"],
            "x_custom": "keep-me"
        })
    }

    #[test]
    fn deserializes_typed_fields() {
        let od: ObservedData = serde_json::from_value(sample_json()).unwrap();
        assert_eq!(od.id, "observed-data--1");
        assert_eq!(od.number_observed, 3);
        assert_eq!(od.first_observed, "2020-01-01T00:00:00Z");
        assert_eq!(od.object_refs, vec!["ipv4-addr--1", "domain-name--1"]);
    }

    #[test]
    fn object_view_exposes_typed_and_custom_props() {
        let od: ObservedData = serde_json::from_value(sample_json()).unwrap();
        assert_eq!(od.type_(), Some("observed-data"));
        assert_eq!(od.id(), Some("observed-data--1"));
        assert_eq!(od.property("number_observed"), Some(StixValue::Integer(3)));
        assert_eq!(
            od.property("first_observed"),
            Some(StixValue::String("2020-01-01T00:00:00Z".into()))
        );
        // custom property preserved via `additional`
        assert_eq!(od.property("x_custom"), Some(StixValue::String("keep-me".into())));
        assert_eq!(od.property("nope"), None);
    }

    #[test]
    fn sco_ids_prefers_object_refs() {
        let od: ObservedData = serde_json::from_value(sample_json()).unwrap();
        assert_eq!(od.sco_ids(), vec!["ipv4-addr--1", "domain-name--1"]);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-model`
Expected: FAIL — `ObservedData` not found.

- [ ] **Step 3: Implement ObservedData**

At the top of `crates/stix-model/src/sdo.rs` (above the test module):

```rust
//! Typed STIX Domain Objects. Phase 1 implements `observed-data`; more are additive.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::value::StixValue;
use crate::view::ObjectView;

/// The STIX `observed-data` SDO.
///
/// Carries the temporal fields and `object_refs` the matcher needs. Unknown or
/// custom properties are retained in `additional` (via `#[serde(flatten)]`) so the
/// `ObjectView` still exposes them. Tolerates STIX 2.0 inline `objects` as well as
/// 2.1 `object_refs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservedData {
    #[serde(rename = "type")]
    pub type_: String,
    pub id: String,
    pub first_observed: String,
    pub last_observed: String,
    pub number_observed: u64,
    #[serde(default)]
    pub object_refs: Vec<String>,
    /// STIX 2.0 inline observed objects (`objects`), if present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objects: Option<BTreeMap<String, StixValue>>,
    /// Any other properties, retained for `ObjectView` and round-tripping.
    #[serde(flatten)]
    pub additional: BTreeMap<String, StixValue>,
}

impl ObservedData {
    /// The referenced SCO ids. Prefers 2.1 `object_refs`; otherwise empty.
    /// (2.0 inline `objects` are keyed locally, not by id, so they yield no refs.)
    pub fn sco_ids(&self) -> Vec<&str> {
        self.object_refs.iter().map(String::as_str).collect()
    }
}

impl ObjectView for ObservedData {
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
            "first_observed" => Some(StixValue::String(self.first_observed.clone())),
            "last_observed" => Some(StixValue::String(self.last_observed.clone())),
            "number_observed" => Some(StixValue::Integer(self.number_observed as i64)),
            "object_refs" => Some(StixValue::List(
                self.object_refs
                    .iter()
                    .map(|s| StixValue::String(s.clone()))
                    .collect(),
            )),
            "objects" => self.objects.clone().map(StixValue::Object),
            other => self.additional.get(other).cloned(),
        }
    }
}
```

In `crates/stix-model/src/lib.rs`, add:

```rust
pub mod sdo;

pub use sdo::ObservedData;
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p stix-model`
Expected: PASS — all `sdo` tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-model/src/sdo.rs crates/stix-model/src/lib.rs
git commit -m "feat(model): add typed ObservedData SDO with ObjectView"
```

---

## Task 6: StixObject enum + deserialization dispatch

**Files:**
- Create: `crates/stix-model/src/object.rs`
- Modify: `crates/stix-model/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/stix-model/src/object.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::StixValue;
    use crate::view::ObjectView;

    #[test]
    fn observed_data_becomes_typed() {
        let v = serde_json::json!({
            "type": "observed-data",
            "id": "observed-data--1",
            "first_observed": "2020-01-01T00:00:00Z",
            "last_observed": "2020-01-01T00:05:00Z",
            "number_observed": 1,
            "object_refs": ["file--1"]
        });
        let obj = StixObject::from_json(v).unwrap();
        assert!(matches!(obj, StixObject::Typed(TypedObject::ObservedData(_))));
        assert_eq!(obj.type_(), Some("observed-data"));
    }

    #[test]
    fn unknown_type_becomes_generic() {
        let v = serde_json::json!({
            "type": "ipv4-addr",
            "id": "ipv4-addr--1",
            "value": "198.51.100.1"
        });
        let obj = StixObject::from_json(v).unwrap();
        assert!(matches!(obj, StixObject::Generic(_)));
        assert_eq!(obj.type_(), Some("ipv4-addr"));
        assert_eq!(obj.property("value"), Some(StixValue::String("198.51.100.1".into())));
    }

    #[test]
    fn missing_type_is_an_error() {
        let v = serde_json::json!({"id": "x--1"});
        assert!(StixObject::from_json(v).is_err());
    }

    #[test]
    fn deserializes_via_serde() {
        // serde path (used by Bundle) routes through the same dispatch.
        let v = serde_json::json!({
            "type": "ipv4-addr",
            "id": "ipv4-addr--2",
            "value": "203.0.113.5"
        });
        let obj: StixObject = serde_json::from_value(v).unwrap();
        assert_eq!(obj.id(), Some("ipv4-addr--2"));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-model`
Expected: FAIL — `StixObject` not found.

- [ ] **Step 3: Implement the enum and dispatch**

At the top of `crates/stix-model/src/object.rs` (above the test module):

```rust
//! `StixObject`: the hybrid typed-or-generic object, with deserialization dispatch.

use serde::de::{Deserialize, Deserializer, Error as DeError};
use serde::Serialize;

use crate::error::{ModelError, Result};
use crate::sdo::ObservedData;
use crate::value::StixValue;
use crate::view::{GenericObject, ObjectView};

/// A STIX object: either a recognized typed object or a generic value bag.
#[derive(Debug, Clone, PartialEq)]
pub enum StixObject {
    Typed(TypedObject),
    Generic(GenericObject),
}

/// The set of types with dedicated typed structs. Additive: new variants slot in
/// here and in [`StixObject::from_json`]'s dispatch.
#[derive(Debug, Clone, PartialEq)]
pub enum TypedObject {
    ObservedData(ObservedData),
}

impl StixObject {
    /// Build a `StixObject` from a JSON value, dispatching on its `type` property.
    pub fn from_json(value: serde_json::Value) -> Result<Self> {
        let type_ = value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| ModelError::InvalidObject("missing 'type' property".to_string()))?
            .to_string();

        match type_.as_str() {
            "observed-data" => {
                let od: ObservedData = serde_json::from_value(value)?;
                Ok(StixObject::Typed(TypedObject::ObservedData(od)))
            }
            _ => Ok(StixObject::Generic(GenericObject::from_json(value)?)),
        }
    }
}

impl ObjectView for StixObject {
    fn id(&self) -> Option<&str> {
        match self {
            StixObject::Typed(t) => t.id(),
            StixObject::Generic(g) => g.id(),
        }
    }

    fn type_(&self) -> Option<&str> {
        match self {
            StixObject::Typed(t) => t.type_(),
            StixObject::Generic(g) => g.type_(),
        }
    }

    fn property(&self, name: &str) -> Option<StixValue> {
        match self {
            StixObject::Typed(t) => t.property(name),
            StixObject::Generic(g) => g.property(name),
        }
    }
}

impl ObjectView for TypedObject {
    fn id(&self) -> Option<&str> {
        match self {
            TypedObject::ObservedData(od) => od.id(),
        }
    }

    fn type_(&self) -> Option<&str> {
        match self {
            TypedObject::ObservedData(od) => od.type_(),
        }
    }

    fn property(&self, name: &str) -> Option<StixValue> {
        match self {
            TypedObject::ObservedData(od) => od.property(name),
        }
    }
}

impl Serialize for StixObject {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            StixObject::Typed(TypedObject::ObservedData(od)) => od.serialize(serializer),
            StixObject::Generic(g) => g.properties().serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for StixObject {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        StixObject::from_json(value).map_err(DeError::custom)
    }
}
```

In `crates/stix-model/src/lib.rs`, add:

```rust
pub mod object;

pub use object::{StixObject, TypedObject};
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p stix-model`
Expected: PASS — all `object` tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-model/src/object.rs crates/stix-model/src/lib.rs
git commit -m "feat(model): add StixObject typed/generic dispatch"
```

---

## Task 7: Bundle

**Files:**
- Create: `crates/stix-model/src/bundle.rs`
- Modify: `crates/stix-model/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/stix-model/src/bundle.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::ObjectView;

    fn bundle_json() -> &'static str {
        r#"{
            "type": "bundle",
            "id": "bundle--1",
            "objects": [
                {"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.2.3.4"},
                {"type": "observed-data", "id": "observed-data--1",
                 "first_observed": "2020-01-01T00:00:00Z",
                 "last_observed": "2020-01-01T00:00:00Z",
                 "number_observed": 1, "object_refs": ["ipv4-addr--1"]}
            ]
        }"#
    }

    #[test]
    fn parses_bundle() {
        let b = Bundle::from_json_str(bundle_json()).unwrap();
        assert_eq!(b.id.as_deref(), Some("bundle--1"));
        assert_eq!(b.objects.len(), 2);
        assert_eq!(b.objects[0].type_(), Some("ipv4-addr"));
        assert_eq!(b.objects[1].type_(), Some("observed-data"));
    }

    #[test]
    fn rejects_non_bundle() {
        let err = Bundle::from_json_str(r#"{"type": "ipv4-addr", "id": "x--1"}"#).unwrap_err();
        assert!(matches!(err, crate::error::ModelError::NotABundle(_)));
    }

    #[test]
    fn round_trips() {
        let b = Bundle::from_json_str(bundle_json()).unwrap();
        let s = serde_json::to_string(&b).unwrap();
        let b2 = Bundle::from_json_str(&s).unwrap();
        assert_eq!(b.objects.len(), b2.objects.len());
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-model`
Expected: FAIL — `Bundle` not found.

- [ ] **Step 3: Implement Bundle**

At the top of `crates/stix-model/src/bundle.rs` (above the test module):

```rust
//! The STIX `bundle` container.

use serde::{Deserialize, Serialize};

use crate::error::{ModelError, Result};
use crate::object::StixObject;

/// A STIX bundle: a `type: "bundle"` envelope around a list of objects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    pub objects: Vec<StixObject>,
}

impl Bundle {
    /// Parse a bundle from a JSON string, validating the `type` is `bundle`.
    pub fn from_json_str(s: &str) -> Result<Self> {
        let bundle: Bundle = serde_json::from_str(s)?;
        if bundle.type_ != "bundle" {
            return Err(ModelError::NotABundle(format!(
                "type was '{}'",
                bundle.type_
            )));
        }
        Ok(bundle)
    }
}
```

In `crates/stix-model/src/lib.rs`, add:

```rust
pub mod bundle;

pub use bundle::Bundle;
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p stix-model`
Expected: PASS — all `bundle` tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-model/src/bundle.rs crates/stix-model/src/lib.rs
git commit -m "feat(model): add Bundle parser"
```

---

## Task 8: ObjectStore

**Files:**
- Create: `crates/stix-model/src/store.rs`
- Modify: `crates/stix-model/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/stix-model/src/store.rs` with the test module first:

```rust
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-model`
Expected: FAIL — `ObjectStore` not found.

- [ ] **Step 3: Implement ObjectStore**

At the top of `crates/stix-model/src/store.rs` (above the test module):

```rust
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
```

In `crates/stix-model/src/lib.rs`, add:

```rust
pub mod store;

pub use store::ObjectStore;
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p stix-model`
Expected: PASS — all `store` tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-model/src/store.rs crates/stix-model/src/lib.rs
git commit -m "feat(model): add ObjectStore for id/reference resolution"
```

---

## Task 9: SpecVersion + end-to-end integration test

**Files:**
- Create: `crates/stix-model/src/version.rs`
- Modify: `crates/stix-model/src/lib.rs`
- Create: `crates/stix-model/tests/fixtures/bundle.json`
- Create: `crates/stix-model/tests/integration.rs`

- [ ] **Step 1: Write the failing unit test for SpecVersion**

Create `crates/stix-model/src/version.rs`:

```rust
//! STIX specification version seam.

use serde::{Deserialize, Serialize};

/// The STIX spec version a document conforms to. Phase 1 targets 2.1; the enum
/// exists so version-specific behavior can be added without API churn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SpecVersion {
    #[default]
    #[serde(rename = "2.1")]
    V2_1,
}

impl SpecVersion {
    /// The canonical version string (e.g. `"2.1"`).
    pub fn as_str(&self) -> &'static str {
        match self {
            SpecVersion::V2_1 => "2.1",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_2_1() {
        assert_eq!(SpecVersion::default(), SpecVersion::V2_1);
        assert_eq!(SpecVersion::default().as_str(), "2.1");
    }
}
```

- [ ] **Step 2: Wire the module and run the unit test**

In `crates/stix-model/src/lib.rs`, add:

```rust
pub mod version;

pub use version::SpecVersion;
```

Run: `cargo test -p stix-model version`
Expected: PASS — `default_is_2_1` passes.

- [ ] **Step 3: Create the integration fixture**

Create `crates/stix-model/tests/fixtures/bundle.json`:

```json
{
  "type": "bundle",
  "id": "bundle--a1",
  "objects": [
    {
      "type": "ipv4-addr",
      "id": "ipv4-addr--a1",
      "value": "198.51.100.5"
    },
    {
      "type": "domain-name",
      "id": "domain-name--a1",
      "value": "evil.example"
    },
    {
      "type": "observed-data",
      "id": "observed-data--a1",
      "first_observed": "2020-03-01T12:00:00Z",
      "last_observed": "2020-03-01T12:10:00Z",
      "number_observed": 5,
      "object_refs": ["ipv4-addr--a1", "domain-name--a1"]
    }
  ]
}
```

- [ ] **Step 4: Write the integration test**

Create `crates/stix-model/tests/integration.rs`:

```rust
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
    assert_eq!(domain.property("value").unwrap().as_str(), Some("evil.example"));
}
```

- [ ] **Step 5: Run the integration test**

Run: `cargo test -p stix-model --test integration`
Expected: PASS — bundle parses, observed-data is typed, refs resolve through the store.

- [ ] **Step 6: Commit**

```bash
git add crates/stix-model/src/version.rs crates/stix-model/src/lib.rs crates/stix-model/tests/
git commit -m "feat(model): add SpecVersion and end-to-end integration test"
```

---

## Task 10: Lint, format, and final verification

**Files:** none (verification only)

- [ ] **Step 1: Format**

Run: `cargo fmt --all`
Then review: `git diff`.

- [ ] **Step 2: Clippy (treat warnings as errors)**

Run: `cargo clippy -p stix-model --all-targets -- -D warnings`
Expected: no warnings. Fix any that appear.

> Note: if clippy flags `len()` without `is_empty()` on `ObjectStore`, both are
> already provided. If it suggests deriving `Eq` for `SpecVersion`, it is already
> derived.

- [ ] **Step 3: Full workspace test run**

Run: `cargo test`
Expected: both `stix-pattern` and `stix-model` test suites PASS.

- [ ] **Step 4: Commit any fmt/clippy fixes**

```bash
git add -A
git commit -m "chore(model): fmt + clippy clean"
```

---

## Self-Review Notes (already applied)

- **Spec coverage:** `StixValue` (Task 3), typed core + generic fallback with `ObjectView`
  (Tasks 4–6), `Bundle` (Task 7), `ObjectStore` resolving refs (Task 8), `SpecVersion`
  seam (Task 9). All `stix-model` spec bullets map to a task.
- **Deliberate spec refinement:** `ObjectView::property` returns an owned `StixValue` and
  does *not* take an `ObjectPath`, so `stix-model` stays independent of `stix-pattern`
  (honoring the spec's dependency edges). `ObjectPath` walking lives in `stix-matcher`
  (Plan 3). Documented at the top of this plan.
- **Phase-1 scope:** exactly one typed SDO (`ObservedData`) — the one the matcher needs.
  More typed SDO/SCO structs are additive (new `TypedObject` variant + dispatch arm).
  This is intentional YAGNI, not a gap.
- **Type consistency:** `ObjectView { id, type_, property }`, `StixValue` variants/accessors,
  `StixObject`/`TypedObject`, `GenericObject::from_json`, `Bundle::from_json_str`,
  `ObjectStore::{from_bundle, from_objects, get, len, is_empty}`, `ObservedData::sco_ids`
  are used consistently across tasks and the integration test.
- **No placeholders:** every code step contains complete, compilable code.
```
