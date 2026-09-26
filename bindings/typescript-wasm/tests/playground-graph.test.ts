import { describe, it, expect } from "vitest";
import { Engine } from "../dist/index.js";
import { EXAMPLES } from "../playground/examples.js";
import {
  instructionsInOrder,
  parseListing,
  escapeLabel,
  irToMermaid,
} from "../playground/graph.js";

const engine = new Engine();

describe("irToMermaid", () => {
  for (const ex of EXAMPLES) {
    it(`graphs "${ex.label}"`, () => {
      const p = engine.parsePattern(ex.pattern);
      const text = irToMermaid(p.ir, p.irListing);
      const instrs = instructionsInOrder(p.ir);
      expect(text.startsWith("flowchart TB\n")).toBe(true);
      expect(text.match(/^  subgraph /gm)).toHaveLength(p.ir.blocks.length + 1);
      for (const ins of instrs) {
        expect(text).toMatch(new RegExp(`^    i${ins.id}\\["`, "m"));
      }
      const observes = instrs.filter((i: any) => i.op.op === "observe").length;
      expect(text.match(/ -\.-> /g) ?? []).toHaveLength(observes);
    });
  }

  it("labels nodes with their listing line", () => {
    const p = engine.parsePattern("[file:size > 1024]");
    expect(irToMermaid(p.ir, p.irListing)).toContain('i1["t1 = gt t0, 1024"]');
  });

  it("draws operand edges and the observe edge in the direction values flow", () => {
    const p = engine.parsePattern("[file:size > 1024]");
    const text = irToMermaid(p.ir, p.irListing);
    // load i0 -> compare i1 -> yield i2 -.-> observe i3 -> ret i4
    expect(text).toContain("  i0 --> i1");
    expect(text).toContain("  i1 --> i2");
    expect(text).toContain("  i2 -.-> i3");
    expect(text).toContain("  i3 --> i4");
  });

  it("escapes Mermaid-significant characters in labels", () => {
    const p = engine.parsePattern(`[file:name = 'a"]#<b>&']`);
    const text = irToMermaid(p.ir, p.irListing);
    expect(text).toContain("'a#34;#93;#35;#60;b#62;#38;'");
    expect(text).not.toContain(`'a"]`);
  });

  it("throws when the listing does not match the program", () => {
    const p = engine.parsePattern("[file:size > 1024]");
    expect(() => irToMermaid(p.ir, "block main (observation):\n  o0 = observe     b1\n")).toThrow(
      /listing has 1 instruction lines but the program has 5 instructions/,
    );
  });
});

describe("parseListing", () => {
  it("handles negated mnemonics, terminators, and spaces inside literals", () => {
    const listing =
      "block b1 (comparison):\n" +
      "  t0 = load        file:name\n" +
      "  t1 = not eq      t0, 'a  b'\n" +
      "       yield       t1\n" +
      "\n";
    expect(parseListing(listing)).toEqual([
      { name: "t0", mnemonic: "load", operands: "file:name", label: "t0 = load file:name" },
      { name: "t1", mnemonic: "not eq", operands: "t0, 'a  b'", label: "t1 = not eq t0, 'a  b'" },
      { name: null, mnemonic: "yield", operands: "t1", label: "yield t1" },
    ]);
  });
});

describe("escapeLabel", () => {
  it("replaces each significant character with a numeric entity", () => {
    expect(escapeLabel('#"<>&[]`')).toBe("#35;#34;#60;#62;#38;#91;#93;#96;");
  });
});
