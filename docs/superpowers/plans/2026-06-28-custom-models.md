# Consumer-Injectable Custom Models Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a consumer register their own object models in `stix-model` — typed Rust structs (`register::<T>`) and/or data-level validate/normalize hooks (`register_handler`) — so custom STIX types deserialize, validate, gain computed properties, and match, all without forking the crate, with a documented runnable example.

**Architecture:** Add a `CustomObject` trait (blanket-impl'd over `ObjectView + Serialize`) and a `StixObject::Custom(Arc<dyn CustomObject>)` variant to `stix-model`, plus a `ModelRegistry` that maps a STIX `type` to a handler producing a `StixObject`. The data-level handler is the binding-friendly primitive; `register::<T>` is sugar on top. The matcher is untouched — custom objects flow through `ObjectView`.

**Tech Stack:** Rust (edition 2021), `serde`/`serde_json`, existing `stix-model`/`stix-matcher`/`stix` crates. No new dependencies.

---

## Current shapes (verified against the code)

- `view.rs`: `trait ObjectView { fn id(&self)->Option<&str>; fn type_(&self)->Option<&str>; fn property(&self,name:&str)->Option<StixValue>; }`; `GenericObject` (private `properties: BTreeMap<String,StixValue>`, `from_json`, `properties()`).
- `object.rs`: `#[derive(Debug, Clone, PartialEq)] pub enum StixObject { Typed(TypedObject), Generic(GenericObject) }`; `impl StixObject { pub fn from_json(Value)->Result<Self> }`; exhaustive `impl ObjectView for StixObject` (3 methods) and `impl Serialize for StixObject` — **both must gain a `Custom` arm**; manual `Deserialize` (registry-free, unchanged).
- `bundle.rs`: `pub struct Bundle { pub type_: String, pub id: Option<String>, pub objects: Vec<StixObject> }`.
- `error.rs`: `ModelError::{Json(#[from] serde_json::Error), InvalidObject(String), NotABundle(String)}`; `Result<T>`.

## File Structure

- `crates/stix-model/src/view.rs` — add `CustomObject` trait + blanket impl.
- `crates/stix-model/src/object.rs` — add `Custom` variant, manual `PartialEq`, `Custom` arms, `downcast_ref`.
- `crates/stix-model/src/registry.rs` — **new**: `ModelRegistry`.
- `crates/stix-model/src/lib.rs` — re-exports.
- `crates/stix/Cargo.toml` — add `serde` dev-dependency (for the example/test).
- `crates/stix/examples/custom_model.rs` — **new**: runnable end-to-end example.
- `crates/stix/tests/custom_models.rs` — **new**: end-to-end match integration test.
- `README.md` — new "Custom object types" section.

---

## Task 1: CustomObject trait + blanket impl

**Files:**
- Modify: `crates/stix-model/src/view.rs`
- Modify: `crates/stix-model/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Add to the bottom of the `mod tests` block in `crates/stix-model/src/view.rs` (inside the existing test module, after the last test):

```rust
    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    struct TestWidget {
        #[serde(rename = "type")]
        type_: String,
        id: String,
        risk: i64,
    }

    impl ObjectView for TestWidget {
        fn id(&self) -> Option<&str> {
            Some(&self.id)
        }
        fn type_(&self) -> Option<&str> {
            Some(&self.type_)
        }
        fn property(&self, name: &str) -> Option<StixValue> {
            match name {
                "risk" => Some(StixValue::Integer(self.risk)),
                _ => None,
            }
        }
    }

    #[test]
    fn custom_object_blanket_impl_provides_json_and_any() {
        let w = TestWidget {
            type_: "x-widget".into(),
            id: "x-widget--1".into(),
            risk: 90,
        };
        // Blanket impl gives `as_json` and `as_any` for free.
        let json = CustomObject::as_json(&w);
        assert_eq!(json["risk"], serde_json::json!(90));
        assert!(w.as_any().downcast_ref::<TestWidget>().is_some());
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-model view`
Expected: FAIL — `CustomObject` not found.

- [ ] **Step 3: Implement the trait + blanket impl**

In `crates/stix-model/src/view.rs`, add imports at the top (below the existing `use` lines):

```rust
use std::any::Any;

use serde::Serialize;
```

Then add, after the `ObjectView` trait definition (before `GenericObject`):

```rust
/// A consumer-supplied object type that the matcher can view uniformly.
///
/// Blanket-implemented for any [`ObjectView`] that is also `Serialize`, so a
/// consumer only writes an `ObjectView` impl. `as_json` backs serialization and
/// equality; `as_any` enables downcasting back to the concrete type.
pub trait CustomObject: ObjectView + std::fmt::Debug + Send + Sync {
    fn as_json(&self) -> serde_json::Value;
    fn as_any(&self) -> &dyn Any;
}

impl<T> CustomObject for T
where
    T: ObjectView + Serialize + std::fmt::Debug + Send + Sync + 'static,
{
    fn as_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
```

In `crates/stix-model/src/lib.rs`, update the `view` re-export line:

```rust
pub use view::{CustomObject, GenericObject, ObjectView};
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p stix-model view`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-model/src/view.rs crates/stix-model/src/lib.rs
git commit -m "feat(model): add CustomObject trait with blanket impl"
```

---

## Task 2: StixObject::Custom variant

**Files:**
- Modify: `crates/stix-model/src/object.rs`

- [ ] **Step 1: Write the failing test**

Add to the bottom of the `mod tests` block in `crates/stix-model/src/object.rs`:

```rust
    use std::sync::Arc;

    #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Widget {
        #[serde(rename = "type")]
        type_: String,
        id: String,
        risk: i64,
    }

    impl ObjectView for Widget {
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
                "risk" => Some(StixValue::Integer(self.risk)),
                _ => None,
            }
        }
    }

    fn widget(risk: i64) -> StixObject {
        StixObject::Custom(Arc::new(Widget {
            type_: "x-widget".into(),
            id: "x-widget--1".into(),
            risk,
        }))
    }

    #[test]
    fn custom_exposes_object_view() {
        let o = widget(90);
        assert_eq!(o.type_(), Some("x-widget"));
        assert_eq!(o.id(), Some("x-widget--1"));
        assert_eq!(o.property("risk"), Some(StixValue::Integer(90)));
        assert_eq!(o.property("missing"), None);
    }

    #[test]
    fn custom_downcasts_to_concrete_type() {
        let o = widget(90);
        let w = o.downcast_ref::<Widget>().expect("downcast");
        assert_eq!(w.risk, 90);
        // Wrong target type yields None.
        assert!(o.downcast_ref::<String>().is_none());
        // Non-custom objects yield None.
        let generic = StixObject::from_json(serde_json::json!({"type":"x","id":"x--1"})).unwrap();
        assert!(generic.downcast_ref::<Widget>().is_none());
    }

    #[test]
    fn custom_clone_and_eq_and_serialize() {
        assert_eq!(widget(90), widget(90));
        assert_ne!(widget(90), widget(10));
        let json = serde_json::to_value(widget(90)).unwrap();
        assert_eq!(json["risk"], serde_json::json!(90));
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-model object`
Expected: FAIL — `StixObject::Custom` / `downcast_ref` not found, and compile errors.

- [ ] **Step 3: Add the variant and supporting impls**

In `crates/stix-model/src/object.rs`:

(a) Update the top imports — replace the `use crate::view::{GenericObject, ObjectView};` line with:

```rust
use std::sync::Arc;

use crate::view::{CustomObject, GenericObject, ObjectView};
```

(b) Change the `StixObject` derive (remove `PartialEq`, which we hand-write) and add the variant:

```rust
/// A STIX object: a recognized typed object, a generic value bag, or a
/// consumer-registered custom object.
#[derive(Debug, Clone)]
pub enum StixObject {
    Typed(TypedObject),
    Generic(GenericObject),
    Custom(Arc<dyn CustomObject>),
}
```

(c) Add `downcast_ref` to the existing `impl StixObject` block (right after `from_json`):

```rust
    /// If this is a registered custom object of concrete type `T`, borrow it.
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        match self {
            StixObject::Custom(c) => c.as_any().downcast_ref::<T>(),
            _ => None,
        }
    }
```

(d) Add a hand-written `PartialEq` (because `Arc<dyn CustomObject>` is not `PartialEq`). Place it right after the `impl StixObject { .. }` block:

```rust
impl PartialEq for StixObject {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (StixObject::Typed(a), StixObject::Typed(b)) => a == b,
            (StixObject::Generic(a), StixObject::Generic(b)) => a == b,
            (StixObject::Custom(a), StixObject::Custom(b)) => a.as_json() == b.as_json(),
            _ => false,
        }
    }
}
```

(e) Add the `Custom` arm to each of the three `impl ObjectView for StixObject` methods:

```rust
    fn id(&self) -> Option<&str> {
        match self {
            StixObject::Typed(t) => t.id(),
            StixObject::Generic(g) => g.id(),
            StixObject::Custom(c) => c.id(),
        }
    }

    fn type_(&self) -> Option<&str> {
        match self {
            StixObject::Typed(t) => t.type_(),
            StixObject::Generic(g) => g.type_(),
            StixObject::Custom(c) => c.type_(),
        }
    }

    fn property(&self, name: &str) -> Option<StixValue> {
        match self {
            StixObject::Typed(t) => t.property(name),
            StixObject::Generic(g) => g.property(name),
            StixObject::Custom(c) => c.property(name),
        }
    }
```

(f) Add the `Custom` arm to `impl Serialize for StixObject`:

```rust
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            StixObject::Typed(TypedObject::ObservedData(od)) => od.serialize(serializer),
            StixObject::Generic(g) => g.properties().serialize(serializer),
            StixObject::Custom(c) => c.as_json().serialize(serializer),
        }
    }
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p stix-model object`
Expected: PASS — all `object` tests pass (existing + new `custom_*`).

- [ ] **Step 5: Commit**

```bash
git add crates/stix-model/src/object.rs
git commit -m "feat(model): add StixObject::Custom variant with downcast and equality"
```

---

## Task 3: ModelRegistry with data-level handler + parsing

**Files:**
- Create: `crates/stix-model/src/registry.rs`
- Modify: `crates/stix-model/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/stix-model/src/registry.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ModelError;
    use crate::object::{StixObject, TypedObject};
    use crate::view::ObjectView;

    #[test]
    fn handler_validates_and_rejects() {
        let mut reg = ModelRegistry::new();
        reg.register_handler("x-thing", |obj| {
            if obj.get("risk_score").is_none() {
                return Err(ModelError::InvalidObject("missing risk_score".into()));
            }
            Ok(obj)
        });
        let ok = reg.parse_object(serde_json::json!({"type":"x-thing","id":"x--1","risk_score":1}));
        assert!(ok.is_ok());
        let bad = reg.parse_object(serde_json::json!({"type":"x-thing","id":"x--1"}));
        assert!(matches!(bad, Err(ModelError::InvalidObject(_))));
    }

    #[test]
    fn handler_adds_computed_property() {
        let mut reg = ModelRegistry::new();
        reg.register_handler("x-thing", |mut obj| {
            let score = obj.get("risk_score").and_then(|v| v.as_i64()).unwrap_or(0);
            obj["risk_band"] = serde_json::json!(if score > 80 { "high" } else { "low" });
            Ok(obj)
        });
        let parsed = reg
            .parse_object(serde_json::json!({"type":"x-thing","id":"x--1","risk_score":90}))
            .unwrap();
        // The enriched object is stored as data (a Generic object).
        assert!(matches!(parsed, StixObject::Generic(_)));
        assert_eq!(
            parsed.property("risk_band"),
            Some(crate::value::StixValue::String("high".into()))
        );
    }

    #[test]
    fn unregistered_types_use_builtin_dispatch() {
        let reg = ModelRegistry::new();
        // Built-in observed-data dispatch still works.
        let od = reg
            .parse_object(serde_json::json!({
                "type":"observed-data","id":"observed-data--1",
                "first_observed":"2020-01-01T00:00:00Z","last_observed":"2020-01-01T00:00:00Z",
                "number_observed":1,"object_refs":[]
            }))
            .unwrap();
        assert!(matches!(od, StixObject::Typed(TypedObject::ObservedData(_))));
        // Unknown types fall back to Generic.
        let g = reg
            .parse_object(serde_json::json!({"type":"ipv4-addr","id":"ipv4-addr--1","value":"1.2.3.4"}))
            .unwrap();
        assert!(matches!(g, StixObject::Generic(_)));
    }

    #[test]
    fn parse_bundle_routes_objects_through_handlers() {
        let mut reg = ModelRegistry::new();
        reg.register_handler("x-thing", |mut obj| {
            obj["seen"] = serde_json::json!(true);
            Ok(obj)
        });
        let bundle = reg
            .parse_bundle(
                r#"{"type":"bundle","id":"bundle--1","objects":[
                    {"type":"x-thing","id":"x--1"},
                    {"type":"ipv4-addr","id":"ipv4-addr--1","value":"1.2.3.4"}
                ]}"#,
            )
            .unwrap();
        assert_eq!(bundle.objects.len(), 2);
        assert_eq!(
            bundle.objects[0].property("seen"),
            Some(crate::value::StixValue::Bool(true))
        );
    }

    #[test]
    fn parse_bundle_rejects_non_bundle() {
        let reg = ModelRegistry::new();
        let err = reg.parse_bundle(r#"{"type":"ipv4-addr","id":"x--1"}"#).unwrap_err();
        assert!(matches!(err, ModelError::NotABundle(_)));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-model registry`
Expected: FAIL — `ModelRegistry` not found.

- [ ] **Step 3: Implement the registry (data-level handler form)**

At the top of `crates/stix-model/src/registry.rs` (above the test module):

```rust
//! `ModelRegistry`: register consumer-supplied handling for STIX object types.

use std::collections::HashMap;

use serde_json::Value;

use crate::bundle::Bundle;
use crate::error::{ModelError, Result};
use crate::object::StixObject;
use crate::view::GenericObject;

/// A handler that turns a raw JSON object of a registered type into a `StixObject`.
type TypeHandler = Box<dyn Fn(Value) -> Result<StixObject> + Send + Sync>;

/// Maps a STIX `type` to a consumer-supplied handler. Registered handlers take
/// precedence over the built-in dispatch in [`StixObject::from_json`].
///
/// Two registration forms:
/// - [`register_handler`](ModelRegistry::register_handler): a data-level
///   `Value -> Result<Value>` validate/normalize hook (the form bindings bridge a
///   host callable onto). The result is stored as a generic object.
/// - [`register`](ModelRegistry::register): a typed Rust convenience that stores a
///   `StixObject::Custom`.
#[derive(Default)]
pub struct ModelRegistry {
    handlers: HashMap<String, TypeHandler>,
}

impl ModelRegistry {
    /// An empty registry (parsing behaves like the built-in dispatch).
    pub fn new() -> Self {
        ModelRegistry::default()
    }

    /// Register a data-level validate/normalize hook for `type_name`. The hook may
    /// reject the object (return `Err`) or return an enriched object (e.g. with a
    /// computed property). The result is stored as a [`StixObject::Generic`].
    pub fn register_handler<F>(&mut self, type_name: impl Into<String>, hook: F)
    where
        F: Fn(Value) -> Result<Value> + Send + Sync + 'static,
    {
        self.handlers.insert(
            type_name.into(),
            Box::new(move |value| {
                let normalized = hook(value)?;
                Ok(StixObject::Generic(GenericObject::from_json(normalized)?))
            }),
        );
    }

    /// Parse a single JSON object, dispatching on its `type`: a registered handler
    /// wins; otherwise the built-in dispatch applies.
    pub fn parse_object(&self, value: Value) -> Result<StixObject> {
        let type_ = value
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| ModelError::InvalidObject("missing 'type' property".to_string()))?
            .to_string();

        match self.handlers.get(&type_) {
            Some(handler) => handler(value),
            None => StixObject::from_json(value),
        }
    }

    /// Parse a bundle, routing every object through [`parse_object`](Self::parse_object).
    pub fn parse_bundle(&self, json: &str) -> Result<Bundle> {
        let value: Value = serde_json::from_str(json)?;
        let type_ = value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if type_ != "bundle" {
            return Err(ModelError::NotABundle(format!("type was '{type_}'")));
        }
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .map(|s| s.to_string());
        let objects = match value.get("objects") {
            Some(Value::Array(arr)) => arr
                .iter()
                .map(|o| self.parse_object(o.clone()))
                .collect::<Result<Vec<_>>>()?,
            _ => Vec::new(),
        };
        Ok(Bundle {
            type_,
            id,
            objects,
        })
    }
}
```

In `crates/stix-model/src/lib.rs`, add the module and re-export (keep modules alphabetical):

```rust
pub mod registry;
```

and to the re-export block:

```rust
pub use registry::ModelRegistry;
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p stix-model registry`
Expected: PASS — all five registry tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-model/src/registry.rs crates/stix-model/src/lib.rs
git commit -m "feat(model): add ModelRegistry with data-level handler and parsing"
```

---

## Task 4: Typed `register::<T>` convenience + module doctest

**Files:**
- Modify: `crates/stix-model/src/registry.rs`

- [ ] **Step 1: Write the failing test**

Add these tests inside the existing `mod tests` in `crates/stix-model/src/registry.rs`:

```rust
    use crate::view::CustomObject;
    use crate::value::StixValue;

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    struct Widget {
        #[serde(rename = "type")]
        type_: String,
        id: String,
        risk: i64,
    }

    impl ObjectView for Widget {
        fn id(&self) -> Option<&str> {
            Some(&self.id)
        }
        fn type_(&self) -> Option<&str> {
            Some(&self.type_)
        }
        fn property(&self, name: &str) -> Option<StixValue> {
            match name {
                "risk" => Some(StixValue::Integer(self.risk)),
                _ => None,
            }
        }
    }

    #[test]
    fn register_typed_yields_custom_and_downcasts() {
        let mut reg = ModelRegistry::new();
        reg.register::<Widget>("x-widget");
        let parsed = reg
            .parse_object(serde_json::json!({"type":"x-widget","id":"x-widget--1","risk":90}))
            .unwrap();
        assert!(matches!(parsed, StixObject::Custom(_)));
        assert_eq!(parsed.property("risk"), Some(StixValue::Integer(90)));
        let w = parsed.downcast_ref::<Widget>().expect("downcast");
        assert_eq!(w.risk, 90);
    }

    #[test]
    fn registered_handler_overrides_builtin() {
        // A consumer may even override a core type's dispatch.
        let mut reg = ModelRegistry::new();
        reg.register::<Widget>("observed-data");
        let parsed = reg
            .parse_object(serde_json::json!({"type":"observed-data","id":"x--1","risk":7}))
            .unwrap();
        assert!(matches!(parsed, StixObject::Custom(_)));
    }

    // Ensure `CustomObject` is reachable for the blanket impl bound.
    fn _assert_widget_is_custom_object(_: &dyn CustomObject) {}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-model registry`
Expected: FAIL — `register` method not found.

- [ ] **Step 3: Implement `register::<T>` and add the module doctest**

In `crates/stix-model/src/registry.rs`, add `use serde::de::DeserializeOwned;` and `use std::sync::Arc;` to the imports, and add this method inside `impl ModelRegistry` (after `register_handler`):

```rust
    /// Register a typed Rust struct for `type_name`. Objects of that type
    /// deserialize into `T` and are stored as [`StixObject::Custom`], retrievable
    /// with [`StixObject::downcast_ref`]. `T` only needs to implement
    /// [`ObjectView`](crate::view::ObjectView) (plus `Serialize`/`Deserialize`).
    pub fn register<T>(&mut self, type_name: impl Into<String>)
    where
        T: DeserializeOwned + crate::view::CustomObject + 'static,
    {
        self.handlers.insert(
            type_name.into(),
            Box::new(|value| {
                let obj: T = serde_json::from_value(value)?;
                Ok(StixObject::Custom(Arc::new(obj)))
            }),
        );
    }
```

Then add a crate-facing doctest as the module's doc comment — replace the first line
`//! \`ModelRegistry\`: register consumer-supplied handling for STIX object types.`
with:

```rust
//! `ModelRegistry`: register consumer-supplied handling for STIX object types.
//!
//! # Example: validate and add a computed property
//!
//! ```
//! use stix_model::ModelRegistry;
//!
//! let mut registry = ModelRegistry::new();
//! registry.register_handler("x-acme-widget", |mut obj| {
//!     let score = obj.get("risk_score").and_then(|v| v.as_i64()).unwrap_or(0);
//!     obj["risk_band"] = serde_json::json!(if score > 80 { "high" } else { "low" });
//!     Ok(obj)
//! });
//!
//! let bundle = registry
//!     .parse_bundle(r#"{"type":"bundle","objects":[
//!         {"type":"x-acme-widget","id":"x-acme-widget--1","risk_score":90}
//!     ]}"#)
//!     .unwrap();
//!
//! use stix_model::ObjectView;
//! assert_eq!(
//!     bundle.objects[0].property("risk_band"),
//!     Some(stix_model::StixValue::String("high".into()))
//! );
//! ```
```

- [ ] **Step 4: Run the tests + doctest to verify they pass**

Run: `cargo test -p stix-model`
Expected: PASS — registry unit tests and the module doctest pass.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-model/src/registry.rs
git commit -m "feat(model): add typed register::<T> convenience and module doctest"
```

---

## Task 5: End-to-end matching integration test

**Files:**
- Create: `crates/stix/tests/custom_models.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/stix/tests/custom_models.rs`:

```rust
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
```

- [ ] **Step 2: Run the test to verify it fails (or compile-fails for a missing dev-dep)**

Run: `cargo test -p stix --test custom_models`
Expected: FAIL to compile — `serde` is not yet a dev-dependency of the `stix` crate (added in the next step). If `serde` resolves, the tests should pass.

- [ ] **Step 3: Add `serde` to the `stix` crate's dev-dependencies**

In `crates/stix/Cargo.toml`, update `[dev-dependencies]`:

```toml
[dev-dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p stix --test custom_models`
Expected: PASS — all three end-to-end tests pass (typed match, computed-property match, downcast).

- [ ] **Step 5: Commit**

```bash
git add crates/stix/tests/custom_models.rs crates/stix/Cargo.toml
git commit -m "test(stix): end-to-end custom-model matching integration tests"
```

---

## Task 6: Runnable example

**Files:**
- Create: `crates/stix/examples/custom_model.rs`

- [ ] **Step 1: Write the example**

Create `crates/stix/examples/custom_model.rs`:

```rust
//! Run with: `cargo run -p stix --example custom_model`
//!
//! Demonstrates registering a consumer-defined custom STIX object type, getting
//! typed access to it after parsing, and matching a pattern against both a stored
//! and a computed property.

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
            println!("typed access -> widget {} (risk_score={})", w.id, w.risk_score);
        }
    }

    // Match against a computed property the matcher resolves through ObjectView.
    let pattern = parse("[x-acme-widget:risk_band = 'high']").expect("parse pattern");
    let result = match_bundle(&pattern, &bundle).expect("match");
    println!("pattern [x-acme-widget:risk_band = 'high'] matched: {}", result.is_match());
    assert!(result.is_match());
}
```

- [ ] **Step 2: Build the example to verify it compiles**

Run: `cargo build -p stix --example custom_model`
Expected: compiles with no errors.

- [ ] **Step 3: Run the example to verify output**

Run: `cargo run -p stix --example custom_model`
Expected output (order of the typed-access line then the match line):

```
typed access -> widget x-acme-widget--1 (risk_score=90)
pattern [x-acme-widget:risk_band = 'high'] matched: true
```

- [ ] **Step 4: Commit**

```bash
git add crates/stix/examples/custom_model.rs
git commit -m "docs(stix): add runnable custom_model example"
```

---

## Task 7: README "Custom object types" section

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add the section**

In `README.md`, add a new section immediately **after** the "Architecture & design"
section (before "Development"):

```markdown
## Custom object types

Custom and unknown STIX types already parse and match out of the box (they become a
generic value-backed object). When you want **typed access**, **validation**, or
**computed properties**, register them with a `ModelRegistry`.

### Rust — typed structs

```rust
use serde::{Deserialize, Serialize};
use stix::model::{ModelRegistry, ObjectView, StixValue};

#[derive(Debug, Serialize, Deserialize)]
struct AcmeWidget { #[serde(rename = "type")] type_: String, id: String, risk_score: i64 }

impl ObjectView for AcmeWidget {
    fn id(&self) -> Option<&str> { Some(&self.id) }
    fn type_(&self) -> Option<&str> { Some(&self.type_) }
    fn property(&self, name: &str) -> Option<StixValue> {
        match name {
            "risk_score" => Some(StixValue::Integer(self.risk_score)),
            // computed property, resolved on demand
            "risk_band" => Some(StixValue::String(
                if self.risk_score > 80 { "high" } else { "low" }.into())),
            _ => None,
        }
    }
}

let mut registry = ModelRegistry::new();
registry.register::<AcmeWidget>("x-acme-widget");
let bundle = registry.parse_bundle(json).unwrap();

// Typed access after parsing:
if let Some(w) = bundle.objects[0].downcast_ref::<AcmeWidget>() { /* w.risk_score */ }
```

A runnable version lives in [`crates/stix/examples/custom_model.rs`](crates/stix/examples/custom_model.rs)
— `cargo run -p stix --example custom_model`.

### Rust — data-level validate/normalize hook

For validation or computed properties without a struct, register a
`Value -> Result<Value>` hook. It runs once per object at import; the result is
stored as data, so matching stays callback-free:

```rust
registry.register_handler("x-acme-widget", |mut obj| {
    if obj.get("risk_score").is_none() {
        return Err(stix::model::ModelError::InvalidObject("missing risk_score".into()));
    }
    let score = obj["risk_score"].as_i64().unwrap_or(0);
    obj["risk_band"] = serde_json::json!(if score > 80 { "high" } else { "low" });
    Ok(obj)
});
```

### TypeScript / Python (planned bindings)

You won't define a Rust struct from a binding. Typed access is native to the host
language (define a TS `interface`/Python class over the parsed object), and custom
types match with no registration at all. For validation or computed properties you
register the same import-time hook — a host function the core invokes once per
object at parse time:

```ts
stix.registerType("x-acme-widget", {
  normalize(obj) {
    if (!obj.risk_score) throw new Error("missing risk_score");
    return { ...obj, risk_band: obj.risk_score > 80 ? "high" : "low" };
  },
});
```
```

- [ ] **Step 2: Verify the doc renders (sanity check the markdown)**

Run: `grep -n "## Custom object types" README.md`
Expected: prints the new heading's line number (section present).

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: document custom object types (Rust + TS/Python sketch)"
```

---

## Task 8: Lint, format, and final verification

**Files:** none (verification only)

- [ ] **Step 1: Format**

Run: `cargo fmt --all`
Then review: `git diff`.

- [ ] **Step 2: Clippy across the workspace (warnings as errors)**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: no warnings. Fix any inline. (Watch for `clippy::type_complexity` on
`TypeHandler` — it is already aliased to a named type, which satisfies the lint.)

- [ ] **Step 3: Full workspace test run, including the example build**

Run: `cargo test` then `cargo build --examples`
Expected: all suites PASS (`stix-model` now includes `registry` tests + doctest;
`stix` includes the `custom_models` integration test); the example builds.

- [ ] **Step 4: Commit any fmt/clippy fixes**

```bash
git add -A
git commit -m "chore(model): fmt + clippy clean for custom models"
```

---

## Self-Review Notes (already applied)

- **Spec coverage:** `CustomObject` trait + blanket impl (Task 1); `StixObject::Custom`
  with `Clone`/manual `PartialEq`/`Serialize`/`ObjectView`/`downcast_ref` (Task 2);
  `ModelRegistry` data-level `register_handler` primitive + `parse_object`/`parse_bundle`
  with handler precedence (Task 3); typed `register::<T>` sugar (Task 4); end-to-end
  matching incl. computed property (Task 5); runnable example (Task 6); README with
  Rust + TS/Python documentation (Task 7). All spec sections map to a task. Matcher is
  untouched, as the spec requires.
- **Non-breaking:** the registry-free `from_json`/`Deserialize` path is unchanged and
  never produces `Custom`; only the `PartialEq` derive→manual change is internal.
- **Type consistency:** `CustomObject { as_json, as_any }`, `StixObject::Custom(Arc<dyn CustomObject>)`,
  `downcast_ref::<T>`, `ModelRegistry::{new, register, register_handler, parse_object, parse_bundle}`,
  `TypeHandler = Box<dyn Fn(Value) -> Result<StixObject> + Send + Sync>`, and `Bundle { type_, id, objects }`
  are used consistently across tasks and tests, and match the verified current code.
- **No placeholders:** every code step is complete and compilable.
```
