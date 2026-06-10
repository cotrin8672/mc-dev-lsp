package io.github.mcdev.core.mixin

import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata

class InjectMethodCompletionService(
    private val classIndex: ClassIndex,
) {
    fun complete(
        context: AnnotationContext,
        options: MixinCompletionOptions = MixinCompletionOptions(),
    ): List<McCompletionItem> {
        if (context.slot != AnnotationSlot.METHOD) return emptyList()
        val targetOwners = MixinTargetResolver.resolveTargets(context.mixinTargetInternalNames, classIndex)
        if (targetOwners.isEmpty()) return emptyList()
        val partial = context.partialValue.trim('"')
        val methodPrefix = partial.substringBefore('(')
        val items = mutableListOf<McCompletionItem>()
        for (owner in targetOwners) {
            val methods = classIndex.getMethods(owner).filter { it.name.startsWith(methodPrefix) }
            val grouped = methods.groupBy { it.name }
            for ((name, overloads) in grouped) {
                when {
                    options.injectMethodDescriptorMode == InjectMethodDescriptorMode.ALWAYS -> {
                        overloads.forEach { method ->
                            items += toCompletionItem(owner, method, descriptorQualified = true)
                        }
                    }
                    options.injectMethodDescriptorMode == InjectMethodDescriptorMode.NEVER -> {
                        items += toCompletionItem(owner, overloads.first(), descriptorQualified = false)
                    }
                    overloads.size == 1 -> {
                        items += toCompletionItem(owner, overloads.first(), descriptorQualified = false)
                    }
                    else -> {
                        overloads.forEach { method ->
                            items += toCompletionItem(owner, method, descriptorQualified = true)
                        }
                    }
                }
            }
        }
        return items.sortedBy { it.sortKey }
    }

    private fun toCompletionItem(
        owner: String,
        method: MethodIndexEntry,
        descriptorQualified: Boolean,
    ): McCompletionItem {
        val insertText = if (descriptorQualified) {
            "${method.name}${method.descriptor}"
        } else {
            method.name
        }
        return McCompletionItem(
            label = method.readableSignature,
            detail = AnnotationContextExtractor.internalToFqn(owner),
            documentation = method.descriptor,
            filterText = "${method.name} ${method.readableSignature}",
            insertText = insertText,
            kind = McCompletionKind.METHOD,
            sortKey = "0200_${method.name}",
            metadata = McCompletionMetadata(
                source = "mixin.injectMethod",
                owner = owner,
                name = method.name,
                descriptor = method.descriptor,
                namespace = "NAMED",
            ),
        )
    }
}
