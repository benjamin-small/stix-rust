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

    long ptr() { return ptr; }

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
