package io.github.mcdev.core.mixin

import io.github.mcdev.core.bytecode.ConstantValue

object ConstantAtHintFormatter {
    fun hintFor(candidate: AtTargetCandidate): String? {
        if (candidate.kind != AtTargetKind.CONSTANT) return null
        return when (val value = candidate.constantValue) {
            is ConstantValue.StringValue -> """@Constant(stringValue = "${escapeString(value.value)}")"""
            is ConstantValue.IntValue -> "@Constant(intValue = ${value.value})"
            is ConstantValue.LongValue -> "@Constant(longValue = ${value.value}L)"
            is ConstantValue.FloatValue -> "@Constant(floatValue = ${value.value}f)"
            is ConstantValue.DoubleValue -> "@Constant(doubleValue = ${value.value})"
            is ConstantValue.ClassLiteral ->
                """@Constant(classValue = "${value.internalName.replace('/', '.')}.class")"""
            ConstantValue.NullValue -> "@Constant(nullValue = true)"
            null -> null
        }
    }

    private fun escapeString(value: String): String =
        value
            .replace("\\", "\\\\")
            .replace("\"", "\\\"")
            .replace("\n", "\\n")
            .replace("\r", "\\r")
            .replace("\t", "\\t")
}
