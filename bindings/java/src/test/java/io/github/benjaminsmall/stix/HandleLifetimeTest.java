package io.github.benjaminsmall.stix;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ConcurrentLinkedQueue;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Test;

class HandleLifetimeTest {
    private static final String BUNDLE = "{\"type\":\"bundle\",\"id\":\"bundle--1\","
        + "\"objects\":["
        + "{\"type\":\"ipv4-addr\",\"id\":\"ipv4-addr--1\",\"value\":\"198.51.100.5\"},"
        + "{\"type\":\"observed-data\",\"id\":\"observed-data--1\","
        + "\"first_observed\":\"2020-01-01T00:00:00Z\",\"last_observed\":\"2020-01-01T00:00:00Z\","
        + "\"number_observed\":1,\"object_refs\":[\"ipv4-addr--1\"]}]}";
    private static final String PATTERN = "[ipv4-addr:value = '198.51.100.5']";

    @Test
    void patternMethodsThrowAfterClose() {
        try (Engine engine = new Engine()) {
            Pattern p = engine.parsePattern(PATTERN);
            p.close();
            IllegalStateException e = assertThrows(IllegalStateException.class, p::ast);
            assertTrue(e.getMessage().contains("closed"), e.getMessage());
        }
    }

    @Test
    void bundleMethodsThrowAfterClose() {
        try (Engine engine = new Engine()) {
            Bundle b = engine.parseBundle(BUNDLE);
            b.close();
            assertThrows(IllegalStateException.class, b::objectCount);
            assertThrows(IllegalStateException.class, () -> b.object(0));
            assertThrows(IllegalStateException.class, b::iterator);
        }
    }

    @Test
    void iteratorThrowsIfBundleClosedMidIteration() {
        try (Engine engine = new Engine()) {
            Bundle b = engine.parseBundle(BUNDLE);
            var it = b.iterator();
            assertTrue(it.hasNext());
            b.close();
            assertThrows(IllegalStateException.class, it::next);
        }
    }

    @Test
    void engineMethodsThrowAfterClose() {
        Engine engine = new Engine();
        Pattern p = engine.parsePattern(PATTERN);
        Bundle b = engine.parseBundle(BUNDLE);
        engine.close();
        assertThrows(IllegalStateException.class, () -> engine.parsePattern(PATTERN));
        assertThrows(IllegalStateException.class, () -> engine.parseBundle(BUNDLE));
        // Patterns and bundles do not depend on the engine that made them.
        try (Engine other = new Engine()) {
            assertTrue(other.matchBundle(p, b).matched());
        }
        p.close();
        b.close();
    }

    @Test
    void matchBundleRejectsClosedPatternBundleAndEngine() {
        Engine engine = new Engine();
        Pattern p = engine.parsePattern(PATTERN);
        Bundle b = engine.parseBundle(BUNDLE);
        assertTrue(engine.matchBundle(p, b).matched());

        Pattern closedP = engine.parsePattern(PATTERN);
        closedP.close();
        assertThrows(IllegalStateException.class, () -> engine.matchBundle(closedP, b));

        Bundle closedB = engine.parseBundle(BUNDLE);
        closedB.close();
        assertThrows(IllegalStateException.class, () -> engine.matchBundle(p, closedB));

        engine.close();
        assertThrows(IllegalStateException.class, () -> engine.matchBundle(p, b));
        p.close();
        b.close();
    }

    @Test
    void doubleCloseIsIdempotent() {
        Engine engine = new Engine();
        Pattern p = engine.parsePattern(PATTERN);
        Bundle b = engine.parseBundle(BUNDLE);
        engine.close();
        engine.close();
        p.close();
        p.close();
        b.close();
        b.close();
    }

    @Test
    void tryWithResourcesStillWorks() {
        try (Engine engine = new Engine();
             Pattern p = engine.parsePattern(PATTERN);
             Bundle b = engine.parseBundle(BUNDLE)) {
            assertFalse(p.ast().isEmpty());
            assertEquals(2, b.objectCount());
            assertTrue(engine.matchBundle(p, b).matched());
        }
    }

    @Test
    void closeWaitsForInFlightCallThenLaterCallsThrow() throws Exception {
        try (Engine engine = new Engine()) {
            Pattern p = engine.parsePattern(PATTERN);
            CountDownLatch inside = new CountDownLatch(1);
            CountDownLatch release = new CountDownLatch(1);
            p.inReadLockHook = () -> {
                inside.countDown();
                try {
                    release.await();
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                }
            };
            ExecutorService pool = Executors.newFixedThreadPool(1);
            try {
                Future<Object> reader = pool.submit(() -> p.ast());
                assertTrue(inside.await(10, TimeUnit.SECONDS));
                p.inReadLockHook = () -> { };

                Thread closer = new Thread(p::close);
                closer.start();
                closer.join(300);
                assertTrue(closer.isAlive(), "close() must wait for the in-flight call");

                release.countDown();
                assertFalse(reader.get(10, TimeUnit.SECONDS).toString().isEmpty());
                closer.join(10_000);
                assertFalse(closer.isAlive(), "close() must complete after the call ends");
                assertThrows(IllegalStateException.class, p::ast);
            } finally {
                release.countDown();
                pool.shutdownNow();
            }
        }
    }

    @Test
    void concurrentUseAndCloseYieldsResultsOrIllegalStateOnly() throws Exception {
        int threads = 8;
        for (int round = 0; round < 20; round++) {
            Engine engine = new Engine();
            Pattern p = engine.parsePattern(PATTERN);
            Bundle b = engine.parseBundle(BUNDLE);
            ExecutorService pool = Executors.newFixedThreadPool(threads + 1);
            CountDownLatch start = new CountDownLatch(1);
            ConcurrentLinkedQueue<Throwable> bad = new ConcurrentLinkedQueue<>();
            List<Future<?>> fs = new ArrayList<>();
            for (int t = 0; t < threads; t++) {
                fs.add(pool.submit(() -> {
                    start.await();
                    for (int i = 0; i < 200; i++) {
                        try {
                            assertFalse(p.ast().isEmpty());
                            assertTrue(engine.matchBundle(p, b).matched());
                            assertEquals(2, b.objectCount());
                        } catch (IllegalStateException expected) {
                            // closed: acceptable
                        } catch (Throwable x) {
                            bad.add(x);
                        }
                    }
                    return null;
                }));
            }
            fs.add(pool.submit(() -> {
                start.await();
                Thread.sleep(1);
                p.close();
                b.close();
                return null;
            }));
            start.countDown();
            for (Future<?> f : fs) {
                f.get(30, TimeUnit.SECONDS);
            }
            pool.shutdown();
            engine.close();
            assertTrue(bad.isEmpty(), String.valueOf(bad.peek()));
            assertThrows(IllegalStateException.class, p::ast);
        }
    }
}
