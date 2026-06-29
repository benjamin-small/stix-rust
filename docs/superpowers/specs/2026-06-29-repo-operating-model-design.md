# Repository Operating Model — Design

**Date:** 2026-06-29
**Status:** Approved (brainstorming complete; pending spec review)
**Scope:** Stand up the repo structure, per-area subagents, area docs, and issue
conventions that the language bindings will be built into. This precedes the
`stix-ffi` facade and the bindings themselves.

## Purpose

Prepare `stix-rust` to grow language bindings (Python, Java, TypeScript) as
well-bounded areas, each with a dedicated Claude Code subagent, self-contained
documentation, and a lightweight issue/PR workflow the parent agent uses to file
and route work. The goal: a newcomer can be linked directly to
`bindings/<lang>/README.md` and get the complete experience for that interface,
and the parent agent can delegate each area to its owning subagent.

This is **scaffolding only** — no Rust source changes. `cargo build`/`cargo test`
stay green throughout (we add directories, docs, and config).

## Decisions (settled in brainstorming)

- **Additive layout.** Existing `crates/` is untouched (it is the `rust-core`
  area). Bindings are added under `bindings/`.
- **5 subagents**, matching the real toolchain split: `rust-core`,
  `python-binding`, `java-binding`, `typescript-node-binding`,
  `typescript-wasm-binding`.
- **Lightweight SDLC**: per-area + type labels, issue/PR templates, a documented
  parent workflow. **No CODEOWNERS** (solo repo — low value; `AGENTS.md` is the
  ownership map instead).
- **Planned-placeholder READMEs** for each binding area now; filled as each binding
  ships.

## Architecture

Three layers.

### (a) Directory layout

```
stix-rust/
├── README.md                       # hub: toolkit overview + links to every area
├── AGENTS.md                       # ownership map: area → agent → paths → skills
├── .github/
│   ├── ISSUE_TEMPLATE/
│   │   ├── task.yml                # area-tagged task form
│   │   └── bug.yml                 # area-tagged bug form
│   └── pull_request_template.md
├── .claude/
│   ├── agents/
│   │   ├── rust-core.md
│   │   ├── python-binding.md
│   │   ├── java-binding.md
│   │   ├── typescript-node-binding.md
│   │   └── typescript-wasm-binding.md
│   └── skills/                     # area skills (see Skills below)
├── crates/                         # rust-core area (unchanged)
└── bindings/
    ├── python/README.md            # status: planned
    ├── java/README.md              # status: planned
    ├── typescript-node/README.md   # status: planned
    └── typescript-wasm/README.md   # status: planned
```

Each `bindings/<lang>/README.md` is self-contained (overview, install, quick start
skeleton, build/test commands, status) so a direct link gives the full experience.

### (b) Subagents

Five definitions under `.claude/agents/`. Each frontmatter carries `name`,
`description` (precise enough for the parent to auto-route), `tools`, and `model`;
the body is a system prompt that:
- states the area's responsibility and **owned paths** (the boundary),
- points at the area's docs and skills,
- references `AGENTS.md` for shared conventions and the issue workflow.

| Agent | Owns | Toolchain |
| --- | --- | --- |
| `rust-core` | `crates/**` | cargo |
| `python-binding` | `bindings/python/**` | PyO3 + maturin |
| `java-binding` | `bindings/java/**` | jni-rs + Gradle/Maven |
| `typescript-node-binding` | `bindings/typescript-node/**` | napi-rs + npm |
| `typescript-wasm-binding` | `bindings/typescript-wasm/**` | wasm-bindgen + npm |

### (c) SDLC

- **Labels** (`gh label create`): area — `area:rust-core`, `area:python`,
  `area:java`, `area:ts-node`, `area:ts-wasm`; type — `type:feat`, `type:bug`,
  `type:docs`, `type:chore`.
- **Issue templates** (`.github/ISSUE_TEMPLATE/task.yml`, `bug.yml`) with an **area
  dropdown** that applies the matching `area:` label.
- **PR template** with an area checklist and a link to `AGENTS.md`.
- **Parent workflow** (documented in `AGENTS.md`): file an issue with the right
  `area:` label → delegate to the owning subagent via the Agent tool → subagent
  implements in its subtree on a branch → PR (area label + template) → parent
  reviews/merges.

## Components

- **`AGENTS.md`** — the single source of truth. A table mapping *area → agent →
  owned paths → relevant skills*, plus the issue/PR workflow and branch/label
  conventions. This makes "skills live with their agent" discoverable regardless of
  where the harness physically loads skills from.
- **5 agent definition files** — as above.
- **Area READMEs** — 4 planned-placeholder binding READMEs + an updated root
  `README.md` that links to each area (and to the existing crate docs).
- **Skills** — authored as **directory-scoped skills** under each area when the
  harness discovers them; otherwise centralized under `.claude/skills/` with an
  `area:`-prefixed name. Either way they are indexed in `AGENTS.md`. (For *this*
  spec we establish the location + index; area-specific skills are authored with
  their binding, not now.) This is the one harness-dependent detail; the design
  degrades gracefully to the central location.
- **GitHub config** — labels, issue templates, PR template.

## Data Flow (issue lifecycle)

```
parent: identify work
  └─ gh issue create  (+ area:<x>, type:<y> labels, via template)
       └─ parent delegates → Agent(subagent_type=<area agent>)
            └─ subagent: branch, implement within owned paths, open PR (area label)
                 └─ parent: review → merge → close issue
```

## Error Handling / Edge Cases

- **Harness skill discovery** may not pick up nested/scoped skills. Mitigation:
  `AGENTS.md` indexes every skill by area; if scoped discovery fails, skills live in
  central `.claude/skills/` with `area:`-prefixed names. No functionality depends on
  physical nesting.
- **Agent auto-routing** depends on clear `description` fields; each is written to be
  unambiguous about its area so the parent selects correctly.
- **No source churn.** Only additive files; the workspace `members` list is *not*
  changed here (binding crates join it when each binding is actually built).

## Testing / Verification

Because this is scaffolding, verification is structural, not unit tests:

- `cargo test` (workspace) still passes — nothing in `crates/` changed.
- The 5 agents appear in the available-agents list (and have valid frontmatter).
- `gh label list` shows all area + type labels.
- Issue templates are valid YAML (`.github/ISSUE_TEMPLATE/*.yml` parse) and the PR
  template exists.
- Every binding README renders and is reachable from the root README; the root
  README links to all areas.
- `AGENTS.md` lists all 5 areas with paths and the workflow.

## Out of Scope

- The `stix-ffi` facade and any binding implementation (separate specs — this just
  prepares their homes).
- Area-specific skills' *content* (authored with each binding).
- CODEOWNERS (intentionally skipped on a solo repo).
- CI workflows for building/publishing per-language artifacts (added with each
  binding, which knows its own build).

## Future Considerations

- When a second human joins, revisit CODEOWNERS (the `AGENTS.md` map ports directly).
- Per-area release/publish CI (PyPI, npm, Maven Central) belongs to each binding's
  own spec.
- A top-level "interfaces" section in the root README that the bindings populate as
  they ship.
