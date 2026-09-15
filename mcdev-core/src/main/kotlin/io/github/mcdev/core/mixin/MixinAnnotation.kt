package io.github.mcdev.core.mixin

import io.github.mcdev.core.mixinextras.ExpressionCompletionPosition

enum class MixinAnnotation(
    val simpleName: String,
    vararg fqns: String,
) {
    MIXIN("Mixin", "org.spongepowered.asm.mixin.Mixin"),
    FINAL("Final", "org.spongepowered.asm.mixin.Final"),
    MUTABLE("Mutable", "org.spongepowered.asm.mixin.Mutable"),
    PSEUDO("Pseudo", "org.spongepowered.asm.mixin.Pseudo"),
    SOFT_OVERRIDE("SoftOverride", "org.spongepowered.asm.mixin.SoftOverride"),
    SHADOW("Shadow", "org.spongepowered.asm.mixin.Shadow"),
    ACCESSOR("Accessor", "org.spongepowered.asm.mixin.gen.Accessor"),
    INVOKER("Invoker", "org.spongepowered.asm.mixin.gen.Invoker"),
    INJECT("Inject", "org.spongepowered.asm.mixin.injection.Inject"),
    REDIRECT("Redirect", "org.spongepowered.asm.mixin.injection.Redirect"),
    MODIFY_ARG("ModifyArg", "org.spongepowered.asm.mixin.injection.ModifyArg"),
    MODIFY_ARGS("ModifyArgs", "org.spongepowered.asm.mixin.injection.ModifyArgs"),
    MODIFY_VARIABLE("ModifyVariable", "org.spongepowered.asm.mixin.injection.ModifyVariable"),
    MODIFY_CONSTANT("ModifyConstant", "org.spongepowered.asm.mixin.injection.ModifyConstant"),
    MODIFY_EXPRESSION_VALUE("ModifyExpressionValue", "com.llamalad7.mixinextras.injector.ModifyExpressionValue"),
    MODIFY_RETURN_VALUE("ModifyReturnValue", "com.llamalad7.mixinextras.injector.ModifyReturnValue"),
    MODIFY_RECEIVER("ModifyReceiver", "com.llamalad7.mixinextras.injector.ModifyReceiver"),
    WRAP_OPERATION("WrapOperation", "com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation"),
    WRAP_WITH_CONDITION(
        "WrapWithCondition",
        "com.llamalad7.mixinextras.injector.WrapWithCondition",
        "com.llamalad7.mixinextras.injector.v2.WrapWithCondition",
    ),
    WRAP_METHOD("WrapMethod", "com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod"),
    OVERWRITE("Overwrite", "org.spongepowered.asm.mixin.Overwrite"),
    SURROGATE("Surrogate", "org.spongepowered.asm.mixin.injection.Surrogate"),
    AT("At", "org.spongepowered.asm.mixin.injection.At"),
    CONSTANT("Constant", "org.spongepowered.asm.mixin.injection.Constant"),
    SLICE("Slice", "org.spongepowered.asm.mixin.injection.Slice"),
    LOCAL("Local", "com.llamalad7.mixinextras.sugar.Local"),
    SHARE("Share", "com.llamalad7.mixinextras.sugar.Share"),
    DEFINITION("Definition", "com.llamalad7.mixinextras.expression.Definition"),
    DEFINITIONS("Definitions", "com.llamalad7.mixinextras.expression.Definitions"),
    EXPRESSION("Expression", "com.llamalad7.mixinextras.expression.Expression"),
    EXPRESSIONS("Expressions", "com.llamalad7.mixinextras.expression.Expressions"),
    ;

    val officialFqns: Set<String> = setOf(*fqns)

    companion object {
        private val byName = entries.associateBy { it.simpleName }

        fun fromSimpleName(name: String): MixinAnnotation? = byName[name]

        fun fromOfficialFqn(fqn: String): MixinAnnotation? =
            entries.firstOrNull { fqn in it.officialFqns }
    }
}

enum class AnnotationSlot {
    ATTRIBUTE,
    CLASS,
    TARGETS,
    METHOD,
    HANDLER,
    VALUE,
    TARGET,
    ACCESSOR_VALUE,
    INVOKER_VALUE,
    SHADOW_MEMBER,
    OVERWRITE_METHOD,
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

/**
 * Decoded expression string content from the value literal start through the caret.
 * Intended for [io.github.mcdev.core.mixinextras.ExpressionReceiverParser].
 */
data class DecodedExpressionPrefix(
    val prefix: String,
    val cursor: Int,
)

data class MixinCompletionOptions(
    val classInsertMode: MixinClassInsertMode = MixinClassInsertMode.IMPORT,
    val injectMethodDescriptorMode: InjectMethodDescriptorMode = InjectMethodDescriptorMode.AUTO,
    val preferredAtTarget: String = "smart",
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
    /** True for a Shadow method, false for a Shadow field, null when the declaration kind is unknown. */
    val shadowMemberIsMethod: Boolean? = null,
    val existingAttributes: Set<String> = emptySet(),
    val parentInjectorAnnotation: MixinAnnotation? = null,
    val atInsideSlice: Boolean = false,
    val expressionCompletionPosition: ExpressionCompletionPosition? = null,
    /** Decoded Java string content for the current expression identifier token, when applicable. */
    val decodedPartialValue: String? = null,
    /** Decoded expression content from the value literal start through the caret, when applicable. */
    val decodedExpressionPrefix: DecodedExpressionPrefix? = null,
    val attributeName: String? = null,
    val resolvedAnnotationFqn: String? = null,
)
