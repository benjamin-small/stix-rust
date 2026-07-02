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
