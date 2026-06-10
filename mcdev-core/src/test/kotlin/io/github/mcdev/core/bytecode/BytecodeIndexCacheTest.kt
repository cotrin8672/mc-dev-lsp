package io.github.mcdev.core.bytecode

import io.github.mcdev.core.model.MappingNamespace
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull

class BytecodeIndexCacheTest {
    @Test
    fun storesAndRetrievesIndexByClasspathHash() {
        val cache = BytecodeIndexCache()
        val index = ClassMemberIndexBuilder.build(BytecodeFixtureCompiler.provider())
        val cached = CachedBytecodeIndex("hash-1", index, emptyMap())
        cache.putIndex(cached)
        assertEquals(cached, cache.getIndex("hash-1"))
    }

    @Test
    fun returnsNullForMismatchedClasspathHash() {
        val cache = BytecodeIndexCache()
        cache.putIndex(CachedBytecodeIndex("hash-1", ClassMemberIndex(emptyMap(), emptyMap(), emptyMap()), emptyMap()))
        assertNull(cache.getIndex("hash-2"))
    }

    @Test
    fun storesAndRetrievesInstructionCache() {
        val cache = BytecodeIndexCache()
        val key = MethodInstructionCacheKey(
            classRef = BytecodeClassRef("demo/Class", "fixture.jar"),
            methodName = "run",
            descriptor = "()V",
            namespace = MappingNamespace.NAMED,
        )
        val candidates = listOf(AtTargetCandidate("demo/Class", "run", "()V", 0, AtTargetKind.INVOKE_VIRTUAL))
        cache.putInstructions(key, candidates)
        assertEquals(candidates, cache.getInstructions(key))
    }

    @Test
    fun invalidatesInstructionsForStaleClasspathEntry() {
        val cache = BytecodeIndexCache()
        val className = BytecodeFixtureCompiler.internalName("InvokeSamples")
        cache.putIndex(
            CachedBytecodeIndex(
                classpathHash = "hash-1",
                memberIndex = ClassMemberIndexBuilder.build(BytecodeFixtureCompiler.provider()),
                classCacheKeys = mapOf(
                    className to BytecodeCacheKey("fixture.jar", 1L, className),
                ),
            ),
        )
        val key = MethodInstructionCacheKey(
            classRef = BytecodeClassRef(className, "fixture.jar"),
            methodName = "virtualInvoke",
            descriptor = "()V",
        )
        cache.putInstructions(key, listOf(AtTargetCandidate("x", "y", "()V", 0, AtTargetKind.INVOKE_VIRTUAL)))
        cache.invalidateClasspathEntry("fixture.jar", 2L)
        assertNull(cache.getInstructions(key))
        val updated = assertNotNull(cache.getIndex("hash-1"))
        assertEquals(2L, updated.classCacheKeys[className]?.lastModified)
    }

    @Test
    fun clearRemovesAllCachedState() {
        val cache = BytecodeIndexCache()
        cache.putIndex(CachedBytecodeIndex("hash-1", ClassMemberIndex(emptyMap(), emptyMap(), emptyMap()), emptyMap()))
        cache.clear()
        assertNull(cache.getIndex("hash-1"))
    }

    @Test
    fun invalidateClassRemovesInstructionCacheEntries() {
        val cache = BytecodeIndexCache()
        val className = "demo/Class"
        val key = MethodInstructionCacheKey(
            classRef = BytecodeClassRef(className, "fixture.jar"),
            methodName = "run",
            descriptor = "()V",
        )
        cache.putInstructions(key, emptyList())
        cache.invalidateClass(className)
        assertNull(cache.getInstructions(key))
    }

    @Test
    fun cacheKeyForPathUsesFileLastModified() {
        val temp = java.nio.file.Files.createTempFile("fixture", ".jar")
        val key = BytecodeIndexCache.cacheKeyForPath(temp, "demo/Class")
        assertEquals(temp.toString(), key.classpathEntryId)
        assertEquals("demo/Class", key.internalName)
        java.nio.file.Files.deleteIfExists(temp)
    }
}
