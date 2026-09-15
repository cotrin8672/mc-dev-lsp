package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.bytecode.OccurrenceResultClassification
import io.github.mcdev.core.descriptor.DescriptorParseResult
import io.github.mcdev.core.descriptor.DescriptorRenderer
import io.github.mcdev.core.descriptor.JvmType
import io.github.mcdev.core.descriptor.MemberTarget
import io.github.mcdev.core.descriptor.MemberTargetParser
import io.github.mcdev.core.descriptor.MethodSelector
import io.github.mcdev.core.descriptor.Pattern
import io.github.mcdev.core.descriptor.parseMethodDescriptor
import io.github.mcdev.core.descriptor.parseMethodSelector
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mixin.AnnotationContextExtractor
import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.AtTargetKind
import io.github.mcdev.core.mixin.AtTargetOperationKind
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.JavaSourceImports
import io.github.mcdev.core.mixin.JavaTypeDescriptorResolver
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.mixin.MixinAnnotation
import io.github.mcdev.core.mixin.MixinTargetResolver
import io.github.mcdev.core.text.JavaStringContentDecoder
import org.objectweb.asm.ClassReader
import org.objectweb.asm.MethodVisitor
import org.objectweb.asm.Opcodes
import org.objectweb.asm.Type
import org.objectweb.asm.tree.AbstractInsnNode
import org.objectweb.asm.tree.ClassNode
import org.objectweb.asm.tree.VarInsnNode

class HandlerSignatureService(
    private val classIndex: ClassIndex,
    private val bytecodeIndex: BytecodeIndex? = null,
) {
    fun resolveTargetMethod(
        mixinTargets: List<String>,
        methodAttribute: String,
    ): MethodIndexEntry? = when (val resolution = resolveTargetMethodInternal(mixinTargets, methodAttribute)) {
        is TargetMethodResolution.Resolved -> resolution.method
        is TargetMethodResolution.Ambiguous, TargetMethodResolution.NotFound -> null
    }

    fun wrapWithConditionTargetStatus(
        site: MixinExtrasAnnotationSite,
        mixinTargets: List<String>,
        source: String? = null,
        resolvedContext: ExpressionContext? = null,
    ): WrapWithConditionTargetStatus {
        val targetMethod = when (val resolution = resolveTargetMethodInternal(mixinTargets, site.methodAttribute)) {
            is TargetMethodResolution.Resolved -> resolution.method
            is TargetMethodResolution.Ambiguous, TargetMethodResolution.NotFound ->
                return WrapWithConditionTargetStatus.UNRESOLVED
        }
        return resolveWrapWithConditionTarget(
            site = site,
            targetMethod = targetMethod,
            mixinTargets = mixinTargets,
            source = source,
            resolvedContext = resolvedContext,
        ).status
    }

    private fun resolveTargetMethodInternal(
        mixinTargets: List<String>,
        methodAttribute: String,
    ): TargetMethodResolution {
        val openParen = methodAttribute.indexOf('(')
        val name = if (openParen >= 0) methodAttribute.substring(0, openParen) else methodAttribute
        val explicitDescriptor = if (openParen >= 0) methodAttribute.substring(openParen) else null
        val owners = MixinTargetResolver.resolveTargets(mixinTargets, classIndex)
        if (owners.isEmpty()) return TargetMethodResolution.NotFound
        val matchesByOwner = owners.map { owner ->
            classIndex.getMethods(owner).filter { it.name == name }
        }
        if (matchesByOwner.any { it.isEmpty() }) return TargetMethodResolution.NotFound
        val matches = matchesByOwner.flatten()
        if (explicitDescriptor != null) {
            val exactMatchesByOwner = matchesByOwner.map { methods ->
                methods.filter { it.descriptor == explicitDescriptor }
            }
            if (exactMatchesByOwner.any { it.isEmpty() }) return TargetMethodResolution.NotFound
            if (exactMatchesByOwner.any { it.map { method -> method.isStatic }.distinct().size > 1 }) {
                return TargetMethodResolution.Ambiguous(exactMatchesByOwner.flatten())
            }
            val exactMatches = exactMatchesByOwner.map { ownerMatches ->
                ownerMatches.singleOrNull() ?: return TargetMethodResolution.Ambiguous(ownerMatches)
            }
            if (exactMatches.map { it.isStatic }.distinct().size > 1) {
                return TargetMethodResolution.Ambiguous(exactMatches)
            }
            return TargetMethodResolution.Resolved(exactMatches.first())
        }
        val ownerKeys = matchesByOwner.map { methods ->
            methods.map { it.descriptor to it.isStatic }.distinct()
        }
        if (ownerKeys.any { it.size != 1 } || matchesByOwner.any { it.size != 1 }) {
            return TargetMethodResolution.Ambiguous(matches)
        }
        val commonKey = ownerKeys.first().single()
        if (ownerKeys.any { it.single() != commonKey }) return TargetMethodResolution.Ambiguous(matches)
        return TargetMethodResolution.Resolved(matchesByOwner.first().single())
    }

    fun expectedSignature(
        source: String,
        site: MixinExtrasAnnotationSite,
        mixinTargets: List<String>,
        resolvedContext: ExpressionContext? = null,
    ): HandlerSignatureSpec? {
        val targetMethod = when (val resolution = resolveTargetMethodInternal(mixinTargets, site.methodAttribute)) {
            is TargetMethodResolution.Resolved -> resolution.method
            is TargetMethodResolution.Ambiguous, TargetMethodResolution.NotFound -> return null
        }
        // @Inject constructors are only unambiguously safe at RETURN. The
        // WrapOperation NEW path has its own bytecode proof for being after
        // the mandatory super() call; keep that validated path available.
        if (targetMethod.name == "<init>") {
            val injectAtReturn = site.annotation == MixinExtrasAnnotation.INJECT &&
                site.atValue.equals("RETURN", ignoreCase = true)
            val wrapOperationNew = site.annotation == MixinExtrasAnnotation.WRAP_OPERATION &&
                site.atValue.equals("NEW", ignoreCase = true)
            if (!injectAtReturn && !wrapOperationNew) return null
        }
        // ARRAYLENGTH and array element get/set operations need dedicated
        // bytecode-derived parameter semantics. The current target model only
        // represents ordinary field operations, so do not emit a misleading
        // field signature for an array selector.
        if (site.atArgs.any { argument ->
                argument.substringBefore('=').trim().equals("array", ignoreCase = true)
            }) {
            return null
        }
        return when (site.annotation) {
            MixinExtrasAnnotation.INJECT -> expectedInject(source, site, targetMethod)
            MixinExtrasAnnotation.REDIRECT -> expectedRedirect(source, site, targetMethod, mixinTargets)
            MixinExtrasAnnotation.MODIFY_ARG -> expectedModifyArg(source, site, targetMethod, mixinTargets)
            MixinExtrasAnnotation.MODIFY_ARGS -> expectedModifyArgs(site)
            MixinExtrasAnnotation.MODIFY_VARIABLE ->
                expectedModifyVariable(source, site, targetMethod, mixinTargets)
            MixinExtrasAnnotation.MODIFY_EXPRESSION_VALUE ->
                expectedModifyExpressionValue(source, site, targetMethod, mixinTargets, resolvedContext)
            MixinExtrasAnnotation.MODIFY_CONSTANT -> expectedModifyConstant(site)
            MixinExtrasAnnotation.MODIFY_RETURN_VALUE -> expectedModifyReturnValue(targetMethod)
            MixinExtrasAnnotation.MODIFY_RECEIVER ->
                expectedModifyReceiver(source, site, targetMethod, mixinTargets, resolvedContext)
            MixinExtrasAnnotation.WRAP_OPERATION ->
                expectedWrapOperation(source, site, targetMethod, mixinTargets, resolvedContext)
            MixinExtrasAnnotation.WRAP_WITH_CONDITION ->
                expectedWrapWithCondition(source, site, targetMethod, mixinTargets, resolvedContext)
            MixinExtrasAnnotation.WRAP_METHOD -> expectedWrapMethod(targetMethod)
            else -> null
        }
    }

    private fun expectedInject(
        source: String,
        site: MixinExtrasAnnotationSite,
        targetMethod: MethodIndexEntry,
    ): HandlerSignatureSpec? {
        // @Inject without an injection point is incomplete. Waiting until @At is
        // present avoids suggesting a declaration while the annotation is still
        // being typed.
        if (site.atValue.isNullOrBlank()) return null
        val localsCapture = annotationAttributeValue(source, site, "locals")
            ?.substringAfterLast('.')
            ?.trim()
        if (localsCapture != null && localsCapture != "NO_CAPTURE") return null
        val parameters = methodParameterSpecs(targetMethod.descriptor).toMutableList()
        val returnDescriptor = methodReturnDescriptor(targetMethod.descriptor)
        val callbackDescriptor: String
        val callbackReadableType: String
        val callbackGenericDescriptor: String?
        if (returnDescriptor == "V") {
            callbackDescriptor = CALLBACK_INFO_DESCRIPTOR
            callbackReadableType = "CallbackInfo"
            callbackGenericDescriptor = null
        } else {
            callbackDescriptor = CALLBACK_INFO_RETURNABLE_DESCRIPTOR
            callbackGenericDescriptor = boxedDescriptor(returnDescriptor)
            callbackReadableType = buildString {
                append("CallbackInfoReturnable")
                callbackGenericDescriptor?.let {
                    append('<').append(OperationSignatureRenderer.readableType(it)).append('>')
                }
            }
        }
        parameters += HandlerParameterSpec(
            name = "ci",
            typeDescriptor = callbackDescriptor,
            readableType = callbackReadableType,
            genericTypeDescriptor = callbackGenericDescriptor,
        )
        return HandlerSignatureSpec(
            returnTypeDescriptor = "V",
            readableReturnType = "void",
            parameters = parameters,
        )
    }

    private fun expectedRedirect(
        source: String,
        site: MixinExtrasAnnotationSite,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
    ): HandlerSignatureSpec? {
        return when (site.atValue?.uppercase()) {
            "INVOKE" -> {
                val wrapped = parseAtWrapOperationInvokeTarget(
                    atTarget = site.atTarget,
                    targetMethod = targetMethod,
                    mixinTargets = mixinTargets,
                    atOrdinal = site.atOrdinal,
                ) ?: return null
                if (!isKnownMethodTarget(wrapped)) return null
                val parameters = buildList {
                    if (!wrapped.isStatic) {
                        val receiverDescriptor = "L${wrapped.owner};"
                        add(
                            HandlerParameterSpec(
                                name = receiverParameterName(wrapped.owner),
                                typeDescriptor = receiverDescriptor,
                                readableType = OperationSignatureRenderer.readableType(receiverDescriptor),
                            ),
                        )
                    }
                    addAll(methodParameterSpecs(wrapped.parameterDescriptors))
                }
                HandlerSignatureSpec(
                    returnTypeDescriptor = wrapped.returnDescriptor,
                    readableReturnType = OperationSignatureRenderer.readableType(wrapped.returnDescriptor),
                    parameters = parameters,
                )
            }
            "FIELD" -> {
                val field = parseAtFieldTarget(
                    atTarget = site.atTarget,
                    targetMethod = targetMethod,
                    mixinTargets = mixinTargets,
                    allowedOperationKinds = fieldWrapOperationKinds,
                    atOrdinal = site.atOrdinal,
                ) ?: return null
                val parameters = buildList {
                    if (field.operationKind == AtTargetOperationKind.FIELD_GET_INSTANCE ||
                        field.operationKind == AtTargetOperationKind.FIELD_PUT_INSTANCE
                    ) {
                        val receiverDescriptor = "L${field.owner};"
                        add(
                            HandlerParameterSpec(
                                name = receiverParameterName(field.owner),
                                typeDescriptor = receiverDescriptor,
                                readableType = OperationSignatureRenderer.readableType(receiverDescriptor),
                            ),
                        )
                    }
                    if (field.operationKind == AtTargetOperationKind.FIELD_PUT_INSTANCE ||
                        field.operationKind == AtTargetOperationKind.FIELD_PUT_STATIC
                    ) {
                        add(
                            HandlerParameterSpec(
                                name = "value",
                                typeDescriptor = field.fieldDescriptor,
                                readableType = OperationSignatureRenderer.readableType(field.fieldDescriptor),
                            ),
                        )
                    }
                }
                val returnDescriptor = if (
                    field.operationKind == AtTargetOperationKind.FIELD_GET_INSTANCE ||
                    field.operationKind == AtTargetOperationKind.FIELD_GET_STATIC
                ) {
                    field.fieldDescriptor
                } else {
                    "V"
                }
                HandlerSignatureSpec(
                    returnTypeDescriptor = returnDescriptor,
                    readableReturnType = OperationSignatureRenderer.readableType(returnDescriptor),
                    parameters = parameters,
                )
            }
            "NEW" -> {
                val operation = expectedWrapOperationNew(site, targetMethod, mixinTargets) ?: return null
                val original = operation.parameters.lastOrNull()?.takeIf { it.isOperation } ?: return null
                if (original.operationGenericDescriptor == null) return null
                operation.copy(
                    parameters = operation.parameters.dropLast(1),
                    operationCallArgs = emptyList(),
                )
            }
            else -> null
        }
    }

    private fun expectedModifyArg(
        source: String,
        site: MixinExtrasAnnotationSite,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
    ): HandlerSignatureSpec? {
        if (site.atValue?.uppercase() != "INVOKE") return null
        val wrapped = parseAtWrapOperationInvokeTarget(
            atTarget = site.atTarget,
            targetMethod = targetMethod,
            mixinTargets = mixinTargets,
            atOrdinal = site.atOrdinal,
        ) ?: return null
        if (!isKnownMethodTarget(wrapped)) return null
        val index = annotationIntegerAttribute(source, site, "index")
        val argumentIndex = when {
            index != null -> index
            wrapped.parameterDescriptors.size == 1 -> 0
            else -> return null
        }
        val descriptor = wrapped.parameterDescriptors.getOrNull(argumentIndex) ?: return null
        return HandlerSignatureSpec(
            returnTypeDescriptor = descriptor,
            readableReturnType = OperationSignatureRenderer.readableType(descriptor),
            parameters = listOf(
                HandlerParameterSpec(
                    name = "original",
                    typeDescriptor = descriptor,
                    readableType = OperationSignatureRenderer.readableType(descriptor),
                ),
            ),
        )
    }

    private fun expectedModifyArgs(site: MixinExtrasAnnotationSite): HandlerSignatureSpec? {
        if (site.atValue?.uppercase() != "INVOKE") return null
        val argsDescriptor = "Lorg/spongepowered/asm/mixin/injection/invoke/arg/Args;"
        return HandlerSignatureSpec(
            returnTypeDescriptor = "V",
            readableReturnType = "void",
            parameters = listOf(
                HandlerParameterSpec(
                    name = "args",
                    typeDescriptor = argsDescriptor,
                    readableType = "Args",
                ),
            ),
        )
    }

    private fun expectedModifyVariable(
        source: String,
        site: MixinExtrasAnnotationSite,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
    ): HandlerSignatureSpec? {
        val atValue = site.atValue?.uppercase() ?: return null
        if (atValue != "LOAD" && atValue != "STORE") return null
        if (site.atShift !is AtShiftSpec.Before) return null
        val index = annotationIntegerAttribute(source, site, "index")
        val ordinal = annotationIntegerAttribute(source, site, "ordinal")
        if (index != null && index < 0 || ordinal != null && ordinal < 0) return null
        val names = annotationStringArrayAttribute(source, site, "name") +
            annotationStringArrayAttribute(source, site, "names")
        val argsOnly = annotationBooleanAttribute(source, site, "argsOnly") ?: false
        val bytecode = bytecodeIndex ?: return null
        val owners = MixinTargetResolver.resolveTargets(mixinTargets, classIndex)
        if (owners.isEmpty()) return null
        val descriptors = mutableSetOf<String>()
        for (owner in owners) {
            val classBytes = bytecode.getClassBytes(owner) ?: return null
            val localInstructions = localVariableInstructions(
                classBytes = classBytes,
                methodName = targetMethod.name,
                methodDescriptor = targetMethod.descriptor,
                atValue = atValue,
            )
            if (localInstructions.isEmpty()) return null
            val matchingInstructions = index?.let { slot ->
                localInstructions.filter { it.slotIndex == slot }
            } ?: localInstructions
            if (matchingInstructions.isEmpty()) return null
            val selectedInstructions = site.atOrdinal?.let { matchingInstructions.getOrNull(it)?.let(::listOf) }
                ?: matchingInstructions
                .takeIf { site.atOrdinal == null }
                ?: return null
            val captures = LocalCaptureExtractor.extract(
                classBytes = classBytes,
                methodName = targetMethod.name,
                methodDescriptor = targetMethod.descriptor,
                instructionOccurrenceIndices = selectedInstructions.map { it.occurrenceIndex }.toSet(),
            )
            val snapshots = (captures as? LocalCaptureResult.Success)?.snapshots ?: return null
            for (instruction in selectedInstructions) {
                val snapshot = snapshots.singleOrNull {
                    it.instructionOccurrenceIndex == instruction.occurrenceIndex
                } ?: return null
                val candidates = snapshot.candidates.filter { candidate ->
                    (index == null || candidate.slotIndex == index) &&
                        (!argsOnly || candidate.isArgument) &&
                        (names.isEmpty() || candidate.name in names) &&
                        (ordinal == null || candidate.ordinal == ordinal)
                }
                val candidate = candidates.singleOrNull() ?: return null
                descriptors += candidate.descriptor
            }
        }
        val descriptor = descriptors.singleOrNull() ?: return null
        return HandlerSignatureSpec(
            returnTypeDescriptor = descriptor,
            readableReturnType = OperationSignatureRenderer.readableType(descriptor),
            parameters = listOf(
                HandlerParameterSpec(
                    name = "original",
                    typeDescriptor = descriptor,
                    readableType = OperationSignatureRenderer.readableType(descriptor),
                ),
            ),
        )
    }

    private fun isKnownMethodTarget(target: WrappedOperationTarget): Boolean =
        classIndex.getMethods(target.owner).any {
            it.descriptor == "(${target.parameterDescriptors.joinToString("")})${target.returnDescriptor}"
        }

    private fun methodParameterSpecs(descriptor: String): List<HandlerParameterSpec> =
        methodParameterSpecs(methodParameterDescriptors(descriptor))

    private fun methodParameterSpecs(descriptors: List<String>): List<HandlerParameterSpec> =
        descriptors.mapIndexed { index, parameterDescriptor ->
            HandlerParameterSpec(
                name = "arg$index",
                typeDescriptor = parameterDescriptor,
                readableType = OperationSignatureRenderer.readableType(parameterDescriptor),
            )
        }

    private fun boxedDescriptor(descriptor: String): String = when (descriptor) {
        "Z" -> "Ljava/lang/Boolean;"
        "B" -> "Ljava/lang/Byte;"
        "C" -> "Ljava/lang/Character;"
        "S" -> "Ljava/lang/Short;"
        "I" -> "Ljava/lang/Integer;"
        "J" -> "Ljava/lang/Long;"
        "F" -> "Ljava/lang/Float;"
        "D" -> "Ljava/lang/Double;"
        else -> descriptor
    }

    private fun annotationIntegerAttribute(
        source: String,
        site: MixinExtrasAnnotationSite,
        name: String,
    ): Int? = annotationAttributeValue(source, site, name)?.trim()?.toIntOrNull()

    private fun annotationBooleanAttribute(
        source: String,
        site: MixinExtrasAnnotationSite,
        name: String,
    ): Boolean? = annotationAttributeValue(source, site, name)?.trim()?.let { value ->
        when {
            value.equals("true", ignoreCase = true) -> true
            value.equals("false", ignoreCase = true) -> false
            else -> null
        }
    }

    private fun annotationStringArrayAttribute(
        source: String,
        site: MixinExtrasAnnotationSite,
        name: String,
    ): List<String> {
        val value = annotationAttributeValue(source, site, name)?.trim() ?: return emptyList()
        return Regex("\\\"([^\\\"]*)\\\"")
            .findAll(value)
            .map { it.groupValues[1] }
            .toList()
    }

    private fun annotationAttributeValue(
        source: String,
        site: MixinExtrasAnnotationSite,
        name: String,
    ): String? {
        val start = positionToOffset(source, site.annotationRange.start)
        val end = positionToOffset(source, site.annotationRange.end).coerceAtMost(source.length)
        if (start < 0 || start >= end) return null
        val annotation = source.substring(start, end)
        val open = annotation.indexOf('(')
        val close = annotation.lastIndexOf(')')
        if (open < 0 || close <= open) return null
        for (member in splitTopLevelCommas(annotation.substring(open + 1, close))) {
            val trimmed = member.text.trim()
            val equals = findTopLevelEquals(trimmed)
            if (equals < 0) continue
            if (trimmed.substring(0, equals).trim() == name) {
                return trimmed.substring(equals + 1).trim()
            }
        }
        return null
    }

    private data class LocalVariableInstruction(
        val occurrenceIndex: Int,
        val slotIndex: Int,
    )

    private fun localVariableInstructions(
        classBytes: ByteArray,
        methodName: String,
        methodDescriptor: String,
        atValue: String,
    ): List<LocalVariableInstruction> {
        val classNode = ClassNode()
        try {
            ClassReader(classBytes).accept(classNode, ClassReader.SKIP_FRAMES)
        } catch (_: RuntimeException) {
            return emptyList()
        }
        val method = classNode.methods.firstOrNull { it.name == methodName && it.desc == methodDescriptor }
            ?: return emptyList()
        val result = mutableListOf<LocalVariableInstruction>()
        var occurrenceIndex = 0
        var instruction: AbstractInsnNode? = method.instructions.first
        while (instruction != null) {
            val opcode = instruction.opcode
            if (opcode >= 0) {
                val variable = instruction as? VarInsnNode
                if (variable != null && isLocalVariableOpcode(opcode, atValue)) {
                    result += LocalVariableInstruction(occurrenceIndex, variable.`var`)
                }
                occurrenceIndex++
            }
            instruction = instruction.next
        }
        return result
    }

    private fun isLocalVariableOpcode(opcode: Int, atValue: String): Boolean = when (atValue) {
        "LOAD" -> opcode in Opcodes.ILOAD..Opcodes.ALOAD
        "STORE" -> opcode in Opcodes.ISTORE..Opcodes.ASTORE
        else -> false
    }

    private fun positionToOffset(source: String, position: McTextPosition): Int {
        if (position.line <= 0) return position.character.coerceAtLeast(0)
        var offset = 0
        repeat(position.line) {
            val newline = source.indexOf('\n', offset)
            if (newline < 0) return source.length
            offset = newline + 1
        }
        return (offset + position.character).coerceAtMost(source.length)
    }

    fun generateHandlerStub(
        source: String,
        site: MixinExtrasAnnotationSite,
        mixinTargets: List<String>,
        methodName: String = "mcdevHandler",
        indent: String = "    ",
        resolvedContext: ExpressionContext? = null,
        signatureSpec: HandlerSignatureSpec? = null,
    ): String? {
        val targetMethod = when (val resolution = resolveTargetMethodInternal(mixinTargets, site.methodAttribute)) {
            is TargetMethodResolution.Resolved -> resolution.method
            is TargetMethodResolution.Ambiguous, TargetMethodResolution.NotFound -> return null
        }
        val spec = signatureSpec ?: expectedSignature(source, site, mixinTargets, resolvedContext) ?: return null
        val params = spec.parameters.joinToString(", ") { "${it.readableType} ${it.name}" }
        val callArgs = spec.operationCallArgs.joinToString(", ")
        val body = when (site.annotation) {
            MixinExtrasAnnotation.WRAP_WITH_CONDITION -> "${indent}return true;\n"
            MixinExtrasAnnotation.WRAP_OPERATION, MixinExtrasAnnotation.WRAP_METHOD -> {
                val op = spec.parameters.lastOrNull()?.name ?: "original"
                val ret = if (spec.returnTypeDescriptor == "V") "" else "return "
                "${indent}${ret}$op.call($callArgs);\n"
            }
            MixinExtrasAnnotation.REDIRECT -> "${indent}throw new UnsupportedOperationException();\n"
            MixinExtrasAnnotation.MODIFY_ARG,
            MixinExtrasAnnotation.MODIFY_VARIABLE,
            -> {
                if (spec.returnTypeDescriptor == "V" || spec.parameters.isEmpty()) {
                    "${indent}// TODO\n"
                } else {
                    "${indent}return ${spec.parameters.first().name};\n"
                }
            }
            MixinExtrasAnnotation.MODIFY_EXPRESSION_VALUE,
            MixinExtrasAnnotation.MODIFY_CONSTANT,
            MixinExtrasAnnotation.MODIFY_RETURN_VALUE,
            MixinExtrasAnnotation.MODIFY_RECEIVER,
            -> {
                if (spec.parameters.isEmpty()) {
                    "${indent}// TODO\n"
                } else {
                    val original = spec.parameters.first().name
                    "${indent}return $original;\n"
                }
            }
            else -> "${indent}// TODO\n"
        }
        val staticModifier = if (targetMethod.isStatic) "static " else ""
        return "${indent}${staticModifier}${spec.readableReturnType} $methodName($params) {\n$body${indent}}\n"
    }

    fun validateCommonSugarConstraints(
        handler: HandlerMethodDeclaration,
        targetMethod: MethodIndexEntry? = null,
        injectorAnnotation: MixinExtrasAnnotation? = null,
    ): List<HandlerValidationIssue> {
        val issues = mutableListOf<HandlerValidationIssue>()
        issues += validateSugarParametersAreTrailing(handler)
        issues += validateShareParameterTypes(handler)
        if (targetMethod != null) {
            issues += validateCancellableParameterTypes(handler, targetMethod)
            if (injectorAnnotation == MixinExtrasAnnotation.INJECT) {
                issues += validateInjectCallbackParameter(handler, targetMethod)
            }
        }
        return issues
    }

    fun returnTypeMatches(
        expected: HandlerSignatureSpec,
        handler: HandlerMethodDeclaration,
    ): Boolean {
        val actual = handler.returnTypeDescriptor ?: return false
        return typeMatches(
            actual,
            expected.returnTypeDescriptor,
            handler.hasMethodLevelCoerce,
            expected.acceptedReturnTypeDescriptors,
        )
    }

    fun parameterTypeMatches(
        expected: HandlerParameterSpec,
        actual: HandlerParameterDeclaration,
    ): Boolean = if (expected.isOperation) {
        actual.isOperation &&
            expected.operationGenericDescriptor != null &&
            actual.operationGenericName?.let {
                operationGenericMatches(expected, it)
            } == true
    } else {
        typeMatches(
            actual.typeDescriptor,
            expected.typeDescriptor,
            actual.hasCoerce,
            expected.acceptedTypeDescriptors,
        )
    }

    /**
     * Keep a valid existing int-like choice when a signature fix has to rewrite
     * another part of the handler declaration. The official injector uses one
     * replacement type for every int-like position, so rewriting a boolean
     * handler back to the default int would make its body invalid.
     */
    fun specializeIntLikeSignature(
        expected: HandlerSignatureSpec,
        handler: HandlerMethodDeclaration,
    ): HandlerSignatureSpec {
        val actualBase = handler.parameters.filterNot { it.isSugar }
        val ordinaryChoices = buildList {
            if (expected.acceptedReturnTypeDescriptors.isNotEmpty()) {
                handler.returnTypeDescriptor
                    ?.takeIf { it in expected.acceptedReturnTypeDescriptors }
                    ?.let(::add)
            }
            expected.parameters.forEachIndexed { index, parameter ->
                if (parameter.acceptedTypeDescriptors.isNotEmpty()) {
                    actualBase.getOrNull(index)
                        ?.typeDescriptor
                        ?.takeIf { it in parameter.acceptedTypeDescriptors }
                        ?.let(::add)
                }
            }
        }
        val choice = ordinaryChoices.distinct().singleOrNull()
            ?: buildList {
                expected.parameters.forEachIndexed { index, parameter ->
                    if (parameter.acceptedOperationGenericDescriptors.isNotEmpty()) {
                        actualBase.getOrNull(index)
                            ?.takeIf { it.isOperation }
                            ?.operationGenericName
                            ?.let { operationGenericDescriptor(parameter, it) }
                            ?.let(::add)
                    }
                }
            }.distinct().singleOrNull()
            ?: return expected

        val returnDescriptor = if (expected.acceptedReturnTypeDescriptors.isNotEmpty()) {
            choice
        } else {
            expected.returnTypeDescriptor
        }
        val parameters = expected.parameters.map { parameter ->
            when {
                parameter.acceptedTypeDescriptors.isNotEmpty() -> parameter.copy(
                    typeDescriptor = choice,
                    readableType = OperationSignatureRenderer.readableType(choice),
                )
                parameter.acceptedOperationGenericDescriptors.isNotEmpty() -> parameter.copy(
                    operationGenericDescriptor = choice,
                    readableType = OperationSignatureRenderer.renderOperationType(choice),
                )
                else -> parameter
            }
        }
        return expected.copy(
            returnTypeDescriptor = returnDescriptor,
            readableReturnType = OperationSignatureRenderer.readableType(returnDescriptor),
            parameters = parameters,
        )
    }

    fun validateHandler(
        source: String,
        site: MixinExtrasAnnotationSite,
        mixinTargets: List<String>,
        handler: HandlerMethodDeclaration,
        resolvedContext: ExpressionContext? = null,
    ): List<HandlerValidationIssue> {
        val targetResolution = resolveTargetMethodInternal(mixinTargets, site.methodAttribute)
        val targetMethod = (targetResolution as? TargetMethodResolution.Resolved)?.method
        val commonSugarIssues = validateCommonSugarConstraints(handler, targetMethod, site.annotation)
        val wrapMethodUnsupportedSugarIssues =
            if (site.annotation == MixinExtrasAnnotation.WRAP_METHOD) {
                handler.parameters.mapNotNull { param ->
                    if (!param.isSugar) return@mapNotNull null
                    val sugarName = when (param.sugarSpec) {
                        is HandlerParameterSugarSpec.Local -> "@Local"
                        is HandlerParameterSugarSpec.Cancellable -> "@Cancellable"
                        is HandlerParameterSugarSpec.Share, null -> return@mapNotNull null
                    }
                    HandlerValidationIssue(
                        code = MixinExtrasDiagnosticCodes.UNSUPPORTED_SUGAR_PARAMETER,
                        message = "WrapMethod does not support $sugarName; only @Share is supported",
                        range = param.sugarAnnotationRange ?: param.range ?: handler.range,
                    )
                }
            } else {
                emptyList()
        }
        val preResolutionIssues = commonSugarIssues + wrapMethodUnsupportedSugarIssues
        when (targetResolution) {
            is TargetMethodResolution.Ambiguous -> {
                val name = site.methodAttribute.substringBefore('(')
                return preResolutionIssues + listOf(
                    HandlerValidationIssue(
                        code = MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH,
                        message = "Ambiguous target method '$name'; an explicit method descriptor is required",
                        range = handler.range,
                    ),
                )
            }
            else -> Unit
        }
        if (site.annotation == MixinExtrasAnnotation.WRAP_METHOD && targetMethod != null) {
            if (targetMethod.name == "<init>" || targetMethod.name == "<clinit>") {
                return preResolutionIssues + HandlerValidationIssue(
                    code = MixinExtrasDiagnosticCodes.WRAP_METHOD_INVALID_TARGET,
                    message = "WrapMethod cannot target an initializer method",
                    range = handler.range,
                )
            }
            if (handler.isStatic != targetMethod.isStatic) {
                val expectedModifier = if (targetMethod.isStatic) "static" else "instance"
                return preResolutionIssues + HandlerValidationIssue(
                    code = MixinExtrasDiagnosticCodes.WRAP_METHOD_STATIC_MISMATCH,
                    message = "WrapMethod handler must be $expectedModifier to match the target method",
                    range = handler.range,
                )
            }
        }
        if (site.annotation == MixinExtrasAnnotation.MODIFY_RETURN_VALUE) {
            if (targetMethod != null && methodReturnDescriptor(targetMethod.descriptor) == "V") {
                return preResolutionIssues + listOf(
                    HandlerValidationIssue(
                        code = MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH,
                        message = "ModifyReturnValue cannot target a void method",
                        range = handler.range,
                    ),
                )
            }
        }
        val expected = expectedSignature(source, site, mixinTargets, resolvedContext)
            ?: return preResolutionIssues
        val issues = mutableListOf<HandlerValidationIssue>()
        issues += preResolutionIssues
        if (!intLikeTypesMatch(expected, handler)) {
            issues += HandlerValidationIssue(
                code = MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH,
                message = "All int-like handler types must use one type (boolean, byte, char, short, or int)",
                range = handler.range,
            )
        }
        if (!returnTypeMatches(expected, handler)) {
            issues += HandlerValidationIssue(
                code = MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE,
                message = "MixinExtras handler return type should be ${expected.readableReturnType}",
                range = handler.range,
            )
        }
        val operationParams = handler.parameters.filter { it.isOperation }
        when (site.annotation) {
            MixinExtrasAnnotation.WRAP_WITH_CONDITION -> {
                if (operationParams.isNotEmpty()) {
                    issues += HandlerValidationIssue(
                        code = MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH,
                        message = "WrapWithCondition handler must not declare an Operation parameter",
                        range = handler.range,
                    )
                    return issues
                }
            }
            MixinExtrasAnnotation.WRAP_OPERATION,
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
                if (expectedOp != null &&
                    (actualOp.operationGenericName == null ||
                        !operationGenericMatches(expectedOp, actualOp.operationGenericName))
                ) {
                    issues += HandlerValidationIssue(
                        code = MixinExtrasDiagnosticCodes.WRONG_OPERATION_GENERIC,
                        message = "Operation generic should be ${OperationSignatureRenderer.readableType(expectedOp.operationGenericDescriptor!!)}",
                        range = handler.range,
                    )
                }
                val nonSugarParams = handler.parameters.filter { !it.isSugar }
                when (site.annotation) {
                    MixinExtrasAnnotation.WRAP_METHOD -> {
                        if (actualOp != nonSugarParams.lastOrNull()) {
                            issues += HandlerValidationIssue(
                                code = MixinExtrasDiagnosticCodes.MISSING_OPERATION_PARAMETER,
                                message = "Operation<T> must be the last handler parameter",
                                range = handler.range,
                            )
                        }
                    }
                    MixinExtrasAnnotation.WRAP_OPERATION -> {
                        val requiredCount = expected.parameters.size
                        if (requiredCount > nonSugarParams.size ||
                            !nonSugarParams[requiredCount - 1].isOperation
                        ) {
                            issues += HandlerValidationIssue(
                                code = MixinExtrasDiagnosticCodes.MISSING_OPERATION_PARAMETER,
                                message = "Operation<T> must be the last required wrapped-operation parameter",
                                range = handler.range,
                            )
                        }
                    }
                    else -> Unit
                }
            }
            MixinExtrasAnnotation.MODIFY_EXPRESSION_VALUE,
            MixinExtrasAnnotation.MODIFY_RETURN_VALUE,
            -> {
                val expectedOriginal = expected.parameters.firstOrNull()
                val actualOriginal = handler.parameters.firstOrNull { !it.isSugar }
                if (expectedOriginal != null &&
                        (actualOriginal == null ||
                        !parameterTypeMatches(expectedOriginal, actualOriginal))
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
        if (issues.isEmpty() && !parametersMatch(site.annotation, expected, handler)) {
            issues += HandlerValidationIssue(
                code = MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH,
                message = "MixinExtras handler signature mismatch",
                range = handler.range,
            )
        }
        return issues
    }

    private fun parametersMatch(
        annotation: MixinExtrasAnnotation,
        expected: HandlerSignatureSpec,
        handler: HandlerMethodDeclaration,
    ): Boolean = when (annotation) {
        MixinExtrasAnnotation.MODIFY_RETURN_VALUE,
        MixinExtrasAnnotation.MODIFY_EXPRESSION_VALUE,
        -> optionalCapturedPrefixParametersMatch(expected, handler)
        MixinExtrasAnnotation.MODIFY_RECEIVER,
        MixinExtrasAnnotation.WRAP_OPERATION,
        MixinExtrasAnnotation.WRAP_WITH_CONDITION,
        -> requiredPlusCapturedPrefixParametersMatch(expected, handler)
        else -> strictParametersMatch(expected, handler)
    }

    private fun optionalCapturedPrefixParametersMatch(
        expected: HandlerSignatureSpec,
        handler: HandlerMethodDeclaration,
    ): Boolean {
        if (!sugarParametersAreTrailing(handler.parameters)) return false
        val actualBase = handler.parameters.filter { !it.isSugar }
        if (actualBase.isEmpty()) return false
        val expectedOriginal = expected.parameters.firstOrNull() ?: return false
        val actualOriginal = actualBase.first()
        if (!parameterTypeMatches(expectedOriginal, actualOriginal)) {
            return false
        }
        return capturedPrefixMatches(
            actualBase.drop(1),
            expected.optionalCapturedTargetParameters,
        )
    }

    private fun requiredPlusCapturedPrefixParametersMatch(
        expected: HandlerSignatureSpec,
        handler: HandlerMethodDeclaration,
    ): Boolean {
        if (!sugarParametersAreTrailing(handler.parameters)) return false
        val actualBase = handler.parameters.filter { !it.isSugar }
        val expectedRequired = expected.parameters
        if (actualBase.size < expectedRequired.size) return false
        if (!requiredParametersMatch(expectedRequired, actualBase.take(expectedRequired.size))) return false
        return capturedPrefixMatches(
            actualBase.drop(expectedRequired.size),
            expected.optionalCapturedTargetParameters,
        )
    }

    private fun strictParametersMatch(expected: HandlerSignatureSpec, handler: HandlerMethodDeclaration): Boolean {
        if (!sugarParametersAreTrailing(handler.parameters)) return false
        val actualBase = handler.parameters.filter { !it.isSugar }
        return requiredParametersMatch(expected.parameters, actualBase)
    }

    private fun requiredParametersMatch(
        expectedRequired: List<HandlerParameterSpec>,
        actualRequired: List<HandlerParameterDeclaration>,
    ): Boolean {
        if (actualRequired.size != expectedRequired.size) return false
        return actualRequired.zip(expectedRequired).all { (actual, exp) ->
            parameterTypeMatches(exp, actual)
        }
    }

    private fun capturedPrefixMatches(
        capturedParams: List<HandlerParameterDeclaration>,
        availableCaptured: List<HandlerParameterSpec>,
    ): Boolean {
        if (capturedParams.size > availableCaptured.size) return false
        return capturedParams.indices.all { index ->
            val actual = capturedParams[index]
            val exp = availableCaptured[index]
            typeMatches(actual.typeDescriptor, exp.typeDescriptor, actual.hasCoerce)
        }
    }

    private fun typeMatches(
        actualDescriptor: String?,
        expectedDescriptor: String,
        coerce: Boolean,
        acceptedDescriptors: Set<String> = emptySet(),
    ): Boolean {
        val actual = actualDescriptor ?: return false
        if (actual == expectedDescriptor || actual in acceptedDescriptors) return true
        return coerce && coerceCompatible(actual, expectedDescriptor)
    }

    private fun intLikeTypesMatch(
        expected: HandlerSignatureSpec,
        handler: HandlerMethodDeclaration,
    ): Boolean {
        val expectedBase = handler.parameters.filter { !it.isSugar }
        val actualTypes = mutableListOf<String>()

        if (expected.acceptedReturnTypeDescriptors.isNotEmpty()) {
            val actual = handler.returnTypeDescriptor ?: return false
            if (actual !in expected.acceptedReturnTypeDescriptors) return false
            actualTypes += actual
        }

        expected.parameters.forEachIndexed { index, parameter ->
            if (parameter.acceptedTypeDescriptors.isNotEmpty()) {
                val actual = expectedBase.getOrNull(index)?.typeDescriptor ?: return false
                if (actual !in parameter.acceptedTypeDescriptors) return false
                actualTypes += actual
            }
            if (parameter.acceptedOperationGenericDescriptors.isNotEmpty()) {
                val actual = expectedBase.getOrNull(index)
                    ?.takeIf { it.isOperation }
                    ?.operationGenericName
                    ?: return false
                val descriptor = operationGenericDescriptor(parameter, actual) ?: return false
                actualTypes += descriptor
            }
        }

        return actualTypes.distinct().size <= 1
    }

    /**
     * Checks the official @Coerce direction: the handler declaration is the
     * source type and the injector contract is the target type. References
     * are accepted only when the contract type is a subtype/implementation of
     * the handler type.
     */
    private fun coerceCompatible(fromDescriptor: String, expectedDescriptor: String): Boolean {
        val fromType = runCatching { Type.getType(fromDescriptor) }.getOrNull() ?: return false
        val expectedType = runCatching { Type.getType(expectedDescriptor) }.getOrNull() ?: return false
        if (fromType.sort == Type.METHOD || expectedType.sort == Type.METHOD) return false

        if (fromType.sort == Type.ARRAY || expectedType.sort == Type.ARRAY) {
            if (fromType.sort != Type.ARRAY || expectedType.sort != Type.ARRAY) return false
            if (fromType.dimensions != expectedType.dimensions) return false
            val fromElement = fromType.elementType
            val expectedElement = expectedType.elementType
            if (fromElement.sort <= Type.DOUBLE || expectedElement.sort <= Type.DOUBLE) {
                return false
            }
            return coerceReferenceCompatible(fromElement.internalName, expectedElement.internalName)
        }

        if (fromType.sort <= Type.DOUBLE || expectedType.sort <= Type.DOUBLE) {
            return fromDescriptor in setOf("B", "S", "C", "Z") && expectedDescriptor == "I"
        }
        if (fromType.sort != Type.OBJECT || expectedType.sort != Type.OBJECT) return false
        return coerceReferenceCompatible(fromType.internalName, expectedType.internalName)
    }

    private fun coerceReferenceCompatible(fromInternalName: String, expectedInternalName: String): Boolean {
        if (fromInternalName == expectedInternalName) return true
        if (fromInternalName == "java/lang/Object") return isKnownReference(expectedInternalName)
        val fromDescriptor = "L$fromInternalName;"
        val expectedDescriptor = "L$expectedInternalName;"
        bytecodeIndex?.resolveCommonSuperClass(fromDescriptor, expectedDescriptor)?.let {
            if (it == fromDescriptor) return true
        }
        if (!isKnownReference(fromInternalName) || !isKnownReference(expectedInternalName)) return false
        return runCatching {
            val from = Class.forName(fromInternalName.replace('/', '.'), false, javaClass.classLoader)
            val expected = Class.forName(expectedInternalName.replace('/', '.'), false, javaClass.classLoader)
            from.isAssignableFrom(expected)
        }.getOrDefault(false)
    }

    private fun isKnownReference(internalName: String): Boolean =
        internalName == "java/lang/Object" ||
            classIndex.findClass(internalName) != null ||
            bytecodeIndex?.getClassBytes(internalName) != null ||
            runCatching {
                Class.forName(internalName.replace('/', '.'), false, javaClass.classLoader)
            }.isSuccess

    private fun sugarParametersAreTrailing(parameters: List<HandlerParameterDeclaration>): Boolean {
        var seenSugar = false
        for (param in parameters) {
            if (param.isSugar) {
                seenSugar = true
            } else if (seenSugar) {
                return false
            }
        }
        return true
    }

    private fun validateSugarParametersAreTrailing(handler: HandlerMethodDeclaration): List<HandlerValidationIssue> {
        val lastNonSugarIndex = handler.parameters.indexOfLast { !it.isSugar }
        if (lastNonSugarIndex < 0) return emptyList()
        val offending = handler.parameters.withIndex().firstOrNull { (index, param) ->
            param.isSugar && index < lastNonSugarIndex
        } ?: return emptyList()
        return listOf(
            HandlerValidationIssue(
                code = MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH,
                message = "Sugar parameters must be trailing",
                range = offending.value.sugarAnnotationRange ?: offending.value.range ?: handler.range,
            ),
        )
    }

    private fun validateShareParameterTypes(handler: HandlerMethodDeclaration): List<HandlerValidationIssue> =
        handler.parameters.mapNotNull { param ->
            if (!param.isSugar || param.sugarSpec !is HandlerParameterSugarSpec.Share) return@mapNotNull null
            val descriptor = param.typeDescriptor ?: return@mapNotNull null
            if (shareRefValueDescriptor(descriptor) != null) return@mapNotNull null
            HandlerValidationIssue(
                code = MixinExtrasDiagnosticCodes.INVALID_SHARE_PARAMETER_TYPE,
                message = "Share parameter type must be in com.llamalad7.mixinextras.sugar.ref",
                range = handler.range,
            )
        }

    private fun validateCancellableParameterTypes(
        handler: HandlerMethodDeclaration,
        targetMethod: MethodIndexEntry,
    ): List<HandlerValidationIssue> {
        val expectedDescriptor = if (methodReturnDescriptor(targetMethod.descriptor) == "V") {
            CALLBACK_INFO_DESCRIPTOR
        } else {
            CALLBACK_INFO_RETURNABLE_DESCRIPTOR
        }
        val expectedReadableType = OperationSignatureRenderer.readableType(expectedDescriptor)
        return handler.parameters.mapNotNull { param ->
            if (!param.isSugar || param.sugarSpec !is HandlerParameterSugarSpec.Cancellable) return@mapNotNull null
            val descriptor = param.typeDescriptor ?: return@mapNotNull null
            if (descriptor == expectedDescriptor) return@mapNotNull null
            HandlerValidationIssue(
                code = MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH,
                message = "Cancellable parameter type should be $expectedReadableType",
                range = param.sugarAnnotationRange ?: handler.range,
            )
        }
    }

    private fun validateInjectCallbackParameter(
        handler: HandlerMethodDeclaration,
        targetMethod: MethodIndexEntry,
    ): List<HandlerValidationIssue> {
        if (handler.parameters.none { it.sugarSpec is HandlerParameterSugarSpec.Cancellable }) return emptyList()
        val expectedDescriptor = if (methodReturnDescriptor(targetMethod.descriptor) == "V") {
            CALLBACK_INFO_DESCRIPTOR
        } else {
            CALLBACK_INFO_RETURNABLE_DESCRIPTOR
        }
        val ordinaryParameters = handler.parameters.filterNot { it.isSugar }
        val targetParameterCount = methodParameterDescriptors(targetMethod.descriptor).size
        val callback = ordinaryParameters.getOrNull(targetParameterCount)
            ?.takeIf { it.typeDescriptor == expectedDescriptor }
            ?: ordinaryParameters.singleOrNull()?.takeIf { it.typeDescriptor == expectedDescriptor }
        if (callback != null) return emptyList()
        val sugar = handler.parameters.firstOrNull { it.sugarSpec is HandlerParameterSugarSpec.Cancellable }
        return listOf(
            HandlerValidationIssue(
                code = MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH,
                message = "Inject handler must include ${OperationSignatureRenderer.readableType(expectedDescriptor)} as its ordinary callback parameter when using @Cancellable",
                range = sugar?.sugarAnnotationRange ?: sugar?.range ?: handler.range,
            ),
        )
    }

    private fun internalNameFromDescriptor(descriptor: String): String? {
        if (descriptor.startsWith("L") && descriptor.endsWith(";")) {
            return descriptor.substring(1, descriptor.length - 1)
        }
        return null
    }

    private fun expectedModifyExpressionValue(
        source: String,
        site: MixinExtrasAnnotationSite,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
        resolvedContext: ExpressionContext? = null,
    ): HandlerSignatureSpec? {
        val expressionType = inferExpressionValueInfo(source, site, targetMethod, mixinTargets, resolvedContext)
            ?: return null
        val acceptedIntLikeTypes = if (expressionType.intLike) INT_LIKE_DESCRIPTORS else emptySet()
        return HandlerSignatureSpec(
            returnTypeDescriptor = expressionType.descriptor,
            readableReturnType = OperationSignatureRenderer.readableType(expressionType.descriptor),
            parameters = listOf(
                HandlerParameterSpec(
                    name = "original",
                    typeDescriptor = expressionType.descriptor,
                    readableType = OperationSignatureRenderer.readableType(expressionType.descriptor),
                    acceptedTypeDescriptors = acceptedIntLikeTypes,
                ),
            ),
            optionalCapturedTargetParameters = capturedTargetParametersFrom(targetMethod),
            acceptedReturnTypeDescriptors = acceptedIntLikeTypes,
        )
    }

    private fun inferExpressionValueInfo(
        source: String,
        site: MixinExtrasAnnotationSite,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
        resolvedContext: ExpressionContext?,
    ): ExpressionValueTypeInfo? {
        if (site.atValue.equals("MIXINEXTRAS:EXPRESSION", ignoreCase = true)) {
            val index = bytecodeIndex ?: return null
            return ExpressionContextResolver.inferExpressionValueTypeInfo(
                source = source,
                site = site,
                targetMethod = targetMethod,
                mixinTargets = mixinTargets,
                bytecodeIndex = index,
                classIndex = classIndex,
                resolvedContext = resolvedContext,
            )
        }
        return inferExpressionValueType(source, site, targetMethod, mixinTargets, resolvedContext)
            ?.let(::ExpressionValueTypeInfo)
    }

    private fun expectedModifyConstant(site: MixinExtrasAnnotationSite): HandlerSignatureSpec? {
        if (site.expandZeroConditions.isNotEmpty() && site.atArgs.any {
                it.substringBefore('=') != "intValue"
            }
        ) {
            return null
        }
        val valueDescriptor = if (site.atArgs.isEmpty()) {
            if (site.expandZeroConditions.isNotEmpty()) "I" else return null
        } else {
            ConstantAtArgsParser.parse(site.atArgs) ?: return null
        }
        return HandlerSignatureSpec(
            returnTypeDescriptor = valueDescriptor,
            readableReturnType = OperationSignatureRenderer.readableType(valueDescriptor),
            parameters = listOf(
                HandlerParameterSpec(
                    name = "original",
                    typeDescriptor = valueDescriptor,
                    readableType = OperationSignatureRenderer.readableType(valueDescriptor),
                ),
            ),
        )
    }

    private fun expectedModifyReturnValue(targetMethod: MethodIndexEntry): HandlerSignatureSpec? {
        val returnType = methodReturnDescriptor(targetMethod.descriptor)
        if (returnType == "V") return null
        return HandlerSignatureSpec(
            returnTypeDescriptor = returnType,
            readableReturnType = OperationSignatureRenderer.readableType(returnType),
            parameters = listOf(
                HandlerParameterSpec(
                    name = "original",
                    typeDescriptor = returnType,
                    readableType = OperationSignatureRenderer.readableType(returnType),
                ),
            ),
            optionalCapturedTargetParameters = capturedTargetParametersFrom(targetMethod),
        )
    }

    private fun expectedModifyReceiver(
        source: String,
        site: MixinExtrasAnnotationSite,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
        resolvedContext: ExpressionContext? = null,
    ): HandlerSignatureSpec? {
        val wrapped = if (site.atValue.equals("MIXINEXTRAS:EXPRESSION", ignoreCase = true)) {
            val index = bytecodeIndex ?: return null
            val layout = ExpressionContextResolver.inferModifyReceiverLayout(
                source = source,
                site = site,
                targetMethod = targetMethod,
                mixinTargets = mixinTargets,
                bytecodeIndex = index,
                classIndex = classIndex,
                resolvedContext = resolvedContext,
            ) ?: return null
            WrappedInstructionTarget(
                owner = layout.receiverOwnerInternalName,
                parameterDescriptors = layout.parameterDescriptors,
                isStatic = false,
            )
        } else {
            parseAtModifyReceiverTarget(site.atTarget, targetMethod, mixinTargets) ?: return null
        }
        val receiverType = "L${wrapped.owner};"
        val params = mutableListOf<HandlerParameterSpec>()
        params += HandlerParameterSpec(
            name = receiverParameterName(wrapped.owner),
            typeDescriptor = receiverType,
            readableType = OperationSignatureRenderer.readableType(receiverType),
        )
        wrapped.parameterDescriptors.forEachIndexed { index, descriptor ->
            params += HandlerParameterSpec(
                name = "arg$index",
                typeDescriptor = descriptor,
                readableType = OperationSignatureRenderer.readableType(descriptor),
            )
        }
        return HandlerSignatureSpec(
            returnTypeDescriptor = receiverType,
            readableReturnType = OperationSignatureRenderer.readableType(receiverType),
            parameters = params,
            optionalCapturedTargetParameters = capturedTargetParametersFrom(targetMethod),
        )
    }

    private fun expectedWrapOperation(
        source: String,
        site: MixinExtrasAnnotationSite,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
        resolvedContext: ExpressionContext? = null,
    ): HandlerSignatureSpec? {
        val hasAtSelector = hasAtSelector(site)
        if (site.hasConstantSelector == hasAtSelector) return null
        if (site.hasConstantSelector) {
            return buildConstantWrapOperationSignature(targetMethod)
        }
        if (site.atValue.equals("MIXINEXTRAS:EXPRESSION", ignoreCase = true)) {
            val index = bytecodeIndex ?: return null
            val layout = ExpressionContextResolver.inferSimpleOperationLayout(
                source = source,
                site = site,
                targetMethod = targetMethod,
                mixinTargets = mixinTargets,
                bytecodeIndex = index,
                classIndex = classIndex,
                resolvedContext = resolvedContext,
            ) ?: return null
            return buildSimpleOperationSignature(layout, targetMethod)
        }
        if (site.atValue == "NEW") {
            return expectedWrapOperationNew(site, targetMethod, mixinTargets)
        }
        if (site.atValue != "FIELD" && site.atValue != "INVOKE") return null
        if (site.atValue == "FIELD" && isFieldAtTarget(site.atTarget)) {
            val field = parseAtFieldTarget(
                atTarget = site.atTarget,
                targetMethod = targetMethod,
                mixinTargets = mixinTargets,
                allowedOperationKinds = fieldWrapOperationKinds,
                atOrdinal = site.atOrdinal,
            )
                ?: return null
            return buildFieldWrapOperationSignature(field, targetMethod)
        }
        if (site.atValue != "INVOKE") return null
        val wrapped = parseAtWrapOperationInvokeTarget(
            atTarget = site.atTarget,
            targetMethod = targetMethod,
            mixinTargets = mixinTargets,
            atOrdinal = site.atOrdinal,
        ) ?: return null
        site.atOrdinal?.let { ordinal ->
            val atTarget = site.atTarget ?: return null
            val methodTarget = when (val parsed = MemberTargetParser.parse(atTarget)) {
                is DescriptorParseResult.Success -> parsed.value as? MemberTarget.Method ?: return null
                is DescriptorParseResult.Failure -> return null
            }
            val index = bytecodeIndex ?: return null
            val mixinOwners = MixinTargetResolver.resolveTargets(mixinTargets, classIndex)
            if (mixinOwners.isEmpty()) return null
            val methodDescriptor = DescriptorRenderer.toDescriptor(methodTarget.descriptor)
            var unifiedIsStatic: Boolean? = null
            for (mixinOwner in mixinOwners) {
                val matchingCandidates = index.getAtTargetCandidates(
                    mixinOwner,
                    targetMethod.name,
                    targetMethod.descriptor,
                    "INVOKE",
                ).filter {
                    it.kind == AtTargetKind.INVOKE &&
                        it.owner == methodTarget.owner &&
                        it.name == methodTarget.name &&
                        it.descriptor == methodDescriptor &&
                        it.operationKind in wrapWithConditionInvokeOperationKinds
                }
                if (matchingCandidates.any { it.instructionOccurrenceIndex < 0 }) return null
                val selectedCandidate = matchingCandidates
                    .sortedBy { it.instructionOccurrenceIndex }
                    .getOrNull(ordinal)
                    ?: return null
                val selectedIsStatic = selectedCandidate.operationKind == AtTargetOperationKind.INVOKE_STATIC
                if (selectedIsStatic != wrapped.isStatic) return null
                if (unifiedIsStatic == null) {
                    unifiedIsStatic = selectedIsStatic
                } else if (unifiedIsStatic != selectedIsStatic) {
                    return null
                }
            }
        }
        return buildWrapSignature(wrapped, operationGenericFromReturn(wrapped), targetMethod = targetMethod)
    }

    private fun expectedWrapWithCondition(
        source: String,
        site: MixinExtrasAnnotationSite,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
        resolvedContext: ExpressionContext? = null,
    ): HandlerSignatureSpec? {
        val resolution = resolveWrapWithConditionTarget(
            site = site,
            targetMethod = targetMethod,
            mixinTargets = mixinTargets,
            source = source,
            resolvedContext = resolvedContext,
        )
        if (resolution.status != WrapWithConditionTargetStatus.VALID_VOID &&
            resolution.status != WrapWithConditionTargetStatus.VALID_POPPED_NON_VOID
        ) {
            return null
        }
        val wrapped = resolution.wrapped ?: return null
        return buildWrapWithConditionSignature(wrapped, targetMethod)
    }

    private fun resolveWrapWithConditionTarget(
        site: MixinExtrasAnnotationSite,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
        source: String? = null,
        resolvedContext: ExpressionContext? = null,
    ): WrapWithConditionTargetResolution = when (site.atValue?.uppercase()) {
        "FIELD" -> resolveWrapWithConditionFieldTarget(site, targetMethod, mixinTargets)
        "INVOKE" -> resolveWrapWithConditionInvokeTarget(site, targetMethod, mixinTargets)
        "MIXINEXTRAS:EXPRESSION" -> if (source == null) {
            WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)
        } else {
            resolveWrapWithConditionExpressionTarget(
                source = source,
                site = site,
                targetMethod = targetMethod,
                mixinTargets = mixinTargets,
                resolvedContext = resolvedContext,
            )
        }
        null -> WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)
        else -> WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.INVALID_INSTRUCTION)
    }

    private fun resolveWrapWithConditionExpressionTarget(
        source: String,
        site: MixinExtrasAnnotationSite,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
        resolvedContext: ExpressionContext?,
    ): WrapWithConditionTargetResolution {
        val index = bytecodeIndex
            ?: return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)
        val layouts = ExpressionContextResolver.inferWrapWithConditionLayouts(
            source = source,
            site = site,
            targetMethod = targetMethod,
            mixinTargets = mixinTargets,
            bytecodeIndex = index,
            classIndex = classIndex,
            resolvedContext = resolvedContext,
        ) ?: return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)

        var unified: WrappedInstructionTarget? = null
        var hasPoppedResult = false
        for (layout in layouts) {
            when (layout.targetKind) {
                WrapWithConditionTargetKind.FIELD -> if (
                    layout.resultClassification != OccurrenceResultClassification.VOID
                ) {
                    return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.INVALID_INSTRUCTION)
                }
                WrapWithConditionTargetKind.INVOKE -> when (layout.resultClassification) {
                    OccurrenceResultClassification.RETAINED ->
                        return WrapWithConditionTargetResolution(
                            WrapWithConditionTargetStatus.INVALID_RETAINED_NON_VOID,
                        )
                    OccurrenceResultClassification.NOT_APPLICABLE ->
                        return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.INVALID_INSTRUCTION)
                    OccurrenceResultClassification.IMMEDIATELY_POPPED -> hasPoppedResult = true
                    OccurrenceResultClassification.VOID -> Unit
                }
                WrapWithConditionTargetKind.OTHER ->
                    return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.INVALID_INSTRUCTION)
            }

            val current = WrappedInstructionTarget(
                owner = layout.ownerInternalName,
                parameterDescriptors = layout.parameterDescriptors,
                isStatic = layout.isStatic,
            )
            if (unified == null) {
                unified = current
            } else if (unified != current) {
                return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)
            }
        }

        return WrapWithConditionTargetResolution(
            status = if (hasPoppedResult) {
                WrapWithConditionTargetStatus.VALID_POPPED_NON_VOID
            } else {
                WrapWithConditionTargetStatus.VALID_VOID
            },
            wrapped = unified,
        )
    }

    private fun resolveWrapWithConditionFieldTarget(
        site: MixinExtrasAnnotationSite,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
    ): WrapWithConditionTargetResolution {
        val index = bytecodeIndex
            ?: return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)
        val mixinOwners = MixinTargetResolver.resolveTargets(mixinTargets, classIndex)
        if (mixinOwners.isEmpty()) {
            return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)
        }
        val atTarget = site.atTarget?.takeIf { it.isNotBlank() }
            ?: return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)
        val parsed = MemberTargetParser.parse(atTarget)
        if (parsed !is DescriptorParseResult.Success) {
            return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)
        }
        val fieldTarget = parsed.value as? MemberTarget.Field
            ?: return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.INVALID_INSTRUCTION)
        val fieldOwner = fieldTarget.owner
        val fieldName = fieldTarget.name
        val fieldDescriptor = DescriptorRenderer.toDescriptor(fieldTarget.descriptor)
        var unifiedIsStatic: Boolean? = null
        for (mixinOwner in mixinOwners) {
            val matchingCandidates = index.getAtTargetCandidates(
                mixinOwner,
                targetMethod.name,
                targetMethod.descriptor,
                "FIELD",
            ).filter {
                it.kind == AtTargetKind.FIELD &&
                    it.owner == fieldOwner &&
                    it.name == fieldName &&
                    it.descriptor == fieldDescriptor
            }
            if (site.atOrdinal != null && matchingCandidates.any { it.instructionOccurrenceIndex < 0 }) {
                return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)
            }
            val sortedCandidates = matchingCandidates.sortedBy { it.instructionOccurrenceIndex }
            val selectedCandidates = site.atOrdinal?.let { ordinal ->
                sortedCandidates.getOrNull(ordinal)?.let(::listOf)
                    ?: return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)
            } ?: matchingCandidates
            if (selectedCandidates.isEmpty()) {
                return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)
            }
            var ownerIsStatic: Boolean? = null
            for (candidate in selectedCandidates) {
                when (val operationKind = candidate.operationKind) {
                    AtTargetOperationKind.FIELD_GET_INSTANCE,
                    AtTargetOperationKind.FIELD_GET_STATIC,
                    -> return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.INVALID_INSTRUCTION)
                    AtTargetOperationKind.FIELD_PUT_INSTANCE -> {
                        if (ownerIsStatic == null) {
                            ownerIsStatic = false
                        } else if (ownerIsStatic != false) {
                            return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)
                        }
                    }
                    AtTargetOperationKind.FIELD_PUT_STATIC -> {
                        if (ownerIsStatic == null) {
                            ownerIsStatic = true
                        } else if (ownerIsStatic != true) {
                            return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)
                        }
                    }
                    null -> return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)
                    else -> return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)
                }
            }
            if (unifiedIsStatic == null) {
                unifiedIsStatic = ownerIsStatic
            } else if (unifiedIsStatic != ownerIsStatic) {
                return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)
            }
        }
        return WrapWithConditionTargetResolution(
            status = WrapWithConditionTargetStatus.VALID_VOID,
            wrapped = WrappedInstructionTarget(
                owner = fieldOwner,
                parameterDescriptors = listOf(fieldDescriptor),
                isStatic = unifiedIsStatic ?: return WrapWithConditionTargetResolution(
                    WrapWithConditionTargetStatus.UNRESOLVED,
                ),
            ),
        )
    }

    private fun resolveWrapWithConditionInvokeTarget(
        site: MixinExtrasAnnotationSite,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
    ): WrapWithConditionTargetResolution {
        val index = bytecodeIndex
            ?: return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)
        val mixinOwners = MixinTargetResolver.resolveTargets(mixinTargets, classIndex)
        if (mixinOwners.isEmpty()) {
            return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)
        }
        val atTarget = site.atTarget?.takeIf { it.isNotBlank() }
            ?: return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)
        val parsed = MemberTargetParser.parse(atTarget)
        if (parsed !is DescriptorParseResult.Success) {
            return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)
        }
        val methodTarget = parsed.value as? MemberTarget.Method
            ?: return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.INVALID_INSTRUCTION)
        if (methodTarget.name == "<init>") {
            return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.INVALID_INSTRUCTION)
        }
        val calleeOwner = methodTarget.owner
        val calleeName = methodTarget.name
        val methodDescriptor = DescriptorRenderer.toDescriptor(methodTarget.descriptor)
        val parameterDescriptors = methodTarget.descriptor.parameters.map(DescriptorRenderer::toDescriptor)
        var unifiedIsStatic: Boolean? = null
        var hasPoppedResult = false
        for (mixinOwner in mixinOwners) {
            val matchingCandidates = index.getAtTargetCandidates(
                mixinOwner,
                targetMethod.name,
                targetMethod.descriptor,
                "INVOKE",
            ).filter {
                it.kind == AtTargetKind.INVOKE &&
                    it.owner == calleeOwner &&
                    it.name == calleeName &&
                    it.descriptor == methodDescriptor &&
                    it.name != "<init>" &&
                    it.operationKind in wrapWithConditionInvokeOperationKinds
            }
            if (site.atOrdinal != null && matchingCandidates.any { it.instructionOccurrenceIndex < 0 }) {
                return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)
            }
            val sortedCandidates = matchingCandidates.sortedBy { it.instructionOccurrenceIndex }
            val selectedCandidates = site.atOrdinal?.let { ordinal ->
                sortedCandidates.getOrNull(ordinal)?.let(::listOf)
                    ?: return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)
            } ?: matchingCandidates
            if (selectedCandidates.isEmpty()) {
                return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)
            }
            var ownerIsStatic: Boolean? = null
            for (candidate in selectedCandidates) {
                when (candidate.occurrenceResultClassification) {
                    OccurrenceResultClassification.RETAINED ->
                        return WrapWithConditionTargetResolution(
                            WrapWithConditionTargetStatus.INVALID_RETAINED_NON_VOID,
                        )
                    OccurrenceResultClassification.NOT_APPLICABLE ->
                        return WrapWithConditionTargetResolution(
                            WrapWithConditionTargetStatus.INVALID_INSTRUCTION,
                        )
                    OccurrenceResultClassification.IMMEDIATELY_POPPED -> hasPoppedResult = true
                    OccurrenceResultClassification.VOID -> Unit
                }
                val isStatic = candidate.operationKind == AtTargetOperationKind.INVOKE_STATIC
                if (ownerIsStatic == null) {
                    ownerIsStatic = isStatic
                } else if (ownerIsStatic != isStatic) {
                    return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)
                }
            }
            if (unifiedIsStatic == null) {
                unifiedIsStatic = ownerIsStatic
            } else if (unifiedIsStatic != ownerIsStatic) {
                return WrapWithConditionTargetResolution(WrapWithConditionTargetStatus.UNRESOLVED)
            }
        }
        val status = if (hasPoppedResult) {
            WrapWithConditionTargetStatus.VALID_POPPED_NON_VOID
        } else {
            WrapWithConditionTargetStatus.VALID_VOID
        }
        return WrapWithConditionTargetResolution(
            status = status,
            wrapped = WrappedInstructionTarget(
                owner = calleeOwner,
                parameterDescriptors = parameterDescriptors,
                isStatic = unifiedIsStatic ?: return WrapWithConditionTargetResolution(
                    WrapWithConditionTargetStatus.UNRESOLVED,
                ),
            ),
        )
    }

    private fun buildWrapWithConditionSignature(
        wrapped: WrappedInstructionTarget,
        targetMethod: MethodIndexEntry,
    ): HandlerSignatureSpec {
        val params = mutableListOf<HandlerParameterSpec>()
        if (!wrapped.isStatic) {
            val receiverType = "L${wrapped.owner};"
            params += HandlerParameterSpec(
                name = receiverParameterName(wrapped.owner),
                typeDescriptor = receiverType,
                readableType = OperationSignatureRenderer.readableType(receiverType),
            )
        }
        wrapped.parameterDescriptors.forEachIndexed { index, descriptor ->
            params += HandlerParameterSpec(
                name = "arg$index",
                typeDescriptor = descriptor,
                readableType = OperationSignatureRenderer.readableType(descriptor),
            )
        }
        return HandlerSignatureSpec(
            returnTypeDescriptor = "Z",
            readableReturnType = OperationSignatureRenderer.readableType("Z"),
            parameters = params,
            optionalCapturedTargetParameters = capturedTargetParametersFrom(targetMethod),
        )
    }

    private fun expectedWrapMethod(targetMethod: MethodIndexEntry): HandlerSignatureSpec? {
        if (targetMethod.name == "<init>" || targetMethod.name == "<clinit>") return null
        val parsed = parseMethodDescriptor(targetMethod.descriptor)
        if (parsed !is DescriptorParseResult.Success) {
            return null
        }
        val returnType = DescriptorRenderer.toDescriptor(parsed.value.returnType)
        val params = mutableListOf<HandlerParameterSpec>()
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

    private fun hasAtSelector(site: MixinExtrasAnnotationSite): Boolean =
        site.atValue != null || site.atTarget != null

    private fun buildConstantWrapOperationSignature(targetMethod: MethodIndexEntry): HandlerSignatureSpec =
        HandlerSignatureSpec(
            returnTypeDescriptor = "Z",
            readableReturnType = OperationSignatureRenderer.readableType("Z"),
            parameters = listOf(
                HandlerParameterSpec(
                    name = "obj",
                    typeDescriptor = "Ljava/lang/Object;",
                    readableType = OperationSignatureRenderer.readableType("Ljava/lang/Object;"),
                ),
                HandlerParameterSpec(
                    name = "original",
                    typeDescriptor = "Lcom/llamalad7/mixinextras/injector/wrapoperation/Operation;",
                    readableType = OperationSignatureRenderer.renderOperationType("Z"),
                    isOperation = true,
                    operationGenericDescriptor = "Z",
                ),
            ),
            operationCallArgs = listOf("obj"),
            optionalCapturedTargetParameters = capturedTargetParametersFrom(targetMethod),
        )

    private fun buildWrapSignature(
        wrapped: WrappedOperationTarget,
        operationGeneric: String,
        returnDescriptor: String = operationGeneric,
        targetMethod: MethodIndexEntry? = null,
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
            optionalCapturedTargetParameters = targetMethod?.let(::capturedTargetParametersFrom) ?: emptyList(),
        )
    }

    private fun buildSimpleOperationSignature(
        layout: SimpleOperationLayout,
        targetMethod: MethodIndexEntry,
    ): HandlerSignatureSpec {
        val params = layout.argumentDescriptors.mapIndexed { index, descriptor ->
            val acceptedIntLikeTypes = if (layout.argumentIntLike.getOrNull(index) == true) {
                INT_LIKE_DESCRIPTORS
            } else {
                emptySet()
            }
            HandlerParameterSpec(
                name = layout.parameterNames[index],
                typeDescriptor = descriptor,
                readableType = OperationSignatureRenderer.readableType(descriptor),
                acceptedTypeDescriptors = acceptedIntLikeTypes,
            )
        }.toMutableList()
        val acceptedOperationTypes = if (layout.returnIntLike) INT_LIKE_DESCRIPTORS else emptySet()
        params += HandlerParameterSpec(
            name = "original",
            typeDescriptor = "Lcom/llamalad7/mixinextras/injector/wrapoperation/Operation;",
            readableType = OperationSignatureRenderer.renderOperationType(layout.returnDescriptor),
            isOperation = true,
            operationGenericDescriptor = layout.returnDescriptor,
            acceptedOperationGenericDescriptors = acceptedOperationTypes,
        )
        return HandlerSignatureSpec(
            returnTypeDescriptor = layout.returnDescriptor,
            readableReturnType = OperationSignatureRenderer.readableType(layout.returnDescriptor),
            parameters = params,
            operationCallArgs = layout.parameterNames,
            optionalCapturedTargetParameters = capturedTargetParametersFrom(targetMethod),
            acceptedReturnTypeDescriptors = acceptedOperationTypes,
        )
    }

    private fun capturedTargetParametersFrom(targetMethod: MethodIndexEntry): List<HandlerParameterSpec> =
        methodParameterDescriptors(targetMethod.descriptor).mapIndexed { index, descriptor ->
            HandlerParameterSpec(
                name = "arg$index",
                typeDescriptor = descriptor,
                readableType = OperationSignatureRenderer.readableType(descriptor),
            )
        }

    fun inferExpressionValueType(
        source: String,
        site: MixinExtrasAnnotationSite,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
        resolvedContext: ExpressionContext? = null,
    ): String? {
        val atValue = site.atValue?.uppercase()
        if (atValue == "MIXINEXTRAS:EXPRESSION") {
            val index = bytecodeIndex ?: return null
            return ExpressionContextResolver.inferExpressionValueType(
                source = source,
                site = site,
                targetMethod = targetMethod,
                mixinTargets = mixinTargets,
                bytecodeIndex = index,
                classIndex = classIndex,
                resolvedContext = resolvedContext,
            )
        }
        if (atValue == "CONSTANT") {
            return ConstantAtArgsParser.parse(site.atArgs)
        }
        if (atValue == "INVOKE") {
            val wrapped = parseAtInvokeTarget(site.atTarget) ?: return null
            if (wrapped.returnDescriptor == "V") return null
            return wrapped.returnDescriptor
        }
        if (atValue == "FIELD") {
            val field = parseAtFieldTarget(site.atTarget, targetMethod, mixinTargets, fieldModifyExpressionValueKinds)
                ?: return null
            return field.fieldDescriptor
        }
        if (atValue == "NEW") {
            return inferNewExpressionValueType(site.atTarget, targetMethod, mixinTargets)
        }
        return null
    }

    private fun inferNewExpressionValueType(
        atTarget: String?,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
    ): String? {
        if (!atTarget.isNullOrBlank()) {
            return parseAtNewTarget(atTarget)
        }
        return inferNewExpressionValueTypeFromBytecode(targetMethod, mixinTargets)
    }

    private fun parseAtNewTarget(atTarget: String): String? {
        if (atTarget.isBlank() || hasMalformedNewTargetWhitespace(atTarget) || atTarget.startsWith("[")) {
            return null
        }
        if (!usesInternalNewOwnerForm(atTarget)) return null
        val selector = parseMethodSelectorOrNull(atTarget) ?: return null
        return newTypeDescriptorFromSelector(selector)
    }

    private fun inferNewExpressionValueTypeFromBytecode(
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
    ): String? {
        val index = bytecodeIndex ?: return null
        val mixinOwners = MixinTargetResolver.resolveTargets(mixinTargets, classIndex)
        if (mixinOwners.isEmpty()) return null
        val ownerDescriptors = mixinOwners.flatMap { mixinOwner ->
            index.getAtTargetCandidates(
                mixinOwner,
                targetMethod.name,
                targetMethod.descriptor,
                "NEW",
            )
        }.filter { it.kind == AtTargetKind.NEW }
            .mapNotNull { candidate -> newTypeDescriptorFromInternalOwner(candidate.owner) }
            .distinct()
        return ownerDescriptors.singleOrNull()
    }

    private fun parseMethodSelectorOrNull(input: String): MethodSelector? =
        when (val parsed = parseMethodSelector(input)) {
            is DescriptorParseResult.Success -> parsed.value
            is DescriptorParseResult.Failure -> null
        }

    private fun newTypeDescriptorFromInternalOwner(internalOwner: String): String? =
        parseMethodSelectorOrNull("L$internalOwner;")?.let { newTypeDescriptorFromSelector(it) }

    private fun newTypeDescriptorFromSelector(selector: MethodSelector): String? {
        val owner = when (val ownerPattern = selector.owner) {
            is Pattern.Exact -> ownerPattern.value
            is Pattern.Any -> return null
        }
        val ownerDescriptor = "L$owner;"
        val isClassOnly = selector.name is Pattern.Any && selector.descriptor is Pattern.Any
        if (isClassOnly) return ownerDescriptor
        val isExactInit = selector.name is Pattern.Exact &&
            selector.name.value == "<init>" &&
            selector.descriptor is Pattern.Exact &&
            selector.descriptor.value.returnType == JvmType.VoidType
        return if (isExactInit) ownerDescriptor else null
    }

    private fun expectedWrapOperationNew(
        site: MixinExtrasAnnotationSite,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
    ): HandlerSignatureSpec? {
        val index = bytecodeIndex ?: return null
        val targetFilter = when {
            site.atTarget.isNullOrBlank() -> NewWrapTargetFilter(null, null)
            else -> parseNewWrapTargetFilter(site.atTarget) ?: return null
        }
        val requestedOrdinal = site.atOrdinal
        val mixinOwners = MixinTargetResolver.resolveTargets(mixinTargets, classIndex)
        if (mixinOwners.isEmpty()) return null

        var unifiedOwner: String? = null
        var unifiedParameterDescriptors: List<String>? = null

        for (mixinOwner in mixinOwners) {
            val classBytes = index.getClassBytes(mixinOwner) ?: return null
            val pairedSites = extractNewSitePairs(classBytes, targetMethod.name, targetMethod.descriptor) ?: return null
            var candidates = index.getAtTargetCandidates(
                mixinOwner,
                targetMethod.name,
                targetMethod.descriptor,
                "NEW",
            ).filter { it.kind == AtTargetKind.NEW }

            targetFilter.ownerInternalName?.let { owner ->
                candidates = candidates.filter { it.owner == owner }
            }
            requestedOrdinal?.let { ordinal ->
                candidates = candidates.filter { (it.ordinal ?: 0) == ordinal }
            }
            targetFilter.constructorDescriptor?.let { constructorDescriptor ->
                candidates = candidates.filter { candidate ->
                    pairedSites.find {
                        it.owner == candidate.owner && it.ordinal == (candidate.ordinal ?: 0)
                    }?.constructorDescriptor == constructorDescriptor
                }
            }
            if (candidates.isEmpty()) return null

            if (site.atTarget.isNullOrBlank()) {
                val distinctOwners = candidates.map { it.owner }.distinct()
                if (distinctOwners.size != 1) return null
            }

            val resolvedSites = candidates.map { candidate ->
                pairedSites.find {
                    it.owner == candidate.owner && it.ordinal == (candidate.ordinal ?: 0)
                } ?: return null
            }
            val parameterSets = resolvedSites.map { it.parameterDescriptors }.distinct()
            if (parameterSets.size != 1) return null

            val owner = resolvedSites.first().owner
            val parameterDescriptors = parameterSets.single()
            if (unifiedOwner == null) {
                unifiedOwner = owner
            } else if (unifiedOwner != owner) {
                return null
            }
            if (unifiedParameterDescriptors == null) {
                unifiedParameterDescriptors = parameterDescriptors
            } else if (unifiedParameterDescriptors != parameterDescriptors) {
                return null
            }
        }

        val allocatedType = "L${unifiedOwner!!};"
        return buildWrapSignature(
            WrappedOperationTarget(
                owner = unifiedOwner,
                parameterDescriptors = unifiedParameterDescriptors!!,
                returnDescriptor = allocatedType,
                isStatic = true,
            ),
            operationGeneric = allocatedType,
            targetMethod = targetMethod,
        )
    }

    private data class NewWrapTargetFilter(
        val ownerInternalName: String?,
        val constructorDescriptor: String?,
    )

    private data class PairedNewSite(
        val owner: String,
        val ordinal: Int,
        val constructorDescriptor: String,
        val parameterDescriptors: List<String>,
    )

    private fun parseNewWrapTargetFilter(atTarget: String): NewWrapTargetFilter? {
        if (atTarget.isBlank() || hasMalformedNewTargetWhitespace(atTarget) || atTarget.startsWith("[")) {
            return null
        }
        if (!usesInternalNewOwnerForm(atTarget)) return null
        val selector = parseMethodSelectorOrNull(atTarget) ?: return null
        val owner = when (val ownerPattern = selector.owner) {
            is Pattern.Exact -> ownerPattern.value
            is Pattern.Any -> return null
        }
        val constructorDescriptor = when {
            selector.name is Pattern.Exact &&
                selector.name.value == "<init>" &&
                selector.descriptor is Pattern.Exact &&
                selector.descriptor.value.returnType == JvmType.VoidType ->
                DescriptorRenderer.toDescriptor(selector.descriptor.value)
            selector.name is Pattern.Any && selector.descriptor is Pattern.Any -> null
            else -> return null
        }
        return NewWrapTargetFilter(owner, constructorDescriptor)
    }

    private fun extractNewSitePairs(
        classBytes: ByteArray,
        methodName: String,
        methodDescriptor: String,
    ): List<PairedNewSite>? {
        val reader = ClassReader(classBytes)
        val collector = NewConstructorPairCollector(methodName, methodDescriptor)
        reader.accept(collector, ClassReader.SKIP_DEBUG or ClassReader.SKIP_FRAMES)
        return collector.buildPairs()
    }

    private class NewConstructorPairCollector(
        private val targetMethodName: String,
        private val targetMethodDescriptor: String,
    ) : org.objectweb.asm.ClassVisitor(Opcodes.ASM9) {
        private val newOwners = mutableListOf<String>()
        private val constructorDescriptors = mutableMapOf<Int, String>()
        private val pendingNewStack = ArrayDeque<Int>()
        private var constructorInitialized = targetMethodName != "<init>"
        private var newBeforeConstructorInitialization = false
        private var failed = false

        override fun visitMethod(
            access: Int,
            name: String,
            descriptor: String,
            signature: String?,
            exceptions: Array<out String>?,
        ): MethodVisitor? {
            if (name != targetMethodName || descriptor != targetMethodDescriptor) {
                return null
            }
            return object : MethodVisitor(Opcodes.ASM9) {
                override fun visitTypeInsn(opcode: Int, type: String) {
                    if (opcode != Opcodes.NEW) return
                    if (targetMethodName == "<init>" && !constructorInitialized) {
                        newBeforeConstructorInitialization = true
                    }
                    val index = newOwners.size
                    newOwners += type
                    pendingNewStack.addLast(index)
                }

                override fun visitMethodInsn(
                    opcode: Int,
                    owner: String,
                    name: String,
                    descriptor: String,
                    isInterface: Boolean,
                ) {
                    if (opcode != Opcodes.INVOKESPECIAL || name != "<init>") return
                    val index = pendingNewStack.removeLastOrNull()
                    if (index == null) {
                        if (targetMethodName == "<init>" && !constructorInitialized) {
                            // With no outstanding NEW value, the only valid
                            // uninitialized receiver is this/super.
                            constructorInitialized = true
                        }
                        return
                    }
                    if (newOwners[index] != owner || constructorDescriptors.containsKey(index)) {
                        failed = true
                        return
                    }
                    if (targetMethodName == "<init>" && !constructorInitialized) {
                        // A constructor call paired with NEW initializes an
                        // allocated object, so the mandatory this/super call
                        // has not happened yet. The earlier NEW is unsafe.
                        newBeforeConstructorInitialization = true
                    }
                    constructorDescriptors[index] = descriptor
                }

                override fun visitEnd() {
                    if (pendingNewStack.isNotEmpty()) {
                        failed = true
                    }
                }
            }
        }

        fun buildPairs(): List<PairedNewSite>? {
            if (
                failed ||
                newBeforeConstructorInitialization ||
                newOwners.indices.any { constructorDescriptors[it] == null }
            ) {
                return null
            }
            val ordinals = mutableMapOf<String, Int>()
            return newOwners.indices.mapNotNull { index ->
                val owner = newOwners[index]
                val constructorDescriptor = constructorDescriptors[index] ?: return null
                val ordinal = ordinals.getOrDefault(owner, 0)
                ordinals[owner] = ordinal + 1
                PairedNewSite(
                    owner = owner,
                    ordinal = ordinal,
                    constructorDescriptor = constructorDescriptor,
                    parameterDescriptors = Type.getArgumentTypes(constructorDescriptor).map { it.descriptor },
                )
            }
        }
    }

    private fun hasMalformedNewTargetWhitespace(value: String): Boolean =
        value.any { it.isWhitespace() || it.isISOControl() }

    private fun usesInternalNewOwnerForm(input: String): Boolean {
        if (!input.startsWith("L")) return false
        val semicolon = input.indexOf(';')
        if (semicolon <= 0) return false
        val descriptorStart = input.indexOf('(')
        return descriptorStart < 0 || semicolon < descriptorStart
    }

    private fun parseAtInvokeTarget(atTarget: String?): WrappedOperationTarget? =
        parseAtWrapWithConditionTarget(atTarget)?.toWrappedOperationTarget()

    private fun parseAtWrapOperationInvokeTarget(
        atTarget: String?,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
        atOrdinal: Int? = null,
    ): WrappedOperationTarget? {
        val parsed = atTarget?.let(MemberTargetParser::parse)
        if (parsed !is DescriptorParseResult.Success) return null
        val target = parsed.value as? MemberTarget.Method ?: return null
        if (target.name == "<init>") return null
        val methodDescriptor = DescriptorRenderer.toDescriptor(target.descriptor)
        val parameterDescriptors = target.descriptor.parameters.map(DescriptorRenderer::toDescriptor)
        val returnDescriptor = DescriptorRenderer.toDescriptor(target.descriptor.returnType)
        val isStatic = classIndex.getMethods(target.owner)
            .find { it.name == target.name && it.descriptor == methodDescriptor }
            ?.isStatic
            ?: false
        val fallback = WrappedOperationTarget(
            owner = target.owner,
            parameterDescriptors = parameterDescriptors,
            returnDescriptor = returnDescriptor,
            isStatic = isStatic,
        )
        val index = bytecodeIndex ?: return fallback
        val mixinOwners = MixinTargetResolver.resolveTargets(mixinTargets, classIndex)
        if (mixinOwners.isEmpty()) return fallback
        val effectiveReceivers = mutableListOf<String>()
        var effectiveStatic: Boolean? = null
        for (mixinOwner in mixinOwners) {
            val candidates = index.getAtTargetCandidates(
                mixinOwner,
                targetMethod.name,
                targetMethod.descriptor,
                "INVOKE",
            ).filter {
                it.kind == AtTargetKind.INVOKE &&
                    it.owner == target.owner &&
                    it.name == target.name &&
                    it.descriptor == methodDescriptor
            }
            if (atOrdinal != null && candidates.any { it.instructionOccurrenceIndex < 0 }) return null
            val selectedCandidates = atOrdinal?.let { ordinal ->
                candidates.sortedBy { it.instructionOccurrenceIndex }.getOrNull(ordinal)?.let(::listOf)
                    ?: return null
            } ?: candidates
            val operationKinds = selectedCandidates.mapNotNull { it.operationKind }.distinct()
            if (selectedCandidates.isEmpty()) return fallback
            if (operationKinds.isEmpty()) return fallback
            val operationKind = operationKinds.singleOrNull() ?: return null
            val candidateStatic = operationKind == AtTargetOperationKind.INVOKE_STATIC
            if (effectiveStatic == null) {
                effectiveStatic = candidateStatic
            } else if (effectiveStatic != candidateStatic) {
                return null
            }
            if (!candidateStatic) {
                effectiveReceivers += when (operationKind) {
                    AtTargetOperationKind.INVOKE_SPECIAL -> mixinOwner
                    AtTargetOperationKind.INVOKE_VIRTUAL,
                    AtTargetOperationKind.INVOKE_INTERFACE,
                    -> target.owner
                    else -> return null
                }
            }
        }
        if (effectiveStatic == false && effectiveReceivers.distinct().size != 1) return null
        val receiverOwner = effectiveReceivers.distinct().singleOrNull() ?: target.owner
        return fallback.copy(
            owner = receiverOwner,
            isStatic = effectiveStatic ?: fallback.isStatic,
        )
    }

    private fun parseAtModifyReceiverTarget(
        atTarget: String?,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
    ): WrappedInstructionTarget? {
        if (atTarget.isNullOrBlank()) return null
        return when (val parsed = MemberTargetParser.parse(atTarget)) {
            is DescriptorParseResult.Success -> when (val target = parsed.value) {
                is MemberTarget.Method -> {
                    val methodDescriptor = DescriptorRenderer.toDescriptor(target.descriptor)
                    classIndex.getMethods(target.owner)
                        .find { it.name == target.name && it.descriptor == methodDescriptor }
                        ?.takeIf { it.isStatic }
                        ?.let { return null }
                    resolveModifyReceiverInvokeTarget(
                        calleeOwner = target.owner,
                        name = target.name,
                        methodDescriptor = methodDescriptor,
                        parameterDescriptors = target.descriptor.parameters.map(DescriptorRenderer::toDescriptor),
                        targetMethod = targetMethod,
                        mixinTargets = mixinTargets,
                    )
                }
                is MemberTarget.Field -> {
                    val field = parseAtFieldTarget(atTarget, targetMethod, mixinTargets, fieldModifyReceiverKinds)
                        ?: return null
                    when (field.operationKind) {
                        AtTargetOperationKind.FIELD_GET_INSTANCE -> WrappedInstructionTarget(
                            owner = field.owner,
                            parameterDescriptors = emptyList(),
                            isStatic = false,
                        )
                        AtTargetOperationKind.FIELD_PUT_INSTANCE -> WrappedInstructionTarget(
                            owner = field.owner,
                            parameterDescriptors = listOf(field.fieldDescriptor),
                            isStatic = false,
                        )
                        else -> null
                    }
                }
            }
            else -> null
        }
    }

    private fun isFieldAtTarget(atTarget: String?): Boolean {
        if (atTarget.isNullOrBlank()) return false
        val parsed = MemberTargetParser.parse(atTarget)
        return parsed is DescriptorParseResult.Success && parsed.value is MemberTarget.Field
    }

    private fun parseAtFieldTarget(
        atTarget: String?,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
        allowedOperationKinds: Set<AtTargetOperationKind>,
        atOrdinal: Int? = null,
    ): FieldWrapTarget? {
        if (atTarget.isNullOrBlank()) return null
        val parsed = MemberTargetParser.parse(atTarget)
        if (parsed !is DescriptorParseResult.Success) return null
        val target = parsed.value
        if (target !is MemberTarget.Field) return null
        val fieldDescriptor = DescriptorRenderer.toDescriptor(target.descriptor)
        val operationKind = resolveFieldOperationKind(
            owner = target.owner,
            name = target.name,
            descriptor = fieldDescriptor,
            targetMethod = targetMethod,
            mixinTargets = mixinTargets,
            atOrdinal = atOrdinal,
        ) ?: return null
        if (operationKind !in allowedOperationKinds) return null
        return FieldWrapTarget(
            owner = target.owner,
            fieldDescriptor = fieldDescriptor,
            operationKind = operationKind,
        )
    }

    private fun resolveModifyReceiverInvokeTarget(
        calleeOwner: String,
        name: String,
        methodDescriptor: String,
        parameterDescriptors: List<String>,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
    ): WrappedInstructionTarget? {
        val index = bytecodeIndex ?: return null
        val mixinOwners = MixinTargetResolver.resolveTargets(mixinTargets, classIndex)
        if (mixinOwners.isEmpty()) return null
        val effectiveReceiverOwners = mixinOwners.mapNotNull { mixinOwner ->
            val candidates = index.getAtTargetCandidates(
                mixinOwner,
                targetMethod.name,
                targetMethod.descriptor,
                "INVOKE",
            ).filter {
                it.kind == AtTargetKind.INVOKE &&
                    it.owner == calleeOwner &&
                    it.name == name &&
                    it.descriptor == methodDescriptor
            }
            val operationKinds = candidates.mapNotNull { it.operationKind }.distinct()
            when (val operationKind = operationKinds.singleOrNull()) {
                AtTargetOperationKind.INVOKE_STATIC,
                AtTargetOperationKind.FIELD_GET_INSTANCE,
                AtTargetOperationKind.FIELD_PUT_INSTANCE,
                AtTargetOperationKind.FIELD_GET_STATIC,
                AtTargetOperationKind.FIELD_PUT_STATIC,
                -> return null
                AtTargetOperationKind.INVOKE_VIRTUAL,
                AtTargetOperationKind.INVOKE_INTERFACE,
                -> calleeOwner
                AtTargetOperationKind.INVOKE_SPECIAL -> mixinOwner
                null -> return null
            }
        }
        if (effectiveReceiverOwners.size != mixinOwners.size) return null
        val receiverOwner = effectiveReceiverOwners.distinct().singleOrNull() ?: return null
        return WrappedInstructionTarget(
            owner = receiverOwner,
            parameterDescriptors = parameterDescriptors,
            isStatic = false,
        )
    }

    private fun resolveFieldOperationKind(
        owner: String,
        name: String,
        descriptor: String,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
        atOrdinal: Int?,
    ): AtTargetOperationKind? {
        val index = bytecodeIndex ?: return null
        val mixinOwners = MixinTargetResolver.resolveTargets(mixinTargets, classIndex)
        if (mixinOwners.isEmpty()) return null
        val candidates = mutableListOf<AtTargetCandidate>()
        for (mixinOwner in mixinOwners) {
            val matchingCandidates = index.getAtTargetCandidates(
                mixinOwner,
                targetMethod.name,
                targetMethod.descriptor,
                "FIELD",
            ).filter {
                it.kind == AtTargetKind.FIELD &&
                    it.owner == owner &&
                    it.name == name &&
                    it.descriptor == descriptor
            }
            if (atOrdinal == null) {
                candidates += matchingCandidates
            } else {
                if (matchingCandidates.any { it.instructionOccurrenceIndex < 0 }) return null
                val selectedCandidate = matchingCandidates
                    .sortedBy { it.instructionOccurrenceIndex }
                    .getOrNull(atOrdinal)
                    ?: return null
                candidates += selectedCandidate
            }
        }
        if (atOrdinal != null && candidates.any { it.operationKind == null }) return null
        val operationKinds = candidates.mapNotNull { it.operationKind }.distinct()
        return operationKinds.singleOrNull()
    }

    private fun buildFieldWrapOperationSignature(
        field: FieldWrapTarget,
        targetMethod: MethodIndexEntry,
    ): HandlerSignatureSpec? =
        when (field.operationKind) {
            AtTargetOperationKind.FIELD_GET_INSTANCE -> buildWrapSignature(
                WrappedOperationTarget(
                    owner = field.owner,
                    parameterDescriptors = emptyList(),
                    returnDescriptor = field.fieldDescriptor,
                    isStatic = false,
                ),
                operationGeneric = field.fieldDescriptor,
                returnDescriptor = field.fieldDescriptor,
                targetMethod = targetMethod,
            )
            AtTargetOperationKind.FIELD_GET_STATIC -> buildWrapSignature(
                WrappedOperationTarget(
                    owner = field.owner,
                    parameterDescriptors = emptyList(),
                    returnDescriptor = field.fieldDescriptor,
                    isStatic = true,
                ),
                operationGeneric = field.fieldDescriptor,
                returnDescriptor = field.fieldDescriptor,
                targetMethod = targetMethod,
            )
            AtTargetOperationKind.FIELD_PUT_INSTANCE -> buildWrapSignature(
                WrappedOperationTarget(
                    owner = field.owner,
                    parameterDescriptors = listOf(field.fieldDescriptor),
                    returnDescriptor = "V",
                    isStatic = false,
                ),
                operationGeneric = "V",
                returnDescriptor = "V",
                targetMethod = targetMethod,
            )
            AtTargetOperationKind.FIELD_PUT_STATIC -> buildWrapSignature(
                WrappedOperationTarget(
                    owner = field.owner,
                    parameterDescriptors = listOf(field.fieldDescriptor),
                    returnDescriptor = "V",
                    isStatic = true,
                ),
                operationGeneric = "V",
                returnDescriptor = "V",
                targetMethod = targetMethod,
            )
            else -> null
        }

    private fun buildFieldWrapWithConditionSignature(
        field: FieldWrapTarget,
        targetMethod: MethodIndexEntry,
    ): HandlerSignatureSpec? =
        when (field.operationKind) {
            AtTargetOperationKind.FIELD_PUT_INSTANCE -> buildWrapWithConditionSignature(
                WrappedInstructionTarget(
                    owner = field.owner,
                    parameterDescriptors = listOf(field.fieldDescriptor),
                    isStatic = false,
                ),
                targetMethod,
            )
            AtTargetOperationKind.FIELD_PUT_STATIC -> buildWrapWithConditionSignature(
                WrappedInstructionTarget(
                    owner = field.owner,
                    parameterDescriptors = listOf(field.fieldDescriptor),
                    isStatic = true,
                ),
                targetMethod,
            )
            else -> null
        }

    private fun parseAtWrapWithConditionTarget(atTarget: String?): WrappedInstructionTarget? {
        if (atTarget.isNullOrBlank()) return null
        return when (val parsed = MemberTargetParser.parse(atTarget)) {
            is DescriptorParseResult.Success -> when (val target = parsed.value) {
                is MemberTarget.Method -> {
                    val methodDescriptor = DescriptorRenderer.toDescriptor(target.descriptor)
                    val isStatic = classIndex.getMethods(target.owner)
                        .find { it.name == target.name && it.descriptor == methodDescriptor }
                        ?.isStatic
                        ?: false
                    WrappedInstructionTarget(
                        owner = target.owner,
                        parameterDescriptors = target.descriptor.parameters.map(DescriptorRenderer::toDescriptor),
                        isStatic = isStatic,
                        returnDescriptor = DescriptorRenderer.toDescriptor(target.descriptor.returnType),
                    )
                }
                is MemberTarget.Field -> null
            }
            else -> null
        }
    }

    private fun operationGenericFromReturn(wrapped: WrappedOperationTarget): String = wrapped.returnDescriptor

    private fun methodReturnDescriptor(descriptor: String): String {
        val close = descriptor.indexOf(')')
        return if (close >= 0 && close + 1 < descriptor.length) descriptor.substring(close + 1) else "V"
    }

    private fun methodParameterDescriptors(descriptor: String): List<String> {
        val parsed = parseMethodDescriptor(descriptor)
        if (parsed !is DescriptorParseResult.Success) return emptyList()
        return parsed.value.parameters.map(DescriptorRenderer::toDescriptor)
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

    private fun operationGenericMatches(expected: HandlerParameterSpec, actualGenericName: String): Boolean {
        val expectedDescriptor = expected.operationGenericDescriptor ?: return false
        return operationGenericMatches(expectedDescriptor, actualGenericName) ||
            expected.acceptedOperationGenericDescriptors.any {
                operationGenericMatches(it, actualGenericName)
            }
    }

    private fun operationGenericDescriptor(
        expected: HandlerParameterSpec,
        actualGenericName: String,
    ): String? {
        val expectedDescriptors = buildList {
            expected.operationGenericDescriptor?.let(::add)
            addAll(expected.acceptedOperationGenericDescriptors)
        }
        return expectedDescriptors.firstOrNull { operationGenericMatches(it, actualGenericName) }
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

    private data class WrappedInstructionTarget(
        val owner: String,
        val parameterDescriptors: List<String>,
        val isStatic: Boolean,
        val returnDescriptor: String? = null,
    ) {
        fun toWrappedOperationTarget(): WrappedOperationTarget = WrappedOperationTarget(
            owner = owner,
            parameterDescriptors = parameterDescriptors,
            returnDescriptor = returnDescriptor ?: "V",
            isStatic = isStatic,
        )
    }

    private data class WrappedOperationTarget(
        val owner: String,
        val parameterDescriptors: List<String>,
        val returnDescriptor: String,
        val isStatic: Boolean,
    )

    private data class FieldWrapTarget(
        val owner: String,
        val fieldDescriptor: String,
        val operationKind: AtTargetOperationKind,
    )

    private val fieldWrapOperationKinds = setOf(
        AtTargetOperationKind.FIELD_GET_INSTANCE,
        AtTargetOperationKind.FIELD_GET_STATIC,
        AtTargetOperationKind.FIELD_PUT_INSTANCE,
        AtTargetOperationKind.FIELD_PUT_STATIC,
    )

    private val fieldWrapWithConditionKinds = setOf(
        AtTargetOperationKind.FIELD_PUT_INSTANCE,
        AtTargetOperationKind.FIELD_PUT_STATIC,
    )

    private val fieldModifyReceiverKinds = setOf(
        AtTargetOperationKind.FIELD_GET_INSTANCE,
        AtTargetOperationKind.FIELD_PUT_INSTANCE,
    )

    private val fieldModifyExpressionValueKinds = setOf(
        AtTargetOperationKind.FIELD_GET_INSTANCE,
        AtTargetOperationKind.FIELD_GET_STATIC,
    )

    private val wrapWithConditionInvokeOperationKinds = setOf(
        AtTargetOperationKind.INVOKE_VIRTUAL,
        AtTargetOperationKind.INVOKE_STATIC,
        AtTargetOperationKind.INVOKE_SPECIAL,
        AtTargetOperationKind.INVOKE_INTERFACE,
    )

    private data class WrapWithConditionTargetResolution(
        val status: WrapWithConditionTargetStatus,
        val wrapped: WrappedInstructionTarget? = null,
    )

    private sealed interface TargetMethodResolution {
        data class Resolved(val method: MethodIndexEntry) : TargetMethodResolution
        data class Ambiguous(val candidates: List<MethodIndexEntry>) : TargetMethodResolution
        data object NotFound : TargetMethodResolution
    }

    companion object {
        private val INT_LIKE_DESCRIPTORS = setOf("I", "Z", "B", "C", "S")
        private const val CALLBACK_INFO_DESCRIPTOR =
            "Lorg/spongepowered/asm/mixin/injection/callback/CallbackInfo;"
        private const val CALLBACK_INFO_RETURNABLE_DESCRIPTOR =
            "Lorg/spongepowered/asm/mixin/injection/callback/CallbackInfoReturnable;"
        private const val OBJECT_DESCRIPTOR = "Ljava/lang/Object;"
        private const val OFFICIAL_COERCE_FQN = "org.spongepowered.asm.mixin.injection.Coerce"
        private const val OFFICIAL_OPERATION_FQN =
            "com.llamalad7.mixinextras.injector.wrapoperation.Operation"
        private const val OFFICIAL_CANCELLABLE_FQN = "com.llamalad7.mixinextras.sugar.Cancellable"

        private val OFFICIAL_SHARE_REF_VALUE_DESCRIPTORS = mapOf(
            "com/llamalad7/mixinextras/sugar/ref/LocalRef" to OBJECT_DESCRIPTOR,
            "com/llamalad7/mixinextras/sugar/ref/LocalBooleanRef" to "Z",
            "com/llamalad7/mixinextras/sugar/ref/LocalByteRef" to "B",
            "com/llamalad7/mixinextras/sugar/ref/LocalCharRef" to "C",
            "com/llamalad7/mixinextras/sugar/ref/LocalShortRef" to "S",
            "com/llamalad7/mixinextras/sugar/ref/LocalIntRef" to "I",
            "com/llamalad7/mixinextras/sugar/ref/LocalLongRef" to "J",
            "com/llamalad7/mixinextras/sugar/ref/LocalFloatRef" to "F",
            "com/llamalad7/mixinextras/sugar/ref/LocalDoubleRef" to "D",
        )

        internal fun shareRefValueDescriptor(descriptor: String): String? {
            if (!descriptor.startsWith("L") || !descriptor.endsWith(";")) return null
            return OFFICIAL_SHARE_REF_VALUE_DESCRIPTORS[descriptor.substring(1, descriptor.length - 1)]
        }

        private fun annotationNamePattern(): Regex = Regex("""@([\w.$]+)\s*\(""")

        private fun resolveMixinAnnotation(
            name: String,
            imports: JavaSourceImports,
        ): MixinAnnotation? {
            if ('.' in name) return MixinAnnotation.fromOfficialFqn(name)
            val annotation = MixinAnnotation.fromSimpleName(name) ?: return null
            val explicitImport = imports.explicit[name]
            return if (explicitImport == null) {
                annotation
            } else {
                annotation.takeIf { explicitImport in it.officialFqns }
            }
        }

        private fun resolveAnnotation(
            name: String,
            imports: JavaSourceImports,
        ): MixinExtrasAnnotation? =
            resolveMixinAnnotation(name, imports)?.let { MixinExtrasAnnotation.fromMixinAnnotation(it) }

        fun findAnnotationSites(source: String): List<MixinExtrasAnnotationSite> =
            findAnnotationSites(source, MixinExtrasAnnotation.injectorAnnotations)

        fun findSugarHandlerAnnotationSites(source: String): List<MixinExtrasAnnotationSite> =
            findAnnotationSites(source, MixinExtrasAnnotation.sugarHandlerInjectorAnnotations)

        private fun findAnnotationSites(
            source: String,
            allowedAnnotations: Set<MixinExtrasAnnotation>,
        ): List<MixinExtrasAnnotationSite> {
            val sites = mutableListOf<MixinExtrasAnnotationSite>()
            val imports = JavaTypeDescriptorResolver.importsFor(source)
            val injectorAnnotationOffsets = AnnotationContextExtractor.findInjectorAnnotationOffsets(source).toSet()
            val annotationMatches = annotationNamePattern().findAll(source)
                .filter { it.range.first in injectorAnnotationOffsets }
            annotationMatches.forEach { match ->
                val annotation = resolveAnnotation(match.groupValues[1], imports) ?: return@forEach
                if (annotation !in allowedAnnotations) return@forEach
                val annotationStart = match.range.first
                val parenStart = match.range.last
                val bodyEnd = findMatchingParen(source, parenStart) ?: return@forEach
                val body = source.substring(parenStart + 1, bodyEnd)
                val annotationEnd = bodyEnd + 1
                val methodSelectors = parseMethodSelectors(body)
                if (methodSelectors.isEmpty()) return@forEach
                val handler = parseHandlerMethod(source, methodAnnotationBlockStart(source, annotationStart), imports)
                if (annotation == MixinExtrasAnnotation.MODIFY_CONSTANT) {
                    val failClosed = hasNonDefaultModifyConstantSlice(body)
                    val constantAtArgsSets = if (failClosed) null else parseModifyConstantAtArgsSets(body)
                    for (methodAttr in methodSelectors) {
                        if (constantAtArgsSets == null) {
                            sites += MixinExtrasAnnotationSite(
                                annotation = annotation,
                                methodAttribute = methodAttr,
                                atValue = null,
                                atTarget = null,
                                atArgs = emptyList(),
                                atId = null,
                                atOrdinal = null,
                                atShift = AtShiftSpec.Before,
                                hasConstantSelector = false,
                                annotationRange = offsetRange(source, annotationStart, annotationEnd),
                                handlerMethod = handler,
                            )
                        } else {
                            for (constant in constantAtArgsSets) {
                                sites += MixinExtrasAnnotationSite(
                                    annotation = annotation,
                                    methodAttribute = methodAttr,
                                    atValue = "CONSTANT",
                                    atTarget = null,
                                    atArgs = constant.atArgs,
                                    atId = null,
                                    atOrdinal = constant.atOrdinal,
                                    atShift = AtShiftSpec.Before,
                                    hasConstantSelector = false,
                                    annotationRange = offsetRange(source, annotationStart, annotationEnd),
                                    handlerMethod = handler,
                                    expandZeroConditions = constant.expandZeroConditions,
                                )
                            }
                        }
                    }
                    return@forEach
                }
                val atInfos = extractAtInfo(body, imports)
                val hasConstantSelector = annotation == MixinExtrasAnnotation.WRAP_OPERATION &&
                    hasTopLevelConstantSelector(body)
                for (methodAttr in methodSelectors) {
                    for (atInfo in atInfos) {
                        sites += MixinExtrasAnnotationSite(
                            annotation = annotation,
                            methodAttribute = methodAttr,
                            atValue = atInfo.atValue,
                            atTarget = atInfo.atTarget,
                            atArgs = atInfo.atArgs,
                            atId = atInfo.atId,
                            atOrdinal = atInfo.atOrdinal,
                            atShift = atInfo.atShift,
                            hasConstantSelector = hasConstantSelector,
                            annotationRange = offsetRange(source, annotationStart, annotationEnd),
                            handlerMethod = handler,
                        )
                    }
                }
            }
            return sites
        }

        private fun parseMethodSelectors(body: String): List<String> {
            for (member in splitTopLevelCommas(body)) {
                val trimmed = member.text.trim()
                if (trimmed.isEmpty()) continue
                val equalsIndex = findTopLevelEquals(trimmed)
                if (equalsIndex < 0) continue
                val key = trimmed.substring(0, equalsIndex).trim()
                if (key != "method") continue
                return parseMethodSelectorValues(trimmed.substring(equalsIndex + 1).trim())
            }
            return emptyList()
        }

        private fun parseMethodSelectorValues(value: String): List<String> {
            val trimmed = value.trim()
            if (trimmed.startsWith('{')) {
                val close = trimmed.lastIndexOf('}')
                if (close <= 0) return emptyList()
                return splitTopLevelCommas(trimmed.substring(1, close))
                    .mapNotNull { parseStringLiteral(it.text.trim()) }
            }
            return parseStringOrStringArray(trimmed).toList()
        }

        private fun hasNonDefaultModifyConstantSlice(body: String): Boolean {
            for (member in splitTopLevelCommas(body)) {
                val trimmed = member.text.trim()
                if (trimmed.isEmpty()) continue
                val equalsIndex = findTopLevelEquals(trimmed)
                if (equalsIndex < 0) continue
                val key = trimmed.substring(0, equalsIndex).trim()
                if (key != "slice") continue
                return !isDefaultEmptySlice(trimmed.substring(equalsIndex + 1).trim())
            }
            return false
        }

        private data class ParsedConstantAnnotation(
            val atArgs: List<String>,
            val atOrdinal: Int?,
            val expandZeroConditions: Set<Int> = emptySet(),
        )

        private fun isDefaultEmptySlice(value: String): Boolean {
            val trimmed = value.trim()
            return Regex("""@Slice\s*\(\s*\)""").matches(trimmed) || trimmed == "{}"
        }

        private fun parseModifyConstantAtArgsSets(body: String): List<ParsedConstantAnnotation>? {
            for (member in splitTopLevelCommas(body)) {
                val trimmed = member.text.trim()
                if (trimmed.isEmpty()) continue
                val equalsIndex = findTopLevelEquals(trimmed)
                if (equalsIndex < 0) continue
                val key = trimmed.substring(0, equalsIndex).trim()
                if (key != "constant") continue
                return parseConstantAnnotationOrArray(trimmed.substring(equalsIndex + 1).trim())
            }
            return listOf(ParsedConstantAnnotation(emptyList(), null))
        }

        private fun parseConstantAnnotationOrArray(value: String): List<ParsedConstantAnnotation>? {
            val trimmed = value.trim()
            if (trimmed.startsWith('{')) {
                val close = trimmed.lastIndexOf('}')
                if (close <= 0) return null
                val elements = splitTopLevelCommas(trimmed.substring(1, close))
                if (elements.isEmpty() || elements.all { it.text.trim().isEmpty() }) {
                    return listOf(ParsedConstantAnnotation(emptyList(), null))
                }
                val results = mutableListOf<ParsedConstantAnnotation>()
                for (element in elements) {
                    val elementTrimmed = element.text.trim()
                    if (elementTrimmed.isEmpty()) return null
                    val parsed = readConstantAnnotationAtArgs(elementTrimmed) ?: return null
                    results += parsed
                }
                return results
            }
            return readConstantAnnotationAtArgs(trimmed)?.let { listOf(it) }
        }

        private fun parseExpandZeroConditions(value: String): Set<Int>? {
            val trimmed = value.trim()
            if (trimmed.startsWith('{')) {
                val close = findMatchingCloseBrace(trimmed, 0) ?: return null
                if (skipAtBodyWhitespace(trimmed, close + 1) < trimmed.length) return null
                val elements = splitTopLevelCommas(trimmed.substring(1, close))
                if (elements.size == 1 && elements.single().text.trim().isEmpty()) {
                    return emptySet()
                }
                val opcodes = mutableSetOf<Int>()
                for (element in elements) {
                    val condition = element.text.trim()
                    if (condition.isEmpty()) return null
                    opcodes += parseExpandZeroCondition(condition) ?: return null
                }
                return opcodes
            }
            return parseExpandZeroCondition(trimmed)
        }

        private fun parseExpandZeroCondition(value: String): Set<Int>? {
            val parsed = readAtEnumConstant(value, 0) ?: return null
            if (skipAtBodyWhitespace(value, parsed.end) < value.length) return null
            return when (parsed.value.substringAfterLast('.')) {
                "LESS_THAN_ZERO" -> setOf(Opcodes.IFLT, Opcodes.IFGE)
                "LESS_THAN_OR_EQUAL_TO_ZERO" -> setOf(Opcodes.IFLE, Opcodes.IFGT)
                "GREATER_THAN_ZERO" -> setOf(Opcodes.IFGT, Opcodes.IFLE)
                "GREATER_THAN_OR_EQUAL_TO_ZERO" -> setOf(Opcodes.IFGE, Opcodes.IFLT)
                else -> null
            }
        }

        private fun readConstantAnnotationAtArgs(source: String): ParsedConstantAnnotation? {
            var index = source.indexOfFirst { !it.isWhitespace() }
            if (index < 0 || source[index] != '@') return null
            index++
            val nameStart = index
            while (index < source.length && (source[index].isLetterOrDigit() || source[index] in "._$")) {
                index++
            }
            if (index == nameStart || !isConstantAnnotationName(source.substring(nameStart, index))) {
                return null
            }
            while (index < source.length && source[index].isWhitespace()) {
                index++
            }
            if (source.getOrNull(index) != '(') return null
            val closeParen = findMatchingParen(source, index) ?: return null
            if (skipAtBodyWhitespace(source, closeParen + 1) < source.length) return null
            return normalizeConstantAtArgsFromBody(source.substring(index + 1, closeParen))
        }

        private fun isConstantAnnotationName(qualifiedName: String): Boolean =
            qualifiedName.substringAfterLast('.') == "Constant"

        private fun normalizeConstantAtArgsFromBody(body: String): ParsedConstantAnnotation? {
            if (body.isBlank()) return ParsedConstantAnnotation(emptyList(), null)
            val args = mutableListOf<String>()
            var atOrdinal: Int? = null
            var expandZeroConditions = emptySet<Int>()
            for (member in splitTopLevelCommas(body)) {
                val trimmed = member.text.trim()
                if (trimmed.isEmpty()) continue
                val equalsIndex = findTopLevelEquals(trimmed)
                if (equalsIndex < 0) return null
                val key = trimmed.substring(0, equalsIndex).trim()
                val rawValue = trimmed.substring(equalsIndex + 1).trim()
                when (key) {
                    "slice" -> {
                        val parsed = parseStringLiteral(rawValue) ?: return null
                        if (parsed.isNotEmpty()) return null
                    }
                    "ordinal" -> {
                        val parsed = readIntegerLiteral(rawValue) ?: return null
                        val ordinalValue = parsed.toIntOrNull() ?: return null
                        if (ordinalValue < -1) return null
                        atOrdinal = ordinalValue.takeUnless { it == -1 }
                    }
                    "log" -> {
                        val normalized = rawValue.trim()
                        if (!normalized.equals("true", ignoreCase = true) &&
                            !normalized.equals("false", ignoreCase = true)
                        ) {
                            return null
                        }
                    }
                    "intValue" -> {
                        val parsed = readIntegerLiteral(rawValue) ?: return null
                        args += "intValue=$parsed"
                    }
                    "longValue" -> {
                        val parsed = readLongLiteral(rawValue) ?: return null
                        args += "longValue=$parsed"
                    }
                    "floatValue" -> {
                        val parsed = readFloatLiteral(rawValue) ?: return null
                        args += "floatValue=$parsed"
                    }
                    "doubleValue" -> {
                        val parsed = readDoubleLiteral(rawValue) ?: return null
                        args += "doubleValue=$parsed"
                    }
                    "stringValue" -> {
                        val parsed = parseStringLiteral(rawValue) ?: return null
                        args += "stringValue=$parsed"
                    }
                    "classValue" -> {
                        val className = parseClassLiteral(rawValue) ?: return null
                        args += "classValue=$className"
                    }
                    "nullValue" -> {
                        val normalized = rawValue.trim()
                        when {
                            normalized.equals("true", ignoreCase = true) -> args += "nullValue=true"
                            normalized.equals("false", ignoreCase = true) -> return null
                            else -> return null
                        }
                    }
                    "expandZeroConditions" -> {
                        expandZeroConditions = parseExpandZeroConditions(rawValue) ?: return null
                    }
                    else -> return null
                }
            }
            if (args.isEmpty()) return ParsedConstantAnnotation(emptyList(), atOrdinal, expandZeroConditions)
            return if (ConstantAtArgsParser.parse(args) != null) {
                ParsedConstantAnnotation(args, atOrdinal, expandZeroConditions)
            } else {
                null
            }
        }

        private fun readIntegerLiteral(value: String): String? {
            val trimmed = value.trim()
            return readAtDecimalIntLiteral(trimmed, 0)?.value?.toString()
                ?: trimmed.toIntOrNull()?.toString()
        }

        private fun readLongLiteral(value: String): String? {
            val trimmed = value.trim()
            val normalized = when {
                trimmed.endsWith("L") -> trimmed.dropLast(1)
                trimmed.endsWith("l") -> trimmed.dropLast(1)
                else -> trimmed
            }
            return normalized.toLongOrNull()?.toString()
        }

        private fun readFloatLiteral(value: String): String? {
            val trimmed = value.trim()
            val normalized = when {
                trimmed.endsWith("f") -> trimmed.dropLast(1)
                trimmed.endsWith("F") -> trimmed.dropLast(1)
                else -> trimmed
            }
            return normalized.toFloatOrNull()?.toString()
        }

        private fun readDoubleLiteral(value: String): String? {
            val trimmed = value.trim()
            val normalized = when {
                trimmed.endsWith("d") -> trimmed.dropLast(1)
                trimmed.endsWith("D") -> trimmed.dropLast(1)
                else -> trimmed
            }
            return normalized.toDoubleOrNull()?.toString()
        }

        private fun hasTopLevelConstantSelector(body: String): Boolean {
            var depth = 0
            var inString = false
            var i = 0
            while (i < body.length) {
                when {
                    inString -> {
                        if (body[i] == '\\') {
                            i += 2
                            continue
                        }
                        if (body[i] == '"') inString = false
                        i++
                    }
                    body[i] == '"' -> {
                        inString = true
                        i++
                    }
                    body[i] == '(' -> {
                        depth++
                        i++
                    }
                    body[i] == ')' -> {
                        depth = (depth - 1).coerceAtLeast(0)
                        i++
                    }
                    depth == 0 && body[i] == 'c' -> {
                        val match = Regex("""\bconstant\s*=\s*@Constant\b""").find(body, i)
                        if (match != null && match.range.first == i) return true
                        i++
                    }
                    else -> i++
                }
            }
            return false
        }

        private data class AtInfo(
            val atValue: String?,
            val atTarget: String?,
            val atArgs: List<String>,
            val atId: String?,
            val atOrdinal: Int?,
            val atShift: AtShiftSpec,
        )

        private enum class AtShiftKind {
            BEFORE,
            AFTER,
            BY,
        }

        private sealed interface AtByParseResult {
            data object Missing : AtByParseResult
            data object Invalid : AtByParseResult
            data class Valid(val value: Int) : AtByParseResult
        }

        private fun defaultAtInfo(): AtInfo =
            AtInfo(null, null, emptyList(), null, null, AtShiftSpec.Before)

        private fun extractAtInfo(body: String, imports: JavaSourceImports): List<AtInfo> {
            val atValueSource = splitTopLevelCommas(body)
                .asSequence()
                .mapNotNull { member ->
                    val memberText = member.text.trim()
                    val memberStart = skipAtBodyWhitespace(memberText, 0)
                    val memberWithoutLeadingComments = memberText.substring(memberStart)
                    val equalsIndex = findTopLevelEquals(memberWithoutLeadingComments)
                    if (
                        equalsIndex < 0 ||
                        memberWithoutLeadingComments.substring(0, equalsIndex).trim() != "at"
                    ) {
                        null
                    } else {
                        memberWithoutLeadingComments.substring(equalsIndex + 1).trim()
                    }
                }
                .firstOrNull()
                ?: return listOf(defaultAtInfo())
            val lexicalAtOffsets = AnnotationContextExtractor
                .findAnnotationOffsets(atValueSource, MixinAnnotation.AT)
                .toSet()
            val atMatches = annotationNamePattern().findAll(atValueSource).filter { match ->
                match.range.first in lexicalAtOffsets &&
                resolveMixinAnnotation(match.groupValues[1], imports) == MixinAnnotation.AT
            }.toList()
            if (atMatches.isEmpty()) return listOf(defaultAtInfo())
            return atMatches.map { atMatch -> extractSingleAtInfo(atValueSource, atMatch) }
        }

        private fun extractSingleAtInfo(source: String, atMatch: MatchResult): AtInfo {
            val paren = atMatch.range.last
            val close = findMatchingParen(source, paren)
                ?: return defaultAtInfo()
            val atBody = source.substring(paren + 1, close)
            var atValue: String? = null
            var atTarget: String? = null
            var atArgs = emptyList<String>()
            var atId: String? = null
            var atOrdinal: Int? = null
            var atShiftRaw: String? = null
            var atShiftMalformed = false
            var atByRaw: AtByParseResult = AtByParseResult.Missing
            var index = skipAtBodyWhitespace(atBody, 0)
            if (atBody.getOrNull(index) == '"') {
                val shorthand = readAtStringLiteral(atBody, index)
                if (shorthand != null) {
                    atValue = shorthand.value
                    index = skipAtBodyWhitespace(atBody, shorthand.end)
                    if (atBody.getOrNull(index) == ',') {
                        index++
                    }
                }
            }
            while (index < atBody.length) {
                index = skipAtBodyWhitespace(atBody, index)
                if (index >= atBody.length) break
                val attribute = readAtIdentifier(atBody, index) ?: break
                index += attribute.length
                index = skipAtBodyWhitespace(atBody, index)
                if (atBody.getOrNull(index) != '=') break
                index++
                index = skipAtBodyWhitespace(atBody, index)
                when (attribute) {
                    "value" -> atValue = readAtStringLiteral(atBody, index)?.also { index = it.end }?.value
                    "target" -> atTarget = readAtStringLiteral(atBody, index)?.also { index = it.end }?.value
                    "args" -> parseAtStringOrStringArray(atBody, index)?.let { parsed ->
                        atArgs = parsed.values
                        index = parsed.end
                    } ?: break
                    "id" -> atId = readAtStringLiteral(atBody, index)?.also { index = it.end }?.value
                    "ordinal" -> {
                        val parsed = readAtDecimalIntLiteral(atBody, index)
                        if (parsed != null) {
                            atOrdinal = parsed.value.takeUnless { it == -1 }
                            index = parsed.end
                        } else {
                            index = skipAtAnnotationValue(atBody, index)
                        }
                    }
                    "shift" -> {
                        val parsed = readAtEnumConstant(atBody, index)
                        if (parsed != null) {
                            atShiftRaw = parsed.value
                            index = parsed.end
                        } else {
                            atShiftMalformed = true
                            index = skipAtAnnotationValue(atBody, index)
                        }
                    }
                    "by" -> {
                        val parsed = readAtDecimalIntLiteral(atBody, index)
                        if (parsed != null) {
                            atByRaw = AtByParseResult.Valid(parsed.value)
                            index = parsed.end
                        } else {
                            atByRaw = AtByParseResult.Invalid
                            index = skipAtAnnotationValue(atBody, index)
                        }
                    }
                    else -> index = skipAtAnnotationValue(atBody, index)
                }
                index = skipAtBodyWhitespace(atBody, index)
                if (atBody.getOrNull(index) == ',') {
                    index++
                }
            }
            return AtInfo(
                atValue,
                atTarget,
                atArgs,
                atId,
                atOrdinal,
                resolveAtShift(atShiftRaw, atShiftMalformed, atByRaw),
            )
        }

        private fun resolveAtShift(
            shiftRaw: String?,
            shiftMalformed: Boolean,
            byRaw: AtByParseResult,
        ): AtShiftSpec {
            if (shiftMalformed) return AtShiftSpec.Unresolved
            if (shiftRaw == null) return AtShiftSpec.Before
            return when (parseAtShiftKind(shiftRaw)) {
                AtShiftKind.BEFORE -> AtShiftSpec.Before
                AtShiftKind.AFTER -> AtShiftSpec.After
                AtShiftKind.BY -> when (byRaw) {
                    is AtByParseResult.Valid -> AtShiftSpec.By(byRaw.value)
                    AtByParseResult.Missing -> AtShiftSpec.By(0)
                    AtByParseResult.Invalid -> AtShiftSpec.Unresolved
                }
                null -> AtShiftSpec.Unresolved
            }
        }

        private fun parseAtShiftKind(raw: String): AtShiftKind? {
            val simple = raw.substringAfterLast('.')
            if (!raw.contains('.')) {
                return when (raw) {
                    "BEFORE" -> AtShiftKind.BEFORE
                    "AFTER" -> AtShiftKind.AFTER
                    "BY" -> AtShiftKind.BY
                    else -> null
                }
            }
            val typePart = raw.substringBeforeLast('.')
            if (typePart != "At.Shift" && !typePart.endsWith(".At.Shift")) {
                return null
            }
            return when (simple) {
                "BEFORE" -> AtShiftKind.BEFORE
                "AFTER" -> AtShiftKind.AFTER
                "BY" -> AtShiftKind.BY
                else -> null
            }
        }

        private data class AtParsedStrings(val values: List<String>, val end: Int)

        private data class AtStringLiteral(val value: String, val end: Int)

        private data class AtIntLiteral(val value: Int, val end: Int)

        private data class AtEnumConstant(val value: String, val end: Int)

        private fun readAtEnumConstant(source: String, start: Int): AtEnumConstant? {
            var index = skipAtBodyWhitespace(source, start)
            if (index >= source.length || !source[index].isJavaIdentifierStart()) return null
            val valueStart = index
            index++
            while (index < source.length) {
                when {
                    source[index].isJavaIdentifierPart() -> index++
                    source[index] == '.' -> {
                        if (index + 1 >= source.length || !source[index + 1].isJavaIdentifierStart()) return null
                        index++
                    }
                    else -> break
                }
            }
            val value = source.substring(valueStart, index)
            if (value.isEmpty()) return null
            return AtEnumConstant(value, index)
        }

        private fun readAtDecimalIntLiteral(source: String, start: Int): AtIntLiteral? {
            var index = skipAtBodyWhitespace(source, start)
            if (index >= source.length) return null
            var negative = false
            when (source[index]) {
                '-' -> {
                    negative = true
                    index++
                }
                '+' -> index++
            }
            if (index >= source.length || !source[index].isDigit()) return null
            var value = 0L
            val digitStart = index
            while (index < source.length && source[index].isDigit()) {
                value = value * 10 + (source[index] - '0')
                if (value > Int.MAX_VALUE.toLong() + if (negative) 1 else 0) return null
                index++
            }
            if (index == digitStart) return null
            if (source.getOrNull(index) == '.') return null
            if (index < source.length) {
                val ch = source[index]
                if (ch.isLetterOrDigit() || ch == '_') return null
            }
            val intValue = if (negative) -value else value
            if (intValue < Int.MIN_VALUE || intValue > Int.MAX_VALUE) return null
            return AtIntLiteral(intValue.toInt(), index)
        }

        private fun parseAtStringOrStringArray(source: String, start: Int): AtParsedStrings? {
            val index = skipAtBodyWhitespace(source, start)
            if (index >= source.length) return null
            return when (source[index]) {
                '"' -> readAtStringLiteral(source, index)?.let { AtParsedStrings(listOf(it.value), it.end) }
                '{' -> parseAtStringArray(source, index)
                else -> null
            }
        }

        private fun parseAtStringArray(source: String, openBrace: Int): AtParsedStrings? {
            val closeBrace = findMatchingCloseBrace(source, openBrace) ?: return null
            val elements = splitTopLevelCommas(source.substring(openBrace + 1, closeBrace))
            val values = mutableListOf<String>()
            for (element in elements) {
                val trimmedStart = skipAtBodyWhitespace(element.text, 0)
                if (trimmedStart >= element.text.length) {
                    if (elements.size > 1) return null
                    continue
                }
                val literal = readAtStringLiteral(element.text, trimmedStart) ?: return null
                if (skipAtBodyWhitespace(element.text, literal.end) < element.text.length) return null
                values += literal.value
            }
            return AtParsedStrings(values, closeBrace + 1)
        }

        private fun readAtStringLiteral(source: String, start: Int): AtStringLiteral? {
            if (source.getOrNull(start) != '"') return null
            val builder = StringBuilder()
            var index = start + 1
            while (index < source.length) {
                when {
                    source[index] == '\\' -> {
                        if (index + 1 >= source.length) return null
                        when (source[index + 1]) {
                            '\\' -> builder.append('\\')
                            '"' -> builder.append('"')
                            else -> {
                                builder.append('\\')
                                builder.append(source[index + 1])
                            }
                        }
                        index += 2
                    }
                    source[index] == '"' -> return AtStringLiteral(builder.toString(), index + 1)
                    else -> {
                        builder.append(source[index])
                        index++
                    }
                }
            }
            return null
        }

        private fun readAtIdentifier(source: String, start: Int): String? {
            if (start >= source.length || !source[start].isJavaIdentifierStart()) return null
            var index = start + 1
            while (index < source.length && source[index].isJavaIdentifierPart()) {
                index++
            }
            return source.substring(start, index)
        }

        private fun skipAtBodyWhitespace(source: String, start: Int): Int {
            var index = start
            while (index < source.length) {
                when {
                    source[index].isWhitespace() -> index++
                    isAtLineCommentStart(source, index) -> index = skipAtLineComment(source, index)
                    isAtBlockCommentStart(source, index) -> index = skipAtBlockComment(source, index)
                    else -> return index
                }
            }
            return index
        }

        private fun isAtLineCommentStart(source: String, index: Int): Boolean =
            source.getOrNull(index) == '/' && source.getOrNull(index + 1) == '/'

        private fun isAtBlockCommentStart(source: String, index: Int): Boolean =
            source.getOrNull(index) == '/' && source.getOrNull(index + 1) == '*'

        private fun skipAtLineComment(source: String, start: Int): Int {
            var index = start + 2
            while (index < source.length && source[index] != '\n') {
                index++
            }
            return index
        }

        private fun skipAtBlockComment(source: String, start: Int): Int {
            var index = start + 2
            while (index + 1 < source.length) {
                if (source[index] == '*' && source[index + 1] == '/') {
                    return index + 2
                }
                index++
            }
            return source.length
        }

        private fun skipAtAnnotationValue(source: String, start: Int): Int {
            var index = skipAtBodyWhitespace(source, start)
            if (index >= source.length) return index
            return when (source[index]) {
                '"' -> readAtStringLiteral(source, index)?.end ?: (index + 1)
                '{' -> findMatchingCloseBrace(source, index)?.plus(1) ?: (index + 1)
                '(' -> findMatchingParen(source, index)?.plus(1) ?: (index + 1)
                '@' -> skipAtNestedAnnotation(source, index)
                else -> skipAtBareAnnotationValue(source, index)
            }
        }

        private fun skipAtNestedAnnotation(source: String, start: Int): Int {
            if (source.getOrNull(start) != '@') return start + 1
            var index = start + 1
            while (index < source.length && (source[index].isLetterOrDigit() || source[index] in "._$")) {
                index++
            }
            if (source.getOrNull(index) != '(') return index
            return findMatchingParen(source, index)?.plus(1) ?: (index + 1)
        }

        private fun skipAtBareAnnotationValue(source: String, start: Int): Int {
            var index = start
            var genericDepth = 0
            var parenDepth = 0
            while (index < source.length) {
                when {
                    isAtLineCommentStart(source, index) -> index = skipAtLineComment(source, index)
                    isAtBlockCommentStart(source, index) -> index = skipAtBlockComment(source, index)
                    source[index] == '<' -> {
                        genericDepth++
                        index++
                    }
                    source[index] == '>' -> {
                        genericDepth = (genericDepth - 1).coerceAtLeast(0)
                        index++
                    }
                    source[index] == '(' -> {
                        parenDepth++
                        index++
                    }
                    source[index] == ')' -> {
                        parenDepth = (parenDepth - 1).coerceAtLeast(0)
                        index++
                    }
                    source[index] == ',' && genericDepth == 0 && parenDepth == 0 -> return index
                    source[index] == '}' && genericDepth == 0 && parenDepth == 0 -> return index
                    else -> index++
                }
            }
            return index
        }

        private fun findMatchingCloseBrace(source: String, openIndex: Int): Int? {
            if (source.getOrNull(openIndex) != '{') return null
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
                    source[i] == '{' -> depth++
                    source[i] == '}' -> {
                        depth--
                        if (depth == 0) return i
                    }
                }
                i++
            }
            return null
        }

        fun parseHandlerMethod(
            source: String,
            startOffset: Int,
            imports: JavaSourceImports = JavaTypeDescriptorResolver.importsFor(source),
        ): HandlerMethodDeclaration? {
            var offset = startOffset
            var hasMethodLevelCoerce = false
            while (offset < source.length) {
                offset = skipAtBodyWhitespace(source, offset)
                if (offset >= source.length) return null
                if (source[offset] == '@') {
                    val annotationName = readAnnotationQualifiedName(source, offset + 1).first
                    hasMethodLevelCoerce = hasMethodLevelCoerce ||
                        isOfficialCoerceAnnotation(annotationName, imports)
                    val annotationEnd = skipAnnotation(source, offset) ?: return null
                    offset = annotationEnd
                    continue
                }
                break
            }
            if (offset >= source.length) return null
            val slice = source.substring(offset)
            val maskedSlice = AnnotationContextExtractor.maskNonCode(slice)
            val headerMatch = Regex(
                """^(?:(?:public|protected|private|static|final|synchronized|native|strictfp|default)\s+)*([\w<>\[\].?,]+)\s+([\w$]+)\s*\(""",
            ).find(maskedSlice) ?: return null
            val isStatic = Regex("""\bstatic\s+""").containsMatchIn(headerMatch.value)
            val returnTypeName = headerMatch.groupValues[1]
            val methodName = headerMatch.groupValues[2]
            val parenStart = headerMatch.range.last
            val parenEnd = findMatchingParen(slice, parenStart) ?: return null
            val paramsRaw = slice.substring(parenStart + 1, parenEnd)
            val parameters = parseParameters(source, offset + parenStart + 1, paramsRaw, imports)
            val end = offset + parenEnd + 1
            return HandlerMethodDeclaration(
                methodName = methodName,
                returnTypeName = returnTypeName,
                returnTypeDescriptor = null,
                parameters = parameters,
                range = offsetRange(source, offset, end),
                isStatic = isStatic,
                hasMethodLevelCoerce = hasMethodLevelCoerce,
            )
        }

        private fun parseParameters(
            source: String,
            paramsStartOffset: Int,
            paramsRaw: String,
            imports: JavaSourceImports,
        ): List<HandlerParameterDeclaration> {
            if (paramsRaw.isBlank()) return emptyList()
            return splitTopLevelCommas(paramsRaw).mapNotNull { segment ->
                val trimmed = segment.text.trim()
                if (trimmed.isEmpty()) return@mapNotNull null
                val trimStartInSegment = segment.text.indexOfFirst { !it.isWhitespace() }
                val trimEndInSegment = segment.text.indexOfLast { !it.isWhitespace() } + 1
                val paramStartAbs = paramsStartOffset + segment.start + trimStartInSegment
                val paramEndAbs = paramsStartOffset + segment.start + trimEndInSegment
                val sugarStrip = stripSugarAnnotations(trimmed, imports)
                val withoutSugarAnnotations = sugarStrip.text
                val parseable = AnnotationContextExtractor.maskNonCode(withoutSugarAnnotations).trim()
                val genericOp = Regex("""Operation\s*<\s*([\w<>\[\].]+)\s*>""").find(parseable)
                val erasedTypeName = parseable.substringBeforeLast(' ').trim()
                val isOperation = genericOp != null ||
                    parseable.contains("Operation<") ||
                    erasedTypeName == "Operation" ||
                    erasedTypeName == OFFICIAL_OPERATION_FQN
                val typeName = genericOp?.let { "Operation<${it.groupValues[1]}>" }
                    ?: parseable.substringBeforeLast(' ').trim()
                val name = parseable.substringAfterLast(' ').trim()
                val sugarAnnotationRange = sugarStrip.sugarAnnotationRange?.let { sugarRange ->
                    offsetRange(
                        source,
                        paramStartAbs + sugarRange.first,
                        paramStartAbs + sugarRange.last + 1,
                    )
                }
                val coerceAnnotationRange = sugarStrip.coerceAnnotationRange?.let { coerceRange ->
                    offsetRange(
                        source,
                        paramStartAbs + coerceRange.first,
                        paramStartAbs + coerceRange.last + 1,
                    )
                }
                HandlerParameterDeclaration(
                    name = name,
                    typeName = typeName,
                    typeDescriptor = null,
                    isOperation = isOperation,
                    operationGenericName = genericOp?.groupValues?.get(1),
                    isSugar = sugarStrip.isSugar,
                    sugarSpec = sugarStrip.sugarSpec,
                    range = offsetRange(source, paramStartAbs, paramEndAbs),
                    sugarAnnotationRange = sugarAnnotationRange,
                    hasCoerce = sugarStrip.hasCoerce,
                    coerceAnnotationRange = coerceAnnotationRange,
                )
            }
        }

        private data class TopLevelCommaSegment(
            val text: String,
            val start: Int,
            val end: Int,
        )

        private fun splitTopLevelCommas(value: String): List<TopLevelCommaSegment> {
            val results = mutableListOf<TopLevelCommaSegment>()
            var start = 0
            var genericDepth = 0
            var parenDepth = 0
            var braceDepth = 0
            var inString = false
            var i = 0
            while (i < value.length) {
                when {
                    inString -> {
                        if (value[i] == '\\' && i + 1 < value.length) {
                            i += 2
                            continue
                        }
                        if (value[i] == '"') inString = false
                    }
                    value[i] == '"' -> inString = true
                    value[i] == '<' -> genericDepth++
                    value[i] == '>' -> genericDepth = (genericDepth - 1).coerceAtLeast(0)
                    value[i] == '(' -> parenDepth++
                    value[i] == ')' -> parenDepth = (parenDepth - 1).coerceAtLeast(0)
                    value[i] == '{' -> braceDepth++
                    value[i] == '}' -> braceDepth = (braceDepth - 1).coerceAtLeast(0)
                    value[i] == ',' && genericDepth == 0 && parenDepth == 0 && braceDepth == 0 && !inString -> {
                        results += TopLevelCommaSegment(value.substring(start, i), start, i)
                        start = i + 1
                    }
                }
                i++
            }
            results += TopLevelCommaSegment(value.substring(start), start, value.length)
            return results
        }

        private data class SugarStripResult(
            val text: String,
            val isSugar: Boolean,
            val sugarSpec: HandlerParameterSugarSpec?,
            val sugarAnnotationRange: IntRange? = null,
            val hasCoerce: Boolean = false,
            val coerceAnnotationRange: IntRange? = null,
        )

        private enum class SugarAnnotationKind {
            LOCAL,
            SHARE,
            CANCELLABLE,
        }

        private fun stripSugarAnnotations(parameter: String, imports: JavaSourceImports): SugarStripResult {
            var isSugar = false
            var sugarSpec: HandlerParameterSugarSpec? = null
            var sugarAnnotationRange: IntRange? = null
            var hasCoerce = false
            var coerceAnnotationRange: IntRange? = null
            val result = StringBuilder()
            var i = 0
            while (i < parameter.length) {
                if (parameter[i] == '@') {
                    val atPos = i
                    val annotationStart = i + 1
                    val (annotationName, nameEnd) = readAnnotationQualifiedName(parameter, annotationStart)
                    i = nameEnd
                    if (isOfficialCoerceAnnotation(annotationName, imports)) {
                        hasCoerce = true
                        var coerceEnd = nameEnd
                        while (i < parameter.length && parameter[i].isWhitespace()) {
                            i++
                        }
                        if (parameter.getOrNull(i) == '(') {
                            val close = findMatchingParen(parameter, i)
                            if (close != null) {
                                i = close + 1
                                coerceEnd = i
                            }
                        }
                        coerceAnnotationRange = coerceAnnotationRange ?: (atPos until coerceEnd)
                        result.append(' ')
                        continue
                    }
                    val sugarKind = sugarAnnotationKind(annotationName, imports)
                    if (sugarKind != null) {
                        isSugar = true
                        var sugarEnd = nameEnd
                        while (i < parameter.length && parameter[i].isWhitespace()) {
                            i++
                        }
                        sugarSpec = if (parameter.getOrNull(i) == '(') {
                            val close = findMatchingParen(parameter, i)
                            if (close != null) {
                                val body = parameter.substring(i + 1, close)
                                i = close + 1
                                sugarEnd = i
                                parseSugarSpec(sugarKind, body)
                            } else {
                                i++
                                while (i < parameter.length && !parameter[i].isWhitespace()) {
                                    i++
                                }
                                sugarEnd = i
                                defaultSugarSpec(sugarKind)
                            }
                        } else {
                            defaultSugarSpec(sugarKind)
                        }
                        sugarAnnotationRange = atPos until sugarEnd
                        result.append(' ')
                        continue
                    }
                    result.append('@')
                    result.append(annotationName)
                    continue
                }
                result.append(parameter[i])
                i++
            }
            return SugarStripResult(
                text = result.toString().trim(),
                isSugar = isSugar,
                sugarSpec = sugarSpec,
                sugarAnnotationRange = sugarAnnotationRange,
                hasCoerce = hasCoerce,
                coerceAnnotationRange = coerceAnnotationRange,
            )
        }

        private fun readAnnotationQualifiedName(source: String, start: Int): Pair<String, Int> {
            var i = start
            while (i < source.length) {
                when (val ch = source[i]) {
                    '.', '_' -> i++
                    else -> if (ch.isLetterOrDigit()) i++ else break
                }
            }
            return source.substring(start, i) to i
        }

        private fun methodAnnotationBlockStart(source: String, annotationStart: Int): Int {
            var blockStart = annotationStart
            var cursor = annotationStart
            while (true) {
                val previousEnd = skipBackwardWhitespaceAndComments(source, cursor)
                if (previousEnd <= 0) return blockStart
                var candidate = source.lastIndexOf('@', previousEnd - 1)
                var previousStart: Int? = null
                while (candidate >= 0) {
                    if (skipAnnotation(source, candidate) == previousEnd) {
                        previousStart = candidate
                        break
                    }
                    candidate = source.lastIndexOf('@', candidate - 1)
                }
                if (previousStart == null) return blockStart
                blockStart = previousStart
                cursor = previousStart
            }
        }

        private fun skipBackwardWhitespaceAndComments(source: String, end: Int): Int {
            var position = end
            while (position > 0) {
                while (position > 0 && source[position - 1].isWhitespace()) {
                    position--
                }
                if (position >= 2 && source[position - 2] == '*' && source[position - 1] == '/') {
                    val open = source.lastIndexOf("/*", position - 2)
                    if (open < 0) break
                    position = open
                    continue
                }
                val lineStart = source.lastIndexOf('\n', position - 1).let { if (it < 0) 0 else it + 1 }
                if (source.substring(lineStart, position).trimStart().startsWith("//")) {
                    position = lineStart
                    continue
                }
                break
            }
            return position
        }

        private fun isOfficialCoerceAnnotation(
            name: String,
            imports: JavaSourceImports,
        ): Boolean {
            if ('.' in name) return name == OFFICIAL_COERCE_FQN
            val explicitImport = imports.explicit[name]
            return if (explicitImport == null) {
                name == "Coerce"
            } else {
                explicitImport == OFFICIAL_COERCE_FQN
            }
        }

        private fun sugarAnnotationKind(
            qualifiedName: String,
            imports: JavaSourceImports,
        ): SugarAnnotationKind? {
            when (resolveMixinAnnotation(qualifiedName, imports)) {
                MixinAnnotation.LOCAL -> return SugarAnnotationKind.LOCAL
                MixinAnnotation.SHARE -> return SugarAnnotationKind.SHARE
                else -> Unit
            }
            val canonicalName = when {
                '.' in qualifiedName -> qualifiedName
                else -> imports.explicit[qualifiedName]
            }
            if (canonicalName == OFFICIAL_CANCELLABLE_FQN) return SugarAnnotationKind.CANCELLABLE
            return if (qualifiedName == "Cancellable" && qualifiedName !in imports.explicit) {
                SugarAnnotationKind.CANCELLABLE
            } else {
                null
            }
        }

        private fun defaultSugarSpec(kind: SugarAnnotationKind): HandlerParameterSugarSpec = when (kind) {
            SugarAnnotationKind.LOCAL -> HandlerParameterSugarSpec.Local()
            SugarAnnotationKind.SHARE -> HandlerParameterSugarSpec.Share()
            SugarAnnotationKind.CANCELLABLE -> HandlerParameterSugarSpec.Cancellable
        }

        private fun parseSugarSpec(kind: SugarAnnotationKind, body: String): HandlerParameterSugarSpec =
            runCatching {
                when (kind) {
                    SugarAnnotationKind.LOCAL -> parseLocalSugarSpec(body)
                    SugarAnnotationKind.SHARE -> parseShareSugarSpec(body)
                    SugarAnnotationKind.CANCELLABLE -> HandlerParameterSugarSpec.Cancellable
                }
            }.getOrDefault(defaultSugarSpec(kind))

        private fun parseLocalSugarSpec(body: String): HandlerParameterSugarSpec.Local {
            var argsOnly = false
            var index: Int? = null
            var ordinal: Int? = null
            val names = mutableSetOf<String>()
            var print = false
            var typeClassName: String? = null
            if (body.isBlank()) {
                return HandlerParameterSugarSpec.Local()
            }
            for (member in splitTopLevelCommas(body)) {
                val trimmed = member.text.trim()
                if (trimmed.isEmpty()) continue
                val equalsIndex = findTopLevelEquals(trimmed)
                if (equalsIndex < 0) continue
                val key = trimmed.substring(0, equalsIndex).trim()
                val value = trimmed.substring(equalsIndex + 1).trim()
                when (key) {
                    "argsOnly" -> argsOnly = parseBooleanLiteral(value)
                    "index" -> index = parseOptionalIntLiteral(value)
                    "ordinal" -> ordinal = parseOptionalIntLiteral(value)
                    "name" -> names.addAll(parseStringOrStringArray(value))
                    "print" -> print = parseBooleanLiteral(value)
                    "type" -> parseClassLiteral(value)?.let { typeClassName = it }
                }
            }
            return HandlerParameterSugarSpec.Local(
                argsOnly = argsOnly,
                index = index,
                ordinal = ordinal,
                names = names,
                print = print,
                typeClassName = typeClassName,
            )
        }

        private fun parseShareSugarSpec(body: String): HandlerParameterSugarSpec.Share {
            val trimmed = body.trim()
            if (trimmed.isEmpty()) {
                return HandlerParameterSugarSpec.Share()
            }
            parseStringLiteral(trimmed)?.let { value ->
                return HandlerParameterSugarSpec.Share(value = value)
            }
            var value: String? = null
            var namespace: String? = null
            var namespaceSpecified = false
            for (member in splitTopLevelCommas(trimmed)) {
                val memberTrimmed = member.text.trim()
                if (memberTrimmed.isEmpty()) continue
                val equalsIndex = findTopLevelEquals(memberTrimmed)
                if (equalsIndex < 0) continue
                val key = memberTrimmed.substring(0, equalsIndex).trim()
                val rawValue = memberTrimmed.substring(equalsIndex + 1).trim()
                when (key) {
                    "value" -> value = parseStringLiteral(rawValue)
                    "namespace" -> {
                        namespace = parseStringLiteral(rawValue)
                        namespaceSpecified = true
                    }
                }
            }
            return HandlerParameterSugarSpec.Share(
                value = value,
                namespace = namespace,
                namespaceSpecified = namespaceSpecified,
            )
        }

        private fun findTopLevelEquals(value: String): Int {
            var inString = false
            var i = 0
            while (i < value.length) {
                when {
                    inString -> {
                        if (value[i] == '\\') {
                            i += 2
                            continue
                        }
                        if (value[i] == '"') inString = false
                    }
                    value[i] == '"' -> inString = true
                    value[i] == '=' -> return i
                }
                i++
            }
            return -1
        }

        private fun parseBooleanLiteral(value: String): Boolean =
            value.trim().equals("true", ignoreCase = true)

        private fun parseClassLiteral(value: String): String? {
            val trimmed = value.trim()
            if (!trimmed.endsWith(".class")) return null
            val className = trimmed.removeSuffix(".class").trim()
            if (className.isEmpty() || !isValidClassLiteralName(className)) return null
            return className
        }

        private fun isValidClassLiteralName(name: String): Boolean {
            if (name in PRIMITIVE_CLASS_LITERAL_NAMES) return true
            for (part in name.split('.')) {
                if (part.isEmpty()) return false
                if (!part.first().let { it.isLetter() || it == '_' }) return false
                if (part.drop(1).any { !(it.isLetterOrDigit() || it == '_' || it == '$') }) return false
            }
            return true
        }

        private val PRIMITIVE_CLASS_LITERAL_NAMES = setOf(
            "void", "boolean", "byte", "char", "short", "int", "long", "float", "double",
        )

        private fun parseOptionalIntLiteral(value: String): Int? {
            val parsed = value.trim().toIntOrNull() ?: return null
            return if (parsed == -1) null else parsed
        }

        private fun parseStringLiteral(value: String): String? {
            val trimmed = value.trim()
            if (trimmed.length < 2 || trimmed.first() != '"' || trimmed.last() != '"') return null
            val content = trimmed.substring(1, trimmed.length - 1)
            return unescapeJavaString(content)
        }

        private fun parseStringOrStringArray(value: String): Set<String> {
            val trimmed = value.trim()
            if (trimmed.startsWith('{')) {
                val close = trimmed.lastIndexOf('}')
                if (close <= 0) return emptySet()
                return splitTopLevelCommas(trimmed.substring(1, close))
                    .mapNotNull { parseStringLiteral(it.text.trim()) }
                    .toSet()
            }
            return parseStringLiteral(trimmed)?.let { setOf(it) } ?: emptySet()
        }

        private fun unescapeJavaString(value: String): String? {
            val result = StringBuilder()
            var i = 0
            while (i < value.length) {
                when (value[i]) {
                    '\\' -> {
                        if (i + 1 >= value.length) return null
                        if (value[i + 1] == 'u') {
                            var j = i + 1
                            while (j < value.length && value[j] == 'u') j++
                            if (j + 4 > value.length) return null
                            val hex = value.substring(j, j + 4)
                            if (!hex.all { it in '0'..'9' || it in 'a'..'f' || it in 'A'..'F' }) return null
                            result.append(hex.toInt(16).toChar())
                            i = j + 4
                        } else {
                            val decoded = JavaStringContentDecoder.decodeEscape(value, i, value.length) ?: return null
                            result.append(decoded.char)
                            i = decoded.nextIndex
                        }
                    }
                    '"', '\n', '\r' -> return null
                    else -> {
                        result.append(value[i])
                        i++
                    }
                }
            }
            return result.toString()
        }

        fun enrichHandlerTypes(handler: HandlerMethodDeclaration, classIndex: ClassIndex): HandlerMethodDeclaration {
            val returnDescriptor = descriptorFromHandlerType(handler.returnTypeName, classIndex)
            val params = handler.parameters.map { param ->
                param.copy(
                    typeDescriptor = when {
                        param.isOperation -> "Lcom/llamalad7/mixinextras/injector/wrapoperation/Operation;"
                        else -> descriptorFromHandlerType(param.typeName, classIndex)
                    },
                )
            }
            return handler.copy(returnTypeDescriptor = returnDescriptor, parameters = params)
        }

        private fun descriptorFromHandlerType(typeName: String, classIndex: ClassIndex): String? =
            OperationSignatureRenderer.descriptorFromTypeName(typeName, classIndex)
                ?: JavaTypeDescriptorResolver.descriptorOrNull(
                    typeName,
                    JavaSourceImports(
                        packageName = null,
                        explicit = classIndex.findClasses("", limit = 100_000)
                            .associate { it.simpleName to it.fqn },
                    ),
                )
                ?: reflectHandlerTypeDescriptor(typeName)

        private fun reflectHandlerTypeDescriptor(typeName: String): String? {
            var normalized = typeName.trim()
            var arrayDepth = 0
            while (normalized.endsWith("[]")) {
                arrayDepth++
                normalized = normalized.removeSuffix("[]").trim()
            }
            val genericStart = normalized.indexOf('<')
            if (genericStart >= 0) normalized = normalized.substring(0, genericStart).trim()
            val candidates = if ('.' in normalized) {
                listOf(normalized)
            } else {
                listOf("java.lang.$normalized", normalized)
            }
            val clazz = candidates.asSequence().mapNotNull { candidate ->
                runCatching {
                    Class.forName(candidate, false, HandlerSignatureService::class.java.classLoader)
                }.getOrNull()
            }.firstOrNull() ?: return null
            return "[".repeat(arrayDepth) + Type.getDescriptor(clazz)
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
            var end = readAnnotationQualifiedName(source, atOffset + 1).second
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

enum class WrapWithConditionTargetStatus {
    VALID_VOID,
    VALID_POPPED_NON_VOID,
    INVALID_RETAINED_NON_VOID,
    INVALID_INSTRUCTION,
    UNRESOLVED,
}

data class HandlerValidationIssue(
    val code: String,
    val message: String,
    val range: McTextRange,
)
