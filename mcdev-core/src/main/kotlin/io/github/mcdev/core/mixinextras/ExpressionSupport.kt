package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionInsertTextFormat
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata
import io.github.mcdev.core.mixin.AnnotationContext
import io.github.mcdev.core.mixin.AnnotationSlot
import io.github.mcdev.core.mixin.MixinAnnotation
import io.github.mcdev.core.mixin.SemanticCompletionContextExtractor

class ExpressionSupport {
    private val expressionAtValues = listOf("MIXINEXTRAS:EXPRESSION")

    private val featureSnippets = listOf(
        FeatureSnippet("modifyexpressionvalue", "ModifyExpressionValue", "ModifyExpressionValue(method = \"${'$'}{1}\", at = @At(\"${'$'}{2}\"))${'$'}0"),
        FeatureSnippet("modifyreturnvalue", "ModifyReturnValue", "ModifyReturnValue(method = \"${'$'}{1}\", at = @At(\"RETURN\"))${'$'}0"),
        FeatureSnippet("modifyreceiver", "ModifyReceiver", "ModifyReceiver(method = \"${'$'}{1}\", at = @At(value = \"INVOKE\", target = \"${'$'}{2}\"))${'$'}0"),
        FeatureSnippet("wrapoperation", "WrapOperation", "WrapOperation(method = \"${'$'}{1}\", at = @At(value = \"INVOKE\", target = \"${'$'}{2}\"))${'$'}0"),
        FeatureSnippet("wrapwithcondition", "WrapWithCondition", "WrapWithCondition(method = \"${'$'}{1}\", at = @At(value = \"INVOKE\", target = \"${'$'}{2}\"))${'$'}0"),
        FeatureSnippet("wrapmethod", "WrapMethod", "WrapMethod(method = \"${'$'}{1}\")${'$'}0"),
        FeatureSnippet("definition", "Definition", "Definition(id = \"${'$'}{1}\")${'$'}0"),
        FeatureSnippet("definitions", "Definitions", "Definitions({ ${'$'}{1} })${'$'}0"),
        FeatureSnippet("expression", "Expression", "Expression(\"${'$'}{1}\")${'$'}0"),
        FeatureSnippet("expressions", "Expressions", "Expressions({ ${'$'}{1} })${'$'}0"),
        FeatureSnippet("share", "Share", "Share(\"${'$'}{1}\")${'$'}0"),
        FeatureSnippet("sharenamespace", "Share namespace", "Share(value = \"${'$'}{1}\", namespace = \"${'$'}{2}\")${'$'}0"),
        FeatureSnippet("local", "Local", "Local${'$'}0"),
        FeatureSnippet("localordinal", "Local ordinal", "Local(ordinal = ${'$'}{1:0})${'$'}0"),
        FeatureSnippet("localindex", "Local index", "Local(index = ${'$'}{1:0})${'$'}0"),
        FeatureSnippet("localname", "Local name", "Local(name = \"${'$'}{1}\")${'$'}0"),
        FeatureSnippet("localargsonly", "Local argsOnly", "Local(argsOnly = ${'$'}{1|true,false|})${'$'}0"),
        FeatureSnippet("localtype", "Local type", "Local(type = ${'$'}{1:void}.class)${'$'}0"),
        FeatureSnippet("cancellable", "Cancellable", "Cancellable${'$'}0"),
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
                    insertTextFormat = McCompletionInsertTextFormat.SNIPPET,
                )
            }

    private data class FeatureSnippet(
        val trigger: String,
        val label: String,
        val insertText: String,
        val detail: String = "MixinExtras feature",
    )
}
