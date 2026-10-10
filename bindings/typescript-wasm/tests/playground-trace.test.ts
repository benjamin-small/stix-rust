import { describe, it, expect } from "vitest";
import fs from "node:fs";
import path from "node:path";
import { Engine } from "../dist/index.js";
import { traceMatch } from "../playground/trace.js";
import { bundleToGraph } from "../playground/bundle-graph.js";

const dir = path.join(__dirname, "../../../datasets/hackers-1995");
const text = fs.readFileSync(path.join(dir, "bundle.json"), "utf8");
const objects = JSON.parse(text).objects as any[];
const patterns = JSON.parse(fs.readFileSync(path.join(dir, "patterns.json"), "utf8")) as any[];
const engine = new Engine();
const bundle = engine.parseBundle(text);
const graph = bundleToGraph(objects);
const nodeIds = new Set(graph.nodes.map((n: any) => n.id));
const edgeIds = new Set(graph.edges.map((e: any) => e.id));
const byId = new Map(objects.map((o) => [o.id, o]));

for (const p of patterns) {
  describe(`trace for "${p.name}"`, () => {
    const ids = engine.matchBundle(engine.parsePattern(p.pattern), bundle).observedDataIds;
    const t = traceMatch(objects, ids);

    it("glows the matched observed-data and their SCOs", () => {
      for (const od of p.expect.observed_data) {
        expect(t.nodes.get(od)).toBe("glow");
        for (const sco of byId.get(od).object_refs) expect(t.nodes.get(sco)).toBe("glow");
      }
    });

    it("highlights only real graph nodes and edges", () => {
      for (const id of t.nodes.keys()) expect(nodeIds.has(id)).toBe(true);
      for (const k of t.edges) expect(edgeIds.has(k)).toBe(true);
    });

    if (!p.expect.matched) {
      it("highlights nothing for a non-match", () => {
        expect(t.nodes.size).toBe(0);
        expect(t.edges.size).toBe(0);
      });
    }
  });
}

describe("trace through sightings and indicators", () => {
  it("reaches what a sighted indicator indicates, and one hop beyond", () => {
    const sighted = objects.filter((o) => o.type === "sighting");
    const od = sighted[0].observed_data_refs[0];
    const t = traceMatch(objects, [od]);
    expect(t.nodes.get(sighted[0].id)).toBe("trace");
    expect(t.nodes.get(sighted[0].sighting_of_ref)).toBe("trace");
    const indicates = objects.filter(
      (o) => o.type === "relationship" && o.source_ref === sighted[0].sighting_of_ref && o.relationship_type === "indicates",
    );
    expect(indicates.length).toBeGreaterThan(0);
    for (const r of indicates) expect(["glow", "trace"]).toContain(t.nodes.get(r.target_ref));
  });

  it("is empty for no matches", () => {
    const t = traceMatch(objects, []);
    expect(t.nodes.size).toBe(0);
  });
});
