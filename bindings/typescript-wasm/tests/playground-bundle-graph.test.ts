import { describe, it, expect } from "vitest";
import fs from "node:fs";
import path from "node:path";
import { bundleToGraph, refsOf, isContainerRef, labelOf, familyOf } from "../playground/bundle-graph.js";

const objects = JSON.parse(
  fs.readFileSync(path.join(__dirname, "../../../datasets/hackers-1995/bundle.json"), "utf8"),
).objects as any[];

describe("bundleToGraph on the Hackers bundle", () => {
  const g = bundleToGraph(objects);
  const nodeIds = new Set(g.nodes.map((n: any) => n.id));

  it("has one node per non-relationship object, sightings included", () => {
    expect(g.nodes.length).toBe(objects.filter((o) => o.type !== "relationship").length);
    for (const s of objects.filter((o) => o.type === "sighting")) expect(nodeIds.has(s.id)).toBe(true);
  });

  it("turns every relationship into a labelled edge", () => {
    for (const r of objects.filter((o) => o.type === "relationship")) {
      expect(g.edges.some((e: any) => e.id === `${r.source_ref}|${r.target_ref}|${r.relationship_type}`)).toBe(true);
    }
  });

  it("excludes container references", () => {
    const containers = new Set(objects.filter((o) => ["report", "grouping", "note", "opinion"].includes(o.type)).map((o) => o.id));
    for (const e of g.edges as any[]) {
      if (e.attrs.label === "object_refs") expect(containers.has(e.source)).toBe(false);
      expect(["created_by_ref", "object_marking_refs"]).not.toContain(e.attrs.label);
    }
  });

  it("only connects nodes that exist, with keys as ids", () => {
    for (const e of g.edges as any[]) {
      expect(nodeIds.has(e.source) && nodeIds.has(e.target)).toBe(true);
      expect(e.id).toBe(`${e.source}|${e.target}|${e.attrs.label}`);
      expect(e.attrs.key).toBe(e.id);
    }
    expect(new Set(g.edges.map((e: any) => e.id)).size).toBe(g.edges.length);
  });

  it("labels and families every node", () => {
    for (const n of g.nodes as any[]) {
      expect(typeof n.label).toBe("string");
      expect(n.label.length).toBeGreaterThan(0);
      expect(n.attrs.stix_id).toBe(n.id);
      expect(["actor", "capability", "target", "detection", "context", "observable"]).toContain(n.attrs.family);
    }
  });
});

describe("helpers", () => {
  it("drops edges to unknown or relationship ids", () => {
    const g = bundleToGraph([
      { type: "malware", id: "malware--1", name: "m", sample_refs: ["file--missing"] },
      { type: "relationship", id: "relationship--1", relationship_type: "uses", source_ref: "malware--1", target_ref: "tool--missing" },
      { type: "note", id: "note--1", content: "x", object_refs: ["relationship--1"] },
    ]);
    expect(g.edges).toEqual([]);
  });

  it("finds references at any depth", () => {
    expect(refsOf({ a_ref: "x--1", ext: { inner: { b_refs: ["y--1", "z--1"] } } })).toEqual([
      ["a_ref", "x--1"], ["b_refs", "y--1"], ["b_refs", "z--1"],
    ]);
  });

  it("classifies container references", () => {
    expect(isContainerRef("report", "object_refs")).toBe(true);
    expect(isContainerRef("observed-data", "object_refs")).toBe(false);
    expect(isContainerRef("malware", "created_by_ref")).toBe(true);
  });

  it("labels objects sensibly", () => {
    expect(labelOf({ type: "threat-actor", name: "Zero Cool" })).toBe("Zero Cool");
    expect(labelOf({ type: "domain-name", value: "a.example" })).toBe("a.example");
    expect(labelOf({ type: "observed-data", first_observed: "1995-08-14T23:02:00.000Z" })).toBe("observed-data 1995-08-14");
    expect(familyOf("threat-actor")).toBe("actor");
    expect(familyOf("domain-name")).toBe("observable");
  });
});
