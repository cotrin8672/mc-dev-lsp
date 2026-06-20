package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.mixin.MixinAnnotation

enum class MixinExtrasAnnotation(val simpleName: String) {
    MODIFY_EXPRESSION_VALUE("ModifyExpressionValue"),
    MODIFY_RETURN_VALUE("ModifyReturnValue"),
    MODIFY_RECEIVER("ModifyReceiver"),
    WRAP_OPERATION("WrapOperation"),
    WRAP_WITH_CONDITION("WrapWithCondition"),
    WRAP_METHOD("WrapMethod"),
    LOCAL("Local"),
    SHARE("Share"),
    CANCELLABLE("Cancellable"),
    DEFINITION("Definition"),
    DEFINITIONS("Definitions"),
    EXPRESSION("Expression"),
    EXPRESSIONS("Expressions"),
    ;

    companion object {
        private val byName = entries.associateBy { it.simpleName }

        fun fromSimpleName(name: String): MixinExtrasAnnotation? = byName[name]

        fun fromMixinAnnotation(annotation: MixinAnnotation): MixinExtrasAnnotation? = when (annotation) {
            MixinAnnotation.MODIFY_EXPRESSION_VALUE -> MODIFY_EXPRESSION_VALUE
            MixinAnnotation.MODIFY_RETURN_VALUE -> MODIFY_RETURN_VALUE
            MixinAnnotation.MODIFY_RECEIVER -> MODIFY_RECEIVER
            MixinAnnotation.WRAP_OPERATION -> WRAP_OPERATION
            MixinAnnotation.WRAP_WITH_CONDITION -> WRAP_WITH_CONDITION
            MixinAnnotation.WRAP_METHOD -> WRAP_METHOD
            else -> null
        }

        val injectorAnnotations = setOf(
            MODIFY_EXPRESSION_VALUE,
            MODIFY_RETURN_VALUE,
            MODIFY_RECEIVER,
            WRAP_OPERATION,
            WRAP_WITH_CONDITION,
            WRAP_METHOD,
        )
    }
}
