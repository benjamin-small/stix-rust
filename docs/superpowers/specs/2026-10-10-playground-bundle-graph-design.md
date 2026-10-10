# Playground bundle graph — design

**Date:** 2026-10-10
**Status:** Approved in conversation; pending written-spec review
**Builds on:** `2026-09-26-pattern-playground-design.md` (the playground) and
`2026-10-10-hackers-dataset-design.md` (the dataset).

## Goal

Turn the pattern playground into a **showcase**: the *Hackers* (1995) STIX 2.1
dataset is preloaded and drawn as an interactive object graph, the hero of the
page. Typing (or clicking) a pattern runs it against the bundle and lights up
what it matched and the story behind the match. Clicking an object shows its
details; the dataset's wiki-style report becomes a guided tour of the graph.

## Non-goals

- Loading a user's own bundle (paste/upload). The page shows the Hackers
  dataset only.
- Large-bundle tooling (search, filtering, performance work for thousands of
  objects). Showcase bundles are tens to low hundreds of objects.
- Changing the existing IR inspector tabs' behaviour.
- A non-WebGL2 graph renderer (a fallback *message* only).

## Audience and success

For visitors to the docs site who want to see what stix-rust does. Success:
on load, the Hackers graph is visible and explorable; every example chip
produces a visible spotlight-and-trace; clicking any node or any id cited in
the report focuses it and shows its details; nothing about the existing
pattern-inspection tools regresses.

## Layout (graph-first)

```
┌ header: STIX Pattern Playground · Hackers (1995) ───────────────────────┐
│ pattern input ………………………………………………………  [parse/match status]          │
│ example chips: [Ellingson domain] [file SHA-256] [AND] [OR] [no match]… │
├──────────────────────────────────────────────┬──────────────────────────┤
│                                              │ Details panel            │
│              bundle graph (hero)             │  default: the report     │
│        WebGL2 canvas, pan/zoom/select        │  narrative (Markdown);   │
│                                              │  on node click: details  │
├──────────────────────────────────────────────┴──────────────────────────┤
│ ▸ Inspector (collapsed by default): Graph · IR listing · AST · IR JSON · │
│   Canonical — today's tabs, unchanged                                   │
└─────────────────────────────────────────────────────────────────────────┘
```

- The existing IR "Graph" tab (Mermaid) stays inside the inspector and is
  labelled **"IR graph"** there, to distinguish it from the bundle graph.
- Phone width: the details panel stacks below the graph; the page never scrolls
  horizontally.
- Light and dark themes as today.

## Data flow

### Loading the dataset

- The playground build (`npm run build:playground`, `scripts/build-playground.mjs`)
  copies `datasets/hackers-1995/bundle.json` and `patterns.json` from the repo
  root into `playground-dist/datasets/hackers-1995/`, the same way it already
  reads the book's `mermaid.min.js`. The page never ships a hand-copied
  dataset; what is published is exactly what CI validated.
- On start, the page `fetch`es both files and parses the bundle once with
  `engine.parseBundle`. The parsed `Bundle` is kept for matching; the raw JSON is
  kept for the graph and the details panel.

### Bundle → graph (`playground/bundle-graph.js`, pure)

`bundleToGraph(objects) → { nodes, edges }`:

- **Nodes:** every object except `relationship`. Sightings are nodes, so a trace
  can pass through them. Node attributes: `id`, `stix_type`, `label` (the
  object's `name`; SCOs by `value`, `name` or hash; otherwise `abstract`, or
  the type), and `tier` (see matching).
- **Edges:**
  - each `relationship` is one edge `source_ref → target_ref`, labelled with
    `relationship_type`;
  - each other `*_ref` / `*_refs` property, at any depth, is an edge from the
    holder to the target, labelled with the property name, **except container
    references**: `object_refs` on `report`, `grouping`, `note` and `opinion`,
    plus `created_by_ref` and `object_marking_refs` everywhere. This is the same
    rule the dataset validator uses, so the report does not become a hub.
  - Edges to ids not in the bundle are dropped.
- Type styling is declarative: one style rule per STIX type family
  (threat-actor/intrusion-set/campaign, malware/tool/attack-pattern,
  identity/location/infrastructure, indicator/sighting/observed-data, SCOs,
  report/grouping/note/opinion), each a distinct colour. The library offers
  three shapes (circle, square, diamond), so shapes repeat across families as a
  secondary cue; colour is what tells families apart. The legend in the details
  panel's default view explains them.

### Matching

- Each pattern change (typed, debounced as today, or a chip click) runs
  `engine.matchBundle(pattern, bundle)`.
- **New API:** `MatchResult.observedDataIds` returns the ids of the matched
  `observed-data`, mapped from the matcher's observation indices in bundle
  order. It is added in `stix-ffi` (Rust) and exposed by the wasm binding
  (raw layer and TS wrapper) so no consumer re-derives the index convention.
  Other bindings may adopt it later.
- A pattern that fails to parse keeps today's error banner and underline, and
  leaves the graph un-highlighted.

### Spotlight and trace (`playground/trace.js`, pure)

`traceMatch(objects, matchedObservedIds) → Map<id, tier>`. Tiers:

- **glow:** each matched `observed-data`, plus every object in its `object_refs`
  (the SCOs that matched), plus those SCOs' own non-container references, so a
  file's `content_ref` artifact glows too;
- **trace:** sightings whose `observed_data_refs` include a matched
  observed-data → each sighting's `sighting_of_ref` (the indicator) → targets of
  that indicator's `indicates` and `based-on` relationships → one further hop
  from those targets along `attributed-to`, `authored-by` and `uses`. Edges on
  these paths are drawn as trace edges;
- **dim:** everything else.

With no match, every node is undimmed and the status reads
"no observations matched". With an empty pattern, likewise.

The graph library's per-node and per-edge stylers (`setNodeStyler` /
`setEdgeStyler`) read the current tier map. The library has no outline styles, so
the tiers are drawn as:

- glow: gold, largest radius;
- trace: the type colour, enlarged;
- dim: about 20% opacity.

Glow and trace edges are gold and wider, with their relationship label shown;
other edges are faint. (Amended 2026-10-10: the design discussion said "halo"
and "dashed outline", which the library does not offer.)

## Details panel

- **Default view:** the report. Its `description` is rendered as Markdown, and
  every cited `type--uuid` in it becomes a link that focuses and selects that
  node and opens its details. The legend appears under the report.
- **Node view**, on click or a citation link:
  - the type, the name or label, the description (rendered as Markdown), and a
    short table of key properties per type (for example `aliases`, `roles`,
    `primary_motivation`, `pattern`, `value`, `hashes`, `first_observed`);
  - incoming and outgoing relationships and references, each a link that
    focuses the related object;
  - a "Back to report" control.
- **Markdown:** rendered with `marked`, vendored at build time from an npm
  devDependency. Raw HTML in the source is escaped, not rendered, so only
  Markdown formatting reaches the DOM.

## Example chips

The chips replace today's example dropdown. One chip per `patterns.json` entry,
labelled with its `name`. Clicking a chip fills the pattern input and runs it.
Today's pattern-inspector examples, which are not about the dataset, stay
available in the inspector drawer.

## The graphing library

- `@poietic-tech/graphing-library`, the maintainer's library, pinned `~0.9.0`
  as a devDependency of `bindings/typescript-wasm`. The build copies its `dist/`
  and `pkg/` into `playground-dist/vendor/graphing-library/` with their relative
  layout intact (`dist/mount.js` imports `../pkg/graph_explorer_wasm.js`), so no
  bundler is needed.
- Use: `mountGraph(canvasId)`, `load(json)`, `setStyle(...)` plus node and edge
  stylers for tiers, `attachPointer(client, canvas, { onNodeClick })`,
  `selectId` / `focus`, `fitView`, `start`.
- **WebGL2 required.** If `mountGraph` fails, the graph area shows "This browser
  can't run the graph view (WebGL2 unavailable)". The pattern bar, details panel
  (report) and inspector keep working.
- Pre-1.0: upgrades are deliberate version bumps, verified in the browser.

## Error handling

| Situation | Behaviour |
| --- | --- |
| Dataset fetch or parse fails | Banner "Couldn't load the Hackers dataset: …"; the pattern inspector still works |
| WebGL2 / graph mount fails | Fallback message in the graph area; everything else works |
| Pattern parse error | Existing banner and underline; graph un-highlighted |
| Pattern matches nothing | Status "no observations matched"; graph un-dimmed |
| Match error (e.g. unsupported qualifier) | Status shows the error; graph un-highlighted |

## Testing

- **Rust (rust-core):** a `stix-ffi` unit test that `MatchOutcome::observed_data_ids` maps
  indices to ids in bundle order, using a bundle with at least two
  observed-data and a pattern matching only the second.
- **wasm binding (ts-wasm):**
  - a vitest for `observedDataIds` on the Hackers bundle;
  - vitest suites for the pure modules:
    - `bundle-graph.js`: node and edge counts on the Hackers bundle, container
      refs excluded, sightings as nodes, edge labels;
    - `trace.js`: for each `patterns.json` entry, the expected glow set, a
      non-empty trace for the indicator-backed patterns, empty tiers for the
      no-match pattern;
    - the citation linker: every cited id becomes a link, and nothing that
      isn't an id does.
- **Browser check (parent):**
  - the graph renders and every chip shows a spotlight and trace;
  - node click opens details, and citation links focus nodes;
  - "Back to report" works;
  - the inspector still works;
  - dark mode, phone width, and the WebGL2 fallback (simulated by forcing the
    mount to fail).

## Ownership and delivery

Two PRs, in order:

1. **rust-core:** `stix_ffi::MatchOutcome` gains a field
   `observed_data_ids: Vec<String>` (the matched observed-data ids, in the same
   order as `observations`), filled by `Engine::match_bundle`, with a test.
2. **ts-wasm:**
   - the `observedDataIds` getter;
   - the page restructure, `bundle-graph.js`, `trace.js` and the details panel;
   - the build-script copies of `datasets/`, `marked` and the graphing library;
   - the tests above;
   - the README playground section updated.

The docs workflow already runs `npm run build:playground`, so publishing needs
no workflow change. The dataset stays parent-owned and is only read by the
build.
