package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.descriptor.DescriptorParseResult
import io.github.mcdev.core.descriptor.DescriptorRenderer
import io.github.mcdev.core.descriptor.MemberTarget
import io.github.mcdev.core.descriptor.MemberTargetParser
import io.github.mcdev.core.descriptor.parseMethodDescriptor
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.mixin.MixinTargetResolver

class HandlerSignatureService(
    private val classIndex: ClassIndex,
    private val bytecodeIndex: BytecodeIndex? = null,
) {
    fun resolveTargetMethod(
        mixinTargets: List<String>,
        methodAttribute: String,
    ): MethodIndexEntry? {
        val name = methodAttribute.substringBefore('(')
        val descriptorSuffix = methodAttribute.substringAfter('(', "")
        val owners = MixinTargetResolver.resolveTargets(mixinTargets, classIndex)
        val searchOwners = owners.ifEmpty {
            classIndex.findClasses("", limit = 1000).map { it.internalName }
        }
        val matches = searchOwners.flatMap { owner -> classIndex.getMethods(owner).filter { it.name == name } }
        if (matches.isEmpty()) return null
        if (descriptorSuffix.isEmpty()) return matches.singleOrNull() ?: matches.first()
        val fullDescriptor = "($descriptorSuffix"
        return matches.find { it.descriptor == fullDescriptor || methodAttribute.endsWith(it.descriptor) }
            ?: matches.first()
    }

    fun expectedSignature(
        source: String,
        site: MixinExtrasAnnotationSite,
        mixinTargets: List<String>,
    ): HandlerSignatureSpec? {
        val targetMethod = resolveTargetMethod(mixinTargets, site.methodAttribute) ?: return null
        return when (site.annotation) {
            MixinExtrasAnnotation.MODIFY_EXPRESSION_VALUE -> expectedModifyExpressionValue(source, site, targetMethod, mixinTargets)
            MixinExtrasAnnotation.MODIFY_RETURN_VALUE -> expectedModifyReturnValue(targetMethod)
            MixinExtrasAnnotation.WRAP_OPERATION -> expectedWrapOperation(site, targetMethod)
            MixinExtrasAnnotation.WRAP_WITH_CONDITION -> expectedWrapWithCondition(site, targetMethod)
            MixinExtrasAnnotation.WRAP_METHOD -> expectedWrapMethod(targetMethod, mixinTargets)
            else -> null
        }
    }

    fun generateHandlerStub(
        source: String,
        site: MixinExtrasAnnotationSite,
        mixinTargets: List<String>,
        methodName: String = "mcdevHandler",
        indent: String = "    ",
    ): String? {
        val spec = expectedSignature(source, site, mixinTargets) ?: return null
        val params = spec.parameters.joinToString(", ") { "${it.readableType} ${it.name}" }
        val callArgs = spec.operationCallArgs.joinToString(", ")
        val body = when (site.annotation) {
            MixinExtrasAnnotation.WRAP_WITH_CONDITION -> {
                val op = spec.parameters.lastOrNull()?.name ?: "original"
                "${indent}return $op.call($callArgs);\n"
            }
            MixinExtrasAnnotation.WRAP_OPERATION, MixinExtrasAnnotation.WRAP_METHOD -> {
                val op = spec.parameters.lastOrNull()?.name ?: "original"
                val ret = if (spec.returnTypeDescriptor == "V") "" else "return "
                "${indent}${ret}$op.call($callArgs);\n"
            }
            MixinExtrasAnnotation.MODIFY_EXPRESSION_VALUE, MixinExtrasAnnotation.MODIFY_RETURN_VALUE -> {
                if (spec.parameters.isEmpty()) {
                    "${indent}// TODO\n"
                } else {
                    val original = spec.parameters.first().name
                    "${indent}return $original;\n"
                }
            }
            else -> "${indent}// TODO\n"
        }
        return "${indent}${spec.readableReturnType} $methodName($params) {\n$body${indent}}\n"
    }

    fun validateHandler(
        source: String,
        site: MixinExtrasAnnotationSite,
        mixinTargets: List<String>,
        handler: HandlerMethodDeclaration,
    ): List<HandlerValidationIssue> {
        val expected = expectedSignature(source, site, mixinTargets) ?: return emptyList()
        val issues = mutableListOf<HandlerValidationIssue>()
        if (handler.returnTypeDescriptor != null && handler.returnTypeDescriptor != expected.returnTypeDescriptor) {
            issues += HandlerValidationIssue(
                code = MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE,
                message = "MixinExtras handler return type should be ${expected.readableReturnType}",
                range = handler.range,
            )
        }
        val operationParams = handler.parameters.filter { it.isOperation }
        when (site.annotation) {
            MixinExtrasAnnotation.WRAP_OPERATION,
            MixinExtrasAnnotation.WRAP_WITH_CONDITION,
            MixinExtrasAnnotation.WRAP_METHOD,
            -> {
                if (operationParams.isEmpty()) {
                    issues += HandlerValidationIssue(
                        code = MixinExtrasDiagnosticCodes.MISSING_OPERATION_PARAMETER,
                        message = "MixinExtras handler is missing Operation parameter",
                        range = handler.range,
                    )
                    return issues
                }
                val expectedOp = expected.parameters.lastOrNull { it.isOperation }
                val actualOp = operationParams.last()
                if (expectedOp != null && actualOp.operationGenericName != null) {
                    if (!operationGenericMatches(expectedOp.operationGenericDescriptor!!, actualOp.operationGenericName)) {
                        issues += HandlerValidationIssue(
                            code = MixinExtrasDiagnosticCodes.WRONG_OPERATION_GENERIC,
                            message = "Operation generic should be ${OperationSignatureRenderer.readableType(expectedOp.operationGenericDescriptor!!)}",
                            range = handler.range,
                        )
                    }
                }
                if (actualOp != handler.parameters.last()) {
                    issues += HandlerValidationIssue(
                        code = MixinExtrasDiagnosticCodes.MISSING_OPERATION_PARAMETER,
                        message = "Operation<T> must be the last handler parameter",
                        range = handler.range,
                    )
                }
            }
            MixinExtrasAnnotation.MODIFY_EXPRESSION_VALUE,
            MixinExtrasAnnotation.MODIFY_RETURN_VALUE,
            -> {
                val expectedOriginal = expected.parameters.firstOrNull()
                val actualOriginal = handler.parameters.firstOrNull()
                if (expectedOriginal != null && actualOriginal?.typeDescriptor != null &&
                    actualOriginal.typeDescriptor != expectedOriginal.typeDescriptor
                ) {
                    issues += HandlerValidationIssue(
                        code = MixinExtrasDiagnosticCodes.WRONG_ORIGINAL_VALUE_TYPE,
                        message = "Original value parameter should be ${expectedOriginal.readableType}",
                        range = handler.range,
                    )
                }
            }
            else -> Unit
        }
        if (issues.isEmpty() && !parametersMatch(expected, handler)) {
            issues += HandlerValidationIssue(
                code = MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH,
                message = "MixinExtras handler signature mismatch",
                range = handler.range,
            )
        }
        return issues
    }

    private fun parametersMatch(expected: HandlerSignatureSpec, handler: HandlerMethodDeclaration): Boolean {
        if (handler.parameters.size != expected.parameters.size) return false
        return handler.parameters.zip(expected.parameters).all { (actual, exp) ->
            if (exp.isOperation) {
                actual.isOperation
            } else {
                actual.typeDescriptor == null || actual.typeDescriptor == exp.typeDescriptor
            }
        }
    }

    private fun expectedModifyExpressionValue(
        source: String,
        site: MixinExtrasAnnotationSite,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
    ): HandlerSignatureSpec {
        val expressionType = inferExpressionValueType(source, site, targetMethod, mixinTargets)
        return HandlerSignatureSpec(
            returnTypeDescriptor = expressionType,
            readableReturnType = OperationSignatureRenderer.readableType(expressionType),
            parameters = listOf(
                HandlerParameterSpec(
                    name = "original",
                    typeDescriptor = expressionType,
                    readableType = OperationSignatureRenderer.readableType(expressionType),
                ),
            ),
        )
    }

    private fun expectedModifyReturnValue(targetMethod: MethodIndexEntry): HandlerSignatureSpec {
        val returnType = methodReturnDescriptor(targetMethod.descriptor)
        val params = if (returnType == "V") {
            emptyList()
        } else {
            listOf(
                HandlerParameterSpec(
                    name = "original",
                    typeDescriptor = returnType,
                    readableType = OperationSignatureRenderer.readableType(returnType),
                ),
            )
        }
        return HandlerSignatureSpec(
            returnTypeDescriptor = returnType,
            readableReturnType = OperationSignatureRenderer.readableType(returnType),
            parameters = params,
        )
    }

    private fun expectedWrapOperation(
        site: MixinExtrasAnnotationSite,
        @Suppress("UNUSED_PARAMETER") targetMethod: MethodIndexEntry,
    ): HandlerSignatureSpec? {
        val wrapped = parseAtInvokeTarget(site.atTarget) ?: return null
        return buildWrapSignature(wrapped, operationGenericFromReturn(wrapped))
    }

    private fun expectedWrapWithCondition(
        site: MixinExtrasAnnotationSite,
        @Suppress("UNUSED_PARAMETER") targetMethod: MethodIndexEntry,
    ): HandlerSignatureSpec? {
        val wrapped = parseAtInvokeTarget(site.atTarget) ?: return null
        return buildWrapSignature(wrapped, "Z", returnDescriptor = "Z")
    }

    private fun expectedWrapMethod(targetMethod: MethodIndexEntry, mixinTargets: List<String>): HandlerSignatureSpec {
        val parsed = parseMethodDescriptor(targetMethod.descriptor)
        if (parsed !is DescriptorParseResult.Success) {
            return HandlerSignatureSpec("V", "void", emptyList())
        }
        val returnType = DescriptorRenderer.toDescriptor(parsed.value.returnType)
        val params = mutableListOf<HandlerParameterSpec>()
        if (!targetMethod.isStatic) {
            val ownerInternal = mixinTargets.firstOrNull()
            val receiverType = ownerInternal?.let { "L$it;" } ?: "Ljava/lang/Object;"
            val receiverReadable = ownerInternal?.substringAfterLast('/') ?: "Object"
            params += HandlerParameterSpec(
                name = decapitalize(receiverReadable),
                typeDescriptor = receiverType,
                readableType = OperationSignatureRenderer.readableType(receiverType),
            )
        }
        parsed.value.parameters.forEachIndexed { index, param ->
            params += HandlerParameterSpec(
                name = "arg$index",
                typeDescriptor = DescriptorRenderer.toDescriptor(param),
                readableType = DescriptorRenderer.render(param),
            )
        }
        val callArgs = params.map { it.name }
        params += HandlerParameterSpec(
            name = "original",
            typeDescriptor = "Lcom/llamalad7/mixinextras/injector/wrapoperation/Operation;",
            readableType = OperationSignatureRenderer.renderOperationType(returnType),
            isOperation = true,
            operationGenericDescriptor = returnType,
        )
        return HandlerSignatureSpec(
            returnTypeDescriptor = returnType,
            readableReturnType = OperationSignatureRenderer.readableType(returnType),
            parameters = params,
            operationCallArgs = callArgs,
        )
    }

    private fun buildWrapSignature(
        wrapped: WrappedOperationTarget,
        operationGeneric: String,
        returnDescriptor: String = operationGeneric,
    ): HandlerSignatureSpec {
        val params = mutableListOf<HandlerParameterSpec>()
        val callArgs = mutableListOf<String>()
        if (!wrapped.isStatic) {
            val receiverType = "L${wrapped.owner};"
            val receiverName = receiverParameterName(wrapped.owner)
            params += HandlerParameterSpec(
                name = receiverName,
                typeDescriptor = receiverType,
                readableType = OperationSignatureRenderer.readableType(receiverType),
            )
            callArgs += receiverName
        }
        wrapped.parameterDescriptors.forEachIndexed { index, descriptor ->
            params += HandlerParameterSpec(
                name = "arg$index",
                typeDescriptor = descriptor,
                readableType = OperationSignatureRenderer.readableType(descriptor),
            )
            callArgs += "arg$index"
        }
        params += HandlerParameterSpec(
            name = "original",
            typeDescriptor = "Lcom/llamalad7/mixinextras/injector/wrapoperation/Operation;",
            readableType = OperationSignatureRenderer.renderOperationType(operationGeneric),
            isOperation = true,
            operationGenericDescriptor = operationGeneric,
        )
        return HandlerSignatureSpec(
            returnTypeDescriptor = returnDescriptor,
            readableReturnType = OperationSignatureRenderer.readableType(returnDescriptor),
            parameters = params,
            operationCallArgs = callArgs,
        )
    }

    private fun inferExpressionValueType(
        source: String,
        site: MixinExtrasAnnotationSite,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
    ): String {
        val atValue = site.atValue?.uppercase()
        if (atValue == "MIXINEXTRAS:EXPRESSION" && bytecodeIndex != null) {
            ExpressionContextResolver.inferExpressionValueType(
                source = source,
                site = site,
                targetMethod = targetMethod,
                mixinTargets = mixinTargets,
                bytecodeIndex = bytecodeIndex,
                classIndex = classIndex,
            )?.let { return it }
        }
        if (atValue == "CONSTANT") {
            val args = site.atTarget.orEmpty()
            return when {
                args.contains("floatValue") -> "F"
                args.contains("intValue") -> "I"
                args.contains("longValue") -> "J"
                args.contains("doubleValue") -> "D"
                args.contains("stringValue") -> "Ljava/lang/String;"
                else -> "Ljava/lang/Object;"
            }
        }
        if (atValue == "RETURN") {
            return methodReturnDescriptor(targetMethod.descriptor)
        }
        return "Ljava/lang/Object;"
    }

    private fun parseAtInvokeTarget(atTarget: String?): WrappedOperationTarget? {
        if (atTarget.isNullOrBlank()) return null
        return when (val parsed = MemberTargetParser.parse(atTarget)) {
            is DescriptorParseResult.Success -> when (val target = parsed.value) {
                is MemberTarget.Method -> {
                    val isStatic = classIndex.getMethods(target.owner)
                        .find { it.name == target.name && it.descriptor == DescriptorRenderer.toDescriptor(target.descriptor) }
                        ?.isStatic
                        ?: false
                    WrappedOperationTarget(
                        owner = target.owner,
                        name = target.name,
                        parameterDescriptors = target.descriptor.parameters.map(DescriptorRenderer::toDescriptor),
                        returnDescriptor = DescriptorRenderer.toDescriptor(target.descriptor.returnType),
                        isStatic = isStatic,
                    )
                }
                else -> null
            }
            else -> null
        }
    }

    private fun operationGenericFromReturn(wrapped: WrappedOperationTarget): String = wrapped.returnDescriptor

    private fun methodReturnDescriptor(descriptor: String): String {
        val close = descriptor.indexOf(')')
        return if (close >= 0 && close + 1 < descriptor.length) descriptor.substring(close + 1) else "V"
    }

    private fun simpleNameFromInternal(internalName: String): String =
        internalName.substringAfterLast('/')

    private fun decapitalize(value: String): String =
        if (value.isEmpty()) value else value.replaceFirstChar { it.lowercase() }

    private fun receiverParameterName(ownerInternalName: String): String =
        if (ownerInternalName.startsWith("java/")) {
            "instance"
        } else {
            decapitalize(simpleNameFromInternal(ownerInternalName))
        }

    private fun operationGenericMatches(expectedDescriptor: String, actualGenericName: String): Boolean {
        val expectedReadable = OperationSignatureRenderer.readableType(expectedDescriptor)
        if (actualGenericName.equals(expectedReadable, ignoreCase = true)) return true
        val boxed = when (expectedDescriptor) {
            "I" -> "Integer"
            "Z" -> "Boolean"
            "J" -> "Long"
            "F" -> "Float"
            "D" -> "Double"
            "B" -> "Byte"
            "C" -> "Character"
            "S" -> "Short"
            "V" -> "Void"
            else -> null
        }
        return boxed != null && actualGenericName.equals(boxed, ignoreCase = true)
    }

    private data class WrappedOperationTarget(
        val owner: String,
        val name: String,
        val parameterDescriptors: List<String>,
        val returnDescriptor: String,
        val isStatic: Boolean,
    )

    companion object {
        private val annotationNamePattern = Regex(
            """@(ModifyExpressionValue|ModifyReturnValue|ModifyReceiver|WrapOperation|WrapWithCondition|WrapMethod)\s*\(""",
        )

        fun findAnnotationSites(source: String): List<MixinExtrasAnnotationSite> {
            val sites = mutableListOf<MixinExtrasAnnotationSite>()
            annotationNamePattern.findAll(source).forEach { match ->
                val annotation = MixinExtrasAnnotation.fromSimpleName(match.groupValues[1]) ?: return@forEach
                val annotationStart = match.range.first
                val parenStart = match.range.last
                val bodyEnd = findMatchingParen(source, parenStart) ?: return@forEach
                val body = source.substring(parenStart + 1, bodyEnd)
                val annotationEnd = bodyEnd + 1
                val methodAttr = Regex("""method\s*=\s*"([^"]+)""").find(body)?.groupValues?.get(1) ?: return@forEach
                val (atValue, atTarget, argsMatch) = extractAtInfo(body)
                val handler = parseHandlerMethod(source, annotationEnd)
                sites += MixinExtrasAnnotationSite(
                    annotation = annotation,
                    methodAttribute = methodAttr,
                    atValue = atValue,
                    atTarget = atTarget ?: argsMatch,
                    annotationRange = offsetRange(source, annotationStart, annotationEnd),
                    handlerMethod = handler,
                )
            }
            return sites
        }

        private fun extractAtInfo(body: String): Triple<String?, String?, String?> {
            val atIndex = body.indexOf("@At")
            if (atIndex < 0) return Triple(null, null, null)
            val paren = body.indexOf('(', atIndex)
            if (paren < 0) return Triple(null, null, null)
            val close = findMatchingParen(body, paren) ?: return Triple(null, null, null)
            val atBody = body.substring(paren + 1, close)
            val atValue = Regex("""value\s*=\s*"([^"]+)""").find(atBody)?.groupValues?.get(1)
                ?: Regex(""""([^"]+)"""").find(atBody)?.groupValues?.get(1)
            val atTarget = Regex("""target\s*=\s*"([^"]+)""").find(atBody)?.groupValues?.get(1)
            val argsMatch = Regex("""args\s*=\s*"([^"]+)""").find(atBody)?.groupValues?.get(1)
            return Triple(atValue, atTarget, argsMatch)
        }

        fun parseHandlerMethod(source: String, startOffset: Int): HandlerMethodDeclaration? {
            var offset = startOffset
            while (offset < source.length) {
                while (offset < source.length && source[offset].isWhitespace()) {
                    offset++
                }
                if (offset >= source.length) return null
                if (source[offset] == '@') {
                    val annotationEnd = skipAnnotation(source, offset) ?: return null
                    offset = annotationEnd
                    continue
                }
                break
            }
            if (offset >= source.length) return null
            val slice = source.substring(offset)
            val match = Regex(
                """^(?:public|protected|private)?\s*(?:static)?\s*([\w<>\[\].]+)\s+([\w$]+)\s*\(([^)]*)\)""",
            ).find(slice) ?: return null
            val returnTypeName = match.groupValues[1]
            val methodName = match.groupValues[2]
            val paramsRaw = match.groupValues[3]
            val parameters = parseParameters(paramsRaw)
            val end = offset + match.range.last + 1
            return HandlerMethodDeclaration(
                methodName = methodName,
                returnTypeName = returnTypeName,
                returnTypeDescriptor = null,
                parameters = parameters,
                range = offsetRange(source, offset, end),
            )
        }

        private fun parseParameters(paramsRaw: String): List<HandlerParameterDeclaration> {
            if (paramsRaw.isBlank()) return emptyList()
            return paramsRaw.split(',').mapNotNull { part ->
                val trimmed = part.trim()
                if (trimmed.isEmpty()) return@mapNotNull null
                val genericOp = Regex("""Operation\s*<\s*([\w<>\[\].]+)\s*>""").find(trimmed)
                val isOperation = genericOp != null || trimmed.contains("Operation<")
                val typeName = genericOp?.let { "Operation<${it.groupValues[1]}>" }
                    ?: trimmed.substringBeforeLast(' ').trim()
                val name = trimmed.substringAfterLast(' ').trim()
                HandlerParameterDeclaration(
                    name = name,
                    typeName = typeName,
                    typeDescriptor = null,
                    isOperation = isOperation,
                    operationGenericName = genericOp?.groupValues?.get(1),
                )
            }
        }

        fun enrichHandlerTypes(handler: HandlerMethodDeclaration, classIndex: ClassIndex): HandlerMethodDeclaration {
            val returnDescriptor = OperationSignatureRenderer.descriptorFromTypeName(handler.returnTypeName, classIndex)
            val params = handler.parameters.map { param ->
                param.copy(
                    typeDescriptor = when {
                        param.isOperation -> "Lcom/llamalad7/mixinextras/injector/wrapoperation/Operation;"
                        else -> OperationSignatureRenderer.descriptorFromTypeName(param.typeName, classIndex)
                    },
                )
            }
            return handler.copy(returnTypeDescriptor = returnDescriptor, parameters = params)
        }

        private fun offsetRange(source: String, start: Int, end: Int): McTextRange {
            return McTextRange(offsetToPosition(source, start), offsetToPosition(source, end))
        }

        private fun offsetToPosition(source: String, offset: Int): McTextPosition {
            var line = 0
            var character = 0
            var i = 0
            while (i < offset && i < source.length) {
                if (source[i] == '\n') {
                    line++
                    character = 0
                } else {
                    character++
                }
                i++
            }
            return McTextPosition(line, character)
        }

        private fun skipAnnotation(source: String, atOffset: Int): Int? {
            if (source.getOrNull(atOffset) != '@') return null
            var end = atOffset + 1
            while (end < source.length && (source[end].isLetterOrDigit() || source[end] == '_')) {
                end++
            }
            if (source.getOrNull(end) != '(') return end
            return findMatchingParen(source, end)?.plus(1) ?: end
        }

        private fun findMatchingParen(source: String, openIndex: Int): Int? {
            if (source.getOrNull(openIndex) != '(') return null
            var depth = 0
            var inString = false
            var i = openIndex
            while (i < source.length) {
                when {
                    inString -> {
                        if (source[i] == '\\') {
                            i += 2
                            continue
                        }
                        if (source[i] == '"') inString = false
                    }
                    source[i] == '"' -> inString = true
                    source[i] == '(' -> depth++
                    source[i] == ')' -> {
                        depth--
                        if (depth == 0) return i
                    }
                }
                i++
            }
            return null
        }
    }
}

data class HandlerValidationIssue(
    val code: String,
    val message: String,
    val range: McTextRange,
)
