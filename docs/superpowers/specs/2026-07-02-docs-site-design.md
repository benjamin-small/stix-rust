# Documentation Site — Design

**Date:** 2026-07-02
**Status:** Approved (brainstorming complete; pending spec review)
**Scope:** A published documentation site for the whole toolkit (guide + API
reference), plus the doc-comment backfill and the umbrella-crate rename that make
it accurate. Registry **publishing is a separate later cycle**; this spec only makes
the docs publish-ready.

## Purpose

A single site that fully explains the project: what it is, how to use it from every
language, how it works inside, and — prominently — what it does *not* do yet. Anyone
linked to the site gets the complete experience; `/api/` carries the always-current
rustdoc reference.

Grounded in a fresh documentation-oriented analysis of the five core crates
(public-API inventory, newcomer concepts, doc-comment gaps, and ten "sharp edges"),
performed by an Explore agent against `main`.

## Decisions (settled in brainstorming)

- **Hosting:** GitHub Pages at `https://benjamin-small.github.io/stix-rust/`,
  deployed by a GitHub Actions workflow on push to `main`. (User said "GitLab page";
  repo is on GitHub — GitHub Pages confirmed as the intent.)
- **Generator:** **mdBook** (+ `mdbook-mermaid` for diagrams). Rust-native, installed
  via cargo, no npm/Python toolchain in CI.
- **Scope:** guide site + published rustdoc under `/api/` + doc-comment backfill +
  Mermaid architecture diagrams.
- **Install pages are publish-ready:** package-manager install commands are the
  primary form, each with a clearly marked *"not yet published — install from
  source"* callout that the future publishing cycle deletes.
- **Package naming (verified against the registries, 2026-07-02):**
  - crates.io: `stix` and `stix2` are **taken**; `stix-pattern`, `stix-model`,
    `stix-matcher`, `stix-ffi`, and `stix-rust` are available.
  - PyPI `stix-rust`: available. npm `@stix-rust/node` / `@stix-rust/wasm`: available.
  - **Umbrella crate renames to package `stix-rust`**, keeping `[lib] name = "stix"`
    so all existing `use stix::...` code, `stix-ffi`, and the bindings compile
    unchanged. Docs print the final names.

## Work streams (two plans, disjoint files, can run in parallel)

### Stream A — core polish (owner: `rust-core` agent; area `crates/` + root manifest)

1. **Umbrella-crate rename.** `crates/stix/Cargo.toml`: `name = "stix-rust"`,
   add `[lib] name = "stix"`; root `[workspace.dependencies]` entry becomes
   `stix = { path = "crates/stix", package = "stix-rust" }`. No source-code changes;
   workspace tests stay green.
2. **Doc-comment backfill** (from the analysis checklist). Add missing rustdoc to:
   - `stix_matcher::compare::{value_eq_literal, value_cmp_literal, value_in_set}`
   - `stix_matcher::eval::{eval_comparison, eval_comparison_expression, eval_pattern}`
   - `stix_matcher::pattern_ops::{like_matches, regex_matches}`
   - `stix_matcher::subset::{is_subset, is_superset}`
   - `stix_model::{StixObject, TypedObject}` enum-level docs
   - `stix_model::StixValue` accessor methods; `ObjectStore` methods;
     `stix_pattern::PathStep`; `SpecVersion` variants
   No behavior changes; `cargo test` + clippy `-D warnings` stay green.

### Stream B — the site (owner: parent, inline; area `docs/book/` + `.github/workflows/`)

mdBook sources, the Mermaid diagrams, and the deploy workflow.

## Site structure

```
docs/book/
├── book.toml                      # title, mdbook-mermaid preprocessor, git-repo link
└── src/
    ├── SUMMARY.md
    ├── introduction.md            # what STIX is, what the toolkit does, honest status
    ├── getting-started.md         # install (publish-ready) + 10-line quick start
    ├── guide/
    │   ├── patterns.md            # grammar, operator table, precedence, AST-as-JSON
    │   ├── objects.md             # typed/generic/custom trichotomy, ObjectView, StixValue, ObjectStore
    │   ├── matching.md            # observations, 4 entry points, binding enumeration
    │   └── custom-types.md        # registry, hooks, computed properties (all languages)
    ├── bindings/
    │   ├── python.md  java.md  typescript-node.md  typescript-wasm.md
    │   │                          # same worked example per language; adapted from area READMEs
    ├── architecture.md            # Mermaid: crate graph, data flow, match walk-through; stix-ffi story
    ├── limitations.md             # the 10 sharp edges, first-class page
    ├── contributing.md            # dev commands, AGENTS.md pointer
    └── api.md                     # entry page linking into /api/ rustdoc
```

**Content requirements (from the analysis):**
- The guide teaches the three newcomer concept clusters: AST shape + precedence
  (patterns); `ObjectView` + the trichotomy (objects); observation semantics +
  binding enumeration (matching).
- `limitations.md` covers all ten sharp edges: FOLLOWEDBY unsupported; temporal
  qualifiers unsupported; timestamps compared as strings; binding enumeration vs
  full MITRE semantics; `_ref` traversal requires an `ObjectStore`; property
  synthesis on typed objects; `[*]` expansion semantics; integer/float
  distinctness; LIKE anchored vs MATCHES unanchored + invalid-regex-never-matches;
  ISSUBSET/ISSUPERSET are IP/CIDR-only.
- Binding pages show the identical worked example (parse → import → match →
  custom-type hook → error handling) in each language.
- Diagrams (Mermaid): crate dependency graph; parse/import/match data flow; a
  step-by-step match evaluation of one concrete pattern.

## CI / deployment

`.github/workflows/docs.yml`, on `push: branches: [main]` (plus `workflow_dispatch`):

1. Checkout; install Rust; `cargo install mdbook mdbook-mermaid` (cached).
2. `mdbook build docs/book` → `site/`.
3. `cargo doc --workspace --no-deps` → copy `target/doc` → `site/api/`.
4. Upload artifact → `actions/deploy-pages` (Pages "GitHub Actions" source, enabled
   via `gh api` once).

## Error handling / edge cases

- The workflow fails loudly if `mdbook build` or `cargo doc` errors (no partial deploys).
- `/api/` links use rustdoc's stable per-crate index pages (`api/stix/index.html`
  etc. — note the lib name remains `stix`, so rustdoc paths don't change with the
  package rename).
- Site base path is `/stix-rust/` (project Pages); book links are relative so they
  survive the prefix.

## Verification

- Stream A: `cargo test` (16 suites) + `cargo clippy --workspace --all-targets -- -D warnings`
  green; `cargo doc --workspace --no-deps` builds without warnings on the backfilled items.
- Stream B: `mdbook build` clean locally; workflow runs green on `main`; site root
  and `/api/stix/index.html` reachable; every SUMMARY.md chapter renders.

## Out of scope (deferred to the publishing cycle)

- Actually publishing to crates.io / PyPI / npm / Maven Central; release CI (wheel
  and prebuilt-binary matrices); version bump to 0.1.0; the user-side
  account/namespace/OIDC checklist. Install pages carry the "not yet published"
  callouts until then.
- Docs versioning (one live version for now).

## Future considerations

- The publishing cycle deletes the install callouts and adds registry badges.
- If Maven Central lands later, the Java page gains its dependency snippet then.
- A `linkcheck` step can be added to CI once the site stabilizes.
