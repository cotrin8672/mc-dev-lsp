package io.github.mcdev.core.bytecode

enum class AtTargetKind {
    INVOKE_VIRTUAL,
    INVOKE_STATIC,
    INVOKE_SPECIAL,
    INVOKE_INTERFACE,
    FIELD_GET_INSTANCE,
    FIELD_PUT_INSTANCE,
    FIELD_GET_STATIC,
    FIELD_PUT_STATIC,
    NEW,
    CONSTANT,
    RETURN,
}

data class AtTargetCandidate(
    val owner: String,
    val name: String,
    val descriptor: String,
    val ordinal: Int,
    val kind: AtTargetKind,
    val constantValue: ConstantValue? = null,
)

sealed interface ConstantValue {
    data class StringValue(val value: String) : ConstantValue
    data class IntValue(val value: Int) : ConstantValue
    data class LongValue(val value: Long) : ConstantValue
    data class FloatValue(val value: Float) : ConstantValue
    data class DoubleValue(val value: Double) : ConstantValue
    data class ClassLiteral(val internalName: String) : ConstantValue
    data object NullValue : ConstantValue
}
