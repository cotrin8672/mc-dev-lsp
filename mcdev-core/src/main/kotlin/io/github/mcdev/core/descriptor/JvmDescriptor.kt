package io.github.mcdev.core.descriptor

sealed interface JvmType {
    data object ByteType : JvmType
    data object CharType : JvmType
    data object DoubleType : JvmType
    data object FloatType : JvmType
    data object IntType : JvmType
    data object LongType : JvmType
    data object ShortType : JvmType
    data object BooleanType : JvmType
    data object VoidType : JvmType
    data class ObjectType(val internalName: String) : JvmType
    data class ArrayType(val component: JvmType) : JvmType
}

data class MethodDescriptor(
    val parameters: List<JvmType>,
    val returnType: JvmType,
)

sealed interface DescriptorParseResult<out T> {
    data class Success<T>(val value: T) : DescriptorParseResult<T>
    data class Failure(val error: DescriptorParseError) : DescriptorParseResult<Nothing>
}

data class DescriptorParseError(
    val message: String,
    val offset: Int,
)

sealed interface MemberTarget {
    val owner: String
    val name: String

    data class Method(
        override val owner: String,
        override val name: String,
        val descriptor: MethodDescriptor,
    ) : MemberTarget

    data class Field(
        override val owner: String,
        override val name: String,
        val descriptor: JvmType,
    ) : MemberTarget
}
