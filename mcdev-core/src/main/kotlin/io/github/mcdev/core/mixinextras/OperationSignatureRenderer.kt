package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.descriptor.DescriptorRenderer
import io.github.mcdev.core.descriptor.DescriptorParseResult
import io.github.mcdev.core.descriptor.parseFieldDescriptor

object OperationSignatureRenderer {
    private val operationGenericPrimitiveTypes = mapOf(
        "B" to "Byte",
        "C" to "Character",
        "D" to "Double",
        "F" to "Float",
        "I" to "Integer",
        "J" to "Long",
        "S" to "Short",
        "Z" to "Boolean",
        "V" to "Void",
    )

    fun renderOperationType(returnTypeDescriptor: String): String {
        val generic = operationGenericPrimitiveTypes[returnTypeDescriptor]
            ?: readableType(returnTypeDescriptor)
        return "Operation<$generic>"
    }

    fun readableType(descriptor: String): String = when (descriptor) {
        "V" -> "void"
        else -> when (val parsed = parseFieldDescriptor(descriptor)) {
            is DescriptorParseResult.Success -> DescriptorRenderer.render(parsed.value)
            else -> descriptor
        }
    }

    fun descriptorFromTypeName(typeName: String, classIndex: io.github.mcdev.core.mixin.ClassIndex): String? {
        val trimmed = typeName.trim()
        if (trimmed.isEmpty()) return null
        return when (trimmed) {
            "byte" -> "B"
            "char" -> "C"
            "double" -> "D"
            "float" -> "F"
            "int" -> "I"
            "long" -> "J"
            "short" -> "S"
            "boolean" -> "Z"
            "void" -> "V"
            else -> {
                val fqn = trimmed.removeSuffix("[]")
                val arraySuffix = if (trimmed.endsWith("[]")) "[" else ""
                val entry = classIndex.findClassByFqn(fqn)
                    ?: classIndex.findClasses(fqn.substringAfterLast('.'), limit = 10)
                        .singleOrNull { it.simpleName == fqn.substringAfterLast('.') }
                entry?.let { "$arraySuffix L${it.internalName};" }?.replace(" ", "")
            }
        }
    }
}
