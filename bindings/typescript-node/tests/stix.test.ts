import { describe, it, expect } from "vitest";
import {
  Engine,
  StixError,
  ParseError,
  ModelError,
  ValidationError,
} from "../dist/index.js";

const BUNDLE = JSON.stringify({
  type: "bundle",
  id: "bundle--1",
  objects: [
    { type: "ipv4-addr", id: "ipv4-addr--1", value: "198.51.100.5" },
    {
      type: "observed-data", id: "observed-data--1",
      first_observed: "2020-01-01T00:00:00Z", last_observed: "2020-01-01T00:00:00Z",
      number_observed: 1, object_refs: ["ipv4-addr--1"],
    },
  ],
});

describe("stix node binding", () => {
  it("parses a pattern to an AST object", () => {
    const engine = new Engine();
    const ast = engine.parsePattern("[ipv4-addr:value = '198.51.100.5']").ast;
    expect(typeof ast).toBe("object");
    expect(JSON.stringify(ast)).toContain("ipv4-addr");
  });

  it("reads and iterates bundle objects", () => {
    const engine = new Engine();
    const bundle = engine.parseBundle(BUNDLE);
    expect(bundle.objectCount()).toBe(2);
    expect(bundle.object(0).id).toBe("ipv4-addr--1");
    expect(bundle.object(99)).toBeUndefined();
    expect([...bundle].map((o) => o.type)).toContain("observed-data");
  });

  it("matches (hit and miss)", () => {
    const engine = new Engine();
    const bundle = engine.parseBundle(BUNDLE);
    const hit = engine.parsePattern("[ipv4-addr:value = '198.51.100.5']");
    const res = engine.matchBundle(hit, bundle);
    expect(res.matched).toBe(true);
    expect(Array.isArray(res.observations)).toBe(true);
    const miss = engine.parsePattern("[ipv4-addr:value = '203.0.113.9']");
    expect(engine.matchBundle(miss, bundle).matched).toBe(false);
  });

  it("applies a custom-type hook and matches a computed property", () => {
    const engine = new Engine();
    engine.registerType("x-acme-widget", (obj) => ({
      ...obj,
      risk_band: obj.risk_score > 80 ? "high" : "low",
    }));
    const bundle = engine.parseBundle(JSON.stringify({
      type: "bundle",
      objects: [
        { type: "x-acme-widget", id: "x-acme-widget--1", risk_score: 90 },
        {
          type: "observed-data", id: "observed-data--1",
          first_observed: "2020-01-01T00:00:00Z", last_observed: "2020-01-01T00:00:00Z",
          number_observed: 1, object_refs: ["x-acme-widget--1"],
        },
      ],
    }));
    const pattern = engine.parsePattern("[x-acme-widget:risk_band = 'high']");
    expect(engine.matchBundle(pattern, bundle).matched).toBe(true);
  });

  it("maps errors to the StixError hierarchy", () => {
    const engine = new Engine();
    expect(() => engine.parsePattern("[bad")).toThrow(ParseError);
    expect(() => engine.parseBundle('{"type":"ipv4-addr","id":"x--1"}')).toThrow(ModelError);
    engine.registerType("x-thing", () => { throw new Error("nope"); });
    expect(() =>
      engine.parseBundle('{"type":"bundle","objects":[{"type":"x-thing","id":"x--1"}]}')
    ).toThrow(ValidationError);
    try { engine.parsePattern("[bad"); } catch (e) { expect(e).toBeInstanceOf(StixError); }
  });
});
