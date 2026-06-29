# Repository Operating Model Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the repository's operating model — additive `bindings/` directory with planned-placeholder READMEs, an `AGENTS.md` ownership map, five area subagents, and a lightweight GitHub issue/PR/label workflow — so language bindings can later be built into prepared, well-owned homes.

**Architecture:** Pure scaffolding — only additive files (docs, `.claude/agents/*.md`, `.github/` templates) plus GitHub labels. No Rust source changes; the Cargo workspace `members` list is not touched. Verification is structural (files exist, YAML parses, labels exist, `cargo test` stays green).

**Tech Stack:** Markdown, Claude Code agent definitions, GitHub issue forms (YAML) + `gh` CLI for labels. No code compilation.

---

## File Structure

- `AGENTS.md` — ownership map (area → agent → paths → skills) + parent issue/delegation workflow + conventions.
- `bindings/python/README.md`, `bindings/java/README.md`, `bindings/typescript-node/README.md`, `bindings/typescript-wasm/README.md` — planned-placeholder area docs.
- `README.md` (root) — add an "Interfaces / language bindings" section linking each area.
- `.claude/agents/rust-core.md`, `python-binding.md`, `java-binding.md`, `typescript-node-binding.md`, `typescript-wasm-binding.md` — five subagent definitions.
- `.github/ISSUE_TEMPLATE/task.yml`, `.github/ISSUE_TEMPLATE/bug.yml`, `.github/pull_request_template.md` — issue/PR templates.
- GitHub labels (created via `gh`, not files).

---

## Task 1: AGENTS.md ownership map

**Files:**
- Create: `AGENTS.md`

- [ ] **Step 1: Create AGENTS.md**

Create `AGENTS.md`:

```markdown
# Agents & Operating Model

This repository is organized into **areas**, each owned by a dedicated Claude Code
subagent. The parent agent orchestrates: it files issues, routes them to the owning
subagent, and reviews/merges the resulting PRs.

## Areas

| Area | Agent | Owns (paths) | Toolchain | Skills |
| --- | --- | --- | --- | --- |
| Rust core | `rust-core` | `crates/**` | cargo | (none yet) |
| Python | `python-binding` | `bindings/python/**` | PyO3 + maturin | (none yet) |
| Java | `java-binding` | `bindings/java/**` | jni-rs + Gradle | (none yet) |
| TypeScript (Node) | `typescript-node-binding` | `bindings/typescript-node/**` | napi-rs + npm | (none yet) |
| TypeScript (wasm) | `typescript-wasm-binding` | `bindings/typescript-wasm/**` | wasm-bindgen + npm | (none yet) |

Area-specific skills are listed in the "Skills" column as they are authored, and
live either as directory-scoped skills under the area or under `.claude/skills/`
with an `area:`-prefixed name. This table is the source of truth for what belongs
to whom.

## Parent workflow

1. **File an issue** with `gh issue create`, applying one `area:<x>` label and one
   `type:<y>` label (or use an issue template and apply the area label per the
   selected dropdown).
2. **Delegate** to the owning subagent via the Agent tool (`subagent_type` = the
   area's agent), pointing it at the issue.
3. The subagent **implements within its owned paths only**, on a branch, and opens a
   PR using the PR template (with the area label).
4. The parent **reviews and merges**, then closes the issue.

## Labels

- Area: `area:rust-core`, `area:python`, `area:java`, `area:ts-node`, `area:ts-wasm`
- Type: `type:feat`, `type:bug`, `type:docs`, `type:chore`

## Boundaries

A subagent edits only files under its owned paths. Cross-area changes (e.g. a
binding needing a new `stix-ffi` method) are split: the core change is an issue for
`rust-core`; the binding consumes it once merged.
```

- [ ] **Step 2: Verify it renders and is complete**

Run: `grep -c "area:" AGENTS.md`
Expected: prints a number ≥ 5 (all area labels listed).

- [ ] **Step 3: Commit**

```bash
git add AGENTS.md
git commit -m "docs: add AGENTS.md operating model and ownership map"
```

---

## Task 2: Binding area placeholder READMEs

**Files:**
- Create: `bindings/python/README.md`
- Create: `bindings/java/README.md`
- Create: `bindings/typescript-node/README.md`
- Create: `bindings/typescript-wasm/README.md`

- [ ] **Step 1: Create the Python area README**

Create `bindings/python/README.md`:

```markdown
# stix-rust — Python binding

> **Status: planned.** This area is scaffolded; the binding is not yet implemented.

Python bindings for the [stix-rust](../../README.md) toolkit — parse STIX 2.1
patterns, import STIX objects, and match patterns against observations, from Python.

- **Toolchain:** [PyO3](https://pyo3.rs) + [maturin](https://www.maturin.rs)
- **Surface:** typed handles (`Engine`, `Pattern`, `Bundle`, `MatchResult`) with JSON
  for deep structure (AST dumps, object properties), wrapping the `stix-ffi` facade.
- **Owner agent:** `python-binding`

## Planned usage

```python
import stix

engine = stix.Engine()
pattern = engine.parse_pattern("[ipv4-addr:value = '198.51.100.1']")
bundle = engine.parse_bundle(open("bundle.json").read())
result = engine.match_bundle(pattern, bundle)
assert result.matched
```

## Build & test (once implemented)

```bash
maturin develop      # build + install into the current venv
pytest               # run the Python test suite
```
```

- [ ] **Step 2: Create the Java area README**

Create `bindings/java/README.md`:

```markdown
# stix-rust — Java binding

> **Status: planned.** This area is scaffolded; the binding is not yet implemented.

Java bindings for the [stix-rust](../../README.md) toolkit, via JNI.

- **Toolchain:** [jni-rs](https://github.com/jni-rs/jni-rs) (native lib) + Gradle (JAR)
- **Surface:** typed handles (`Engine`, `Pattern`, `Bundle`, `MatchResult`) with JSON
  for deep structure, wrapping the `stix-ffi` facade.
- **Owner agent:** `java-binding`

## Planned usage

```java
try (Engine engine = new Engine()) {
    Pattern pattern = engine.parsePattern("[ipv4-addr:value = '198.51.100.1']");
    Bundle bundle = engine.parseBundle(json);
    MatchResult result = engine.matchBundle(pattern, bundle);
    assert result.matched();
}
```

## Build & test (once implemented)

```bash
cargo build --release        # build the native JNI library
./gradlew test               # run the Java test suite
```
```

- [ ] **Step 3: Create the TypeScript (Node) area README**

Create `bindings/typescript-node/README.md`:

```markdown
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
```

- [ ] **Step 4: Create the TypeScript (wasm) area README**

Create `bindings/typescript-wasm/README.md`:

```markdown
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
```

- [ ] **Step 5: Verify all four exist with the status banner**

Run: `grep -rl "Status: planned" bindings/`
Expected: lists all four `bindings/*/README.md` files.

- [ ] **Step 6: Commit**

```bash
git add bindings/
git commit -m "docs: scaffold planned-placeholder READMEs for binding areas"
```

---

## Task 3: Link the areas from the root README

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add an Interfaces section**

In `README.md`, add this section immediately **after** the "Workspace layout"
section (before "Installation"):

```markdown
## Language interfaces

The Rust crates are the core. Language bindings (in progress) live under
[`bindings/`](bindings/) — each has self-contained docs you can link to directly:

| Interface | Toolchain | Docs | Status |
| --- | --- | --- | --- |
| Python | PyO3 + maturin | [`bindings/python`](bindings/python/README.md) | 🚧 planned |
| Java | jni-rs | [`bindings/java`](bindings/java/README.md) | 🚧 planned |
| TypeScript (Node) | napi-rs | [`bindings/typescript-node`](bindings/typescript-node/README.md) | 🚧 planned |
| TypeScript (wasm) | wasm-bindgen | [`bindings/typescript-wasm`](bindings/typescript-wasm/README.md) | 🚧 planned |

Contributor and agent conventions are documented in [`AGENTS.md`](AGENTS.md).
```

- [ ] **Step 2: Verify the link section is present**

Run: `grep -n "## Language interfaces" README.md`
Expected: prints the heading's line number.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: link language interface areas from the root README"
```

---

## Task 4: rust-core and python subagent definitions

**Files:**
- Create: `.claude/agents/rust-core.md`
- Create: `.claude/agents/python-binding.md`

- [ ] **Step 1: Create the rust-core agent**

Create `.claude/agents/rust-core.md`:

```markdown
---
name: rust-core
description: Owns the Rust core crates under crates/ (stix-pattern, stix-model, stix-matcher, stix, and the planned stix-ffi facade). Use for any change to the parsing, object model, matching engine, umbrella crate, or the FFI facade — i.e. work under crates/. Not for language bindings.
tools: Read, Write, Edit, Bash, Grep, Glob
model: inherit
---

You own the **Rust core** of stix-rust: everything under `crates/`
(`stix-pattern`, `stix-model`, `stix-matcher`, `stix`, and the planned
`stix-ffi` facade).

Responsibilities:
- Implement and maintain parsing, the object model, the matching engine, the
  umbrella crate, and the FFI facade.
- Keep `cargo test` green and `cargo clippy --workspace --all-targets -- -D warnings`
  clean. Develop test-first.
- Only edit files under `crates/` (and workspace `Cargo.toml` when adding a core
  crate). Do not touch `bindings/`.

Conventions live in `AGENTS.md`. The bindings depend on the `stix-ffi` facade you
own; when a binding needs a new capability, expose it here as a small, stable,
panic-free facade method.
```

- [ ] **Step 2: Create the python-binding agent**

Create `.claude/agents/python-binding.md`:

```markdown
---
name: python-binding
description: Owns the Python binding under bindings/python/ (PyO3 + maturin). Use for any change to the Python interface — pyclass handles, the maturin build, the Python test suite, or Python-facing docs. Not for the Rust core or other language bindings.
tools: Read, Write, Edit, Bash, Grep, Glob, WebSearch, WebFetch
model: inherit
---

You own the **Python binding** of stix-rust, under `bindings/python/`.

Responsibilities:
- Wrap the `stix-ffi` facade with PyO3 `#[pyclass]` handles (`Engine`, `Pattern`,
  `Bundle`, `MatchResult`), mapping facade errors to Python exceptions and bridging
  Python callables into custom-model registration hooks.
- Maintain the maturin build and a `pytest` suite. Keep the area's README accurate.
- Only edit files under `bindings/python/`. Core changes belong to `rust-core`.

Conventions and the issue workflow live in `AGENTS.md`. The surface is hybrid:
typed handles with JSON for deep structure (AST dumps, object properties).
```

- [ ] **Step 3: Verify frontmatter is valid**

Run: `head -4 .claude/agents/rust-core.md .claude/agents/python-binding.md`
Expected: each file begins with `---` then a `name:` line matching the filename.

- [ ] **Step 4: Commit**

```bash
git add .claude/agents/rust-core.md .claude/agents/python-binding.md
git commit -m "chore: add rust-core and python-binding subagents"
```

---

## Task 5: java and typescript subagent definitions

**Files:**
- Create: `.claude/agents/java-binding.md`
- Create: `.claude/agents/typescript-node-binding.md`
- Create: `.claude/agents/typescript-wasm-binding.md`

- [ ] **Step 1: Create the java-binding agent**

Create `.claude/agents/java-binding.md`:

```markdown
---
name: java-binding
description: Owns the Java binding under bindings/java/ (jni-rs + Gradle). Use for any change to the Java interface — JNI glue, the native library build, the Gradle/JUnit test suite, or Java-facing docs. Not for the Rust core or other language bindings.
tools: Read, Write, Edit, Bash, Grep, Glob, WebSearch, WebFetch
model: inherit
---

You own the **Java binding** of stix-rust, under `bindings/java/`.

Responsibilities:
- Wrap the `stix-ffi` facade via `jni-rs`, exposing `Engine`/`Pattern`/`Bundle`/
  `MatchResult` Java classes backed by native handles; map facade errors to Java
  exceptions and bridge Java callbacks (global refs + JVM attach) into custom-model
  registration hooks.
- Maintain the native build and a Gradle/JUnit test suite. Keep the README accurate.
- Only edit files under `bindings/java/`. Core changes belong to `rust-core`.

Conventions and the issue workflow live in `AGENTS.md`. The surface is hybrid:
typed handles with JSON for deep structure.
```

- [ ] **Step 2: Create the typescript-node-binding agent**

Create `.claude/agents/typescript-node-binding.md`:

```markdown
---
name: typescript-node-binding
description: Owns the native Node.js TypeScript binding under bindings/typescript-node/ (napi-rs). Use for any change to the Node addon interface — napi glue, the npm build, the Node test suite, or its docs. Not for the wasm binding, the Rust core, or other languages.
tools: Read, Write, Edit, Bash, Grep, Glob, WebSearch, WebFetch
model: inherit
---

You own the **TypeScript (Node) binding** of stix-rust, under
`bindings/typescript-node/`.

Responsibilities:
- Wrap the `stix-ffi` facade via `napi-rs`, exposing `Engine`/`Pattern`/`Bundle`/
  `MatchResult` classes; map facade errors to JS errors and bridge JS functions
  (threadsafe functions) into custom-model registration hooks.
- Maintain the npm package, prebuilt-binary build, and a test suite. Keep the README
  accurate and the TS API shape aligned with the wasm sibling.
- Only edit files under `bindings/typescript-node/`. Core changes belong to
  `rust-core`.

Conventions and the issue workflow live in `AGENTS.md`. Surface is hybrid: typed
handles with JSON for deep structure.
```

- [ ] **Step 3: Create the typescript-wasm-binding agent**

Create `.claude/agents/typescript-wasm-binding.md`:

```markdown
---
name: typescript-wasm-binding
description: Owns the WebAssembly TypeScript binding under bindings/typescript-wasm/ (wasm-bindgen). Use for any change to the wasm interface — wasm-bindgen glue, the wasm-pack build, the browser/Node test suite, or its docs. Not for the Node addon, the Rust core, or other languages.
tools: Read, Write, Edit, Bash, Grep, Glob, WebSearch, WebFetch
model: inherit
---

You own the **TypeScript (WebAssembly) binding** of stix-rust, under
`bindings/typescript-wasm/`.

Responsibilities:
- Wrap the `stix-ffi` facade via `wasm-bindgen`, exposing `Engine`/`Pattern`/
  `Bundle`/`MatchResult`; map facade errors to JS errors and bridge `js_sys::Function`
  callbacks (synchronous, single-threaded) into custom-model registration hooks.
- Maintain the `wasm-pack` build and a test suite that runs in the browser and Node.
  Keep the README accurate and the TS API shape aligned with the Node sibling.
- Only edit files under `bindings/typescript-wasm/`. Core changes belong to
  `rust-core`.

Conventions and the issue workflow live in `AGENTS.md`. Surface is hybrid: typed
handles with JSON for deep structure.
```

- [ ] **Step 4: Verify all five agents exist with matching names**

Run: `for f in .claude/agents/*.md; do echo "$f:"; grep -m1 "^name:" "$f"; done`
Expected: five files, each `name:` equal to its filename stem (`rust-core`,
`python-binding`, `java-binding`, `typescript-node-binding`,
`typescript-wasm-binding`).

- [ ] **Step 5: Commit**

```bash
git add .claude/agents/java-binding.md .claude/agents/typescript-node-binding.md .claude/agents/typescript-wasm-binding.md
git commit -m "chore: add java and typescript subagents"
```

---

## Task 6: GitHub issue & PR templates

**Files:**
- Create: `.github/ISSUE_TEMPLATE/task.yml`
- Create: `.github/ISSUE_TEMPLATE/bug.yml`
- Create: `.github/pull_request_template.md`

- [ ] **Step 1: Create the task issue form**

Create `.github/ISSUE_TEMPLATE/task.yml`:

```yaml
name: Task
description: A unit of work in a specific area
title: "[task] "
labels: ["type:feat"]
body:
  - type: dropdown
    id: area
    attributes:
      label: Area
      description: Which area owns this work? (The parent will apply the matching area label.)
      options:
        - rust-core
        - python
        - java
        - ts-node
        - ts-wasm
    validations:
      required: true
  - type: textarea
    id: what
    attributes:
      label: What needs to be done
    validations:
      required: true
  - type: textarea
    id: acceptance
    attributes:
      label: Acceptance criteria
    validations:
      required: true
```

- [ ] **Step 2: Create the bug issue form**

Create `.github/ISSUE_TEMPLATE/bug.yml`:

```yaml
name: Bug
description: Something is broken in a specific area
title: "[bug] "
labels: ["type:bug"]
body:
  - type: dropdown
    id: area
    attributes:
      label: Area
      description: Which area is affected? (The parent will apply the matching area label.)
      options:
        - rust-core
        - python
        - java
        - ts-node
        - ts-wasm
    validations:
      required: true
  - type: textarea
    id: expected
    attributes:
      label: Expected vs actual
    validations:
      required: true
  - type: textarea
    id: repro
    attributes:
      label: Steps to reproduce
    validations:
      required: true
```

- [ ] **Step 3: Create the PR template**

Create `.github/pull_request_template.md`:

```markdown
## Summary

<!-- What changed and why -->

## Area

<!-- Check the one area this PR touches (see AGENTS.md). Cross-area PRs should be split. -->

- [ ] rust-core
- [ ] python
- [ ] java
- [ ] ts-node
- [ ] ts-wasm

## Checklist

- [ ] Changes confined to the area's owned paths
- [ ] Tests added/updated and passing
- [ ] Area README updated if the interface changed
```

- [ ] **Step 4: Verify the YAML templates parse**

Run: `python3 -c "import yaml,glob; [yaml.safe_load(open(f)) for f in glob.glob('.github/ISSUE_TEMPLATE/*.yml')]; print('templates valid')"`
Expected: prints `templates valid` (no exception).

- [ ] **Step 5: Commit**

```bash
git add .github/
git commit -m "chore: add issue forms and PR template"
```

---

## Task 7: Create GitHub labels and final verification

**Files:** none (GitHub state + verification only)

- [ ] **Step 1: Create the area and type labels**

Run each (idempotent — `--force` updates if it already exists):

```bash
gh label create "area:rust-core" --color "5319e7" --description "Rust core crates" --force
gh label create "area:python"    --color "1d76db" --description "Python (PyO3) binding" --force
gh label create "area:java"      --color "b60205" --description "Java (jni-rs) binding" --force
gh label create "area:ts-node"   --color "0e8a16" --description "TypeScript Node (napi) binding" --force
gh label create "area:ts-wasm"   --color "fbca04" --description "TypeScript wasm binding" --force
gh label create "type:feat"      --color "a2eeef" --description "New feature" --force
gh label create "type:bug"       --color "d73a4a" --description "Bug" --force
gh label create "type:docs"      --color "0075ca" --description "Documentation" --force
gh label create "type:chore"     --color "cfd3d7" --description "Tooling/maintenance" --force
```

- [ ] **Step 2: Verify the labels exist**

Run: `gh label list | grep -E "area:|type:" | wc -l`
Expected: `9` (five area + four type labels).

- [ ] **Step 3: Verify the workspace is still healthy (no source touched)**

Run: `cargo test 2>&1 | grep -c "test result: ok"`
Expected: a non-zero count, all suites passing — scaffolding changed no Rust code.

- [ ] **Step 4: Verify the structure is complete**

Run:
```bash
ls bindings/*/README.md .claude/agents/*.md .github/ISSUE_TEMPLATE/*.yml .github/pull_request_template.md AGENTS.md
```
Expected: lists 4 binding READMEs, 5 agent files, 2 issue templates, 1 PR template, and `AGENTS.md` — all present.

- [ ] **Step 5: Commit (if any verification produced changes)**

No file changes are expected in this task (labels live on GitHub). If `git status`
is clean, skip the commit. Otherwise:

```bash
git add -A
git commit -m "chore: finalize operating-model scaffolding"
```

---

## Self-Review Notes (already applied)

- **Spec coverage:** additive layout + binding READMEs (Task 2), `AGENTS.md`
  ownership map + workflow (Task 1), 5 subagents (Tasks 4–5), root README links
  (Task 3), labels + issue/PR templates (Tasks 6–7), structural verification +
  `cargo test` green (Task 7). No CODEOWNERS (per spec). Planned-placeholder READMEs
  (per spec). All spec sections map to a task.
- **No source churn:** no task edits `crates/` or the workspace `members` list; Task 7
  explicitly re-verifies `cargo test`.
- **Consistency:** area slugs (`rust-core`, `python`, `java`, `ts-node`, `ts-wasm`),
  agent names (`rust-core`, `python-binding`, `java-binding`,
  `typescript-node-binding`, `typescript-wasm-binding`), and owned paths are used
  identically across `AGENTS.md`, the agent files, READMEs, labels, and templates.
- **Harness-dependent detail flagged:** skills location (scoped vs central) is
  documented in `AGENTS.md` rather than hard-coded, matching the spec's graceful
  degradation.
- **No placeholders:** every file's full content is in the plan.
```
