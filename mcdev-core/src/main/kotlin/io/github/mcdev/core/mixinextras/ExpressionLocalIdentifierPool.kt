package io.github.mcdev.core.mixinextras

import com.llamalad7.mixinextras.expression.impl.flow.FlowValue
import com.llamalad7.mixinextras.expression.impl.flow.expansion.InsnExpander
import com.llamalad7.mixinextras.expression.impl.pool.IdentifierPool
import org.objectweb.asm.Opcodes
import org.objectweb.asm.Type
import org.objectweb.asm.tree.MethodNode
import org.objectweb.asm.tree.VarInsnNode

/** Bind local discriminators to live frames of this method, never to a global slot guess. */
internal fun OfficialExpressionIdentifierPool.forMethod(
    classBytes: ByteArray,
    method: MethodNode,
    cancellationChecker: OfficialExpressionCancellationChecker = OfficialExpressionCancellationChecker.NONE,
): IdentifierPool {
    if (locals.isEmpty()) return delegate
    val instructions = method.instructions.toArray().filter { it.opcode >= 0 }
    val positions = instructions.withIndex().associate { it.value to it.index }
    val requested = instructions.mapNotNull { insn ->
        if (insn !is VarInsnNode) return@mapNotNull null
        val index = positions.getValue(insn)
        val snapshot = if (insn.opcode in Opcodes.ISTORE..Opcodes.ASTORE) index + 1 else index
        snapshot.takeIf { it < instructions.size }
    }.toSet()
    val result = LocalCaptureExtractor.extract(
        classBytes,
        method.name,
        method.desc,
        requested,
        OfficialExpressionCancellationChecker {
            try {
                cancellationChecker.checkCancelled()
            } catch (throwable: Throwable) {
                throw LocalCaptureExtractor.LocalCaptureCancellation(throwable)
            }
        },
    )
    val snapshots = when (result) {
        is LocalCaptureResult.Success -> result.snapshots.associateBy { it.instructionOccurrenceIndex }
        is LocalCaptureResult.Failure -> throw IllegalStateException("Cannot resolve expression locals: ${result.error}")
    }
    val byId = locals.groupBy { it.id }
    return object : IdentifierPool() {
        override fun memberExists(id: String): Boolean = delegate.memberExists(id)
        override fun typeExists(id: String): Boolean = delegate.typeExists(id)
        override fun matchesType(id: String, type: Type): Boolean = delegate.matchesType(id, type)
        override fun matchesMember(id: String, node: FlowValue): Boolean {
            if (delegate.memberExists(id) && delegate.matchesMember(id, node)) return true
            val definitions = byId[id] ?: return false
            val variable = node.insn as? VarInsnNode ?: return false
            val representative = InsnExpander.getRepresentative(node) ?: variable
            val index = positions[representative] ?: return false
            val snapshotIndex = if (variable.opcode in Opcodes.ISTORE..Opcodes.ASTORE) index + 1 else index
            val candidates = snapshots[snapshotIndex]?.candidates ?: return false
            return definitions.any { definition ->
                val resolution = LocalDiscriminatorResolver.resolve(definition.spec, definition.descriptor, candidates)
                resolution is LocalDiscriminatorResolution.Resolved && resolution.candidate.slotIndex == variable.`var`
            }
        }
    }
}
