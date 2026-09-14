package io.github.mcdev.jdtls.mixin

import java.util.concurrent.Callable
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotSame
import kotlin.test.assertTrue

class JdtInjectionPointFeatureProbeTest {
    private val specifierFqn = "org.spongepowered.asm.mixin.injection.InjectionPoint\$Specifier"

    @Test
    fun nullProjectReturnsFalseWithoutThrowing() {
        val probe = JdtInjectionPointFeatureProbe()
        assertFalse(probe.injectionPointSpecifierSupported(null, 0L))
    }

    @Test
    fun presentTypeReturnsTrue() {
        var calls = 0
        val probe = countingProbe(onLookup = {
            calls++
            Any()
        })

        assertTrue(probe.injectionPointSpecifierSupported(Any(), 0L))
        assertEquals(1, calls)
    }

    @Test
    fun missingTypeReturnsFalse() {
        val probe = countingProbe(onLookup = { null })

        assertFalse(probe.injectionPointSpecifierSupported(Any(), 0L))
    }

    @Test
    fun binaryTypeCountsAsPresent() {
        val probe = countingProbe(onLookup = { BinaryTypeMarker })

        assertTrue(probe.injectionPointSpecifierSupported(Any(), 0L))
    }

    @Test
    fun lookupExceptionFailsClosed() {
        val probe = countingProbe(onLookup = { error("findType failed") })

        assertFalse(probe.injectionPointSpecifierSupported(Any(), 0L))
    }

    @Test
    fun cachesExactlyOnceForSameProjectIdentityAndVersion() {
        var calls = 0
        val probe = countingProbe(onLookup = {
            calls++
            Any()
        })
        val project = Any()

        assertTrue(probe.injectionPointSpecifierSupported(project, 7L))
        assertTrue(probe.injectionPointSpecifierSupported(project, 7L))
        assertEquals(1, calls)
    }

    @Test
    fun recomputesWhenProjectSessionVersionChanges() {
        var calls = 0
        val probe = countingProbe(onLookup = {
            calls++
            Any()
        })
        val project = Any()

        assertTrue(probe.injectionPointSpecifierSupported(project, 1L))
        assertTrue(probe.injectionPointSpecifierSupported(project, 2L))
        assertEquals(2, calls)
    }

    @Test
    fun equalButDistinctProjectObjectsDoNotShareCache() {
        var calls = 0
        val probe = countingProbe(onLookup = {
            calls++
            Any()
        })
        val first = EqualFakeJavaProject("project-a")
        val second = EqualFakeJavaProject("project-a")
        assertEquals(first, second)
        assertNotSame(first, second)

        assertTrue(probe.injectionPointSpecifierSupported(first, 1L))
        assertTrue(probe.injectionPointSpecifierSupported(second, 1L))
        assertEquals(2, calls)
    }

    @Test
    fun concurrentIdenticalProbesPerformSingleLookup() {
        val calls = AtomicInteger(0)
        val readyGate = CountDownLatch(32)
        val startGate = CountDownLatch(1)
        val probe = JdtInjectionPointFeatureProbe { _, fqn ->
            assertEquals(specifierFqn, fqn)
            calls.incrementAndGet()
            startGate.await(5, TimeUnit.SECONDS)
            Any()
        }
        val project = Any()
        val executor = Executors.newFixedThreadPool(32)
        val results = Array(32) { false }
        try {
            repeat(32) { index ->
                executor.submit {
                    readyGate.countDown()
                    results[index] = probe.injectionPointSpecifierSupported(project, 42L)
                }
            }
            assertTrue(readyGate.await(5, TimeUnit.SECONDS), "worker threads did not become ready")
            val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
            while (calls.get() < 1 && System.nanoTime() < deadline) {
                Thread.yield()
            }
            assertEquals(1, calls.get(), "expected exactly one thread to begin lookup")
            startGate.countDown()
            executor.shutdown()
            assertTrue(executor.awaitTermination(5, TimeUnit.SECONDS), "executor did not finish in time")
            assertEquals(1, calls.get())
            assertTrue(results.all { it })
        } finally {
            startGate.countDown()
            executor.shutdownNow()
            executor.awaitTermination(5, TimeUnit.SECONDS)
        }
    }

    @Test
    fun distinctProjectSessionVersionsPerformConcurrentLookups() {
        val calls = AtomicInteger(0)
        val inLookup = CountDownLatch(2)
        val release = CountDownLatch(1)
        val probe = JdtInjectionPointFeatureProbe { _, fqn ->
            assertEquals(specifierFqn, fqn)
            if (calls.incrementAndGet() <= 2) {
                inLookup.countDown()
                assertTrue(release.await(5, TimeUnit.SECONDS), "lookups were not released")
            }
            Any()
        }
        val project = Any()
        val executor = Executors.newFixedThreadPool(2)
        try {
            val first = executor.submit(Callable { probe.injectionPointSpecifierSupported(project, 1L) })
            val second = executor.submit(Callable { probe.injectionPointSpecifierSupported(project, 2L) })
            assertTrue(inLookup.await(5, TimeUnit.SECONDS), "both lookups did not start concurrently")
            assertEquals(2, calls.get(), "expected one lookup per distinct cache key")
            release.countDown()
            assertTrue(first.get(5, TimeUnit.SECONDS))
            assertTrue(second.get(5, TimeUnit.SECONDS))
        } finally {
            release.countDown()
            executor.shutdownNow()
            executor.awaitTermination(5, TimeUnit.SECONDS)
        }
    }

    @Test
    fun lookupExceptionIsNotCachedAndNextRequestRetries() {
        val calls = AtomicInteger(0)
        val probe = countingProbe(onLookup = {
            if (calls.incrementAndGet() == 1) {
                error("transient findType failure")
            }
            Any()
        })
        val project = Any()

        assertFalse(probe.injectionPointSpecifierSupported(project, 0L))
        assertTrue(probe.injectionPointSpecifierSupported(project, 0L))
        assertEquals(2, calls.get())
    }

    @Test
    fun lookupUsesInjectionPointSpecifierFqn() {
        val observed = mutableListOf<String>()
        val probe = JdtInjectionPointFeatureProbe { _, fqn ->
            observed += fqn
            Any()
        }

        probe.injectionPointSpecifierSupported(Any(), 0L)

        assertEquals(listOf(specifierFqn), observed)
    }

    private fun countingProbe(onLookup: (Any) -> Any?): JdtInjectionPointFeatureProbe =
        JdtInjectionPointFeatureProbe { project, fqn ->
            assertEquals(specifierFqn, fqn)
            onLookup(project)
        }

    private class EqualFakeJavaProject(private val id: String) {
        override fun equals(other: Any?): Boolean =
            other is EqualFakeJavaProject && id == other.id

        override fun hashCode(): Int = id.hashCode()
    }

    private object BinaryTypeMarker
}
