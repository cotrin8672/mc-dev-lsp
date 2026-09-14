package io.github.mcdev.core.mixinextras

import com.llamalad7.mixinextras.expression.impl.pool.SimpleMemberDefinition
import org.objectweb.asm.Handle
import org.objectweb.asm.tree.AbstractInsnNode
import org.objectweb.asm.tree.VarInsnNode

internal class LocalIndexMemberDefinition(
    private val index: Int,
) : SimpleMemberDefinition {
    override fun matches(insn: AbstractInsnNode): Boolean =
        insn is VarInsnNode && insn.`var` == index

    override fun matches(handle: Handle): Boolean = false
}
