package io.github.mcdev.core.mixin

import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata

class AtValueCompletionService {
    private val values = listOf(
        "HEAD",
        "RETURN",
        "TAIL",
        "INVOKE",
        "INVOKE_ASSIGN",
        "FIELD",
        "NEW",
        "CONSTANT",
        "JUMP",
        "LOAD",
        "STORE",
        "MIXINEXTRAS:EXPRESSION",
    )

    fun complete(context: AnnotationContext): List<McCompletionItem> {
        if (context.annotation != MixinAnnotation.AT || context.slot != AnnotationSlot.VALUE) return emptyList()
        val partial = context.partialValue.trim('"')
        return values
            .filter { it.startsWith(partial, ignoreCase = true) }
            .map { value ->
                McCompletionItem(
                    label = value,
                    detail = "@At injection point",
                    documentation = null,
                    filterText = value,
                    insertText = value,
                    kind = McCompletionKind.VALUE,
                    sortKey = "0300_$value",
                    metadata = McCompletionMetadata(source = "mixin.atValue", name = value),
                )
            }
    }
}
