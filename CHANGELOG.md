# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project adheres
to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Releases are tag-driven and cover the Rust crates and the Python, Java, Node and
wasm bindings together, so one entry describes the change everywhere it is
visible. See [RELEASING.md](RELEASING.md).

## [Unreleased]

### Changed

- **BREAKING:** `ObservationExpression::{And, Or, FollowedBy}` and
  `ComparisonExpression::{And, Or}` now hold a `Vec` of two or more operands
  instead of a pair. A left-nested chain is flattened, so `a OR b OR c` is
  `{"Or": [a, b, c]}`; a parenthesized right operand keeps its own node, so
  `a OR (b OR c)` is `{"Or": [a, {"Or": [b, c]}]}`. The JSON for two operands is
  unchanged. Any consumer of `Pattern.ast` that assumed exactly two elements needs
  updating
  ([#52](https://github.com/benjamin-small/stix-rust/issues/52)).
- In the Python binding, a failure converting JSON to a Python object now raises
  `ModelError` rather than `ParseError`, for both `Pattern.ast` and the object
  getters. Code that catches `ParseError` around those may need to catch
  `ModelError` too
  ([#50](https://github.com/benjamin-small/stix-rust/issues/50)).
- **BREAKING:** `ObservationExpression::Observation` is now a struct variant.
  Its serialized JSON changes from `{"Observation": {…}}` to
  `{"Observation": {"expression": {…}, "span": {…}}}`, where the old payload is
  what now sits under `"expression"`. This is visible in every binding's
  `Pattern.ast`, so any consumer that reaches into that JSON needs updating.

### Added

- `stix_pattern::MAX_NESTING` (40), the deepest nesting `parse` accepts: the
  number of parenthesized groups plus qualifiers on any path from the root to a
  leaf. Flat chains such as `a OR b OR c OR …` add no nesting and have no length
  limit.
- `span` fields recording source byte ranges on `Comparison`, `ObjectPath`, and
  the `Observation` and `Qualified` variants of `ObservationExpression`. Note
  that `Pattern.ast` payloads grow accordingly in every binding: about half again
  as large for a small pattern, measured as 174 → 273 bytes of JSON for
  `[file:size > 1024]`.
- `Pattern::without_spans()`, which zeroes every span, for comparing two ASTs
  that came from different source text.
- The `stix_pattern::ir` module: a three-address intermediate representation of a
  parsed pattern. `ir::lower` turns a `Pattern` into a `Program` of comparison
  blocks plus a `main` block, and its output always passes validation.
  `ir::render` turns a `Program` back into canonical pattern text.
  `Program::validate` checks well-formedness, including that each value is used
  at most once and each comparison block has at most one `Observe`, which keeps
  rendered output linear in the size of the program. `validate` also rejects
  `REPEATS` counts and path indices above `i64::MAX`, which the parser cannot
  read back. `render`, `validate` and `span_of` run in linear time.
  `Program::to_listing` prints a human-readable listing, escaping control
  characters so one instruction stays on one line, and `Program::span_of`
  computes the source extent of an instruction that carries no span of its own.
  There is no depth limit: `render`
  handles arbitrarily deep programs without overflowing the stack.
- `stix_ffi::Pattern::ir_json`, `ir_listing` and `canonical`, exposing the IR as
  compact JSON, as the human-readable listing, and as canonical pattern text.
- In the wasm binding, `Pattern.ir`, `Pattern.irListing` and `Pattern.canonical`
  getters over those accessors. The Python, Java and Node bindings do not have
  them yet.
- A pattern playground that parses a pattern in the browser with the wasm build
  and shows its AST, IR listing, IR JSON, IR graph and canonical text:
  <https://benjamin-small.github.io/stix-rust/playground/>.

### Fixed

- A pattern nested too deeply (parenthesized groups plus stacked qualifiers) used
  to crash the process with a stack overflow. It is now rejected with "pattern
  nests too deeply" once it exceeds `MAX_NESTING`. The parser is now iterative,
  long flat chains no longer overflow anywhere, and the matcher no longer recurses
  per object type
  ([#52](https://github.com/benjamin-small/stix-rust/issues/52)).
- `Pattern.ast` in the Python, Node and wasm bindings no longer fails with
  "recursion limit exceeded" for deeply nested or long patterns
  ([#50](https://github.com/benjamin-small/stix-rust/issues/50)).
- The pattern lexer accepts `<>` as a spelling of not-equal, as the STIX 2.1
  grammar allows
  ([#29](https://github.com/benjamin-small/stix-rust/issues/29)).
- String literals are decoded as UTF-8. Previously non-ASCII text such as
  `'café'` was mangled, because each byte was decoded as Latin-1
  ([#30](https://github.com/benjamin-small/stix-rust/issues/30)).
- Float literals too large to represent are rejected with "float literal out of
  range" instead of parsing to infinity
  ([#31](https://github.com/benjamin-small/stix-rust/issues/31)).

<!--
  Deliberately no "Removed" or "BREAKING" entries for ir::MAX_DEPTH,
  IrError::TooDeep or IrError::BlockObservedTwice: the ir module is unreleased, so
  none of these APIs ever shipped (MAX_DEPTH and TooDeep were added and then
  removed after v0.1.1). Do not "fix" this.
-->
