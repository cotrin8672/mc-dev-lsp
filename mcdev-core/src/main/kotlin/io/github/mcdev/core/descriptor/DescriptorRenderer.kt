package io.github.mcdev.core.descriptor

object DescriptorRenderer {
    fun render(type: JvmType): String = when (type) {
        JvmType.ByteType -> "byte"
        JvmType.CharType -> "char"
        JvmType.DoubleType -> "double"
        JvmType.FloatType -> "float"
        JvmType.IntType -> "int"
        JvmType.LongType -> "long"
        JvmType.ShortType -> "short"
        JvmType.BooleanType -> "boolean"
        JvmType.VoidType -> "void"
        is JvmType.ObjectType -> type.internalName.substringAfterLast('/')
        is JvmType.ArrayType -> "${render(type.component)}[]"
    }

    fun renderMethod(descriptor: MethodDescriptor): String =
        descriptor.parameters.joinToString(prefix = "(", postfix = ")") { render(it) } +
            ": ${render(descriptor.returnType)}"

    fun toDescriptor(type: JvmType): String = when (type) {
        JvmType.ByteType -> "B"
        JvmType.CharType -> "C"
        JvmType.DoubleType -> "D"
        JvmType.FloatType -> "F"
        JvmType.IntType -> "I"
        JvmType.LongType -> "J"
        JvmType.ShortType -> "S"
        JvmType.BooleanType -> "Z"
        JvmType.VoidType -> "V"
        is JvmType.ObjectType -> "L${type.internalName};"
        is JvmType.ArrayType -> "[${toDescriptor(type.component)}"
    }

    fun toDescriptor(descriptor: MethodDescriptor): String =
        descriptor.parameters.joinToString(separator = "", prefix = "(", postfix = ")") { toDescriptor(it) } +
            toDescriptor(descriptor.returnType)
}
