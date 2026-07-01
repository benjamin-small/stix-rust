package io.github.benjaminsmall.stix;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import org.junit.jupiter.api.Test;

class StixTest {
    private static final String BUNDLE = "{\"type\":\"bundle\",\"id\":\"bundle--1\","
        + "\"objects\":["
        + "{\"type\":\"ipv4-addr\",\"id\":\"ipv4-addr--1\",\"value\":\"198.51.100.5\"},"
        + "{\"type\":\"observed-data\",\"id\":\"observed-data--1\","
        + "\"first_observed\":\"2020-01-01T00:00:00Z\",\"last_observed\":\"2020-01-01T00:00:00Z\","
        + "\"number_observed\":1,\"object_refs\":[\"ipv4-addr--1\"]}]}";

    @Test
    void parsesPatternToAstMap() {
        try (Engine engine = new Engine();
             Pattern pattern = engine.parsePattern("[ipv4-addr:value = '198.51.100.5']")) {
            Map<String, Object> ast = pattern.ast();
            assertTrue(ast.toString().contains("ipv4-addr"));
        }
    }

    @Test
    void readsAndIteratesBundle() {
        try (Engine engine = new Engine();
             Bundle bundle = engine.parseBundle(BUNDLE)) {
            assertEquals(2, bundle.objectCount());
            assertEquals("ipv4-addr--1", bundle.object(0).orElseThrow().get("id"));
            assertTrue(bundle.object(99).isEmpty());
            List<Object> types = new ArrayList<>();
            for (Map<String, Object> o : bundle) {
                types.add(o.get("type"));
            }
            assertTrue(types.contains("observed-data"));
        }
    }

    @Test
    void matchesHitAndMiss() {
        try (Engine engine = new Engine();
             Bundle bundle = engine.parseBundle(BUNDLE)) {
            try (Pattern hit = engine.parsePattern("[ipv4-addr:value = '198.51.100.5']")) {
                MatchResult r = engine.matchBundle(hit, bundle);
                assertTrue(r.matched());
                assertFalse(r.observations().isEmpty());
            }
            try (Pattern miss = engine.parsePattern("[ipv4-addr:value = '203.0.113.9']")) {
                assertFalse(engine.matchBundle(miss, bundle).matched());
            }
        }
    }

    @Test
    void appliesCustomHookAndMatchesComputedProperty() {
        try (Engine engine = new Engine()) {
            engine.registerType("x-acme-widget", obj -> {
                long score = ((Number) obj.getOrDefault("risk_score", 0)).longValue();
                obj.put("risk_band", score > 80 ? "high" : "low");
                return obj;
            });
            String json = "{\"type\":\"bundle\",\"objects\":["
                + "{\"type\":\"x-acme-widget\",\"id\":\"x-acme-widget--1\",\"risk_score\":90},"
                + "{\"type\":\"observed-data\",\"id\":\"observed-data--1\","
                + "\"first_observed\":\"2020-01-01T00:00:00Z\",\"last_observed\":\"2020-01-01T00:00:00Z\","
                + "\"number_observed\":1,\"object_refs\":[\"x-acme-widget--1\"]}]}";
            try (Bundle bundle = engine.parseBundle(json);
                 Pattern pattern = engine.parsePattern("[x-acme-widget:risk_band = 'high']")) {
                assertTrue(engine.matchBundle(pattern, bundle).matched());
            }
        }
    }

    @Test
    void mapsErrorsToExceptionHierarchy() {
        try (Engine engine = new Engine()) {
            assertThrows(ParseException.class, () -> engine.parsePattern("[bad"));
            assertThrows(ModelException.class,
                () -> engine.parseBundle("{\"type\":\"ipv4-addr\",\"id\":\"x--1\"}"));
            engine.registerType("x-thing", obj -> { throw new RuntimeException("nope"); });
            assertThrows(ValidationException.class, () -> engine.parseBundle(
                "{\"type\":\"bundle\",\"objects\":[{\"type\":\"x-thing\",\"id\":\"x--1\"}]}"));
            StixException ex = assertThrows(StixException.class,
                () -> engine.parsePattern("[bad"));
            assertInstanceOf(StixException.class, ex);
        }
    }
}
