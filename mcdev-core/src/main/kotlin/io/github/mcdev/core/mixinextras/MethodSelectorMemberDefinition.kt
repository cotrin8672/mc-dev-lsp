package io.github.mcdev.core.mixinextras

import com.llamalad7.mixinextras.expression.impl.pool.SimpleMemberDefinition
import io.github.mcdev.core.descriptor.DescriptorParseResult
import io.github.mcdev.core.descriptor.MethodDescriptor
import io.github.mcdev.core.descriptor.MethodSelector
import io.github.mcdev.core.descriptor.Pattern
import io.github.mcdev.core.descriptor.parseMethodDescriptor
import org.objectweb.asm.Handle
import org.objectweb.asm.Opcodes
import org.objectweb.asm.tree.AbstractInsnNode
import org.objectweb.asm.tree.MethodInsnNode

internal class MethodSelectorMemberDefinition(
    private val selector: MethodSelector,
) : SimpleMemberDefinition {
    override fun matches(insn: AbstractInsnNode): Boolean {
        if (insn !is MethodInsnNode) {
            return false
        }
        return matchesMethod(selector, insn.owner, insn.name, insn.desc)
    }

    override fun matches(handle: Handle): Boolean =
        when (handle.tag) {
            Opcodes.H_INVOKEVIRTUAL,
            Opcodes.H_INVOKESTATIC,
            Opcodes.H_INVOKESPECIAL,
            Opcodes.H_INVOKEINTERFACE,
            -> matchesMethod(selector, handle.owner, handle.name, handle.desc)
            else -> false
        }

    private fun matchesMethod(
        selector: MethodSelector,
        owner: String,
        name: String,
        descriptor: String,
    ): Boolean =
        matchesOwner(selector.owner, owner) &&
            matchesName(selector.name, name) &&
            matchesDescriptor(selector.descriptor, descriptor)

    private fun matchesOwner(pattern: Pattern<String>, owner: String): Boolean =
        when (pattern) {
            Pattern.Any -> true
            is Pattern.Exact -> pattern.value == owner
        }

    private fun matchesName(pattern: Pattern<String>, name: String): Boolean =
        when (pattern) {
            Pattern.Any -> true
            is Pattern.Exact -> pattern.value == name
        }

    private fun matchesDescriptor(pattern: Pattern<MethodDescriptor>, descriptor: String): Boolean =
        when (pattern) {
            Pattern.Any -> true
            is Pattern.Exact -> {
                when (val parsed = parseMethodDescriptor(descriptor)) {
                    is DescriptorParseResult.Success -> parsed.value == pattern.value
                    is DescriptorParseResult.Failure -> false
                }
            }
        }
}
