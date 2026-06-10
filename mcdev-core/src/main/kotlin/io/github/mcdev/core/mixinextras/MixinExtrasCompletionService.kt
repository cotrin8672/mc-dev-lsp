package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.mixin.AnnotationContext
import io.github.mcdev.core.mixin.AnnotationSlot
import io.github.mcdev.core.mixin.AtTargetCompletionService
import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.InjectMethodCompletionService
import io.github.mcdev.core.mixin.MixinAnnotation
import io.github.mcdev.core.mixin.MixinCompletionOptions

class MixinExtrasCompletionService(
    private val classIndex: ClassIndex,
    private val bytecodeIndex: BytecodeIndex,
    private val injectMethodCompletion: InjectMethodCompletionService = InjectMethodCompletionService(classIndex),
    private val atTargetCompletion: AtTargetCompletionService = AtTargetCompletionService(),
) {
    private val methodCompletionAnnotations = setOf(
        MixinAnnotation.MODIFY_EXPRESSION_VALUE,
        MixinAnnotation.MODIFY_RETURN_VALUE,
        MixinAnnotation.WRAP_OPERATION,
        MixinAnnotation.WRAP_WITH_CONDITION,
        MixinAnnotation.WRAP_METHOD,
    )

    fun complete(
        context: AnnotationContext,
        options: MixinCompletionOptions = MixinCompletionOptions(),
    ): List<McCompletionItem> {
        if (context.annotation in methodCompletionAnnotations && context.slot == AnnotationSlot.METHOD) {
            return injectMethodCompletion.complete(context, options).map { item ->
                item.copy(metadata = item.metadata.copy(source = "mixinextras.injectMethod"))
            }
        }
        if (context.annotation == MixinAnnotation.AT && context.slot == AnnotationSlot.TARGET &&
            isWrapOperationContext(context)
        ) {
            return completeWrapOperationAtTarget(context)
        }
        return emptyList()
    }

    private fun isWrapOperationContext(context: AnnotationContext): Boolean {
        if (context.atValue != "INVOKE") return false
        val source = context.partialValue
        return source.isNotEmpty() || context.injectMethodName != null
    }

    private fun completeWrapOperationAtTarget(context: AnnotationContext): List<McCompletionItem> {
        val owner = context.mixinTargetInternalNames.firstOrNull() ?: return emptyList()
        val methodName = context.injectMethodName ?: return emptyList()
        val candidates = bytecodeIndex.getAtTargetCandidates(owner, methodName, null, "INVOKE")
        return atTargetCompletion.complete(context, candidates)
    }

    fun completeAtTargetForWrapOperation(
        mixinTargets: List<String>,
        injectMethodName: String,
        injectMethodDescriptor: String?,
        context: AnnotationContext,
    ): List<McCompletionItem> {
        val owner = mixinTargets.firstOrNull() ?: return emptyList()
        val candidates = bytecodeIndex.getAtTargetCandidates(owner, injectMethodName, injectMethodDescriptor, "INVOKE")
        return atTargetCompletion.complete(context, candidates)
    }
}
