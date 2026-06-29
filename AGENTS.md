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
