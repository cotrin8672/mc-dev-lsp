package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.bytecode.InstructionExtractor
import io.github.mcdev.core.bytecode.OccurrenceResultClassification
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.descriptor.DescriptorParseResult
import io.github.mcdev.core.descriptor.DescriptorRenderer
import io.github.mcdev.core.descriptor.parseFieldDescriptor
import io.github.mcdev.core.descriptor.parseMethodDescriptor
import io.github.mcdev.core.mixin.AtTargetKind
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.mixin.MixinTargetResolver
import org.objectweb.asm.Opcodes

data class ExpressionContext(
    val expressionIndex: MixinExtrasExpressionIndex,
    val definitionIndex: MixinExtrasDefinitionIndex,
) {
    fun expressionValuesForAtId(atId: String?): List<String> =
        expressionIndex.valuesForId(atId ?: "")
}

internal data class SimpleOperationLayout(
    val argumentDescriptors: List<String>,
    val parameterNames: List<String>,
    val returnDescriptor: String,
    val argumentIntLike: List<Boolean> = emptyList(),
    val returnIntLike: Boolean = false,
)

internal data class ExpressionValueTypeInfo(
    val descriptor: String,
    val intLike: Boolean = false,
)

internal data class ModifyReceiverLayout(
    val receiverOwnerInternalName: String,
    val parameterDescriptors: List<String>,
)

internal enum class WrapWithConditionTargetKind {
    INVOKE,
    FIELD,
    OTHER,
}

internal data class WrapWithConditionLayout(
    val ownerInternalName: String,
    val parameterDescriptors: List<String>,
    val isStatic: Boolean,
    val targetKind: WrapWithConditionTargetKind,
    val resultClassification: OccurrenceResultClassification,
)

object ExpressionContextResolver {
    private val invokeExpressionPattern = Regex("""^(?:this\.)?([\w.]+)\.(\w+)\s*\(([^)]*)\)\s*$""")
    private val bareInvokePattern = Regex("""^(\w+)\s*\(([^)]*)\)\s*$""")
    private val fieldAccessPattern = Regex("""^(?:this\.)?(\w+)\.(\w+)\s*$""")

    fun resolveExpressionContext(source: String, site: MixinExtrasAnnotationSite): ExpressionContext {
        val range = handlerRegionOffsetRange(source, site) ?: return ExpressionContext(
            expressionIndex = MixinExtrasExpressionIndex(),
            definitionIndex = MixinExtrasDefinitionIndex(),
        )
        return parseHandlerAnnotations(
            handlerRegion = source.substring(range.first, (range.last + 1).coerceAtMost(source.length)),
            regionStartOffset = range.first,
        )
    }

    fun findEnclosingSite(source: String, offset: Int): MixinExtrasAnnotationSite? {
        if (offset < 0 || offset >= source.length) return null
        return HandlerSignatureService.findSugarHandlerAnnotationSites(source)
            .mapNotNull { site ->
                val region = handlerRegionOffsetRange(source, site) ?: return@mapNotNull null
                if (offset !in region) return@mapNotNull null
                site to region.last - region.first
            }
            .minByOrNull { it.second }
            ?.first
    }

    fun resolveExpressionContextForSite(
        source: String,
        site: MixinExtrasAnnotationSite,
        resolvedContexts: List<ResolvedMixinExtrasContext>,
    ): ExpressionContext? {
        if (resolvedContexts.isEmpty()) {
            return resolveExpressionContext(source, site)
        }
        return selectResolvedMixinExtrasContext(site, resolvedContexts)?.context
    }

    fun parseHandlerAnnotations(handlerRegion: String, regionStartOffset: Int = 0): ExpressionContext {
        val expressions = mutableListOf<MixinExtrasExpression>()
        val definitions = mutableListOf<MixinExtrasDefinition>()
        for (annotation in scanAnnotations(handlerRegion)) {
            when {
                isExpressionAnnotationName(annotation.qualifiedName) -> {
                    ExpressionAnnotationParser.parse(annotation.body)?.let { expressions += it }
                }
                isExpressionsAnnotationName(annotation.qualifiedName) -> {
                    expressions += ExpressionsAnnotationParser.parse(annotation.body).expressions
                }
                isDefinitionAnnotationName(annotation.qualifiedName) -> {
                    DefinitionsAnnotationParser.parseDefinitionAt(
                        source = handlerRegion,
                        definitionAtOffset = annotation.startOffsetInHandlerRegion,
                        parsedTextBaseOffset = regionStartOffset,
                    )?.let { definitions += it }
                }
                isDefinitionsAnnotationName(annotation.qualifiedName) -> {
                    definitions += DefinitionsAnnotationParser.parse(
                        annotation.body,
                        parsedTextBaseOffset = regionStartOffset + annotation.bodyStartOffset,
                    ).definitions
                }
            }
        }
        return ExpressionContext(
            expressionIndex = MixinExtrasExpressionIndex(expressions),
            definitionIndex = MixinExtrasDefinitionIndex(definitions),
        )
    }

    fun handlerRegion(source: String, site: MixinExtrasAnnotationSite): String {
        val range = handlerRegionOffsetRange(source, site) ?: return ""
        return source.substring(range.first, (range.last + 1).coerceAtMost(source.length))
    }

    private fun handlerRegionOffsetRange(source: String, site: MixinExtrasAnnotationSite): IntRange? {
        val injectorEnd = positionToOffset(source, site.annotationRange.end)
        val handlerStart = site.handlerMethod?.let { positionToOffset(source, it.range.start) }
            ?: findHandlerMethodStartAfterAnnotations(source, injectorEnd)
        val blockStart = findHandlerModifierBlockStart(source, handlerStart)
        val start = blockStart.coerceIn(0, source.length)
        val end = handlerStart.coerceIn(0, source.length)
        if (start >= end) return null
        return start until end
    }

    fun inferExpressionValueType(
        source: String,
        site: MixinExtrasAnnotationSite,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
        bytecodeIndex: BytecodeIndex,
        classIndex: ClassIndex,
        resolvedContext: ExpressionContext? = null,
    ): String? = inferExpressionValueTypeInfo(
        source = source,
        site = site,
        targetMethod = targetMethod,
        mixinTargets = mixinTargets,
        bytecodeIndex = bytecodeIndex,
        classIndex = classIndex,
        resolvedContext = resolvedContext,
    )?.descriptor

    internal fun inferExpressionValueTypeInfo(
        source: String,
        site: MixinExtrasAnnotationSite,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
        bytecodeIndex: BytecodeIndex,
        classIndex: ClassIndex,
        resolvedContext: ExpressionContext? = null,
    ): ExpressionValueTypeInfo? {
        val context = resolvedContext ?: resolveExpressionContext(source, site)
        val expressions = context.expressionValuesForAtId(site.atId)
            .map { it.trim() }
            .filter { it.isNotEmpty() }
        if (expressions.isEmpty()) return null

        if (site.atValue.equals("MIXINEXTRAS:EXPRESSION", ignoreCase = true)) {
            when (
                val official = inferOfficialExpressionValueType(
                    expressions = expressions,
                    targetMethod = targetMethod,
                    mixinTargets = mixinTargets.distinct(),
                    bytecodeIndex = bytecodeIndex,
                    definitionIndex = context.definitionIndex,
                    typeNameResolver = ClassLiteralTypeNameResolver.forSource(source, classIndex),
                )
            ) {
                is OfficialExpressionInferenceResult.Success -> {
                    return ExpressionValueTypeInfo(official.descriptor, official.intLike)
                }
                is OfficialExpressionInferenceResult.Unavailable -> {
                    if (resolvedContext != null) return null
                }
                is OfficialExpressionInferenceResult.NoMatch,
                is OfficialExpressionInferenceResult.Conflict,
                -> return null
            }
        }

        val ownerInternalName = mixinTargets.firstOrNull() ?: return null
        val inferredTypes = expressions.map { expression ->
            inferFromExpression(
                expression = expression,
                ownerInternalName = ownerInternalName,
                targetMethod = targetMethod,
                bytecodeIndex = bytecodeIndex,
                classIndex = classIndex,
            )
        }
        if (inferredTypes.isEmpty()) return null
        if (inferredTypes.any { it == null }) return null
        val distinct = inferredTypes.filterNotNull().distinct()
        return if (distinct.size == 1) ExpressionValueTypeInfo(distinct.single()) else null
    }

    internal fun inferSimpleOperationLayout(
        source: String,
        site: MixinExtrasAnnotationSite,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
        bytecodeIndex: BytecodeIndex,
        classIndex: ClassIndex,
        resolvedContext: ExpressionContext? = null,
    ): SimpleOperationLayout? {
        val context = resolvedContext ?: resolveExpressionContext(source, site)
        val expressions = context.expressionValuesForAtId(site.atId)
            .map { it.trim() }
            .filter { it.isNotEmpty() }
        if (expressions.isEmpty()) return null

        val resolvedOwners = MixinTargetResolver.resolveTargets(mixinTargets, classIndex)
        val matches = when (
            val matchResult = matchOfficialExpressions(
                expressions = expressions,
                targetMethod = targetMethod,
                mixinTargets = resolvedOwners,
                bytecodeIndex = bytecodeIndex,
                definitionIndex = context.definitionIndex,
                contextType = OfficialExpressionMatchContextType.WRAP_OPERATION,
                typeNameResolver = ClassLiteralTypeNameResolver.forSource(source, classIndex),
            )
        ) {
            is OfficialExpressionMatchesResult.Available -> matchResult.matches
            OfficialExpressionMatchesResult.Unavailable,
            OfficialExpressionMatchesResult.NoMatch,
            -> return null
        }

        var agreedLayout: SimpleOperationLayout? = null
        for (match in matches) {
            val layout = simpleOperationLayout(match.match) ?: return null
            if (agreedLayout == null) {
                agreedLayout = layout
            } else if (agreedLayout != layout) {
                return null
            }
        }
        return agreedLayout
    }

    internal fun inferModifyReceiverLayout(
        source: String,
        site: MixinExtrasAnnotationSite,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
        bytecodeIndex: BytecodeIndex,
        classIndex: ClassIndex,
        resolvedContext: ExpressionContext? = null,
    ): ModifyReceiverLayout? {
        val context = resolvedContext ?: resolveExpressionContext(source, site)
        val expressions = context.expressionValuesForAtId(site.atId)
            .map { it.trim() }
            .filter { it.isNotEmpty() }
        if (expressions.isEmpty()) return null

        val resolvedOwners = MixinTargetResolver.resolveTargets(mixinTargets, classIndex)
        val matches = when (
            val matchResult = matchOfficialExpressions(
                expressions = expressions,
                targetMethod = targetMethod,
                mixinTargets = resolvedOwners,
                bytecodeIndex = bytecodeIndex,
                definitionIndex = context.definitionIndex,
                contextType = OfficialExpressionMatchContextType.MODIFY_RECEIVER,
                typeNameResolver = ClassLiteralTypeNameResolver.forSource(source, classIndex),
            )
        ) {
            is OfficialExpressionMatchesResult.Available -> matchResult.matches
            OfficialExpressionMatchesResult.Unavailable,
            OfficialExpressionMatchesResult.NoMatch,
            -> return null
        }

        var agreedLayout: ModifyReceiverLayout? = null
        for (match in matches) {
            val layout = modifyReceiverLayout(match) ?: return null
            if (agreedLayout == null) {
                agreedLayout = layout
            } else if (agreedLayout != layout) {
                return null
            }
        }
        return agreedLayout
    }

    internal fun inferWrapWithConditionLayouts(
        source: String,
        site: MixinExtrasAnnotationSite,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
        bytecodeIndex: BytecodeIndex,
        classIndex: ClassIndex,
        resolvedContext: ExpressionContext? = null,
    ): List<WrapWithConditionLayout>? {
        val context = resolvedContext ?: resolveExpressionContext(source, site)
        val expressions = context.expressionValuesForAtId(site.atId)
            .map { it.trim() }
            .filter { it.isNotEmpty() }
        if (expressions.isEmpty()) return null

        val resolvedOwners = MixinTargetResolver.resolveTargets(mixinTargets, classIndex)
        val matches = when (
            val matchResult = matchOfficialExpressions(
                expressions = expressions,
                targetMethod = targetMethod,
                mixinTargets = resolvedOwners,
                bytecodeIndex = bytecodeIndex,
                definitionIndex = context.definitionIndex,
                contextType = OfficialExpressionMatchContextType.WRAP_WITH_CONDITION,
                typeNameResolver = ClassLiteralTypeNameResolver.forSource(source, classIndex),
            )
        ) {
            is OfficialExpressionMatchesResult.Available -> matchResult.matches
            OfficialExpressionMatchesResult.Unavailable,
            OfficialExpressionMatchesResult.NoMatch,
            -> return null
        }

        val selectedMatches = site.atOrdinal?.let { ordinal ->
            resolvedOwners.flatMap { ownerInternalName ->
                val ownerMatches = matches.filter { it.ownerInternalName == ownerInternalName }
                val instructionIndices = ownerMatches
                    .map { it.match.originalInstructionIndex }
                    .distinct()
                    .sorted()
                val selectedIndex = instructionIndices.getOrNull(ordinal) ?: return null
                ownerMatches.filter { it.match.originalInstructionIndex == selectedIndex }
            }
        } ?: matches
        val layouts = selectedMatches.map { match ->
            wrapWithConditionLayout(match, targetMethod, bytecodeIndex) ?: return null
        }
        return layouts.takeIf { it.isNotEmpty() }
    }

    private sealed interface OfficialExpressionInferenceResult {
        data class Success(val descriptor: String, val intLike: Boolean) : OfficialExpressionInferenceResult

        data object Unavailable : OfficialExpressionInferenceResult

        data object NoMatch : OfficialExpressionInferenceResult

        data object Conflict : OfficialExpressionInferenceResult
    }

    private data class OfficialExpressionMatchAtOwner(
        val ownerInternalName: String,
        val match: OfficialExpressionMatch,
    )

    private sealed interface OfficialExpressionMatchesResult {
        data class Available(val matches: List<OfficialExpressionMatchAtOwner>) : OfficialExpressionMatchesResult

        data object Unavailable : OfficialExpressionMatchesResult

        data object NoMatch : OfficialExpressionMatchesResult
    }

    private fun inferOfficialExpressionValueType(
        expressions: List<String>,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
        bytecodeIndex: BytecodeIndex,
        definitionIndex: MixinExtrasDefinitionIndex,
        typeNameResolver: ClassLiteralTypeNameResolver,
    ): OfficialExpressionInferenceResult {
        val matches = when (
            val matchResult = matchOfficialExpressions(
                expressions = expressions,
                targetMethod = targetMethod,
                mixinTargets = mixinTargets,
                bytecodeIndex = bytecodeIndex,
                definitionIndex = definitionIndex,
                contextType = OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,
                typeNameResolver = typeNameResolver,
            )
        ) {
            is OfficialExpressionMatchesResult.Available -> matchResult.matches
            OfficialExpressionMatchesResult.Unavailable -> return OfficialExpressionInferenceResult.Unavailable
            OfficialExpressionMatchesResult.NoMatch -> return OfficialExpressionInferenceResult.NoMatch
        }

        val typeInfos = matches.map(::officialMatchTypeInfo)
        val descriptors = typeInfos.mapTo(linkedSetOf()) { it.descriptor }
        return when (descriptors.size) {
            0 -> OfficialExpressionInferenceResult.NoMatch
            1 -> OfficialExpressionInferenceResult.Success(descriptors.single(), typeInfos.all { it.intLike })
            else -> OfficialExpressionInferenceResult.Conflict
        }
    }

    private fun matchOfficialExpressions(
        expressions: List<String>,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
        bytecodeIndex: BytecodeIndex,
        definitionIndex: MixinExtrasDefinitionIndex,
        contextType: OfficialExpressionMatchContextType,
        typeNameResolver: ClassLiteralTypeNameResolver = ClassLiteralTypeNameResolver.FAIL_CLOSED,
    ): OfficialExpressionMatchesResult {
        if (mixinTargets.isEmpty()) return OfficialExpressionMatchesResult.Unavailable

        val identifierPool = OfficialExpressionIdentifierPoolBuilder.build(definitionIndex, typeNameResolver).pool
        val commonSuperClass = CommonSuperClassResolver { left, right ->
            bytecodeIndex.resolveCommonSuperClass(left, right)
        }
        val matches = mutableListOf<OfficialExpressionMatchAtOwner>()

        for (ownerInternalName in mixinTargets) {
            val classBytes = bytecodeIndex.getClassBytes(ownerInternalName)
                ?: return OfficialExpressionMatchesResult.Unavailable

            when (
                val matchResult = OfficialExpressionMatcher.match(
                    classBytes = classBytes,
                    methodName = targetMethod.name,
                    methodDescriptor = targetMethod.descriptor,
                    expressions = expressions,
                    contextType = contextType,
                    identifierPool = identifierPool,
                    commonSuperClass = commonSuperClass,
                )
            ) {
                is OfficialExpressionMatchResult.Unavailable ->
                    return OfficialExpressionMatchesResult.Unavailable
                is OfficialExpressionMatchResult.Available -> {
                    for (expressionIndex in expressions.indices) {
                        if (matchResult.matches.none { it.expressionIndex == expressionIndex }) {
                            return OfficialExpressionMatchesResult.NoMatch
                        }
                    }
                    matches += matchResult.matches.map {
                        OfficialExpressionMatchAtOwner(ownerInternalName, it)
                    }
                }
            }
        }

        return OfficialExpressionMatchesResult.Available(matches)
    }

    private fun simpleOperationLayout(match: OfficialExpressionMatch): SimpleOperationLayout? {
        val argumentDecoration = match.decorations[OfficialExpressionMatchDecorationKey.SIMPLE_OPERATION_ARGS]
            as? OfficialExpressionMatchDecorationValue.TypeConstraints
            ?: return null
        val parameterNameDecoration = match.decorations[OfficialExpressionMatchDecorationKey.SIMPLE_OPERATION_PARAM_NAMES]
            as? OfficialExpressionMatchDecorationValue.ParamNames
            ?: return null
        val returnDecoration = match.decorations[OfficialExpressionMatchDecorationKey.SIMPLE_OPERATION_RETURN_TYPE]
            as? OfficialExpressionMatchDecorationValue.TypeConstraint
            ?: return null
        if (argumentDecoration.constraints.size != parameterNameDecoration.names.size) return null

        val argumentDescriptors = argumentDecoration.constraints.map { constraint ->
            operationTypeConstraintDescriptor(constraint, allowVoid = false) ?: return null
        }
        val returnDescriptor =
            operationTypeConstraintDescriptor(returnDecoration.constraint, allowVoid = true) ?: return null
        return SimpleOperationLayout(
            argumentDescriptors = argumentDescriptors,
            parameterNames = parameterNameDecoration.names,
            returnDescriptor = returnDescriptor,
            argumentIntLike = argumentDecoration.constraints.map { it is OfficialExpressionTypeConstraint.IntLike },
            returnIntLike = returnDecoration.constraint is OfficialExpressionTypeConstraint.IntLike,
        )
    }

    private fun modifyReceiverLayout(matchAtOwner: OfficialExpressionMatchAtOwner): ModifyReceiverLayout? {
        if (matchAtOwner.ownerInternalName.isEmpty()) return null
        val match = matchAtOwner.match
        return when (val metadata = match.instructionMetadata) {
            is OfficialExpressionMatchInstructionMetadata.MethodInvocation -> {
                if (metadata.ownerInternalName.isEmpty() || metadata.name.isEmpty()) return null
                if (metadata.name == "<init>") return null
                val receiverOwner = when (match.originalInstructionOpcode) {
                    Opcodes.INVOKEVIRTUAL,
                    Opcodes.INVOKEINTERFACE,
                    -> metadata.ownerInternalName
                    Opcodes.INVOKESPECIAL -> matchAtOwner.ownerInternalName
                    else -> return null
                }
                if (receiverOwner.isEmpty()) return null
                val descriptor = when (val parsed = parseMethodDescriptor(metadata.descriptor)) {
                    is DescriptorParseResult.Success -> parsed.value
                    is DescriptorParseResult.Failure -> return null
                }
                ModifyReceiverLayout(
                    receiverOwnerInternalName = receiverOwner,
                    parameterDescriptors = descriptor.parameters.map(DescriptorRenderer::toDescriptor),
                )
            }
            is OfficialExpressionMatchInstructionMetadata.FieldAccess -> {
                if (metadata.ownerInternalName.isEmpty() || metadata.name.isEmpty()) return null
                val fieldDescriptor = when (val parsed = parseFieldDescriptor(metadata.descriptor)) {
                    is DescriptorParseResult.Success -> DescriptorRenderer.toDescriptor(parsed.value)
                    is DescriptorParseResult.Failure -> return null
                }
                val parameterDescriptors = when (match.originalInstructionOpcode) {
                    Opcodes.GETFIELD -> emptyList()
                    Opcodes.PUTFIELD -> listOf(fieldDescriptor)
                    else -> return null
                }
                ModifyReceiverLayout(
                    receiverOwnerInternalName = metadata.ownerInternalName,
                    parameterDescriptors = parameterDescriptors,
                )
            }
            else -> null
        }
    }

    private fun wrapWithConditionLayout(
        matchAtOwner: OfficialExpressionMatchAtOwner,
        targetMethod: MethodIndexEntry,
        bytecodeIndex: BytecodeIndex,
    ): WrapWithConditionLayout? {
        val match = matchAtOwner.match
        return when (val metadata = match.instructionMetadata) {
            is OfficialExpressionMatchInstructionMetadata.MethodInvocation -> {
                val descriptor = when (val parsed = parseMethodDescriptor(metadata.descriptor)) {
                    is DescriptorParseResult.Success -> parsed.value
                    is DescriptorParseResult.Failure -> return null
                }
                if (metadata.ownerInternalName.isEmpty() || metadata.name.isEmpty()) return null
                if (metadata.name == "<init>") {
                    return WrapWithConditionLayout(
                        ownerInternalName = "",
                        parameterDescriptors = emptyList(),
                        isStatic = true,
                        targetKind = WrapWithConditionTargetKind.OTHER,
                        resultClassification = OccurrenceResultClassification.NOT_APPLICABLE,
                    )
                }
                val parameterDescriptors = descriptor.parameters.map(DescriptorRenderer::toDescriptor)
                val returnDescriptor = DescriptorRenderer.toDescriptor(descriptor.returnType)
                val resultClassification = if (returnDescriptor == "V") {
                    OccurrenceResultClassification.VOID
                } else {
                    val classBytes = bytecodeIndex.getClassBytes(matchAtOwner.ownerInternalName)
                        ?: return null
                    runCatching {
                        InstructionExtractor.extract(
                            classBytes = classBytes,
                            methodName = targetMethod.name,
                            methodDescriptor = targetMethod.descriptor,
                        )
                    }.getOrNull()
                        ?.firstOrNull {
                            it.instructionOccurrenceIndex == match.originalInstructionIndex &&
                                it.owner == metadata.ownerInternalName &&
                                it.name == metadata.name &&
                                it.descriptor == metadata.descriptor &&
                                it.kind.name.startsWith("INVOKE_")
                        }
                        ?.occurrenceResultClassification
                        ?: return null
                }
                WrapWithConditionLayout(
                    ownerInternalName = metadata.ownerInternalName,
                    parameterDescriptors = parameterDescriptors,
                    isStatic = match.originalInstructionOpcode == Opcodes.INVOKESTATIC,
                    targetKind = WrapWithConditionTargetKind.INVOKE,
                    resultClassification = resultClassification,
                )
            }
            is OfficialExpressionMatchInstructionMetadata.FieldAccess -> {
                if (metadata.ownerInternalName.isEmpty() || metadata.name.isEmpty()) return null
                val fieldDescriptor = when (val parsed = parseFieldDescriptor(metadata.descriptor)) {
                    is DescriptorParseResult.Success -> DescriptorRenderer.toDescriptor(parsed.value)
                    is DescriptorParseResult.Failure -> return null
                }
                val isPut = match.originalInstructionOpcode == Opcodes.PUTFIELD ||
                    match.originalInstructionOpcode == Opcodes.PUTSTATIC
                val isStatic = match.originalInstructionOpcode == Opcodes.GETSTATIC ||
                    match.originalInstructionOpcode == Opcodes.PUTSTATIC
                val parameterDescriptors = if (isPut) listOf(fieldDescriptor) else emptyList()
                WrapWithConditionLayout(
                    ownerInternalName = metadata.ownerInternalName,
                    parameterDescriptors = parameterDescriptors,
                    isStatic = isStatic,
                    targetKind = WrapWithConditionTargetKind.FIELD,
                    resultClassification = if (isPut) {
                        OccurrenceResultClassification.VOID
                    } else {
                        OccurrenceResultClassification.RETAINED
                    },
                )
            }
            is OfficialExpressionMatchInstructionMetadata.TypeOperation,
            null,
            -> WrapWithConditionLayout(
                ownerInternalName = "",
                parameterDescriptors = emptyList(),
                isStatic = true,
                targetKind = WrapWithConditionTargetKind.OTHER,
                resultClassification = OccurrenceResultClassification.NOT_APPLICABLE,
            )
        }
    }

    private fun operationTypeConstraintDescriptor(
        constraint: OfficialExpressionTypeConstraint,
        allowVoid: Boolean,
    ): String? {
        val descriptor = officialTypeConstraintDescriptor(constraint)
        if (allowVoid && descriptor == "V") return descriptor
        return when (parseFieldDescriptor(descriptor)) {
            is DescriptorParseResult.Success -> descriptor
            is DescriptorParseResult.Failure -> null
        }
    }

    private fun officialMatchTypeInfo(matchAtOwner: OfficialExpressionMatchAtOwner): ExpressionValueTypeInfo {
        val match = matchAtOwner.match
        val constraint = when (
            val decoration = match.decorations[OfficialExpressionMatchDecorationKey.SIMPLE_EXPRESSION_TYPE]
        ) {
            is OfficialExpressionMatchDecorationValue.TypeConstraint -> decoration.constraint
            else -> match.capturedType
        }
        return ExpressionValueTypeInfo(
            descriptor = officialTypeConstraintDescriptor(constraint),
            intLike = constraint is OfficialExpressionTypeConstraint.IntLike,
        )
    }

    private fun officialTypeConstraintDescriptor(constraint: OfficialExpressionTypeConstraint): String =
        when (constraint) {
            is OfficialExpressionTypeConstraint.Exact -> constraint.descriptor
            is OfficialExpressionTypeConstraint.IntLike -> "I"
        }

    fun inferFromExpression(
        expression: String,
        ownerInternalName: String,
        targetMethod: MethodIndexEntry,
        bytecodeIndex: BytecodeIndex,
        classIndex: ClassIndex,
    ): String? {
        invokeExpressionPattern.matchEntire(expression)?.let { match ->
            val receiver = match.groupValues[1]
            val methodName = match.groupValues[2]
            val receiverOwner = resolveExpressionOwner(receiver, classIndex)
            return inferInvokeReturnType(ownerInternalName, targetMethod, methodName, receiverOwner, bytecodeIndex)
        }
        bareInvokePattern.matchEntire(expression)?.let { match ->
            val methodName = match.groupValues[1]
            return inferInvokeReturnType(ownerInternalName, targetMethod, methodName, null, bytecodeIndex)
        }
        fieldAccessPattern.matchEntire(expression)?.let { match ->
            val fieldName = match.groupValues[2]
            return inferFieldType(ownerInternalName, targetMethod, fieldName, bytecodeIndex, classIndex)
        }
        return null
    }

    private data class ScannedAnnotation(
        val qualifiedName: String,
        val body: String,
        /** Index of `@` within the scanned handler-region substring. */
        val startOffsetInHandlerRegion: Int,
        /** Index of the first character inside `@Ann(...)` within the handler-region substring. */
        val bodyStartOffset: Int,
        val end: Int,
    )

    private fun isExpressionAnnotationName(qualifiedName: String): Boolean =
        qualifiedName == "Expression" ||
            qualifiedName == "com.llamalad7.mixinextras.expression.Expression"

    private fun isExpressionsAnnotationName(qualifiedName: String): Boolean =
        qualifiedName == "Expressions" ||
            qualifiedName == "com.llamalad7.mixinextras.expression.Expressions"

    private fun isDefinitionAnnotationName(qualifiedName: String): Boolean =
        qualifiedName == "Definition" ||
            qualifiedName == "com.llamalad7.mixinextras.expression.Definition"

    private fun isDefinitionsAnnotationName(qualifiedName: String): Boolean =
        qualifiedName == "Definitions" ||
            qualifiedName == "com.llamalad7.mixinextras.expression.Definitions"

    private fun findHandlerMethodStartAfterAnnotations(source: String, fromOffset: Int): Int {
        var index = fromOffset.coerceIn(0, source.length)
        while (index < source.length) {
            index = skipForwardWhitespaceAndComments(source, index)
            if (index >= source.length) break
            if (source[index] == '@') {
                val scanned = scanAnnotation(source, index) ?: break
                index = scanned.end
                continue
            }
            break
        }
        return index.coerceIn(0, source.length)
    }

    private fun findHandlerModifierBlockStart(source: String, handlerStart: Int): Int {
        var blockStart = handlerStart
        var cursor = handlerStart
        while (cursor > 0) {
            val annotationStart = findAnnotationStartBefore(source, cursor) ?: break
            blockStart = annotationStart
            cursor = annotationStart
        }
        return blockStart
    }

    private fun findAnnotationStartBefore(source: String, beforeOffset: Int): Int? {
        val boundary = skipBackwardWhitespaceAndComments(source, beforeOffset)
        if (boundary <= 0) return null
        var search = boundary - 1
        while (search >= 0) {
            if (source[search] == '@' && !isInsideNonCodeLexicalState(source, search)) {
                val scanned = scanAnnotation(source, search)
                if (scanned != null && scanned.end == boundary) {
                    return search
                }
            }
            search--
        }
        return null
    }

    private fun scanAnnotations(region: String): List<ScannedAnnotation> {
        val results = mutableListOf<ScannedAnnotation>()
        var index = 0
        while (index < region.length) {
            index = skipForwardWhitespaceAndComments(region, index)
            if (index >= region.length) break
            if (region[index] != '@') {
                index++
                continue
            }
            val scanned = scanAnnotation(region, index)
            if (scanned == null) {
                index++
                continue
            }
            results += scanned
            index = scanned.end
        }
        return results
    }

    private fun scanAnnotation(source: String, atOffset: Int): ScannedAnnotation? {
        if (source.getOrNull(atOffset) != '@') return null
        var index = atOffset + 1
        val nameStart = index
        while (index < source.length && (source[index].isJavaIdentifierPart() || source[index] == '.')) {
            index++
        }
        if (index == nameStart) return null
        val nameEnd = index
        val qualifiedName = source.substring(nameStart, nameEnd)
        index = skipForwardWhitespaceAndComments(source, index)
        if (source.getOrNull(index) != '(') {
            return ScannedAnnotation(
                qualifiedName = qualifiedName,
                body = "",
                startOffsetInHandlerRegion = atOffset,
                bodyStartOffset = nameEnd,
                end = nameEnd,
            )
        }
        val bodyStart = index + 1
        val closeParen = findMatchingCloseParen(source, index) ?: return null
        return ScannedAnnotation(
            qualifiedName = qualifiedName,
            body = source.substring(bodyStart, closeParen),
            startOffsetInHandlerRegion = atOffset,
            bodyStartOffset = bodyStart,
            end = closeParen + 1,
        )
    }

    private data class StringLiteral(val value: String, val end: Int)

    private fun readStringLiteral(source: String, start: Int): StringLiteral? {
        if (source.getOrNull(start) != '"') return null
        val builder = StringBuilder()
        var index = start + 1
        while (index < source.length) {
            when {
                isLineCommentStart(source, index) || isBlockCommentStart(source, index) -> return null
                source[index] == '\\' -> {
                    if (index + 1 >= source.length) return null
                    builder.append(source[index + 1])
                    index += 2
                }
                source[index] == '"' -> return StringLiteral(builder.toString(), index + 1)
                else -> {
                    builder.append(source[index])
                    index++
                }
            }
        }
        return null
    }

    private fun skipForwardWhitespaceAndComments(source: String, start: Int): Int {
        var index = start
        while (index < source.length) {
            when {
                source[index].isWhitespace() -> index++
                isLineCommentStart(source, index) -> index = skipLineComment(source, index)
                isBlockCommentStart(source, index) -> index = skipBlockComment(source, index)
                else -> break
            }
        }
        return index
    }

    private fun skipBackwardWhitespaceAndComments(source: String, beforeOffset: Int): Int {
        var index = beforeOffset
        while (index > 0) {
            index = skipBackwardWhitespace(source, index)
            val lineCommentStart = findLineCommentStartBefore(source, index)
            if (lineCommentStart != null) {
                index = lineCommentStart
                continue
            }
            val blockCommentStart = findBlockCommentStartBefore(source, index)
            if (blockCommentStart != null) {
                index = blockCommentStart
                continue
            }
            break
        }
        return index
    }

    private fun skipBackwardWhitespace(source: String, beforeOffset: Int): Int {
        var index = beforeOffset
        while (index > 0 && source[index - 1].isWhitespace()) {
            index--
        }
        return index
    }

    private fun findLineCommentStartBefore(source: String, beforeOffset: Int): Int? {
        if (beforeOffset < 2) return null
        if (source[beforeOffset - 1] != '\n' && source[beforeOffset - 1] != '\r') return null
        var index = beforeOffset - 1
        while (index > 0 && source[index - 1] != '\n' && source[index - 1] != '\r') {
            index--
        }
        val lineStart = if (index > 0 && source[index - 1] == '\n') index else index
        val actualStart = if (source.getOrNull(lineStart) == '\r') lineStart + 1 else lineStart
        return if (isLineCommentStart(source, actualStart)) actualStart else null
    }

    private fun findBlockCommentStartBefore(source: String, beforeOffset: Int): Int? {
        var index = skipBackwardWhitespace(source, beforeOffset)
        if (index < 2 || source[index - 1] != '/' || source[index - 2] != '*') return null
        var search = index - 2
        while (search >= 1) {
            if (source[search - 1] == '/' && source[search] == '*') {
                return search - 1
            }
            search--
        }
        return null
    }

    private fun isInsideNonCodeLexicalState(source: String, offset: Int): Boolean {
        var index = 0
        var inString = false
        var inChar = false
        var inLineComment = false
        var inBlockComment = false
        while (index < offset) {
            when {
                inLineComment -> {
                    if (source[index] == '\n' || source[index] == '\r') inLineComment = false
                    index++
                }
                inBlockComment -> {
                    if (index + 1 < source.length && source[index] == '*' && source[index + 1] == '/') {
                        inBlockComment = false
                        index += 2
                    } else {
                        index++
                    }
                }
                inString -> when {
                    source[index] == '\\' -> index = (index + 2).coerceAtMost(source.length)
                    source[index] == '"' -> {
                        inString = false
                        index++
                    }
                    else -> index++
                }
                inChar -> when {
                    source[index] == '\\' -> index = (index + 2).coerceAtMost(source.length)
                    source[index] == '\'' -> {
                        inChar = false
                        index++
                    }
                    else -> index++
                }
                else -> when {
                    isLineCommentStart(source, index) -> {
                        inLineComment = true
                        index += 2
                    }
                    isBlockCommentStart(source, index) -> {
                        inBlockComment = true
                        index += 2
                    }
                    source[index] == '"' -> {
                        inString = true
                        index++
                    }
                    source[index] == '\'' -> {
                        inChar = true
                        index++
                    }
                    else -> index++
                }
            }
        }
        return inString || inChar || inLineComment || inBlockComment
    }

    private fun isLineCommentStart(source: String, index: Int): Boolean =
        index + 1 < source.length && source[index] == '/' && source[index + 1] == '/'

    private fun isBlockCommentStart(source: String, index: Int): Boolean =
        index + 1 < source.length && source[index] == '/' && source[index + 1] == '*'

    private fun skipLineComment(source: String, index: Int): Int {
        var current = index + 2
        while (current < source.length && source[current] != '\n') {
            current++
        }
        return current
    }

    private fun skipBlockComment(source: String, index: Int): Int {
        var current = index + 2
        while (current + 1 < source.length) {
            if (source[current] == '*' && source[current + 1] == '/') {
                return current + 2
            }
            current++
        }
        return source.length
    }

    private fun findMatchingCloseParen(source: String, openIndex: Int): Int? {
        if (source.getOrNull(openIndex) != '(') return null
        var depth = 0
        var index = openIndex
        while (index < source.length) {
            when {
                isLineCommentStart(source, index) -> index = skipLineComment(source, index)
                isBlockCommentStart(source, index) -> index = skipBlockComment(source, index)
                source[index] == '"' -> {
                    val literal = readStringLiteral(source, index) ?: return null
                    index = literal.end
                }
                source[index] == '(' -> {
                    depth++
                    index++
                }
                source[index] == ')' -> {
                    depth--
                    if (depth == 0) return index
                    index++
                }
                else -> index++
            }
        }
        return null
    }

    private fun inferInvokeReturnType(
        ownerInternalName: String,
        targetMethod: MethodIndexEntry,
        methodName: String,
        receiverOwner: String?,
        bytecodeIndex: BytecodeIndex,
    ): String? {
        val candidates = bytecodeIndex.getAtTargetCandidates(
            ownerInternalName,
            targetMethod.name,
            targetMethod.descriptor,
            "INVOKE",
        )
        val match = candidates.filter {
            it.kind == AtTargetKind.INVOKE &&
                it.name == methodName &&
                (receiverOwner == null || it.owner == receiverOwner)
        }.firstOrNull() ?: return null
        return methodReturnDescriptor(match.descriptor)
    }

    private fun resolveExpressionOwner(receiver: String, classIndex: ClassIndex): String? {
        if (!receiver.contains('.')) return null
        return classIndex.findClassByFqn(receiver)?.internalName ?: receiver.replace('.', '/')
    }

    private fun inferFieldType(
        ownerInternalName: String,
        targetMethod: MethodIndexEntry,
        fieldName: String,
        bytecodeIndex: BytecodeIndex,
        classIndex: ClassIndex,
    ): String? {
        val candidates = bytecodeIndex.getAtTargetCandidates(
            ownerInternalName,
            targetMethod.name,
            targetMethod.descriptor,
            "FIELD",
        )
        val match = candidates.filter {
            it.kind == AtTargetKind.FIELD && it.name == fieldName
        }.firstOrNull()
        if (match != null) return match.descriptor
        return classIndex.getFields(ownerInternalName).find { it.name == fieldName }?.descriptor
    }

    private fun methodReturnDescriptor(descriptor: String): String? {
        val close = descriptor.indexOf(')')
        return if (close >= 0 && close + 1 < descriptor.length) descriptor.substring(close + 1) else null
    }

    private fun positionToOffset(source: String, position: McTextPosition): Int {
        var line = 0
        var offset = 0
        while (offset < source.length && line < position.line) {
            if (source[offset] == '\n') line++
            offset++
        }
        return (offset + position.character).coerceAtMost(source.length)
    }

    private fun Char.isJavaIdentifierStart(): Boolean = isLetter() || this == '_' || this == '$'

    private fun Char.isJavaIdentifierPart(): Boolean = isLetterOrDigit() || this == '_' || this == '$'
}
