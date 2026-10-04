package io.github.benjaminsmall.stix;

import java.lang.ref.Cleaner;
import java.lang.ref.Reference;
import java.util.concurrent.locks.ReentrantReadWriteLock;
import java.util.function.LongConsumer;
import java.util.function.LongFunction;

/**
 * Base class for objects owning a native pointer.
 *
 * <p>Lifetime rules:
 * <ul>
 *   <li>Every native call goes through {@link #useHandle}, which takes the read side of a
 *       per-handle lock, throws {@link IllegalStateException} if the handle is closed, and
 *       fences the handle with {@link Reference#reachabilityFence} so the {@link Cleaner}
 *       cannot free it mid-call.</li>
 *   <li>{@link #close()} takes the write side, so it waits for in-flight native calls and
 *       then frees the pointer exactly once. It is idempotent.</li>
 *   <li>Calls on one handle from many threads run concurrently; close() is safe to race
 *       with them (they either complete or throw IllegalStateException).</li>
 * </ul>
 */
abstract class NativeHandle implements AutoCloseable {
    private static final Cleaner CLEANER = Cleaner.create();

    private final String kind;
    private final long ptr;
    private final Cleaner.Cleanable cleanable;
    private final ReentrantReadWriteLock lock = new ReentrantReadWriteLock();
    private boolean closed; // guarded by lock

    NativeHandle(String kind, long ptr, LongConsumer free) {
        this.kind = kind;
        this.ptr = ptr;
        // The action must not capture `this`, or the handle could never become unreachable.
        this.cleanable = CLEANER.register(this, () -> free.accept(ptr));
    }

    /** Test seam: invoked inside the read lock, before the native call. No-op by default. */
    volatile Runnable inReadLockHook = () -> { };

    /** Throws IllegalStateException if the handle is closed. */
    final void checkOpen() {
        useHandle(p -> null);
    }

    /** Runs {@code f} with the live native pointer; throws if the handle is closed. */
    final <T> T useHandle(LongFunction<T> f) {
        lock.readLock().lock();
        try {
            if (closed) {
                throw new IllegalStateException(kind + " is closed");
            }
            inReadLockHook.run();
            return f.apply(ptr);
        } finally {
            lock.readLock().unlock();
            Reference.reachabilityFence(this);
        }
    }

    /** Idempotent. Waits for in-flight native calls on this handle, then frees it. */
    @Override
    public final void close() {
        lock.writeLock().lock();
        try {
            if (!closed) {
                closed = true;
                cleanable.clean();
            }
        } finally {
            lock.writeLock().unlock();
        }
    }
}
