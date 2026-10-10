# stix-rust — TypeScript binding (WebAssembly)

Portable WebAssembly bindings for the [stix-rust](../../README.md) toolkit — runs in
Node and the browser.

- **Package:** `@stix-rust/wasm`
- **Surface:** identical to `@stix-rust/node` — typed handles (`Engine`, `Pattern`,
  `Bundle`, `MatchResult`); native JS objects; `StixError` hierarchy.

## Install

```bash
npm install @stix-rust/wasm
```

## Build & test (from source) (Node)

```bash
cd bindings/typescript-wasm
npm install
npm run build      # wasm (--target nodejs) + TypeScript wrapper -> dist/
npm test           # vitest (in Node)
```

## Browser build

```bash
npm run build:web  # wasm-pack --target web -> pkg-web/
```

In the browser, initialize the module before use (per wasm-pack's web target), then
use the same `Engine`/`Pattern`/`Bundle`/`MatchResult` API.

## Usage (Node)

```ts
import { Engine } from "@stix-rust/wasm";

const engine = new Engine();
const pattern = engine.parsePattern("[ipv4-addr:value = '198.51.100.5']");
console.log(pattern.ast);       // parsed AST as a plain JS object
console.log(pattern.ir);        // lowered IR (schema_version, blocks, main) as a plain JS object
console.log(pattern.irListing); // human-readable listing of the IR blocks
console.log(pattern.canonical); // canonical (re-rendered, normalized) pattern text

const bundle = engine.parseBundle(json);
const result = engine.matchBundle(pattern, bundle);
console.log(result.matched, result.observations);

engine.registerType("x-acme-widget", (obj) => ({
  ...obj,
  risk_band: obj.risk_score > 80 ? "high" : "low",
}));
```

Errors are `StixError` subclasses: `ParseError`, `ModelError`, `MatchError`,
`ValidationError`.

## Playground

`playground/` is a graph-first static page that explores the Hackers (1995)
STIX 2.1 dataset. It draws the bundle's objects as a graph, shows the report
narrative and per-object details beside it, and highlights the observations,
and the objects behind them, that a pattern you type matches. Example patterns
appear as chips. A collapsible Inspector below shows the pattern's IR graph,
IR listing, AST, IR JSON, and canonical text. It is published at `/playground/`
on the project's docs site.

The graph view needs WebGL2. Where it is unavailable, or when the page is
opened with `?nograph`, the graph is replaced by a notice and the report and
pattern tools keep working. The graphing library and `marked` are vendored into
`playground-dist/` at build time from devDependencies; the dataset is copied
from `datasets/hackers-1995/`.

To run it locally:

    npm run build:playground
    python3 -m http.server 8000 -d playground-dist

then open <http://localhost:8000>. Any static server works; it must serve
`.wasm` as `application/wasm`.
