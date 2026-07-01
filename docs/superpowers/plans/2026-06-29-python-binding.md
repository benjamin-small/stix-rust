# Python Binding (SP1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Python binding under `bindings/python/` — a PyO3 + maturin extension that wraps the `stix-ffi` facade with an idiomatic Python API (`Engine`/`Pattern`/`Bundle`/`MatchResult`, native dict/list deep structure, a `StixError` exception hierarchy, custom-type hooks, type stubs).

**Architecture:** A PyO3 cdylib crate excluded from the root Cargo workspace, built with maturin in a "mixed" layout (compiled module `stix._stix`, re-exported by a thin `stix` Python package that also carries the type stubs + `py.typed`). It path-depends on `stix-ffi` and converts deep structure to/from native Python via `pythonize`.

**Tech Stack:** Rust + PyO3 0.22 (`extension-module`, `abi3-py38`), `pythonize` 0.22, `serde_json`; maturin build backend; pytest.

---

## ⚠️ Version-sensitivity note for the implementing agent

This plan targets **`pyo3 = "0.22"`** and **`pythonize = "0.22"`** (pinned in
`Cargo.toml`). PyO3's `Bound`/GIL API and pythonize's function names differ between
versions (e.g. `get_type` vs `get_type_bound`, `depythonize` vs
`depythonize_bound`, `Python::with_gil` ergonomics). **Treat the compiler and pytest
as the source of truth:** keep the pinned versions, and where an API call doesn't
compile, adjust it to the pinned version's form while preserving the exact
Python-facing behavior and signatures specified here. Use WebSearch/WebFetch to
confirm the pinned versions' API if a call is ambiguous. This is the one area where
adapting the literal code to compile reality is expected.

Prerequisite tooling (install if missing): `pip install "maturin>=1.5,<2.0" pytest`.

## File Structure

```
bindings/python/
├── Cargo.toml              # [lib] name="_stix" cdylib; pyo3+pythonize+stix-ffi(path)
├── pyproject.toml          # maturin backend; module-name = "stix._stix"; python-source
├── python/stix/
│   ├── __init__.py         # re-export the compiled symbols
│   ├── __init__.pyi        # type stubs
│   └── py.typed            # PEP 561 marker
├── src/
│   ├── lib.rs              # #[pymodule] _stix: register classes + exceptions
│   ├── errors.rs           # exception types + FfiError -> PyErr
│   ├── handles.rs          # Pattern, Bundle, MatchResult pyclasses
│   └── engine.rs           # Engine pyclass
├── tests/test_stix.py      # pytest suite
└── README.md               # replaces the planned-placeholder
```
Root `Cargo.toml` gains `exclude = ["bindings/python"]`.

---

## Task 1: Scaffold the extension + mixed-layout package

**Files:**
- Modify: `Cargo.toml` (root)
- Create: `bindings/python/Cargo.toml`
- Create: `bindings/python/pyproject.toml`
- Create: `bindings/python/python/stix/__init__.py`
- Create: `bindings/python/python/stix/py.typed`
- Create: `bindings/python/src/lib.rs`

- [ ] **Step 1: Exclude the binding from the root workspace**

In the root `Cargo.toml`, under `[workspace]`, add an `exclude` key (keep `members` as-is):

```toml
[workspace]
resolver = "2"
members = ["crates/stix-pattern", "crates/stix-model", "crates/stix-matcher", "crates/stix", "crates/stix-ffi"]
exclude = ["bindings/python"]
```

- [ ] **Step 2: Create the crate manifest**

Create `bindings/python/Cargo.toml`:

```toml
[package]
name = "stix-python"
version = "0.0.1"
edition = "2021"
license = "MIT OR Apache-2.0"
repository = "https://github.com/benjamin-small/stix-rust"
description = "Python bindings for the stix-rust toolkit."

[lib]
name = "_stix"
crate-type = ["cdylib"]

[dependencies]
stix-ffi = { path = "../../crates/stix-ffi" }
pyo3 = { version = "0.22", features = ["extension-module", "abi3-py38"] }
pythonize = "0.22"
serde_json = "1"
```

- [ ] **Step 3: Create the maturin project file**

Create `bindings/python/pyproject.toml`:

```toml
[build-system]
requires = ["maturin>=1.5,<2.0"]
build-backend = "maturin"

[project]
name = "stix-rust"
requires-python = ">=3.8"
description = "Python bindings for the stix-rust toolkit (parse, import, match STIX 2.1)."
license = { text = "MIT OR Apache-2.0" }
dynamic = ["version"]

[tool.maturin]
module-name = "stix._stix"
python-source = "python"
features = ["pyo3/extension-module"]
```

- [ ] **Step 4: Create the Python package wrapper**

Create `bindings/python/python/stix/__init__.py`:

```python
"""Python bindings for the stix-rust toolkit."""
from ._stix import (
    Engine,
    Pattern,
    Bundle,
    MatchResult,
    StixError,
    ParseError,
    ModelError,
    MatchError,
    ValidationError,
)

__all__ = [
    "Engine",
    "Pattern",
    "Bundle",
    "MatchResult",
    "StixError",
    "ParseError",
    "ModelError",
    "MatchError",
    "ValidationError",
]
```

Create `bindings/python/python/stix/py.typed` (empty file):

```
```

- [ ] **Step 5: Create a minimal compiled module**

Create `bindings/python/src/lib.rs`:

```rust
//! Python bindings for the stix-rust toolkit (compiled module `stix._stix`).
use pyo3::prelude::*;

#[pymodule]
fn _stix(_py: Python<'_>, _m: &Bound<'_, PyModule>) -> PyResult<()> {
    Ok(())
}
```

> Note: this minimal module exports nothing yet, so `__init__.py`'s imports will
> fail until Tasks 2–4 add the symbols. Verify the *build* here; full `import stix`
> succeeds after Task 4. (If you want an importable checkpoint now, temporarily
> comment the `__init__.py` imports and restore them in Task 4 — optional.)

- [ ] **Step 6: Verify the extension builds**

Run: `cd bindings/python && maturin build`
Expected: builds a wheel with no Rust errors (the `_stix` module compiles and links
against `stix-ffi`).

- [ ] **Step 7: Verify the core workspace is unaffected**

Run (from repo root): `cargo test 2>&1 | grep -c "test result: ok"`
Expected: a non-zero count — the root workspace excludes `bindings/python`, so it
still builds/test without a Python toolchain.

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml bindings/python/
git commit -m "feat(python): scaffold PyO3 extension and maturin mixed-layout package"
```

---

## Task 2: Exception hierarchy

**Files:**
- Create: `bindings/python/src/errors.rs`
- Modify: `bindings/python/src/lib.rs`
- Create: `bindings/python/tests/test_stix.py`

- [ ] **Step 1: Write the failing test**

Create `bindings/python/tests/test_stix.py`:

```python
import stix


def test_exception_hierarchy():
    for name in ("ParseError", "ModelError", "MatchError", "ValidationError"):
        cls = getattr(stix, name)
        assert issubclass(cls, stix.StixError)
    assert issubclass(stix.StixError, Exception)
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd bindings/python && maturin develop && python -m pytest tests/test_stix.py -q`
Expected: FAIL — `import stix` fails (symbols not yet exported) or attributes missing.

- [ ] **Step 3: Implement the exceptions**

Create `bindings/python/src/errors.rs`:

```rust
//! Python exception types and the FfiError -> PyErr mapping.
use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use stix_ffi::{ErrorCode, FfiError};

create_exception!(_stix, StixError, PyException, "Base class for all stix errors.");
create_exception!(_stix, ParseError, StixError, "Pattern failed to parse.");
create_exception!(_stix, ModelError, StixError, "Object/bundle import failed.");
create_exception!(_stix, MatchError, StixError, "Matching failed.");
create_exception!(_stix, ValidationError, StixError, "A custom-type hook rejected an object.");

/// Convert a facade error into the matching Python exception.
pub fn to_pyerr(err: FfiError) -> PyErr {
    match err.code {
        ErrorCode::Parse => ParseError::new_err(err.message),
        ErrorCode::Model => ModelError::new_err(err.message),
        ErrorCode::Match => MatchError::new_err(err.message),
        ErrorCode::Validation => ValidationError::new_err(err.message),
    }
}

/// Register the exception types on the module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("StixError", m.py().get_type_bound::<StixError>())?;
    m.add("ParseError", m.py().get_type_bound::<ParseError>())?;
    m.add("ModelError", m.py().get_type_bound::<ModelError>())?;
    m.add("MatchError", m.py().get_type_bound::<MatchError>())?;
    m.add("ValidationError", m.py().get_type_bound::<ValidationError>())?;
    Ok(())
}
```

In `bindings/python/src/lib.rs`, wire the module to register the exceptions:

```rust
//! Python bindings for the stix-rust toolkit (compiled module `stix._stix`).
use pyo3::prelude::*;

mod errors;

#[pymodule]
fn _stix(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    errors::register(m)?;
    Ok(())
}
```

> `__init__.py` also imports `Engine`/`Pattern`/`Bundle`/`MatchResult`, which don't
> exist yet — for this task's pytest to import `stix`, temporarily reduce
> `__init__.py` to import only the five exception names, and restore the full import
> list in Task 4 Step 3. (Or implement Tasks 3–4 before running pytest.)

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd bindings/python && maturin develop && python -m pytest tests/test_stix.py::test_exception_hierarchy -q`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add bindings/python/src/errors.rs bindings/python/src/lib.rs bindings/python/python/stix/__init__.py bindings/python/tests/test_stix.py
git commit -m "feat(python): add StixError exception hierarchy and FfiError mapping"
```

---

## Task 3: Engine + handles (parse + match)

**Files:**
- Create: `bindings/python/src/handles.rs`
- Create: `bindings/python/src/engine.rs`
- Modify: `bindings/python/src/lib.rs`
- Modify: `bindings/python/python/stix/__init__.py`
- Modify: `bindings/python/tests/test_stix.py`

- [ ] **Step 1: Write the failing tests**

Append to `bindings/python/tests/test_stix.py`:

```python
import pytest

BUNDLE = """{"type":"bundle","id":"bundle--1","objects":[
  {"type":"ipv4-addr","id":"ipv4-addr--1","value":"198.51.100.5"},
  {"type":"observed-data","id":"observed-data--1",
   "first_observed":"2020-01-01T00:00:00Z","last_observed":"2020-01-01T00:00:00Z",
   "number_observed":1,"object_refs":["ipv4-addr--1"]}
]}"""


def test_parse_pattern_ast_is_dict():
    engine = stix.Engine()
    pattern = engine.parse_pattern("[ipv4-addr:value = '198.51.100.5']")
    ast = pattern.ast
    assert isinstance(ast, dict)
    # the AST mentions the object type somewhere in its nested structure
    assert "ipv4-addr" in repr(ast)


def test_bundle_access():
    engine = stix.Engine()
    bundle = engine.parse_bundle(BUNDLE)
    assert len(bundle) == 2
    first = bundle.object(0)
    assert isinstance(first, dict)
    assert first["id"] == "ipv4-addr--1"
    assert bundle.object(99) is None
    types = [o["type"] for o in bundle]
    assert "observed-data" in types


def test_match_hit_and_miss():
    engine = stix.Engine()
    bundle = engine.parse_bundle(BUNDLE)
    hit = engine.parse_pattern("[ipv4-addr:value = '198.51.100.5']")
    result = engine.match_bundle(hit, bundle)
    assert result.matched is True
    assert bool(result) is True
    assert isinstance(result.observations, list)

    miss = engine.parse_pattern("[ipv4-addr:value = '203.0.113.9']")
    assert engine.match_bundle(miss, bundle).matched is False


def test_parse_errors_map_to_exceptions():
    engine = stix.Engine()
    with pytest.raises(stix.ParseError):
        engine.parse_pattern("[bad")
    with pytest.raises(stix.ModelError):
        engine.parse_bundle('{"type":"ipv4-addr","id":"x--1"}')
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd bindings/python && maturin develop && python -m pytest tests/test_stix.py -q`
Expected: FAIL — `stix.Engine` does not exist.

- [ ] **Step 3: Implement the handles**

Create `bindings/python/src/handles.rs`:

```rust
//! Pyclass handles: Pattern, Bundle, MatchResult.
use pyo3::prelude::*;
use pyo3::types::PyList;

use crate::errors::to_pyerr;

/// Parse a JSON string into a native Python object (dict/list/...).
fn json_to_py(py: Python<'_>, json: &str) -> PyResult<PyObject> {
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| crate::errors::ParseError::new_err(e.to_string()))?;
    let obj = pythonize::pythonize(py, &value)
        .map_err(|e| crate::errors::ModelError::new_err(e.to_string()))?;
    Ok(obj.into())
}

#[pyclass]
pub struct Pattern {
    pub(crate) inner: stix_ffi::Pattern,
}

#[pymethods]
impl Pattern {
    /// The pattern's AST as a native Python dict.
    #[getter]
    fn ast(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_to_py(py, &self.inner.to_json())
    }

    fn __repr__(&self) -> String {
        "Pattern(...)".to_string()
    }
}

#[pyclass]
pub struct Bundle {
    pub(crate) inner: stix_ffi::Bundle,
}

#[pymethods]
impl Bundle {
    /// Number of objects in the bundle.
    fn object_count(&self) -> usize {
        self.inner.object_count()
    }

    fn __len__(&self) -> usize {
        self.inner.object_count()
    }

    /// The object at `index` as a native Python dict, or None if out of range.
    fn object(&self, py: Python<'_>, index: usize) -> PyResult<Option<PyObject>> {
        match self.inner.object_json(index) {
            Some(json) => Ok(Some(json_to_py(py, &json)?)),
            None => Ok(None),
        }
    }

    /// Iterate over the objects (each a dict).
    fn __iter__(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let mut items: Vec<PyObject> = Vec::with_capacity(slf.inner.object_count());
        for i in 0..slf.inner.object_count() {
            if let Some(json) = slf.inner.object_json(i) {
                items.push(json_to_py(py, &json)?);
            }
        }
        let list = PyList::new_bound(py, items);
        Ok(list.as_any().iter()?.into())
    }
}

#[pyclass]
pub struct MatchResult {
    pub(crate) matched: bool,
    pub(crate) observations: Vec<u64>,
}

#[pymethods]
impl MatchResult {
    #[getter]
    fn matched(&self) -> bool {
        self.matched
    }

    #[getter]
    fn observations(&self) -> Vec<u64> {
        self.observations.clone()
    }

    fn __bool__(&self) -> bool {
        self.matched
    }
}

// Re-export the mapping helper for engine.rs.
pub(crate) use to_pyerr as _to_pyerr;
```

- [ ] **Step 4: Implement the Engine (parse + match)**

Create `bindings/python/src/engine.rs`:

```rust
//! The Engine pyclass: parse patterns/bundles and run matches.
use pyo3::prelude::*;

use crate::errors::to_pyerr;
use crate::handles::{Bundle, MatchResult, Pattern};

#[pyclass]
pub struct Engine {
    inner: stix_ffi::Engine,
}

#[pymethods]
impl Engine {
    #[new]
    fn new() -> Self {
        Engine {
            inner: stix_ffi::Engine::new(),
        }
    }

    /// Parse a STIX pattern string into a Pattern handle.
    fn parse_pattern(&self, src: &str) -> PyResult<Pattern> {
        let inner = self.inner.parse_pattern(src).map_err(to_pyerr)?;
        Ok(Pattern { inner })
    }

    /// Parse a STIX bundle JSON string into a Bundle handle.
    fn parse_bundle(&self, json: &str) -> PyResult<Bundle> {
        let inner = self.inner.parse_bundle(json).map_err(to_pyerr)?;
        Ok(Bundle { inner })
    }

    /// Match a pattern against a bundle.
    fn match_bundle(&self, pattern: &Pattern, bundle: &Bundle) -> PyResult<MatchResult> {
        let outcome = self
            .inner
            .match_bundle(&pattern.inner, &bundle.inner)
            .map_err(to_pyerr)?;
        Ok(MatchResult {
            matched: outcome.matched,
            observations: outcome.observations,
        })
    }
}
```

In `bindings/python/src/lib.rs`, register the classes:

```rust
//! Python bindings for the stix-rust toolkit (compiled module `stix._stix`).
use pyo3::prelude::*;

mod engine;
mod errors;
mod handles;

#[pymodule]
fn _stix(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    errors::register(m)?;
    m.add_class::<engine::Engine>()?;
    m.add_class::<handles::Pattern>()?;
    m.add_class::<handles::Bundle>()?;
    m.add_class::<handles::MatchResult>()?;
    Ok(())
}
```

Ensure `bindings/python/python/stix/__init__.py` imports the full set (restore if you
trimmed it in Task 2) — it should match the Task 1 Step 4 content exactly.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd bindings/python && maturin develop && python -m pytest tests/test_stix.py -q`
Expected: PASS — exception, pattern-AST, bundle-access, match, and error-mapping
tests all pass.

- [ ] **Step 6: Commit**

```bash
git add bindings/python/src/ bindings/python/python/stix/__init__.py bindings/python/tests/test_stix.py
git commit -m "feat(python): add Engine, Pattern, Bundle, MatchResult pyclasses"
```

---

## Task 4: register_type custom-model hook

**Files:**
- Modify: `bindings/python/src/engine.rs`
- Modify: `bindings/python/tests/test_stix.py`

- [ ] **Step 1: Write the failing tests**

Append to `bindings/python/tests/test_stix.py`:

```python
CUSTOM_BUNDLE = """{"type":"bundle","id":"bundle--1","objects":[
  {"type":"x-acme-widget","id":"x-acme-widget--1","risk_score":90},
  {"type":"observed-data","id":"observed-data--1",
   "first_observed":"2020-01-01T00:00:00Z","last_observed":"2020-01-01T00:00:00Z",
   "number_observed":1,"object_refs":["x-acme-widget--1"]}
]}"""


def test_register_type_computed_property_matches():
    engine = stix.Engine()

    def normalize(obj):
        obj["risk_band"] = "high" if obj.get("risk_score", 0) > 80 else "low"
        return obj

    engine.register_type("x-acme-widget", normalize)
    bundle = engine.parse_bundle(CUSTOM_BUNDLE)
    pattern = engine.parse_pattern("[x-acme-widget:risk_band = 'high']")
    assert engine.match_bundle(pattern, bundle).matched is True


def test_register_type_rejection_raises_validation_error():
    engine = stix.Engine()

    def require_score(obj):
        if "risk_score" not in obj:
            raise ValueError("missing risk_score")
        return obj

    engine.register_type("x-acme-widget", require_score)
    with pytest.raises(stix.ValidationError):
        engine.parse_bundle(
            '{"type":"bundle","objects":[{"type":"x-acme-widget","id":"x--1"}]}'
        )
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd bindings/python && maturin develop && python -m pytest tests/test_stix.py -q`
Expected: FAIL — `Engine.register_type` does not exist.

- [ ] **Step 3: Implement register_type**

In `bindings/python/src/engine.rs`, add `use pyo3::types::PyAnyMethods;` if needed
for `.call1`, and add this method inside `#[pymethods] impl Engine` (after
`match_bundle`). It bridges a Python callable into the facade's data-level hook:

```rust
    /// Register a custom object type. `hook` is a callable `dict -> dict`: it may
    /// mutate/return the object (adding computed properties) or raise to reject it
    /// (surfaced as ValidationError from parse_bundle). Runs only at parse time.
    fn register_type(&mut self, type_name: &str, hook: Py<PyAny>) {
        self.inner.register_type(
            type_name,
            Box::new(move |value: serde_json::Value| {
                Python::with_gil(|py| {
                    // Value -> Python dict
                    let arg = pythonize::pythonize(py, &value).map_err(|e| e.to_string())?;
                    // call the Python hook
                    let result = hook
                        .call1(py, (arg,))
                        .map_err(|e| e.value_bound(py).to_string())?;
                    // returned dict -> Value
                    let out: serde_json::Value =
                        pythonize::depythonize(result.bind(py)).map_err(|e| e.to_string())?;
                    Ok(out)
                })
            }),
        );
    }
```

> Version-sensitive spots (adapt to the pinned pyo3/pythonize if they don't compile,
> preserving behavior): `e.value_bound(py)` (PyErr → message), `result.bind(py)`
> (Py<PyAny> → &Bound), and `pythonize`/`depythonize` argument forms. The behavior to
> preserve: convert the object to a dict, call the hook, convert the result back; a
> raised Python exception becomes `Err(<its message>)`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd bindings/python && maturin develop && python -m pytest tests/test_stix.py -q`
Expected: PASS — the whole suite, including the computed-property match and the
ValidationError rejection.

- [ ] **Step 5: Commit**

```bash
git add bindings/python/src/engine.rs bindings/python/tests/test_stix.py
git commit -m "feat(python): add Engine.register_type bridging a Python hook"
```

---

## Task 5: Type stubs

**Files:**
- Create: `bindings/python/python/stix/__init__.pyi`

- [ ] **Step 1: Write the stubs**

Create `bindings/python/python/stix/__init__.pyi`:

```python
from typing import Any, Callable, Iterator, Optional

class StixError(Exception): ...
class ParseError(StixError): ...
class ModelError(StixError): ...
class MatchError(StixError): ...
class ValidationError(StixError): ...

class Pattern:
    @property
    def ast(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...

class Bundle:
    def object_count(self) -> int: ...
    def __len__(self) -> int: ...
    def object(self, index: int) -> Optional[dict[str, Any]]: ...
    def __iter__(self) -> Iterator[dict[str, Any]]: ...

class MatchResult:
    @property
    def matched(self) -> bool: ...
    @property
    def observations(self) -> list[int]: ...
    def __bool__(self) -> bool: ...

class Engine:
    def __init__(self) -> None: ...
    def parse_pattern(self, src: str) -> Pattern: ...
    def parse_bundle(self, json: str) -> Bundle: ...
    def match_bundle(self, pattern: Pattern, bundle: Bundle) -> MatchResult: ...
    def register_type(
        self, type_name: str, hook: Callable[[dict[str, Any]], dict[str, Any]]
    ) -> None: ...
```

- [ ] **Step 2: Verify stubs ship and the package still imports**

Run: `cd bindings/python && maturin develop && python -c "import stix; assert stix.Engine"`
Expected: no error. Confirm the stub + marker are present:
`ls python/stix/__init__.pyi python/stix/py.typed`
Expected: both listed.

- [ ] **Step 3: Commit**

```bash
git add bindings/python/python/stix/__init__.pyi
git commit -m "docs(python): add type stubs for the stix package"
```

---

## Task 6: README + final verification

**Files:**
- Modify: `bindings/python/README.md`

- [ ] **Step 1: Replace the planned-placeholder README**

Overwrite `bindings/python/README.md`:

```markdown
# stix-rust — Python binding

Python bindings for the [stix-rust](../../README.md) toolkit — parse STIX 2.1
patterns, import STIX bundles, match patterns against observations, and register
custom object types, from Python.

- **Toolchain:** PyO3 + maturin. Module `stix` (compiled core `stix._stix`).
- **Surface:** typed handles (`Engine`, `Pattern`, `Bundle`, `MatchResult`); deep
  structure (the pattern AST, bundle objects) as native `dict`/`list`.

## Install (from source)

```bash
pip install "maturin>=1.5,<2.0"
cd bindings/python
maturin develop          # builds the extension into the active venv
```

## Usage

```python
import stix

engine = stix.Engine()
pattern = engine.parse_pattern("[ipv4-addr:value = '198.51.100.5']")
print(pattern.ast)                      # AST as a dict

bundle = engine.parse_bundle(open("bundle.json").read())
print(len(bundle), [o["type"] for o in bundle])

result = engine.match_bundle(pattern, bundle)
print(result.matched, result.observations)
```

### Custom object types

Register a `dict -> dict` hook; it runs at import time and may add computed
properties or raise to reject (surfaced as `stix.ValidationError`):

```python
def normalize(obj):
    obj["risk_band"] = "high" if obj.get("risk_score", 0) > 80 else "low"
    return obj

engine.register_type("x-acme-widget", normalize)
```

### Errors

`stix.StixError` is the base; `ParseError`, `ModelError`, `MatchError`, and
`ValidationError` are subclasses.

## Test

```bash
cd bindings/python
maturin develop
python -m pytest -q
```
```

- [ ] **Step 2: Final binding verification**

Run: `cd bindings/python && maturin develop && python -m pytest -q`
Expected: all pytest tests pass.

- [ ] **Step 3: Rust lint/format on the binding crate**

Run: `cd bindings/python && cargo fmt && cargo clippy -- -D warnings`
Expected: clippy clean (the binding crate is standalone; this is separate from the
workspace clippy).

- [ ] **Step 4: Confirm the core workspace is still green**

Run (repo root): `cargo test 2>&1 | grep -c "test result: ok"`
Expected: non-zero, unchanged — the excluded binding doesn't affect the workspace.

- [ ] **Step 5: Commit**

```bash
git add bindings/python/README.md bindings/python/src
git commit -m "docs(python): real README; fmt + clippy clean"
```

---

## Self-Review Notes (already applied)

- **Spec coverage:** mixed-layout scaffold + workspace exclusion (Task 1); exception
  hierarchy + FfiError mapping (Task 2); `Engine` parse/match + `Pattern.ast` (dict)
  + `Bundle` len/iter/object + `MatchResult` matched/observations/`__bool__`
  (Task 3); `register_type` Python-hook bridge with `ValidationError` (Task 4); type
  stubs + `py.typed` (Tasks 1 & 5); README + verification (Task 6). Native dict/list
  via pythonize throughout. All spec sections map to a task.
- **Type consistency:** `Engine.{parse_pattern, parse_bundle, match_bundle,
  register_type}`, `Pattern.ast`, `Bundle.{object_count,__len__,object,__iter__}`,
  `MatchResult.{matched,observations,__bool__}`, and the `Stix*Error` names match
  across the Rust pyclasses, `__init__.py`, the stubs, and the tests. The Rust side
  wraps the verified `stix_ffi` API (`Engine`, `Pattern::to_json`,
  `Bundle::{object_count,object_json}`, `MatchOutcome::{matched,observations}`,
  `FfiError{code,message}`/`ErrorCode`).
- **Version note** at the top scopes the one area (PyO3/pythonize API drift) where
  the agent adapts code to the pinned versions by compiling — behavior/signatures
  are fully specified, so this is not an open placeholder.
- **No placeholders:** every step has complete file content or an exact command.
```
