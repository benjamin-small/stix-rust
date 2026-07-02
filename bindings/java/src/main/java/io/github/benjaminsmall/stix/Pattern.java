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
