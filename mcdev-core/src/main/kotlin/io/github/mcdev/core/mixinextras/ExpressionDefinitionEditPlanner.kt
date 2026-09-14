package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.codeaction.McTextEdit
import io.github.mcdev.core.mixin.AnnotationContext
import io.github.mcdev.core.mixin.AnnotationSlot
import io.github.mcdev.core.mixin.MixinAnnotation
import io.github.mcdev.core.mixin.MixinImportEditBuilder

sealed interface ExpressionDefinitionEditPlanResult {
    data class Available(
        val insertText: String,
        val additionalEdits: List<McTextEdit>,
    ) : ExpressionDefinitionEditPlanResult

    data class Unavailable(
        val reason: String,
    ) : ExpressionDefinitionEditPlanResult
}

object ExpressionDefinitionEditPlanner {
    private const val DEFINITION_FQN = "com.llamalad7.mixinextras.expression.Definition"

    fun plan(
        context: AnnotationContext,
        expressionAnnotationOffset: Int,
        source: String,
        definitionIndex: MixinExtrasDefinitionIndex,
        candidate: OfficialExpressionMemberCandidate,
    ): ExpressionDefinitionEditPlanResult {
        if (context.annotation != MixinAnnotation.EXPRESSION && context.annotation != MixinAnnotation.EXPRESSIONS) {
            return ExpressionDefinitionEditPlanResult.Unavailable("annotation is not Expression")
        }
        if (context.slot != AnnotationSlot.VALUE) {
            return ExpressionDefinitionEditPlanResult.Unavailable("expression completion context is not in VALUE slot")
        }
        if (context.expressionCompletionPosition != ExpressionCompletionPosition.AFTER_DOT) {
            return ExpressionDefinitionEditPlanResult.Unavailable("expression is not in AFTER_DOT completion state")
        }
        if (expressionAnnotationOffset < 0 || expressionAnnotationOffset >= source.length) {
            return ExpressionDefinitionEditPlanResult.Unavailable("invalid expression annotation offset")
        }
        if (source[expressionAnnotationOffset] != '@') {
            return ExpressionDefinitionEditPlanResult.Unavailable("expression annotation offset does not point at @")
        }
        if (expressionAnnotationOffset != context.annotationStartOffset) {
            return ExpressionDefinitionEditPlanResult.Unavailable("expression annotation offset mismatch")
        }

        val selector = canonicalExactSelector(candidate)
            ?: return ExpressionDefinitionEditPlanResult.Unavailable("candidate cannot be represented as exact selector")

        val reusedId = findReusableDefinitionId(definitionIndex, candidate.kind, selector)
        val identifier = reusedId ?: allocateDefinitionId(definitionIndex, candidate.name)
        val insertText = when (candidate.kind) {
            OfficialExpressionMemberKind.FIELD -> identifier
            OfficialExpressionMemberKind.METHOD -> "$identifier()"
        }

        if (reusedId != null) {
            return ExpressionDefinitionEditPlanResult.Available(insertText, emptyList())
        }

        val additionalEdits = buildList {
            add(definitionInsertEdit(source, expressionAnnotationOffset, identifier, candidate, selector))
            MixinImportEditBuilder.buildImportEdit(source, DEFINITION_FQN)?.let(::add)
        }
        val normalized = normalizeAdditionalEdits(additionalEdits)
            ?: return ExpressionDefinitionEditPlanResult.Unavailable("planned additional edits overlap or collide")

        return ExpressionDefinitionEditPlanResult.Available(insertText, normalized)
    }

    internal fun canonicalExactSelector(candidate: OfficialExpressionMemberCandidate): String? {
        if (candidate.ownerInternalName.isBlank() || candidate.name.isBlank() || candidate.descriptor.isBlank()) {
            return null
        }
        return when (candidate.kind) {
            OfficialExpressionMemberKind.FIELD ->
                "L${candidate.ownerInternalName};${candidate.name}:${candidate.descriptor}"
            OfficialExpressionMemberKind.METHOD ->
                "L${candidate.ownerInternalName};${candidate.name}${candidate.descriptor}"
        }
    }

    internal fun findReusableDefinitionId(
        definitionIndex: MixinExtrasDefinitionIndex,
        kind: OfficialExpressionMemberKind,
        exactSelector: String,
    ): String? {
        val matchingIds = linkedSetOf<String>()
        for (definition in definitionIndex.definitions) {
            val id = definition.id ?: continue
            val references = when (kind) {
                OfficialExpressionMemberKind.FIELD -> definition.rawFieldReferences
                OfficialExpressionMemberKind.METHOD -> definition.rawMethodReferences
            }
            if (exactSelector in references) {
                matchingIds += id
            }
        }
        if (matchingIds.size != 1) {
            return null
        }
        val id = matchingIds.single()
        if (definitionIndex.definitionsWithId(id).size != 1) {
            return null
        }
        return id
    }

    internal fun allocateDefinitionId(
        definitionIndex: MixinExtrasDefinitionIndex,
        memberName: String,
    ): String {
        if (!isReadableBaseId(memberName)) {
            return "member"
        }
        val occupied = definitionIndex.definitions.mapNotNull { it.id }.toSet()
        var candidate = memberName
        var suffix = 2
        while (candidate in occupied) {
            candidate = "$memberName$suffix"
            suffix++
        }
        return candidate
    }

    private fun definitionInsertEdit(
        source: String,
        expressionAnnotationOffset: Int,
        identifier: String,
        candidate: OfficialExpressionMemberCandidate,
        selector: String,
    ): McTextEdit {
        val indent = lineIndentAt(source, expressionAnnotationOffset)
        val lineEnding = lineEndingAt(source, expressionAnnotationOffset)
        val memberAttribute = when (candidate.kind) {
            OfficialExpressionMemberKind.FIELD -> "field"
            OfficialExpressionMemberKind.METHOD -> "method"
        }
        val definitionLine = "@Definition(id = \"$identifier\", $memberAttribute = \"$selector\")$lineEnding$indent"
        return McTextEdit(
            startOffset = expressionAnnotationOffset,
            endOffset = expressionAnnotationOffset,
            newText = definitionLine,
        )
    }

    private fun lineIndentAt(source: String, offset: Int): String {
        val lineStart = source.lastIndexOf('\n', (offset - 1).coerceAtLeast(0)).let { if (it < 0) 0 else it + 1 }
        val indentEnd = (lineStart until offset).firstOrNull { !source[it].isWhitespace() } ?: offset
        return source.substring(lineStart, indentEnd)
    }

    private fun lineEndingAt(source: String, offset: Int): String {
        val prevNewline = source.lastIndexOf('\n', (offset - 1).coerceAtLeast(0))
        if (prevNewline > 0 && source[prevNewline - 1] == '\r') {
            return "\r\n"
        }
        val windowStart = (offset - 256).coerceAtLeast(0)
        val windowEnd = (offset + 256).coerceAtMost(source.length)
        var crlfCount = 0
        var lfCount = 0
        var index = windowStart
        while (index < windowEnd) {
            when {
                source[index] == '\r' && source.getOrNull(index + 1) == '\n' -> {
                    crlfCount++
                    index += 2
                }
                source[index] == '\n' -> {
                    lfCount++
                    index++
                }
                else -> index++
            }
        }
        return if (crlfCount > lfCount) "\r\n" else "\n"
    }

    private fun normalizeAdditionalEdits(edits: List<McTextEdit>): List<McTextEdit>? {
        if (edits.isEmpty()) {
            return emptyList()
        }
        val sorted = edits.sortedBy { it.startOffset }
        for (index in sorted.indices) {
            val current = sorted[index]
            for (otherIndex in index + 1 until sorted.size) {
                val other = sorted[otherIndex]
                if (current.startOffset == other.startOffset && current.endOffset == other.endOffset) {
                    return null
                }
                if (current.endOffset > other.startOffset) {
                    return null
                }
            }
        }
        return sorted
    }

    private fun isReadableBaseId(name: String): Boolean =
        name.isNotEmpty() &&
            (name.first().isLetter() || name.first() == '_' || name.first() == '$') &&
            name.all { it.isLetterOrDigit() || it == '_' || it == '$' }
}
