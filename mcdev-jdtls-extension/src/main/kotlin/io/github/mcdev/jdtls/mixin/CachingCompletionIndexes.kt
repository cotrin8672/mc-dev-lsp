package io.github.mcdev.jdtls.mixin

import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FieldIndexEntry
import io.github.mcdev.core.mixin.MethodIndexEntry
import java.util.concurrent.ConcurrentHashMap

data class CandidateCacheDebug(
    val hit: Boolean,
    val buildMs: Long,
)

class CompletionIndexCaches {
    private data class MemberKey(
        val projectSessionVersion: Long,
        val owner: String,
        val kind: String,
    )

    private data class AtTargetKey(
        val projectSessionVersion: Long,
        val owner: String,
        val methodName: String,
        val methodDescriptor: String?,
        val atValue: String,
    )

    private val methods = ConcurrentHashMap<MemberKey, List<MethodIndexEntry>>()
    private val fields = ConcurrentHashMap<MemberKey, List<FieldIndexEntry>>()
    private val atTargets = ConcurrentHashMap<AtTargetKey, List<AtTargetCandidate>>()
    private val lastDebug = ThreadLocal.withInitial { CandidateCacheDebug(hit = false, buildMs = 0) }

    fun classIndex(
        delegate: ClassIndex,
        projectSessionVersion: Long,
    ): ClassIndex = object : ClassIndex {
        override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> =
            delegate.findClasses(prefix, limit)

        override fun findClass(internalName: String): ClassIndexEntry? =
            delegate.findClass(internalName)

        override fun findClassByFqn(fqn: String): ClassIndexEntry? =
            delegate.findClassByFqn(fqn)

        override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> {
            val key = MemberKey(projectSessionVersion, ownerInternalName, "method")
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
            val key = MemberKey(projectSessionVersion, ownerInternalName, "field")
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

    fun bytecodeIndex(
        delegate: BytecodeIndex,
        projectSessionVersion: Long,
    ): BytecodeIndex = object : BytecodeIndex {
        override fun getAtTargetCandidates(
            ownerInternalName: String,
            methodName: String,
            methodDescriptor: String?,
            atValue: String,
        ): List<AtTargetCandidate> {
            val key = AtTargetKey(projectSessionVersion, ownerInternalName, methodName, methodDescriptor, atValue)
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
