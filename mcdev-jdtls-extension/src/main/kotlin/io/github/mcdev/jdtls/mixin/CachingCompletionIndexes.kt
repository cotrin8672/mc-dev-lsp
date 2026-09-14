package io.github.mcdev.jdtls.mixin

import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FieldIndexEntry
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.mixinextras.ExpressionMemberCompletionCache
import java.util.Collections
import java.util.LinkedHashMap

private const val MAX_MEMBER_CACHE_ENTRIES = 256
private const val MAX_AT_TARGET_CACHE_ENTRIES = 256
private const val MAX_EXPRESSION_MEMBER_COMPLETION_CACHE_ENTRIES = 16

private fun <K, V> synchronizedAccessOrderLruMap(maxEntries: Int): MutableMap<K, V> {
    val backing = object : LinkedHashMap<K, V>(maxEntries, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<K, V>?): Boolean =
            size > maxEntries
    }
    return Collections.synchronizedMap(backing)
}

data class CandidateCacheDebug(
    val hit: Boolean,
    val buildMs: Long,
)

class CompletionIndexCaches {
    private class ClassIndexDelegateIdentity(private val delegate: ClassIndex) {
        override fun equals(other: Any?): Boolean =
            other is ClassIndexDelegateIdentity && delegate === other.delegate

        override fun hashCode(): Int = System.identityHashCode(delegate)
    }

    private class BytecodeIndexDelegateIdentity(private val delegate: BytecodeIndex) {
        override fun equals(other: Any?): Boolean =
            other is BytecodeIndexDelegateIdentity && delegate === other.delegate

        override fun hashCode(): Int = System.identityHashCode(delegate)
    }

    private data class MemberKey(
        val delegateIdentity: ClassIndexDelegateIdentity,
        val projectSessionVersion: Long,
        val owner: String,
        val kind: String,
    )

    private data class AtTargetKey(
        val delegateIdentity: BytecodeIndexDelegateIdentity,
        val projectSessionVersion: Long,
        val owner: String,
        val methodName: String,
        val methodDescriptor: String?,
        val atValue: String,
    )

    private data class ExpressionMemberCompletionCacheKey(
        val classIndexDelegateIdentity: ClassIndexDelegateIdentity,
        val bytecodeIndexDelegateIdentity: BytecodeIndexDelegateIdentity,
        val projectSessionVersion: Long,
    )

    private val methods =
        synchronizedAccessOrderLruMap<MemberKey, List<MethodIndexEntry>>(MAX_MEMBER_CACHE_ENTRIES)
    private val fields =
        synchronizedAccessOrderLruMap<MemberKey, List<FieldIndexEntry>>(MAX_MEMBER_CACHE_ENTRIES)
    private val atTargets =
        synchronizedAccessOrderLruMap<AtTargetKey, List<AtTargetCandidate>>(MAX_AT_TARGET_CACHE_ENTRIES)
    private val expressionMemberCompletionCaches =
        synchronizedAccessOrderLruMap<ExpressionMemberCompletionCacheKey, ExpressionMemberCompletionCache>(
            MAX_EXPRESSION_MEMBER_COMPLETION_CACHE_ENTRIES,
        )
    private val lastDebug = ThreadLocal.withInitial { CandidateCacheDebug(hit = false, buildMs = 0) }

    fun classIndex(
        delegate: ClassIndex,
        projectSessionVersion: Long,
    ): ClassIndex {
        val delegateIdentity = ClassIndexDelegateIdentity(delegate)
        return object : ClassIndex {
            override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> =
                delegate.findClasses(prefix, limit)

            override fun findClass(internalName: String): ClassIndexEntry? =
                delegate.findClass(internalName)

            override fun findClassByFqn(fqn: String): ClassIndexEntry? =
                delegate.findClassByFqn(fqn)

            override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> {
                val key = MemberKey(delegateIdentity, projectSessionVersion, ownerInternalName, "method")
                methods[key]?.let {
                    record(hit = true, buildMs = 0)
                    return it
                }
                val started = System.nanoTime()
                val value = delegate.getMethods(ownerInternalName)
                methods[key] = value
                record(hit = false, buildMs = elapsedMs(started))
                return value
            }

            override fun getFields(ownerInternalName: String): List<FieldIndexEntry> {
                val key = MemberKey(delegateIdentity, projectSessionVersion, ownerInternalName, "field")
                fields[key]?.let {
                    record(hit = true, buildMs = 0)
                    return it
                }
                val started = System.nanoTime()
                val value = delegate.getFields(ownerInternalName)
                fields[key] = value
                record(hit = false, buildMs = elapsedMs(started))
                return value
            }
        }
    }

    fun bytecodeIndex(
        delegate: BytecodeIndex,
        projectSessionVersion: Long,
    ): BytecodeIndex {
        val delegateIdentity = BytecodeIndexDelegateIdentity(delegate)
        return object : BytecodeIndex {
        override fun getAtTargetCandidates(
            ownerInternalName: String,
            methodName: String,
            methodDescriptor: String?,
            atValue: String,
        ): List<AtTargetCandidate> {
            val key = AtTargetKey(delegateIdentity, projectSessionVersion, ownerInternalName, methodName, methodDescriptor, atValue)
            atTargets[key]?.let {
                record(hit = true, buildMs = 0)
                return it
            }
            val started = System.nanoTime()
            val value = delegate.getAtTargetCandidates(ownerInternalName, methodName, methodDescriptor, atValue)
            atTargets[key] = value
            record(hit = false, buildMs = elapsedMs(started))
            return value
        }

        override fun getReturnOrdinalCount(
            ownerInternalName: String,
            methodName: String,
            methodDescriptor: String?,
        ): Int = delegate.getReturnOrdinalCount(ownerInternalName, methodName, methodDescriptor)

        override fun getClassBytes(ownerInternalName: String): ByteArray? =
            delegate.getClassBytes(ownerInternalName)

        override fun resolveCommonSuperClass(type1Descriptor: String, type2Descriptor: String): String? =
            delegate.resolveCommonSuperClass(type1Descriptor, type2Descriptor)
        }
    }

    fun expressionMemberCompletionCache(
        classIndexDelegate: ClassIndex,
        bytecodeIndexDelegate: BytecodeIndex,
        projectSessionVersion: Long,
    ): ExpressionMemberCompletionCache {
        val key = ExpressionMemberCompletionCacheKey(
            classIndexDelegateIdentity = ClassIndexDelegateIdentity(classIndexDelegate),
            bytecodeIndexDelegateIdentity = BytecodeIndexDelegateIdentity(bytecodeIndexDelegate),
            projectSessionVersion = projectSessionVersion,
        )
        synchronized(expressionMemberCompletionCaches) {
            expressionMemberCompletionCaches[key]?.let { return it }
            val value = ExpressionMemberCompletionCache()
            expressionMemberCompletionCaches[key] = value
            return value
        }
    }

    fun resetDebug() {
        lastDebug.set(CandidateCacheDebug(hit = false, buildMs = 0))
    }

    fun debug(): CandidateCacheDebug = lastDebug.get()

    private fun record(hit: Boolean, buildMs: Long) {
        val current = lastDebug.get()
        lastDebug.set(
            CandidateCacheDebug(
                hit = current.hit || hit,
                buildMs = current.buildMs + buildMs,
            ),
        )
    }

    private fun elapsedMs(started: Long): Long =
        ((System.nanoTime() - started) / 1_000_000).coerceAtLeast(0)
}
