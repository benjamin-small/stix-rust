# stix-rust — TypeScript binding (Node)

Native Node.js bindings for the [stix-rust](../../README.md) toolkit, via napi-rs.

- **Package:** `@stix-rust/node`
- **Surface:** typed handles (`Engine`, `Pattern`, `Bundle`, `MatchResult`); deep
  structure (AST, objects) as native JS objects; `StixError` hierarchy.

## Install

```bash
npm install @stix-rust/node
```

## Build & test (from source)

```bash
cd bindings/typescript-node
npm install
npm run build      # native addon + TypeScript wrapper -> dist/
npm test           # vitest
```

## Usage

```ts
import { Engine } from "@stix-rust/node";

const engine = new Engine();
const pattern = engine.parsePattern("[ipv4-addr:value = '198.51.100.5']");
console.log(pattern.ast);                     // AST as an object

const bundle = engine.parseBundle(json);
console.log(bundle.objectCount(), [...bundle].map((o) => o.type));

const result = engine.matchBundle(pattern, bundle);
console.log(result.matched, result.observations);
```

### Custom object types

```ts
engine.registerType("x-acme-widget", (obj) => ({
  ...obj,
  risk_band: obj.risk_score > 80 ? "high" : "low",
}));
```

Hooks run at `parseBundle` time; throwing raises `ValidationError`. Errors are
`StixError` subclasses: `ParseError`, `ModelError`, `MatchError`, `ValidationError`.
Passing the wrong kind of object (e.g. `matchBundle(bundle, pattern)`) also raises
`ValidationError`.

## Internals

`binding.js` is the raw napi layer and is not a public API. It exports plain
functions over opaque `External` handles (`createEngine`, `parsePattern`,
`matchBundle`, ...) rather than `#[napi]` classes: napi-rs 2.x does not
type-check class instances on unwrap, while `External<T>` checks the Rust type
on every call, so a wrong handle is a `[validation]` error instead of a crash.
