package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.diagnostics.McTextRange

data class HandlerParameterSpec(
    val name: String,
    val typeDescriptor: String,
    val readableType: String,
    val isOperation: Boolean = false,
    val operationGenericDescriptor: String? = null,
    /** Additional descriptors accepted when the official expression type is int-like. */
    val acceptedTypeDescriptors: Set<String> = emptySet(),
    /** Additional Operation<T> descriptors accepted for an int-like operation result. */
    val acceptedOperationGenericDescriptors: Set<String> = emptySet(),
)

data class HandlerSignatureSpec(
    val returnTypeDescriptor: String,
    val readableReturnType: String,
    val parameters: List<HandlerParameterSpec>,
    val operationCallArgs: List<String> = emptyList(),
    /** Target-method parameters that may optionally be captured as a leading prefix after required handler params. */
    val optionalCapturedTargetParameters: List<HandlerParameterSpec> = emptyList(),
    /** Additional return descriptors accepted when the official expression result is int-like. */
    val acceptedReturnTypeDescriptors: Set<String> = emptySet(),
)

sealed interface HandlerParameterSugarSpec {
    data class Local(
        val argsOnly: Boolean = false,
        val index: Int? = null,
        val ordinal: Int? = null,
        val names: Set<String> = emptySet(),
        val print: Boolean = false,
        val typeClassName: String? = null,
    ) : HandlerParameterSugarSpec

    data class Share(
        val value: String? = null,
        val namespace: String? = null,
        val namespaceSpecified: Boolean = false,
    ) : HandlerParameterSugarSpec

    data object Cancellable : HandlerParameterSugarSpec
}

data class HandlerParameterDeclaration(
    val name: String,
    val typeName: String,
    val typeDescriptor: String?,
    val isOperation: Boolean,
    val operationGenericName: String?,
    val isSugar: Boolean = false,
    val sugarSpec: HandlerParameterSugarSpec? = null,
    val range: McTextRange? = null,
    val sugarAnnotationRange: McTextRange? = null,
    val hasCoerce: Boolean = false,
    val coerceAnnotationRange: McTextRange? = null,
)

data class HandlerMethodDeclaration(
    val methodName: String,
    val returnTypeName: String,
    val returnTypeDescriptor: String?,
    val parameters: List<HandlerParameterDeclaration>,
    val range: McTextRange,
    val isStatic: Boolean = false,
    val hasMethodLevelCoerce: Boolean = false,
)

/** Parsed `@At(shift = ..., by = ...)` occurrence offset for future `@Local` resolution. */
sealed interface AtShiftSpec {
    data object Before : AtShiftSpec

    data object After : AtShiftSpec

    data class By(val offset: Int) : AtShiftSpec

    data object Unresolved : AtShiftSpec
}

data class MixinExtrasAnnotationSite(
    val annotation: MixinExtrasAnnotation,
    val methodAttribute: String,
    val atValue: String?,
    val atTarget: String?,
    /** Parsed `@At(args = ...)` entries in lexical order. */
    val atArgs: List<String> = emptyList(),
    /** Id from the nested `@At(id = "...")` selector, when present. */
    val atId: String? = null,
    /** Ordinal from the nested `@At(ordinal = ...)` attribute, when present and valid. */
    val atOrdinal: Int? = null,
    /** Parsed `@At(shift = ..., by = ...)`; absent shift defaults to [AtShiftSpec.Before]. */
    val atShift: AtShiftSpec = AtShiftSpec.Before,
    /** True when a top-level `constant = @Constant(...)` selector is present on WrapOperation. */
    val hasConstantSelector: Boolean = false,
    val annotationRange: McTextRange,
    val handlerMethod: HandlerMethodDeclaration?,
    /** Typed `@Constant(expandZeroConditions = ...)` unary zero-branch opcodes. */
    val expandZeroConditions: Set<Int> = emptySet(),
)
