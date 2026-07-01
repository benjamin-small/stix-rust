import stix


def test_exception_hierarchy():
    for name in ("ParseError", "ModelError", "MatchError", "ValidationError"):
        cls = getattr(stix, name)
        assert issubclass(cls, stix.StixError)
    assert issubclass(stix.StixError, Exception)


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
