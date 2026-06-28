# STIX-Rust Parser & Matcher — Design

**Date:** 2026-06-28
**Status:** Approved (brainstorming complete; pending spec review)

## Purpose

A reusable, pure-Rust library to:

1. **Parse** STIX 2 patterning-language patterns into a typed AST.
2. **Import** STIX 2 objects (SDOs, SCOs, SROs) and bundles.
3. **Match** patterns against a set of objects ("test patterns against an object set").

Reusability is the paramount goal. Later phases add Python and TypeScript bindings.

## Scope (Phase 1)

- **STIX version:** Target **2.1** now, with internal seams so additional versions
  (2.0, future) can be added without public API churn.
- **Pattern grammar:** Parse the **complete** patterning grammar (comparisons,
  `AND`/`OR`/`FOLLOWEDBY`, and the `WITHIN` / `REPEATS` / `START..STOP` qualifiers).
- **Matching:** Implement **core matching** first — single-observation comparison
  expressions (all operators) plus boolean logic. `FOLLOWEDBY` sequencing and
  temporal qualifiers are **parsed but not yet matched**; reaching them yields an
  explicit, typed "unsupported" result rather than silently passing.

### Out of scope (Phase 1)

- FOLLOWEDBY / temporal-qualifier *matching* (parsing is in scope).
- Python / TypeScript bindings (later phases; design leaves clean seams).
- Consumer-registered custom typed models (see Future Considerations).

## Reference Alignment

The OASIS/MITRE reference (`cti-pattern-matcher`) exposes
`match(pattern, observed_data_sdos, stix_version=...)`:

- Each `observed-data` SDO is **one observation**; its `number_observed`,
  `first_observed`, `last_observed` drive temporal qualifiers.
- 2.0 holds SCOs inline in `objects`; 2.1 holds `object_refs` resolved against the
  surrounding bundle.
- The engine is a single-pass, generator/binding-set search (not backtracking).

Our internal matching unit (`Observation` = set of SCOs + temporal metadata)
matches the reference's internal model, while we expose **multiple entry points**
for ergonomics.

## Architecture

A Cargo workspace of four library crates, with binding crates reserved for later:

```
stix-rust/                  (workspace root)
├── crates/
│   ├── stix-model/         object model: types, value view, bundle, object store
│   ├── stix-pattern/       lexer + recursive-descent parser → AST
│   ├── stix-matcher/       matching engine (model + pattern → results)
│   └── stix/               umbrella: re-exports the three; high-level entry points
└── bindings/               (later phases)
    ├── stix-py/            PyO3
    └── stix-ts/            wasm-bindgen / napi
```

**Dependency edges (no cycles):**

- `stix-pattern` → std + serde only
- `stix-model` → serde only
- `stix-matcher` → `stix-model` + `stix-pattern`
- `stix` → all three

Each lower crate is independently usable (e.g. depend on `stix-pattern` alone for
just the parser).

## Components

### `stix-model`

- **`StixValue`** — uniform dynamic value type (string, int, float, bool,
  timestamp, binary, list, map, null) that the matcher walks.
- **Typed core** — `serde`-derived structs for common SDOs (indicator,
  observed-data, malware, …) and SCOs (file, ipv4-addr, network-traffic, url, …),
  each with `#[serde(flatten)] extra` to retain custom/unknown properties.
- **`StixObject`** — `Typed(..)` | `Generic(value map)`; deserialization tries
  typed, falls back to generic. Both implement the **`ObjectView`** trait:
  `id()`, `type_()`, `get_path(&ObjectPath) -> Option<StixValue>`. The matcher
  consumes `ObjectView` only, so it never needs a dual typed/untyped code path.
- **`Bundle`** — parse/serialize STIX bundles.
- **`ObjectStore`** — `id → object` index; resolves `object_refs` and reference
  properties (e.g. `network-traffic.src_ref`).
- **Version awareness** — `SpecVersion` enum + internal trait seam for
  version-specific differences (inline `objects` vs `object_refs`, type renames).

### `stix-pattern`

- **Lexer** → token stream: string/binary/timestamp literals, operators, and
  keywords (`AND` `OR` `FOLLOWEDBY` `WITHIN` `REPEATS` `START` `STOP` `LIKE`
  `MATCHES` `ISSUBSET` `ISSUPERSET` `EXISTS` `IN` `NOT`).
- **Recursive-descent + Pratt** parser → typed **AST**:
  - `Pattern` → `ObservationExpression` tree (`FollowedBy`/`And`/`Or`,
    parenthesized, with `Qualifier`s: `Within(seconds)`, `Repeats(n)`,
    `StartStop(t, t)`).
  - `ComparisonExpression` tree (`And`/`Or` of
    `Comparison { object_path, operator, value, negated }`).
  - `ObjectPath` (object-type root, then property/key/index/`*` steps, with
    reference dereference handling).
- Parses the **full** grammar even though qualifier/FOLLOWEDBY matching is deferred.
- **`ParseError`** carries byte offset + span for good diagnostics and clean
  binding error mapping.
- AST is `serde`-serializable (useful for tooling and bindings).

### `stix-matcher`

- Normalizes all inputs to
  **`Observation { scos: Vec<ObjectView>, first_observed, last_observed, number_observed }`**.
- **Entry points** (all convert to `Vec<Observation>`):
  - `match_observations(&Pattern, &[Observation])`
  - `match_observed_data(&Pattern, &[ObservedData], &ObjectStore)` — MITRE-compatible
  - `match_bundle(&Pattern, &Bundle)` — derives observations from observed-data SDOs
  - `match_scos(&Pattern, &[StixObject])` — treats the list as a single observation
- **Phase-1 matching:** comparison expressions (all operators incl.
  `LIKE`/`MATCHES`/`IN`/`ISSUBSET`/`ISSUPERSET`/`EXISTS`), boolean logic, and
  object-path resolution (incl. list / `*` / reference dereference) within a single
  observation.
- **Deferred (parsed, not matched):** `FOLLOWEDBY` sequencing + temporal
  qualifiers return a clear typed `Unsupported` result rather than passing
  silently. Engine uses a binding-set model (like the reference) so
  cross-observation matching slots in later.
- Returns **`MatchResult`**: matched/not + which observations/objects bound.

## Data Flow

```
pattern string ──[stix-pattern lexer+parser]──► Pattern AST
JSON bundle ─────[stix-model]──► Bundle / StixObject / ObjectStore
                                       │
Pattern AST + observations ──[stix-matcher]──► MatchResult
```

Entry points in `stix-matcher` convert bundles / observed-data / SCO lists into the
normalized `Observation` set before running the engine.

## Error Handling

- Per-crate error enums via `thiserror`: `ParseError`, `ModelError`, `MatchError`.
- **No panics** in library paths; everything returns `Result`.
- Invalid patterns fail at parse; matching errors (e.g. reaching an unsupported
  feature, malformed object path against data) are distinct from "no match."
- Errors carry structure (codes, spans) for clean mapping onto Python exceptions /
  JS errors in later phases.

## Testing

- **TDD throughout** (test-driven-development skill during implementation).
- Unit tests per crate: lexer tokens, parser AST shapes, each comparison operator,
  object-path edge cases.
- **Conformance corpus:** import OASIS valid/invalid pattern lists and MITRE
  matcher example observed-data + expected results as fixtures, to validate
  spec-faithfulness.
- Round-trip tests: parse → serialize AST; object deserialize → serialize.
- Doc tests on public entry points.

## Future Considerations

### Consumer-injected custom models (explicitly requested)

The hybrid object model already lets custom/unknown object types parse and match
via the value-backed `Generic` path + `ObjectView`. The future enhancement is to
let a **consumer of the library register their own models and semantics**:

- A **registry** mapping `type` name → consumer-provided deserializer / typed
  struct, so custom SDO/SCO types deserialize into the consumer's types while still
  implementing `ObjectView`.
- Registration of **custom properties** and **custom object types** without forking
  the crate.
- Optional **custom object-path / operator hooks** for domain-specific semantics.

Design the `ObjectView` trait, deserialization entry, and (later) the matcher to
accept an injected registry/config object so this is additive, not a breaking
change. Build in a later phase; keep the seams clean now.

### Other

- FOLLOWEDBY + temporal-qualifier matching (completes the matcher).
- STIX 2.0 (and future versions) via the `SpecVersion` seam.
- Python (PyO3) and TypeScript (wasm-bindgen / napi) bindings.
