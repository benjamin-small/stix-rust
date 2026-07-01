# TypeScript wasm Binding (SP4) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `@stix-rust/wasm` under `bindings/typescript-wasm/` — a WebAssembly build (wasm-bindgen) wrapping the `stix-ffi` facade, with a hand-written TypeScript wrapper presenting the same shared TS surface as the Node binding.

**Architecture:** A wasm-bindgen cdylib crate (excluded from the root Cargo workspace) exposes raw handle classes and throws `"[code] message"` errors. The identical TypeScript wrapper (as SP3) adds the `StixError` hierarchy, an iterable `Bundle`, and `registerType` (applied JS-side at `parseBundle` time). Deep structure crosses as native JS objects via `serde-wasm-bindgen`. Built with wasm-pack; tested in Node (`--target nodejs`); a `--target web` build is provided for browsers.

**Tech Stack:** Rust + wasm-bindgen 0.2 + js-sys + serde-wasm-bindgen; wasm-pack; TypeScript; vitest.

---

## ⚠️ Toolchain-adaptation note for the implementing agent

The **public API, TS wrapper logic, and tests below are exact** — implement them as
written. The **build glue** (wasm-pack target/out-dir, generated module filename,
tsconfig module resolution / import paths from `pkg/`) varies; adapt it to what your
installed `wasm-pack`/`wasm-bindgen` produce, keeping the published API and passing
tests unchanged. Prereqs (install if missing): `cargo install wasm-pack` (or
`npm i -g wasm-pack`), Node ≥ 18, and `npm install` in the package. Use WebFetch on
the wasm-bindgen book if an attribute differs. If the wasm/Node toolchain cannot run
here at all, STOP and report (CI will build) rather than faking green.

**Refinement vs spec:** `registerType` hooks are applied in the TypeScript wrapper at
`parseBundle` time (identical to the Node binding), not stored in the Rust `Engine`;
the raw Rust layer therefore has no `register_type`. Primary build/test target is
`--target nodejs` (synchronous, no `init()`); a `--target web` build is provided for
browsers and documented in the README.

## File Structure

```
bindings/typescript-wasm/
├── Cargo.toml            # cdylib; wasm-bindgen + js-sys + serde-wasm-bindgen; dep stix-ffi(path)
├── package.json          # @stix-rust/wasm; typescript, vitest
├── tsconfig.json
├── src/lib.rs            # #[wasm_bindgen] raw Engine/Pattern/Bundle/MatchResult
├── ts/errors.ts          # StixError hierarchy + parseCode helper
├── ts/index.ts           # wrapper
├── tests/stix.test.ts    # vitest
└── README.md
```
Root `Cargo.toml` `[workspace] exclude` gains `"bindings/typescript-wasm"`.

---

## Task 1: Scaffold the crate + npm package

**Files:**
- Modify: `Cargo.toml` (root)
- Create: `bindings/typescript-wasm/Cargo.toml`, `package.json`, `tsconfig.json`, `src/lib.rs` (minimal)

- [ ] **Step 1: Exclude from the workspace**

In root `Cargo.toml`, extend the `exclude` list under `[workspace]` (keep existing entries):

```toml
exclude = ["bindings/python", "bindings/typescript-node", "bindings/typescript-wasm"]
```

- [ ] **Step 2: Crate manifest**

Create `bindings/typescript-wasm/Cargo.toml`:

```toml
[package]
name = "stix-wasm"
version = "0.0.1"
edition = "2021"
license = "MIT OR Apache-2.0"

[lib]
crate-type = ["cdylib"]

[dependencies]
stix-ffi = { path = "../../crates/stix-ffi" }
wasm-bindgen = "0.2"
js-sys = "0.3"
serde-wasm-bindgen = "0.6"
serde_json = "1"
```

- [ ] **Step 3: package.json + tsconfig**

Create `bindings/typescript-wasm/package.json`:

```json
{
  "name": "@stix-rust/wasm",
  "version": "0.0.1",
  "main": "dist/index.js",
  "types": "dist/index.d.ts",
  "files": ["dist", "pkg"],
  "scripts": {
    "build:wasm": "wasm-pack build --target nodejs --out-dir pkg",
    "build:web": "wasm-pack build --target web --out-dir pkg-web",
    "build": "npm run build:wasm && tsc",
    "test": "npm run build:wasm && tsc && vitest run"
  },
  "devDependencies": {
    "typescript": "^5.4.0",
    "vitest": "^2.0.0"
  }
}
```

Create `bindings/typescript-wasm/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "CommonJS",
    "moduleResolution": "Node",
    "declaration": true,
    "outDir": "dist",
    "rootDir": ".",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "allowJs": false
  },
  "include": ["ts/**/*.ts"]
}
```

> If tsc complains that `pkg/` imports are outside `rootDir`, set `"rootDir": "."`
> (as above) or drop `rootDir` and rely on `include`; adapt so `dist/index.js`
> resolves `../pkg`. This is expected build-glue adaptation.

- [ ] **Step 4: Minimal Rust module**

Create `bindings/typescript-wasm/src/lib.rs`:

```rust
//! WebAssembly bindings for the stix-rust toolkit (raw wasm-bindgen layer).
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn _healthcheck() -> bool {
    true
}
```

- [ ] **Step 5: Verify the wasm builds and the workspace is unaffected**

Run: `cd bindings/typescript-wasm && npm install && npm run build:wasm`
Expected: `pkg/` produced (`stix_wasm.js`, `stix_wasm_bg.wasm`, `stix_wasm.d.ts`).

Run (repo root): `cargo test 2>&1 | grep -c "test result: ok"`
Expected: non-zero — the excluded crate doesn't affect the workspace.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml bindings/typescript-wasm/
git commit -m "feat(ts-wasm): scaffold wasm-bindgen crate and npm package"
```

---

## Task 2: Raw wasm-bindgen layer (handles + error tagging)

**Files:**
- Modify: `bindings/typescript-wasm/src/lib.rs`

- [ ] **Step 1: Implement the raw classes**

Replace `bindings/typescript-wasm/src/lib.rs` with:

```rust
//! WebAssembly bindings for the stix-rust toolkit (raw wasm-bindgen layer).
//!
//! Errors are thrown as `"[code] message"`; the TypeScript wrapper maps the code
//! prefix onto the StixError subclass hierarchy.
use wasm_bindgen::prelude::*;

fn err_js(e: stix_ffi::FfiError) -> JsValue {
    let code = match e.code {
        stix_ffi::ErrorCode::Parse => "parse",
        stix_ffi::ErrorCode::Model => "model",
        stix_ffi::ErrorCode::Match => "match",
        stix_ffi::ErrorCode::Validation => "validation",
    };
    JsError::new(&format!("[{code}] {}", e.message)).into()
}

fn json_to_js(json: &str) -> Result<JsValue, JsValue> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| JsError::new(&format!("[model] {e}")))?;
    serde_wasm_bindgen::to_value(&value).map_err(|e| JsError::new(&e.to_string()).into())
}

#[wasm_bindgen]
pub struct Pattern {
    inner: stix_ffi::Pattern,
}

#[wasm_bindgen]
impl Pattern {
    #[wasm_bindgen(getter)]
    pub fn ast(&self) -> Result<JsValue, JsValue> {
        json_to_js(&self.inner.to_json())
    }
}

#[wasm_bindgen]
pub struct Bundle {
    inner: stix_ffi::Bundle,
}

#[wasm_bindgen]
impl Bundle {
    #[wasm_bindgen(js_name = objectCount)]
    pub fn object_count(&self) -> u32 {
        self.inner.object_count() as u32
    }

    #[wasm_bindgen]
    pub fn object(&self, index: u32) -> Result<JsValue, JsValue> {
        match self.inner.object_json(index as usize) {
            Some(json) => json_to_js(&json),
            None => Ok(JsValue::UNDEFINED),
        }
    }
}

#[wasm_bindgen]
pub struct MatchResult {
    matched: bool,
    observations: Vec<u32>,
}

#[wasm_bindgen]
impl MatchResult {
    #[wasm_bindgen(getter)]
    pub fn matched(&self) -> bool {
        self.matched
    }

    #[wasm_bindgen(getter)]
    pub fn observations(&self) -> Vec<u32> {
        self.observations.clone()
    }
}

#[wasm_bindgen]
pub struct Engine {
    inner: stix_ffi::Engine,
}

#[wasm_bindgen]
impl Engine {
    #[wasm_bindgen(constructor)]
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Engine {
            inner: stix_ffi::Engine::new(),
        }
    }

    #[wasm_bindgen(js_name = parsePattern)]
    pub fn parse_pattern(&self, src: String) -> Result<Pattern, JsValue> {
        self.inner
            .parse_pattern(&src)
            .map(|inner| Pattern { inner })
            .map_err(err_js)
    }

    #[wasm_bindgen(js_name = parseBundle)]
    pub fn parse_bundle(&self, json: String) -> Result<Bundle, JsValue> {
        self.inner
            .parse_bundle(&json)
            .map(|inner| Bundle { inner })
            .map_err(err_js)
    }

    #[wasm_bindgen(js_name = matchBundle)]
    pub fn match_bundle(&self, pattern: &Pattern, bundle: &Bundle) -> Result<MatchResult, JsValue> {
        self.inner
            .match_bundle(&pattern.inner, &bundle.inner)
            .map(|o| MatchResult {
                matched: o.matched,
                observations: o.observations.iter().map(|&i| i as u32).collect(),
            })
            .map_err(err_js)
    }
}
```

- [ ] **Step 2: Build to verify it compiles**

Run: `cd bindings/typescript-wasm && npm run build:wasm`
Expected: builds; `pkg/stix_wasm.d.ts` declares `Engine`, `Pattern`, `Bundle`,
`MatchResult` with the camelCase methods above.

- [ ] **Step 3: Commit**

```bash
git add bindings/typescript-wasm/src/lib.rs
git commit -m "feat(ts-wasm): raw wasm-bindgen Engine/Pattern/Bundle/MatchResult"
```

---

## Task 3: TypeScript wrapper (errors + public API + hooks)

**Files:**
- Create: `bindings/typescript-wasm/ts/errors.ts`
- Create: `bindings/typescript-wasm/ts/index.ts`

- [ ] **Step 1: Write the error hierarchy**

Create `bindings/typescript-wasm/ts/errors.ts` (identical to the Node binding's):

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

Create `bindings/typescript-wasm/ts/index.ts`. It differs from the Node wrapper only
in the raw import path and `Array.from` on `observations` (wasm returns a typed
array):

```ts
import {
  Engine as RawEngine,
  Pattern as RawPattern,
  Bundle as RawBundle,
  MatchResult as RawMatchResult,
} from "../pkg/stix_wasm.js";
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
    return v === undefined || v === null ? undefined : v;
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
  get observations(): number[] { return Array.from(this.raw.observations); }
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

Run: `cd bindings/typescript-wasm && npm run build`
Expected: `dist/` produced, no TS errors. (If module resolution from `../pkg`
complains, adapt tsconfig per the note in Task 1 Step 3.)

- [ ] **Step 4: Commit**

```bash
git add bindings/typescript-wasm/ts/
git commit -m "feat(ts-wasm): TypeScript wrapper with StixError hierarchy and hooks"
```

---

## Task 4: vitest suite

**Files:**
- Create: `bindings/typescript-wasm/tests/stix.test.ts`

- [ ] **Step 1: Write the tests**

Create `bindings/typescript-wasm/tests/stix.test.ts` (same behavior suite as Node,
importing the wasm wrapper):

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

describe("stix wasm binding", () => {
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

Run: `cd bindings/typescript-wasm && npm test`
Expected: all vitest tests pass (the `test` script builds wasm + ts first).

- [ ] **Step 3: Commit**

```bash
git add bindings/typescript-wasm/tests/
git commit -m "test(ts-wasm): vitest suite for the wasm binding"
```

---

## Task 5: README + final verification

**Files:**
- Modify: `bindings/typescript-wasm/README.md`
- Create: `bindings/typescript-wasm/.gitignore`

- [ ] **Step 1: gitignore build artifacts**

Create `bindings/typescript-wasm/.gitignore`:

```
node_modules/
dist/
target/
pkg/
pkg-web/
```

- [ ] **Step 2: Replace the placeholder README**

Overwrite `bindings/typescript-wasm/README.md`:

```markdown
# stix-rust — TypeScript binding (WebAssembly)

Portable WebAssembly bindings for the [stix-rust](../../README.md) toolkit — runs in
Node and the browser.

- **Package:** `@stix-rust/wasm`
- **Surface:** identical to `@stix-rust/node` — typed handles (`Engine`, `Pattern`,
  `Bundle`, `MatchResult`); native JS objects; `StixError` hierarchy.

## Build & test (Node)

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
console.log(pattern.ast);

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
```

- [ ] **Step 3: Final verification**

Run: `cd bindings/typescript-wasm && npm install && npm test`
Expected: vitest green.

Run: `cd bindings/typescript-wasm && cargo clippy --target wasm32-unknown-unknown -- -D warnings`
Expected: clippy clean (add the target with `rustup target add wasm32-unknown-unknown`
if needed; if clippy for the wasm target is unavailable, run `cargo check
--target wasm32-unknown-unknown` instead and note it).

Run (repo root): `cargo test 2>&1 | grep -c "test result: ok"`
Expected: non-zero, unchanged.

- [ ] **Step 4: Commit**

```bash
git add bindings/typescript-wasm/README.md bindings/typescript-wasm/.gitignore
git commit -m "docs(ts-wasm): real README and gitignore"
```

---

## Self-Review Notes (already applied)

- **Spec coverage:** native JS objects (serde-wasm-bindgen; Task 2); `StixError`
  hierarchy in the wrapper (Task 3); `Engine`/`Pattern.ast`/`Bundle`/`MatchResult`
  (Tasks 2–3); `registerType` applied JS-side with `ValidationError` (Task 3); vitest
  (Task 4); package `@stix-rust/wasm`, workspace-excluded, `--target web` browser
  build (Tasks 1 & 5); README (Task 5). All SP4 spec points map to a task.
- **Parity with SP3:** the `ts/errors.ts` and `ts/index.ts` are identical to the Node
  binding except the raw import path and `Array.from(observations)` (wasm returns a
  typed array) — keeping the two packages' surfaces aligned per the spec.
- **Type consistency:** wrapper `Engine.{parsePattern,parseBundle,matchBundle,
  registerType}`, `Pattern.ast`, `Bundle.{objectCount,object,[Symbol.iterator]}`,
  `MatchResult.{matched,observations}`, `Stix*Error` — consistent across the raw
  wasm classes, the wrapper, and the tests; wraps the verified `stix_ffi` API.
- **Toolchain note** scopes wasm-pack/tsconfig glue; API/wrapper/tests are exact.
- **No placeholders:** every step has complete content or an exact command.
```
