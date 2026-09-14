package io.github.mcdev.core.mixinextras

import com.llamalad7.mixinextras.expression.impl.ExpressionParserFacade
import com.llamalad7.mixinextras.expression.impl.ast.expressions.Expression

internal sealed interface OfficialExpressionParseResult {
    data class Success(val expression: Expression) : OfficialExpressionParseResult

    data class SyntaxFailure(
        val message: String,
        val cause: Throwable?,
    ) : OfficialExpressionParseResult
}

internal object OfficialExpressionParser {
    fun parse(source: String): OfficialExpressionParseResult =
        try {
            OfficialExpressionParseResult.Success(ExpressionParserFacade.parse(source))
        } catch (exception: RuntimeException) {
            OfficialExpressionParseResult.SyntaxFailure(
                message = exception.message ?: exception.toString(),
                cause = exception,
            )
        }
}
