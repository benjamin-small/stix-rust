# Release & Publishing (0.1.0) — Design

**Date:** 2026-07-02
**Status:** Approved (brainstorming complete; pending spec review)
**Scope:** First public release: crates.io + PyPI + npm. **Maven Central is
deferred** to its own later cycle (Sonatype verification + jar-bundled native libs).

## Decisions (settled in brainstorming)

- **Registries this cycle:** crates.io (5 crates), PyPI (`stix-rust`), npm
  (`@stix-rust/node`, `@stix-rust/wasm`). Names verified available 2026-07-02.
- **Mechanism:** tag-driven CI — pushing `v0.1.0` triggers three release workflows.
  **OIDC trusted publishing** is the steady state; see Bootstrap below for the
  first-release exception.
- **Build matrix (standard trio):** linux-x86_64, macos-arm64, windows-x64 for
  Python wheels (abi3, one per platform + sdist) and napi prebuilds. wasm is
  platform-independent.
- **Versioning:** lockstep **0.1.0** across all five crates and all three packages;
  one tag = one release everywhere.

## Changes to the repo

1. **Version bumps** — `0.0.1` → `0.1.0` in the five crate manifests,
   `bindings/python` (crate version; pyproject reads it), and both npm
   `package.json`s.
2. **Publishable workspace deps** — crates.io rejects bare `path` dependencies, so
   the root `[workspace.dependencies]` internal entries gain `version = "0.1.0"`
   (path + version form). Publish order follows the DAG:
   `stix-pattern`, `stix-model` → `stix-matcher` → `stix-rust` → `stix-ffi`.
3. **Three workflows**, all `on: push: tags: ['v*']`:
   - `release-crates.yml` — `cargo publish` the five crates in dependency order
     (with a wait-for-index step between tiers).
   - `release-python.yml` — maturin-action wheel matrix (trio) + sdist → PyPI.
   - `release-npm.yml` — napi prebuild matrix (trio) → `@stix-rust/node`;
     `wasm-pack` build → `@stix-rust/wasm`.
4. **`RELEASING.md`** — the runbook: the user-side checklist (below), the tag
   procedure, and the OIDC flip after release #1.

## Auth: bootstrap vs steady state

- **PyPI:** supports *pending publishers* — pure OIDC from the very first release.
  User configures a pending publisher for `stix-rust` pointing at this repo +
  `release-python.yml`.
- **crates.io and npm:** trusted publishing is configured **on an existing
  crate/package**, so release #1 authenticates with short-lived tokens the user
  creates (`CARGO_REGISTRY_TOKEN`, `NPM_TOKEN` repo secrets). After 0.1.0 exists,
  both flip to OIDC and the tokens are revoked/deleted. The workflows read the
  token from secrets when present, so the flip is config-only.

## User-side checklist (only you can do these)

1. crates.io: log in (GitHub SSO), create an API token scoped to publish; add as
   `CARGO_REGISTRY_TOKEN` repo secret.
2. PyPI: create account; add a **pending publisher** for `stix-rust`
   (repo `benjamin-small/stix-rust`, workflow `release-python.yml`).
3. npm: create account; **create the `@stix-rust` org**; create a granular publish
   token; add as `NPM_TOKEN` repo secret.
4. After the first successful release: configure trusted publishing on crates.io
   (all five crates) and npm (both packages), then delete both secrets.

## Flow & gate

Merging this cycle's PR publishes **nothing** — workflows only fire on tags. The
release itself: user completes the checklist → we push `v0.1.0` → watch the three
workflows → verify `cargo add stix-rust`, `pip install stix-rust`,
`npm i @stix-rust/node` resolve. Then a small follow-up PR removes the docs'
"not yet published" callouts and adds registry badges.

## Verification (pre-tag)

- `cargo publish --dry-run -p <crate>` for the two leaf crates (full-DAG dry-run
  isn't possible before deps exist on the index — noted in RELEASING.md).
- `cargo test` (16 suites) green after bumps; binding test suites still pass.
- Workflow YAML parses; `maturin build` and `npm pack` succeed locally.

## Out of scope

- Maven Central (own cycle). Broad platform matrix (add on demand). Changelog
  automation / release-plz. Docs badge/callout PR content beyond the follow-up noted
  above.
