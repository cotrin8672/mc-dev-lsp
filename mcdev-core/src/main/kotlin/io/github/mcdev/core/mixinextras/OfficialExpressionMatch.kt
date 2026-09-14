package io.github.mcdev.core.mixinextras

import com.llamalad7.mixinextras.expression.impl.point.ExpressionContext

enum class OfficialExpressionMatchContextType {
    CUSTOM,
    INJECT,
    MODIFY_ARG,
    MODIFY_ARGS,
    MODIFY_CONSTANT,
    MODIFY_EXPRESSION_VALUE,
    MODIFY_RECEIVER,
    MODIFY_RETURN_VALUE,
    MODIFY_VARIABLE,
    REDIRECT,
    SLICE,
    WRAP_OPERATION,
    WRAP_WITH_CONDITION,
    ;

    internal fun toOfficialType(): ExpressionContext.Type =
        when (this) {
            CUSTOM -> ExpressionContext.Type.CUSTOM
            INJECT -> ExpressionContext.Type.INJECT
            MODIFY_ARG -> ExpressionContext.Type.MODIFY_ARG
            MODIFY_ARGS -> ExpressionContext.Type.MODIFY_ARGS
            MODIFY_CONSTANT -> ExpressionContext.Type.MODIFY_CONSTANT
            MODIFY_EXPRESSION_VALUE -> ExpressionContext.Type.MODIFY_EXPRESSION_VALUE
            MODIFY_RECEIVER -> ExpressionContext.Type.MODIFY_RECEIVER
            MODIFY_RETURN_VALUE -> ExpressionContext.Type.MODIFY_RETURN_VALUE
            MODIFY_VARIABLE -> ExpressionContext.Type.MODIFY_VARIABLE
            REDIRECT -> ExpressionContext.Type.REDIRECT
            SLICE -> ExpressionContext.Type.SLICE
            WRAP_OPERATION -> ExpressionContext.Type.WRAP_OPERATION
            WRAP_WITH_CONDITION -> ExpressionContext.Type.WRAP_WITH_CONDITION
        }
}

fun interface CommonSuperClassResolver {
    fun resolve(type1Descriptor: String, type2Descriptor: String): String?
}

fun interface OfficialExpressionCancellationChecker {
    fun checkCancelled()

    companion object {
        val NONE = OfficialExpressionCancellationChecker { }
    }
}

class OfficialExpressionIdentifierPool internal constructor(
    internal val delegate: com.llamalad7.mixinextras.expression.impl.pool.IdentifierPool,
    internal val locals: List<ExpressionLocalDefinition> = emptyList(),
) {
    companion object {
        val EMPTY = OfficialExpressionIdentifierPool(
            com.llamalad7.mixinextras.expression.impl.pool.IdentifierPool(),
        )
    }
}

internal data class ExpressionLocalDefinition(
    val id: String,
    val spec: HandlerParameterSugarSpec.Local,
    val descriptor: String,
)

enum class OfficialExpressionMatchDecorationKey {
    SIMPLE_EXPRESSION_TYPE,
    SIMPLE_OPERATION_ARGS,
    SIMPLE_OPERATION_PARAM_NAMES,
    SIMPLE_OPERATION_RETURN_TYPE,
    IS_STRING_CONCAT_EXPRESSION,
}

sealed class OfficialExpressionTypeConstraint {
    data class Exact(val descriptor: String) : OfficialExpressionTypeConstraint()

    data object IntLike : OfficialExpressionTypeConstraint()
}

sealed class OfficialExpressionMatchDecorationValue {
    data class TypeConstraint(val constraint: OfficialExpressionTypeConstraint) : OfficialExpressionMatchDecorationValue()

    data class TypeConstraints(val constraints: List<OfficialExpressionTypeConstraint>) :
        OfficialExpressionMatchDecorationValue()

    data class ParamNames(val names: List<String>) : OfficialExpressionMatchDecorationValue()

    data class Flag(val value: Boolean) : OfficialExpressionMatchDecorationValue()
}

sealed class OfficialExpressionMatchInstructionMetadata {
    data class MethodInvocation(
        val ownerInternalName: String,
        val name: String,
        val descriptor: String,
        val isInterface: Boolean,
    ) : OfficialExpressionMatchInstructionMetadata()

    data class FieldAccess(
        val ownerInternalName: String,
        val name: String,
        val descriptor: String,
    ) : OfficialExpressionMatchInstructionMetadata()

    data class TypeOperation(
        val typeInternalName: String,
    ) : OfficialExpressionMatchInstructionMetadata()
}

data class OfficialExpressionMatch(
    val expressionIndex: Int,
    val expressionStartOffset: Int,
    val originalInstructionOpcode: Int,
    val originalInstructionIndex: Int,
    val capturedType: OfficialExpressionTypeConstraint,
    val decorations: Map<OfficialExpressionMatchDecorationKey, OfficialExpressionMatchDecorationValue>,
    val instructionMetadata: OfficialExpressionMatchInstructionMetadata?,
)

sealed interface OfficialExpressionMatchResult {
    data class Available(val matches: List<OfficialExpressionMatch>) : OfficialExpressionMatchResult

    data class Unavailable(
        val reason: String,
        val cause: Throwable? = null,
    ) : OfficialExpressionMatchResult
}
