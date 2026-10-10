# Playground Bundle Graph Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the pattern playground graph-first: draw the *Hackers* STIX bundle with `@poietic-tech/graphing-library`, light up a pattern's matches with a spotlight-and-trace, and show object details and the report narrative, with citation links, in a side panel.

**Architecture:**
- **Rust:** `stix-ffi`'s `MatchOutcome` gains matched `observed_data_ids`, which the wasm binding exposes as `observedDataIds`.
- **Pure ES modules in `playground/`:**
  - `bundle-graph.js` turns objects into nodes and edges;
  - `trace.js` turns a match into tiers;
  - `render.js` renders Markdown and links citations.
  Each is tested with vitest against the real dataset.
- **Thin DOM and engine modules:**
  - `graph-view.js` wraps the graphing library;
  - `details.js` drives the side panel.
  `main.js` wires them into a restructured page.
- **Build:** the build script vendors the graphing library and `marked` from npm devDependencies, and copies the dataset from `datasets/`, so the page needs no bundler and no runtime CDN.

**Tech Stack:** Rust (`stix-ffi`), wasm-bindgen, plain HTML/CSS/ES modules, `@poietic-tech/graphing-library` ~0.9.0 (WebGL2), `marked` ^18, vitest.

**Spec:** `docs/superpowers/specs/2026-10-10-playground-bundle-graph-design.md`

## Global Constraints

- **Area boundaries (AGENTS.md):**
  - Task 1 edits only `crates/**` (rust-core).
  - Tasks 2–7 edit only `bindings/typescript-wasm/**` (ts-wasm).
  - Task 8 is the parent's.
  - `datasets/**` is read-only for everyone here; the build only copies it.
- No bundler and no runtime CDN. Third-party code is vendored at build time from npm devDependencies into `playground-dist/vendor/`.
- `@poietic-tech/graphing-library` is pinned `~0.9.0`; `marked` is pinned `^18.1.0`.
- **Edge key format, shared by `bundle-graph.js` and `trace.js`:** `` `${source}|${target}|${label}` ``. `label` is the `relationship_type` for relationships and the property name (e.g. `object_refs`) for references.
- **Container references are excluded from the graph.** That means `object_refs` on `report`, `grouping`, `note` and `opinion`, plus `created_by_ref` and `object_marking_refs` everywhere. This is the same rule as `crates/stix/tests/datasets.rs`.
- Every bundle-derived string inserted into the DOM is HTML-escaped. Markdown's raw HTML is escaped, never rendered.
- **Tier rendering:**
  - glow: gold `#f5b301`, radius 14;
  - trace: the type colour, radius 11;
  - dim: opacity 0.2;
  - highlighted edges: gold, width 2.5, label shown;
  - other edges: opacity 0.12.
- The page must work without the graph (WebGL2 missing, or `?nograph` in the URL): the pattern bar, the details panel and the inspector still function.
- **Working tree:** it holds the maintainer's unrelated uncommitted edits (an owner rename to poietic-tech in about 10 files). Never stage them: `git add` named files only, never `-A` or `.`.
- Commits carry no attribution trailers.

## Review Focus

1. **A pattern typed before the graph finishes mounting.** The tiers from the latest match must still appear once the graph is ready. Pinned in Task 6: `graph-view.js` stores tiers and applies them after mount; Task 8 browser-checks typing immediately after load.
2. **A bundle reference to an id that is not a node** (a dangling ref, or a reference to a relationship). It must be dropped, not passed to the library, which rejects dangling edges. Pinned in Task 3 (`drops edges to unknown or relationship ids`).
3. **Raw HTML or a `<script>` in a description.** It must render as escaped text. Pinned in Task 5 (`escapes raw HTML`).
4. **Non-ASCII labels and narrative text** (é, emoji). They must render and not break citation linking. Pinned in Task 5 (`non-ASCII text survives`); labels are checked in Task 8.
5. **The graph unavailable.** The rest of the page must keep working, with a visible fallback message. Pinned in Task 7 (`?nograph` forces the fallback) and checked in Task 8.

---

### Task 1: `MatchOutcome::observed_data_ids` (rust-core, PR 1)

**Files:**
- Modify: `crates/stix-ffi/src/handles.rs` (the `MatchOutcome` struct and the `match_outcome_fields` test)
- Modify: `crates/stix-ffi/src/engine.rs` (`Engine::match_bundle`)
- Test: in `crates/stix-ffi/src/engine.rs` `mod tests`, or `crates/stix-ffi/tests/facade.rs`, wherever `match_bundle` is already tested

**Interfaces:**
- Produces: `pub observed_data_ids: Vec<String>` on `stix_ffi::MatchOutcome`. These are the matched `observed-data` ids, the same length and order as `observations`; entry *k* is the id of the `observations[k]`-th `observed-data` object in bundle order.

Branch: `git fetch origin && git checkout -b feat/ffi-observed-data-ids origin/main`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn match_bundle_reports_observed_data_ids_in_bundle_order() {
    let bundle_json = r#"{"type":"bundle","id":"bundle--00000000-0000-4000-8000-000000000000","objects":[
        {"type":"domain-name","spec_version":"2.1","id":"domain-name--00000000-0000-4000-8000-000000000001","value":"a.example"},
        {"type":"domain-name","spec_version":"2.1","id":"domain-name--00000000-0000-4000-8000-000000000002","value":"b.example"},
        {"type":"observed-data","spec_version":"2.1","id":"observed-data--00000000-0000-4000-8000-00000000000a",
         "created":"1995-01-01T00:00:00.000Z","modified":"1995-01-01T00:00:00.000Z",
         "first_observed":"1995-01-01T00:00:00.000Z","last_observed":"1995-01-01T00:00:00.000Z",
         "number_observed":1,"object_refs":["domain-name--00000000-0000-4000-8000-000000000001"]},
        {"type":"observed-data","spec_version":"2.1","id":"observed-data--00000000-0000-4000-8000-00000000000b",
         "created":"1995-01-01T00:00:00.000Z","modified":"1995-01-01T00:00:00.000Z",
         "first_observed":"1995-01-01T00:00:00.000Z","last_observed":"1995-01-01T00:00:00.000Z",
         "number_observed":1,"object_refs":["domain-name--00000000-0000-4000-8000-000000000002"]}
    ]}"#;
    let engine = Engine::new();
    let bundle = engine.parse_bundle(bundle_json).unwrap();
    let hit_second = engine.parse_pattern("[domain-name:value = 'b.example']").unwrap();
    let out = engine.match_bundle(&hit_second, &bundle).unwrap();
    assert!(out.matched);
    assert_eq!(out.observations, vec![1]);
    assert_eq!(out.observed_data_ids, vec!["observed-data--00000000-0000-4000-8000-00000000000b".to_string()]);

    let miss = engine.parse_pattern("[domain-name:value = 'c.example']").unwrap();
    let out = engine.match_bundle(&miss, &bundle).unwrap();
    assert!(!out.matched);
    assert!(out.observed_data_ids.is_empty());
}
```

Also update the existing `match_outcome_fields` test in `handles.rs` so it constructs the struct with the new field: `observed_data_ids: vec![]`.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-ffi match_bundle_reports_observed_data_ids_in_bundle_order`
Expected: compile error, `no field observed_data_ids`.

- [ ] **Step 3: Implement**

In `handles.rs`:

```rust
/// The outcome of a match: whether it matched and which observations bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchOutcome {
    /// Whether the pattern matched.
    pub matched: bool,
    /// The indices of the observations that participated in the match.
    pub observations: Vec<u64>,
    /// The ids of those observations' `observed-data` objects: entry *k* is the
    /// id of the `observations[k]`-th `observed-data` in bundle order.
    pub observed_data_ids: Vec<String>,
}
```

In `engine.rs` `match_bundle`:

```rust
        let result = stix::matcher::match_bundle(pattern.inner(), bundle.inner())?;
        // Same filter, same order as stix_matcher::match_bundle, so index i agrees.
        let observed: Vec<&str> = bundle
            .inner()
            .objects
            .iter()
            .filter_map(|o| match o {
                StixObject::Typed(TypedObject::ObservedData(od)) => Some(od.common.id.as_str()),
                _ => None,
            })
            .collect();
        let observations: Vec<u64> = result.observations().iter().map(|&i| i as u64).collect();
        let observed_data_ids = result
            .observations()
            .iter()
            .filter_map(|&i| observed.get(i).map(|s| s.to_string()))
            .collect();
        Ok(MatchOutcome { matched: result.is_match(), observations, observed_data_ids })
```

Import `StixObject` and `TypedObject` via the `stix` umbrella re-exports, as the existing ffi code does. If the id field on `ObservedData` is not `common.id`, use whatever `stix_model::ObservedData` exposes (check `crates/stix-model/src/sdo.rs`). The filter must stay identical to `crates/stix-matcher/src/lib.rs` `match_bundle`, or the indices will disagree.

- [ ] **Step 4: Run the tests and workspace checks**

Run: `cargo test -p stix-ffi && cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings`
Expected: all pass.

Then confirm the bindings still compile. They only read `MatchOutcome`:
`cargo check --manifest-path bindings/typescript-wasm/Cargo.toml`, and the same for `bindings/typescript-node`, `bindings/python` and `bindings/java/rust`.

- [ ] **Step 5: Commit, push, and open PR 1**

```bash
git add crates/stix-ffi/src/handles.rs crates/stix-ffi/src/engine.rs
git commit -m "feat(ffi): report matched observed-data ids on MatchOutcome"
git push -u origin feat/ffi-observed-data-ids
```

Open the PR with labels `area:rust-core` and `type:feat`, titled "ffi: matched observed-data ids on MatchOutcome", using the PR template.

---

### Task 2: wasm `observedDataIds` (ts-wasm, PR 2)

**Files:**
- Modify: `bindings/typescript-wasm/src/lib.rs` (`MatchResult` and `Engine::match_bundle`)
- Modify: `bindings/typescript-wasm/ts/index.ts` (the `MatchResult` wrapper)
- Test: `bindings/typescript-wasm/tests/stix.test.ts`

**Interfaces:**
- Consumes: `stix_ffi::MatchOutcome::observed_data_ids` from Task 1.
- Produces:
  - raw wasm `MatchResult.observedDataIds: string[]` (getter, `js_name = observedDataIds`);
  - TS wrapper getter `get observedDataIds(): string[]`.

Branch: `git checkout -b feat/playground-bundle-graph feat/ffi-observed-data-ids`. PR 2 is stacked on PR 1; Tasks 2–7 all go on this branch.

- [ ] **Step 1: Write the failing test** (append inside the existing `describe` in `tests/stix.test.ts`)

```ts
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
```

Add `import fs from "node:fs";` and `import path from "node:path";` to the file's imports. If `__dirname` is not available in this vitest config, derive it with `fileURLToPath(new URL(".", import.meta.url))`. The same rule applies to Tasks 3 and 4.

- [ ] **Step 2: Run it and verify it fails**

Run: `npm test` in `bindings/typescript-wasm`
Expected: FAIL. `observedDataIds` is undefined.

- [ ] **Step 3: Implement**

In `src/lib.rs`, add `observed_data_ids: Vec<String>` to `MatchResult`. Fill it in `match_bundle` from `o.observed_data_ids`. Add:

```rust
    #[wasm_bindgen(getter, js_name = observedDataIds)]
    pub fn observed_data_ids(&self) -> Vec<String> {
        self.observed_data_ids.clone()
    }
```

In `ts/index.ts`, add this to the wrapper `MatchResult`:

```ts
  get observedDataIds(): string[] { return Array.from(this.raw.observedDataIds); }
```

- [ ] **Step 4: Run the tests and confirm they pass**

Run: `npm test` (all pass), then `cargo clippy --manifest-path Cargo.toml -- -D warnings`.

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs ts/index.ts tests/stix.test.ts
git commit -m "feat(wasm): MatchResult.observedDataIds"
```

---

### Task 3: `bundle-graph.js` (ts-wasm)

**Files:**
- Create: `bindings/typescript-wasm/playground/bundle-graph.js`
- Test: `bindings/typescript-wasm/tests/playground-bundle-graph.test.ts`

**Interfaces:**
- Produces:
  - `refsOf(value) → Array<[property: string, id: string]>`, collected at any depth;
  - `isContainerRef(holderType, property) → boolean`;
  - `labelOf(object) → string`;
  - `familyOf(stixType) → "actor"|"capability"|"target"|"detection"|"context"|"observable"`;
  - `bundleToGraph(objects) → { nodes: Array<{id, label, attrs: {stix_id, stix_type, family}}>, edges: Array<{id, source, target, attrs: {key, label}}> }`. Here `id === attrs.key === \`${source}|${target}|${label}\``.

- [ ] **Step 1: Write the failing test**

```ts
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
```

- [ ] **Step 2: Run it and verify it fails**

Run: `npx vitest run tests/playground-bundle-graph.test.ts` after `npm test` has built `dist/`.
Expected: FAIL, because `../playground/bundle-graph.js` doesn't exist.

- [ ] **Step 3: Implement** `playground/bundle-graph.js`

```js
// Turns a STIX bundle's objects into the node/edge graph the bundle view draws.
// Relationships become edges; every other *_ref / *_refs property becomes an
// edge from its holder, except container references — the same rule the
// dataset validator (crates/stix/tests/datasets.rs) uses — so a report or
// grouping does not become a hub.

const CONTAINER_TYPES = new Set(["report", "grouping", "note", "opinion"]);

const FAMILIES = {
  actor: ["threat-actor", "intrusion-set", "campaign"],
  capability: ["malware", "tool", "attack-pattern", "vulnerability", "course-of-action"],
  target: ["identity", "location", "infrastructure"],
  detection: ["indicator", "sighting", "observed-data", "malware-analysis", "incident"],
  context: ["report", "grouping", "note", "opinion"],
};

/** Every [property, id] reference in `value`, at any depth, in document order. */
export function refsOf(value) {
  const out = [];
  const walk = (v) => {
    if (Array.isArray(v)) {
      v.forEach(walk);
    } else if (v && typeof v === "object") {
      for (const [k, x] of Object.entries(v)) {
        if (k.endsWith("_ref") && typeof x === "string") out.push([k, x]);
        else if (k.endsWith("_refs") && Array.isArray(x)) {
          for (const s of x) if (typeof s === "string") out.push([k, s]);
        }
        walk(x);
      }
    }
  };
  walk(value);
  return out;
}

/** Whether a reference only lists contents rather than adding structure. */
export function isContainerRef(holderType, property) {
  return (
    property === "created_by_ref" ||
    property === "object_marking_refs" ||
    (property === "object_refs" && CONTAINER_TYPES.has(holderType))
  );
}

/** The text a node shows. */
export function labelOf(o) {
  if (typeof o.name === "string") return o.name;
  if (typeof o.value === "string") return o.value;
  if (o.type === "observed-data" && o.first_observed) return `observed-data ${o.first_observed.slice(0, 10)}`;
  if (o.type === "sighting" && o.first_seen) return `sighting ${o.first_seen.slice(0, 10)}`;
  if (typeof o.abstract === "string") return o.abstract;
  if (o.type === "artifact" && o.mime_type) return `artifact (${o.mime_type})`;
  return o.type;
}

/** The colour/shape family for a STIX type. */
export function familyOf(type) {
  for (const [family, types] of Object.entries(FAMILIES)) if (types.includes(type)) return family;
  return "observable";
}

/** `{ nodes, edges }` in the graphing library's `load()` shape. */
export function bundleToGraph(objects) {
  const nodes = [];
  for (const o of objects) {
    if (o.type === "relationship") continue;
    nodes.push({ id: o.id, label: labelOf(o), attrs: { stix_id: o.id, stix_type: o.type, family: familyOf(o.type) } });
  }
  const nodeIds = new Set(nodes.map((n) => n.id));
  const edges = [];
  const seen = new Set();
  const addEdge = (source, target, label) => {
    if (source === target || !nodeIds.has(source) || !nodeIds.has(target)) return;
    const key = `${source}|${target}|${label}`;
    if (seen.has(key)) return;
    seen.add(key);
    edges.push({ id: key, source, target, attrs: { key, label } });
  };
  for (const o of objects) {
    if (o.type === "relationship") {
      addEdge(o.source_ref, o.target_ref, o.relationship_type);
      continue;
    }
    for (const [property, target] of refsOf(o)) {
      if (!isContainerRef(o.type, property)) addEdge(o.id, target, property);
    }
  }
  return { nodes, edges };
}
```

- [ ] **Step 4: Run the tests and confirm they pass**

Run: `npm test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add playground/bundle-graph.js tests/playground-bundle-graph.test.ts
git commit -m "feat(playground): bundle → graph model"
```

---

### Task 4: `trace.js` (ts-wasm)

**Files:**
- Create: `bindings/typescript-wasm/playground/trace.js`
- Test: `bindings/typescript-wasm/tests/playground-trace.test.ts`

**Interfaces:**
- Consumes: `refsOf` and `isContainerRef` from Task 3; the TS wrapper's `observedDataIds` from Task 2 (test only).
- Produces: `traceMatch(objects, matchedObservedIds) → { nodes: Map<id, "glow"|"trace">, edges: Set<edgeKey> }`. Objects not in `nodes` are dimmed by the caller. Edge keys use the Global Constraints format.

- [ ] **Step 1: Write the failing test**

```ts
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
```

- [ ] **Step 2: Run it and verify it fails**

Run: `npm test`
Expected: FAIL, because the module is missing.

- [ ] **Step 3: Implement** `playground/trace.js`

```js
// Spotlight-and-trace for a pattern match. The matched observed-data and what
// they reference glow; the trail outward — sightings of them, the indicators
// sighted, what those indicate or are based on, and one further hop of
// attribution, authorship or use — is traced. Everything else dims.

import { refsOf, isContainerRef } from "./bundle-graph.js";

const FROM_INDICATOR = new Set(["indicates", "based-on"]);
const ONWARD = new Set(["attributed-to", "authored-by", "uses"]);

export function traceMatch(objects, matchedObservedIds) {
  const byId = new Map(objects.map((o) => [o.id, o]));
  const nodes = new Map();
  const edges = new Set();
  const isNode = (id) => byId.has(id) && byId.get(id).type !== "relationship";
  const mark = (id, tier) => {
    if (isNode(id) && nodes.get(id) !== "glow") nodes.set(id, tier);
  };
  const edge = (source, target, label) => {
    if (isNode(source) && isNode(target) && source !== target) edges.add(`${source}|${target}|${label}`);
  };

  // Glow: the matched observed-data, the SCOs they reference, and those SCOs'
  // own structural references (e.g. a file's content_ref artifact).
  const matched = new Set(matchedObservedIds.filter((id) => byId.get(id)?.type === "observed-data"));
  for (const odId of matched) {
    mark(odId, "glow");
    for (const sco of byId.get(odId).object_refs ?? []) {
      if (!isNode(sco)) continue;
      mark(sco, "glow");
      edge(odId, sco, "object_refs");
      for (const [property, target] of refsOf(byId.get(sco))) {
        if (!isContainerRef(byId.get(sco).type, property) && isNode(target)) {
          mark(target, "glow");
          edge(sco, target, property);
        }
      }
    }
  }

  // Trace: sightings of the matched observed-data → indicators → targets → one hop.
  const indicators = new Set();
  for (const s of objects) {
    if (s.type !== "sighting") continue;
    const hits = (s.observed_data_refs ?? []).filter((r) => matched.has(r));
    if (hits.length === 0) continue;
    mark(s.id, "trace");
    for (const r of hits) edge(s.id, r, "observed_data_refs");
    if (isNode(s.sighting_of_ref)) {
      mark(s.sighting_of_ref, "trace");
      edge(s.id, s.sighting_of_ref, "sighting_of_ref");
      indicators.add(s.sighting_of_ref);
    }
  }
  const relationships = objects.filter((o) => o.type === "relationship");
  const targets = new Set();
  for (const r of relationships) {
    if (indicators.has(r.source_ref) && FROM_INDICATOR.has(r.relationship_type) && isNode(r.target_ref)) {
      mark(r.target_ref, "trace");
      edge(r.source_ref, r.target_ref, r.relationship_type);
      targets.add(r.target_ref);
    }
  }
  for (const r of relationships) {
    if (targets.has(r.source_ref) && ONWARD.has(r.relationship_type) && isNode(r.target_ref)) {
      mark(r.target_ref, "trace");
      edge(r.source_ref, r.target_ref, r.relationship_type);
    }
  }
  return { nodes, edges };
}
```

- [ ] **Step 4: Run the tests and confirm they pass**

Run: `npm test`
Expected: all pass.

If the "one hop beyond" test fails because the dataset's sighted indicator lacks an `indicates` relationship, stop and report it. The dataset is out of scope here, so don't change it.

- [ ] **Step 5: Commit**

```bash
git add playground/trace.js tests/playground-trace.test.ts
git commit -m "feat(playground): spotlight-and-trace tiers for a match"
```

---

### Task 5: `render.js`, Markdown and citations (ts-wasm)

**Files:**
- Modify: `bindings/typescript-wasm/package.json` (devDependency `"marked": "^18.1.0"`; regenerate the lockfile with `npx -y npm@10 install`)
- Create: `bindings/typescript-wasm/playground/render.js`
- Test: `bindings/typescript-wasm/tests/playground-render.test.ts`

**Interfaces:**
- Produces:
  - `escapeHtml(text) → string`;
  - `linkCitations(markdown, knownIds: Set<string>) → string`. Each known `type--uuid`, with or without surrounding backticks, becomes `` [`id`](#obj:id) ``;
  - `renderMarkdown(markdown, MarkedCtor, knownIds) → html string`, with raw HTML escaped.
- `MarkedCtor` is the `Marked` class. Tests import it from `marked`; the page imports it from the vendored copy (Task 6).

- [ ] **Step 1: Write the failing test**

```ts
import { describe, it, expect } from "vitest";
import { Marked } from "marked";
import { escapeHtml, linkCitations, renderMarkdown } from "../playground/render.js";

const ID = "threat-actor--11111111-2222-4333-8444-555555555555";
const OTHER = "malware--11111111-2222-4333-8444-555555555556";
const known = new Set([ID]);

describe("render", () => {
  it("links known ids, with or without backticks", () => {
    expect(linkCitations(`*Zero Cool* (\`${ID}\`) and ${ID}`, known)).toBe(
      `*Zero Cool* ([\`${ID}\`](#obj:${ID})) and [\`${ID}\`](#obj:${ID})`,
    );
  });

  it("leaves unknown ids alone", () => {
    expect(linkCitations(`\`${OTHER}\``, known)).toBe(`\`${OTHER}\``);
  });

  it("renders Markdown with citation links", () => {
    const html = renderMarkdown(`## Cast\n\n*Zero Cool* (\`${ID}\`)`, Marked, known);
    expect(html).toContain("<h2");
    expect(html).toContain(`<a href="#obj:${ID}"><code>${ID}</code></a>`);
  });

  it("escapes raw HTML", () => {
    const html = renderMarkdown(`hello <script>alert(1)</script> <b>x</b>`, Marked, known);
    expect(html).not.toContain("<script>");
    expect(html).not.toContain("<b>");
    expect(html).toContain("&lt;script&gt;");
  });

  it("non-ASCII text survives", () => {
    const html = renderMarkdown(`Café 😀 — (\`${ID}\`)`, Marked, known);
    expect(html).toContain("Café 😀");
    expect(html).toContain(`#obj:${ID}`);
  });

  it("escapes HTML special characters", () => {
    expect(escapeHtml(`<a href="x">'&'</a>`)).toBe("&lt;a href=&quot;x&quot;&gt;&#39;&amp;&#39;&lt;/a&gt;");
  });
});
```

- [ ] **Step 2: Run it and verify it fails**

Run: `npm test`
Expected: FAIL, because the module is missing.

- [ ] **Step 3: Implement** `playground/render.js`

```js
// Markdown for the details panel. STIX id citations become links that focus the
// cited object (#obj:<id>); raw HTML in the source is escaped, never rendered.

const ID_RE = /`?([a-z][a-z0-9-]*--[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})`?/g;

const ESCAPES = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" };

export function escapeHtml(text) {
  return String(text).replace(/[&<>"']/g, (c) => ESCAPES[c]);
}

export function linkCitations(markdown, knownIds) {
  return markdown.replace(ID_RE, (match, id) => (knownIds.has(id) ? `[\`${id}\`](#obj:${id})` : match));
}

export function renderMarkdown(markdown, MarkedCtor, knownIds) {
  const marked = new MarkedCtor({
    gfm: true,
    renderer: {
      html: (token) => escapeHtml(typeof token === "string" ? token : token.text),
    },
  });
  return marked.parse(linkCitations(markdown ?? "", knownIds));
}
```

marked's renderer `html` signature differs between major versions. This code handles both the string form and the token form. If marked 18 needs `{ async: false }` for a synchronous `parse`, add it.

- [ ] **Step 4: Run the tests and confirm they pass**

Run: `npm test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add package.json package-lock.json playground/render.js tests/playground-render.test.ts
git commit -m "feat(playground): Markdown rendering with citation links"
```

---

### Task 6: Vendoring and `graph-view.js` (ts-wasm)

**Files:**
- Modify: `bindings/typescript-wasm/package.json` (devDependency `"@poietic-tech/graphing-library": "~0.9.0"`; refresh the lockfile with npm 10)
- Modify: `bindings/typescript-wasm/scripts/build-playground.mjs`
- Create: `bindings/typescript-wasm/playground/graph-view.js`

**Interfaces:**
- Consumes: `bundleToGraph` output (Task 3); trace tiers (Task 4).
- Produces:
  - The build output layout:
    - `playground-dist/vendor/graphing-library/{dist,pkg}/` (both copied whole, preserving the relative import `dist/mount.js` → `../pkg/graph_explorer_wasm.js`);
    - `playground-dist/vendor/marked/marked.esm.js` (from `node_modules/marked/lib/marked.esm.js`);
    - `playground-dist/datasets/hackers-1995/{bundle.json,patterns.json}` (from the repo's `datasets/`).
  - `createGraphView(canvas, graph, { onNodeClick, dark }) → Promise<{ setTiers(t|null), focus(id), destroy() }>`. It rejects if the library or WebGL2 is unavailable.

- [ ] **Step 1: Extend the build script**

Add to `scripts/build-playground.mjs`, after the existing copies:

```js
const vendor = join(out, "vendor");
const lib = from("node_modules", "@poietic-tech", "graphing-library");
for (const sub of ["dist", "pkg"]) {
  if (!existsSync(join(lib, sub))) {
    console.error(`@poietic-tech/graphing-library/${sub} is missing; run npm ci`);
    process.exit(1);
  }
  cpSync(join(lib, sub), join(vendor, "graphing-library", sub), {
    recursive: true,
    filter: (src) => !src.endsWith(".map") && !src.endsWith(".d.ts"),
  });
}
mkdirSync(join(vendor, "marked"), { recursive: true });
cpSync(from("node_modules", "marked", "lib", "marked.esm.js"), join(vendor, "marked", "marked.esm.js"));
cpSync(from("..", "..", "datasets", "hackers-1995"), join(out, "datasets", "hackers-1995"), {
  recursive: true,
  filter: (src) => !src.endsWith("README.md"),
});
```

Don't filter `.d.ts` from `pkg/` if `graph_explorer_wasm.js` needs a sibling file. Only `.wasm` and `.js` are needed at runtime, so check what `pkg/` contains and keep those.

- [ ] **Step 2: Write `playground/graph-view.js`**

```js
// The bundle graph: mounts @poietic-tech/graphing-library on a canvas, loads the
// bundle graph, styles nodes by STIX type family, and draws match tiers. The
// canvas-sizing recipe follows the library's docs/BROWSER.md.

const FAMILY_COLOURS = {
  actor: "#e05a5a", capability: "#9b6bd6", target: "#4aa3df",
  detection: "#e08a3c", context: "#8a8f98", observable: "#2bb5a5",
};
const FAMILY_SHAPES = {
  actor: "diamond", capability: "square", target: "circle",
  detection: "square", context: "circle", observable: "circle",
};
const GLOW = "#f5b301";

export { FAMILY_COLOURS, FAMILY_SHAPES };

const pixelRatio = () => Math.min(window.devicePixelRatio || 1, 2);

function sizeCanvas(canvas) {
  const ratio = pixelRatio();
  const w = Math.round((canvas.clientWidth || window.innerWidth || 1280) * ratio);
  const h = Math.round((canvas.clientHeight || window.innerHeight || 720) * ratio);
  if (canvas.width === w && canvas.height === h) return false;
  canvas.width = w;
  canvas.height = h;
  return true;
}

export async function createGraphView(canvas, graph, { onNodeClick, dark }) {
  const { mountGraph } = await import("./vendor/graphing-library/dist/mount.js");
  const { attachPointer } = await import("./vendor/graphing-library/dist/pointer.js");

  sizeCanvas(canvas);
  const client = await mountGraph(canvas.id);
  client.setPixelRatio(pixelRatio());
  client.setStyle({
    node_base: { color: FAMILY_COLOURS.context, radius: 9, shape: "circle", label_visible: true },
    node_rules: Object.entries(FAMILY_COLOURS).map(([family, color]) => ({
      when: { attr: "family", equals: family },
      set: { color, shape: FAMILY_SHAPES[family] },
    })),
    edge_base: { color: dark ? "#5d6670" : "#a3abb5", width: 1.2, label_attr: "label", label_visible: false },
  });
  client.setBackground(dark ? "#0d1117" : "#f6f8fa");
  client.load(JSON.stringify(graph));
  client.fitView();
  const pointer = attachPointer(client, canvas, {
    onNodeClick: (hit) => { if (hit.kind === "node") onNodeClick?.(hit.id); },
  });
  const apply = () => {
    if (sizeCanvas(canvas)) client.resize(canvas.width, canvas.height);
    client.setPixelRatio(pixelRatio());
  };
  const ro = new ResizeObserver(apply);
  ro.observe(canvas);
  window.addEventListener("resize", apply);
  client.start();

  let tiers = null;
  const applyTiers = () => {
    client.setNodeStyler(
      tiers
        ? (attrs) => {
            const t = tiers.nodes.get(attrs.stix_id);
            if (t === "glow") return { color: GLOW, radius: 14, opacity: 1 };
            if (t === "trace") return { radius: 11, opacity: 1 };
            return { opacity: 0.2 };
          }
        : null,
    );
    client.setEdgeStyler(
      tiers
        ? (attrs) =>
            tiers.edges.has(attrs.key)
              ? { color: GLOW, width: 2.5, opacity: 1, label_visible: true }
              : { opacity: 0.12 }
        : null,
    );
  };

  return {
    /** Show match tiers; `null` or an empty trace clears highlighting. */
    setTiers(t) {
      tiers = t && t.nodes.size > 0 ? t : null;
      applyTiers();
    },
    focus(id) {
      client.selectId(id);
      client.focus(id);
    },
    destroy() {
      ro.disconnect();
      window.removeEventListener("resize", apply);
      pointer.detach();
      client.stop();
    },
  };
}
```

The calls are taken from the library's 0.9.0 `client.d.ts`, `pointer.d.ts` and `docs/BROWSER.md`. If a call differs in the installed version (for example the `attachPointer` argument order, or whether re-registering a styler triggers a re-render), adapt the call, not the behaviour, and record it in your report.

If re-registering stylers doesn't restyle, look in `client.d.ts` for a re-render or dirty call and use it.

- [ ] **Step 3: Build and smoke-check**

Run: `npm ci && npm run build:playground && ls playground-dist/vendor/graphing-library/dist playground-dist/vendor/graphing-library/pkg playground-dist/vendor/marked playground-dist/datasets/hackers-1995`
Expected: `mount.js`, `pointer.js` and `client.js` in `dist/`; the `.js` and `.wasm` files in `pkg/`; `marked.esm.js`; and `bundle.json` plus `patterns.json`.

Run `node --check playground-dist/graph-view.js`.

- [ ] **Step 4: Commit**

```bash
git add package.json package-lock.json scripts/build-playground.mjs playground/graph-view.js
git commit -m "feat(playground): vendor the graphing library and marked; graph view"
```

---

### Task 7: Page restructure (ts-wasm), then open PR 2

**Files:**
- Modify: `bindings/typescript-wasm/playground/index.html`
- Modify: `bindings/typescript-wasm/playground/playground.css`
- Modify: `bindings/typescript-wasm/playground/main.js`
- Create: `bindings/typescript-wasm/playground/details.js`
- Modify: `bindings/typescript-wasm/README.md` (Playground section)

**Interfaces:**
- Consumes:
  - `bundleToGraph`, `labelOf` (Task 3); `traceMatch` (Task 4);
  - `renderMarkdown`, `escapeHtml` (Task 5);
  - `createGraphView`, `FAMILY_COLOURS`, `FAMILY_SHAPES` (Task 6);
  - the raw wasm `MatchResult.observedDataIds` (Task 2).
- Produces: the finished page.

- [ ] **Step 1: Restructure `index.html` (graph-first layout)**

Keep every existing id that `main.js` and the inspector use: `pattern`, `backdrop`, `parse-error`, `panes`, `graph`, `listing`, `ast`, `ir`, `canonical`, the tab ids, `examples` and `load-error`.

The new structure, top to bottom:
1. **`<header>`** with the title "STIX Pattern Playground" and the subtitle "Hackers (1995) — a STIX 2.1 dataset, explored in WebAssembly".
2. **`<section class="pattern-bar">`**, containing:
   - the existing editor (`#backdrop` and `#pattern`);
   - the existing `#parse-error` banner;
   - a new `<p id="match-status" role="status">`;
   - a new `<div id="chips" class="chips" aria-label="Example patterns">`.
3. **`<main class="explorer">`**, containing:
   - `<div class="graph-wrap"><canvas id="bundle-graph" aria-label="Object graph of the Hackers STIX bundle"></canvas><p id="graph-fallback" hidden></p></div>`;
   - `<aside id="details" class="details" aria-live="polite"></aside>`.
4. **`<details class="inspector">`** with `<summary>Inspector — IR graph, listing, AST, IR JSON, canonical</summary>`. Inside: the existing `#examples` select, labelled "Inspector examples"; the existing tablist, with the first tab's text renamed "IR graph"; and `#panes` with all five panes, unchanged.

- [ ] **Step 2: CSS**

- `.explorer` is a grid: `minmax(0,1fr) minmax(280px, 380px)`.
- `.graph-wrap` has `height: min(70vh, 640px)`, and the canvas fills it at `width:100%;height:100%;display:block`.
- `.details` has `max-height: min(70vh, 640px); overflow:auto`.
- At 900px and below, `.explorer` becomes one column and `.details` loses its max-height.
- Style `.chips` as wrapping buttons, `#match-status` as small muted text, and the `.legend` swatches. Keep the existing tokens, dark-mode block and `[hidden]` rule.
- The page must never scroll horizontally at 375px.

- [ ] **Step 3: Write `details.js`**

```js
// The details panel: the report narrative by default; an object's details when
// one is selected. Links of the form #obj:<id> focus that object.

import { renderMarkdown, escapeHtml } from "./render.js";
import { labelOf } from "./bundle-graph.js";
import { FAMILY_COLOURS, FAMILY_SHAPES } from "./graph-view.js";

const KEY_PROPS = [
  "aliases", "roles", "threat_actor_types", "sophistication", "resource_level", "primary_motivation",
  "malware_types", "is_family", "tool_types", "infrastructure_types", "identity_class", "sectors",
  "region", "country", "pattern", "pattern_type", "valid_from", "indicator_types", "value", "hashes",
  "mime_type", "size", "first_observed", "last_observed", "number_observed", "first_seen", "last_seen",
  "count", "published", "report_types", "context", "opinion", "abstract",
];

export function createDetails(panel, objects, { Marked, onFocus }) {
  const byId = new Map(objects.map((o) => [o.id, o]));
  const known = new Set(byId.keys());
  const report = objects.find((o) => o.type === "report");
  const link = (id) =>
    byId.has(id) ? `<a href="#obj:${escapeHtml(id)}">${escapeHtml(labelOf(byId.get(id)))}</a>` : escapeHtml(id);
  const md = (text) => renderMarkdown(text ?? "", Marked, known);
  const value = (v) =>
    Array.isArray(v) ? v.map((x) => escapeHtml(String(x))).join(", ")
    : v && typeof v === "object" ? Object.entries(v).map(([k, x]) => `${escapeHtml(k)}: ${escapeHtml(String(x))}`).join("<br>")
    : escapeHtml(String(v));

  const legend = () =>
    `<div class="legend">${Object.entries(FAMILY_COLOURS)
      .map(([f, c]) => `<span><i style="background:${c}" data-shape="${FAMILY_SHAPES[f]}"></i>${f}</span>`)
      .join("")}</div>`;

  function connections(id) {
    const out = [];
    for (const o of objects) {
      if (o.type === "relationship") {
        if (o.source_ref === id) out.push(`<li>${escapeHtml(o.relationship_type)} → ${link(o.target_ref)}</li>`);
        if (o.target_ref === id) out.push(`<li>${link(o.source_ref)} → ${escapeHtml(o.relationship_type)}</li>`);
      }
    }
    return out.length ? `<h4>Relationships</h4><ul>${out.join("")}</ul>` : "";
  }

  function showReport() {
    panel.innerHTML = report
      ? `<h2>${escapeHtml(report.name)}</h2>${md(report.description)}${legend()}`
      : `<p>No report in this bundle.</p>${legend()}`;
  }

  function show(id) {
    const o = byId.get(id);
    if (!o) return showReport();
    if (o.type === "report") return showReport();
    const props = KEY_PROPS.filter((k) => o[k] !== undefined)
      .map((k) => `<tr><th>${escapeHtml(k)}</th><td>${value(o[k])}</td></tr>`).join("");
    const refs = Object.entries(o)
      .filter(([k, v]) => (k.endsWith("_ref") && typeof v === "string") || (k.endsWith("_refs") && Array.isArray(v)))
      .map(([k, v]) => `<tr><th>${escapeHtml(k)}</th><td>${[].concat(v).map(link).join(", ")}</td></tr>`).join("");
    panel.innerHTML = `
      <p><a href="#report" class="back">← Back to report</a></p>
      <p class="type">${escapeHtml(o.type)}</p>
      <h2>${escapeHtml(labelOf(o))}</h2>
      ${o.description ? md(o.description) : ""}
      ${props || refs ? `<table>${props}${refs}</table>` : ""}
      ${connections(id)}
      <p class="id"><code>${escapeHtml(o.id)}</code></p>`;
  }

  panel.addEventListener("click", (e) => {
    const a = e.target.closest?.("a[href]");
    if (!a) return;
    const href = a.getAttribute("href");
    if (href === "#report") { e.preventDefault(); showReport(); return; }
    if (href.startsWith("#obj:")) { e.preventDefault(); const id = href.slice(5); show(id); onFocus?.(id); }
  });

  return { showReport, show };
}
```

- [ ] **Step 4: Wire `main.js`**

Keep all existing pattern-inspector behaviour. Make these changes:
- **Imports:** `bundleToGraph` from `./bundle-graph.js`, `traceMatch` from `./trace.js`, `createGraphView` from `./graph-view.js`, `createDetails` from `./details.js`, and `{ Marked }` from `./vendor/marked/marked.esm.js`.
- **New module state:** `let bundle = null, objects = [], graphView = null, details = null, lastTiers = null;`
- **In `start()`**, after `init()` and `new Engine()`:
  1. Fetch `datasets/hackers-1995/bundle.json` and `patterns.json`. On failure, show `#load-error` with "Couldn't load the Hackers dataset: …" and continue, so the pattern inspector still works.
  2. `bundle = engine.parseBundle(text)` and `objects = JSON.parse(text).objects`.
  3. `details = createDetails($("details"), objects, { Marked, onFocus: (id) => graphView?.focus(id) })`, then `details.showReport()`.
  4. Render one `<button>` per patterns.json entry into `#chips`, using its `name` as the text. A click sets `input.value = p.pattern` and calls `update()`.
  5. Load the graph:
     - If `new URLSearchParams(location.search).has("nograph")`, skip the graph and show the fallback.
     - Otherwise: `try { graphView = await createGraphView($("bundle-graph"), bundleToGraph(objects), { onNodeClick: (id) => details.show(id), dark: matchMedia("(prefers-color-scheme: dark)").matches }); graphView.setTiers(lastTiers); } catch (e) { showGraphFallback(e); }`.
     - `showGraphFallback` hides the canvas and shows `#graph-fallback` with "This browser can't run the graph view (WebGL2 unavailable). The pattern tools and report still work."
  6. Set the first chip's pattern as the initial input value instead of `EXAMPLES[0]`. Keep the inspector `#examples` select populated from `EXAMPLES`.
- **In `update()`**, after a successful parse and before `pattern.free()`:

```js
    if (bundle) {
      try {
        const r = engine.matchBundle(pattern, bundle);
        const ids = Array.from(r.observedDataIds);
        r.free();
        lastTiers = traceMatch(objects, ids);
        $("match-status").textContent = ids.length
          ? `${ids.length} observation${ids.length === 1 ? "" : "s"} matched`
          : "no observations matched";
      } catch (e) {
        lastTiers = null;
        $("match-status").textContent = messageOf(e).replace(/^\[\w+\]\s?/, "");
      }
      graphView?.setTiers(lastTiers);
    }
```

- **Clearing the highlight:** on empty input, or a parse error, set `lastTiers = null`, call `graphView?.setTiers(null)` and clear `#match-status`.

- [ ] **Step 5: Build, test, and update the README**

Run: `npm test && npm run build:playground`
Expected: all tests pass and the build succeeds. `node --check` passes on every `playground-dist/*.js` file.

Update the README's Playground section:
- the graph-first layout;
- the Hackers dataset;
- the graph needs WebGL2, and `?nograph` disables it;
- the graphing library and `marked` are vendored from devDependencies.

- [ ] **Step 6: Commit, push, and open PR 2**

```bash
git add playground/index.html playground/playground.css playground/main.js playground/details.js README.md
git commit -m "feat(playground): graph-first bundle explorer"
git push -u origin feat/playground-bundle-graph
```

Open the PR against `feat/ffi-observed-data-ids`, which is stacked on PR 1. Use labels `area:ts-wasm` and `type:feat`, and the title "playground: graph-first Hackers bundle explorer".

In the body, note:
- that it is stacked on PR 1;
- that browser verification is pending with the parent;
- the vendoring.

---

### Task 8: Browser check (parent)

**Files:** `.claude/launch.json` is local only and never committed. It already ignores the `playground` entry, which serves `bindings/typescript-wasm/playground-dist` on port 8000.

- [ ] **Step 1:** On the PR 2 branch, run `cd bindings/typescript-wasm && npm ci && npm run build:playground`. Then run `preview_start` with the name `playground`.
- [ ] **Step 2:** Check each of the following:
  1. The page loads with no console errors. The graph draws 40 nodes, coloured by family. The panel shows the report and the legend.
  2. **Every chip** produces a match status. Its matched observed-data and SCOs glow, the indicator-backed chips show a trace, and the no-match chip un-dims the graph.
  3. Clicking a node opens its details. A relationship link there focuses the related node. "Back to report" returns to the report.
  4. A citation link in the report focuses its node and opens its details.
  5. Typing a pattern immediately after load still highlights once the graph is ready.
  6. The inspector opens, all five tabs work, and the IR graph renders.
  7. Dark mode.
  8. At 375 px the layout stacks with no horizontal scroll.
  9. `?nograph` shows the fallback, and chips, status and panel still work.
  10. Non-ASCII labels render, for example "Ramón Sánchez" in a Cast entry.
- [ ] **Step 3:** Stop the preview and reset the viewport. Report the failures, with specifics, to the implementer for fixes.
