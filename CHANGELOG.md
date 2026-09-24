# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project adheres
to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Releases are tag-driven and cover the Rust crates and the Python, Java, Node and
wasm bindings together, so one entry describes the change everywhere it is
visible. See [RELEASING.md](RELEASING.md).

## [Unreleased]

### Changed

- **BREAKING:** `ObservationExpression::Observation` is now a struct variant.
  Its serialized JSON changes from `{"Observation": {…}}` to
  `{"Observation": {"expression": {…}, "span": {…}}}`, where the old payload is
  what now sits under `"expression"`. This is visible in every binding's
  `Pattern.ast`, so any consumer that reaches into that JSON needs updating.

### Added

- `span` fields recording source byte ranges on `Comparison`, `ObjectPath`, and
  the `Observation` and `Qualified` variants of `ObservationExpression`. Note
  that `Pattern.ast` payloads grow accordingly in every binding: about half again
  as large for a small pattern, measured as 174 → 273 bytes of JSON for
  `[file:size > 1024]`.
- `Pattern::without_spans()`, which zeroes every span, for comparing two ASTs
  that came from different source text.
- The `stix_pattern::ir` module: a three-address intermediate representation of a
  parsed pattern. `ir::lower` turns a `Pattern` into a `Program` of comparison
  blocks plus a `main` block, `ir::render` turns a `Program` back into canonical
  pattern text, `Program::validate` checks well-formedness, `Program::to_listing`
  prints a human-readable listing, and `Program::span_of` computes the source
  extent of an instruction that carries no span of its own.

### Known issues

- Rendering a pattern containing a non-ASCII string literal does not round-trip:
  the lexer decodes string literals as Latin-1
  ([#30](https://github.com/benjamin-small/stix-rust/issues/30)).
- The grammar's `<>` spelling of not-equal is not accepted by the lexer
  ([#29](https://github.com/benjamin-small/stix-rust/issues/29)).
