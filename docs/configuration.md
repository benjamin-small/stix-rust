# Configuration

The library crates and their test suite require no secrets, environment variables, or runtime configuration files. STIX documents and patterns are supplied by library callers.

Development uses the stable Rust toolchain described in the root README. Individual language bindings have their own build instructions under `bindings/`.

Release automation uses repository or environment configuration rather than local `.env` files:

- crates.io publishing currently reads `CARGO_REGISTRY_TOKEN` from a GitHub Actions secret;
- npm publishing currently reads `NPM_TOKEN` from a GitHub Actions secret; and
- PyPI publishing uses trusted publishing with GitHub OIDC.

See `RELEASING.md` for setup, rotation, and recovery instructions. Never place registry tokens in tracked files.
