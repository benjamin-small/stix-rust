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
