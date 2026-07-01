# stix-rust — Java binding

Java bindings for the [stix-rust](../../README.md) toolkit, via JNI (jni-rs).

- **Package:** `io.github.benjaminsmall.stix`
- **Surface:** typed handles (`Engine`, `Pattern`, `Bundle`, `MatchResult`); deep
  structure (AST, objects) as Jackson `Map<String,Object>`; `StixException`
  hierarchy.

## Build & test

```bash
cd bindings/java
gradle test        # builds the native lib (cargo) then runs JUnit
```

## Usage

```java
import io.github.benjaminsmall.stix.*;
import java.util.Map;

try (Engine engine = new Engine();
     Pattern pattern = engine.parsePattern("[ipv4-addr:value = '198.51.100.5']");
     Bundle bundle = engine.parseBundle(json)) {
    Map<String, Object> ast = pattern.ast();
    MatchResult result = engine.matchBundle(pattern, bundle);
    System.out.println(result.matched() + " " + result.observations());
}
```

### Custom object types

```java
engine.registerType("x-acme-widget", obj -> {
    long score = ((Number) obj.getOrDefault("risk_score", 0)).longValue();
    obj.put("risk_band", score > 80 ? "high" : "low");
    return obj;
});
```

Hooks run at `parseBundle` time (throwing raises `ValidationException`). Errors are
`StixException` subclasses: `ParseException`, `ModelException`, `MatchException`,
`ValidationException`. Handles are `AutoCloseable` (use try-with-resources); a
`Cleaner` frees any not explicitly closed.

> The native library is built by cargo and loaded from `rust/target/release` via
> `java.library.path` during tests. Bundling the native library into a jar for
> distribution is a publish-time follow-up.
