# Python Binding (SP1) — Design

**Date:** 2026-06-29
**Status:** Approved (brainstorming complete; pending spec review)
**Scope:** SP1 of the language-bindings effort — the Python binding wrapping the
`stix-ffi` facade with PyO3 + maturin.
**Owner agent:** `python-binding` (lives under `bindings/python/`).

## Purpose

Give Python users the toolkit: parse STIX 2.1 patterns, import STIX bundles, match
patterns against observations, and register custom object types — all through an
idiomatic Python API backed by the `stix-ffi` facade. The "deep structure" half of
the hybrid surface (pattern AST, object JSON) is exposed as **native Python
dict/list**, not strings.

## Decisions (settled in brainstorming)

- **Deep structure → native dict/list** via `pythonize`/`depythonize` (not JSON
  strings).
- **Exception hierarchy:** `StixError` base + `ParseError`, `ModelError`,
  `MatchError`, `ValidationError`, mapped from `FfiError.code`.
- **Type stubs + `py.typed`** shipped (first-class typing).
- **Module import name `stix`**; distribution name `stix-rust` (not yet published).
- **Out of the root Cargo workspace** (`[workspace] exclude`) so the core
  `cargo test` stays Python-free and clean; the binding builds independently and
  path-depends on `stix-ffi`.

## Architecture

A PyO3 extension crate at `bindings/python/`, built with maturin.

```
bindings/python/
├── Cargo.toml          # crate-type = ["cdylib"]; pyo3 (abi3) + pythonize; dep on stix-ffi (path)
├── pyproject.toml      # [build-system] maturin; project metadata
├── src/
│   ├── lib.rs          # #[pymodule] stix: registers classes + exceptions
│   ├── errors.rs       # exception types + FfiError -> PyErr mapping
│   ├── engine.rs       # Engine pyclass
│   └── handles.rs      # Pattern, Bundle, MatchResult pyclasses
├── stix.pyi            # type stubs
├── py.typed            # PEP 561 marker
└── tests/
    └── test_stix.py    # pytest suite
```

The root `Cargo.toml` gains `exclude = ["bindings/python"]` under `[workspace]`.
`bindings/python/Cargo.toml` path-depends on `../../crates/stix-ffi`. Because it is
excluded, `cargo test` at the repo root is unaffected (no libpython link).

## Components

### Exceptions (`errors.rs`)

A base `StixError(Exception)` and four subclasses, created with PyO3's
`create_exception!`:

- `StixError` — base.
- `ParseError`, `ModelError`, `MatchError`, `ValidationError` — subclasses.

`FfiError -> PyErr` maps `ErrorCode::{Parse, Model, Match, Validation}` to the
matching subclass, preserving the message. All four subclasses are exported from the
module so users can `except stix.ParseError:`.

### `Engine` (`engine.rs`, `#[pyclass]`)

- `Engine()` — construct (wraps `stix_ffi::Engine`).
- `parse_pattern(self, src: str) -> Pattern`
- `parse_bundle(self, json: str) -> Bundle`
- `match_bundle(self, pattern: Pattern, bundle: Bundle) -> MatchResult`
- `register_type(self, type_name: str, hook: Callable[[dict], dict]) -> None`

`register_type` stores the Python callable (`Py<PyAny>`) and installs a facade hook:
on each object of `type_name` during `parse_bundle`, the hook
1. converts the object `serde_json::Value` → Python `dict` (`pythonize`),
2. calls the Python callable **holding the GIL** (safe: hooks run synchronously,
   in-call-stack, at parse time only),
3. on success, converts the returned `dict` → `Value` (`depythonize`) and returns
   `Ok(value)`;
4. on a raised Python exception, returns `Err(message)` → surfaces as
   `ValidationError` from `parse_bundle`.

Because `Engine::register_type` needs `&mut self`, the pyclass holds the
`stix_ffi::Engine` in a way that allows mutation (e.g. the pyclass is `#[pyclass]`
with `&mut self` methods, which PyO3 supports via its interior borrow checking).

### `Pattern` (`handles.rs`, `#[pyclass]`)

- `ast` (getter) `-> dict` — the AST: `serde_json::from_str(handle.to_json())` then
  `pythonize`. (Computed on access; patterns are small.)
- `__repr__` → `"Pattern(...)"`.

### `Bundle` (`handles.rs`, `#[pyclass]`)

- `object_count(self) -> int` and `__len__`.
- `object(self, index: int) -> Optional[dict]` — `object_json(index)` → parse →
  `pythonize`; `None` if out of range.
- `__iter__` / `__getitem__` for Pythonic iteration over objects (each a dict).

### `MatchResult` (`handles.rs`, `#[pyclass]`)

- `matched` (getter) `-> bool`.
- `observations` (getter) `-> list[int]` (from `MatchOutcome.observations`).
- `__bool__` returns `matched`.

### Module (`lib.rs`)

`#[pymodule] fn stix(m)` registers `Engine`, `Pattern`, `Bundle`, `MatchResult`, and
the five exception types.

### Type stubs (`stix.pyi`, `py.typed`)

Hand-written stubs mirroring the classes/methods above with precise types
(`Callable[[dict], dict]`, `Optional[dict]`, `list[int]`), plus the exception
classes. `py.typed` marks the package PEP 561-compliant. maturin includes both in
the wheel.

## Data Flow

```
str ──Engine.parse_pattern──► Pattern ──.ast──► dict (AST)
str ──Engine.parse_bundle───► Bundle  ──.object(i)/iter──► dict (object)
Pattern + Bundle ──Engine.match_bundle──► MatchResult{matched, observations}

custom type: Python Callable[[dict],dict]  ──register_type──►
   facade hook (pythonize/depythonize, GIL-held) ──invoked during parse_bundle──►
   raised exception → ValidationError
```

## Error Handling

- Every facade `FfiError` becomes the matching `Stix*Error` subclass (by `code`)
  with the message. No panics cross into Python.
- `Bundle.object(i)` out of range → `None` (not an exception).
- A `register_type` hook raising any Python exception → `ValidationError` from
  `parse_bundle`, message taken from the Python exception's `str()`.

## Testing

`pytest` suite (`tests/test_stix.py`), run after `maturin develop`:

- parse a pattern → `.ast` is a dict whose structure includes the object type.
- parse a bundle → `len()`, iterate objects (dicts), `object(out_of_range)` is `None`.
- match (hit) and non-match → `MatchResult.matched` / `bool(result)` / `.observations`.
- custom type: `register_type` with a hook adding a computed property; a pattern
  matches that property end-to-end.
- error mapping: a bad pattern raises `ParseError`; a non-bundle raises `ModelError`;
  a hook that raises → `ValidationError`; all are subclasses of `StixError`.

The `python-binding` agent runs `maturin develop && pytest` locally for verification.

## Out of Scope

- Publishing to PyPI / building release wheels in CI (added later, per binding).
- `match_scos` / `match_observed_data` entry points (facade exposes only
  `match_bundle` for now; add when needed).
- Async APIs.
- The other bindings (Java, TS) — separate specs.

## Future Considerations

- When `stix-ffi` adds more entry points, expose them as `Engine` methods.
- A PyPI release workflow (cibuildwheel/maturin-action) belongs to a follow-up.
- If typed access to custom objects is wanted in Python, it maps onto the core's
  `StixObject::downcast_ref` via a future facade method.
