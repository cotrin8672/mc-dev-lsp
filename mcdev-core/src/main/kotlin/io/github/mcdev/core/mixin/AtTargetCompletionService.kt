package io.github.mcdev.core.mixin

import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata

class AtTargetCompletionService {
    fun complete(
        context: AnnotationContext,
        candidates: List<AtTargetCandidate>,
    ): List<McCompletionItem> {
        if (context.annotation != MixinAnnotation.AT || context.slot != AnnotationSlot.TARGET) return emptyList()
        val partial = context.partialValue.trim('"')
        return candidates
            .filter { candidate ->
                partial.isEmpty() ||
                    candidate.displayLabel.contains(partial, ignoreCase = true) ||
                    formatTarget(candidate).contains(partial, ignoreCase = true)
            }
            .map { candidate ->
                val insertText = formatTarget(candidate)
                val constantHint = ConstantAtHintFormatter.hintFor(candidate)
                McCompletionItem(
                    label = candidate.displayLabel,
                    detail = candidate.detail,
                    documentation = constantHint ?: insertText,
                    filterText = "${candidate.name} ${candidate.displayLabel} ${candidate.detail}",
                    insertText = insertText,
                    kind = when (candidate.kind) {
                        AtTargetKind.FIELD -> McCompletionKind.FIELD
                        else -> McCompletionKind.METHOD
                    },
                    sortKey = "0400_${candidate.name}",
                    metadata = McCompletionMetadata(
                        source = "mixin.atTarget",
                        owner = candidate.owner,
                        name = candidate.name,
                        descriptor = candidate.descriptor,
                        namespace = candidate.namespace.name,
                    ),
                )
            }
    }

    fun formatTarget(candidate: AtTargetCandidate): String = when (candidate.kind) {
        AtTargetKind.RETURN -> "RETURN"
        AtTargetKind.CONSTANT -> candidate.displayLabel
        AtTargetKind.FIELD -> "L${candidate.owner};${candidate.name}:${candidate.descriptor}"
        AtTargetKind.NEW -> if (candidate.name == "<init>") {
            "L${candidate.owner};<init>${candidate.descriptor}"
        } else {
            "L${candidate.owner};"
        }
        else -> "L${candidate.owner};${candidate.name}${candidate.descriptor}"
    }
}
