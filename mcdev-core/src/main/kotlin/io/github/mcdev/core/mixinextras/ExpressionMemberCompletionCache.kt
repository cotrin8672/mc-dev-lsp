package io.github.mcdev.core.mixinextras

import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock

class ExpressionMemberCompletionCache(
    capacity: Int = DEFAULT_CAPACITY,
) {
    private val lock = ReentrantLock()
    private val map = object : LinkedHashMap<CacheKey, List<OfficialExpressionMemberCandidate>>(capacity, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<CacheKey, List<OfficialExpressionMemberCandidate>>?): Boolean =
            size > capacity
    }

    internal fun get(key: CacheKey): List<OfficialExpressionMemberCandidate>? =
        lock.withLock { map[key] }

    internal fun put(key: CacheKey, candidates: List<OfficialExpressionMemberCandidate>) {
        lock.withLock {
            map[key] = candidates
        }
    }

    internal data class CacheKey(
        val receiverExpression: String,
        val ownerInternalName: String,
        val methodName: String,
        val methodDescriptor: String,
        val contextType: OfficialExpressionMatchContextType,
        val classBytesFingerprint: String,
        val definitionIndexFingerprint: String,
    )

    companion object {
        const val DEFAULT_CAPACITY = 32
    }
}
