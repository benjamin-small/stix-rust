# Pattern Playground Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A static page on the project's GitHub Pages site that parses a STIX pattern in WebAssembly and shows its AST, IR listing, IR JSON, IR graph, and canonical text.

**Architecture:** `stix-ffi` gains three IR accessors (PR 1, `rust-core`). The wasm binding exposes them. A plain-ES-module page under `bindings/typescript-wasm/playground/` loads the `wasm-pack --target web` build and the book's vendored Mermaid (PR 2, `typescript-wasm`). The parent wires the page into the Pages deploy (PR 3). The graph is Mermaid text generated in JS from the IR JSON, with node labels taken from the Rust-generated listing.

**Tech Stack:** Rust (stix-ffi, serde_json), wasm-bindgen + wasm-pack, plain HTML/CSS/ES modules, Mermaid 11 (vendored at `docs/book/mermaid.min.js`), vitest, GitHub Actions Pages.

**Spec:** `docs/superpowers/specs/2026-09-26-pattern-playground-design.md`

## Global Constraints

- Follow `AGENTS.md`: each task edits only its area's owned paths. Tasks 1 → `crates/**` (`rust-core`); Tasks 2–5 → `bindings/typescript-wasm/**` (`typescript-wasm-binding`); Tasks 6–7 → parent (`.github/`, `docs/book/`).
- PR grouping: Task 1 = PR 1; Tasks 2–5 = PR 2 (branch off `main` after PR 1 merges); Task 6 is the parent's check of PR 2 before merge; Task 7 = PR 3.
- No new npm dependencies, no bundler, no framework, no new `package.json`.
- No new public API in `stix-pattern`. Only `stix-ffi` and the wasm binding change their API.
- Spans and parse-error offsets from Rust are **UTF-8 byte offsets**. Convert them with `playground/spans.js` before indexing a JS string.
- The Rust `ParseError` display format is `parse error at bytes {start}..{end}: {message}` (`crates/stix-pattern/src/error.rs:24`). The wasm layer prefixes it with `[parse] `.
- Mermaid runs with `securityLevel: "strict"`.
- Browser storage (`localStorage`) is only used for the remembered tab, and every access is wrapped in try/catch.
- Commit messages carry no attribution trailers.

---

### Task 1: IR accessors on `stix_ffi::Pattern` (PR 1, rust-core)

**Files:**
- Modify: `crates/stix-ffi/src/handles.rs` (add methods after `to_json`, around line 22; add tests to the existing `mod tests` around line 62)

**Interfaces:**
- Consumes: `stix::pattern::ir::{lower, render}`, `Program::to_listing()`. These are already reachable through the umbrella crate's `pub use stix_pattern as pattern`.
- Produces:
  - `pub fn ir_json(&self) -> String`: compact JSON of `ir::lower(ast)`, `"null"` on the (unreachable) serialization failure
  - `pub fn ir_listing(&self) -> String`
  - `pub fn canonical(&self) -> String`

- [ ] **Step 1: Write the failing tests** — append inside `mod tests` in `crates/stix-ffi/src/handles.rs`:

```rust
    const TWO_OBSERVATIONS: &str = "[file:size > 1024] OR [file:name = 'a']";

    #[test]
    fn pattern_ir_json_has_blocks_and_main() {
        let handle = Pattern::new(stix::parse(TWO_OBSERVATIONS).unwrap());
        let ir: serde_json::Value = serde_json::from_str(&handle.ir_json()).unwrap();
        assert_eq!(ir["schema_version"], 1);
        assert_eq!(ir["blocks"].as_array().unwrap().len(), 2);
        assert_eq!(ir["main"]["kind"], "main");
    }

    #[test]
    fn pattern_ir_listing_names_blocks() {
        let handle = Pattern::new(stix::parse(TWO_OBSERVATIONS).unwrap());
        let listing = handle.ir_listing();
        assert!(listing.contains("block b1 (comparison):"), "{listing}");
        assert!(listing.contains("block main (observation):"), "{listing}");
    }

    #[test]
    fn pattern_canonical_normalizes_spacing() {
        let handle = Pattern::new(stix::parse("[file:size>1024]  OR [file:name='a']").unwrap());
        assert_eq!(handle.canonical(), TWO_OBSERVATIONS);
    }
```

- [ ] **Step 2: Run the tests and confirm they fail**

Run: `cargo test -p stix-ffi pattern_`
Expected: compile error `no method named ir_json` (and similar for the others).

- [ ] **Step 3: Implement.** Insert this after `to_json` in `impl Pattern`:

```rust
    /// The pattern lowered to the IR, serialized as compact JSON.
    pub fn ir_json(&self) -> String {
        // As with `to_json`, serialization is infallible in practice.
        serde_json::to_string(&self.lowered()).unwrap_or_else(|_| "null".to_string())
    }

    /// The IR as a human-readable listing; see `Program::to_listing`.
    pub fn ir_listing(&self) -> String {
        self.lowered().to_listing()
    }

    /// Canonical pattern text, rendered back from the IR.
    pub fn canonical(&self) -> String {
        stix::pattern::ir::render(&self.lowered())
    }

    fn lowered(&self) -> stix::pattern::ir::Program {
        stix::pattern::ir::lower(&self.inner)
    }
```

- [ ] **Step 4: Run the tests and the workspace checks**

Run: `cargo test -p stix-ffi && cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings`
Expected: all pass, no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-ffi/src/handles.rs
git commit -m "feat(ffi): expose IR JSON, IR listing, and canonical text on Pattern"
```

---

### Task 2: Wasm getters `ir`, `irListing`, `canonical` (PR 2, typescript-wasm)

**Files:**
- Modify: `bindings/typescript-wasm/src/lib.rs` (the `impl Pattern` block, around line 35)
- Modify: `bindings/typescript-wasm/ts/index.ts` (the `Pattern` class, around line 18)
- Modify: `bindings/typescript-wasm/tests/stix.test.ts`
- Modify: `bindings/typescript-wasm/README.md` (the API section: list the three getters next to `ast`)

**Interfaces:**
- Consumes: `stix_ffi::Pattern::{ir_json, ir_listing, canonical}` from Task 1.
- Produces:
  - Raw wasm class `Pattern` (used by the page, Tasks 5): getters `ir: any` (throws `"[model] ..."` on the unreachable JSON failure), `irListing: string`, `canonical: string`, plus the existing `ast` and wasm-bindgen's `free()`.
  - TS wrapper `Pattern` (used by tests, Tasks 3–4): getters `ir: any`, `irListing: string`, `canonical: string`.

- [ ] **Step 1: Write the failing test.** Add this inside the `describe("stix wasm binding", ...)` block in `tests/stix.test.ts`:

```ts
  it("exposes the IR, its listing, and canonical text", () => {
    const p = new Engine().parsePattern("[file:size>1024]  OR [file:name='a']");
    expect(p.ir.schema_version).toBe(1);
    expect(p.ir.blocks).toHaveLength(2);
    expect(p.ir.main.kind).toBe("main");
    expect(p.irListing).toContain("block main (observation):");
    expect(p.canonical).toBe("[file:size > 1024] OR [file:name = 'a']");
  });
```

- [ ] **Step 2: Run the test and confirm it fails**

Run (in `bindings/typescript-wasm`): `npm test`
Expected: the `tsc` step fails with `Property 'ir' does not exist on type 'Pattern'`.

- [ ] **Step 3: Implement.** In `src/lib.rs`, extend `impl Pattern`:

```rust
    #[wasm_bindgen(getter)]
    pub fn ir(&self) -> Result<JsValue, JsValue> {
        json_to_js(&self.inner.ir_json())
    }

    #[wasm_bindgen(getter, js_name = irListing)]
    pub fn ir_listing(&self) -> String {
        self.inner.ir_listing()
    }

    #[wasm_bindgen(getter)]
    pub fn canonical(&self) -> String {
        self.inner.canonical()
    }
```

In `ts/index.ts`, extend the wrapper `Pattern` class after `get ast()`:

```ts
  get ir(): any {
    try { return this.raw.ir; } catch (e) { throw toStixError(e); }
  }
  get irListing(): string { return this.raw.irListing; }
  get canonical(): string { return this.raw.canonical; }
```

Add the three getters to the README's API listing, with one line each, in the same style as `ast`.

- [ ] **Step 4: Run the tests and confirm they pass**

Run: `npm test`
Expected: all tests pass, including the new one.

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs ts/index.ts tests/stix.test.ts README.md
git commit -m "feat(wasm): expose ir, irListing, and canonical getters on Pattern"
```

---

### Task 3: `spans.js` — byte offsets and parse-error spans (PR 2)

**Files:**
- Create: `bindings/typescript-wasm/playground/spans.js`
- Test: `bindings/typescript-wasm/tests/playground-spans.test.ts`

**Interfaces:**
- Consumes: TS wrapper `Engine`, `ParseError` (`../dist/index.js`) — test only.
- Produces (ES module exports):
  - `byteToUtf16(text: string, byte: number): number`, which clamps to `text.length`
  - `spanToRange(text: string, span: {start: number, end: number}): {start: number, end: number}`, which returns UTF-16 indices
  - `parseErrorSpan(message: string): {start: number, end: number} | null`, which returns byte offsets

- [ ] **Step 1: Write the failing test** — `tests/playground-spans.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { Engine, ParseError } from "../dist/index.js";
import { byteToUtf16, spanToRange, parseErrorSpan } from "../playground/spans.js";

describe("byteToUtf16", () => {
  it("is the identity on ASCII", () => {
    expect(byteToUtf16("[file:size > 1]", 6)).toBe(6);
  });

  it("accounts for multi-byte characters", () => {
    // "[file:name = '" is 14 bytes and 14 units; "é" is 2 bytes / 1 unit;
    // "😀" is 4 bytes / 2 units.
    const s = "[file:name = 'é😀x']";
    expect(byteToUtf16(s, 14)).toBe(14);
    expect(byteToUtf16(s, 16)).toBe(15);
    expect(byteToUtf16(s, 20)).toBe(17);
    expect(byteToUtf16(s, 21)).toBe(18);
  });

  it("clamps offsets past the end", () => {
    expect(byteToUtf16("abc", 99)).toBe(3);
  });

  it("converts a span to a range", () => {
    expect(spanToRange("é = 1", { start: 0, end: 2 })).toEqual({ start: 0, end: 1 });
  });
});

describe("parseErrorSpan", () => {
  it("reads the span out of a real parse error", () => {
    let message = "";
    try {
      new Engine().parsePattern("[file:size >");
    } catch (e) {
      expect(e).toBeInstanceOf(ParseError);
      message = (e as Error).message;
    }
    expect(parseErrorSpan(message)).toEqual({ start: 12, end: 12 });
  });

  it("also reads the raw wasm message with its [parse] prefix", () => {
    expect(parseErrorSpan("[parse] parse error at bytes 3..7: nope")).toEqual({ start: 3, end: 7 });
  });

  it("returns null for a message with no span", () => {
    expect(parseErrorSpan("something else")).toBeNull();
  });
});
```

- [ ] **Step 2: Run the test and confirm it fails**

Run: `npm test`
Expected: FAIL with `Failed to load url ../playground/spans.js` (or similar module-not-found).

- [ ] **Step 3: Implement** — `playground/spans.js`:

```js
// Source spans from the Rust core are UTF-8 byte offsets; JS strings index by
// UTF-16 code unit. These helpers convert between the two.

/** The UTF-16 index in `text` of UTF-8 byte offset `byte`, clamped to the end. */
export function byteToUtf16(text, byte) {
  let bytes = 0;
  let i = 0;
  while (i < text.length && bytes < byte) {
    const cp = text.codePointAt(i);
    bytes += cp < 0x80 ? 1 : cp < 0x800 ? 2 : cp < 0x10000 ? 3 : 4;
    i += cp > 0xffff ? 2 : 1;
  }
  return i;
}

/** A byte span as a UTF-16 `{start, end}` range into `text`. */
export function spanToRange(text, span) {
  return { start: byteToUtf16(text, span.start), end: byteToUtf16(text, span.end) };
}

// The `Display` format of stix-pattern's `ParseError`.
const PARSE_ERROR_RE = /parse error at bytes (\d+)\.\.(\d+): /;

/** The byte span a parse-error message names, or null if it names none. */
export function parseErrorSpan(message) {
  const m = PARSE_ERROR_RE.exec(message);
  return m ? { start: Number(m[1]), end: Number(m[2]) } : null;
}
```

- [ ] **Step 4: Run the tests and confirm they pass**

Run: `npm test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add playground/spans.js tests/playground-spans.test.ts
git commit -m "feat(playground): byte-offset and parse-error span helpers"
```

---

### Task 4: `examples.js` and `graph.js` — IR to Mermaid (PR 2)

**Files:**
- Create: `bindings/typescript-wasm/playground/examples.js`
- Create: `bindings/typescript-wasm/playground/graph.js`
- Test: `bindings/typescript-wasm/tests/playground-graph.test.ts`

**Interfaces:**
- Consumes: TS wrapper getters `ir`, `irListing` from Task 2 (test only).
- Produces (ES module exports):
  - `EXAMPLES: {label: string, pattern: string}[]` from `examples.js`
  - `instructionsInOrder(program): Instruction[]`, which lists comparison blocks' instructions and then main's, matching the listing order
  - `isInstructionLine(line: string): boolean`
  - `parseListing(listing: string): {name: string|null, mnemonic: string, operands: string, label: string}[]`, with one entry per instruction line
  - `escapeLabel(s: string): string`
  - `irToMermaid(program, listing: string): string`, which uses Mermaid node ids `i<InstrId>` and throws `Error` when the listing and the program disagree

- [ ] **Step 1: Write `examples.js`.** The tests iterate over it. Every pattern below has been checked to parse:

```js
// Example patterns offered in the playground's picker. The first one loads on
// page load. Each one exercises a different part of the IR.
export const EXAMPLES = [
  { label: "Single comparison", pattern: "[file:size > 1024]" },
  {
    label: "AND / OR in one observation",
    pattern: "[file:name = 'invoice.exe' OR file:size > 1024 AND file:size < 4096]",
  },
  { label: "EXISTS", pattern: "[EXISTS file:hashes.'SHA-256']" },
  { label: "IN a set", pattern: "[ipv4-addr:value IN ('198.51.100.1', '203.0.113.7')]" },
  { label: "NOT with an operator", pattern: "[domain-name:value NOT LIKE '%.example.com']" },
  {
    label: "FOLLOWEDBY … WITHIN",
    pattern:
      "[ipv4-addr:value = '198.51.100.1'] FOLLOWEDBY [domain-name:value = 'evil.example'] WITHIN 300 SECONDS",
  },
  { label: "REPEATS", pattern: "[network-traffic:dst_port = 22] REPEATS 5 TIMES" },
  {
    label: "START … STOP",
    pattern: "[file:name = 'a.exe'] START t'2024-01-01T00:00:00Z' STOP t'2024-02-01T00:00:00Z'",
  },
  {
    label: "Observation-level AND / OR",
    pattern: "([ipv4-addr:value = '1.2.3.4'] AND [file:size > 0]) OR [process:name MATCHES '^cmd']",
  },
];
```

- [ ] **Step 2: Write the failing test** — `tests/playground-graph.test.ts`:

```ts
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
```

- [ ] **Step 3: Run the test and confirm it fails**

Run: `npm test`
Expected: FAIL with `Failed to load url ../playground/graph.js`.

- [ ] **Step 4: Implement** — `playground/graph.js`:

```js
// Builds a Mermaid flowchart from a lowered IR program.
//
// Node labels are the program's own listing lines (`Pattern.irListing`), so
// literal and path formatting stays in Rust instead of being re-implemented here.

/** Every instruction: comparison blocks first, then main — the listing's order. */
export function instructionsInOrder(program) {
  return [...program.blocks, program.main].flatMap((b) => b.instructions);
}

/** Whether a listing line is an instruction (as opposed to a header or blank). */
export function isInstructionLine(line) {
  return /^ +\S/.test(line);
}

// An instruction line: indent, optional "name = ", a mnemonic (possibly
// "not <op>"), then the operands. Only the alignment padding between fields is
// dropped; the operands are kept verbatim, so literals keep their spaces.
const LINE_RE = /^ +(?:(\S+) += )?(not \S+|\S+)(?: +(.*))?$/;

/** The listing's instruction lines, in order, split into their fields. */
export function parseListing(listing) {
  return listing
    .split("\n")
    .filter(isInstructionLine)
    .map((line) => {
      const [, name = null, mnemonic, operands = ""] = LINE_RE.exec(line);
      const head = name ? `${name} = ${mnemonic}` : mnemonic;
      return { name, mnemonic, operands, label: operands ? `${head} ${operands}` : head };
    });
}

const ENTITIES = {
  "#": "#35;",
  '"': "#34;",
  "<": "#60;",
  ">": "#62;",
  "&": "#38;",
  "[": "#91;",
  "]": "#93;",
  "`": "#96;",
};

/** Escape text for use inside a quoted Mermaid label. */
export function escapeLabel(s) {
  return s.replace(/[#"<>&[\]`]/g, (c) => ENTITIES[c]);
}

const OPERAND_FIELDS = ["lhs", "rhs", "input", "value"];

// The instruction ids an op reads. A compare's `rhs` is a literal operand
// object, not an id, so only numeric fields count.
function operandsOf(op) {
  return OPERAND_FIELDS.map((f) => op[f]).filter((v) => typeof v === "number");
}

const CLASS_OF = {
  load: "test",
  compare: "test",
  and: "bool",
  or: "bool",
  observe: "observe",
  followed_by: "temporal",
  within: "temporal",
  repeats: "temporal",
  start_stop: "temporal",
  yield: "term",
  ret: "term",
};

const nodeId = (id) => `i${id}`;

/** Mermaid flowchart text for `program`, labelled from its `listing`. */
export function irToMermaid(program, listing) {
  const instrs = instructionsInOrder(program);
  const lines = parseListing(listing);
  if (lines.length !== instrs.length) {
    throw new Error(
      `listing has ${lines.length} instruction lines but the program has ${instrs.length} instructions`,
    );
  }
  const labelOf = new Map(instrs.map((ins, k) => [ins.id, lines[k].label]));

  const out = ["flowchart TB"];
  for (const block of [...program.blocks, program.main]) {
    const isMain = block.kind === "main";
    const title = isMain ? "main" : `block b${block.id} (comparison)`;
    out.push(`  subgraph ${isMain ? "blk_main" : `blk_${block.id}`}["${escapeLabel(title)}"]`);
    for (const ins of block.instructions) {
      const cls = CLASS_OF[ins.op.op] ?? "test";
      out.push(`    ${nodeId(ins.id)}["${escapeLabel(labelOf.get(ins.id))}"]:::${cls}`);
    }
    out.push("  end");
  }

  // A comparison block's value is its terminator's; `observe` reads it.
  const yieldOf = new Map(
    program.blocks.filter((b) => b.instructions.length).map((b) => [b.id, b.instructions.at(-1).id]),
  );
  for (const ins of instrs) {
    for (const src of operandsOf(ins.op)) out.push(`  ${nodeId(src)} --> ${nodeId(ins.id)}`);
    if (ins.op.op === "observe" && yieldOf.has(ins.op.block)) {
      out.push(`  ${nodeId(yieldOf.get(ins.op.block))} -.-> ${nodeId(ins.id)}`);
    }
  }

  out.push(
    "  classDef test stroke:#2f7ed8,stroke-width:2px",
    "  classDef bool stroke:#8250df,stroke-width:2px",
    "  classDef observe stroke:#1a7f37,stroke-width:2px",
    "  classDef temporal stroke:#bf8700,stroke-width:2px",
    "  classDef term stroke:#6e7781,stroke-width:1px,stroke-dasharray:4",
  );
  return out.join("\n");
}
```

- [ ] **Step 5: Run the tests and confirm they pass**

Run: `npm test`
Expected: all pass, including one "graphs …" case per example.

If the edge-order assertions fail, check the actual ids in `p.ir` for `[file:size > 1024]`. Lowering assigns 0 = load, 1 = compare, 2 = yield, 3 = observe, 4 = ret. If that ever differs, **stop and report it rather than changing the test**.

- [ ] **Step 6: Commit**

```bash
git add playground/examples.js playground/graph.js tests/playground-graph.test.ts
git commit -m "feat(playground): example patterns and IR-to-Mermaid graph builder"
```

---

### Task 5: The page, its build script, and docs (PR 2)

**Files:**
- Create: `bindings/typescript-wasm/playground/index.html`
- Create: `bindings/typescript-wasm/playground/playground.css`
- Create: `bindings/typescript-wasm/playground/main.js`
- Create: `bindings/typescript-wasm/scripts/build-playground.mjs`
- Modify: `bindings/typescript-wasm/package.json` (add a script)
- Modify: `bindings/typescript-wasm/.gitignore` (add `playground-dist/`)
- Modify: `bindings/typescript-wasm/README.md` (add a "Playground" section)

**Interfaces:**
- Consumes: the raw web-target module `./pkg/stix_wasm.js`, which provides a default `init()` and an `Engine` whose `parsePattern(src)` returns a raw `Pattern` with `ast`, `ir`, `irListing` and `canonical` getters plus `free()`. Also `EXAMPLES`, `irToMermaid`, `instructionsInOrder`, `isInstructionLine` (Task 4), `spanToRange` and `parseErrorSpan` (Task 3), and the global `mermaid` from `mermaid.min.js`.
- Produces: `npm run build:playground`, which writes a self-contained static site to `bindings/typescript-wasm/playground-dist/` containing `index.html`, `playground.css`, `main.js`, `graph.js`, `spans.js`, `examples.js`, `pkg/stix_wasm.js`, `pkg/stix_wasm_bg.wasm` and `mermaid.min.js`.

- [ ] **Step 1: Write `playground/index.html`**

```html
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>STIX Pattern Playground</title>
  <link rel="stylesheet" href="playground.css">
  <script src="mermaid.min.js"></script>
  <script type="module" src="main.js"></script>
</head>
<body>
  <header>
    <h1>STIX Pattern Playground</h1>
    <p>Parse a STIX 2.1 pattern with <a href="../">stix-rust</a>, compiled to
      WebAssembly, and inspect its AST and IR. Nothing leaves your browser.</p>
  </header>
  <div id="load-error" class="banner error" role="alert" hidden></div>
  <main>
    <section class="input">
      <label for="examples">Example</label>
      <select id="examples"></select>
      <div class="editor">
        <div id="backdrop" class="backdrop" aria-hidden="true"></div>
        <textarea id="pattern" spellcheck="false" autocomplete="off" aria-label="STIX pattern"></textarea>
      </div>
      <div id="parse-error" class="banner error" role="status" hidden></div>
    </section>
    <section class="output">
      <nav role="tablist" aria-label="Output">
        <button role="tab" data-tab="graph">Graph</button>
        <button role="tab" data-tab="listing">IR listing</button>
        <button role="tab" data-tab="ast">AST</button>
        <button role="tab" data-tab="ir">IR JSON</button>
        <button role="tab" data-tab="canonical">Canonical</button>
      </nav>
      <div id="panes">
        <div class="pane" data-pane="graph" id="graph"></div>
        <pre class="pane" data-pane="listing" id="listing"></pre>
        <pre class="pane" data-pane="ast" id="ast"></pre>
        <pre class="pane" data-pane="ir" id="ir"></pre>
        <pre class="pane" data-pane="canonical" id="canonical"></pre>
      </div>
    </section>
  </main>
</body>
</html>
```

- [ ] **Step 2: Write `playground/playground.css`**

```css
:root {
  --bg: #ffffff; --fg: #1f2328; --muted: #59636e; --border: #d1d9e0;
  --panel: #f6f8fa; --accent: #0969da; --err: #cf222e; --err-bg: #ffebe9;
  --hl: #fff8c5;
  --mono: 14px/1.5 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  color-scheme: light dark;
  font-family: system-ui, -apple-system, "Segoe UI", sans-serif;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #0d1117; --fg: #e6edf3; --muted: #9198a1; --border: #3d444d;
    --panel: #151b23; --accent: #4493f8; --err: #f85149; --err-bg: #3c1618;
    --hl: #4b3b0a;
  }
}
* { box-sizing: border-box; }
[hidden] { display: none !important; }
body { margin: 0; background: var(--bg); color: var(--fg); }
a { color: var(--accent); }
header { padding: 16px 24px 0; }
h1 { font-size: 1.4rem; margin: 0 0 4px; }
header p { margin: 0; color: var(--muted); }
main {
  display: grid; grid-template-columns: minmax(0, 2fr) minmax(0, 3fr);
  gap: 16px; padding: 16px 24px 24px;
}
@media (max-width: 900px) {
  header { padding: 16px 16px 0; }
  main { grid-template-columns: minmax(0, 1fr); padding: 16px; }
}
.input label { display: block; font-size: .85rem; color: var(--muted); margin-bottom: 4px; }
select { margin-bottom: 8px; max-width: 100%; font: inherit; }

/* The textarea sits over a mirror of its text; marks in the mirror show the
   error and hover ranges through the transparent textarea. */
.editor { position: relative; }
.editor textarea, .backdrop {
  font: var(--mono); padding: 10px; margin: 0; width: 100%;
  border: 1px solid var(--border); border-radius: 6px;
  white-space: pre-wrap; overflow-wrap: anywhere;
}
.editor textarea {
  position: relative; display: block; height: 10rem; resize: vertical;
  background: transparent; color: var(--fg);
}
.backdrop {
  position: absolute; inset: 0; overflow: hidden;
  color: transparent; background: var(--panel); border-color: transparent;
}
.backdrop mark { color: transparent; border-radius: 2px; }
.backdrop mark.hl { background: var(--hl); }
.backdrop mark.err {
  background: transparent;
  text-decoration: underline wavy var(--err); text-decoration-skip-ink: none;
}
.backdrop mark.caret { background: transparent; border-left: 2px solid var(--err); margin-left: -1px; }

.banner { margin-top: 8px; padding: 8px 10px; border-radius: 6px; font-size: .9rem; overflow-wrap: anywhere; }
.banner.error { background: var(--err-bg); color: var(--err); }
#load-error { margin: 16px 24px 0; }

nav[role=tablist] { display: flex; flex-wrap: wrap; gap: 4px; border-bottom: 1px solid var(--border); }
nav button {
  font: inherit; background: none; border: none; cursor: pointer;
  border-bottom: 2px solid transparent; padding: 6px 10px; color: var(--muted);
}
nav button[aria-selected=true] { color: var(--fg); border-bottom-color: var(--accent); }
.pane {
  margin: 0; padding: 12px; min-height: 16rem; max-height: 70vh; overflow: auto;
  background: var(--panel); border: 1px solid var(--border); border-top: none;
  border-radius: 0 0 6px 6px; font: var(--mono); font-size: 13px;
}
#panes.stale .pane { opacity: .45; }
#listing .instr:hover { background: var(--hl); }
#graph svg { max-width: 100%; height: auto; }
```

- [ ] **Step 3: Write `playground/main.js`**

```js
import init, { Engine } from "./pkg/stix_wasm.js";
import { EXAMPLES } from "./examples.js";
import { irToMermaid, instructionsInOrder, isInstructionLine } from "./graph.js";
import { spanToRange, parseErrorSpan } from "./spans.js";

const $ = (id) => document.getElementById(id);
const input = $("pattern");
const backdrop = $("backdrop");
const errorBanner = $("parse-error");
const panes = $("panes");
const mermaid = globalThis.mermaid; // from mermaid.min.js; absent if it failed to load

const TABS = ["graph", "listing", "ast", "ir", "canonical"];
const TAB_KEY = "stix-playground-tab";
const DEBOUNCE_MS = 150;

let engine;
let current = null; // { text, instrs } from the last successful parse
let errorRange = null; // UTF-16 range of the current parse error
let graphSeq = 0;

const messageOf = (e) => (e instanceof Error ? e.message : String(e));

// Redraw the mirror behind the textarea, with at most one marked range.
function renderBackdrop(range, cls) {
  const text = input.value;
  backdrop.replaceChildren();
  if (!range) {
    backdrop.append(text + "\n");
  } else {
    const mark = document.createElement("mark");
    const empty = range.start === range.end;
    mark.className = empty ? `${cls} caret` : cls;
    mark.textContent = text.slice(range.start, range.end);
    backdrop.append(text.slice(0, range.start), mark, text.slice(range.end) + "\n");
  }
  backdrop.scrollTop = input.scrollTop;
}

// The resting state of the mirror: the parse error, if there is one.
function showMark() {
  renderBackdrop(errorRange, "err");
}

function highlightInstruction(ins) {
  if (!current || !ins?.span) return showMark();
  renderBackdrop(spanToRange(current.text, ins.span), "hl");
}

// Mermaid gives each node element an id like "flowchart-i3-12"; "i3" is the
// node id graph.js assigned to instruction 3.
function nodeInstrId(el) {
  const m = el && /flowchart-i(\d+)-/.exec(el.id);
  return m ? Number(m[1]) : null;
}

function selectTab(name) {
  for (const b of document.querySelectorAll("[data-tab]")) {
    b.setAttribute("aria-selected", String(b.dataset.tab === name));
  }
  for (const p of document.querySelectorAll("[data-pane]")) p.hidden = p.dataset.pane !== name;
  try { localStorage.setItem(TAB_KEY, name); } catch { /* storage unavailable */ }
}

function clearPanes() {
  for (const p of document.querySelectorAll("[data-pane]")) p.replaceChildren();
}

function renderListing(listing) {
  const pre = $("listing");
  pre.replaceChildren();
  let k = 0;
  for (const line of listing.split("\n")) {
    const span = document.createElement("span");
    span.textContent = line + "\n";
    if (isInstructionLine(line)) {
      span.className = "instr";
      span.dataset.k = String(k++);
    }
    pre.append(span);
  }
}

async function renderGraph(ir, listing) {
  const seq = ++graphSeq;
  const el = $("graph");
  if (!mermaid) {
    el.textContent = "Mermaid failed to load, so the graph is unavailable.";
    return;
  }
  try {
    const source = irToMermaid(ir, listing);
    await mermaid.parse(source);
    const { svg } = await mermaid.render(`graph-${seq}`, source);
    if (seq === graphSeq) el.innerHTML = svg;
  } catch (e) {
    if (seq === graphSeq) el.textContent = `Graph rendering failed: ${messageOf(e)}`;
  }
}

function update() {
  const text = input.value;
  if (!text.trim()) {
    current = null;
    errorRange = null;
    errorBanner.hidden = true;
    panes.classList.remove("stale");
    clearPanes();
    showMark();
    return;
  }

  let pattern;
  try {
    pattern = engine.parsePattern(text);
  } catch (e) {
    const message = messageOf(e);
    const span = parseErrorSpan(message);
    errorRange = span ? spanToRange(text, span) : null;
    errorBanner.textContent = message.replace(/^\[\w+\]\s?/, "");
    errorBanner.hidden = false;
    panes.classList.add("stale"); // keep the last good output, dimmed
    showMark();
    return;
  }

  errorRange = null;
  errorBanner.hidden = true;
  panes.classList.remove("stale");
  try {
    const ir = pattern.ir;
    const listing = pattern.irListing;
    $("ast").textContent = JSON.stringify(pattern.ast, null, 2);
    $("ir").textContent = JSON.stringify(ir, null, 2);
    $("canonical").textContent = pattern.canonical;
    renderListing(listing);
    current = { text, instrs: instructionsInOrder(ir) };
    renderGraph(ir, listing);
  } finally {
    pattern.free();
  }
  showMark();
}

async function start() {
  try {
    await init();
    engine = new Engine();
  } catch (e) {
    const banner = $("load-error");
    banner.textContent = `Couldn't load the WebAssembly module: ${messageOf(e)}`;
    banner.hidden = false;
    document.querySelector("main").hidden = true;
    return;
  }

  mermaid?.initialize({
    startOnLoad: false,
    securityLevel: "strict",
    suppressErrorRendering: true,
    theme: matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "default",
  });

  const select = $("examples");
  EXAMPLES.forEach((ex, i) => select.append(new Option(ex.label, String(i))));
  select.addEventListener("change", () => {
    input.value = EXAMPLES[Number(select.value)].pattern;
    update();
  });

  for (const b of document.querySelectorAll("[data-tab]")) {
    b.addEventListener("click", () => selectTab(b.dataset.tab));
  }
  let saved = null;
  try { saved = localStorage.getItem(TAB_KEY); } catch { /* storage unavailable */ }
  selectTab(TABS.includes(saved) ? saved : "graph");

  let timer;
  input.addEventListener("input", () => {
    errorRange = null;
    showMark();
    clearTimeout(timer);
    timer = setTimeout(update, DEBOUNCE_MS);
  });
  input.addEventListener("scroll", () => { backdrop.scrollTop = input.scrollTop; });

  $("listing").addEventListener("mouseover", (e) => {
    const k = e.target.closest?.("[data-k]")?.dataset.k;
    highlightInstruction(k === undefined ? null : current?.instrs[Number(k)]);
  });
  $("listing").addEventListener("mouseleave", showMark);
  $("graph").addEventListener("mouseover", (e) => {
    const id = nodeInstrId(e.target.closest?.("g.node"));
    highlightInstruction(id === null ? null : current?.instrs.find((i) => i.id === id));
  });
  $("graph").addEventListener("mouseleave", showMark);

  input.value = EXAMPLES[0].pattern;
  update();
}

start();
```

- [ ] **Step 4: Write `scripts/build-playground.mjs`**

```js
// Assembles the static playground into playground-dist/: the page's own files,
// the wasm-pack web build (as pkg/), and the book's vendored Mermaid.
// Run via `npm run build:playground`, which builds pkg-web/ first.
import { cpSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const from = (...parts) => join(root, ...parts);
const out = from("playground-dist");

if (!existsSync(from("pkg-web", "stix_wasm.js"))) {
  console.error("pkg-web/ is missing; run `npm run build:web` first");
  process.exit(1);
}

rmSync(out, { recursive: true, force: true });
mkdirSync(out);
cpSync(from("playground"), out, { recursive: true });
// Skip wasm-pack's npm packaging files; the page needs only the JS and wasm.
const PACKAGING = new Set([".gitignore", "package.json", "README.md"]);
cpSync(from("pkg-web"), join(out, "pkg"), {
  recursive: true,
  filter: (src) => !PACKAGING.has(basename(src)),
});
cpSync(from("..", "..", "docs", "book", "mermaid.min.js"), join(out, "mermaid.min.js"));
console.log(`playground assembled in ${out}`);
```

- [ ] **Step 5: Wire up `package.json` and `.gitignore`.** Add this to `scripts` in `package.json`:

```json
    "build:playground": "npm run build:web && node scripts/build-playground.mjs",
```

Add this line to `.gitignore`:

```
playground-dist/
```

- [ ] **Step 6: Build it and check the output**

Run:
```bash
npm run build:playground && ls playground-dist playground-dist/pkg && node --check playground-dist/main.js && node --check playground-dist/graph.js && node --check playground-dist/spans.js
```
Expected: `playground-dist/` lists `index.html playground.css main.js graph.js spans.js examples.js mermaid.min.js pkg`, and `pkg/` contains `stix_wasm.js` and `stix_wasm_bg.wasm`. `node --check` prints nothing. `npm test` still passes.

- [ ] **Step 7: Add the README section.** Add a "Playground" section to `README.md`:

```markdown
## Playground

`playground/` is a static page that parses a pattern in the browser and shows
its AST, IR listing, IR JSON, IR graph, and canonical text. It is published at
`/playground/` on the project's docs site.

To run it locally:

    npm run build:playground
    python3 -m http.server 8000 -d playground-dist

then open <http://localhost:8000>. Any static server works; it must serve
`.wasm` as `application/wasm`.
```

- [ ] **Step 8: Commit, then open PR 2**

```bash
git add playground/index.html playground/playground.css playground/main.js scripts/build-playground.mjs package.json .gitignore README.md
git commit -m "feat(playground): static pattern playground page and build script"
```

Open PR 2 with the `area:ts-wasm` label, using the PR template. Do not claim browser verification in the PR. The parent does it in Task 6.

---

### Task 6: Browser check of PR 2 (parent)

**Files:**
- Create or modify: `.claude/launch.json`, to add a `playground` configuration

**Interfaces:**
- Consumes: the PR 2 branch; `npm run build:playground`.
- Produces: a verification result for the PR 2 review. It can be approval, or specific defects sent back to `typescript-wasm-binding`.

- [ ] **Step 1: Build.** On the PR 2 branch, in `bindings/typescript-wasm`, run `npm ci && npm run build:playground`.

- [ ] **Step 2: Add the launch config and start the preview.** Add this entry to `.claude/launch.json`:

```json
{
  "name": "playground",
  "runtimeExecutable": "python3",
  "runtimeArgs": ["-m", "http.server", "8000", "-d", "bindings/typescript-wasm/playground-dist"],
  "port": 8000
}
```

Then start it with `preview_start` (name `playground`).

- [ ] **Step 3: Walk through the checklist** in the browser pane, reading the page with `read_page` / `get_page_text` and taking screenshots for visual checks:
  1. The page loads with no console errors (`read_console_messages`). The first example is shown, and the Graph tab renders an SVG.
  2. Every example in the picker renders all five tabs, and the Canonical tab matches the example text.
  3. Typing `[file:size >` shows the banner `parse error at bytes 12..12: expected a literal value` and a caret at the end of the input. The panes dim but keep their content.
  4. Hovering a listing line and a graph node highlights the matching source text. In the graph, check that `nodeInstrId` finds nodes. If Mermaid's node ids don't match `/flowchart-i(\d+)-/`, record the actual id format and send the fix back to the subagent.
  5. Switch tabs and reload: the tab choice persists.
  6. Dark mode (`resize_window` with `colorScheme: "dark"`) keeps text and graph legible.
  7. At phone width (`resize_window` preset `mobile`), the layout stacks with no horizontal page scroll.
  8. Non-ASCII input, `[file:name = 'é😀' AND file:size > 1]`: hovering `file:size` in the listing highlights exactly `file:size > 1`.

- [ ] **Step 4:** Reset the viewport with `resize_window` preset `desktop` and stop the preview. If every check passes, merge PR 1 then PR 2 (PR 1 should already be merged). Otherwise send the specific failures back to the subagent.

---

### Task 7: Publish on Pages and link from the book (PR 3, parent)

**Files:**
- Modify: `.github/workflows/docs.yml` (the `build` job)
- Modify: `docs/book/src/SUMMARY.md`
- Modify: `docs/book/src/introduction.md`
- Create: `docs/book/src/playground.md` (mdBook's SUMMARY can only link `.md` pages, so a stub page carries the link)

**Interfaces:**
- Consumes: `npm run build:playground` from Task 5.
- Produces: `<pages-url>/playground/`.

- [ ] **Step 1: Update the workflow.** In `docs.yml`, replace the toolchain step and add the playground build before "Assemble site":

```yaml
      - uses: dtolnay/rust-toolchain@stable
        with: { targets: wasm32-unknown-unknown }
      - uses: Swatinem/rust-cache@v2
      - uses: actions/setup-node@v4
        with: { node-version: 20 }
      - name: Install mdbook + mermaid
        run: cargo install mdbook mdbook-mermaid --locked
      - name: Install wasm-pack
        run: cargo install wasm-pack --locked
      - name: Build book
        run: mdbook build docs/book
      - name: Build API docs
        run: cargo doc --workspace --no-deps
      - name: Build playground
        working-directory: bindings/typescript-wasm
        run: npm ci && npm run build:playground
```

Then add this line at the end of the "Assemble site" `run` block:

```yaml
          cp -r bindings/typescript-wasm/playground-dist site/playground
```

- [ ] **Step 2: Link it from the book.** In `SUMMARY.md`, add this under the `---` separator next to the API Reference link:

```markdown
[Playground](playground.md)
```

Create `docs/book/src/playground.md`:

```markdown
# Playground

The [pattern playground](../playground/) parses a STIX pattern in your browser,
using this library compiled to WebAssembly. It shows the pattern's AST, its IR
as a listing, as JSON, and as a graph, and the canonical text rendered back
from the IR.
```

In `introduction.md`, add one sentence near the top pointing to the playground, and link it the same way (`../playground/` resolves from the book root because the book is served at the site root).

- [ ] **Step 3: Verify locally**

Run: `mdbook build docs/book && ls docs/book/book/playground.html`, then assemble the site the same way the workflow does:

```bash
rm -rf /tmp/site && mkdir -p /tmp/site && cp -r docs/book/book/* /tmp/site/ && cp -r bindings/typescript-wasm/playground-dist /tmp/site/playground
```

Serve `/tmp/site` with `python3 -m http.server`. Check that the book's Playground page link opens the playground and that the playground's `stix-rust` link returns to the book. Use the scratchpad directory instead of `/tmp` if the session provides one.

- [ ] **Step 4: Commit and open PR 3**

```bash
git add .github/workflows/docs.yml docs/book/src/SUMMARY.md docs/book/src/introduction.md docs/book/src/playground.md
git commit -m "docs: build and publish the pattern playground on GitHub Pages"
```

After merge, confirm the `docs` workflow run succeeds and that `<pages-url>/playground/` loads.
