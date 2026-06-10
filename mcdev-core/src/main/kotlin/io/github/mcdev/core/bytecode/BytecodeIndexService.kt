package io.github.mcdev.core.bytecode

import io.github.mcdev.core.model.MappingNamespace

sealed interface BytecodeIndexResult<out T> {
    data class Success<T>(val value: T) : BytecodeIndexResult<T>
    data class Failure(val error: BytecodeIndexError) : BytecodeIndexResult<Nothing>
}

class BytecodeIndexService(
    private val cache: BytecodeIndexCache = BytecodeIndexCache(),
) {
    fun buildIndex(provider: ClassBytesProvider, classpathEntryId: String? = null): ClassMemberIndex {
        val hash = provider.classpathHash()
        cache.getIndex(hash)?.let { return it.memberIndex }

        val index = ClassMemberIndexBuilder.build(provider, classpathEntryId)
        val cacheKeys = buildCacheKeys(provider)
        cache.putIndex(
            CachedBytecodeIndex(
                classpathHash = hash,
                memberIndex = index,
                classCacheKeys = cacheKeys,
            ),
        )
        return index
    }

    fun extractInstructions(
        provider: ClassBytesProvider,
        ownerInternalName: String,
        methodName: String,
        methodDescriptor: String,
        namespace: MappingNamespace = MappingNamespace.NAMED,
        classpathEntryId: String = provider.classpathEntryIds().firstOrNull() ?: "default",
    ): BytecodeIndexResult<List<AtTargetCandidate>> {
        val classBytes = provider.getClassBytes(ownerInternalName)
            ?: return BytecodeIndexResult.Failure(ClassBytesMissingError(ownerInternalName))

        val index = buildIndex(provider)
        if (index.findMethod(ownerInternalName, methodName, methodDescriptor) == null) {
            return BytecodeIndexResult.Failure(
                MethodNotFoundError(ownerInternalName, methodName, methodDescriptor),
            )
        }

        val cacheKey = MethodInstructionCacheKey(
            classRef = BytecodeClassRef(ownerInternalName, classpathEntryId),
            methodName = methodName,
            descriptor = methodDescriptor,
            namespace = namespace,
        )
        cache.getInstructions(cacheKey)?.let { return BytecodeIndexResult.Success(it) }

        val candidates = InstructionExtractor.extract(classBytes, methodName, methodDescriptor)
        cache.putInstructions(cacheKey, candidates)
        return BytecodeIndexResult.Success(candidates)
    }

    fun getCachedIndex(classpathHash: String): CachedBytecodeIndex? = cache.getIndex(classpathHash)

    fun cache(): BytecodeIndexCache = cache

    private fun buildCacheKeys(provider: ClassBytesProvider): Map<String, BytecodeCacheKey> {
        val entryIds = provider.classpathEntryIds()
        return when (provider) {
            is InMemoryClassBytesProvider -> {
                provider.internalNames().associateWith { internalName ->
                    val entryId = entryIds.firstOrNull() ?: "in-memory"
                    val lastModified = provider.classpathEntryHash(entryId) ?: 0L
                    BytecodeCacheKey(entryId, lastModified, internalName)
                }
            }
            else -> emptyMap()
        }
    }
}
