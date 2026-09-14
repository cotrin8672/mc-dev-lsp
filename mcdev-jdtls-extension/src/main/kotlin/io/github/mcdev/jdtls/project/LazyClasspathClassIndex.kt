package io.github.mcdev.jdtls.project

import io.github.mcdev.core.bytecode.ClassMemberIndexBuilder
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FieldIndexEntry
import io.github.mcdev.core.mixin.MethodIndexEntry
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock

class LazyClasspathClassIndex(
    private val provider: ClasspathClassBytesProvider,
    private val maxCachedOwners: Int = DEFAULT_MAX_CACHED_OWNERS,
    private val asmParseCounter: AtomicInteger = AtomicInteger(0),
) : ClassIndex {
    private val ownerCache = LruOwnerIndexCache(maxCachedOwners)

    internal fun asmParseCount(): Int = asmParseCounter.get()

    internal fun cachedOwnerCount(): Int = ownerCache.size()

    override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> {
        if (limit <= 0) {
            return emptyList()
        }
        return provider.findInternalNames(prefix, limit)
            .map(::entryFromInternalName)
    }

    override fun findClass(internalName: String): ClassIndexEntry? =
        if (provider.containsInternalName(internalName)) {
            entryFromInternalName(internalName)
        } else {
            null
        }

    override fun findClassByFqn(fqn: String): ClassIndexEntry? =
        provider.findInternalNameByFqn(fqn)?.let(::entryFromInternalName)

    override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> =
        ownerIndex(ownerInternalName)?.methods.orEmpty()

    override fun getFields(ownerInternalName: String): List<FieldIndexEntry> =
        ownerIndex(ownerInternalName)?.fields.orEmpty()

    private fun ownerIndex(ownerInternalName: String): OwnerIndex? =
        ownerCache.getOrPut(ownerInternalName) { loadOwnerIndex(ownerInternalName) }

    private fun loadOwnerIndex(ownerInternalName: String): OwnerIndex? {
        val bytes = provider.getClassBytes(ownerInternalName) ?: return null
        asmParseCounter.incrementAndGet()
        val partial = ClassMemberIndexBuilder.indexClass(bytes, ownerInternalName)
        val adapter = ClassMemberIndexAdapter(partial)
        return OwnerIndex(
            methods = adapter.getMethods(ownerInternalName),
            fields = adapter.getFields(ownerInternalName),
        )
    }

    private data class OwnerIndex(
        val methods: List<MethodIndexEntry>,
        val fields: List<FieldIndexEntry>,
    )

    private class LruOwnerIndexCache(maxSize: Int) {
        private val lock = ReentrantLock()
        private val map = object : LinkedHashMap<String, OwnerIndex?>(maxSize, 0.75f, true) {
            override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, OwnerIndex?>?): Boolean =
                size > maxSize
        }

        fun get(key: String): OwnerIndex? = lock.withLock { map[key] }

        fun getOrPut(key: String, loader: () -> OwnerIndex?): OwnerIndex? = lock.withLock {
            map[key]?.let { return it }
            val loaded = loader()
            if (loaded != null) {
                map[key] = loaded
            }
            loaded
        }

        fun size(): Int = lock.withLock { map.size }
    }

    companion object {
        const val DEFAULT_MAX_CACHED_OWNERS = 256

        internal fun entryFromInternalName(internalName: String): ClassIndexEntry {
            val simpleName = internalName.substringAfterLast('/')
            val packageName = internalName.substringBeforeLast('/', "").replace('/', '.')
            return ClassIndexEntry(
                simpleName = simpleName,
                packageName = packageName,
                internalName = internalName,
            )
        }
    }
}
