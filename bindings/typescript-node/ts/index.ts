import * as native from "../binding.js";
import type { ExternalObject } from "../binding.js";
import {
  StixError,
  ParseError,
  ModelError,
  MatchError,
  ValidationError,
  toStixError,
} from "./errors.js";

export { StixError, ParseError, ModelError, MatchError, ValidationError };

/** Run a raw-layer call, mapping its "[code] message" errors to StixError. */
function call<T>(fn: () => T): T {
  try { return fn(); } catch (e) { throw toStixError(e); }
}

/** Throw ValidationError unless `value` is an instance of `cls`. */
function expectInstance(value: unknown, cls: Function, what: string): void {
  if (!(value instanceof cls)) {
    const got = value === null ? "null"
      : typeof value === "object" ? (value as object).constructor?.name ?? "object"
      : typeof value;
    throw new ValidationError(`${what} must be a ${cls.name}, got ${got}`);
  }
}

/**
 * The handle behind a wrapper method's `this`, after checking that `this` really
 * is a `cls` (methods can be detached and called on anything).
 */
function rawOf<R>(self: unknown, cls: abstract new (...args: any[]) => { readonly raw: R }): R {
  expectInstance(self, cls, "this");
  return (self as { readonly raw: R }).raw;
}

export class Pattern {
  /** @internal */ readonly raw: ExternalObject<"Pattern">;
  /** @internal */ constructor(raw: ExternalObject<"Pattern">) { this.raw = raw; }
  get ast(): any { return call(() => native.patternAst(rawOf(this, Pattern))); }
}

export class Bundle {
  /** @internal */ readonly raw: ExternalObject<"Bundle">;
  /** @internal */ constructor(raw: ExternalObject<"Bundle">) { this.raw = raw; }
  objectCount(): number {
    return call(() => native.bundleObjectCount(rawOf(this, Bundle)));
  }
  object(index: number): any | undefined {
    const v = call(() => native.bundleObject(rawOf(this, Bundle), index));
    return v === null ? undefined : v;
  }
  *[Symbol.iterator](): Iterator<any> {
    const n = this.objectCount();
    for (let i = 0; i < n; i++) yield this.object(i);
  }
}

export class MatchResult {
  /** @internal */ readonly raw: native.MatchOutcome;
  /** @internal */ constructor(raw: native.MatchOutcome) { this.raw = raw; }
  get matched(): boolean { return rawOf(this, MatchResult).matched; }
  get observations(): number[] { return [...rawOf(this, MatchResult).observations]; }
}

export type CustomHook = (obj: any) => any;

export class Engine {
  #raw: ExternalObject<"Engine">;
  #hooks = new Map<string, CustomHook>();

  constructor() { this.#raw = native.createEngine(); }

  parsePattern(src: string): Pattern {
    expectInstance(this, Engine, "this");
    return new Pattern(call(() => native.parsePattern(this.#raw, src)));
  }

  parseBundle(json: string): Bundle {
    expectInstance(this, Engine, "this");
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
    return new Bundle(call(() => native.parseBundle(this.#raw, text)));
  }

  matchBundle(pattern: Pattern, bundle: Bundle): MatchResult {
    expectInstance(this, Engine, "this");
    expectInstance(pattern, Pattern, "pattern");
    expectInstance(bundle, Bundle, "bundle");
    return new MatchResult(
      call(() => native.matchBundle(this.#raw, pattern.raw, bundle.raw)),
    );
  }

  registerType(typeName: string, hook: CustomHook): void {
    expectInstance(this, Engine, "this");
    this.#hooks.set(typeName, hook);
  }
}
