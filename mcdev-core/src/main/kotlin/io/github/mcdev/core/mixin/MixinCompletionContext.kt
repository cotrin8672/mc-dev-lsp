package io.github.mcdev.core.mixin

import io.github.mcdev.core.diagnostics.McTextRange

sealed interface MixinCompletionContext {
    data class MixinTarget(
        val annotationRange: McTextRange,
        val partialValue: String,
    ) : MixinCompletionContext

    data class InjectMethod(
        val injector: InjectorModel,
        val partialValue: String,
    ) : MixinCompletionContext

    data class AtValue(
        val injector: InjectorModel,
        val atSelector: AtSelectorModel,
        val partialValue: String,
    ) : MixinCompletionContext

    data class AtTarget(
        val injector: InjectorModel,
        val atSelector: AtSelectorModel,
        val owner: String?,
        val methodName: String?,
        val methodDescriptor: String?,
        val partialValue: String,
    ) : MixinCompletionContext

    data class AccessorValue(
        val member: MixinMemberModel,
        val partialValue: String,
    ) : MixinCompletionContext

    data class InvokerValue(
        val member: MixinMemberModel,
        val partialValue: String,
    ) : MixinCompletionContext

    data class ShadowMember(
        val partialValue: String,
    ) : MixinCompletionContext
}
