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
| `stix-matcher` | Match a pattern AST against observed objects        | 🚧 In progress |
| `stix`         | Umbrella crate re-exporting everything + entry points | 🚧 Planned    |

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

## Installation

Until the crates are published, depend on them by git:

```toml
[dependencies]
stix-pattern = { git = "https://github.com/benjamin-small/stix-rust" }
stix-model   = { git = "https://github.com/benjamin-small/stix-rust" }
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

### Matching *(coming with `stix-matcher`)*

The matcher will accept several entry points, all reducing to the same engine:

```rust,ignore
// Planned API — not yet available.
use stix::{parse, match_bundle};

let pattern = parse("[ipv4-addr:value = '198.51.100.5']")?;
let result  = match_bundle(&pattern, &bundle)?;
assert!(result.matched());
```

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
> The forthcoming matcher implements single-observation comparison + boolean
> matching first; `FOLLOWEDBY` sequencing and temporal qualifiers are parsed and
> will return an explicit "unsupported" result rather than silently passing, until
> their matching semantics land.

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
- [ ] **`stix-matcher`** — object-path resolution, all comparison operators, boolean logic, multiple entry points (observations / observed-data / bundle / SCO list)
- [ ] **`stix`** — umbrella crate with high-level entry points
- [ ] `FOLLOWEDBY` sequencing + temporal-qualifier matching
- [ ] STIX 2.0 support via the version seam
- [ ] Consumer-injectable custom typed models (registry)
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
