package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.descriptor.DescriptorParseResult
import io.github.mcdev.core.descriptor.parseFieldDescriptor
import org.objectweb.asm.ClassReader
import org.objectweb.asm.Type
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock

class BytecodeCommonSuperClassResolver(
    private val classBytesLookup: (internalName: String) -> ByteArray?,
    maxCachedHierarchies: Int = DEFAULT_MAX_CACHED_HIERARCHIES,
    maxCachedPairs: Int = DEFAULT_MAX_CACHED_PAIRS,
) : CommonSuperClassResolver {
    init {
        require(maxCachedHierarchies > 0) { "maxCachedHierarchies must be positive" }
        require(maxCachedPairs > 0) { "maxCachedPairs must be positive" }
    }

    private val hierarchyCache = BoundedLruCache<String, ClassHierarchyInfo?>(maxCachedHierarchies)
    private val pairCache = BoundedLruCache<PairCacheKey, String?>(maxCachedPairs)

    override fun resolve(type1Descriptor: String, type2Descriptor: String): String? {
        val cacheKey = PairCacheKey.canonical(type1Descriptor, type2Descriptor)
        return pairCache.getOrCompute(cacheKey) {
            resolveUncached(type1Descriptor, type2Descriptor)
        }
    }

    private fun resolveUncached(type1Descriptor: String, type2Descriptor: String): String? {
        val type1 = parseType(type1Descriptor) ?: return null
        val type2 = parseType(type2Descriptor) ?: return null

        if (type1Descriptor == type2Descriptor) {
            return type1Descriptor
        }

        if (type1.sort <= Type.DOUBLE && type2.sort <= Type.DOUBLE) {
            return null
        }
        if (type1.sort <= Type.DOUBLE || type2.sort <= Type.DOUBLE) {
            return null
        }

        if (isAssignableFrom(type1Descriptor, type2Descriptor)) {
            return type2Descriptor
        }
        if (isAssignableFrom(type2Descriptor, type1Descriptor)) {
            return type1Descriptor
        }

        if (type1.sort == Type.ARRAY || type2.sort == Type.ARRAY) {
            return resolveArrayTypes(type1, type2)
        }

        return resolveReferenceTypes(type1.internalName, type2.internalName)
    }

    private fun resolveArrayTypes(type1: Type, type2: Type): String? {
        if (type1.sort != Type.ARRAY && type2.sort != Type.ARRAY) {
            return null
        }
        if (type1.sort != Type.ARRAY) {
            return resolveArrayWithNonArray(type2, type1)
        }
        if (type2.sort != Type.ARRAY) {
            return resolveArrayWithNonArray(type1, type2)
        }

        val element1 = Type.getType(type1.descriptor.substring(1))
        val element2 = Type.getType(type2.descriptor.substring(1))

        if (element1.sort <= Type.DOUBLE || element2.sort <= Type.DOUBLE) {
            if (element1 != element2) {
                return OBJECT_DESCRIPTOR
            }
            return type1.descriptor
        }

        val commonElement = resolve(element1.descriptor, element2.descriptor) ?: return null
        return "[" + commonElement
    }

    private fun resolveArrayWithNonArray(arrayType: Type, otherType: Type): String? {
        if (otherType.sort == Type.OBJECT && isArraySuperReference(otherType.internalName)) {
            return otherType.descriptor
        }
        return OBJECT_DESCRIPTOR
    }

    private fun resolveReferenceTypes(internalName1: String, internalName2: String): String? {
        val ordered1 = collectAncestorsOrdered(internalName1) ?: return null
        val ancestors2 = collectAncestorsSet(internalName2) ?: return null

        for (ancestor in ordered1) {
            if (ancestor in ancestors2) {
                return Type.getObjectType(ancestor).descriptor
            }
        }
        return null
    }

    private fun isAssignableFrom(subDescriptor: String, superDescriptor: String): Boolean {
        if (subDescriptor == superDescriptor) {
            return true
        }

        val subType = parseType(subDescriptor) ?: return false
        val superType = parseType(superDescriptor) ?: return false

        if (superType.sort == Type.OBJECT && superType.internalName == "java/lang/Object") {
            return true
        }
        if (subType.sort == Type.ARRAY) {
            return isArrayAssignableFrom(subType, superType)
        }
        if (superType.sort == Type.ARRAY) {
            return false
        }

        return isReferenceAssignableFrom(subType.internalName, superType.internalName)
    }

    private fun isArrayAssignableFrom(subType: Type, superType: Type): Boolean {
        if (superType.sort == Type.OBJECT && isArraySuperReference(superType.internalName)) {
            return true
        }
        if (superType.sort != Type.ARRAY || subType.sort != Type.ARRAY) {
            return false
        }

        val subComponent = Type.getType(subType.descriptor.substring(1))
        val superComponent = Type.getType(superType.descriptor.substring(1))
        if (subComponent.sort <= Type.DOUBLE || superComponent.sort <= Type.DOUBLE) {
            return subComponent == superComponent
        }
        return isAssignableFrom(subComponent.descriptor, superComponent.descriptor)
    }

    private fun isReferenceAssignableFrom(subInternalName: String, superInternalName: String): Boolean =
        isReferenceAssignableFrom(subInternalName, superInternalName, mutableSetOf())

    private fun isReferenceAssignableFrom(
        subInternalName: String,
        superInternalName: String,
        visiting: MutableSet<String>,
    ): Boolean {
        if (subInternalName == superInternalName) {
            return true
        }
        if (superInternalName == "java/lang/Object") {
            return true
        }
        if (!visiting.add(subInternalName)) {
            return false
        }

        val info = loadHierarchyInfo(subInternalName) ?: return false
        info.superInternalName?.let { superName ->
            if (superName == superInternalName || isReferenceAssignableFrom(superName, superInternalName, visiting)) {
                return true
            }
        }
        for (interfaceName in info.interfaceInternalNames) {
            if (
                interfaceName == superInternalName ||
                isReferenceAssignableFrom(interfaceName, superInternalName, visiting)
            ) {
                return true
            }
        }
        return false
    }

    private fun collectAncestorsOrdered(internalName: String): List<String>? {
        val ordered = mutableListOf<String>()
        val visited = mutableSetOf<String>()

        fun visit(current: String) {
            if (!visited.add(current)) {
                return
            }
            ordered += current
            if (current == "java/lang/Object") {
                return
            }

            val info = loadHierarchyInfo(current) ?: return
            info.superInternalName?.let(::visit) ?: visit("java/lang/Object")
            info.interfaceInternalNames.forEach(::visit)
        }

        visit(internalName)
        if (ordered.singleOrNull() == internalName && loadHierarchyInfo(internalName) == null) {
            return null
        }
        return ordered
    }

    private fun collectAncestorsSet(internalName: String): Set<String>? =
        collectAncestorsOrdered(internalName)?.toSet()

    private fun loadHierarchyInfo(internalName: String): ClassHierarchyInfo? =
        hierarchyCache.getOrCompute(internalName) {
            val bytes = classBytesLookup(internalName)
            if (bytes == null) {
                null
            } else {
                runCatching {
                    val reader = ClassReader(bytes)
                    ClassHierarchyInfo(
                        superInternalName = reader.superName?.takeUnless { it == "java/lang/Object" },
                        interfaceInternalNames = reader.interfaces?.toList() ?: emptyList(),
                    )
                }.getOrNull()
            }
        }

    private fun parseType(descriptor: String): Type? {
        if (parseFieldDescriptor(descriptor) !is DescriptorParseResult.Success) {
            return null
        }
        return runCatching { Type.getType(descriptor) }
            .getOrNull()
            ?.takeUnless { it.sort == Type.METHOD || it.sort == Type.VOID }
    }

    private fun isArraySuperReference(internalName: String): Boolean =
        internalName == "java/lang/Object" ||
            internalName == "java/lang/Cloneable" ||
            internalName == "java/io/Serializable"

    private data class ClassHierarchyInfo(
        val superInternalName: String?,
        val interfaceInternalNames: List<String>,
    )

    private data class PairCacheKey(
        val first: String,
        val second: String,
    ) {
        companion object {
            fun canonical(left: String, right: String): PairCacheKey =
                if (left <= right) {
                    PairCacheKey(left, right)
                } else {
                    PairCacheKey(right, left)
                }
        }
    }

    private class BoundedLruCache<K, V>(private val maxSize: Int) {
        private val lock = ReentrantLock()
        private val map = object : LinkedHashMap<K, Any?>(maxSize, 0.75f, true) {
            override fun removeEldestEntry(eldest: MutableMap.MutableEntry<K, Any?>?): Boolean =
                size > maxSize
        }

        fun getOrCompute(key: K, compute: () -> V): V {
            lock.withLock {
                if (key in map) {
                    return unwrap(map[key])
                }
            }

            val value = compute()

            lock.withLock {
                if (key in map) {
                    return unwrap(map[key])
                }
                map[key] = wrap(value)
                return value
            }
        }

        private fun wrap(value: V): Any? = if (value == null) NULL_SENTINEL else value

        @Suppress("UNCHECKED_CAST")
        private fun unwrap(stored: Any?): V =
            if (stored === NULL_SENTINEL) null as V else stored as V

        private companion object {
            private val NULL_SENTINEL = Any()
        }
    }

    private companion object {
        const val DEFAULT_MAX_CACHED_HIERARCHIES = 256
        const val DEFAULT_MAX_CACHED_PAIRS = 256
        const val OBJECT_DESCRIPTOR = "Ljava/lang/Object;"
    }
}
