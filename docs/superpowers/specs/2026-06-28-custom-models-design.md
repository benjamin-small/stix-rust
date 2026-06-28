# Consumer-Injectable Custom Models — Design

**Date:** 2026-06-28
**Status:** Approved (brainstorming complete; pending spec review)
**Scope:** Sub-project A of the "custom models" feature. Matcher operator hooks
(sub-project B) are a separate later spec.

## Purpose

Let a consumer of the library extend the object model without forking it:

1. **Typed access** — register a consumer's own Rust struct for a STIX `type`, so
   parsed bundles yield that struct with typed field access, while still matching.
2. **Custom property / path semantics** — let a consumer control how an object's
   properties are produced (validation, normalization, computed/derived fields)
   that the matcher then sees.
3. **Validation hooks** — let a consumer reject objects of a given type at import.

A documented, runnable example is part of the deliverable.

### Already true today (no work needed)

`GenericObject` preserves *all* properties and implements `ObjectView`, and the
matcher resolves object-paths dynamically. So **custom/unknown object types already
parse and match correctly** — they land in `StixObject::Generic`. This feature adds
what `Generic` does not: typed ergonomics, import-time validation, and
computed/derived properties.

## Cross-language design constraint (drives the core API)

Bindings (Python via PyO3, TypeScript via napi/wasm-bindgen) are a later phase, but
the core API is shaped now so they are a thin wrapper, not a redesign:

- **Typed access is Rust-only sugar.** TS/Python represent STIX objects as native
  objects/dicts; consumers there use their own interfaces/types. They do not need a
  Rust struct.
- **The extension primitive is data-level:** a single import-time hook
  `Value -> Result<Value>` that validates and/or returns an enriched object.
  Computed properties become real stored fields, so the matcher reads them as plain
  data with **no per-match callback** (no FFI on the hot path). This is the only
  surface a binding must bridge (host callable → Rust closure), and PyO3 / napi /
  wasm-bindgen can all hold and synchronously invoke a host callable.

Therefore the registry's primitive is a data-level handler, and the typed
`register::<T>()` form is sugar layered on top.

## Architecture

A new module `registry.rs` in `stix-model`, a new `CustomObject` trait in
`view.rs`, and a new `StixObject::Custom` variant in `object.rs`. No changes to
`stix-pattern` or `stix-matcher`.

```
stix-model/src/
├── view.rs       # + CustomObject trait (+ blanket impl)
├── object.rs     # + StixObject::Custom(Arc<dyn CustomObject>) + downcast_ref
└── registry.rs   # ModelRegistry: registration + parse_object / parse_bundle
```

## Components

### `CustomObject` trait (`view.rs`)

```rust
pub trait CustomObject: ObjectView + std::fmt::Debug + Send + Sync {
    /// Canonical JSON for serialization and equality.
    fn as_json(&self) -> serde_json::Value;
    /// For downcasting back to the concrete consumer type.
    fn as_any(&self) -> &dyn std::any::Any;
}
```

A **blanket impl** covers any type that already implements `ObjectView` and is
serializable, so consumers implement only `ObjectView`:

```rust
impl<T> CustomObject for T
where
    T: ObjectView + serde::Serialize + std::fmt::Debug + Send + Sync + 'static,
{
    fn as_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
    fn as_any(&self) -> &dyn std::any::Any { self }
}
```

### `StixObject::Custom` (`object.rs`)

```rust
pub enum StixObject {
    Typed(TypedObject),
    Generic(GenericObject),
    Custom(std::sync::Arc<dyn CustomObject>),
}
```

- **`Clone`**: derivable — `Arc<dyn _>` is `Clone`.
- **`PartialEq`**: hand-written (a `dyn` trait object is not `PartialEq`). Compare
  `Custom` vs `Custom` by `as_json()` equality; `Typed`/`Generic` keep structural
  equality; mixed variants are unequal.
- **`Serialize`**: `Custom` serializes via `as_json()`.
- **`ObjectView`**: `Custom` arm delegates to the inner trait object.
- **`Deserialize`** (the registry-free path) never produces `Custom` — default
  behavior is unchanged, so this is **non-breaking**.
- New helper: `StixObject::downcast_ref::<T: 'static>(&self) -> Option<&T>` returns
  the concrete consumer type from a `Custom` (via `as_any().downcast_ref`).

### `ModelRegistry` (`registry.rs`)

```rust
type TypeHandler =
    Box<dyn Fn(serde_json::Value) -> Result<StixObject, ModelError> + Send + Sync>;

pub struct ModelRegistry {
    handlers: std::collections::HashMap<String, TypeHandler>,
}
```

Registration forms (both compile to a `TypeHandler`):

- **Data-level primitive (binding-friendly):**
  `register_handler(type_name, f)` where `f: Fn(Value) -> Result<Value, ModelError>`.
  The returned value is validated/normalized; it is stored as
  `StixObject::Generic`. Used for validation and computed/derived properties, and is
  the exact surface a binding bridges a host callable onto.
- **Typed convenience (Rust-only sugar):**
  `register::<T>(type_name)` where
  `T: serde::de::DeserializeOwned + ObjectView + Serialize + Debug + Send + Sync + 'static`.
  Deserializes into `T` and stores `StixObject::Custom(Arc::new(t))`.

Parsing:

- `parse_object(&self, value) -> Result<StixObject>`: read the `type`; if a handler
  is registered, run it; otherwise fall back to `StixObject::from_json` (built-in
  `observed-data` dispatch, else `Generic`). **Registered handlers take precedence**
  over built-in dispatch, so a consumer may override even core types (documented as
  advanced use).
- `parse_bundle(&self, json: &str) -> Result<Bundle>`: parse the envelope, validate
  `type == "bundle"`, and run every object through `parse_object`. Fail-fast on the
  first handler/validation error.

`ModelRegistry` is `Send + Sync` (handlers are `Fn + Send + Sync`), so it can be
shared across threads and reused across many parses.

## Data Flow

```
JSON ──[ModelRegistry::parse_bundle]──► Bundle { objects: Vec<StixObject> }
            per object: type lookup ─► handler?  yes ─► Custom | enriched Generic
                                                  no  ─► built-in Typed | Generic
Bundle ──[stix-matcher::match_bundle]──► MatchResult   (unchanged; reads ObjectView)
```

The consumer parses with the registry, then calls the existing matcher entry points
unchanged. Custom objects participate via `ObjectView`.

## Error Handling

- Handler and validation failures surface as `ModelError` (reusing
  `InvalidObject(String)`, which carries the message). `parse_bundle` is fail-fast.
- No panics: the blanket `as_json()` falls back to `Value::Null` if serialization
  somehow fails rather than unwrapping in library code paths intended to be
  infallible (a custom `Serialize` that errors is the consumer's bug; we degrade
  rather than crash).

## Testing

- **Unit (`registry.rs`):** typed `register::<T>` → `parse_object` yields `Custom`,
  `downcast_ref` returns the struct, `property()` exposes its fields; data-level
  `register_handler` validation rejects a bad object; computed-property enrichment
  yields an enriched `Generic`; handler precedence over built-in dispatch; default
  (no registration) still yields `Generic`/`Typed`.
- **Unit (`object.rs`):** `Custom` round-trips through `as_json`/serialize; `Clone`
  and `PartialEq` behave; `downcast_ref` to the wrong type returns `None`.
- **Integration (`stix-model` or `stix`):** parse a bundle containing a custom type
  with the registry, then match a pattern against one of its (typed or computed)
  properties end-to-end through `stix-matcher`.
- **Example compiles/runs:** the runnable example is built in CI via
  `cargo build --examples`.

## Documentation (explicit deliverable)

Three forms, all kept in sync:

1. **Module doctest** in `registry.rs` — a minimal `register::<T>` + parse + match.
2. **Runnable example** `crates/stix/examples/custom_model.rs` — end-to-end:
   define `AcmeWidget`, implement `ObjectView` (showing a computed property), derive
   `Serialize`/`Deserialize`/`Debug`, register it, parse a bundle, downcast for
   typed access, and match a pattern against a custom property. Plus a second
   snippet using `register_handler` for validation + computed enrichment. Run with
   `cargo run -p stix --example custom_model`.
3. **README "Custom object types" section** — the Rust example, and a documented
   **TypeScript/Python sketch** of the import-hook model so the cross-language story
   is written down (per the binding constraint above), e.g.:

   ```ts
   stix.registerType("x-acme-widget", {
     normalize(obj) {
       if (!obj.risk_score) throw new Error("missing risk_score");
       return { ...obj, risk_band: obj.risk_score > 80 ? "high" : "low" };
     },
   });
   ```

## Out of Scope (Phase A)

- Matcher operator hooks / custom comparison operators (sub-project B).
- Actual Python/TypeScript bindings (later phase) — this spec only ensures the core
  API shape makes them a thin wrapper.
- Per-match host callbacks (deliberately avoided; computed properties are resolved
  at import time and stored as data).

## Future Considerations

- The `register_handler` primitive is the bridge point for the eventual bindings:
  a binding wraps a host callable into the `Fn(Value) -> Result<Value>` closure.
- Sub-project B (operator hooks) will add a parallel registry in `stix-matcher`;
  keeping this registry in `stix-model` (data/import concerns) and operators in
  `stix-matcher` (evaluation concerns) preserves the crate dependency boundaries.
