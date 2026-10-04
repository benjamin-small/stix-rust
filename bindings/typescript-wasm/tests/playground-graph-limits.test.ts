import { describe, it, expect } from "vitest";
import { Engine } from "../dist/index.js";
import { irToMermaid } from "../playground/graph.js";
import { MERMAID_MAX_EDGES, MERMAID_MAX_TEXT_SIZE } from "../playground/limits.js";

const engine = new Engine();

function measure(pattern: string) {
  const p = engine.parsePattern(pattern);
  const text = irToMermaid(p.ir, p.irListing);
  const edges = text.split("\n").filter((l) => /-->|-\.->|---|==>/.test(l)).length;
  return { edges, size: text.length };
}

function worstShape(depth: number) {
  let s = "[a:b = 1]";
  for (let i = 0; i < depth; i++) {
    s = "[a:b = 1] FOLLOWEDBY [a:b = 1] OR [a:b = 1] AND (" + s + ")";
  }
  return s;
}

describe("playground graph size limits", () => {
  it("keeps the depth-40 worst shape under the limits", () => {
    const m = measure(worstShape(40));
    console.log("worst-shape", m);
    expect(m.edges).toBeLessThan(MERMAID_MAX_EDGES);
    expect(m.size).toBeLessThan(MERMAID_MAX_TEXT_SIZE);
  });

  it("still caps a 2000-term flat OR", () => {
    const flat = Array.from({ length: 2000 }, () => "[a:b = 1]").join(" OR ");
    const m = measure(flat);
    console.log("flat-or-2000", m);
    expect(m.edges).toBeGreaterThan(MERMAID_MAX_EDGES);
  });
});
