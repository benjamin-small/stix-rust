import stix


def test_exception_hierarchy():
    for name in ("ParseError", "ModelError", "MatchError", "ValidationError"):
        cls = getattr(stix, name)
        assert issubclass(cls, stix.StixError)
    assert issubclass(stix.StixError, Exception)
