# Java Binding (SP2) — Design

**Date:** 2026-06-29
**Status:** Approved (brainstorming complete; pending spec review)
**Scope:** SP2 of the language-bindings effort — the Java binding wrapping the
`stix-ffi` facade via JNI (jni-rs).
**Owner agent:** `java-binding` (`bindings/java/`).

## Purpose

Give JVM users the toolkit — parse patterns, import bundles, match, register custom
types — through an idiomatic Java API backed by `stix-ffi`. Deep structure (pattern
AST, bundle objects) is exposed as native `Map<String,Object>` via Jackson.

## Decisions (settled in brainstorming)

- **Deep structure → Jackson `Map<String,Object>`/`List`.** The binding depends on
  Jackson; custom-type hooks are `Function<Map<String,Object>,Map<String,Object>>`
  applied **Java-side** at `parseBundle` time (the same refinement as the TS
  bindings), so the JNI layer stays String↔String with no callback bridging.
- **Unchecked exception hierarchy:** `StixException extends RuntimeException` +
  `ParseException`, `ModelException`, `MatchException`, `ValidationException`.
- **Package `io.github.benjaminsmall.stix`** — convention-correct reverse-DNS for a
  GitHub-hosted project without its own domain (publish-safe on Maven Central).
- **Excluded from the root Cargo workspace** (own `[workspace]` table in the crate +
  root `exclude`) so core `cargo test` stays toolchain-free.
- **Build:** Gradle (Kotlin DSL) + JUnit 5; the native `cdylib` is built by cargo and
  loaded via `java.library.path` in tests.

## Architecture

```
bindings/java/
├── rust/
│   ├── Cargo.toml            # cdylib; jni 0.21; dep stix-ffi(path); own [workspace] table
│   └── src/lib.rs            # JNI exports (String<->String, opaque long handles)
├── build.gradle.kts          # builds native, Jackson dep, JUnit 5
├── settings.gradle.kts
├── src/main/java/io/github/benjaminsmall/stix/
│   ├── Engine.java  Pattern.java  Bundle.java  MatchResult.java
│   ├── StixException.java  ParseException.java  ModelException.java
│   ├── MatchException.java  ValidationException.java
│   └── NativeLoader.java
├── src/test/java/io/github/benjaminsmall/stix/StixTest.java
└── README.md
```
Root `Cargo.toml` `[workspace] exclude` gains `"bindings/java/rust"`.

## JNI layer (`rust/src/lib.rs`)

Minimal, panic-free, String↔String with opaque `long` (boxed-pointer) handles. Each
`FfiError` is thrown as the matching Java exception via `env.throw_new(<FQN>, msg)`;
the method then returns a default value.

Exported functions (JNI naming `Java_io_github_benjaminsmall_stix_<Class>_<method>`):

- **Engine:** `nativeNew() -> long`; `nativeFree(long)`;
  `nativeParsePattern(long enginePtr, String src) -> long` (Pattern ptr; throws
  `ParseException`); `nativeParseBundle(long enginePtr, String json) -> long` (Bundle
  ptr; throws `ModelException`); `nativeMatchBundle(long patternPtr, long bundlePtr)
  -> String` (JSON `{"matched":bool,"observations":[..]}`; throws `MatchException`).
- **Pattern:** `nativeAst(long) -> String` (AST JSON); `nativeFree(long)`.
- **Bundle:** `nativeObjectCount(long) -> int`; `nativeObject(long, int) -> String`
  (object JSON, or `null` if out of range); `nativeFree(long)`.

Handles: `Box::into_raw(Box::new(stix_ffi::Engine/Pattern/Bundle))` as `jlong`;
`nativeFree` reconstitutes and drops the `Box`. `nativeMatchBundle` borrows the
pattern/bundle pointers without taking ownership.

## Java layer

### Exceptions

`StixException(RuntimeException)` with a `String code` (`"parse"|"model"|"match"|
"validation"`) and four subclasses. Native throws `Parse`/`Model`/`Match`; the Java
wrapper throws `Validation` for hook failures.

### `Engine implements AutoCloseable`

- `Engine()` → `nativeNew()`.
- `parsePattern(String src) -> Pattern`.
- `parseBundle(String json) -> Bundle` — **applies registered hooks Java-side**:
  parse `json` with Jackson; for each object whose `type` has a registered hook,
  convert the node to `Map`, apply the `Function` (a thrown hook →
  `ValidationException`), write the result back; reserialize; call
  `nativeParseBundle`.
- `matchBundle(Pattern, Bundle) -> MatchResult` — parse the native JSON result with
  Jackson into `MatchResult`.
- `registerType(String type, Function<Map<String,Object>,Map<String,Object>> hook)`.
- `close()` → `nativeFree`.

### `Pattern implements AutoCloseable`

- `ast() -> Map<String,Object>` — Jackson-parse `nativeAst`.
- `close()`.

### `Bundle implements AutoCloseable, Iterable<Map<String,Object>>`

- `objectCount() -> int`.
- `object(int index) -> Optional<Map<String,Object>>` — `nativeObject`; `null` →
  `Optional.empty()`.
- `iterator()` over `object(0..objectCount())`.
- `close()`.

### `MatchResult`

Pure Java value: `matched() -> boolean`, `observations() -> List<Long>` (built from
the native match JSON). No native handle.

### Resource management

All handle-holding classes implement `AutoCloseable` (try-with-resources for
deterministic freeing) and additionally register with a shared
`java.lang.ref.Cleaner` that calls `nativeFree` if the object is GC'd without
`close()` — a safety net against native leaks.

### `NativeLoader`

A static utility that `System.loadLibrary("stix_java")` once (in a static
initializer on `Engine`). Tests set `-Djava.library.path` to the cargo output dir;
production jar-bundling of the native library per platform is a publish-time
follow-up (out of scope).

## Data Flow

```
String ──Engine.parsePattern──► Pattern ──ast()──► Map (Jackson)
String ──Engine.parseBundle (Java-side hooks)──► Bundle ──object(i)/iterator──► Map
Pattern + Bundle ──Engine.matchBundle──► native JSON ──Jackson──► MatchResult

custom type: Function<Map,Map> ──registerType──► applied in parseBundle (Java) ──►
   thrown hook → ValidationException
FfiError (native) ──env.throw_new──► ParseException | ModelException | MatchException
```

## Error Handling

- Native never panics into the JVM; each fallible facade call maps `FfiError` →
  `env.throw_new(<exception FQN>, message)` and returns a default.
- The wrapper throws `ValidationException` for hook failures; `object(oob)` →
  `Optional.empty()`.

## Testing

JUnit 5 via Gradle; the `test` task depends on a `cargoBuild` task
(`cargo build --release --manifest-path rust/Cargo.toml`) and sets
`-Djava.library.path` to the cargo output dir.

- parse a pattern → `ast()` is a `Map` mentioning the object type.
- parse a bundle → `objectCount()`, iterate objects (`Map`s), `object(oob)` empty.
- match hit and miss → `matched()` / `observations()`.
- `registerType` hook adds a computed property; a pattern matches it.
- error mapping: bad pattern → `ParseException`; non-bundle → `ModelException`; a
  hook that throws → `ValidationException`; all are `instanceof StixException`.
- resource management: try-with-resources closes handles without error.

The `java-binding` agent runs `./gradlew test` locally (which builds the native lib
first). If the JDK/Gradle/JNI toolchain is unavailable in the environment, the agent
stops and reports (CI covers the build) rather than faking green.

## Out of Scope

- Publishing to Maven Central / per-platform native-lib jar bundling.
- `matchScos` / `matchObservedData` (facade exposes only `match_bundle` for now).
- A typed Java object model (deep structure is `Map`, matching the other bindings).

## Future Considerations

- Native-library packaging: bundle `.so`/`.dylib`/`.dll` as jar resources and extract
  at load time (via a `NativeLoader` upgrade) for a self-contained artifact.
- A Maven Central publish workflow under the `io.github.benjaminsmall` namespace.
- Additional facade entry points surface as new `Engine` methods.
