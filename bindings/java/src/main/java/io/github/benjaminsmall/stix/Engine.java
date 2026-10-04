package io.github.benjaminsmall.stix;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import java.util.ArrayList;
import java.util.concurrent.ConcurrentHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.function.Function;

/**
 * Parses patterns/bundles and runs matches. Register custom-type hooks here. Using a
 * closed engine throws {@link IllegalStateException}. Parsing and matching may be called
 * from several threads, and registerType is thread-safe (hooks live in a concurrent map).
 */
public final class Engine extends NativeHandle {
    static { NativeLoader.load(); }
    private static final ObjectMapper MAPPER = new ObjectMapper();

    private final Map<String, Function<Map<String, Object>, Map<String, Object>>> hooks =
        new ConcurrentHashMap<>();

    public Engine() {
        super("Engine", nativeNew(), Engine::nativeFree);
    }

    /** @throws IllegalStateException if the engine is closed */
    public Pattern parsePattern(String src) {
        return new Pattern(useHandle(p -> nativeParsePattern(p, src)));
    }

    /** @throws IllegalStateException if the engine is closed */
    public Bundle parseBundle(String json) {
        checkOpen();
        String toNative = json;
        if (!hooks.isEmpty()) {
            toNative = applyHooks(json);
        }
        final String payload = toNative;
        return new Bundle(useHandle(p -> nativeParseBundle(p, payload)));
    }

    /** @throws IllegalStateException if the engine, pattern or bundle is closed */
    public MatchResult matchBundle(Pattern pattern, Bundle bundle) {
        checkOpen();
        String json = Objects.requireNonNull(pattern, "pattern")
            .useHandle(pp -> Objects.requireNonNull(bundle, "bundle")
                .useHandle(bp -> nativeMatchBundle(pp, bp)));
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
                Function<Map<String, Object>, Map<String, Object>> hook = type == null ? null : hooks.get(type);
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

    private static native long nativeNew();
    private static native void nativeFree(long ptr);
    private static native long nativeParsePattern(long enginePtr, String src);
    private static native long nativeParseBundle(long enginePtr, String json);
    private static native String nativeMatchBundle(long patternPtr, long bundlePtr);
}
