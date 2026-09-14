package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange

data class ResolvedMixinExtrasContext(
    val handlerRange: McTextRange,
    val context: ExpressionContext,
)

fun selectResolvedMixinExtrasContext(
    site: MixinExtrasAnnotationSite,
    contexts: List<ResolvedMixinExtrasContext>,
): ResolvedMixinExtrasContext? {
    if (contexts.isEmpty()) return null
    val byAnnotation = contexts.filter { it.handlerRange.containsRange(site.annotationRange) }
    if (byAnnotation.size == 1) return byAnnotation.single()
    if (byAnnotation.size > 1) return null
    val handlerRange = site.handlerMethod?.range ?: return null
    val byHandler = contexts.filter {
        it.handlerRange.containsRange(handlerRange) || it.handlerRange.overlapsRange(handlerRange)
    }
    return when (byHandler.size) {
        1 -> byHandler.single()
        else -> null
    }
}

private fun McTextPosition.compareToPosition(other: McTextPosition): Int {
    val lineCompare = line.compareTo(other.line)
    if (lineCompare != 0) return lineCompare
    return character.compareTo(other.character)
}

private fun McTextRange.containsRange(inner: McTextRange): Boolean =
    start.compareToPosition(inner.start) <= 0 && inner.end.compareToPosition(end) <= 0

private fun McTextRange.overlapsRange(other: McTextRange): Boolean =
    start.compareToPosition(other.end) < 0 && other.start.compareToPosition(end) < 0
