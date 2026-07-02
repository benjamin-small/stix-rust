# Docs Core Polish (Stream A) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the core crates documentation-ready: rename the umbrella package to `stix-rust` (lib name stays `stix` — zero source churn) and backfill every missing rustdoc comment, enforced by `#![warn(missing_docs)]`.

**Architecture:** Two independent changes in `crates/` + the root manifest. The rename is Cargo-metadata-only. The backfill uses the compiler as the test: turn on `missing_docs` warnings in all five crates, let clippy `-D warnings` enumerate every gap, document until clean — which also permanently enforces full public-API docs.

**Tech Stack:** Cargo/rustdoc only. No new dependencies, no behavior changes.

---

## Task 1: Rename umbrella package to `stix-rust` (lib stays `stix`)

**Files:**
- Modify: `crates/stix/Cargo.toml`
- Modify: `Cargo.toml` (root)

- [ ] **Step 1: Update the crate manifest**

In `crates/stix/Cargo.toml`, change the `[package] name` and add a `[lib]` section
(keep everything else as-is):

```toml
[package]
name = "stix-rust"
version = "0.0.1"
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "Umbrella crate for the stix-rust toolkit."

[lib]
name = "stix"
```

- [ ] **Step 2: Update the workspace dependency entry**

In the root `Cargo.toml` `[workspace.dependencies]`, change:

```toml
stix = { path = "crates/stix" }
```

to:

```toml
stix = { path = "crates/stix", package = "stix-rust" }
```

(`stix-ffi`'s `stix = { workspace = true }` and all `use stix::...` code keep
working unchanged because the lib target is still named `stix`.)

- [ ] **Step 3: Verify nothing broke**

Run: `cargo test 2>&1 | grep -c "test result: ok"`
Expected: 16 (unchanged; all suites pass — including stix-ffi, the example, and doc tests).

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add crates/stix/Cargo.toml Cargo.toml
git commit -m "chore: rename umbrella package to stix-rust (lib remains stix)"
```

---

## Task 2: Turn on missing_docs and enumerate the gaps

**Files:**
- Modify: `crates/stix-pattern/src/lib.rs`, `crates/stix-model/src/lib.rs`,
  `crates/stix-matcher/src/lib.rs`, `crates/stix/src/lib.rs`, `crates/stix-ffi/src/lib.rs`

- [ ] **Step 1: Add the lint to each crate root**

At the top of each of the five `lib.rs` files (immediately after the crate-level
`//!` doc comment block), add:

```rust
#![warn(missing_docs)]
```

- [ ] **Step 2: Enumerate the failures (this is the failing test)**

Run: `cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -c "missing documentation"`
Expected: a non-zero count. Capture the full list:
`cargo clippy --workspace 2>&1 | grep -A1 "missing documentation" | head -100`

The list should include at least the known gaps from the docs analysis:
`stix_matcher::compare::{value_eq_literal, value_cmp_literal, value_in_set}`,
`stix_matcher::eval::{eval_comparison, eval_comparison_expression, eval_pattern}`
(these may already have docs — the lint is authoritative),
`stix_matcher::pattern_ops::{like_matches, regex_matches}`,
`stix_matcher::subset::{is_subset, is_superset}`, `stix_model::{StixObject fields,
StixValue accessors, ObjectStore methods}`, `stix_pattern::{PathStep variants,
ComparisonOperator variants, TokenKind variants}`, `SpecVersion` variants, and any
struct fields (`Span.start/end`, `FfiError.code/message`, `Observation` fields,
`MatchOutcome` fields, AST struct fields).

- [ ] **Step 3: Commit the lint enablement separately only if you want a red/green history — otherwise proceed to Task 3 and commit together there.**

(Default: proceed; Task 3 commits lint + docs together so `main` history is never red.)

---

## Task 3: Backfill every flagged doc comment

**Files:**
- Modify: whatever files the lint flagged (expected: `crates/stix-pattern/src/{ast,lexer,error}.rs`,
  `crates/stix-model/src/{object,value,store,version,sdo,bundle}.rs`,
  `crates/stix-matcher/src/{compare,eval,pattern_ops,subset,observation,result}.rs`,
  `crates/stix-ffi/src/{error,handles,engine}.rs`)

- [ ] **Step 1: Write the docs**

Document every flagged item until the lint is clean. Style rules:
- One-sentence summary first, in the third person ("Returns…", "The…"), ending with a period.
- Behavior-bearing functions state their edge semantics. Required content for the
  trickiest items (use these, verbatim or lightly adapted):

```rust
/// Compares a resolved value with a pattern literal for equality.
///
/// Integers and floats compare numerically across the int/float divide; string-like
/// literals (string, timestamp, binary, hex) compare as strings. Timestamps are NOT
/// parsed — they compare lexicographically.
pub fn value_eq_literal(...)

/// Orders a resolved value against a pattern literal.
///
/// Returns `None` when the two are not comparable (e.g. bool vs number). Numeric
/// comparison promotes integers to floats; strings compare lexicographically.
pub fn value_cmp_literal(...)

/// Returns true if the value equals any literal in the set (the `IN` operator).
pub fn value_in_set(...)

/// Tests whether `value` matches the SQL-style `LIKE` pattern.
///
/// `%` matches any run of characters and `_` exactly one; all other characters are
/// literal. The pattern is anchored (must match the whole value). An invalid
/// translation never matches.
pub fn like_matches(...)

/// Tests whether `value` matches the regular expression (the `MATCHES` operator).
///
/// The match is unanchored, mirroring the reference implementation. An invalid
/// regex never matches (returns false rather than erroring).
pub fn regex_matches(...)

/// Returns true if `value` (an IP address or CIDR range) is contained within
/// `range` (the `ISSUBSET` operator). IPv4/IPv6 only; mixed families and
/// unparseable inputs never match.
pub fn is_subset(...)

/// Returns true if `value` contains `range` (the `ISSUPERSET` operator) — the
/// inverse of [`is_subset`].
pub fn is_superset(...)
```

- Enum variants and struct fields get short `///` one-liners (e.g. on
  `PathStep::Index`: `/// A concrete list index step, `[n]`.`; on
  `StixValue::as_str`: `/// The string value, if this is a string.`).
- Do NOT change any signatures or behavior; documentation only. If an item
  shouldn't be public at all, do not change its visibility in this plan — document
  it anyway and note it in your report.

- [ ] **Step 2: Verify the lint is clean (the passing test)**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean — zero warnings, including zero `missing documentation`.

Run: `cargo doc --workspace --no-deps 2>&1 | grep -ciE "warning" || echo 0`
Expected: `0` — rustdoc builds without warnings.

Run: `cargo test 2>&1 | grep -c "test result: ok"`
Expected: 16 — no behavior changed.

- [ ] **Step 3: Commit**

```bash
git add crates/
git commit -m "docs: enforce missing_docs and backfill all public API rustdoc"
```

---

## Self-Review Notes (already applied)

- **Spec coverage:** Stream A of the docs-site spec — the `stix-rust` rename with
  `[lib] name = "stix"` (Task 1, exact manifests shown) and the doc-comment
  backfill (Tasks 2–3). The lint mechanism covers the spec's explicit gap list and
  anything the analysis missed, and permanently enforces coverage.
- **Failing-test discipline:** the `missing_docs` lint IS the failing test (Task 2)
  and its cleanliness the passing test (Task 3), keeping TDD shape for a docs task.
- **No behavior changes:** verified by the unchanged 16-suite test count and clippy.
- **No placeholders:** required doc text for every behavior-bearing function is
  given verbatim; trivial one-liners have their style + examples specified and are
  enumerated by the compiler, not left to discretion.
```
