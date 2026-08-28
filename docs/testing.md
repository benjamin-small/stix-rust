# Testing

Run the Rust workspace gate with:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features
cargo test --all-features
```

The tests cover the STIX pattern lexer and parser, the valid/invalid conformance corpus, object and bundle modeling, object-store references, comparison and observation matching, high-level entry points, serialization, the FFI facade, and binding-facing behavior. Integration and documentation tests run as part of the Cargo test command.

Instrumentation-based line and branch coverage is currently 0% because CI does not configure a Rust coverage reporter. This is a measurement limitation, not a claim that the suites execute no code. Test results remain the acceptance signal until coverage tooling and a baseline are added.
