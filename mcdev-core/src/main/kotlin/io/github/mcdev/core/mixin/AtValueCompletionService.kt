package io.github.mcdev.core.mixin

import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata

class AtValueCompletionService(
    private val injectionPointSpecifierSupported: () -> Boolean = { false },
    private val additionalAtValues: () -> List<String> = { emptyList() },
) {
    private val values = listOf(
        "HEAD",
        "CTOR_HEAD",
        "RETURN",
        "TAIL",
        "INVOKE",
        "INVOKE_STRING",
        "INVOKE_ASSIGN",
        "FIELD",
        "NEW",
        "CONSTANT",
        "LOAD",
        "STORE",
        "MIXINEXTRAS:EXPRESSION",
    )

    private val specifiers = listOf("FIRST", "LAST", "ONE", "ALL", "DEFAULT")

    fun complete(context: AnnotationContext): List<McCompletionItem> {
        if (context.annotation != MixinAnnotation.AT || context.slot != AnnotationSlot.VALUE) return emptyList()
        val partial = context.partialValue.trim('"')
        val candidates = candidateValues(context)
        completeSpecifiers(partial, candidates, context)?.let { return it }
        return candidates
            .filter { it.startsWith(partial, ignoreCase = true) }
            .map { toCompletionItem(it) }
    }

    private fun candidateValues(context: AnnotationContext): List<String> =
        mergeCandidates(baseCandidateValues(context), additionalAtValues())

    private fun baseCandidateValues(context: AnnotationContext): List<String> =
        if (context.atInsideSlice) {
            values
        } else {
            when (context.parentInjectorAnnotation) {
                MixinAnnotation.MODIFY_RETURN_VALUE -> listOf("RETURN", "TAIL", "MIXINEXTRAS:EXPRESSION")
                MixinAnnotation.MODIFY_EXPRESSION_VALUE -> listOf("INVOKE", "FIELD", "NEW", "CONSTANT", "MIXINEXTRAS:EXPRESSION")
                MixinAnnotation.MODIFY_RECEIVER,
                MixinAnnotation.WRAP_WITH_CONDITION -> listOf("INVOKE", "FIELD", "MIXINEXTRAS:EXPRESSION")
                MixinAnnotation.WRAP_OPERATION -> listOf("INVOKE", "FIELD", "NEW", "MIXINEXTRAS:EXPRESSION")
                else -> values
            }
        }

    private fun mergeCandidates(base: List<String>, additional: List<String>): List<String> {
        val seen = mutableSetOf<String>()
        val merged = mutableListOf<String>()
        for (value in base) {
            if (seen.add(value.uppercase())) merged.add(value)
        }
        for (value in additional) {
            if (!isAllowedAdditional(value, base)) continue
            if (seen.add(value.uppercase())) merged.add(value)
        }
        return merged
    }

    private fun isAllowedAdditional(value: String, allowedBase: List<String>): Boolean {
        val builtIn = matchingBuiltIn(value) ?: return true
        return allowedBase.any { it.equals(builtIn, ignoreCase = true) }
    }

    private fun matchingBuiltIn(value: String): String? {
        values.firstOrNull { value.equals(it, ignoreCase = true) }?.let { return it }
        return values
            .filter { value.length > it.length && value.startsWith("$it:", ignoreCase = true) }
            .maxByOrNull { it.length }
    }

    private fun completeSpecifiers(
        partial: String,
        candidates: List<String>,
        context: AnnotationContext,
    ): List<McCompletionItem>? {
        val code = candidates
            .filter { partial.length > it.length && partial.startsWith("$it:", ignoreCase = true) }
            .maxByOrNull { it.length }
            ?: return null
        if (!context.atInsideSlice && !injectionPointSpecifierSupported()) return null
        val suffixPrefix = partial.substring(code.length + 1)
        return specifiers
            .filter { it.startsWith(suffixPrefix, ignoreCase = true) }
            .map { toCompletionItem("$code:$it") }
    }

    private fun toCompletionItem(value: String) = McCompletionItem(
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
