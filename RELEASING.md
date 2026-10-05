# Releasing

Releases are **tag-driven**: pushing a `v*` tag fires three workflows that publish
to crates.io (5 crates), PyPI (`stix-rust`), and npm (`@stix-rust/node`,
`@stix-rust/wasm`). Merging to `main` never publishes anything.

## One-time setup (maintainer accounts — only a human can do these)

1. **crates.io** — log in (GitHub SSO) at crates.io; create an API token
   (Account Settings → API Tokens, scope: publish-new + publish-update). Add it as
   the repo secret **`CARGO_REGISTRY_TOKEN`**.
2. **PyPI** — create an account at pypi.org; under *Publishing*, add a **pending
   publisher** for project `stix-rust`: owner `benjamin-small`, repo `stix-rust`,
   workflow `release-python.yml`, environment `pypi`. (Pure OIDC — no token, works
   from the very first release.)
3. **npm** — create an account at npmjs.com; **create the `@stix-rust`
   organization**; create a granular access token with publish rights for the org.
   Add it as the repo secret **`NPM_TOKEN`**.

## Cutting a release

Before tagging, update [`CHANGELOG.md`](CHANGELOG.md): rename its `[Unreleased]`
section to the version being cut and date it.

When bumping versions, update both `package.json` files
(`bindings/typescript-node` and `bindings/typescript-wasm`), including the Node
binding's `optionalDependencies` on its own `@stix-rust/node-*` platform packages.
Then refresh the lockfiles and commit the result:

```bash
(cd bindings/typescript-node && npx -y npm@10 install --package-lock-only)
(cd bindings/typescript-wasm && npx -y npm@10 install --package-lock-only)
```

The release jobs run `npm ci`, which fails when a lockfile and its `package.json`
disagree.

```bash
# from an up-to-date main with a green docs build
git tag v0.1.0
git push origin v0.1.0
```

Watch the three workflows (`release-crates`, `release-python`, `release-npm`) in the
Actions tab. Afterwards verify each registry serves the release:

```bash
cargo add stix-rust --dry-run          # resolves 0.1.0 from crates.io
pip index versions stix-rust           # lists 0.1.0
npm view @stix-rust/node version       # prints 0.1.0
npm view @stix-rust/wasm version       # prints 0.1.0
```

## If a release run fails

Fix the problem on `main`, delete and re-push the tag:

```bash
git tag -d v0.1.0 && git push origin :refs/tags/v0.1.0
git tag v0.1.0 && git push origin v0.1.0
```

Re-runs are safe: PyPI and npm skip-or-reject already-uploaded files, and
crates.io rejects re-publishing an existing version — for any crate already
published, treat that error as success (comment out published crates in the
workflow loop if needed for a partial retry).

Note: `cargo publish --dry-run` only works for the leaf crates (`stix-pattern`,
`stix-model`) before a release exists — dependents can't resolve their deps from the
index until the leaves are published. The first tag push is the real test.

## After the first successful release: flip to OIDC

1. **crates.io** — for each of the five crates, Settings → Trusted Publishing → add
   this repo + `release-crates.yml`. Then delete the `CARGO_REGISTRY_TOKEN` secret
   and swap the workflow's token env for the crates.io OIDC auth action.
2. **npm** — done. All five packages (`@stix-rust/node`, `@stix-rust/wasm` and
   the three `@stix-rust/node-*` platform packages) trust this repo +
   `release-npm.yml`; the publish jobs authenticate via OIDC (`id-token: write`,
   npm >= 11.5.1 on Node 24), so `NPM_TOKEN` is unused and can be deleted. A new
   platform package must be added as a trusted publisher on npmjs.com before its
   first publish from CI.
3. Revoke both tokens at their registries.

## Follow-up docs PR (after 0.1.0 is live)

- Remove the "**Not yet published**" callouts from
  `docs/book/src/getting-started.md`, the four `docs/book/src/bindings/*.md` pages,
  the root `README.md`, and the binding READMEs.
- Add registry badges (crates.io, PyPI, npm) to the root README.

## Deferred

**Maven Central** (Java) is not part of this pipeline yet — it needs Sonatype
namespace verification for `io.github.benjaminsmall`, GPG signing, and per-platform
native-library jar bundling. Tracked as future work.
