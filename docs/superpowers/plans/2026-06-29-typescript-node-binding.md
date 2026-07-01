# TypeScript Node Binding (SP3) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `@stix-rust/node` under `bindings/typescript-node/` — a native Node addon (napi-rs) wrapping the `stix-ffi` facade, with a hand-written TypeScript wrapper presenting the shared TS surface (`Engine`/`Pattern`/`Bundle`/`MatchResult`, native JS objects, `StixError` hierarchy, `registerType`).

**Architecture:** A napi-rs cdylib crate (excluded from the root Cargo workspace) exposes raw handle classes and throws `"[code] message"` errors. A thin TypeScript wrapper adds the error subclass hierarchy, an iterable `Bundle`, and `registerType` (applied JS-side at `parseBundle` time). Deep structure crosses as native JS objects via napi's `serde-json` feature.

**Tech Stack:** Rust + napi 2 (`serde-json` feature) + napi-derive; `@napi-rs/cli`; TypeScript; vitest. Node ≥ 18.

---

## ⚠️ Toolchain-adaptation note for the implementing agent

The **public API, TS wrapper logic, and tests below are exact** — implement them as
written. The **build glue** (napi CLI flags, generated-binding filenames, tsconfig
`outDir`, module resolution) varies by `@napi-rs/cli` version; adapt those to what
your installed toolchain actually produces, keeping the published API and passing
tests unchanged. Prereqs (install if missing): Node ≥ 18, and
`npm install` inside the package. Use WebFetch on napi.rs docs if a CLI flag differs.
If the Node toolchain cannot run here at all, STOP and report (CI will build) rather
than faking green.

**Refinement vs spec:** the `registerType` hook is applied in the TypeScript wrapper
at `parseBundle` time (transform matching objects, then delegate to raw
`parseBundle`), not stored inside the Rust `Engine`. Same behavior (computed props →
data; thrown hook → `ValidationError`); the raw Rust layer therefore has no
`register_type`.

## File Structure

```
bindings/typescript-node/
├── Cargo.toml            # cdylib; napi + napi-derive; dep stix-ffi(path)
├── build.rs              # napi_build::setup()
├── package.json          # @stix-rust/node; @napi-rs/cli, typescript, vitest
├── tsconfig.json
├── src/lib.rs            # #[napi] raw Engine/Pattern/Bundle/MatchResult
├── ts/errors.ts          # StixError hierarchy + parseCode helper
├── ts/index.ts           # wrapper: Engine/Pattern/Bundle/MatchResult
├── tests/stix.test.ts    # vitest
└── README.md
```
Root `Cargo.toml` `[workspace] exclude` gains `"bindings/typescript-node"`.

---

## Task 1: Scaffold the crate + npm package

**Files:**
- Modify: `Cargo.toml` (root)
- Create: `bindings/typescript-node/Cargo.toml`, `build.rs`, `package.json`, `tsconfig.json`
- Create: `bindings/typescript-node/src/lib.rs` (minimal)

- [ ] **Step 1: Exclude from the workspace**

In root `Cargo.toml`, extend the `exclude` list under `[workspace]`:

```toml
exclude = ["bindings/python", "bindings/typescript-node"]
```

- [ ] **Step 2: Crate manifest + build script**

Create `bindings/typescript-node/Cargo.toml`:

```toml
[package]
name = "stix-node"
version = "0.0.1"
edition = "2021"
license = "MIT OR Apache-2.0"

[lib]
crate-type = ["cdylib"]

[dependencies]
stix-ffi = { path = "../../crates/stix-ffi" }
napi = { version = "2", default-features = false, features = ["napi6", "serde-json"] }
napi-derive = "2"
serde_json = "1"

[build-dependencies]
napi-build = "2"
```

Create `bindings/typescript-node/build.rs`:

```rust
fn main() {
    napi_build::setup();
}
```

- [ ] **Step 3: package.json + tsconfig**

Create `bindings/typescript-node/package.json`:

```json
{
  "name": "@stix-rust/node",
  "version": "0.0.1",
  "main": "dist/index.js",
  "types": "dist/index.d.ts",
  "napi": { "name": "stix-node" },
  "files": ["dist", "*.node"],
  "scripts": {
    "build:native": "napi build --platform --release --js binding.js --dts binding.d.ts",
    "build:ts": "tsc",
    "build": "npm run build:native && npm run build:ts",
    "test": "napi build --platform --js binding.js --dts binding.d.ts && tsc && vitest run"
  },
  "devDependencies": {
    "@napi-rs/cli": "^2.18.0",
    "typescript": "^5.4.0",
    "vitest": "^2.0.0"
  }
}
```

Create `bindings/typescript-node/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "CommonJS",
    "declaration": true,
    "outDir": "dist",
    "rootDir": "ts",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true
  },
  "include": ["ts/**/*.ts"]
}
```

- [ ] **Step 4: Minimal Rust module**

Create `bindings/typescript-node/src/lib.rs`:

```rust
//! Native Node bindings for the stix-rust toolkit (raw napi layer).
#![deny(clippy::all)]

use napi_derive::napi;

#[napi]
pub fn _healthcheck() -> bool {
    true
}
```

- [ ] **Step 5: Verify it builds and the core workspace is unaffected**

Run: `cd bindings/typescript-node && npm install && npm run build:native`
Expected: produces a `.node` addon + `binding.js`/`binding.d.ts` with no errors.

Run (repo root): `cargo test 2>&1 | grep -c "test result: ok"`
Expected: non-zero — the excluded crate doesn't affect the workspace.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml bindings/typescript-node/
git commit -m "feat(ts-node): scaffold napi crate and npm package"
```

---

## Task 2: Raw napi layer (handles + error tagging)

**Files:**
- Modify: `bindings/typescript-node/src/lib.rs`

- [ ] **Step 1: Implement the raw classes**

Replace `bindings/typescript-node/src/lib.rs` with:

```rust
//! Native Node bindings for the stix-rust toolkit (raw napi layer).
//!
//! Errors are thrown as `"[code] message"`; the TypeScript wrapper maps the code
//! prefix onto the StixError subclass hierarchy.
#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;

fn map_err(e: stix_ffi::FfiError) -> Error {
    let code = match e.code {
        stix_ffi::ErrorCode::Parse => "parse",
        stix_ffi::ErrorCode::Model => "model",
        stix_ffi::ErrorCode::Match => "match",
        stix_ffi::ErrorCode::Validation => "validation",
    };
    Error::from_reason(format!("[{code}] {}", e.message))
}

fn json_err(e: serde_json::Error) -> Error {
    Error::from_reason(format!("[model] {e}"))
}

#[napi]
pub struct Pattern {
    inner: stix_ffi::Pattern,
}

#[napi]
impl Pattern {
    #[napi(getter)]
    pub fn ast(&self) -> Result<serde_json::Value> {
        serde_json::from_str(&self.inner.to_json()).map_err(json_err)
    }
}

#[napi]
pub struct Bundle {
    inner: stix_ffi::Bundle,
}

#[napi]
impl Bundle {
    #[napi]
    pub fn object_count(&self) -> u32 {
        self.inner.object_count() as u32
    }

    #[napi]
    pub fn object(&self, index: u32) -> Result<Option<serde_json::Value>> {
        match self.inner.object_json(index as usize) {
            Some(json) => Ok(Some(serde_json::from_str(&json).map_err(json_err)?)),
            None => Ok(None),
        }
    }
}

#[napi]
pub struct MatchResult {
    inner_matched: bool,
    inner_observations: Vec<u32>,
}

#[napi]
impl MatchResult {
    #[napi(getter)]
    pub fn matched(&self) -> bool {
        self.inner_matched
    }

    #[napi(getter)]
    pub fn observations(&self) -> Vec<u32> {
        self.inner_observations.clone()
    }
}

#[napi]
pub struct Engine {
    inner: stix_ffi::Engine,
}

#[napi]
impl Engine {
    #[napi(constructor)]
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Engine {
            inner: stix_ffi::Engine::new(),
        }
    }

    #[napi]
    pub fn parse_pattern(&self, src: String) -> Result<Pattern> {
        self.inner
            .parse_pattern(&src)
            .map(|inner| Pattern { inner })
            .map_err(map_err)
    }

    #[napi]
    pub fn parse_bundle(&self, json: String) -> Result<Bundle> {
        self.inner
            .parse_bundle(&json)
            .map(|inner| Bundle { inner })
            .map_err(map_err)
    }

    #[napi]
    pub fn match_bundle(&self, pattern: &Pattern, bundle: &Bundle) -> Result<MatchResult> {
        self.inner
            .match_bundle(&pattern.inner, &bundle.inner)
            .map(|o| MatchResult {
                inner_matched: o.matched,
                inner_observations: o.observations.iter().map(|&i| i as u32).collect(),
            })
            .map_err(map_err)
    }
}
```

- [ ] **Step 2: Build to verify it compiles and generates types**

Run: `cd bindings/typescript-node && npm run build:native`
Expected: builds; `binding.d.ts` now declares `Engine`, `Pattern`, `Bundle`,
`MatchResult` with the methods above.

- [ ] **Step 3: Commit**

```bash
git add bindings/typescript-node/src/lib.rs
git commit -m "feat(ts-node): raw napi Engine/Pattern/Bundle/MatchResult with tagged errors"
```

---

## Task 3: TypeScript wrapper (errors + public API + hooks)

**Files:**
- Create: `bindings/typescript-node/ts/errors.ts`
- Create: `bindings/typescript-node/ts/index.ts`

- [ ] **Step 1: Write the error hierarchy**

Create `bindings/typescript-node/ts/errors.ts`:

```ts
export type StixErrorCode = "parse" | "model" | "match" | "validation";

export class StixError extends Error {
  readonly code: StixErrorCode;
  constructor(code: StixErrorCode, message: string) {
    super(message);
    this.code = code;
    this.name = new.target.name;
  }
}
export class ParseError extends StixError {
  constructor(message: string) { super("parse", message); }
}
export class ModelError extends StixError {
  constructor(message: string) { super("model", message); }
}
export class MatchError extends StixError {
  constructor(message: string) { super("match", message); }
}
export class ValidationError extends StixError {
  constructor(message: string) { super("validation", message); }
}

const CODE_RE = /^\[(parse|model|match|validation)\]\s?/;

/** Map a raw error (message "[code] msg") to the matching StixError subclass. */
export function toStixError(err: unknown): StixError {
  const raw = err instanceof Error ? err.message : String(err);
  const m = CODE_RE.exec(raw);
  const message = m ? raw.replace(CODE_RE, "") : raw;
  switch (m?.[1]) {
    case "parse": return new ParseError(message);
    case "match": return new MatchError(message);
    case "validation": return new ValidationError(message);
    default: return new ModelError(message);
  }
}
```

- [ ] **Step 2: Write the wrapper**

Create `bindings/typescript-node/ts/index.ts`:

```ts
import {
  Engine as RawEngine,
  Pattern as RawPattern,
  Bundle as RawBundle,
  MatchResult as RawMatchResult,
} from "../binding.js";
import {
  StixError,
  ParseError,
  ModelError,
  MatchError,
  ValidationError,
  toStixError,
} from "./errors.js";

export { StixError, ParseError, ModelError, MatchError, ValidationError };

export class Pattern {
  /** @internal */ readonly raw: RawPattern;
  /** @internal */ constructor(raw: RawPattern) { this.raw = raw; }
  get ast(): any {
    try { return this.raw.ast; } catch (e) { throw toStixError(e); }
  }
}

export class Bundle {
  /** @internal */ readonly raw: RawBundle;
  /** @internal */ constructor(raw: RawBundle) { this.raw = raw; }
  objectCount(): number { return this.raw.objectCount(); }
  object(index: number): any | undefined {
    const v = this.raw.object(index);
    return v === null ? undefined : v;
  }
  *[Symbol.iterator](): Iterator<any> {
    const n = this.raw.objectCount();
    for (let i = 0; i < n; i++) yield this.object(i);
  }
}

export class MatchResult {
  /** @internal */ readonly raw: RawMatchResult;
  /** @internal */ constructor(raw: RawMatchResult) { this.raw = raw; }
  get matched(): boolean { return this.raw.matched; }
  get observations(): number[] { return this.raw.observations; }
}

export type CustomHook = (obj: any) => any;

export class Engine {
  #raw: RawEngine;
  #hooks = new Map<string, CustomHook>();

  constructor() { this.#raw = new RawEngine(); }

  parsePattern(src: string): Pattern {
    try { return new Pattern(this.#raw.parsePattern(src)); }
    catch (e) { throw toStixError(e); }
  }

  parseBundle(json: string): Bundle {
    // Apply registered hooks JS-side, then delegate to the raw parser.
    let text = json;
    if (this.#hooks.size > 0) {
      let doc: any;
      try { doc = JSON.parse(json); }
      catch (e) { throw new ModelError(`invalid JSON: ${(e as Error).message}`); }
      const objects = Array.isArray(doc?.objects) ? doc.objects : [];
      for (let i = 0; i < objects.length; i++) {
        const hook = this.#hooks.get(objects[i]?.type);
        if (hook) {
          try { objects[i] = hook(objects[i]); }
          catch (e) { throw new ValidationError((e as Error).message ?? String(e)); }
        }
      }
      text = JSON.stringify(doc);
    }
    try { return new Bundle(this.#raw.parseBundle(text)); }
    catch (e) { throw toStixError(e); }
  }

  matchBundle(pattern: Pattern, bundle: Bundle): MatchResult {
    try { return new MatchResult(this.#raw.matchBundle(pattern.raw, bundle.raw)); }
    catch (e) { throw toStixError(e); }
  }

  registerType(typeName: string, hook: CustomHook): void {
    this.#hooks.set(typeName, hook);
  }
}
```

- [ ] **Step 3: Build to verify the wrapper compiles**

Run: `cd bindings/typescript-node && npm run build`
Expected: `dist/index.js` + `dist/index.d.ts` produced, no TS errors.

- [ ] **Step 4: Commit**

```bash
git add bindings/typescript-node/ts/
git commit -m "feat(ts-node): TypeScript wrapper with StixError hierarchy and hooks"
```

---

## Task 4: vitest suite

**Files:**
- Create: `bindings/typescript-node/tests/stix.test.ts`

- [ ] **Step 1: Write the tests**

Create `bindings/typescript-node/tests/stix.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import {
  Engine,
  StixError,
  ParseError,
  ModelError,
  ValidationError,
} from "../dist/index.js";

const BUNDLE = JSON.stringify({
  type: "bundle",
  id: "bundle--1",
  objects: [
    { type: "ipv4-addr", id: "ipv4-addr--1", value: "198.51.100.5" },
    {
      type: "observed-data", id: "observed-data--1",
      first_observed: "2020-01-01T00:00:00Z", last_observed: "2020-01-01T00:00:00Z",
      number_observed: 1, object_refs: ["ipv4-addr--1"],
    },
  ],
});

describe("stix node binding", () => {
  it("parses a pattern to an AST object", () => {
    const engine = new Engine();
    const ast = engine.parsePattern("[ipv4-addr:value = '198.51.100.5']").ast;
    expect(typeof ast).toBe("object");
    expect(JSON.stringify(ast)).toContain("ipv4-addr");
  });

  it("reads and iterates bundle objects", () => {
    const engine = new Engine();
    const bundle = engine.parseBundle(BUNDLE);
    expect(bundle.objectCount()).toBe(2);
    expect(bundle.object(0).id).toBe("ipv4-addr--1");
    expect(bundle.object(99)).toBeUndefined();
    expect([...bundle].map((o) => o.type)).toContain("observed-data");
  });

  it("matches (hit and miss)", () => {
    const engine = new Engine();
    const bundle = engine.parseBundle(BUNDLE);
    const hit = engine.parsePattern("[ipv4-addr:value = '198.51.100.5']");
    const res = engine.matchBundle(hit, bundle);
    expect(res.matched).toBe(true);
    expect(Array.isArray(res.observations)).toBe(true);
    const miss = engine.parsePattern("[ipv4-addr:value = '203.0.113.9']");
    expect(engine.matchBundle(miss, bundle).matched).toBe(false);
  });

  it("applies a custom-type hook and matches a computed property", () => {
    const engine = new Engine();
    engine.registerType("x-acme-widget", (obj) => ({
      ...obj,
      risk_band: obj.risk_score > 80 ? "high" : "low",
    }));
    const bundle = engine.parseBundle(JSON.stringify({
      type: "bundle",
      objects: [
        { type: "x-acme-widget", id: "x-acme-widget--1", risk_score: 90 },
        {
          type: "observed-data", id: "observed-data--1",
          first_observed: "2020-01-01T00:00:00Z", last_observed: "2020-01-01T00:00:00Z",
          number_observed: 1, object_refs: ["x-acme-widget--1"],
        },
      ],
    }));
    const pattern = engine.parsePattern("[x-acme-widget:risk_band = 'high']");
    expect(engine.matchBundle(pattern, bundle).matched).toBe(true);
  });

  it("maps errors to the StixError hierarchy", () => {
    const engine = new Engine();
    expect(() => engine.parsePattern("[bad")).toThrow(ParseError);
    expect(() => engine.parseBundle('{"type":"ipv4-addr","id":"x--1"}')).toThrow(ModelError);
    engine.registerType("x-thing", () => { throw new Error("nope"); });
    expect(() =>
      engine.parseBundle('{"type":"bundle","objects":[{"type":"x-thing","id":"x--1"}]}')
    ).toThrow(ValidationError);
    try { engine.parsePattern("[bad"); } catch (e) { expect(e).toBeInstanceOf(StixError); }
  });
});
```

- [ ] **Step 2: Run the tests**

Run: `cd bindings/typescript-node && npm test`
Expected: all vitest tests pass (the `test` script builds native + ts first).

- [ ] **Step 3: Commit**

```bash
git add bindings/typescript-node/tests/
git commit -m "test(ts-node): vitest suite for the node binding"
```

---

## Task 5: README + final verification

**Files:**
- Modify: `bindings/typescript-node/README.md`
- Create: `bindings/typescript-node/.gitignore`

- [ ] **Step 1: gitignore build artifacts**

Create `bindings/typescript-node/.gitignore`:

```
node_modules/
dist/
target/
*.node
binding.js
binding.d.ts
```

- [ ] **Step 2: Replace the placeholder README**

Overwrite `bindings/typescript-node/README.md`:

```markdown
# stix-rust — TypeScript binding (Node)

Native Node.js bindings for the [stix-rust](../../README.md) toolkit, via napi-rs.

- **Package:** `@stix-rust/node`
- **Surface:** typed handles (`Engine`, `Pattern`, `Bundle`, `MatchResult`); deep
  structure (AST, objects) as native JS objects; `StixError` hierarchy.

## Build & test

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
```

- [ ] **Step 3: Final verification**

Run: `cd bindings/typescript-node && npm install && npm test`
Expected: vitest green.

Run: `cd bindings/typescript-node && cargo clippy -- -D warnings`
Expected: clippy clean on the crate.

Run (repo root): `cargo test 2>&1 | grep -c "test result: ok"`
Expected: non-zero, unchanged.

- [ ] **Step 4: Commit**

```bash
git add bindings/typescript-node/README.md bindings/typescript-node/.gitignore
git commit -m "docs(ts-node): real README and gitignore"
```

---

## Self-Review Notes (already applied)

- **Spec coverage:** native JS objects (napi serde-json; Task 2); `StixError`
  hierarchy in the wrapper (Task 3); `Engine`/`Pattern.ast`/`Bundle`
  (count/object/iterate)/`MatchResult` (Tasks 2–3); `registerType` applied JS-side
  with `ValidationError` (Task 3); vitest suite (Task 4); package `@stix-rust/node`,
  workspace-excluded (Task 1); README (Task 5). All SP3 spec points map to a task.
- **Refinement:** hooks applied in the TS wrapper (documented above), so the raw Rust
  layer omits `register_type` — behavior identical to the spec.
- **Type consistency:** wrapper `Engine.{parsePattern,parseBundle,matchBundle,
  registerType}`, `Pattern.ast`, `Bundle.{objectCount,object,[Symbol.iterator]}`,
  `MatchResult.{matched,observations}`, and `Stix*Error` names match across the raw
  napi classes, the wrapper, and the tests, and wrap the verified `stix_ffi` API.
- **Toolchain note** scopes the build-glue adaptation; API/wrapper/tests are exact.
- **No placeholders:** every step has complete content or an exact command.
```
