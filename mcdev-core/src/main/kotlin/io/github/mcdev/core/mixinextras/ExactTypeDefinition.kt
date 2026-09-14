package io.github.mcdev.core.mixinextras

import com.llamalad7.mixinextras.expression.impl.pool.TypeDefinition
import org.objectweb.asm.Type

internal class ExactTypeDefinition(
    private val type: Type,
) : TypeDefinition {
    override fun matches(type: Type): Boolean = this.type == type
}
