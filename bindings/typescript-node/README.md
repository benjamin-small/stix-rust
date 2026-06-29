# stix-rust — TypeScript binding (Node)

> **Status: planned.** This area is scaffolded; the binding is not yet implemented.

Native Node.js bindings for the [stix-rust](../../README.md) toolkit.

- **Toolchain:** [napi-rs](https://napi.rs) + npm (prebuilt native addon)
- **Surface:** typed handles (`Engine`, `Pattern`, `Bundle`, `MatchResult`) with JSON
  for deep structure, wrapping the `stix-ffi` facade.
- **Owner agent:** `typescript-node-binding`
- **Sibling:** a portable WebAssembly build lives in
  [`../typescript-wasm`](../typescript-wasm/README.md); both expose the same TS API
  shape.

## Planned usage

```ts
import { Engine } from "@stix-rust/node";

const engine = new Engine();
const pattern = engine.parsePattern("[ipv4-addr:value = '198.51.100.1']");
const bundle = engine.parseBundle(json);
console.log(engine.matchBundle(pattern, bundle).matched);
```

## Build & test (once implemented)

```bash
npm install
npm run build      # napi build
npm test
```
