import fs from "node:fs";
import path from "node:path";
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

describe("stix wasm binding", () => {
  it("parses a pattern to an AST object", () => {
    const engine = new Engine();
    const ast = engine.parsePattern("[ipv4-addr:value = '198.51.100.5']").ast;
    expect(typeof ast).toBe("object");
    expect(JSON.stringify(ast)).toContain("ipv4-addr");
  });

  it("exposes the IR, its listing, and canonical text", () => {
    const p = new Engine().parsePattern("[file:size>1024]  OR [file:name='a']");
    expect(p.ir.schema_version).toBe(1);
    expect(p.ir.blocks).toHaveLength(2);
    expect(p.ir.main.kind).toBe("main");
    expect(p.irListing).toContain("block main (observation):");
    expect(p.canonical).toBe("[file:size > 1024] OR [file:name = 'a']");
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

  describe("deeply nested patterns (issue #50)", () => {
    const deep = (n: number) => {
      let s = "[a:b = 1]";
      for (let i = 0; i < n; i++) {
        s = `[a:b = 1] FOLLOWEDBY [a:b = 1] OR [a:b = 1] AND (${s})`;
      }
      return s;
    };

    it("returns the AST at MAX_NESTING and it round-trips through JSON", () => {
      const p = new Engine().parsePattern(deep(40));
      const ast = p.ast;
      expect(ast).toBeTruthy();
      expect(JSON.parse(JSON.stringify(ast))).toEqual(ast);
      expect(p.ir).toBeTruthy();
    });

    it("rejects limit + 1 with a ParseError", () => {
      const engine = new Engine();
      expect(() => engine.parsePattern(deep(41))).toThrow(ParseError);
      expect(() => engine.parsePattern(deep(41))).toThrow(/nests too deeply/);
    });

    it("handles a 10,000-term flat OR chain", () => {
      const text = Array.from({ length: 10000 }, () => "[a:b = 1]").join(" OR ");
      const ast: any = new Engine().parsePattern(text).ast;
      const find = (v: any): any =>
        v && typeof v === "object"
          ? "Or" in v ? v.Or : Object.values(v).map(find).find(Boolean)
          : undefined;
      expect(find(ast)).toHaveLength(10000);
    });
  });

  it("reports matched observed-data ids", () => {
    const dir = path.join(__dirname, "../../../datasets/hackers-1995");
    const engine = new Engine();
    const bundle = engine.parseBundle(fs.readFileSync(path.join(dir, "bundle.json"), "utf8"));
    const patterns = JSON.parse(fs.readFileSync(path.join(dir, "patterns.json"), "utf8"));
    for (const p of patterns) {
      const r = engine.matchBundle(engine.parsePattern(p.pattern), bundle);
      expect(r.matched).toBe(p.expect.matched);
      expect([...new Set(r.observedDataIds)].sort()).toEqual([...p.expect.observed_data].sort());
    }
  });
});
