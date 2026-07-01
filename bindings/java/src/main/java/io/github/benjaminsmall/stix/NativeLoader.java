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
