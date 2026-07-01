# TypeScript Bindings (SP3 Node + SP4 wasm) — Design

**Date:** 2026-06-29
**Status:** Approved (brainstorming complete; pending spec review)
**Scope:** SP3 (native Node.js via napi-rs) and SP4 (WebAssembly via wasm-bindgen)
of the language-bindings effort. One shared spec; **two implementation plans**, one
per target, each delegated to its owning agent.
**Owner agents:** `typescript-node-binding` (`bindings/typescript-node/`) and
`typescript-wasm-binding` (`bindings/typescript-wasm/`).

## Purpose

Give TypeScript/JavaScript users the toolkit — parse patterns, import bundles,
match, register custom types — through one **identical TS API** shipped as two
packages: a native Node addon (`@stix-rust/node`) and a portable WebAssembly build
(`@stix-rust/wasm`, browser + Node). Both wrap the `stix-ffi` facade.

## Decisions (settled in brainstorming)

- **Native JS objects** for deep structure (pattern AST, bundle objects), via napi's
  `serde-json` feature (SP3) and `serde-wasm-bindgen` (SP4). Not JSON strings.
- **Error subclass hierarchy**: `StixError` base + `ParseError`, `ModelError`,
  `MatchError`, `ValidationError` — defined in the **TypeScript wrapper** (not minted
  from Rust), driven by a `[code]` prefix the Rust layer puts on thrown errors.
- **Two-layer architecture** per package: a thin Rust FFI layer + a hand-written
  TypeScript wrapper that is the public API. The wrapper is ~identical across both
  packages (kept aligned by this spec) — this is what prevents drift.
- **Packages:** `@stix-rust/node`, `@stix-rust/wasm`.
- **Test runner:** vitest (wasm exercised in Node).
- **Excluded from the root Cargo workspace** (like the Python binding) so core
  `cargo test` stays toolchain-free.

## Shared TypeScript surface (both packages)

```ts
class StixError extends Error { readonly code: "parse" | "model" | "match" | "validation"; }
class ParseError extends StixError {}
class ModelError extends StixError {}
class MatchError extends StixError {}
class ValidationError extends StixError {}

class Pattern { readonly ast: object; }
class Bundle {
  objectCount(): number;
  object(index: number): object | undefined;
  [Symbol.iterator](): Iterator<object>;
}
class MatchResult { readonly matched: boolean; readonly observations: number[]; }

class Engine {
  constructor();
  parsePattern(src: string): Pattern;
  parseBundle(json: string): Bundle;
  matchBundle(pattern: Pattern, bundle: Bundle): MatchResult;
  registerType(typeName: string, hook: (obj: any) => any): void;
}
```

## Architecture (per package)

### Layer 1 — Rust FFI layer

Wraps `stix-ffi` (`Engine`, `Pattern`, `Bundle`, `MatchOutcome`, `FfiError`,
`ErrorCode`).

- **Deep structure → JS object.** SP3: return `serde_json::Value` (enable
  `napi = { features = ["serde-json"] }`), parsed from the facade's JSON. SP4:
  `serde_wasm_bindgen::to_value(&value)`.
- **Errors → tagged JS Error.** Map `FfiError` to a thrown JS error whose message is
  `"[<code>] <message>"` where `<code> ∈ {parse, model, match, validation}`. SP3:
  `napi::Error::from_reason(format!("[{code}] {message}"))`. SP4: `JsError::new(&...)`
  / return `Err(JsValue)`.
- **register_type.** Accept a JS function and install a facade hook that runs
  **synchronously in-call-stack** during `parse_bundle` (no threadsafe-fn / async
  needed, since `parse_bundle` is invoked on the JS thread):
  - SP3: `napi::threadsafe_function`? No — accept `Function` and call it directly
    with `.call()` (synchronous, same-thread).
  - SP4: hold a `js_sys::Function`; call `.call1(&JsValue::NULL, &arg)`.
  - Both: convert object `Value` → JS (serde), call the hook, convert the returned
    JS value → `Value` (serde); a thrown JS error becomes the hook's `Err(message)`
    → surfaces as `ValidationError` from `parse_bundle`.

The raw layer's generated JS/`.d.ts` is internal; consumers use the wrapper.

### Layer 2 — TypeScript wrapper (`index.ts` + `index.d.ts`)

The published entry point. Responsibilities:
- Define the `StixError` hierarchy (the five classes above).
- Wrap the raw `Engine`/`Pattern`/`Bundle`/`MatchResult` so every call that can fail
  is in a `try/catch` that reads the `[code]` prefix off the caught error's message,
  strips it, and throws the matching subclass with the clean message.
- Present the exact shared surface with precise types (hand-written `.d.ts`, or `.ts`
  compiled to `.js` + `.d.ts`).
- For `Bundle`, expose `[Symbol.iterator]` iterating `object(0..objectCount())`.

Because both packages implement this identical wrapper, the only differences are the
import of the raw module and the wasm `init()` step (see below).

## Package specifics

### SP3 — `@stix-rust/node` (`bindings/typescript-node/`)

- `Cargo.toml`: `crate-type = ["cdylib"]`, `napi` + `napi-derive` (+ `serde-json`
  feature), dep on `stix-ffi` (path). Excluded from workspace.
- `package.json`: `@stix-rust/node`, build via `napi build --platform --release`.
- `napi build` emits the native `.node` + a generated binding; the hand-written
  `index.ts` wraps it. `tsc` compiles the wrapper to `dist/`.
- Tests: vitest, run in Node against the built addon.

### SP4 — `@stix-rust/wasm` (`bindings/typescript-wasm/`)

- `Cargo.toml`: `crate-type = ["cdylib"]`, `wasm-bindgen` + `js-sys` +
  `serde-wasm-bindgen`, dep on `stix-ffi` (path). Excluded from workspace.
- `package.json`: `@stix-rust/wasm`, build via `wasm-pack build --target web` (and/or
  `--target nodejs` for the test run).
- wasm exposes an async `init()` (module instantiation); the wrapper re-exports it.
  The TS surface is otherwise identical to SP3.
- Tests: vitest in Node against the `--target nodejs` build.

## Data Flow

```
string ──Engine.parsePattern──► Pattern ──.ast──► JS object
string ──Engine.parseBundle───► Bundle  ──.object(i)/iterate──► JS object
Pattern + Bundle ──Engine.matchBundle──► MatchResult{matched, observations}

custom type: JS (obj)=>obj  ──registerType──► facade hook (serde convert, sync call)
   ──invoked during parseBundle──► thrown JS error → ValidationError
raw error "[code] msg"  ──TS wrapper try/catch──► ParseError|ModelError|MatchError|ValidationError
```

## Error Handling

- Rust layer never panics into JS; each fallible facade call maps `FfiError` →
  `"[code] message"` thrown error.
- TS wrapper converts those into the typed subclass hierarchy; `code` is retained on
  the instance.
- `Bundle.object(i)` out of range → `undefined` (no throw).

## Testing (both packages, shared shape)

vitest suite:
- parse a pattern → `.ast` is a JS object mentioning the object type.
- parse a bundle → `objectCount()`/iteration yields objects; `object(oob)` is
  `undefined`.
- match hit and miss → `matched` / `observations`.
- `registerType` hook adds a computed property; a pattern matches it.
- error mapping: bad pattern → `ParseError`; non-bundle → `ModelError`; a hook that
  throws → `ValidationError`; all `instanceof StixError`.

Each owning agent runs `npm install && <build> && npm test` locally; if the JS/wasm
toolchain is unavailable in the environment, the agent stops and reports (CI covers
the build) rather than faking green.

## Out of Scope

- Publishing to npm / release CI (per-package follow-up).
- `matchScos` / `matchObservedData` (facade exposes only `match_bundle` for now).
- A shared TS "core" package factoring the wrapper (the wrapper is duplicated per
  package for now; see Future).
- Async/streaming APIs beyond wasm's required `init()`.

## Future Considerations

- Factor the identical TS wrapper into a shared `@stix-rust/core-ts` package once npm
  workspace tooling is set up (removes the per-package duplication).
- npm publish workflows (napi prebuilds via `@napi-rs/cli` matrix; wasm via
  `wasm-pack publish`).
- Additional facade entry points surface as new `Engine` methods in both wrappers.
