# stix-ffi Facade Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `crates/stix-ffi` — a small, panic-free, pure-Rust facade over the `stix` umbrella (handles `Engine`/`Pattern`/`Bundle`/`MatchOutcome` + `FfiError`, custom-model host hook) that the language bindings will each wrap.

**Architecture:** A new workspace member depending on the `stix` umbrella. No FFI macros — plain Rust, fully unit-testable on any platform. `Engine` owns a `ModelRegistry`; parse/match return opaque handles or small value types; deep structure (AST, objects) crosses as JSON via `to_json`/`object_json`.

**Tech Stack:** Rust (edition 2021), `stix` umbrella crate (re-exports pattern/model/matcher), `serde_json`, `thiserror`.

---

## Current shapes this facade wraps (verified)

- `stix::parse(&str) -> Result<stix_pattern::Pattern, stix_pattern::ParseError>`; `Pattern` is serde-(de)serializable.
- `stix::model::{ModelRegistry, Bundle, StixObject, ObjectView}`; `ModelRegistry::{new, register_handler(name, Fn(Value)->Result<Value,ModelError>), parse_bundle(&str)->Result<Bundle,ModelError>}`; `Bundle { type_, id, objects: Vec<StixObject> }`; `StixObject` is `Serialize` + impls `ObjectView` (`id`/`type_`).
- `stix::matcher::{match_bundle(&Pattern,&Bundle)->Result<MatchResult,MatchError>, MatchResult}`; `MatchResult::{is_match()->bool, observations()->&[usize]}`.
- Errors: `stix_pattern::ParseError`, `stix_model::ModelError`, `stix_matcher::MatchError` all `impl std::error::Error + Display`.

## File Structure

- `Cargo.toml` (root) — add `crates/stix-ffi` to `members`; add a workspace dep entry.
- `crates/stix-ffi/Cargo.toml` — manifest.
- `crates/stix-ffi/src/lib.rs` — module wiring + re-exports + crate docs.
- `crates/stix-ffi/src/error.rs` — `ErrorCode`, `FfiError`, `From` conversions.
- `crates/stix-ffi/src/handles.rs` — `Pattern`, `Bundle`, `MatchOutcome`.
- `crates/stix-ffi/src/engine.rs` — `Engine`.
- `crates/stix-ffi/tests/facade.rs` — integration test of the whole surface.

---

## Task 1: Scaffold the crate

**Files:**
- Modify: `Cargo.toml` (root)
- Create: `crates/stix-ffi/Cargo.toml`
- Create: `crates/stix-ffi/src/lib.rs`

- [ ] **Step 1: Register the crate in the workspace**

In the root `Cargo.toml`, change the `members` line to include the new crate:

```toml
members = ["crates/stix-pattern", "crates/stix-model", "crates/stix-matcher", "crates/stix", "crates/stix-ffi"]
```

Add to `[workspace.dependencies]` (under the existing entries):

```toml
stix = { path = "crates/stix" }
```

- [ ] **Step 2: Create the crate manifest**

Create `crates/stix-ffi/Cargo.toml`:

```toml
[package]
name = "stix-ffi"
version = "0.0.1"
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "FFI-friendly facade over the stix toolkit, wrapped by the language bindings."

[dependencies]
stix = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
```

- [ ] **Step 3: Create a placeholder lib.rs**

Create `crates/stix-ffi/src/lib.rs`:

```rust
//! FFI-friendly facade over the stix toolkit.
//!
//! Pure Rust (no FFI macros). The language bindings each wrap this surface.

#[cfg(test)]
mod smoke {
    #[test]
    fn crate_builds() {
        assert_eq!(2 + 2, 4);
    }
}
```

- [ ] **Step 4: Verify the workspace builds**

Run: `cargo test -p stix-ffi`
Expected: compiles; `smoke::crate_builds` passes.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/stix-ffi/Cargo.toml crates/stix-ffi/src/lib.rs
git commit -m "feat(ffi): scaffold stix-ffi facade crate"
```

---

## Task 2: FfiError + ErrorCode

**Files:**
- Create: `crates/stix-ffi/src/error.rs`
- Modify: `crates/stix-ffi/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/stix-ffi/src/error.rs`:

```rust
//! The facade's flat error type, mappable onto host-language exceptions.

/// A coarse category each binding maps to its own exception type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    Parse,
    Model,
    Match,
    Validation,
}

/// A flat, FFI-friendly error: a category plus a human-readable message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfiError {
    pub code: ErrorCode,
    pub message: String,
}

impl FfiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        FfiError {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for FfiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for FfiError {}

impl From<stix::pattern::ParseError> for FfiError {
    fn from(e: stix::pattern::ParseError) -> Self {
        FfiError::new(ErrorCode::Parse, e.to_string())
    }
}

impl From<stix::model::ModelError> for FfiError {
    fn from(e: stix::model::ModelError) -> Self {
        FfiError::new(ErrorCode::Model, e.to_string())
    }
}

impl From<stix::matcher::MatchError> for FfiError {
    fn from(e: stix::matcher::MatchError) -> Self {
        FfiError::new(ErrorCode::Match, e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_error_maps_to_parse_code() {
        let e = stix::parse("[bad").unwrap_err();
        let f: FfiError = e.into();
        assert_eq!(f.code, ErrorCode::Parse);
        assert!(!f.message.is_empty());
    }

    #[test]
    fn display_includes_code_and_message() {
        let f = FfiError::new(ErrorCode::Validation, "missing field");
        let s = format!("{f}");
        assert!(s.contains("Validation"));
        assert!(s.contains("missing field"));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-ffi`
Expected: FAIL — `error` module not declared.

- [ ] **Step 3: Wire the module**

Set `crates/stix-ffi/src/lib.rs` to:

```rust
//! FFI-friendly facade over the stix toolkit.
//!
//! Pure Rust (no FFI macros). The language bindings each wrap this surface.

pub mod error;

pub use error::{ErrorCode, FfiError};
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p stix-ffi`
Expected: PASS — both error tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-ffi/src/error.rs crates/stix-ffi/src/lib.rs
git commit -m "feat(ffi): add FfiError and ErrorCode with source conversions"
```

---

## Task 3: Handles — Pattern, Bundle, MatchOutcome

**Files:**
- Create: `crates/stix-ffi/src/handles.rs`
- Modify: `crates/stix-ffi/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/stix-ffi/src/handles.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_to_json_round_trips() {
        let inner = stix::parse("[ipv4-addr:value = '1.2.3.4']").unwrap();
        let handle = Pattern::new(inner.clone());
        let json = handle.to_json();
        let back: stix::pattern::Pattern = serde_json::from_str(&json).unwrap();
        assert_eq!(back, inner);
    }

    #[test]
    fn bundle_object_access() {
        let raw = r#"{"type":"bundle","id":"bundle--1","objects":[
            {"type":"ipv4-addr","id":"ipv4-addr--1","value":"1.2.3.4"}
        ]}"#;
        let inner = stix::model::Bundle::from_json_str(raw).unwrap();
        let handle = Bundle::new(inner);
        assert_eq!(handle.object_count(), 1);
        let obj_json = handle.object_json(0).unwrap();
        assert!(obj_json.contains("ipv4-addr--1"));
        assert!(handle.object_json(5).is_none());
    }

    #[test]
    fn match_outcome_fields() {
        let o = MatchOutcome {
            matched: true,
            observations: vec![0, 2],
        };
        assert!(o.matched);
        assert_eq!(o.observations, vec![0, 2]);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-ffi`
Expected: FAIL — `Pattern`/`Bundle`/`MatchOutcome` not found.

- [ ] **Step 3: Implement the handles**

At the top of `crates/stix-ffi/src/handles.rs` (above the test module):

```rust
//! Opaque handles (`Pattern`, `Bundle`) and the plain `MatchOutcome` value.

/// Opaque handle around a parsed pattern AST.
#[derive(Debug, Clone)]
pub struct Pattern {
    inner: stix::pattern::Pattern,
}

impl Pattern {
    pub(crate) fn new(inner: stix::pattern::Pattern) -> Self {
        Pattern { inner }
    }

    pub(crate) fn inner(&self) -> &stix::pattern::Pattern {
        &self.inner
    }

    /// The pattern's AST serialized as compact JSON.
    pub fn to_json(&self) -> String {
        // Serialization of the AST is infallible in practice; fall back to "null".
        serde_json::to_string(&self.inner).unwrap_or_else(|_| "null".to_string())
    }
}

/// Opaque handle around an imported bundle.
#[derive(Debug, Clone)]
pub struct Bundle {
    inner: stix::model::Bundle,
}

impl Bundle {
    pub(crate) fn new(inner: stix::model::Bundle) -> Self {
        Bundle { inner }
    }

    pub(crate) fn inner(&self) -> &stix::model::Bundle {
        &self.inner
    }

    /// Number of objects in the bundle.
    pub fn object_count(&self) -> usize {
        self.inner.objects.len()
    }

    /// The object at `index` serialized as JSON, or `None` if out of range.
    pub fn object_json(&self, index: usize) -> Option<String> {
        let obj = self.inner.objects.get(index)?;
        Some(serde_json::to_string(obj).unwrap_or_else(|_| "null".to_string()))
    }
}

/// The outcome of a match: whether it matched and which observation indices bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchOutcome {
    pub matched: bool,
    pub observations: Vec<u64>,
}
```

In `crates/stix-ffi/src/lib.rs`, add:

```rust
pub mod handles;

pub use handles::{Bundle, MatchOutcome, Pattern};
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p stix-ffi`
Expected: PASS — all handle tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-ffi/src/handles.rs crates/stix-ffi/src/lib.rs
git commit -m "feat(ffi): add Pattern/Bundle handles and MatchOutcome"
```

---

## Task 4: Engine — parse + match

**Files:**
- Create: `crates/stix-ffi/src/engine.rs`
- Modify: `crates/stix-ffi/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/stix-ffi/src/engine.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;

    fn bundle_json() -> &'static str {
        r#"{"type":"bundle","id":"bundle--1","objects":[
            {"type":"ipv4-addr","id":"ipv4-addr--1","value":"198.51.100.5"},
            {"type":"observed-data","id":"observed-data--1",
             "first_observed":"2020-01-01T00:00:00Z","last_observed":"2020-01-01T00:00:00Z",
             "number_observed":1,"object_refs":["ipv4-addr--1"]}
        ]}"#
    }

    #[test]
    fn parse_pattern_ok_and_err() {
        let engine = Engine::new();
        assert!(engine.parse_pattern("[ipv4-addr:value = '1.2.3.4']").is_ok());
        let err = engine.parse_pattern("[bad").unwrap_err();
        assert_eq!(err.code, ErrorCode::Parse);
    }

    #[test]
    fn parse_bundle_ok_and_non_bundle_err() {
        let engine = Engine::new();
        assert!(engine.parse_bundle(bundle_json()).is_ok());
        let err = engine
            .parse_bundle(r#"{"type":"ipv4-addr","id":"x--1"}"#)
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::Model);
    }

    #[test]
    fn match_bundle_match_and_non_match() {
        let engine = Engine::new();
        let bundle = engine.parse_bundle(bundle_json()).unwrap();

        let hit = engine.parse_pattern("[ipv4-addr:value = '198.51.100.5']").unwrap();
        let outcome = engine.match_bundle(&hit, &bundle).unwrap();
        assert!(outcome.matched);

        let miss = engine.parse_pattern("[ipv4-addr:value = '203.0.113.9']").unwrap();
        assert!(!engine.match_bundle(&miss, &bundle).unwrap().matched);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-ffi`
Expected: FAIL — `Engine` not found.

- [ ] **Step 3: Implement Engine (parse + match)**

At the top of `crates/stix-ffi/src/engine.rs` (above the test module):

```rust
//! The `Engine` handle: owns a registry, parses patterns/bundles, runs matches.

use stix::model::ModelRegistry;

use crate::error::FfiError;
use crate::handles::{Bundle, MatchOutcome, Pattern};

/// The stateful facade handle. Holds the custom-model registry used by
/// `parse_bundle`.
#[derive(Default)]
pub struct Engine {
    registry: ModelRegistry,
}

impl Engine {
    /// A new engine with an empty registry.
    pub fn new() -> Self {
        Engine::default()
    }

    /// Parse a STIX pattern string into a [`Pattern`] handle.
    pub fn parse_pattern(&self, src: &str) -> Result<Pattern, FfiError> {
        let inner = stix::parse(src)?;
        Ok(Pattern::new(inner))
    }

    /// Parse a STIX bundle (consulting registered custom types) into a [`Bundle`].
    pub fn parse_bundle(&self, json: &str) -> Result<Bundle, FfiError> {
        let inner = self.registry.parse_bundle(json)?;
        Ok(Bundle::new(inner))
    }

    /// Match a pattern against a bundle.
    pub fn match_bundle(&self, pattern: &Pattern, bundle: &Bundle) -> Result<MatchOutcome, FfiError> {
        let result = stix::matcher::match_bundle(pattern.inner(), bundle.inner())?;
        Ok(MatchOutcome {
            matched: result.is_match(),
            observations: result.observations().iter().map(|&i| i as u64).collect(),
        })
    }
}
```

In `crates/stix-ffi/src/lib.rs`, add:

```rust
pub mod engine;

pub use engine::Engine;
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p stix-ffi`
Expected: PASS — all engine parse/match tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-ffi/src/engine.rs crates/stix-ffi/src/lib.rs
git commit -m "feat(ffi): add Engine with parse_pattern/parse_bundle/match_bundle"
```

---

## Task 5: Engine — custom-model host hook

**Files:**
- Modify: `crates/stix-ffi/src/engine.rs`

- [ ] **Step 1: Write the failing test**

Add these tests inside the existing `mod tests` in `crates/stix-ffi/src/engine.rs`:

```rust
    use crate::error::ErrorCode as Code;

    fn custom_bundle_json() -> &'static str {
        r#"{"type":"bundle","id":"bundle--1","objects":[
            {"type":"x-acme-widget","id":"x-acme-widget--1","risk_score":90},
            {"type":"observed-data","id":"observed-data--1",
             "first_observed":"2020-01-01T00:00:00Z","last_observed":"2020-01-01T00:00:00Z",
             "number_observed":1,"object_refs":["x-acme-widget--1"]}
        ]}"#
    }

    #[test]
    fn register_type_adds_computed_property_and_matches() {
        let mut engine = Engine::new();
        engine.register_type(
            "x-acme-widget",
            Box::new(|mut obj| {
                let score = obj.get("risk_score").and_then(|v| v.as_i64()).unwrap_or(0);
                obj["risk_band"] = serde_json::json!(if score > 80 { "high" } else { "low" });
                Ok(obj)
            }),
        );
        let bundle = engine.parse_bundle(custom_bundle_json()).unwrap();
        let pattern = engine.parse_pattern("[x-acme-widget:risk_band = 'high']").unwrap();
        assert!(engine.match_bundle(&pattern, &bundle).unwrap().matched);
    }

    #[test]
    fn register_type_rejection_is_validation_error() {
        let mut engine = Engine::new();
        engine.register_type(
            "x-acme-widget",
            Box::new(|obj| {
                if obj.get("risk_score").is_none() {
                    return Err("missing risk_score".to_string());
                }
                Ok(obj)
            }),
        );
        let err = engine
            .parse_bundle(
                r#"{"type":"bundle","objects":[{"type":"x-acme-widget","id":"x--1"}]}"#,
            )
            .unwrap_err();
        assert_eq!(err.code, Code::Validation);
        assert!(err.message.contains("missing risk_score"));
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-ffi engine`
Expected: FAIL — `register_type` not found.

- [ ] **Step 3: Implement register_type**

In `crates/stix-ffi/src/engine.rs`, add `use crate::error::ErrorCode;` to the imports, and add this method inside `impl Engine` (after `new`):

```rust
    /// Register a custom object type. `hook` validates and/or normalizes a raw JSON
    /// object of `type_name`; returning `Err(message)` rejects it (surfaced as a
    /// `Validation` error from `parse_bundle`). Returning an enriched object adds
    /// computed properties, stored as data. The hook runs only at parse time.
    pub fn register_type(
        &mut self,
        type_name: &str,
        hook: Box<dyn Fn(serde_json::Value) -> Result<serde_json::Value, String> + Send + Sync>,
    ) {
        self.registry.register_handler(type_name, move |value| {
            hook(value).map_err(|message| {
                stix::model::ModelError::InvalidObject(format!("[Validation] {message}"))
            })
        });
    }
```

Then, so a hook rejection maps to `ErrorCode::Validation` (not the generic `Model`
mapping), adjust `parse_bundle` to detect the `[Validation]` marker:

```rust
    /// Parse a STIX bundle (consulting registered custom types) into a [`Bundle`].
    pub fn parse_bundle(&self, json: &str) -> Result<Bundle, FfiError> {
        match self.registry.parse_bundle(json) {
            Ok(inner) => Ok(Bundle::new(inner)),
            Err(stix::model::ModelError::InvalidObject(m)) if m.starts_with("[Validation] ") => {
                Err(FfiError::new(
                    ErrorCode::Validation,
                    m.trim_start_matches("[Validation] ").to_string(),
                ))
            }
            Err(e) => Err(e.into()),
        }
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p stix-ffi engine`
Expected: PASS — including the computed-property match and the `Validation` rejection.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-ffi/src/engine.rs
git commit -m "feat(ffi): add register_type host hook with Validation error mapping"
```

---

## Task 6: End-to-end facade integration test

**Files:**
- Create: `crates/stix-ffi/tests/facade.rs`

- [ ] **Step 1: Write the integration test**

Create `crates/stix-ffi/tests/facade.rs`:

```rust
use stix_ffi::{Engine, ErrorCode};

fn bundle_json() -> &'static str {
    r#"{"type":"bundle","id":"bundle--1","objects":[
        {"type":"ipv4-addr","id":"ipv4-addr--1","value":"198.51.100.5"},
        {"type":"observed-data","id":"observed-data--1",
         "first_observed":"2020-01-01T00:00:00Z","last_observed":"2020-01-01T00:00:00Z",
         "number_observed":1,"object_refs":["ipv4-addr--1"]}
    ]}"#
}

#[test]
fn full_surface_round_trip() {
    let engine = Engine::new();

    // Pattern handle -> AST JSON.
    let pattern = engine.parse_pattern("[ipv4-addr:value = '198.51.100.5']").unwrap();
    let ast = pattern.to_json();
    assert!(ast.contains("ipv4-addr"));

    // Bundle handle -> object access.
    let bundle = engine.parse_bundle(bundle_json()).unwrap();
    assert_eq!(bundle.object_count(), 2);
    assert!(bundle.object_json(0).unwrap().contains("ipv4-addr--1"));

    // Match -> outcome.
    let outcome = engine.match_bundle(&pattern, &bundle).unwrap();
    assert!(outcome.matched);
    assert!(!outcome.observations.is_empty());
}

#[test]
fn error_codes_surface() {
    let engine = Engine::new();
    assert_eq!(engine.parse_pattern("[bad").unwrap_err().code, ErrorCode::Parse);
    assert_eq!(
        engine.parse_bundle("not json").unwrap_err().code,
        ErrorCode::Model
    );
}
```

- [ ] **Step 2: Run the integration test**

Run: `cargo test -p stix-ffi --test facade`
Expected: PASS — both end-to-end tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/stix-ffi/tests/facade.rs
git commit -m "test(ffi): end-to-end facade integration test"
```

---

## Task 7: Crate docs, lint, format, final verification

**Files:**
- Modify: `crates/stix-ffi/src/lib.rs`

- [ ] **Step 1: Add a crate-level doc example**

Set `crates/stix-ffi/src/lib.rs` to (module declarations unchanged, doc example added):

```rust
//! FFI-friendly facade over the stix toolkit.
//!
//! Pure Rust (no FFI macros). The language bindings each wrap this surface:
//! an [`Engine`] parses patterns and bundles into opaque [`Pattern`]/[`Bundle`]
//! handles and runs matches, returning a [`MatchOutcome`]; deep structure (the AST,
//! object properties) crosses as JSON.
//!
//! ```
//! use stix_ffi::Engine;
//!
//! let engine = Engine::new();
//! let pattern = engine.parse_pattern("[ipv4-addr:value = '198.51.100.5']").unwrap();
//! let bundle = engine.parse_bundle(r#"{"type":"bundle","objects":[
//!     {"type":"ipv4-addr","id":"ipv4-addr--1","value":"198.51.100.5"},
//!     {"type":"observed-data","id":"observed-data--1",
//!      "first_observed":"2020-01-01T00:00:00Z","last_observed":"2020-01-01T00:00:00Z",
//!      "number_observed":1,"object_refs":["ipv4-addr--1"]}
//! ]}"#).unwrap();
//! assert!(engine.match_bundle(&pattern, &bundle).unwrap().matched);
//! ```

pub mod engine;
pub mod error;
pub mod handles;

pub use engine::Engine;
pub use error::{ErrorCode, FfiError};
pub use handles::{Bundle, MatchOutcome, Pattern};
```

- [ ] **Step 2: Format**

Run: `cargo fmt --all`
Then review: `git diff`.

- [ ] **Step 3: Clippy (warnings as errors)**

Run: `cargo clippy -p stix-ffi --all-targets -- -D warnings`
Expected: no warnings. Fix any inline.

- [ ] **Step 4: Full workspace test run**

Run: `cargo test`
Expected: all suites PASS, including `stix-ffi` unit + integration + doc tests.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-ffi/src/lib.rs
git commit -m "docs(ffi): add crate doc example; fmt + clippy clean"
```

---

## Self-Review Notes (already applied)

- **Spec coverage:** crate scaffold + workspace member (Task 1); `FfiError`/`ErrorCode`
  + source conversions (Task 2); `Pattern`/`Bundle`/`MatchOutcome` handles incl.
  `to_json`/`object_json` (Task 3); `Engine` parse + `match_bundle` (Task 4);
  `register_type` host hook with `Validation` mapping (Task 5); end-to-end test
  (Task 6); crate doc + fmt/clippy (Task 7). `match_bundle`-only per spec (no
  `match_scos`). All spec sections map to a task.
- **Type consistency:** `Engine::{new, register_type, parse_pattern, parse_bundle,
  match_bundle}`, `Pattern::{to_json}`, `Bundle::{object_count, object_json}`,
  `MatchOutcome { matched, observations: Vec<u64> }`, `FfiError { code, message }`,
  `ErrorCode::{Parse, Model, Match, Validation}`, and the `pub(crate)` `inner()`
  accessors are used consistently across tasks and tests, and match the verified
  `stix` umbrella API (`stix::parse`, `stix::model::*`, `stix::matcher::match_bundle`).
- **Validation mapping** is implemented with a `[Validation] ` marker prefix on the
  hook's error so `parse_bundle` can distinguish a hook rejection from other model
  errors — explained inline in Task 5.
- **No placeholders:** every code step is complete and compilable.
```
