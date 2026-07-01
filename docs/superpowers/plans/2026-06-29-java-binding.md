# Java Binding (SP2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Java binding under `bindings/java/` — a JNI (jni-rs) native library plus an idiomatic Java wrapper (`Engine`/`Pattern`/`Bundle`/`MatchResult`, Jackson `Map` deep structure, `StixException` hierarchy, Java-side custom hooks), built and tested with Gradle + JUnit 5.

**Architecture:** A jni-rs cdylib (excluded from the root Cargo workspace, own `[workspace]` table) exposes minimal String↔String native methods over opaque `long` handles and throws Java exceptions on `FfiError`. A Jackson-backed Java wrapper presents the public API, applies `registerType` hooks Java-side at `parseBundle`, and frees native handles via `AutoCloseable` + `Cleaner`.

**Tech Stack:** Rust + jni 0.21; Java 17 + Jackson (databind) + JUnit 5; Gradle (Kotlin DSL). Package `io.github.benjaminsmall.stix`; native lib `stix_java`.

---

## ⚠️ Toolchain-adaptation note for the implementing agent

The **public API, wrapper logic, and tests are exact** — implement them as written.
Adapt only **build glue and version-sensitive JNI API** to your installed tools,
preserving behavior/signatures/tests:
- **jni-rs 0.21** API is version-sensitive (`JNIEnv` mutability, `get_string`/
  `new_string`/`throw_new` shapes). Pin `jni = "0.21"`; if a call doesn't compile,
  adjust to the installed 0.21.x form. Treat the compiler as source of truth.
- **Gradle**: use the system `gradle` if present, else `gradle wrapper` to generate
  `./gradlew`, else install Gradle. Adjust the native-build wiring / `java.library.path`
  to your layout.
- Prereqs: a JDK (17+) and Gradle; `cargo`. If the JDK/Gradle toolchain cannot build
  or run here, STOP and report exactly what failed — do NOT fake green (CI will build).

**JNI function names** follow `Java_io_github_benjaminsmall_stix_<Class>_<method>` (no
identifiers contain underscores, so no `_1` escaping is needed).

## File Structure

```
bindings/java/
├── rust/Cargo.toml            # cdylib "stix_java"; jni 0.21; stix-ffi(path); own [workspace]
├── rust/src/lib.rs            # JNI exports
├── settings.gradle.kts
├── build.gradle.kts
├── src/main/java/io/github/benjaminsmall/stix/
│   ├── StixException.java ParseException.java ModelException.java MatchException.java ValidationException.java
│   ├── NativeLoader.java
│   ├── Engine.java Pattern.java Bundle.java MatchResult.java
├── src/test/java/io/github/benjaminsmall/stix/StixTest.java
└── README.md
```
Root `Cargo.toml` `[workspace] exclude` gains `"bindings/java/rust"`.

---

## Task 1: Scaffold (Rust crate + Gradle project)

**Files:**
- Modify: `Cargo.toml` (root)
- Create: `bindings/java/rust/Cargo.toml`, `bindings/java/rust/src/lib.rs`
- Create: `bindings/java/settings.gradle.kts`, `bindings/java/build.gradle.kts`

- [ ] **Step 1: Exclude the crate from the workspace**

In root `Cargo.toml`, extend the `[workspace] exclude` list:

```toml
exclude = ["bindings/python", "bindings/typescript-node", "bindings/typescript-wasm", "bindings/java/rust"]
```

- [ ] **Step 2: Rust crate manifest (own workspace)**

Create `bindings/java/rust/Cargo.toml`:

```toml
[package]
name = "stix-java"
version = "0.0.1"
edition = "2021"
license = "MIT OR Apache-2.0"

[lib]
name = "stix_java"
crate-type = ["cdylib"]

[dependencies]
stix-ffi = { path = "../../../crates/stix-ffi" }
jni = "0.21"

# Detach from the parent workspace (the crate lives in an excluded path).
[workspace]
```

- [ ] **Step 3: Minimal JNI module**

Create `bindings/java/rust/src/lib.rs`:

```rust
//! JNI bindings for the stix-rust toolkit (raw layer).
use jni::objects::JClass;
use jni::sys::jlong;
use jni::JNIEnv;

#[no_mangle]
pub extern "system" fn Java_io_github_benjaminsmall_stix_NativeLoader_nativeHealthcheck<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jlong {
    1
}
```

- [ ] **Step 4: Gradle project files**

Create `bindings/java/settings.gradle.kts`:

```kotlin
rootProject.name = "stix-java"
```

Create `bindings/java/build.gradle.kts`:

```kotlin
plugins {
    java
}

repositories { mavenCentral() }

java {
    toolchain { languageVersion.set(JavaLanguageVersion.of(17)) }
}

dependencies {
    implementation("com.fasterxml.jackson.core:jackson-databind:2.17.1")
    testImplementation(platform("org.junit:junit-bom:5.10.2"))
    testImplementation("org.junit.jupiter:junit-jupiter")
}

val cargoBuild by tasks.registering(Exec::class) {
    workingDir = file("rust")
    commandLine("cargo", "build", "--release")
}

tasks.test {
    dependsOn(cargoBuild)
    useJUnitPlatform()
    systemProperty("java.library.path", file("rust/target/release").absolutePath)
}
```

- [ ] **Step 5: Verify the native crate builds and Gradle resolves**

Run: `cd bindings/java/rust && cargo build --release`
Expected: builds `target/release/libstix_java.{dylib,so}` (or `stix_java.dll`).

Run: `cd bindings/java && gradle tasks --offline || gradle tasks`
Expected: Gradle lists tasks (confirms the build script parses). If `gradle` is
absent, run `gradle wrapper` first or install Gradle (see toolchain note).

Run (repo root): `cargo test 2>&1 | grep -c "test result: ok"`
Expected: non-zero — the excluded crate doesn't affect the workspace.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml bindings/java/
git commit -m "feat(java): scaffold jni crate and Gradle project"
```

---

## Task 2: Exception hierarchy

**Files:**
- Create the five exception classes under `src/main/java/io/github/benjaminsmall/stix/`

- [ ] **Step 1: Create the base exception**

Create `bindings/java/src/main/java/io/github/benjaminsmall/stix/StixException.java`:

```java
package io.github.benjaminsmall.stix;

/** Base class for all stix errors (unchecked). */
public class StixException extends RuntimeException {
    private final String code;

    public StixException(String code, String message) {
        super(message);
        this.code = code;
    }

    /** One of "parse", "model", "match", "validation". */
    public String code() {
        return code;
    }
}
```

- [ ] **Step 2: Create the four subclasses**

Create `ParseException.java`:

```java
package io.github.benjaminsmall.stix;

public class ParseException extends StixException {
    public ParseException(String message) { super("parse", message); }
}
```

Create `ModelException.java`:

```java
package io.github.benjaminsmall.stix;

public class ModelException extends StixException {
    public ModelException(String message) { super("model", message); }
}
```

Create `MatchException.java`:

```java
package io.github.benjaminsmall.stix;

public class MatchException extends StixException {
    public MatchException(String message) { super("match", message); }
}
```

Create `ValidationException.java`:

```java
package io.github.benjaminsmall.stix;

public class ValidationException extends StixException {
    public ValidationException(String message) { super("validation", message); }
}
```

- [ ] **Step 3: Verify they compile**

Run: `cd bindings/java && gradle compileJava`
Expected: BUILD SUCCESSFUL (the five classes compile).

- [ ] **Step 4: Commit**

```bash
git add bindings/java/src/main/java/io/github/benjaminsmall/stix/*Exception.java
git commit -m "feat(java): add StixException hierarchy"
```

---

## Task 3: JNI native layer

**Files:**
- Modify: `bindings/java/rust/src/lib.rs`

- [ ] **Step 1: Implement the native methods**

Replace `bindings/java/rust/src/lib.rs` with:

```rust
//! JNI bindings for the stix-rust toolkit (raw layer).
//!
//! Handles are boxed pointers passed as jlong. Deep structure crosses as JSON
//! strings; the Java wrapper parses them with Jackson. On FfiError, the matching
//! Java exception is thrown.
use jni::objects::{JClass, JString};
use jni::sys::{jint, jlong, jstring};
use jni::JNIEnv;

fn throw(env: &mut JNIEnv, e: stix_ffi::FfiError) {
    let class = match e.code {
        stix_ffi::ErrorCode::Parse => "io/github/benjaminsmall/stix/ParseException",
        stix_ffi::ErrorCode::Model => "io/github/benjaminsmall/stix/ModelException",
        stix_ffi::ErrorCode::Match => "io/github/benjaminsmall/stix/MatchException",
        stix_ffi::ErrorCode::Validation => "io/github/benjaminsmall/stix/ValidationException",
    };
    let _ = env.throw_new(class, e.message);
}

fn read_string(env: &mut JNIEnv, s: &JString) -> String {
    env.get_string(s).map(|js| js.into()).unwrap_or_default()
}

fn out_string(env: &mut JNIEnv, s: String) -> jstring {
    env.new_string(s)
        .map(|js| js.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

// --- Engine ---

#[no_mangle]
pub extern "system" fn Java_io_github_benjaminsmall_stix_Engine_nativeNew<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jlong {
    Box::into_raw(Box::new(stix_ffi::Engine::new())) as jlong
}

#[no_mangle]
pub extern "system" fn Java_io_github_benjaminsmall_stix_Engine_nativeFree<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    ptr: jlong,
) {
    if ptr != 0 {
        unsafe { drop(Box::from_raw(ptr as *mut stix_ffi::Engine)) };
    }
}

#[no_mangle]
pub extern "system" fn Java_io_github_benjaminsmall_stix_Engine_nativeParsePattern<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    engine_ptr: jlong,
    src: JString<'l>,
) -> jlong {
    let engine = unsafe { &*(engine_ptr as *const stix_ffi::Engine) };
    let src = read_string(&mut env, &src);
    match engine.parse_pattern(&src) {
        Ok(p) => Box::into_raw(Box::new(p)) as jlong,
        Err(e) => {
            throw(&mut env, e);
            0
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_io_github_benjaminsmall_stix_Engine_nativeParseBundle<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    engine_ptr: jlong,
    json: JString<'l>,
) -> jlong {
    let engine = unsafe { &*(engine_ptr as *const stix_ffi::Engine) };
    let json = read_string(&mut env, &json);
    match engine.parse_bundle(&json) {
        Ok(b) => Box::into_raw(Box::new(b)) as jlong,
        Err(e) => {
            throw(&mut env, e);
            0
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_io_github_benjaminsmall_stix_Engine_nativeMatchBundle<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    pattern_ptr: jlong,
    bundle_ptr: jlong,
) -> jstring {
    let pattern = unsafe { &*(pattern_ptr as *const stix_ffi::Pattern) };
    let bundle = unsafe { &*(bundle_ptr as *const stix_ffi::Bundle) };
    // NOTE: match_bundle lives on Engine in stix-ffi; use a throwaway engine (it holds
    // no per-call state relevant to matching).
    let engine = stix_ffi::Engine::new();
    match engine.match_bundle(pattern, bundle) {
        Ok(o) => {
            let json = format!(
                "{{\"matched\":{},\"observations\":{:?}}}",
                o.matched, o.observations
            );
            out_string(&mut env, json)
        }
        Err(e) => {
            throw(&mut env, e);
            std::ptr::null_mut()
        }
    }
}

// --- Pattern ---

#[no_mangle]
pub extern "system" fn Java_io_github_benjaminsmall_stix_Pattern_nativeAst<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    ptr: jlong,
) -> jstring {
    let pattern = unsafe { &*(ptr as *const stix_ffi::Pattern) };
    out_string(&mut env, pattern.to_json())
}

#[no_mangle]
pub extern "system" fn Java_io_github_benjaminsmall_stix_Pattern_nativeFree<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    ptr: jlong,
) {
    if ptr != 0 {
        unsafe { drop(Box::from_raw(ptr as *mut stix_ffi::Pattern)) };
    }
}

// --- Bundle ---

#[no_mangle]
pub extern "system" fn Java_io_github_benjaminsmall_stix_Bundle_nativeObjectCount<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    ptr: jlong,
) -> jint {
    let bundle = unsafe { &*(ptr as *const stix_ffi::Bundle) };
    bundle.object_count() as jint
}

#[no_mangle]
pub extern "system" fn Java_io_github_benjaminsmall_stix_Bundle_nativeObject<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    ptr: jlong,
    index: jint,
) -> jstring {
    let bundle = unsafe { &*(ptr as *const stix_ffi::Bundle) };
    match bundle.object_json(index as usize) {
        Some(json) => out_string(&mut env, json),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "system" fn Java_io_github_benjaminsmall_stix_Bundle_nativeFree<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    ptr: jlong,
) {
    if ptr != 0 {
        unsafe { drop(Box::from_raw(ptr as *mut stix_ffi::Bundle)) };
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd bindings/java/rust && cargo build --release`
Expected: builds the cdylib with no errors. (If a jni 0.21.x signature differs,
adapt per the toolchain note — e.g. `env` mutability or `get_string` argument form.)

- [ ] **Step 3: Clippy**

Run: `cd bindings/java/rust && cargo clippy -- -D warnings`
Expected: clean (the `unsafe` pointer derefs are expected; no clippy warnings).

- [ ] **Step 4: Commit**

```bash
git add bindings/java/rust/src/lib.rs
git commit -m "feat(java): JNI native layer (Engine/Pattern/Bundle) with exception throwing"
```

---

## Task 4: Java wrapper (handles + Jackson + hooks)

**Files:**
- Create: `NativeLoader.java`, `MatchResult.java`, `Pattern.java`, `Bundle.java`, `Engine.java`

- [ ] **Step 1: NativeLoader**

Create `bindings/java/src/main/java/io/github/benjaminsmall/stix/NativeLoader.java`:

```java
package io.github.benjaminsmall.stix;

/** Loads the native stix_java library exactly once. */
final class NativeLoader {
    private static boolean loaded = false;

    private NativeLoader() {}

    static synchronized void load() {
        if (!loaded) {
            System.loadLibrary("stix_java");
            loaded = true;
        }
    }
}
```

- [ ] **Step 2: MatchResult**

Create `MatchResult.java`:

```java
package io.github.benjaminsmall.stix;

import java.util.List;

/** The outcome of a match. */
public final class MatchResult {
    private final boolean matched;
    private final List<Long> observations;

    MatchResult(boolean matched, List<Long> observations) {
        this.matched = matched;
        this.observations = observations;
    }

    public boolean matched() { return matched; }

    public List<Long> observations() { return observations; }
}
```

- [ ] **Step 3: Pattern**

Create `Pattern.java`:

```java
package io.github.benjaminsmall.stix;

import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.lang.ref.Cleaner;
import java.util.Map;

/** A parsed pattern. Holds a native handle; close() (or GC) frees it. */
public final class Pattern implements AutoCloseable {
    static { NativeLoader.load(); }
    private static final Cleaner CLEANER = Cleaner.create();
    private static final ObjectMapper MAPPER = new ObjectMapper();

    private final long ptr;
    private final Cleaner.Cleanable cleanable;

    Pattern(long ptr) {
        this.ptr = ptr;
        this.cleanable = CLEANER.register(this, () -> nativeFree(ptr));
    }

    long ptr() { return ptr; }

    /** The pattern's AST as a Map. */
    public Map<String, Object> ast() {
        String json = nativeAst(ptr);
        try {
            return MAPPER.readValue(json, new TypeReference<Map<String, Object>>() {});
        } catch (Exception e) {
            throw new ModelException(e.getMessage());
        }
    }

    @Override
    public void close() { cleanable.clean(); }

    private static native String nativeAst(long ptr);
    private static native void nativeFree(long ptr);
}
```

- [ ] **Step 4: Bundle**

Create `Bundle.java`:

```java
package io.github.benjaminsmall.stix;

import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.lang.ref.Cleaner;
import java.util.Iterator;
import java.util.Map;
import java.util.NoSuchElementException;
import java.util.Optional;

/** An imported bundle. Iterable over its objects (each a Map). */
public final class Bundle implements AutoCloseable, Iterable<Map<String, Object>> {
    static { NativeLoader.load(); }
    private static final Cleaner CLEANER = Cleaner.create();
    private static final ObjectMapper MAPPER = new ObjectMapper();
    private static final TypeReference<Map<String, Object>> MAP_TYPE =
        new TypeReference<Map<String, Object>>() {};

    private final long ptr;
    private final Cleaner.Cleanable cleanable;

    Bundle(long ptr) {
        this.ptr = ptr;
        this.cleanable = CLEANER.register(this, () -> nativeFree(ptr));
    }

    public int objectCount() { return nativeObjectCount(ptr); }

    public Optional<Map<String, Object>> object(int index) {
        String json = nativeObject(ptr, index);
        if (json == null) {
            return Optional.empty();
        }
        try {
            return Optional.of(MAPPER.readValue(json, MAP_TYPE));
        } catch (Exception e) {
            throw new ModelException(e.getMessage());
        }
    }

    @Override
    public Iterator<Map<String, Object>> iterator() {
        return new Iterator<>() {
            private int i = 0;
            private final int n = objectCount();

            @Override
            public boolean hasNext() { return i < n; }

            @Override
            public Map<String, Object> next() {
                if (!hasNext()) {
                    throw new NoSuchElementException();
                }
                return object(i++).orElseThrow();
            }
        };
    }

    @Override
    public void close() { cleanable.clean(); }

    private static native int nativeObjectCount(long ptr);
    private static native String nativeObject(long ptr, int index);
    private static native void nativeFree(long ptr);
}
```

- [ ] **Step 5: Engine**

Create `Engine.java`:

```java
package io.github.benjaminsmall.stix;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import java.lang.ref.Cleaner;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Function;

/** Parses patterns/bundles and runs matches. Register custom-type hooks here. */
public final class Engine implements AutoCloseable {
    static { NativeLoader.load(); }
    private static final Cleaner CLEANER = Cleaner.create();
    private static final ObjectMapper MAPPER = new ObjectMapper();

    private final long ptr;
    private final Cleaner.Cleanable cleanable;
    private final Map<String, Function<Map<String, Object>, Map<String, Object>>> hooks =
        new HashMap<>();

    public Engine() {
        this.ptr = nativeNew();
        this.cleanable = CLEANER.register(this, () -> nativeFree(ptr));
    }

    public Pattern parsePattern(String src) {
        return new Pattern(nativeParsePattern(ptr, src));
    }

    public Bundle parseBundle(String json) {
        String toNative = json;
        if (!hooks.isEmpty()) {
            toNative = applyHooks(json);
        }
        return new Bundle(nativeParseBundle(ptr, toNative));
    }

    public MatchResult matchBundle(Pattern pattern, Bundle bundle) {
        String json = nativeMatchBundle(pattern.ptr(), bundle.ptr());
        try {
            JsonNode n = MAPPER.readTree(json);
            boolean matched = n.get("matched").asBoolean();
            List<Long> obs = new ArrayList<>();
            for (JsonNode o : n.get("observations")) {
                obs.add(o.asLong());
            }
            return new MatchResult(matched, obs);
        } catch (Exception e) {
            throw new MatchException(e.getMessage());
        }
    }

    public void registerType(
        String typeName, Function<Map<String, Object>, Map<String, Object>> hook) {
        hooks.put(typeName, hook);
    }

    @SuppressWarnings("unchecked")
    private String applyHooks(String json) {
        JsonNode root;
        try {
            root = MAPPER.readTree(json);
        } catch (Exception e) {
            throw new ModelException("invalid JSON: " + e.getMessage());
        }
        JsonNode objects = root.get("objects");
        if (objects != null && objects.isArray()) {
            ArrayNode arr = (ArrayNode) objects;
            for (int i = 0; i < arr.size(); i++) {
                JsonNode obj = arr.get(i);
                String type = obj.path("type").asText(null);
                Function<Map<String, Object>, Map<String, Object>> hook = hooks.get(type);
                if (hook != null) {
                    Map<String, Object> in = MAPPER.convertValue(obj, Map.class);
                    Map<String, Object> out;
                    try {
                        out = hook.apply(in);
                    } catch (RuntimeException e) {
                        throw new ValidationException(
                            e.getMessage() == null ? e.toString() : e.getMessage());
                    }
                    arr.set(i, MAPPER.valueToTree(out));
                }
            }
        }
        try {
            return MAPPER.writeValueAsString(root);
        } catch (Exception e) {
            throw new ModelException(e.getMessage());
        }
    }

    @Override
    public void close() { cleanable.clean(); }

    private static native long nativeNew();
    private static native void nativeFree(long ptr);
    private static native long nativeParsePattern(long enginePtr, String src);
    private static native long nativeParseBundle(long enginePtr, String json);
    private static native String nativeMatchBundle(long patternPtr, long bundlePtr);
}
```

- [ ] **Step 6: Verify it compiles**

Run: `cd bindings/java && gradle compileJava`
Expected: BUILD SUCCESSFUL.

- [ ] **Step 7: Commit**

```bash
git add bindings/java/src/main/java/io/github/benjaminsmall/stix/
git commit -m "feat(java): Java wrapper with Jackson deep structure and hooks"
```

---

## Task 5: JUnit 5 tests

**Files:**
- Create: `bindings/java/src/test/java/io/github/benjaminsmall/stix/StixTest.java`

- [ ] **Step 1: Write the tests**

Create `bindings/java/src/test/java/io/github/benjaminsmall/stix/StixTest.java`:

```java
package io.github.benjaminsmall.stix;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import org.junit.jupiter.api.Test;

class StixTest {
    private static final String BUNDLE = "{\"type\":\"bundle\",\"id\":\"bundle--1\","
        + "\"objects\":["
        + "{\"type\":\"ipv4-addr\",\"id\":\"ipv4-addr--1\",\"value\":\"198.51.100.5\"},"
        + "{\"type\":\"observed-data\",\"id\":\"observed-data--1\","
        + "\"first_observed\":\"2020-01-01T00:00:00Z\",\"last_observed\":\"2020-01-01T00:00:00Z\","
        + "\"number_observed\":1,\"object_refs\":[\"ipv4-addr--1\"]}]}";

    @Test
    void parsesPatternToAstMap() {
        try (Engine engine = new Engine();
             Pattern pattern = engine.parsePattern("[ipv4-addr:value = '198.51.100.5']")) {
            Map<String, Object> ast = pattern.ast();
            assertTrue(ast.toString().contains("ipv4-addr"));
        }
    }

    @Test
    void readsAndIteratesBundle() {
        try (Engine engine = new Engine();
             Bundle bundle = engine.parseBundle(BUNDLE)) {
            assertEquals(2, bundle.objectCount());
            assertEquals("ipv4-addr--1", bundle.object(0).orElseThrow().get("id"));
            assertTrue(bundle.object(99).isEmpty());
            List<Object> types = new ArrayList<>();
            for (Map<String, Object> o : bundle) {
                types.add(o.get("type"));
            }
            assertTrue(types.contains("observed-data"));
        }
    }

    @Test
    void matchesHitAndMiss() {
        try (Engine engine = new Engine();
             Bundle bundle = engine.parseBundle(BUNDLE)) {
            try (Pattern hit = engine.parsePattern("[ipv4-addr:value = '198.51.100.5']")) {
                MatchResult r = engine.matchBundle(hit, bundle);
                assertTrue(r.matched());
                assertFalse(r.observations().isEmpty());
            }
            try (Pattern miss = engine.parsePattern("[ipv4-addr:value = '203.0.113.9']")) {
                assertFalse(engine.matchBundle(miss, bundle).matched());
            }
        }
    }

    @Test
    void appliesCustomHookAndMatchesComputedProperty() {
        try (Engine engine = new Engine()) {
            engine.registerType("x-acme-widget", obj -> {
                long score = ((Number) obj.getOrDefault("risk_score", 0)).longValue();
                obj.put("risk_band", score > 80 ? "high" : "low");
                return obj;
            });
            String json = "{\"type\":\"bundle\",\"objects\":["
                + "{\"type\":\"x-acme-widget\",\"id\":\"x-acme-widget--1\",\"risk_score\":90},"
                + "{\"type\":\"observed-data\",\"id\":\"observed-data--1\","
                + "\"first_observed\":\"2020-01-01T00:00:00Z\",\"last_observed\":\"2020-01-01T00:00:00Z\","
                + "\"number_observed\":1,\"object_refs\":[\"x-acme-widget--1\"]}]}";
            try (Bundle bundle = engine.parseBundle(json);
                 Pattern pattern = engine.parsePattern("[x-acme-widget:risk_band = 'high']")) {
                assertTrue(engine.matchBundle(pattern, bundle).matched());
            }
        }
    }

    @Test
    void mapsErrorsToExceptionHierarchy() {
        try (Engine engine = new Engine()) {
            assertThrows(ParseException.class, () -> engine.parsePattern("[bad"));
            assertThrows(ModelException.class,
                () -> engine.parseBundle("{\"type\":\"ipv4-addr\",\"id\":\"x--1\"}"));
            engine.registerType("x-thing", obj -> { throw new RuntimeException("nope"); });
            assertThrows(ValidationException.class, () -> engine.parseBundle(
                "{\"type\":\"bundle\",\"objects\":[{\"type\":\"x-thing\",\"id\":\"x--1\"}]}"));
            StixException ex = assertThrows(StixException.class,
                () -> engine.parsePattern("[bad"));
            assertInstanceOf(StixException.class, ex);
        }
    }
}
```

- [ ] **Step 2: Run the tests**

Run: `cd bindings/java && gradle test`
Expected: all 5 tests pass. (`gradle test` builds the native lib via the `cargoBuild`
dependency and sets `java.library.path`.)

- [ ] **Step 3: Commit**

```bash
git add bindings/java/src/test/
git commit -m "test(java): JUnit suite for the Java binding"
```

---

## Task 6: README + final verification

**Files:**
- Modify: `bindings/java/README.md`
- Create: `bindings/java/.gitignore`

- [ ] **Step 1: gitignore build artifacts**

Create `bindings/java/.gitignore`:

```
.gradle/
build/
rust/target/
```

- [ ] **Step 2: Replace the placeholder README**

Overwrite `bindings/java/README.md`:

```markdown
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
```

- [ ] **Step 3: Final verification**

Run: `cd bindings/java && gradle test`
Expected: 5 JUnit tests pass.

Run: `cd bindings/java/rust && cargo clippy -- -D warnings`
Expected: clippy clean on the JNI crate.

Run (repo root): `cargo test 2>&1 | grep -c "test result: ok"`
Expected: non-zero, unchanged.

- [ ] **Step 4: Commit**

```bash
git add bindings/java/README.md bindings/java/.gitignore
git commit -m "docs(java): real README and gitignore"
```

---

## Self-Review Notes (already applied)

- **Spec coverage:** jni cdylib + Gradle scaffold, workspace-excluded (Task 1);
  `StixException` unchecked hierarchy (Task 2); JNI String↔String handles + exception
  throwing (Task 3); Java wrapper — `Engine`/`Pattern.ast`(Map)/`Bundle`
  (count/object/iterate)/`MatchResult`, Jackson, Java-side `registerType` hooks with
  `ValidationException`, `AutoCloseable` + `Cleaner` (Task 4); JUnit suite (Task 5);
  README (Task 6). Package `io.github.benjaminsmall.stix`; native lib `stix_java`. All
  spec points map to a task.
- **Refinement:** hooks applied Java-side (documented), so the JNI layer has no
  callback bridging — behavior identical to the spec.
- **Type consistency:** `Engine.{parsePattern,parseBundle,matchBundle,registerType,
  close}`, `Pattern.{ast,close}`, `Bundle.{objectCount,object,iterator,close}`,
  `MatchResult.{matched,observations}`, `Stix*Exception`, and the `native*` method
  names match between the Java classes and the JNI function names in `lib.rs`, and
  wrap the verified `stix_ffi` API. `nativeMatchBundle` is declared on `Engine` and
  exported as `..._Engine_nativeMatchBundle`.
- **Toolchain note** scopes jni-0.21/Gradle adaptation; API/wrapper/tests are exact.
- **No placeholders:** every step has complete content or an exact command.
```
