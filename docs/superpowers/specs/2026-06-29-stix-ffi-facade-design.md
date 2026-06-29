# stix-ffi Facade — Design

**Date:** 2026-06-29
**Status:** Approved (brainstorming complete; pending spec review)
**Scope:** SP0 of the language-bindings effort — the shared, FFI-friendly Rust
facade that the Python, Java, and TypeScript (Node + wasm) bindings each wrap.
**Owner agent:** `rust-core` (the crate lives under `crates/`).

## Purpose

Expose a small, stable, panic-free surface over the `stix` umbrella so each binding
is a thin wrapper instead of re-implementing parse/match/registry plumbing four
times. The surface is **hybrid**: opaque typed handles (`Engine`, `Pattern`,
`Bundle`, `MatchOutcome`) with **JSON for deep structure** (AST dumps, object
properties). It includes consumer custom-model registration via a host callback,
bridged through a data-level closure.

This is **pure Rust** — no `#[pyclass]`/`#[napi]`/`#[wasm_bindgen]`/JNI macros — so
it builds and unit-tests on every platform with no binding present.

## Cross-binding rationale (why a facade, why these shapes)

- One place centralizes error→code mapping, JSON marshaling, and the host-hook
  adapter; all four bindings stay consistent and thin.
- Custom-model hooks run **only at parse time, synchronously, in-call-stack** (the
  property is resolved into stored data — established in the custom-models design),
  so there is **no per-match FFI re-entrancy**. Each binding bridges its host
  callable (PyO3 `Py<PyAny>`, JNI global ref, napi threadsafe fn, wasm
  `js_sys::Function`) into the facade's `Fn(Value) -> Result<Value, String>`.

## Architecture

New workspace member `crates/stix-ffi`, depending on the `stix` umbrella crate
(which re-exports `pattern`/`model`/`matcher`). Added to the workspace `members`
list. Modules:

```
crates/stix-ffi/src/
├── lib.rs        # re-exports; crate docs
├── error.rs      # FfiError, ErrorCode
├── engine.rs     # Engine (owns ModelRegistry) + parse/match methods
└── handles.rs    # Pattern, Bundle, MatchOutcome
```

## Components

### `FfiError` / `ErrorCode` (`error.rs`)

```rust
pub enum ErrorCode { Parse, Model, Match, Validation }

pub struct FfiError { pub code: ErrorCode, pub message: String }
```

- `From<stix_pattern::ParseError>` → `Parse`; `From<stix_model::ModelError>` →
  `Model` (a validation/hook failure surfaced from `parse_bundle` is `Validation` —
  see below); `From<stix_matcher::MatchError>` → `Match`.
- Bindings map `code` → their host exception type, and pass `message` through.

### `Pattern` (`handles.rs`)

Opaque handle wrapping `stix_pattern::Pattern`.

- `to_json(&self) -> String` — the serde AST dump (pretty or compact; compact).

### `Bundle` (`handles.rs`)

Opaque handle wrapping `stix_model::Bundle`.

- `object_count(&self) -> usize`
- `object_json(&self, index: usize) -> Option<String>` — the object at `index`
  serialized to JSON, or `None` if out of range.

### `MatchOutcome` (`handles.rs`)

Plain value (not opaque): `{ matched: bool, observations: Vec<u64> }`. Built from
`stix_matcher::MatchResult` (`is_match()` and the observation indices, widened to
`u64` for binding friendliness). Bindings expose `matched`/`observations` as
fields or getters.

### `Engine` (`engine.rs`)

Owns a `stix_model::ModelRegistry`. The single stateful handle.

```rust
impl Engine {
    pub fn new() -> Self;

    pub fn register_type(
        &mut self,
        type_name: &str,
        hook: Box<dyn Fn(serde_json::Value) -> Result<serde_json::Value, String> + Send + Sync>,
    );

    pub fn parse_pattern(&self, src: &str) -> Result<Pattern, FfiError>;
    pub fn parse_bundle(&self, json: &str) -> Result<Bundle, FfiError>;
    pub fn match_bundle(&self, pattern: &Pattern, bundle: &Bundle) -> Result<MatchOutcome, FfiError>;
}
```

- `register_type` adapts the host hook into `ModelRegistry::register_handler` by
  mapping the hook's `Err(String)` into `ModelError::InvalidObject(String)`.
- `parse_pattern` calls `stix_pattern::parse`; `parse_bundle` calls
  `registry.parse_bundle`; `match_bundle` calls `stix_matcher::match_bundle`.
- **SP0 exposes only `match_bundle`** (YAGNI). `match_scos`/`match_observed_data`
  entry points are added when a binding needs them.

## Data Flow

```
host string ──Engine::parse_pattern──► Pattern handle ──Pattern::to_json──► AST JSON
host string ──Engine::parse_bundle───► Bundle handle  ──Bundle::object_json──► object JSON
Pattern + Bundle ──Engine::match_bundle──► MatchOutcome { matched, observations }

custom type:  host callback ──(binding bridges)──► Box<dyn Fn(Value)->Result<Value,String>>
              registered via Engine::register_type, invoked during parse_bundle only
```

## Error Handling

- The facade is **panic-free**: every fallible method returns `Result<_, FfiError>`;
  out-of-range `object_json` returns `None`, not an error.
- Source errors convert via `From` into the right `ErrorCode`. A custom-hook
  `Err(String)` raised during `parse_bundle` surfaces as `ErrorCode::Validation`.

## Testing (pure Rust, no binding)

- `Engine::parse_pattern` ok + error (bad pattern → `Parse`).
- `Engine::parse_bundle` ok + error (non-bundle → `Model`; hook rejection →
  `Validation`).
- `Engine::match_bundle` match and non-match → correct `MatchOutcome`.
- `register_type` round-trip: register a hook that adds a computed property, parse a
  bundle, match a pattern against that property → `matched == true`.
- `Pattern::to_json` parses back to an equal `Pattern` (round-trip).
- `Bundle::object_count` / `object_json` (valid index returns JSON containing the
  id; out-of-range returns `None`).
- `ErrorCode` mapping: one test per source-error variant.

## Out of Scope

- Any language binding (separate specs SP1–SP4 wrap this facade).
- FFI macros / C ABI — this crate is plain Rust.
- `match_scos` / `match_observed_data` entry points (added on demand).
- Streaming/iterator object access (bindings can loop `object_json` by index).

## Future Considerations

- Additional match entry points and richer `Bundle` access are additive methods on
  the same handles.
- If a binding needs typed downcast of custom objects across FFI, that maps onto
  `StixObject::downcast_ref` in the core; expose via the facade when required.
