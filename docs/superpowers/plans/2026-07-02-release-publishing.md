# Release & Publishing (0.1.0) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prepare the repo to publish 0.1.0 to crates.io (5 crates), PyPI (`stix-rust`), and npm (`@stix-rust/node`, `@stix-rust/wasm`) via tag-triggered workflows, plus the RELEASING.md runbook — publishing itself happens post-merge when the user's checklist is done and `v0.1.0` is pushed.

**Architecture:** Lockstep version bump to 0.1.0 with publishable (path+version) workspace deps; three `on: push: tags: ['v*']` workflows — crates (DAG-ordered `cargo publish`), Python (maturin-action trio matrix + sdist → PyPI via OIDC pending publisher), npm (napi trio prebuilds + wasm). Merging deploys nothing; the tag is the release.

**Tech Stack:** GitHub Actions, cargo, maturin-action, @napi-rs/cli, wasm-pack, pypa/gh-action-pypi-publish.

---

## ⚠️ Toolchain-adaptation note

Workflow **structure, triggers, ordering, and package changes are exact**. Action
version pins and the napi prebuild assembly (`napi artifacts`/`prepublish`
sub-steps, `napi.triples` config) follow @napi-rs/cli v2 conventions — adapt those
sub-steps to the installed CLI's documented flow if they differ, preserving the
outcome: `@stix-rust/node` installs with a working prebuilt binary on
linux-x86_64/macos-arm64/windows-x64. Since tag workflows can't be fully exercised
before the real release, verify by YAML-parse + local dry-runs (Task 5); the first
tag push is the true test and RELEASING.md documents recovery (fix + re-tag
`v0.1.0` after deleting it, safe because failed registries are idempotent to retry).

## Task 1: Lockstep version bump + publishable deps

**Files:**
- Modify: `Cargo.toml` (root), all five `crates/*/Cargo.toml`
- Modify: `bindings/python/Cargo.toml`, `bindings/typescript-node/package.json`, `bindings/typescript-wasm/package.json`

- [ ] **Step 1:** In every crate manifest (`crates/{stix-pattern,stix-model,stix-matcher,stix,stix-ffi}/Cargo.toml` and `bindings/python/Cargo.toml`): `version = "0.0.1"` → `version = "0.1.0"`.
- [ ] **Step 2:** Root `[workspace.dependencies]` internal entries become path+version:

```toml
stix-pattern = { path = "crates/stix-pattern", version = "0.1.0" }
stix-model = { path = "crates/stix-model", version = "0.1.0" }
stix-matcher = { path = "crates/stix-matcher", version = "0.1.0" }
stix = { path = "crates/stix", package = "stix-rust", version = "0.1.0" }
```

Also add to `crates/stix-ffi/Cargo.toml` nothing (it uses the workspace entry), and
bump `bindings/python/Cargo.toml`'s `stix-ffi = { path = ... }` to
`{ path = "../../crates/stix-ffi", version = "0.1.0" }` (needed once stix-ffi is on
the index; harmless before).

- [ ] **Step 3:** Both npm `package.json`s: `"version": "0.1.0"`.
- [ ] **Step 4:** Verify: `cargo test 2>&1 | grep -c "test result: ok"` → 16; `cargo publish --dry-run -p stix-pattern` and `-p stix-model` succeed (leaf crates; deeper crates can't dry-run until deps are on the index — expected, note it).
- [ ] **Step 5:** Commit `chore: bump all versions to 0.1.0 with publishable deps`.

## Task 2: release-crates.yml

**Files:** Create `.github/workflows/release-crates.yml`

- [ ] **Step 1:** Verbatim:

```yaml
name: release-crates

on:
  push:
    tags: ['v*']
  workflow_dispatch:

permissions:
  contents: read

jobs:
  publish:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Publish in dependency order
        env:
          CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}
        run: |
          set -euo pipefail
          for crate in stix-pattern stix-model stix-matcher stix-rust stix-ffi; do
            echo "::group::publish $crate"
            cargo publish -p "$crate" --no-verify
            echo "::endgroup::"
            # wait for the index so dependents resolve
            sleep 45
          done
```

(`--no-verify` skips the per-crate rebuild — the workspace was already tested; the
45s sleep is the pragmatic index-propagation wait. After release #1, flip to
crates.io Trusted Publishing per RELEASING.md and drop the secret.)

- [ ] **Step 2:** YAML-parse check (`ruby -ryaml -e 'YAML.load_file(".github/workflows/release-crates.yml")'`). Commit `ci: add crates.io release workflow`.

## Task 3: release-python.yml

**Files:** Create `.github/workflows/release-python.yml`

- [ ] **Step 1:** Verbatim:

```yaml
name: release-python

on:
  push:
    tags: ['v*']
  workflow_dispatch:

permissions:
  contents: read

jobs:
  wheels:
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64
          - os: macos-latest
            target: aarch64
          - os: windows-latest
            target: x64
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: PyO3/maturin-action@v1
        with:
          working-directory: bindings/python
          target: ${{ matrix.target }}
          args: --release --out dist
      - uses: actions/upload-artifact@v4
        with:
          name: wheels-${{ matrix.os }}
          path: bindings/python/dist
  sdist:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: PyO3/maturin-action@v1
        with:
          working-directory: bindings/python
          command: sdist
          args: --out dist
      - uses: actions/upload-artifact@v4
        with:
          name: sdist
          path: bindings/python/dist
  publish:
    needs: [wheels, sdist]
    runs-on: ubuntu-latest
    environment: pypi
    permissions:
      id-token: write
    steps:
      - uses: actions/download-artifact@v4
        with:
          path: dist
          merge-multiple: true
      - uses: pypa/gh-action-pypi-publish@release/v1
        with:
          packages-dir: dist
```

(OIDC via PyPI *pending publisher* — no token. The `pypi` environment name must
match the pending-publisher config.)

- [ ] **Step 2:** YAML-parse check; local sanity `cd bindings/python && maturin build --release 2>&1 | tail -1` succeeds. Commit `ci: add PyPI release workflow`.

## Task 4: release-npm.yml (+ napi triples)

**Files:** Create `.github/workflows/release-npm.yml`; Modify `bindings/typescript-node/package.json`

- [ ] **Step 1:** Add the trio to the node package's napi config:

```json
"napi": {
  "name": "stix-node",
  "triples": {
    "defaults": false,
    "additional": ["x86_64-unknown-linux-gnu", "aarch64-apple-darwin", "x86_64-pc-windows-msvc"]
  }
}
```

- [ ] **Step 2:** Verbatim workflow:

```yaml
name: release-npm

on:
  push:
    tags: ['v*']
  workflow_dispatch:

permissions:
  contents: read

jobs:
  node-build:
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
          - os: macos-latest
            target: aarch64-apple-darwin
          - os: windows-latest
            target: x86_64-pc-windows-msvc
    runs-on: ${{ matrix.os }}
    defaults: { run: { working-directory: bindings/typescript-node } }
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { targets: '${{ matrix.target }}' }
      - uses: actions/setup-node@v4
        with: { node-version: 20 }
      - run: npm install
      - run: npx napi build --platform --release --target ${{ matrix.target }} --js binding.js --dts binding.d.ts
      - uses: actions/upload-artifact@v4
        with:
          name: node-${{ matrix.target }}
          path: bindings/typescript-node/*.node
  node-publish:
    needs: node-build
    runs-on: ubuntu-latest
    defaults: { run: { working-directory: bindings/typescript-node } }
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: actions/setup-node@v4
        with: { node-version: 20, registry-url: 'https://registry.npmjs.org' }
      - run: npm install
      - uses: actions/download-artifact@v4
        with:
          path: bindings/typescript-node/artifacts
          merge-multiple: false
      - run: npx napi artifacts
      - run: npx napi build --platform --js binding.js --dts binding.d.ts && npx tsc
      - run: npx napi prepublish -t npm --skip-gh-release
        env: { NODE_AUTH_TOKEN: '${{ secrets.NPM_TOKEN }}' }
      - run: npm publish --access public
        env: { NODE_AUTH_TOKEN: '${{ secrets.NPM_TOKEN }}' }
  wasm-publish:
    runs-on: ubuntu-latest
    defaults: { run: { working-directory: bindings/typescript-wasm } }
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { targets: wasm32-unknown-unknown }
      - uses: actions/setup-node@v4
        with: { node-version: 20, registry-url: 'https://registry.npmjs.org' }
      - run: cargo install wasm-pack --locked
      - run: npm install
      - run: npm run build
      - run: npm publish --access public
        env: { NODE_AUTH_TOKEN: '${{ secrets.NPM_TOKEN }}' }
```

(`napi prepublish` creates/publishes the per-platform sub-packages and wires them as
`optionalDependencies` — the standard napi-rs flow; adaptation note applies. After
release #1, flip both packages to npm Trusted Publishing and delete `NPM_TOKEN`.)

- [ ] **Step 3:** YAML-parse check; `cd bindings/typescript-node && npm pack --dry-run` and same for wasm succeed. Commit `ci: add npm release workflow with napi prebuild matrix`.

## Task 5: RELEASING.md runbook

**Files:** Create `RELEASING.md`

- [ ] **Step 1:** Write the runbook containing exactly: (a) the user-side checklist
  from the spec (crates.io token → `CARGO_REGISTRY_TOKEN` secret; PyPI account +
  pending publisher for `stix-rust` on `release-python.yml` env `pypi`; npm account +
  `@stix-rust` org + granular token → `NPM_TOKEN` secret); (b) the release
  procedure (`git tag v0.1.0 && git push origin v0.1.0`, watch the three workflows,
  post-release verification commands `cargo add stix-rust --dry-run`,
  `pip index versions stix-rust`, `npm view @stix-rust/node version`); (c) failure
  recovery (delete tag, fix, re-tag; already-published registries tolerate re-runs —
  crates.io rejects re-publishing the same version, treat as success); (d) the
  post-first-release OIDC flip (configure trusted publishing on crates.io ×5 and npm
  ×2, delete both secrets); (e) the follow-up docs PR (remove "not yet published"
  callouts in `docs/book/src/getting-started.md` + the four binding pages and root
  README; add registry badges).
- [ ] **Step 2:** Commit `docs: add RELEASING.md runbook`.

## Task 6: Verification + PR

- [ ] **Step 1:** `cargo test` → 16 ok; `cargo clippy --workspace --all-targets -- -D warnings` clean; all three workflow YAMLs parse; `mdbook build docs/book` still clean.
- [ ] **Step 2:** Push branch, open PR (area: cross-cutting release infra), noting explicitly: **merging publishes nothing; the tag is the release**.

## Self-Review Notes (already applied)

- **Spec coverage:** lockstep bump + path+version deps (Task 1); three tag-driven
  workflows matching the spec's matrix/order/auth exactly (Tasks 2–4); RELEASING.md
  with the full user checklist, OIDC flip, and follow-up docs PR (Task 5);
  pre-tag verification limits honestly stated (Task 6 + adaptation note).
- **Auth matches spec:** PyPI pure-OIDC via pending publisher; crates/npm bootstrap
  tokens with documented flip.
- **Consistency:** crate publish order = DAG (`stix-rust` before `stix-ffi`); secret
  names (`CARGO_REGISTRY_TOKEN`, `NPM_TOKEN`) and environment name (`pypi`) are
  identical across workflows, checklist, and runbook.
```
