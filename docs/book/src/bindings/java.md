# Java

Package `io.github.benjaminsmall.stix` (JNI via jni-rs). Deep structure arrives as
Jackson `Map<String, Object>`.

## Install

```kotlin
// Gradle (Kotlin DSL)
implementation("io.github.benjaminsmall:stix:0.1.0")
```

From source instead:
 `cd bindings/java && gradle test` builds the
native library (cargo) and runs JUnit. Tests load it from `rust/target/release`
via `java.library.path`.

## Worked example

```java
import io.github.benjaminsmall.stix.*;
import java.util.Map;

try (Engine engine = new Engine()) {

    // 1. parse a pattern; the AST is a Map
    try (Pattern pattern = engine.parsePattern("[ipv4-addr:value = '198.51.100.5']")) {
        Map<String, Object> ast = pattern.ast();

        // 2. import a bundle; iterate its objects
        try (Bundle bundle = engine.parseBundle(json)) {
            System.out.println(bundle.objectCount());
            for (Map<String, Object> obj : bundle) System.out.println(obj.get("type"));
            bundle.object(99);   // Optional.empty() when out of range

            // 3. match — hit and miss
            MatchResult result = engine.matchBundle(pattern, bundle);
            System.out.println(result.matched() + " " + result.observations());
        }
    }

    // 4. custom type with a computed property
    engine.registerType("x-acme-widget", obj -> {
        long score = ((Number) obj.getOrDefault("risk_score", 0)).longValue();
        obj.put("risk_band", score > 80 ? "high" : "low");
        return obj;
    });
    // [x-acme-widget:risk_band = 'high'] now matches enriched bundles
}
```

## Errors

| Exception | Thrown by |
| --- | --- |
| `ParseException` | invalid pattern syntax |
| `ModelException` | invalid JSON / not a bundle |
| `MatchException` | matching failure |
| `ValidationException` | a `registerType` hook threw |

All extend `StixException` (unchecked, carries a `code()`).

## Notes

- `Engine`, `Pattern`, and `Bundle` hold native handles: use **try-with-resources**;
  a `java.lang.ref.Cleaner` frees anything not explicitly closed.
- Hooks run at `parseBundle` time, applied Java-side (Jackson) — no JNI callbacks.
- Bundling the native library into the jar per-platform is a publish-time follow-up.
