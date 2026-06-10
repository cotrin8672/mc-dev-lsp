package io.github.mcdev.core.descriptor

class DescriptorParser(private val input: String) {
    private var index = 0

    fun parseFieldDescriptor(): DescriptorParseResult<JvmType> {
        val type = parseType(allowVoid = false) ?: return failure("expected field descriptor")
        if (!isAtEnd()) return failure("unexpected trailing input")
        return DescriptorParseResult.Success(type)
    }

    fun parseMethodDescriptor(): DescriptorParseResult<MethodDescriptor> {
        if (!consume('(')) return failure("expected '('")
        val parameters = mutableListOf<JvmType>()
        while (!peek(')')) {
            if (isAtEnd()) return failure("unterminated parameter list")
            parameters += parseType(allowVoid = false) ?: return failure("expected parameter descriptor")
        }
        consume(')')
        val returnType = parseType(allowVoid = true) ?: return failure("expected return descriptor")
        if (!isAtEnd()) return failure("unexpected trailing input")
        return DescriptorParseResult.Success(MethodDescriptor(parameters, returnType))
    }

    private fun parseType(allowVoid: Boolean): JvmType? {
        if (isAtEnd()) return null
        return when (val ch = input[index++]) {
            'B' -> JvmType.ByteType
            'C' -> JvmType.CharType
            'D' -> JvmType.DoubleType
            'F' -> JvmType.FloatType
            'I' -> JvmType.IntType
            'J' -> JvmType.LongType
            'S' -> JvmType.ShortType
            'Z' -> JvmType.BooleanType
            'V' -> if (allowVoid) JvmType.VoidType else null
            '[' -> parseType(allowVoid = false)?.let(JvmType::ArrayType)
            'L' -> parseObjectType()
            else -> {
                index--
                null
            }
        }
    }

    private fun parseObjectType(): JvmType.ObjectType? {
        val start = index
        while (!isAtEnd() && input[index] != ';') index++
        if (isAtEnd()) {
            index = start - 1
            return null
        }
        val internalName = input.substring(start, index)
        index++
        if (internalName.isBlank() || internalName.contains('.')) {
            index = start - 1
            return null
        }
        return JvmType.ObjectType(internalName)
    }

    private fun consume(ch: Char): Boolean {
        if (!peek(ch)) return false
        index++
        return true
    }

    private fun peek(ch: Char): Boolean = !isAtEnd() && input[index] == ch

    private fun isAtEnd(): Boolean = index >= input.length

    private fun failure(message: String): DescriptorParseResult.Failure =
        DescriptorParseResult.Failure(DescriptorParseError(message, index.coerceAtMost(input.length)))
}

fun parseFieldDescriptor(input: String): DescriptorParseResult<JvmType> =
    DescriptorParser(input).parseFieldDescriptor()

fun parseMethodDescriptor(input: String): DescriptorParseResult<MethodDescriptor> =
    DescriptorParser(input).parseMethodDescriptor()
