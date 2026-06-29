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
