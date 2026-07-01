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
