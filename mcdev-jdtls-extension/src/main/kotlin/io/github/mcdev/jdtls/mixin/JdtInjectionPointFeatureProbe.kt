package io.github.mcdev.jdtls.mixin

import java.util.Collections
import java.util.LinkedHashMap
import java.util.concurrent.CompletableFuture
import java.util.concurrent.ConcurrentHashMap

private const val MAX_CACHE_ENTRIES = 32

private const val INJECTION_POINT_SPECIFIER_FQN =
    "org.spongepowered.asm.mixin.injection.InjectionPoint\$Specifier"

private fun <K, V> synchronizedAccessOrderLruMap(maxEntries: Int): MutableMap<K, V> {
    val backing = object : LinkedHashMap<K, V>(maxEntries, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<K, V>?): Boolean =
            size > maxEntries
    }
    return Collections.synchronizedMap(backing)
}

internal fun interface JavaProjectTypeLookup {
    fun findType(javaProject: Any, fqn: String): Any?
}

internal object ReflectiveJavaProjectTypeLookup : JavaProjectTypeLookup {
    override fun findType(javaProject: Any, fqn: String): Any? =
        javaProject.javaClass.getMethod("findType", String::class.java).invoke(javaProject, fqn)
}

internal class JdtInjectionPointFeatureProbe(
    private val typeLookup: JavaProjectTypeLookup = ReflectiveJavaProjectTypeLookup,
) {
    private class JavaProjectIdentity(private val javaProject: Any) {
        override fun equals(other: Any?): Boolean =
            other is JavaProjectIdentity && javaProject === other.javaProject

        override fun hashCode(): Int = System.identityHashCode(javaProject)
    }

    private data class CacheKey(
        val javaProjectIdentity: JavaProjectIdentity,
        val projectSessionVersion: Long,
    )

    private val cache = synchronizedAccessOrderLruMap<CacheKey, Boolean>(MAX_CACHE_ENTRIES)
    private val inFlight = ConcurrentHashMap<CacheKey, CompletableFuture<Boolean>>()

    fun injectionPointSpecifierSupported(
        javaProject: Any?,
        projectSessionVersion: Long,
    ): Boolean {
        if (javaProject == null) {
            return false
        }
        val key = CacheKey(JavaProjectIdentity(javaProject), projectSessionVersion)
        return resolveSupported(key, javaProject)
    }

    private fun resolveSupported(key: CacheKey, javaProject: Any): Boolean {
        synchronized(cache) {
            cache[key]?.let { return it }
        }

        val future = CompletableFuture<Boolean>()
        val existing = inFlight.putIfAbsent(key, future)
        if (existing != null) {
            val result = existing.join()
            synchronized(cache) {
                cache[key]?.let { return it }
            }
            return result
        }

        try {
            synchronized(cache) {
                cache[key]?.let { cached ->
                    future.complete(cached)
                    return cached
                }
            }

            val outcome = runCatching {
                typeLookup.findType(javaProject, INJECTION_POINT_SPECIFIER_FQN) != null
            }
            return if (outcome.isSuccess) {
                val supported = outcome.getOrThrow()
                synchronized(cache) {
                    cache[key] = supported
                }
                future.complete(supported)
                supported
            } else {
                future.complete(false)
                false
            }
        } finally {
            inFlight.remove(key)
        }
    }
}
