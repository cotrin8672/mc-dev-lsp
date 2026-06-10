package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.diagnostics.McTextRange

data class HandlerParameterSpec(
    val name: String,
    val typeDescriptor: String,
    val readableType: String,
    val isOperation: Boolean = false,
    val operationGenericDescriptor: String? = null,
)

data class HandlerSignatureSpec(
    val returnTypeDescriptor: String,
    val readableReturnType: String,
    val parameters: List<HandlerParameterSpec>,
    val operationCallArgs: List<String> = emptyList(),
)

data class HandlerParameterDeclaration(
    val name: String,
    val typeName: String,
    val typeDescriptor: String?,
    val isOperation: Boolean,
    val operationGenericName: String?,
)

data class HandlerMethodDeclaration(
    val methodName: String,
    val returnTypeName: String,
    val returnTypeDescriptor: String?,
    val parameters: List<HandlerParameterDeclaration>,
    val range: McTextRange,
)

data class MixinExtrasAnnotationSite(
    val annotation: MixinExtrasAnnotation,
    val methodAttribute: String,
    val atValue: String?,
    val atTarget: String?,
    val annotationRange: McTextRange,
    val handlerMethod: HandlerMethodDeclaration?,
)
