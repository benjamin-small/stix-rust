# Custom Object Types

Custom and unknown STIX types (`x-*` or anything else) **already parse and match
with zero registration** — they become generic value-backed objects that preserve
every property. Registration is for the three things the generic path can't give
you: **typed access**, **validation**, and **computed properties**.

## Rust: typed structs

Register your own struct for a type; parsed bundles then carry *your* type, and you
can downcast back to it:

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
            // a computed property — synthesized, not stored in the JSON:
            "risk_band" => Some(StixValue::String(
                if self.risk_score > 80 { "high" } else { "low" }.into())),
            _ => None,
        }
    }
}

let mut registry = ModelRegistry::new();
registry.register::<AcmeWidget>("x-acme-widget");
let bundle = registry.parse_bundle(json)?;

// typed access after parsing:
if let Some(w) = bundle.objects[0].downcast_ref::<AcmeWidget>() { /* w.risk_score */ }
```

Because `risk_band` is exposed through `ObjectView`, the pattern
`[x-acme-widget:risk_band = 'high']` **matches a property that never existed in the
JSON**. A runnable version lives in the repo:
`cargo run -p stix-rust --example custom_model`.

## Rust: data-level hooks

No struct needed — register a `Value → Result<Value>` hook for validation and
enrichment. It runs **once per object at parse time**; the result is stored as
data, so matching stays callback-free:

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

A hook rejection surfaces from `parse_bundle` as a validation error.

## The same idea in every binding

Each binding exposes the identical import-time hook — a host function taking and
returning the object; **throwing rejects it** (a `ValidationError` /
`ValidationException`):

```python
# Python
engine.register_type("x-acme-widget", lambda obj: {**obj,
    "risk_band": "high" if obj.get("risk_score", 0) > 80 else "low"})
```

```ts
// TypeScript (Node and wasm)
engine.registerType("x-acme-widget", (obj) => ({
  ...obj, risk_band: obj.risk_score > 80 ? "high" : "low" }));
```

```java
// Java
engine.registerType("x-acme-widget", obj -> {
    long score = ((Number) obj.getOrDefault("risk_score", 0)).longValue();
    obj.put("risk_band", score > 80 ? "high" : "low");
    return obj;
});
```

Hooks always run at **parse/import time, synchronously** — never during matching —
so there is no cross-language callback overhead on the hot path.
