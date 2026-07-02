# TypeScript (wasm)

`@stix-rust/wasm` — a portable WebAssembly build (wasm-bindgen) that runs in
**Node and the browser**. The API is identical to
[`@stix-rust/node`](typescript-node.md); on the web target the module is
initialized asynchronously first.

## Install

```bash
npm install @stix-rust/wasm
```

From source instead:
`cd bindings/typescript-wasm && npm install && npm run build`
(Node target; `npm run build:web` produces the browser build.)

## Worked example

```ts
import { Engine, ParseError, ValidationError } from "@stix-rust/wasm";

const engine = new Engine();

// 1. parse a pattern; the AST is a plain object
const pattern = engine.parsePattern("[ipv4-addr:value = '198.51.100.5']");
console.log(pattern.ast.expression);

// 2. import a bundle; iterate its objects
const bundle = engine.parseBundle(json);
console.log(bundle.objectCount(), [...bundle].map((o) => o.type));
bundle.object(99); // undefined when out of range

// 3. match — hit and miss
const result = engine.matchBundle(pattern, bundle);
console.log(result.matched, result.observations);
const miss = engine.parsePattern("[ipv4-addr:value = '203.0.113.9']");
console.assert(!engine.matchBundle(miss, bundle).matched);

// 4. custom type with a computed property
engine.registerType("x-acme-widget", (obj) => ({
  ...obj,
  risk_band: obj.risk_score > 80 ? "high" : "low",
}));
const banded = engine.parseBundle(widgetBundleJson);
const hit = engine.parsePattern("[x-acme-widget:risk_band = 'high']");
console.assert(engine.matchBundle(hit, banded).matched);
```

## Errors

| Error class | Thrown by |
| --- | --- |
| `ParseError` | invalid pattern syntax |
| `ModelError` | invalid JSON / not a bundle |
| `MatchError` | matching failure |
| `ValidationError` | a `registerType` hook threw |

All extend `StixError` (which carries a `.code`). Hooks run at `parseBundle` time;
throwing inside one rejects the object.

## Notes

- **Browser use:** build with the web target and initialize the module before use
  (per wasm-pack's `--target web` conventions); after init the API above is the same.
- Everything runs synchronously and single-threaded inside the wasm module — fine
  for the parse/import/match workload.
