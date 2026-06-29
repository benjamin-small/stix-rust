# stix-rust — Python binding

> **Status: planned.** This area is scaffolded; the binding is not yet implemented.

Python bindings for the [stix-rust](../../README.md) toolkit — parse STIX 2.1
patterns, import STIX objects, and match patterns against observations, from Python.

- **Toolchain:** [PyO3](https://pyo3.rs) + [maturin](https://www.maturin.rs)
- **Surface:** typed handles (`Engine`, `Pattern`, `Bundle`, `MatchResult`) with JSON
  for deep structure (AST dumps, object properties), wrapping the `stix-ffi` facade.
- **Owner agent:** `python-binding`

## Planned usage

```python
import stix

engine = stix.Engine()
pattern = engine.parse_pattern("[ipv4-addr:value = '198.51.100.1']")
bundle = engine.parse_bundle(open("bundle.json").read())
result = engine.match_bundle(pattern, bundle)
assert result.matched
```

## Build & test (once implemented)

```bash
maturin develop      # build + install into the current venv
pytest               # run the Python test suite
```
