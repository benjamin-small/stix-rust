// Wrong-type handles must raise errors, not crash the process (CodeQL
// rust/access-invalid-pointer). A native crash would kill the vitest worker,
// so every probe runs in a child Node process and we assert on its exit code
// and output.
import { describe, it, expect } from "vitest";
import { spawnSync } from "node:child_process";
import * as path from "node:path";

const ROOT = path.resolve(__dirname, "..");
const DIST = path.join(ROOT, "dist", "index.js");
const BINDING = path.join(ROOT, "binding.js");

/** Shared prelude: builds one value of every kind as `fixtures`. */
const PRELUDE = `
const stix = require(${JSON.stringify(DIST)});
const engine = new stix.Engine();
const bundle = engine.parseBundle(JSON.stringify({
  type: "bundle", id: "bundle--1",
  objects: [{ type: "ipv4-addr", id: "ipv4-addr--1", value: "198.51.100.5" }],
}));
const pattern = engine.parsePattern("[ipv4-addr:value = '198.51.100.5']");
const result = engine.matchBundle(pattern, bundle);
const fixtures = { engine, bundle, pattern, result, plain: {}, null: null, number: 42 };
function attempt(label, fn) {
  try { fn(); out.push({ label, threw: false }); }
  catch (e) {
    out.push({
      label, threw: true, name: e && e.name, message: String(e && e.message),
      isValidation: e instanceof stix.ValidationError,
    });
  }
}
const out = [];
`;

type Outcome = {
  label: string;
  threw: boolean;
  name?: string;
  message?: string;
  isValidation?: boolean;
};

/** Run `body` (after PRELUDE) in a child process; return its recorded outcomes. */
function probe(body: string): Outcome[] {
  const script = PRELUDE + body + "\nprocess.stdout.write(JSON.stringify(out));\n";
  const r = spawnSync(process.execPath, ["-e", script], {
    encoding: "utf8",
    timeout: 30_000,
  });
  // Exit code 0 and no signal: anything else (e.g. 139 / SIGSEGV) is a crash.
  expect({ status: r.status, signal: r.signal, stderr: r.stderr }).toEqual({
    status: 0,
    signal: null,
    stderr: "",
  });
  return JSON.parse(r.stdout) as Outcome[];
}

describe("wrapper rejects wrong-type arguments with ValidationError", () => {
  it("matchBundle with swapped arguments", () => {
    const [o] = probe(`attempt("swapped", () => engine.matchBundle(bundle, pattern));`);
    expect(o.threw).toBe(true);
    expect(o.isValidation).toBe(true);
    expect(o.message).toMatch(/pattern.*Pattern/);
  });

  const wrongForPattern = ["engine", "bundle", "result", "plain", "null", "number"];
  const wrongForBundle = ["engine", "pattern", "result", "plain", "null", "number"];

  it.each(wrongForPattern)("matchBundle(pattern = %s)", (kind) => {
    const [o] = probe(
      `attempt("p", () => engine.matchBundle(fixtures[${JSON.stringify(kind)}], bundle));`,
    );
    expect(o).toMatchObject({ threw: true, isValidation: true, name: "ValidationError" });
    expect(o.message).toMatch(/pattern must be a Pattern/);
  });

  it.each(wrongForBundle)("matchBundle(bundle = %s)", (kind) => {
    const [o] = probe(
      `attempt("b", () => engine.matchBundle(pattern, fixtures[${JSON.stringify(kind)}]));`,
    );
    expect(o).toMatchObject({ threw: true, isValidation: true, name: "ValidationError" });
    expect(o.message).toMatch(/bundle must be a Bundle/);
  });

  it("prototype members called with a foreign `this` throw ValidationError", () => {
    const outs = probe(`
      const members = [
        ["Pattern", "ast", "get"],
        ["Bundle", "objectCount", "value"],
        ["Bundle", "object", "value"],
        ["MatchResult", "matched", "get"],
        ["MatchResult", "observations", "get"],
        ["Engine", "parsePattern", "value"],
        ["Engine", "parseBundle", "value"],
        ["Engine", "matchBundle", "value"],
        ["Engine", "registerType", "value"],
        ["Bundle", Symbol.iterator, "value"],
      ];
      // Receivers that pass \`instanceof\` but carry no (or a bogus) native handle.
      const bogus = (cls) => {
        const C = stix[cls];
        const r = { "Object.create": Object.create(C.prototype) };
        if (cls !== "Engine") {
          r["new(null)"] = new C(null);
          r["new(undefined)"] = new C(undefined);
          r["new({})"] = new C({});
          r["new(wrong handle)"] = new C(cls === "Bundle" ? pattern.raw : bundle.raw);
        }
        return r;
      };
      for (const [cls, name, kind] of members) {
        const fn = Object.getOwnPropertyDescriptor(stix[cls].prototype, name)[kind];
        const receivers = { ...bogus(cls) };
        for (const k of Object.keys(fixtures)) {
          const self = fixtures[k];
          if (self && self.constructor && self.constructor.name === cls) continue;
          receivers[k] = self;
        }
        for (const [k, self] of Object.entries(receivers)) {
          // A wrong \`new MatchResult({})\` has no native handle to check, so
          // it is only required not to crash; it is skipped below.
          if (cls === "MatchResult" && k === "new({})") continue;
          if (cls === "MatchResult" && k === "new(wrong handle)") continue;
          attempt(cls + "." + String(name) + " this=" + k, () => {
            const r = fn.call(self, 0);
            if (r && typeof r.next === "function") r.next();
          });
        }
      }
    `);
    expect(outs.length).toBeGreaterThan(50);
    for (const o of outs) {
      expect(o, o.label).toMatchObject({ threw: true, isValidation: true });
    }
  });
});

describe("raw native layer type-checks every handle", () => {
  it("every exported function rejects every wrong-type handle", () => {
    const outs = probe(`
      const raw = require(${JSON.stringify(BINDING)});
      const e = raw.createEngine();
      const p = raw.parsePattern(e, "[ipv4-addr:value = '198.51.100.5']");
      const b = raw.parseBundle(e, '{"type":"bundle","id":"bundle--1","objects":[]}');
      const vals = { engine: e, pattern: p, bundle: b, plain: {}, null: null, number: 7,
                     undef: undefined, wrapper: fixtures.pattern };
      // [function name, argument kinds]; "s" = string, "n" = number.
      const sigs = [
        ["parsePattern", ["engine", "s"]],
        ["parseBundle", ["engine", "s"]],
        ["matchBundle", ["engine", "pattern", "bundle"]],
        ["patternAst", ["pattern"]],
        ["bundleObjectCount", ["bundle"]],
        ["bundleObject", ["bundle", "n"]],
      ];
      const good = { engine: e, pattern: p, bundle: b,
                     s: "[ipv4-addr:value = '1.2.3.4']", n: 0 };
      // Every function the addon exports must be listed above.
      const listed = sigs.map((s) => s[0]).concat(["createEngine"]).sort();
      out.push({ label: "exports", threw: false,
                 message: JSON.stringify(Object.keys(raw).sort()) === JSON.stringify(listed)
                   ? "ok" : "unlisted: " + Object.keys(raw).join(",") });
      for (const [fname, kinds] of sigs) {
        kinds.forEach((kind, i) => {
          if (kind === "s" || kind === "n") return;
          for (const [vk, v] of Object.entries(vals)) {
            if (vk === kind) continue;
            const args = kinds.map((k) => good[k]);
            args[i] = v;
            attempt(fname + " arg" + i + "=" + vk, () => raw[fname](...args));
          }
        });
      }
    `);
    const [exp, ...rest] = outs;
    expect(exp.message).toBe("ok");
    expect(rest.length).toBeGreaterThan(30);
    for (const o of rest) {
      expect(o.threw, o.label).toBe(true);
      expect(o.message, o.label).toMatch(
        /^\[validation\] expected a handle of type (Engine|Pattern|Bundle)$/,
      );
    }
  });

  it("well-typed handles still work end to end", () => {
    const outs = probe(`
      const raw = require(${JSON.stringify(BINDING)});
      const e = raw.createEngine();
      const p = raw.parsePattern(e, "[ipv4-addr:value = '198.51.100.5']");
      const b = raw.parseBundle(e, JSON.stringify({ type: "bundle", id: "bundle--1",
        objects: [
          { type: "ipv4-addr", id: "ipv4-addr--1", value: "198.51.100.5" },
          { type: "observed-data", id: "observed-data--1",
            first_observed: "2020-01-01T00:00:00Z", last_observed: "2020-01-01T00:00:00Z",
            number_observed: 1, object_refs: ["ipv4-addr--1"] },
        ] }));
      const m = raw.matchBundle(e, p, b);
      out.push({ label: JSON.stringify({
        matched: m.matched, count: raw.bundleObjectCount(b),
        first: raw.bundleObject(b, 0).id, missing: raw.bundleObject(b, 99),
        astType: typeof raw.patternAst(p),
      }), threw: false });
    `);
    expect(JSON.parse(outs[0].label)).toEqual({
      matched: true, count: 2, first: "ipv4-addr--1", missing: null, astType: "object",
    });
  });
});
