package io.github.benjaminsmall.stix;

import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.util.Iterator;
import java.util.Map;
import java.util.NoSuchElementException;
import java.util.Optional;

/**
 * An imported bundle. Iterable over its objects (each a Map). Using it (or an iterator
 * obtained from it) after close() throws {@link IllegalStateException}.
 */
public final class Bundle extends NativeHandle implements Iterable<Map<String, Object>> {
    static { NativeLoader.load(); }
    private static final ObjectMapper MAPPER = new ObjectMapper();
    private static final TypeReference<Map<String, Object>> MAP_TYPE =
        new TypeReference<Map<String, Object>>() {};

    Bundle(long ptr) {
        super("Bundle", ptr, Bundle::nativeFree);
    }

    /** @throws IllegalStateException if the bundle is closed */
    public int objectCount() { return useHandle(Bundle::nativeObjectCount); }

    /** @throws IllegalStateException if the bundle is closed */
    public Optional<Map<String, Object>> object(int index) {
        String json = useHandle(p -> nativeObject(p, index));
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

    private static native int nativeObjectCount(long ptr);
    private static native String nativeObject(long ptr, int index);
    private static native void nativeFree(long ptr);
}
