package io.github.mcdev.core.mixinextras

import com.llamalad7.mixinextras.expression.impl.pool.SimpleMemberDefinition
import io.github.mcdev.core.descriptor.DescriptorParseResult
import io.github.mcdev.core.descriptor.FieldSelector
import io.github.mcdev.core.descriptor.JvmType
import io.github.mcdev.core.descriptor.Pattern
import io.github.mcdev.core.descriptor.parseFieldDescriptor
import org.objectweb.asm.Handle
import org.objectweb.asm.tree.AbstractInsnNode
import org.objectweb.asm.tree.FieldInsnNode

internal class FieldSelectorMemberDefinition(
    private val selector: FieldSelector,
) : SimpleMemberDefinition {
    override fun matches(insn: AbstractInsnNode): Boolean {
        if (insn !is FieldInsnNode) {
            return false
        }
        return matchesField(selector, insn.owner, insn.name, insn.desc)
    }

    override fun matches(handle: Handle): Boolean = false

    private fun matchesField(
        selector: FieldSelector,
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

    private fun matchesDescriptor(pattern: Pattern<JvmType>, descriptor: String): Boolean =
        when (pattern) {
            Pattern.Any -> true
            is Pattern.Exact -> {
                when (val parsed = parseFieldDescriptor(descriptor)) {
                    is DescriptorParseResult.Success -> parsed.value == pattern.value
                    is DescriptorParseResult.Failure -> false
                }
            }
        }
}
