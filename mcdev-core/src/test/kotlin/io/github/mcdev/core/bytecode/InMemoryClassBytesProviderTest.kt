package io.github.mcdev.core.bytecode

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

class InMemoryClassBytesProviderTest {
    @Test
    fun returnsClassBytesByInternalName() {
        val bytes = byteArrayOf(0xCA.toByte(), 0xFE.toByte(), 0xBA.toByte(), 0xBE.toByte())
        val provider = InMemoryClassBytesProvider(mapOf("demo/Sample" to bytes))
        assertEquals(bytes.toList(), provider.getClassBytes("demo/Sample")?.toList())
    }

    @Test
    fun returnsNullForMissingClass() {
        val provider = InMemoryClassBytesProvider(emptyMap())
        assertNull(provider.getClassBytes("missing/Class"))
    }

    @Test
    fun rejectsDottedInternalNames() {
        val result = runCatching {
            InMemoryClassBytesProvider(mapOf("demo.Sample" to byteArrayOf(1)))
        }
        assertTrue(result.isFailure)
    }

    @Test
    fun exposesInternalNames() {
        val provider = InMemoryClassBytesProvider(
            mapOf(
                "a/A" to byteArrayOf(1),
                "b/B" to byteArrayOf(2),
            ),
        )
        assertEquals(setOf("a/A", "b/B"), provider.internalNames())
    }

    @Test
    fun classpathHashChangesWhenBytesChange() {
        val first = InMemoryClassBytesProvider(mapOf("a/A" to byteArrayOf(1)))
        val second = InMemoryClassBytesProvider(mapOf("a/A" to byteArrayOf(2)))
        assertNotEquals(first.classpathHash(), second.classpathHash())
    }

    @Test
    fun classpathHashStableForSameContent() {
        val classes = mapOf("a/A" to byteArrayOf(1, 2, 3))
        val first = InMemoryClassBytesProvider(classes)
        val second = InMemoryClassBytesProvider(classes)
        assertEquals(first.classpathHash(), second.classpathHash())
    }

    @Test
    fun reportsDefaultClasspathEntryId() {
        val provider = InMemoryClassBytesProvider(mapOf("a/A" to byteArrayOf(1)))
        assertEquals(setOf("in-memory"), provider.classpathEntryIds())
    }

    @Test
    fun reportsConfiguredEntryHashes() {
        val provider = InMemoryClassBytesProvider(
            classes = mapOf("a/A" to byteArrayOf(1)),
            entryHashes = mapOf("fixture.jar" to 42L),
        )
        assertEquals(setOf("fixture.jar"), provider.classpathEntryIds())
        assertEquals(42L, provider.classpathEntryHash("fixture.jar"))
    }
}
