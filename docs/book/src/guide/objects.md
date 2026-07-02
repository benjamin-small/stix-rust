# Objects & Bundles

`stix-model` imports STIX objects and bundles into a model that is **typed where it
helps and flexible everywhere else**.

## The trichotomy: typed, generic, custom

Every imported object is one of three shapes:

| Shape | When | What you get |
| --- | --- | --- |
| **Typed** | Recognized types (currently `observed-data`) | A real Rust struct (`ObservedData`) with typed fields |
| **Generic** | Every other type — including custom `x-*` types | A value-backed property map preserving *all* properties |
| **Custom** | Types you register yourself | *Your* struct (see [Custom Object Types](custom-types.md)) |

The crucial design point: all three implement the **`ObjectView`** trait —

```rust
pub trait ObjectView {
    fn id(&self) -> Option<&str>;
    fn type_(&self) -> Option<&str>;
    fn property(&self, name: &str) -> Option<StixValue>;
}
```

— and the matcher consumes *only* `ObjectView`. That's why unknown and custom types
match out of the box, and why typed objects can **synthesize** properties on demand
(a property need not literally exist in the JSON to be matchable).

## StixValue

`property()` returns a `StixValue` — a JSON-shaped dynamic value: null, bool,
integer, float, string, list, or object. Two things to know:

- **Integers and floats are distinct** (numeric comparisons promote, but the values
  are stored as parsed).
- **Timestamps, hex, and binary are carried as strings** at this layer; the matcher
  compares them against pattern literals as strings.

## Bundles and the object store

```rust
use stix::model::{Bundle, ObjectStore, ObjectView};

let bundle = Bundle::from_json_str(json)?;      // validates type == "bundle"
let store  = ObjectStore::from_bundle(&bundle); // id → object index

let obj = store.get("ipv4-addr--a1").unwrap();
assert_eq!(obj.property("value").unwrap().as_str(), Some("198.51.100.5"));
```

The `ObjectStore` is how reference properties resolve: when a pattern path walks
through `src_ref`, the matcher looks the id up in the store. **No store → reference
paths resolve to nothing** (not an error — an empty result).

## ObservedData, the typed SDO

`observed-data` is the one built-in typed struct because the matcher needs its
fields: `first_observed` / `last_observed` / `number_observed` (temporal metadata)
and `object_refs` (which SCOs were seen together). It tolerates STIX 2.0's inline
`objects` map as well, and retains unknown properties via a flattened map so nothing
is lost on round-trip.
