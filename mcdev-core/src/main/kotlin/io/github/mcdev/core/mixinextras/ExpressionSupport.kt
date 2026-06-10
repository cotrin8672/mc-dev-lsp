package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata
import io.github.mcdev.core.mixin.AnnotationContext
import io.github.mcdev.core.mixin.AnnotationSlot
import io.github.mcdev.core.mixin.MixinAnnotation

class ExpressionSupport {
    private val expressionAtValues = listOf("MIXINEXTRAS:EXPRESSION")

    private val expressionSnippets = listOf(
        ExpressionSnippet("def", "Definition variable", "@Definition(id = \"\")"),
        ExpressionSnippet("expr", "Expression slice", "@Expression(\"\")"),
        ExpressionSnippet("share", "Shared local", "@Share(\"\")"),
        ExpressionSnippet("local", "Local capture", "@Local"),
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
        return expressionSnippets
            .filter { it.trigger.startsWith(partial, ignoreCase = true) || partial.isEmpty() }
            .map { snippet ->
                McCompletionItem(
                    label = snippet.label,
                    detail = snippet.description,
                    documentation = snippet.insertText,
                    filterText = "${snippet.trigger} ${snippet.label}",
                    insertText = snippet.insertText,
                    kind = McCompletionKind.KEYWORD,
                    sortKey = "0311_${snippet.trigger}",
                    metadata = McCompletionMetadata(source = "mixinextras.expression", name = snippet.trigger),
                )
            }
    }

    fun isExpressionAtValue(atValue: String?): Boolean =
        atValue?.equals("MIXINEXTRAS:EXPRESSION", ignoreCase = true) == true

    private data class ExpressionSnippet(
        val trigger: String,
        val label: String,
        val insertText: String,
        val description: String = label,
    )
}
