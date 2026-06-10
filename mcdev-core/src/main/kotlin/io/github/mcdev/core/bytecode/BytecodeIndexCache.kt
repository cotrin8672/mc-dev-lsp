package io.github.mcdev.core.bytecode

import io.github.mcdev.core.model.MappingNamespace
import java.nio.file.Path

data class BytecodeCacheKey(
    val classpathEntryId: String,
    val lastModified: Long,
    val internalName: String,
)

data class BytecodeClassRef(
    val internalName: String,
    val classpathEntryId: String,
)

data class MethodInstructionCacheKey(
    val classRef: BytecodeClassRef,
    val methodName: String,
    val descriptor: String,
    val namespace: MappingNamespace = MappingNamespace.NAMED,
)

data class CachedBytecodeIndex(
    val classpathHash: String,
    val memberIndex: ClassMemberIndex,
    val classCacheKeys: Map<String, BytecodeCacheKey>,
)

class BytecodeIndexCache {
    private var cachedIndex: CachedBytecodeIndex? = null
    private val instructionCache = mutableMapOf<MethodInstructionCacheKey, List<AtTargetCandidate>>()

    fun getIndex(classpathHash: String): CachedBytecodeIndex? {
        val cached = cachedIndex ?: return null
        return if (cached.classpathHash == classpathHash) cached else null
    }

    fun putIndex(index: CachedBytecodeIndex) {
        if (cachedIndex?.classpathHash != index.classpathHash) {
            instructionCache.clear()
        }
        cachedIndex = index
    }

    fun getInstructions(key: MethodInstructionCacheKey): List<AtTargetCandidate>? =
        instructionCache[key]

    fun putInstructions(key: MethodInstructionCacheKey, candidates: List<AtTargetCandidate>) {
        instructionCache[key] = candidates
    }

    fun invalidateClasspathEntry(entryId: String, newLastModified: Long) {
        val cached = cachedIndex ?: return
        val affectedClasses = cached.classCacheKeys
            .filterValues { it.classpathEntryId == entryId && it.lastModified != newLastModified }
            .keys

        if (affectedClasses.isEmpty()) return

        val updatedKeys = cached.classCacheKeys.toMutableMap()
        affectedClasses.forEach { internalName ->
            updatedKeys[internalName] = BytecodeCacheKey(entryId, newLastModified, internalName)
            instructionCache.keys.removeIf { it.classRef.internalName == internalName }
        }

        cachedIndex = cached.copy(classCacheKeys = updatedKeys)
    }

    fun invalidateClass(internalName: String) {
        instructionCache.keys.removeIf { it.classRef.internalName == internalName }
        val cached = cachedIndex ?: return
        if (internalName in cached.classCacheKeys) {
            cachedIndex = cached.copy(
                classCacheKeys = cached.classCacheKeys - internalName,
            )
        }
    }

    fun clear() {
        cachedIndex = null
        instructionCache.clear()
    }

    companion object {
        fun cacheKeyFor(
            classpathEntryId: String,
            lastModified: Long,
            internalName: String,
        ): BytecodeCacheKey = BytecodeCacheKey(classpathEntryId, lastModified, internalName)

        fun cacheKeyForPath(
            classpathEntry: Path,
            internalName: String,
        ): BytecodeCacheKey = BytecodeCacheKey(
            classpathEntryId = classpathEntry.toString(),
            lastModified = classpathEntry.toFile().lastModified(),
            internalName = internalName,
        )
    }
}
