# stix-rust

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org)
[![Status: alpha](https://img.shields.io/badge/status-alpha-yellow.svg)](#project-status)

A reusable, pure-Rust toolkit for working with [STIX 2.1](https://oasis-open.github.io/cti-documentation/stix/intro.html) — the OASIS standard for representing cyber threat intelligence. `stix-rust` lets you:

- **Parse** STIX patterning-language patterns into a typed, inspectable AST.
- **Import** STIX objects (SDOs, SCOs, SROs) and bundles into a strongly-typed-yet-flexible object model.
- **Match** patterns against a set of observed objects *(in progress — see [Project status](#project-status))*.

Reusability is the guiding principle: the crates have clean, minimal dependency edges so you can pull in only what you need, and the design leaves clean seams for the planned **Python** and **TypeScript** bindings.

---

## Table of contents

- [What is STIX?](#what-is-stix)
- [Project status](#project-status)
- [Workspace layout](#workspace-layout)
- [Installation](#installation)
- [Quick start](#quick-start)
- [Supported pattern grammar](#supported-pattern-grammar)
- [Architecture & design](#architecture--design)
- [Development](#development)
- [Roadmap](#roadmap)
- [Contributing](#contributing)
- [License](#license)

---

## What is STIX?

**STIX (Structured Threat Information eXpression)** is a standardized JSON language for describing cyber threat intelligence — indicators, malware, threat actors, observed data, and the relationships between them. Two pieces matter most here:

- **STIX objects** — JSON documents like an `ipv4-addr`, a `file`, or an `observed-data` SDO, usually delivered together in a **bundle**.
- **The STIX patterning language** — a query-like syntax used inside `indicator` objects to describe *what to look for*, e.g.:

  ```
  [ipv4-addr:value = '198.51.100.1' OR domain-name:value = 'evil.example']
  ```

`stix-rust` parses those patterns and (soon) evaluates them against observed objects to answer: **"does this threat intelligence match what we saw?"**

---

## Project status

`stix-rust` is built in three phases. Phases 1 and 2 are complete and merged; phase 3 is next.

| Crate          | Purpose                                              | Status        |
| -------------- | --------------------------------------------------- | ------------- |
| `stix-pattern` | Lexer + parser for the patterning language → AST    | ✅ Available  |
| `stix-model`   | Object model: values, objects, bundles, object store | ✅ Available  |
| `stix-matcher` | Match a pattern AST against observed objects        | ✅ Available  |
| `stix`         | Umbrella crate re-exporting everything + entry points | ✅ Available  |

> **Alpha:** APIs may change before a `0.1` release. Crates are not yet published to [crates.io](https://crates.io); use a git or path dependency for now.

---

## Workspace layout

```
stix-rust/
├── crates/
│   ├── stix-pattern/   # lexer + recursive-descent parser → pattern AST
│   ├── stix-model/     # StixValue, ObjectView, typed/generic objects, Bundle, ObjectStore
│   ├── stix-matcher/   # (planned) matching engine
│   └── stix/           # (planned) umbrella crate + high-level entry points
├── docs/superpowers/   # design spec & implementation plans
├── LICENSE-MIT
└── LICENSE-APACHE
```

**Dependency edges** are deliberately minimal so each crate is independently usable:

- `stix-pattern` → `serde` only
- `stix-model` → `serde` / `serde_json` only (notably **not** `stix-pattern`)
- `stix-matcher` → `stix-pattern` + `stix-model` *(planned)*

---

## Language interfaces

The Rust crates are the core. Language bindings (in progress) live under
[`bindings/`](bindings/) — each has self-contained docs you can link to directly:

| Interface | Toolchain | Docs | Status |
| --- | --- | --- | --- |
| Python | PyO3 + maturin | [`bindings/python`](bindings/python/README.md) | 🚧 planned |
| Java | jni-rs | [`bindings/java`](bindings/java/README.md) | 🚧 planned |
| TypeScript (Node) | napi-rs | [`bindings/typescript-node`](bindings/typescript-node/README.md) | 🚧 planned |
| TypeScript (wasm) | wasm-bindgen | [`bindings/typescript-wasm`](bindings/typescript-wasm/README.md) | 🚧 planned |

Contributor and agent conventions are documented in [`AGENTS.md`](AGENTS.md).

## Installation

Until the crates are published, depend on them by git. The `stix` umbrella crate
re-exports everything, so it's usually all you need:

```toml
[dependencies]
stix = { git = "https://github.com/benjamin-small/stix-rust" }
```

Or pull in individual crates if you only need part of the toolkit:

```toml
[dependencies]
stix-pattern = { git = "https://github.com/benjamin-small/stix-rust" }
stix-model   = { git = "https://github.com/benjamin-small/stix-rust" }
stix-matcher = { git = "https://github.com/benjamin-small/stix-rust" }
```

Or, if you've cloned the repo, by path:

```toml
[dependencies]
stix-pattern = { path = "../stix-rust/crates/stix-pattern" }
stix-model   = { path = "../stix-rust/crates/stix-model" }
```

---

## Quick start

### Parse a STIX pattern

```rust
use stix_pattern::parse;

fn main() {
    let pattern = parse("[ipv4-addr:value = '198.51.100.1']").unwrap();

    // The result is a typed AST you can walk or serialize.
    let json = serde_json::to_string_pretty(&pattern).unwrap();
    println!("{json}");
}
```

The parser handles the **full** patterning grammar — comparison operators, boolean
logic, observation operators (`AND` / `OR` / `FOLLOWEDBY`), and the
`WITHIN` / `REPEATS` / `START..STOP` qualifiers — and reports precise errors with
byte spans:

```rust
use stix_pattern::parse;

let err = parse("[ipv4-addr:value = ]").unwrap_err();
println!("{err}"); // parse error at bytes 19..20: expected a literal value
```

### Import STIX objects and resolve references

```rust
use stix_model::{Bundle, ObjectStore, ObjectView, StixObject, TypedObject};

let raw = r#"{
  "type": "bundle",
  "id": "bundle--a1",
  "objects": [
    { "type": "ipv4-addr", "id": "ipv4-addr--a1", "value": "198.51.100.5" },
    { "type": "observed-data", "id": "observed-data--a1",
      "first_observed": "2020-03-01T12:00:00Z",
      "last_observed":  "2020-03-01T12:10:00Z",
      "number_observed": 5,
      "object_refs": ["ipv4-addr--a1"] }
  ]
}"#;

// Parse the bundle and index it by id.
let bundle = Bundle::from_json_str(raw).unwrap();
let store = ObjectStore::from_bundle(&bundle);

// Recognized types deserialize into typed structs; everything else stays generic.
for obj in &bundle.objects {
    if let StixObject::Typed(TypedObject::ObservedData(od)) = obj {
        println!("observed {} object(s) at {}", od.number_observed, od.first_observed);

        // Resolve the referenced cyber-observable through the store.
        for id in od.sco_ids() {
            let sco = store.get(id).unwrap();
            // `ObjectView` gives uniform access to any object, typed or generic.
            println!("  {} = {:?}", sco.type_().unwrap(), sco.property("value"));
        }
    }
}
```

### Match a pattern against observed objects

The matcher accepts several entry points (observations, `observed-data` SDOs, a
whole bundle, or a flat SCO list), all reducing to the same engine:

```rust
use stix::{parse, matcher::match_bundle};
use stix::model::Bundle;

let bundle = Bundle::from_json_str(raw).unwrap();
let pattern = parse("[ipv4-addr:value = '198.51.100.5']").unwrap();

let result = match_bundle(&pattern, &bundle).unwrap();
assert!(result.is_match());
```

Supported today: object-path resolution (including `_ref`/`_refs` dereferencing
through the bundle), every comparison operator, boolean logic with correct
"same object" binding within an observation, and observation-level `AND`/`OR`.
`FOLLOWEDBY` and the temporal qualifiers parse but return
`MatchError::Unsupported` rather than silently passing.

---

## Supported pattern grammar

`stix-pattern` parses the complete STIX 2.1 patterning grammar:

| Construct              | Examples                                                                 |
| ---------------------- | ----------------------------------------------------------------------- |
| Comparison operators   | `=` `!=` `<` `<=` `>` `>=` `IN` `LIKE` `MATCHES` `ISSUBSET` `ISSUPERSET` `EXISTS` |
| Negation               | `file:name NOT = 'x'`                                                    |
| Boolean (comparison)   | `[a = 1 AND b = 2]`, `[a = 1 OR b = 2]`, grouping with `( )`             |
| Object paths           | `file:hashes.'SHA-256'`, `network-traffic:protocols[0]`, `x:list[*]`     |
| Typed literals         | `t'2020-01-01T00:00:00Z'` (timestamp), `b'aGk='` (binary), `h'cafe'` (hex) |
| Observation operators  | `[a] AND [b]`, `[a] OR [b]`, `[a] FOLLOWEDBY [b]`                        |
| Qualifiers             | `WITHIN 60 SECONDS`, `REPEATS 5 TIMES`, `START t'…' STOP t'…'`           |

> **Note on matching scope:** the parser understands the *entire* grammar today.
> The matcher implements single-observation comparison + boolean matching and
> observation-level `AND`/`OR`; `FOLLOWEDBY` sequencing and temporal qualifiers are
> parsed but return an explicit `MatchError::Unsupported` result rather than
> silently passing, until their matching semantics land.

---

## Architecture & design

A few principles shape the codebase:

- **Hybrid object model.** Common types (currently `observed-data`) deserialize into
  typed Rust structs; any other or custom type falls back to a generic value bag.
  Both implement a single `ObjectView` trait, so consumers and the matcher get one
  uniform accessor (`id()`, `type_()`, `property(name)`) regardless of the backing
  representation. Custom and unknown properties are always preserved.
- **Version seam.** A `SpecVersion` enum and internal trait boundaries leave room to
  add STIX 2.0 (and future versions) without churning the public API. The library
  targets 2.1 today.
- **No silent failure.** Invalid patterns fail at parse with a precise span; model
  errors are distinct from "no match"; unimplemented matching features report
  themselves explicitly.
- **Bindings-ready.** Pure Rust, no parser-generator or C dependencies, and a
  `serde`-serializable AST — so Python (PyO3) and TypeScript (wasm/napi) bindings
  can be added cleanly in a later phase.

The full design rationale lives in
[`docs/superpowers/specs/`](docs/superpowers/specs/), and the task-by-task
implementation plans in [`docs/superpowers/plans/`](docs/superpowers/plans/).

---

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

---

## Development

Requires a recent stable Rust toolchain (developed against Rust 1.95).

```bash
# Build everything
cargo build

# Run the full test suite (unit + integration + doc tests)
cargo test

# Test a single crate
cargo test -p stix-pattern
cargo test -p stix-model

# Lint (the project keeps clippy clean with warnings denied)
cargo clippy --all-targets -- -D warnings

# Format
cargo fmt --all
```

The crates are developed test-first; `stix-pattern` additionally ships a
conformance corpus of valid/invalid patterns under
`crates/stix-pattern/tests/fixtures/`.

---

## Roadmap

- [x] **`stix-pattern`** — lexer, full-grammar parser, serde AST, conformance corpus
- [x] **`stix-model`** — `StixValue`, `ObjectView`, typed/generic objects, `Bundle`, `ObjectStore`, `SpecVersion`
- [x] **`stix-matcher`** — object-path resolution, all comparison operators, boolean logic, multiple entry points (observations / observed-data / bundle / SCO list)
- [x] **`stix`** — umbrella crate with high-level entry points
- [ ] `FOLLOWEDBY` sequencing + temporal-qualifier matching
- [ ] STIX 2.0 support via the version seam
- [x] Consumer-injectable custom typed models (registry)
- [ ] Python (PyO3) and TypeScript (wasm/napi) bindings

---

## Contributing

Issues and pull requests are welcome. This is an early-stage project; if you're
planning a larger change, opening an issue first to discuss direction is
appreciated. Please keep `cargo test` green and `cargo clippy --all-targets -- -D
warnings` clean.

---

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
