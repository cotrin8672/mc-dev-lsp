package io.github.mcdev.core.mixin

enum class MixinAnnotation(val simpleName: String) {
    MIXIN("Mixin"),
    SHADOW("Shadow"),
    ACCESSOR("Accessor"),
    INVOKER("Invoker"),
    INJECT("Inject"),
    REDIRECT("Redirect"),
    MODIFY_ARG("ModifyArg"),
    MODIFY_ARGS("ModifyArgs"),
    MODIFY_VARIABLE("ModifyVariable"),
    MODIFY_CONSTANT("ModifyConstant"),
    MODIFY_EXPRESSION_VALUE("ModifyExpressionValue"),
    MODIFY_RETURN_VALUE("ModifyReturnValue"),
    MODIFY_RECEIVER("ModifyReceiver"),
    WRAP_OPERATION("WrapOperation"),
    WRAP_WITH_CONDITION("WrapWithCondition"),
    WRAP_METHOD("WrapMethod"),
    AT("At"),
    ;

    companion object {
        private val byName = entries.associateBy { it.simpleName }

        fun fromSimpleName(name: String): MixinAnnotation? = byName[name]
    }
}

enum class AnnotationSlot {
    CLASS,
    TARGETS,
    METHOD,
    VALUE,
    TARGET,
    ACCESSOR_VALUE,
    INVOKER_VALUE,
    SHADOW_MEMBER,
    PREFIX,
    REMAP,
}

enum class MixinClassInsertMode {
    IMPORT,
    FQN,
}

enum class InjectMethodDescriptorMode {
    AUTO,
    ALWAYS,
    NEVER,
}

data class MixinCompletionOptions(
    val classInsertMode: MixinClassInsertMode = MixinClassInsertMode.IMPORT,
    val injectMethodDescriptorMode: InjectMethodDescriptorMode = InjectMethodDescriptorMode.AUTO,
    val preferredAtTarget: String = "descriptor",
)

data class AnnotationContext(
    val annotation: MixinAnnotation,
    val slot: AnnotationSlot,
    val partialValue: String,
    val valueStartOffset: Int,
    val valueEndOffset: Int,
    val annotationStartOffset: Int,
    val annotationEndOffset: Int,
    val mixinTargetInternalNames: List<String> = emptyList(),
    val injectMethodName: String? = null,
    val atValue: String? = null,
    val shadowPrefix: String? = null,
    val shadowRemap: Boolean = true,
)
