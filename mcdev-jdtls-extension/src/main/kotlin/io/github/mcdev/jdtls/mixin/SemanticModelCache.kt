package io.github.mcdev.jdtls.mixin

import io.github.mcdev.core.mixin.MixinClassModel
import java.util.LinkedHashMap

data class CachedSemanticModelResult(
    val model: MixinClassModel,
    val cacheHit: Boolean,
    val astParseMs: Long,
    val documentVersion: Long,
)

class SemanticModelCache(
    private val maxEntries: Int = 64,
    private val maxAgeMillis: Long = 10 * 60 * 1000,
    private val parser: (source: String, documentUri: String) -> MixinClassModel,
) {
    private data class Key(
        val documentUri: String,
        val documentVersion: Long,
        val sourceHash: Int,
    )

    private data class Entry(
        val model: MixinClassModel,
        val astParseMs: Long,
        val createdAtMillis: Long,
    )

    private val entries = object : LinkedHashMap<Key, Entry>(16, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<Key, Entry>): Boolean =
            size > maxEntries
    }

    @Synchronized
    fun get(source: String, documentUri: String, documentVersion: Long?): CachedSemanticModelResult {
        return get(source, documentUri, documentVersion, parser)
    }

    @Synchronized
    fun get(
        source: String,
        documentUri: String,
        documentVersion: Long?,
        compute: (source: String, documentUri: String) -> MixinClassModel,
    ): CachedSemanticModelResult {
        val resolvedVersion = documentVersion ?: source.hashCode().toLong()
        val key = Key(
            documentUri = documentUri,
            documentVersion = resolvedVersion,
            sourceHash = source.hashCode(),
        )
        val now = System.currentTimeMillis()
        entries[key]?.takeIf { now - it.createdAtMillis <= maxAgeMillis }?.let { entry ->
            return CachedSemanticModelResult(
                model = entry.model,
                cacheHit = true,
                astParseMs = entry.astParseMs,
                documentVersion = resolvedVersion,
            )
        }

        val started = System.nanoTime()
        val model = compute(source, documentUri)
        val elapsedMs = ((System.nanoTime() - started) / 1_000_000).coerceAtLeast(0)
        entries[key] = Entry(
            model = model,
            astParseMs = elapsedMs,
            createdAtMillis = now,
        )
        return CachedSemanticModelResult(
            model = model,
            cacheHit = false,
            astParseMs = elapsedMs,
            documentVersion = resolvedVersion,
        )
    }

    @Synchronized
    fun invalidate(documentUri: String) {
        entries.keys.removeIf { it.documentUri == documentUri }
    }

    @Synchronized
    fun invalidateAll() {
        entries.clear()
    }
}
