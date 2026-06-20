package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata
import io.github.mcdev.core.mixin.AnnotationContext
import io.github.mcdev.core.mixin.AnnotationSlot
import io.github.mcdev.core.mixin.MixinAnnotation
import io.github.mcdev.core.mixin.SemanticCompletionContextExtractor

class ExpressionSupport {
    private val expressionAtValues = listOf("MIXINEXTRAS:EXPRESSION")

    private val featureSnippets = listOf(
        FeatureSnippet("modifyexpressionvalue", "ModifyExpressionValue", "@ModifyExpressionValue(method = \"\", at = @At(\"\"))"),
        FeatureSnippet("modifyreturnvalue", "ModifyReturnValue", "@ModifyReturnValue(method = \"\", at = @At(\"RETURN\"))"),
        FeatureSnippet("modifyreceiver", "ModifyReceiver", "@ModifyReceiver(method = \"\", at = @At(value = \"INVOKE\", target = \"\"))"),
        FeatureSnippet("wrapoperation", "WrapOperation", "@WrapOperation(method = \"\", at = @At(value = \"INVOKE\", target = \"\"))"),
        FeatureSnippet("wrapwithcondition", "WrapWithCondition", "@WrapWithCondition(method = \"\", at = @At(value = \"INVOKE\", target = \"\"))"),
        FeatureSnippet("wrapmethod", "WrapMethod", "@WrapMethod(method = \"\")"),
        FeatureSnippet("definition", "Definition", "@Definition(id = \"\")"),
        FeatureSnippet("definitions", "Definitions", "@Definitions({})"),
        FeatureSnippet("expression", "Expression", "@Expression(\"\")"),
        FeatureSnippet("expressions", "Expressions", "@Expressions({})"),
        FeatureSnippet("share", "Share", "@Share(\"\")"),
        FeatureSnippet("sharenamespace", "Share namespace", "@Share(value = \"\", namespace = \"\")"),
        FeatureSnippet("local", "Local", "@Local"),
        FeatureSnippet("localordinal", "Local ordinal", "@Local(ordinal = 0)"),
        FeatureSnippet("localindex", "Local index", "@Local(index = 0)"),
        FeatureSnippet("localname", "Local name", "@Local(name = \"\")"),
        FeatureSnippet("localargsonly", "Local argsOnly", "@Local(argsOnly = true)"),
        FeatureSnippet("localtype", "Local type", "@Local(type = void.class)"),
        FeatureSnippet("cancellable", "Cancellable", "@Cancellable"),
    )

    fun completeAtValue(context: AnnotationContext): List<McCompletionItem> {
        if (context.annotation != MixinAnnotation.AT || context.slot != AnnotationSlot.VALUE) return emptyList()
        val partial = context.partialValue.trim('"')
        return expressionAtValues
            .filter { it.startsWith(partial, ignoreCase = true) }
            .map { value ->
                McCompletionItem(
                    label = value,
                    detail = "MixinExtras expression injection point",
                    documentation = "Use @Definition and @Expression annotations to define expression slices",
                    filterText = value,
                    insertText = value,
                    kind = McCompletionKind.VALUE,
                    sortKey = "0310_$value",
                    metadata = McCompletionMetadata(source = "mixinextras.expressionAtValue", name = value),
                )
            }
    }

    fun completeExpressionAnnotations(context: AnnotationContext): List<McCompletionItem> {
        val partial = context.partialValue.trim()
        return completeFeatureSnippets(partial)
    }

    fun completeFeatureAnnotations(source: String, line: Int, character: Int): List<McCompletionItem> {
        val offset = SemanticCompletionContextExtractor.toOffset(source, line, character) ?: return emptyList()
        val lineStart = source.lastIndexOf('\n', (offset - 1).coerceAtLeast(0)).let { if (it < 0) 0 else it + 1 }
        val beforeCursor = source.substring(lineStart, offset.coerceIn(0, source.length))
        val marker = beforeCursor.lastIndexOf('@')
        if (marker < 0) return emptyList()
        val partial = beforeCursor.substring(marker + 1)
        if (partial.any { it.isWhitespace() || it == '(' || it == ')' || it == '.' }) return emptyList()
        return completeFeatureSnippets(partial)
    }

    fun isExpressionAtValue(atValue: String?): Boolean =
        atValue?.equals("MIXINEXTRAS:EXPRESSION", ignoreCase = true) == true

    private fun completeFeatureSnippets(partial: String): List<McCompletionItem> =
        featureSnippets
            .filter { snippet ->
                partial.isEmpty() ||
                    snippet.trigger.startsWith(partial, ignoreCase = true) ||
                    snippet.label.startsWith(partial, ignoreCase = true)
            }
            .map { snippet ->
                McCompletionItem(
                    label = snippet.label,
                    detail = snippet.detail,
                    documentation = snippet.insertText,
                    filterText = "${snippet.trigger} ${snippet.label}",
                    insertText = snippet.insertText,
                    kind = McCompletionKind.KEYWORD,
                    sortKey = "0311_${snippet.trigger}",
                    metadata = McCompletionMetadata(source = "mixinextras.feature", name = snippet.trigger),
                )
            }

    private data class FeatureSnippet(
        val trigger: String,
        val label: String,
        val insertText: String,
        val detail: String = "MixinExtras feature",
    )
}
