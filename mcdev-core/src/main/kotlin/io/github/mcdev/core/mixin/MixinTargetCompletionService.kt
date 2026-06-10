package io.github.mcdev.core.mixin

import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata

class MixinTargetCompletionService(
    private val classIndex: ClassIndex,
) {
    fun complete(context: AnnotationContext, options: MixinCompletionOptions = MixinCompletionOptions()): List<McCompletionItem> {
        if (context.annotation != MixinAnnotation.MIXIN) return emptyList()
        return when (context.slot) {
            AnnotationSlot.CLASS -> completeClassReference(context, options)
            AnnotationSlot.TARGETS -> completeTargetsString(context)
            else -> emptyList()
        }
    }

    private fun completeClassReference(
        context: AnnotationContext,
        options: MixinCompletionOptions,
    ): List<McCompletionItem> {
        val prefix = context.partialValue
        return classIndex.findClasses(prefix).map { entry ->
            val insertText = when (options.classInsertMode) {
                MixinClassInsertMode.IMPORT -> "${entry.simpleName}.class"
                MixinClassInsertMode.FQN -> "${entry.fqn}.class"
            }
            McCompletionItem(
                label = entry.simpleName,
                detail = entry.packageName,
                documentation = entry.internalName,
                filterText = "${entry.simpleName} ${entry.packageName} ${entry.fqn}",
                insertText = insertText,
                kind = McCompletionKind.CLASS,
                sortKey = "0100_${entry.simpleName}",
                metadata = McCompletionMetadata(
                    source = "mixin.target",
                    owner = entry.internalName,
                    name = entry.simpleName,
                    namespace = "NAMED",
                ),
            )
        }
    }

    private fun completeTargetsString(context: AnnotationContext): List<McCompletionItem> {
        val partial = context.partialValue.trim('"')
        val prefix = partial.substringAfterLast('.').substringAfterLast('/')
        return classIndex.findClasses(prefix).map { entry ->
            McCompletionItem(
                label = entry.simpleName,
                detail = entry.fqn,
                documentation = entry.internalName,
                filterText = "${entry.simpleName} ${entry.fqn}",
                insertText = entry.fqn,
                kind = McCompletionKind.CLASS,
                sortKey = "0100_${entry.simpleName}",
                metadata = McCompletionMetadata(
                    source = "mixin.targets",
                    owner = entry.internalName,
                    name = entry.simpleName,
                    namespace = "NAMED",
                ),
            )
        }
    }
}
