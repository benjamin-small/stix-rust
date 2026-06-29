# stix-rust — TypeScript binding (WebAssembly)

> **Status: planned.** This area is scaffolded; the binding is not yet implemented.

Portable WebAssembly bindings for the [stix-rust](../../README.md) toolkit — runs in
the browser and in Node.

- **Toolchain:** [wasm-bindgen](https://rustwasm.github.io/wasm-bindgen/) + npm
- **Surface:** typed handles (`Engine`, `Pattern`, `Bundle`, `MatchResult`) with JSON
  for deep structure, wrapping the `stix-ffi` facade.
- **Owner agent:** `typescript-wasm-binding`
- **Sibling:** a native Node addon lives in
  [`../typescript-node`](../typescript-node/README.md); both expose the same TS API
  shape.

## Planned usage

```ts
import init, { Engine } from "@stix-rust/wasm";

await init();
const engine = new Engine();
const pattern = engine.parsePattern("[ipv4-addr:value = '198.51.100.1']");
```

## Build & test (once implemented)

```bash
wasm-pack build --target web
npm test
```
