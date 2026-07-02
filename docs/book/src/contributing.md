# Contributing

Issues and pull requests are welcome. The project keeps a disciplined loop: design
specs and implementation plans live under `docs/superpowers/`, and the repository is
divided into agent-owned areas described in
[`AGENTS.md`](https://github.com/benjamin-small/stix-rust/blob/main/AGENTS.md) —
file issues with an `area:*` label and keep PRs within one area.

## Dev commands

**Rust core** (`crates/`):

```bash
cargo test                                          # all suites
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo doc --workspace --no-deps                     # API docs
```

**Bindings** (each is excluded from the root workspace and builds standalone):

```bash
cd bindings/python          && maturin develop && python -m pytest -q
cd bindings/typescript-node && npm install && npm test
cd bindings/typescript-wasm && npm install && npm test
cd bindings/java            && gradle test
```

**This site:**

```bash
cargo install mdbook mdbook-mermaid
mdbook serve docs/book      # live-reload preview at localhost:3000
```

## Ground rules

- Keep `cargo test` green and clippy clean (`-D warnings`, including
  `missing_docs` — every public item is documented).
- Changes confined to one area per PR; cross-area work is split.
- Interface changes update the area README and this book in the same PR.
