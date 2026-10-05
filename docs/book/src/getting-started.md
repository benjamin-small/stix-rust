# Getting Started

## Install

### Rust

```bash
cargo add stix-rust
```

Or from source:

```toml
[dependencies]
stix = { git = "https://github.com/benjamin-small/stix-rust", package = "stix-rust" }
```

The package is `stix-rust`; the library name is `stix`, so code reads `use stix::…`
either way.

### Python

```bash
pip install stix-rust
```

Or from source:

```bash
pip install "maturin>=1.5,<2.0"
cd bindings/python && maturin develop
```

### TypeScript

```bash
npm install @stix-rust/node    # native Node addon
npm install @stix-rust/wasm    # portable WebAssembly (Node + browser)
```

Or from source: `cd bindings/typescript-node && npm install && npm run build`
(same for `typescript-wasm`).

### Java

> **Maven Central: not yet published** — source-only for now; the coordinate below
> is the planned artifact.

```kotlin
// Gradle (Kotlin DSL)
implementation("io.github.benjaminsmall:stix:0.2.0")
```

Or from source: `cd bindings/java && gradle test` builds the native library and
runs the suite; see the [Java page](bindings/java.md) for library-loading details.

## Quick start (Rust)

```rust
use stix::{parse, matcher::match_bundle, model::Bundle};

let pattern = parse("[ipv4-addr:value = '198.51.100.5']")?;

let bundle = Bundle::from_json_str(r#"{
  "type": "bundle",
  "objects": [
    { "type": "ipv4-addr", "id": "ipv4-addr--a1", "value": "198.51.100.5" },
    { "type": "observed-data", "id": "observed-data--a1",
      "first_observed": "2020-03-01T12:00:00Z",
      "last_observed":  "2020-03-01T12:10:00Z",
      "number_observed": 1,
      "object_refs": ["ipv4-addr--a1"] }
  ]
}"#)?;

let result = match_bundle(&pattern, &bundle)?;
assert!(result.is_match());
```

That's the whole loop: **parse a pattern, import a bundle, match**. The same three
steps exist in every binding — see the [language pages](bindings/python.md) for the
identical example in Python, Java, and TypeScript.

Next stops:

- [Patterns](guide/patterns.md) — what the pattern language can express.
- [Matching](guide/matching.md) — how evaluation actually works.
- [Custom Object Types](guide/custom-types.md) — extending the model without forking.
