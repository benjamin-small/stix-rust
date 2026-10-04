package io.github.benjaminsmall.stix;

import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.util.Map;

/**
 * A parsed pattern. Holds a native handle; close() (or GC) frees it. Using it after
 * close() throws {@link IllegalStateException}.
 */
public final class Pattern extends NativeHandle {
    static { NativeLoader.load(); }
    private static final ObjectMapper MAPPER = new ObjectMapper();

    Pattern(long ptr) {
        super("Pattern", ptr, Pattern::nativeFree);
    }

    /**
     * The pattern's AST as a Map.
     *
     * @throws IllegalStateException if the pattern is closed
     */
    public Map<String, Object> ast() {
        String json = useHandle(Pattern::nativeAst);
        try {
            return MAPPER.readValue(json, new TypeReference<Map<String, Object>>() {});
        } catch (Exception e) {
            throw new ModelException(e.getMessage());
        }
    }

    private static native String nativeAst(long ptr);
    private static native void nativeFree(long ptr);
}
