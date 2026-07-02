# Python

The `stix` module (PyO3 + maturin). Deep structure — the pattern AST and bundle
objects — arrives as native `dict`/`list`. Ships type stubs (`py.typed`).

## Install

```bash
pip install stix-rust
```

From source instead:
`pip install "maturin>=1.5,<2.0" && cd bindings/python && maturin develop`

## Worked example

```python
import stix

engine = stix.Engine()

# 1. parse a pattern; the AST is a dict
pattern = engine.parse_pattern("[ipv4-addr:value = '198.51.100.5']")
print(pattern.ast["expression"])

# 2. import a bundle; iterate its objects
bundle = engine.parse_bundle(open("bundle.json").read())
print(len(bundle), [o["type"] for o in bundle])
print(bundle.object(0))          # dict, or None if out of range

# 3. match — hit and miss
result = engine.match_bundle(pattern, bundle)
print(result.matched, result.observations)
miss = engine.parse_pattern("[ipv4-addr:value = '203.0.113.9']")
assert not engine.match_bundle(miss, bundle).matched

# 4. custom type with a computed property
def normalize(obj):
    obj["risk_band"] = "high" if obj.get("risk_score", 0) > 80 else "low"
    return obj
engine.register_type("x-acme-widget", normalize)
banded = engine.parse_bundle(widget_bundle_json)
hit = engine.parse_pattern("[x-acme-widget:risk_band = 'high']")
assert engine.match_bundle(hit, banded).matched
```

## Errors

| Exception | Raised by |
| --- | --- |
| `stix.ParseError` | invalid pattern syntax |
| `stix.ModelError` | invalid JSON / not a bundle |
| `stix.MatchError` | matching failure (e.g. unsupported feature reached) |
| `stix.ValidationError` | a `register_type` hook raised |

All subclass `stix.StixError`. Hooks run at `parse_bundle` time; raising any
exception inside one rejects the object.

## Notes

- Type stubs + `py.typed` are included — IDEs and mypy see precise signatures.
- Requires Python ≥ 3.8 (abi3 wheel).
