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
