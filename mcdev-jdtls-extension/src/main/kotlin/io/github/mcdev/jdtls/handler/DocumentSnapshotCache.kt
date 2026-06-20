package io.github.mcdev.jdtls.handler

import java.util.LinkedHashMap

data class DocumentSnapshot(
    val documentUri: String,
    val version: Long,
    val text: String,
    val textHash: Int,
)

data class DocumentSnapshotResult(
    val snapshot: DocumentSnapshot,
    val cacheHit: Boolean,
    val snapshotMs: Long,
)

class DocumentSnapshotCache(
    private val maxEntries: Int = 64,
    private val maxAgeMillis: Long = 10 * 60 * 1000,
) {
    private data class Key(
        val documentUri: String,
        val version: Long,
        val textHash: Int,
    )

    private data class Entry(
        val snapshot: DocumentSnapshot,
        val createdAtMillis: Long,
    )

    private val entries = object : LinkedHashMap<Key, Entry>(16, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<Key, Entry>): Boolean =
            size > maxEntries
    }

    @Synchronized
    fun get(documentUri: String, documentVersion: Long?, text: String): DocumentSnapshotResult {
        val started = System.nanoTime()
        val version = documentVersion ?: text.hashCode().toLong()
        val key = Key(documentUri, version, text.hashCode())
        val now = System.currentTimeMillis()
        entries[key]?.takeIf { now - it.createdAtMillis <= maxAgeMillis }?.let { entry ->
            return DocumentSnapshotResult(entry.snapshot, cacheHit = true, snapshotMs = elapsedMs(started))
        }
        val snapshot = DocumentSnapshot(
            documentUri = documentUri,
            version = version,
            text = text,
            textHash = text.hashCode(),
        )
        entries[key] = Entry(snapshot, now)
        return DocumentSnapshotResult(snapshot, cacheHit = false, snapshotMs = elapsedMs(started))
    }

    @Synchronized
    fun invalidate(documentUri: String) {
        entries.keys.removeIf { it.documentUri == documentUri }
    }

    @Synchronized
    fun invalidateAll() {
        entries.clear()
    }

    private fun elapsedMs(started: Long): Long =
        ((System.nanoTime() - started) / 1_000_000).coerceAtLeast(0)
}
