package io.github.mcdev.core.mixin

import io.github.mcdev.core.codeaction.McFix
import io.github.mcdev.core.bytecode.OccurrenceResultClassification
import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata
import io.github.mcdev.core.completion.McCompletionReplacementRange
import io.github.mcdev.core.descriptor.DescriptorParseResult
import io.github.mcdev.core.descriptor.JvmType
import io.github.mcdev.core.descriptor.parseMethodDescriptor
import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.core.diagnostics.McSeverity
import io.github.mcdev.core.mixinextras.CommonSuperClassResolver
import io.github.mcdev.core.mixinextras.ClassLiteralTypeNameResolver
import io.github.mcdev.core.mixinextras.ExpressionContextResolver
import io.github.mcdev.core.mixinextras.ExpressionMemberCompletionService
import io.github.mcdev.core.mixinextras.ExpressionSupport
import io.github.mcdev.core.mixinextras.HandlerParameterDeclaration
import io.github.mcdev.core.mixinextras.HandlerParameterSugarSpec
import io.github.mcdev.core.mixinextras.HandlerSignatureService
import io.github.mcdev.core.mixinextras.LocalCaptureValidationService
import io.github.mcdev.core.mixinextras.LocalCompletionService
import io.github.mcdev.core.mixinextras.MixinExtrasAnnotation
import io.github.mcdev.core.mixinextras.MixinExtrasAnnotationSite
import io.github.mcdev.core.mixinextras.MixinExtrasCodeActionService
import io.github.mcdev.core.mixinextras.MixinExtrasCompletionService
import io.github.mcdev.core.mixinextras.MixinExtrasDiagnosticCodes
import io.github.mcdev.core.mixinextras.MixinExtrasDiagnosticRequest
import io.github.mcdev.core.mixinextras.MixinExtrasDiagnosticsService
import io.github.mcdev.core.mixinextras.OfficialExpressionIdentifierPoolBuilder
import io.github.mcdev.core.mixinextras.OfficialExpressionMatchResult
import io.github.mcdev.core.mixinextras.OfficialExpressionMatcher
import io.github.mcdev.core.mixinextras.ResolvedMixinExtrasContext
import io.github.mcdev.core.mixinextras.ShareCompletionService
import io.github.mcdev.core.mixinextras.toOfficialExpressionMatchContextType
import io.github.mcdev.core.mixin.MethodIndexEntry

data class MixinFacadeRequest(
    val bufferText: String,
    val line: Int,
    val character: Int,
    val documentUri: String = "file:///Mixin.java",
    val mixinClassName: String? = null,
    val mixinPackage: String? = null,
    val mixinConfigContent: String? = null,
    val mixinConfigPath: String? = null,
    val semanticModel: MixinClassModel? = null,
)

data class McdevCompletionDebugInfo(
    val command: String,
    val documentUri: String,
    val languageId: String,
    val parseSource: ParseSource?,
    val parseConfidence: ParseConfidence?,
    val usedCompilationUnit: Boolean,
    val usedJavaProject: Boolean,
    val bindingResolvedCount: Int,
    val bindingFailedCount: Int,
    val fallbackReason: String?,
    val semanticContextFound: Boolean,
    val fallbackAnnotationContextUsed: Boolean,
    val fallbackAnnotationContextReason: String?,
    val semanticTargetCount: Int,
    val semanticMemberCount: Int,
    val completionContextKind: String?,
    val owner: String?,
    val methodName: String?,
    val methodDescriptor: String?,
    val candidateCountBeforeFilter: Int,
    val candidateCountAfterFilter: Int,
    val zeroItemReason: String?,
    val warnings: List<String>,
)

data class MixinCompletionResult(
    val items: List<McCompletionItem>,
    val debug: McdevCompletionDebugInfo,
    val replacementRange: McCompletionReplacementRange? = null,
)

class MixinServiceFacade(
    private val classIndex: ClassIndex,
    private val bytecodeIndex: BytecodeIndex,
    private val mixinTargetCompletion: MixinTargetCompletionService = MixinTargetCompletionService(classIndex),
    private val injectMethodCompletion: InjectMethodCompletionService = InjectMethodCompletionService(classIndex),
    private val atValueCompletion: AtValueCompletionService = AtValueCompletionService(),
    private val atTargetCompletion: AtTargetCompletionService = AtTargetCompletionService(),
    private val attributeCompletion: MixinAttributeCompletionService = MixinAttributeCompletionService(),
    private val shadowValidation: ShadowValidationService = ShadowValidationService(classIndex),
    private val accessorService: AccessorService = AccessorService(classIndex),
    private val invokerService: InvokerService = InvokerService(classIndex),
    private val diagnosticsService: MixinDiagnosticsService = MixinDiagnosticsService(classIndex, bytecodeIndex),
    private val codeActionService: MixinCodeActionService = MixinCodeActionService(
        accessorService = accessorService,
        invokerService = invokerService,
    ),
    private val mixinExtrasCompletion: MixinExtrasCompletionService = MixinExtrasCompletionService(
        classIndex,
        bytecodeIndex,
    ),
    private val mixinExtrasDiagnostics: MixinExtrasDiagnosticsService = MixinExtrasDiagnosticsService(classIndex, bytecodeIndex),
    private val shareCompletion: ShareCompletionService = ShareCompletionService(classIndex),
    private val shareSources: () -> Sequence<String> = { emptySequence() },
    private val expressionSupport: ExpressionSupport = ExpressionSupport(
        ExpressionMemberCompletionService(classIndex, bytecodeIndex),
        classIndex,
    ),
) {
    private val handlerSignatureService = HandlerSignatureService(classIndex, bytecodeIndex)
    private val localCaptureValidation = LocalCaptureValidationService(classIndex, bytecodeIndex)
    private val localCompletion = LocalCompletionService(classIndex)
    private val mixinExtrasCodeActions = MixinExtrasCodeActionService(classIndex, handlerSignatureService)
    private val mixinExtrasMethodAnnotations = setOf(
        MixinAnnotation.MODIFY_EXPRESSION_VALUE,
        MixinAnnotation.MODIFY_RETURN_VALUE,
        MixinAnnotation.MODIFY_RECEIVER,
        MixinAnnotation.WRAP_OPERATION,
        MixinAnnotation.WRAP_WITH_CONDITION,
        MixinAnnotation.WRAP_METHOD,
    )
    fun complete(
        request: MixinFacadeRequest,
        options: MixinCompletionOptions = MixinCompletionOptions(),
    ): List<McCompletionItem> = completeWithDebug(request, options).items

    fun completeWithDebug(
        request: MixinFacadeRequest,
        options: MixinCompletionOptions = MixinCompletionOptions(),
        command: String = "mcdev.completion",
        languageId: String = "java",
    ): MixinCompletionResult {
        val effectiveRequest = if (request.semanticModel == null) {
            request.copy(
                semanticModel = MixinSemanticModelParser.parse(
                    source = request.bufferText,
                    sourceUri = request.documentUri,
                    parseSource = ParseSource.HAND_WRITTEN_FALLBACK,
                    confidence = ParseConfidence.MEDIUM,
                ),
            )
        } else {
            request
        }
        val semanticContext = SemanticCompletionContextExtractor.extract(
            source = effectiveRequest.bufferText,
            line = effectiveRequest.line,
            character = effectiveRequest.character,
            model = effectiveRequest.semanticModel,
        )
        val semanticAnnotationContext = semanticContext?.let {
            SemanticCompletionContextExtractor.toAnnotationContext(
                source = effectiveRequest.bufferText,
                line = effectiveRequest.line,
                character = effectiveRequest.character,
                model = effectiveRequest.semanticModel,
                context = it,
            )
        }
        val fallbackContext = if (semanticAnnotationContext == null) {
            val offset = AnnotationContextExtractor.toOffset(effectiveRequest.bufferText, effectiveRequest.line, effectiveRequest.character)
            offset?.let { AnnotationContextExtractor.extractAtOffset(effectiveRequest.bufferText, it) }
        } else {
            null
        }
        val context = semanticAnnotationContext ?: fallbackContext
        val warnings = mutableListOf<String>()
        warnings += effectiveRequest.semanticModel?.warnings.orEmpty()
        val fallbackAnnotationContextReason = if (semanticAnnotationContext == null && fallbackContext != null) {
            "semantic completion context was unavailable"
        } else {
            null
        }
        if (fallbackAnnotationContextReason != null) {
            warnings += "FALLBACK_ANNOTATION_CONTEXT_USED: $fallbackAnnotationContextReason"
        }
        val routedItems = context?.let { routeCompletion(effectiveRequest, it, options) }.orEmpty()
        val featureCompletion = if (routedItems.isEmpty() && !isInsideExpressionValue(context)) {
            expressionSupport.completeFeatureAnnotationsWithRange(
                effectiveRequest.bufferText,
                effectiveRequest.line,
                effectiveRequest.character,
            )
        } else {
            ExpressionSupport.FeatureAnnotationCompletion.EMPTY
        }
        val items = routedItems.ifEmpty {
            if (isInsideExpressionValue(context)) {
                emptyList()
            } else {
                featureCompletion.items
            }
        }
        return MixinCompletionResult(
            items = items,
            debug = debugInfo(
                request = effectiveRequest,
                command = command,
                languageId = languageId,
                context = context,
                semanticContext = semanticContext,
                fallbackAnnotationContextUsed = fallbackContext != null,
                fallbackAnnotationContextReason = fallbackAnnotationContextReason,
                items = items,
                warnings = warnings.distinct(),
            ),
            replacementRange = if (routedItems.isEmpty()) featureCompletion.replacementRange else null,
        )
    }

    fun diagnose(request: MixinFacadeRequest): List<McDiagnostic> {
        val diagnostics = mutableListOf<McDiagnostic>()
        diagnostics += diagnosticsService.analyze(
            MixinDiagnosticRequest(
                source = request.bufferText,
                documentUri = request.documentUri,
                mixinClassName = request.mixinClassName,
                mixinPackage = request.mixinPackage,
                mixinConfigContent = request.mixinConfigContent,
                mixinConfigPath = request.mixinConfigPath,
                semanticModel = request.semanticModel,
            ),
        )
        diagnostics += analyzeMemberDeclarations(request)
        diagnostics += mixinExtrasDiagnostics.analyze(
            MixinExtrasDiagnosticRequest(
                source = request.bufferText,
                documentUri = request.documentUri,
                resolvedContexts = request.semanticModel?.resolvedMixinExtrasContexts.orEmpty(),
            ),
        )
        diagnostics += diagnoseLocalCaptures(request)
        diagnostics += diagnoseStandardInjectorSugarConstraints(request)
        return diagnostics
    }

    fun codeActions(
        request: MixinFacadeRequest,
        diagnosticCode: String? = null,
    ): List<McFix> {
        val diagnostics = diagnose(request).filter { diagnosticCode == null || it.code == diagnosticCode }
        val mixinFixes = codeActionService.fixesForDiagnostics(
            diagnostics = diagnostics,
            documentUri = request.documentUri,
            source = request.bufferText,
            mixinConfigContent = request.mixinConfigContent,
            mixinConfigPath = request.mixinConfigPath,
            mixinPackage = request.mixinPackage,
            classIndex = classIndex,
        )
        val extrasFixes = mixinExtrasCodeActions.fixesForDiagnostics(
            diagnostics = diagnostics,
            documentUri = request.documentUri,
            source = request.bufferText,
            resolvedContexts = request.semanticModel?.resolvedMixinExtrasContexts.orEmpty(),
        )
        val generatedExtrasFixes = mixinExtrasCodeActions.generateHandlerFixesAtPosition(
            documentUri = request.documentUri,
            source = request.bufferText,
            line = request.line,
            character = request.character,
            mixinTargets = MixinTargetResolver.resolveTargetsFromSource(request.bufferText, classIndex),
            resolvedContexts = request.semanticModel?.resolvedMixinExtrasContexts.orEmpty(),
        )
        return MixinExtrasCodeActionService.deduplicateFixes(mixinFixes + extrasFixes + generatedExtrasFixes)
    }

    private fun routeCompletion(
        request: MixinFacadeRequest,
        context: AnnotationContext,
        options: MixinCompletionOptions,
    ): List<McCompletionItem> {
        val source = request.bufferText
        if (context.slot == AnnotationSlot.ATTRIBUTE) {
            val items = attributeCompletion.complete(context)
            if (context.annotation == MixinAnnotation.LOCAL) {
                val matchingSites = findLocalParameterSitesAtOffset(source, context.valueEndOffset)
                if (matchingSites.any { it.site.annotation == MixinExtrasAnnotation.WRAP_METHOD }) {
                    return emptyList()
                }
                if (matchingSites.isNotEmpty()) {
                    return items.filter { it.metadata.name != "type" }
                }
            }
            return items
        }
        return when (context.annotation) {
            MixinAnnotation.MIXIN -> mixinTargetCompletion.complete(context, options)
            MixinAnnotation.INJECT,
            MixinAnnotation.REDIRECT,
            MixinAnnotation.MODIFY_ARG,
            MixinAnnotation.MODIFY_ARGS,
            MixinAnnotation.MODIFY_VARIABLE,
            MixinAnnotation.MODIFY_CONSTANT,
            -> injectMethodCompletion.complete(context.withResolvedMixinTargets(request), options)
            in mixinExtrasMethodAnnotations -> completeMixinExtrasMethod(request, context, options)
            MixinAnnotation.AT -> when (context.slot) {
                AnnotationSlot.VALUE -> mergeAtValueCompletions(context)
                AnnotationSlot.TARGET -> completeAtTarget(request, context)
                else -> emptyList()
            }
            MixinAnnotation.ACCESSOR -> if (context.slot == AnnotationSlot.ACCESSOR_VALUE) {
                accessorService.completeFields(
                    mixinTargets = resolveMixinTargets(request, context),
                    prefix = context.partialValue.trim('"'),
                )
            } else {
                emptyList()
            }
            MixinAnnotation.INVOKER -> if (context.slot == AnnotationSlot.INVOKER_VALUE) {
                invokerService.completeMethods(
                    mixinTargets = resolveMixinTargets(request, context),
                    prefix = context.partialValue.trim('"'),
                )
            } else {
                emptyList()
            }
            MixinAnnotation.SHADOW -> completeShadow(request, context)
            MixinAnnotation.OVERWRITE -> completeOverwrite(request, context)
            MixinAnnotation.LOCAL -> if (context.slot == AnnotationSlot.VALUE) {
                completeLocalAttributeValues(request, context)
            } else {
                emptyList()
            }
            MixinAnnotation.SHARE -> if (
                context.slot == AnnotationSlot.VALUE && context.attributeName != "namespace"
            ) {
                shareCompletion.complete(source, context, shareSources())
            } else {
                emptyList()
            }
            MixinAnnotation.EXPRESSION,
            MixinAnnotation.EXPRESSIONS,
            -> if (context.slot == AnnotationSlot.VALUE) {
                expressionSupport.completeExpressionValue(
                    context = context,
                    source = request.bufferText,
                    resolvedContexts = request.semanticModel?.resolvedMixinExtrasContexts.orEmpty(),
                    mixinTargetOwners = resolveMixinTargets(request, context),
                )
            } else {
                emptyList()
            }
            else -> emptyList()
        }
    }

    private fun completeLocalAttributeValues(
        request: MixinFacadeRequest,
        context: AnnotationContext,
    ): List<McCompletionItem> {
        val attributeName = context.attributeName ?: return emptyList()
        if (attributeName !in LOCAL_VALUE_ATTRIBUTES) return emptyList()

        val source = request.bufferText
        val matchingSites = findLocalParameterSitesAtOffset(source, context.valueStartOffset)
        if (matchingSites.isEmpty()) return emptyList()
        if (matchingSites.any { it.site.annotation == MixinExtrasAnnotation.WRAP_METHOD }) return emptyList()

        val mixinTargets = resolveMixinTargets(request, context)
        if (mixinTargets.isEmpty()) return emptyList()

        val parameter = matchingSites.first().parameter
        val results = matchingSites.flatMap { (site, siteParameter) ->
            resolveLocalCaptures(request, site, siteParameter, mixinTargets)
        }
        if (results.isEmpty()) return emptyList()

        return localCompletion.complete(
            source = source,
            parameter = parameter,
            results = results,
            attributeName = attributeName,
            partialPrefix = context.partialValue.trim().trim('"'),
        )
    }

    private fun resolveLocalCaptures(
        request: MixinFacadeRequest,
        site: MixinExtrasAnnotationSite,
        parameter: HandlerParameterDeclaration,
        mixinTargets: List<String>,
    ): List<LocalCaptureValidationService.Result> {
        val targetMethod = handlerSignatureService.resolveTargetMethod(
            mixinTargets,
            site.methodAttribute,
        ) ?: return emptyList()
        val owners = eligibleLocalCaptureOwners(mixinTargets, targetMethod)
        if (owners.isEmpty()) return emptyList()

        val source = request.bufferText
        val resolvedContexts = request.semanticModel?.resolvedMixinExtrasContexts.orEmpty()
        val points = owners.map { owner ->
            LocalCaptureValidationService.Point(
                owner = owner,
                targetMethod = targetMethod,
                site = site,
                expressionInstructionIndices = resolveExpressionInstructionIndices(
                    source = source,
                    site = site,
                    targetMethod = targetMethod,
                    owner = owner,
                    resolvedContexts = resolvedContexts,
                ),
            )
        }
        return localCaptureValidation.validate(source, parameter, points)
    }

    private data class LocalParameterSite(
        val site: MixinExtrasAnnotationSite,
        val parameter: HandlerParameterDeclaration,
    )

    private fun findLocalParameterSitesAtOffset(source: String, offset: Int): List<LocalParameterSite> {
        val matches = mutableListOf<LocalParameterSite>()
        for (site in HandlerSignatureService.findSugarHandlerAnnotationSites(source)) {
            val handler = site.handlerMethod ?: continue
            for (parameter in handler.parameters) {
                if (parameter.sugarSpec !is HandlerParameterSugarSpec.Local) continue
                val range = parameter.sugarAnnotationRange ?: continue
                val start = AnnotationContextExtractor.toOffset(source, range.start.line, range.start.character)
                    ?: continue
                val end = AnnotationContextExtractor.toOffset(source, range.end.line, range.end.character)
                    ?: continue
                if (offset in start until end) {
                    matches += LocalParameterSite(site, parameter)
                }
            }
        }
        return matches
    }

    private fun eligibleLocalCaptureOwners(
        mixinTargetOwners: List<String>,
        targetMethod: MethodIndexEntry,
    ): List<String> =
        mixinTargetOwners.filter { owner ->
            classIndex.getMethods(owner).any { method ->
                method.name == targetMethod.name && method.descriptor == targetMethod.descriptor
            }
        }

    private fun resolveExpressionInstructionIndices(
        source: String,
        site: MixinExtrasAnnotationSite,
        targetMethod: MethodIndexEntry,
        owner: String,
        resolvedContexts: List<ResolvedMixinExtrasContext>,
    ): Set<Int>? {
        if (!site.atValue.equals(MIXINEXTRAS_EXPRESSION_AT_VALUE, ignoreCase = true)) {
            return null
        }
        val contextType = site.annotation.toOfficialExpressionMatchContextType() ?: return null
        val expressionContext = ExpressionContextResolver.resolveExpressionContextForSite(
            source = source,
            site = site,
            resolvedContexts = resolvedContexts,
        ) ?: return null
        val expressions = expressionContext.expressionValuesForAtId(site.atId)
            .map { it.trim() }
            .filter { it.isNotEmpty() }
        if (expressions.isEmpty()) return null

        val classBytes = bytecodeIndex.getClassBytes(owner) ?: return null
        val identifierPool = OfficialExpressionIdentifierPoolBuilder.build(
            expressionContext.definitionIndex,
            ClassLiteralTypeNameResolver.forSource(source, classIndex),
        ).pool
        val commonSuperClass = CommonSuperClassResolver { left, right ->
            bytecodeIndex.resolveCommonSuperClass(left, right)
        }
        return when (
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
            is OfficialExpressionMatchResult.Unavailable -> null
            is OfficialExpressionMatchResult.Available -> {
                val indices = matchResult.matches.map { it.originalInstructionIndex }.toSet()
                indices.takeIf { it.isNotEmpty() }
            }
        }
    }

    private fun isInsideExpressionValue(context: AnnotationContext?): Boolean {
        if (context == null) return false
        return context.annotation in setOf(MixinAnnotation.EXPRESSION, MixinAnnotation.EXPRESSIONS) &&
            context.slot == AnnotationSlot.VALUE
    }

    private fun mergeAtValueCompletions(context: AnnotationContext): List<McCompletionItem> {
        val coreItems = atValueCompletion.complete(context)
        val expressionItems = expressionSupport.completeAtValue(context)
        val expressionByInsertText = expressionItems.associateBy { it.insertText }
        val merged = mutableListOf<McCompletionItem>()
        val seen = mutableSetOf<String>()
        for (item in coreItems) {
            val preferred = expressionByInsertText[item.insertText] ?: item
            if (seen.add(preferred.insertText)) {
                merged.add(preferred)
            }
        }
        for (item in expressionItems) {
            if (seen.add(item.insertText)) {
                merged.add(item)
            }
        }
        return merged
    }

    private fun completeMixinExtrasMethod(
        request: MixinFacadeRequest,
        context: AnnotationContext,
        options: MixinCompletionOptions,
    ): List<McCompletionItem> {
        val resolvedContext = context.withResolvedMixinTargets(request)
        val extrasItems = mixinExtrasCompletion.complete(resolvedContext, options)
        if (extrasItems.isNotEmpty() || context.annotation != MixinAnnotation.MODIFY_RECEIVER) {
            return extrasItems
        }
        return injectMethodCompletion.complete(resolvedContext, options)
    }

    private fun completeAtTarget(request: MixinFacadeRequest, context: AnnotationContext): List<McCompletionItem> {
        val owners = resolveMixinTargets(request, context)
        if (owners.isEmpty()) return emptyList()
        val methodTarget = parseMethodTarget(
            context.injectMethodName
            ?: return emptyList(),
        )
        val atValue = context.atValue
            ?: return emptyList()
        val candidates = owners.flatMap { owner ->
            bytecodeIndex.getAtTargetCandidates(
                owner,
                methodTarget.name,
                methodTarget.descriptor,
                atValue,
            )
        }.distinct()
        val filteredCandidates = if (context.atInsideSlice) {
            candidates
        } else {
            candidates.filter {
                isValidAtTargetCandidate(context.parentInjectorAnnotation, methodTarget.descriptor, it)
            }
        }
        return atTargetCompletion.complete(context, filteredCandidates)
    }

    private fun isValidAtTargetCandidate(
        injector: MixinAnnotation?,
        targetMethodDescriptor: String?,
        candidate: AtTargetCandidate,
    ): Boolean = when (injector) {
        MixinAnnotation.MODIFY_EXPRESSION_VALUE -> when (candidate.kind) {
            AtTargetKind.INVOKE ->
                candidate.name != "<init>" && hasNonVoidReturn(candidate.descriptor)
            AtTargetKind.FIELD -> candidate.operationKind in FIELD_GET_KINDS
            AtTargetKind.NEW,
            AtTargetKind.CONSTANT,
            -> true
            else -> false
        }
        MixinAnnotation.MODIFY_RETURN_VALUE ->
            hasNonVoidReturn(targetMethodDescriptor) && candidate.kind == AtTargetKind.RETURN
        MixinAnnotation.MODIFY_RECEIVER ->
            candidate.name != "<init>" &&
                (candidate.operationKind in INSTANCE_INVOKE_KINDS || candidate.operationKind in INSTANCE_FIELD_KINDS)
        MixinAnnotation.WRAP_OPERATION -> when (candidate.kind) {
            AtTargetKind.INVOKE -> candidate.name != "<init>" && candidate.operationKind in INVOKE_KINDS
            AtTargetKind.FIELD -> candidate.operationKind in FIELD_KINDS
            AtTargetKind.NEW -> true
            else -> false
        }
        MixinAnnotation.WRAP_WITH_CONDITION ->
            (candidate.name != "<init>" &&
                candidate.operationKind in INVOKE_KINDS &&
                candidate.occurrenceResultClassification in CONDITION_INVOKE_RESULTS) ||
                candidate.operationKind in FIELD_PUT_KINDS
        else -> true
    }

    private fun hasNonVoidReturn(descriptor: String?): Boolean {
        val parsed = descriptor?.let(::parseMethodDescriptor) as? DescriptorParseResult.Success ?: return false
        return parsed.value.returnType != JvmType.VoidType
    }

    private data class MethodTarget(
        val name: String,
        val descriptor: String?,
    )

    private companion object {
        val INVOKE_KINDS = setOf(
            AtTargetOperationKind.INVOKE_VIRTUAL,
            AtTargetOperationKind.INVOKE_STATIC,
            AtTargetOperationKind.INVOKE_SPECIAL,
            AtTargetOperationKind.INVOKE_INTERFACE,
        )
        val INSTANCE_INVOKE_KINDS = setOf(
            AtTargetOperationKind.INVOKE_VIRTUAL,
            AtTargetOperationKind.INVOKE_SPECIAL,
            AtTargetOperationKind.INVOKE_INTERFACE,
        )
        val FIELD_GET_KINDS = setOf(
            AtTargetOperationKind.FIELD_GET_INSTANCE,
            AtTargetOperationKind.FIELD_GET_STATIC,
        )
        val FIELD_PUT_KINDS = setOf(
            AtTargetOperationKind.FIELD_PUT_INSTANCE,
            AtTargetOperationKind.FIELD_PUT_STATIC,
        )
        val FIELD_KINDS = FIELD_GET_KINDS + FIELD_PUT_KINDS
        val INSTANCE_FIELD_KINDS = setOf(
            AtTargetOperationKind.FIELD_GET_INSTANCE,
            AtTargetOperationKind.FIELD_PUT_INSTANCE,
        )
        val CONDITION_INVOKE_RESULTS = setOf(
            OccurrenceResultClassification.VOID,
            OccurrenceResultClassification.IMMEDIATELY_POPPED,
        )
        val LOCAL_VALUE_ATTRIBUTES = setOf("ordinal", "index", "name")
        const val MIXINEXTRAS_EXPRESSION_AT_VALUE = "MIXINEXTRAS:EXPRESSION"
    }

    private fun parseMethodTarget(value: String): MethodTarget {
        val paren = value.indexOf('(')
        return if (paren > 0) {
            MethodTarget(value.substring(0, paren), value.substring(paren))
        } else {
            MethodTarget(value, null)
        }
    }

    private fun resolveMixinTargets(request: MixinFacadeRequest, context: AnnotationContext): List<String> {
        val source = request.bufferText
        val imports = JavaTypeDescriptorResolver.importsFor(source)
        request.semanticModel?.targets
            ?.mapNotNull { MixinTargetResolver.resolveTarget(it.internalName, classIndex, imports) }
            ?.takeIf { it.isNotEmpty() }
            ?.let { return it }
        val rawTargets = context.mixinTargetInternalNames.ifEmpty {
            AnnotationContextExtractor.resolveRawMixinTargets(source, context.valueStartOffset)
        }
        return MixinTargetResolver.resolveTargets(rawTargets, classIndex, imports)
    }

    private fun AnnotationContext.withResolvedMixinTargets(request: MixinFacadeRequest): AnnotationContext =
        copy(mixinTargetInternalNames = resolveMixinTargets(request, this))

    private fun completeShadow(request: MixinFacadeRequest, context: AnnotationContext): List<McCompletionItem> {
        if (context.slot != AnnotationSlot.SHADOW_MEMBER) return emptyList()
        val prefix = context.partialValue
        val targets = resolveMixinTargets(request, context)
        val fields = shadowValidation.completeFields(targets, prefix, context.shadowPrefix)
        val methods = shadowValidation.completeMethods(targets, prefix, context.shadowPrefix)
        val fieldItems = fields.map { field ->
            McCompletionItem(
                label = field.name,
                detail = field.readableType,
                documentation = field.descriptor,
                filterText = "${field.name} ${field.readableType}",
                insertText = field.name,
                kind = McCompletionKind.FIELD,
                sortKey = "0700_${field.name}",
                metadata = McCompletionMetadata(source = "mixin.shadow", name = field.name, descriptor = field.descriptor),
            )
        }
        val methodItems = methods.map { method ->
            McCompletionItem(
                label = method.readableSignature,
                detail = method.descriptor,
                documentation = method.readableSignature,
                filterText = "${method.name} ${method.readableSignature}",
                insertText = method.name,
                kind = McCompletionKind.VALUE,
                sortKey = "0701_${method.name}",
                metadata = McCompletionMetadata(
                    source = "mixin.shadow",
                    name = method.name,
                    descriptor = method.descriptor,
                ),
            )
        }
        return fieldItems + methodItems
    }

    private fun completeOverwrite(request: MixinFacadeRequest, context: AnnotationContext): List<McCompletionItem> {
        if (context.slot != AnnotationSlot.OVERWRITE_METHOD) return emptyList()
        val targets = resolveMixinTargets(request, context)
        return targets
            .flatMap { owner ->
                classIndex.getMethods(owner)
                    .filter { it.name.startsWith(context.partialValue) }
                    .map { owner to it }
            }
            .distinctBy { (_, method) -> method.name to method.descriptor }
            .map { (owner, method) ->
                McCompletionItem(
                    label = method.readableSignature,
                    detail = AnnotationContextExtractor.internalToFqn(owner),
                    documentation = method.descriptor,
                    filterText = "${method.name} ${method.readableSignature}",
                    insertText = method.name,
                    kind = McCompletionKind.VALUE,
                    sortKey = "0702_${method.name}",
                    metadata = McCompletionMetadata(
                        source = "mixin.overwrite",
                        owner = owner,
                        name = method.name,
                        descriptor = method.descriptor,
                    ),
                )
            }
    }

    private fun diagnoseStandardInjectorSugarConstraints(request: MixinFacadeRequest): List<McDiagnostic> {
        val source = request.bufferText
        val imports = JavaTypeDescriptorResolver.importsFor(source)

        val standardInjectorAnnotations =
            MixinExtrasAnnotation.sugarHandlerInjectorAnnotations - MixinExtrasAnnotation.injectorAnnotations
        val diagnostics = mutableListOf<McDiagnostic>()
        for (site in HandlerSignatureService.findSugarHandlerAnnotationSites(source)) {
            if (site.annotation !in standardInjectorAnnotations) continue
            val handler = site.handlerMethod ?: continue
            val annotationOffset = AnnotationContextExtractor.toOffset(
                source,
                site.annotationRange.start.line,
                site.annotationRange.start.character,
            ) ?: continue
            val scope = AnnotationContextExtractor.resolveMixinClassScope(source, annotationOffset) ?: continue
            val mixinTargets = MixinTargetResolver.resolveTargets(scope.rawTargets, classIndex, imports)
            if (mixinTargets.isEmpty()) continue
            val enriched = HandlerSignatureService.enrichHandlerTypes(handler, classIndex)
            val targetMethod = handlerSignatureService.resolveTargetMethod(mixinTargets, site.methodAttribute)
            val issues = handlerSignatureService.validateCommonSugarConstraints(
                enriched,
                targetMethod,
                site.annotation,
            )
            diagnostics += issues.map { issue ->
                McDiagnostic(
                    code = issue.code,
                    severity = McSeverity.ERROR,
                    message = issue.message,
                    range = issue.range,
                )
            }
        }
        return diagnostics.distinctBy { Triple(it.code, it.message, it.range) }
    }

    private fun diagnoseLocalCaptures(request: MixinFacadeRequest): List<McDiagnostic> {
        val source = request.bufferText
        val mixinTargets = MixinTargetResolver.resolveTargetsFromSource(source, classIndex)
        if (mixinTargets.isEmpty()) return emptyList()

        val diagnostics = mutableListOf<McDiagnostic>()
        for (site in HandlerSignatureService.findSugarHandlerAnnotationSites(source)) {
            if (site.annotation == MixinExtrasAnnotation.WRAP_METHOD) continue
            val handler = site.handlerMethod ?: continue
            for (parameter in handler.parameters) {
                if (parameter.sugarSpec !is HandlerParameterSugarSpec.Local) continue
                val results = resolveLocalCaptures(request, site, parameter, mixinTargets)
                if (results.isEmpty()) continue

                val range = parameter.sugarAnnotationRange ?: parameter.range ?: site.annotationRange
                when {
                    results.any { it is LocalCaptureValidationService.Result.Ambiguous } -> {
                        diagnostics += McDiagnostic(
                            code = MixinExtrasDiagnosticCodes.LOCAL_CAPTURE_AMBIGUOUS,
                            severity = McSeverity.ERROR,
                            message = "Local capture is ambiguous; specify ordinal, index, or name.",
                            range = range,
                        )
                    }
                    results.any { it is LocalCaptureValidationService.Result.NotFound } -> {
                        diagnostics += McDiagnostic(
                            code = MixinExtrasDiagnosticCodes.LOCAL_CAPTURE_NOT_FOUND,
                            severity = McSeverity.ERROR,
                            message = "Could not find a matching local variable",
                            range = range,
                        )
                    }
                }
            }
        }
        return diagnostics.distinct()
    }

    private fun analyzeMemberDeclarations(request: MixinFacadeRequest): List<McDiagnostic> {
        val source = request.bufferText
        val mixinTargets = MixinTargetResolver.resolveTargetsFromSource(source, classIndex)
        if (mixinTargets.isEmpty()) return emptyList()
        val diagnostics = mutableListOf<McDiagnostic>()
        semanticShadowDeclarations(request).forEach { declaration ->
            val prefix = MixinMemberDeclarationParser.findShadowPrefix(source)
            val remap = MixinMemberDeclarationParser.findShadowRemap(source)
            diagnostics += shadowValidation.validate(mixinTargets, declaration, prefix, remap)
        }
        semanticAccessorDeclarations(request).forEach { declaration ->
            diagnostics += accessorService.validate(mixinTargets, declaration)
        }
        semanticInvokerDeclarations(request).forEach { declaration ->
            diagnostics += invokerService.validate(mixinTargets, declaration)
        }
        return diagnostics
    }

    private fun semanticShadowDeclarations(request: MixinFacadeRequest): List<ShadowMemberDeclaration> {
        val members = request.semanticModel?.members
            ?.filter { it.annotationKind == MixinMemberAnnotationKind.SHADOW }
            .orEmpty()
        if (members.isEmpty()) return MixinMemberDeclarationParser.parseShadowDeclarations(request.bufferText, classIndex)
        return members.mapNotNull { member ->
            val descriptor = member.methodDescriptor ?: member.returnDescriptor ?: return@mapNotNull null
            ShadowMemberDeclaration(
                name = member.javaName,
                isMethod = member.methodDescriptor != null,
                descriptor = descriptor,
                isStatic = JavaModifier.STATIC in member.modifiers,
                range = member.range,
                parseSource = member.parseSource,
                confidence = member.confidence,
                warnings = member.warnings,
            )
        }
    }

    private fun semanticAccessorDeclarations(request: MixinFacadeRequest): List<AccessorMethodDeclaration> {
        val members = request.semanticModel?.members
            ?.filter { it.annotationKind == MixinMemberAnnotationKind.ACCESSOR }
            .orEmpty()
        if (members.isEmpty()) return MixinMemberDeclarationParser.parseAccessorDeclarations(request.bufferText, classIndex)
        return members.mapNotNull { member ->
            AccessorMethodDeclaration(
                methodName = member.javaName,
                returnTypeDescriptor = member.returnDescriptor ?: return@mapNotNull null,
                parameterDescriptors = member.parameterDescriptors,
                explicitFieldName = member.explicitTargetName,
                range = member.range,
                parseSource = member.parseSource,
                confidence = member.confidence,
                warnings = member.warnings,
            )
        }
    }

    private fun semanticInvokerDeclarations(request: MixinFacadeRequest): List<InvokerMethodDeclaration> {
        val members = request.semanticModel?.members
            ?.filter { it.annotationKind == MixinMemberAnnotationKind.INVOKER }
            .orEmpty()
        if (members.isEmpty()) return MixinMemberDeclarationParser.parseInvokerDeclarations(request.bufferText, classIndex)
        return members.mapNotNull { member ->
            InvokerMethodDeclaration(
                methodName = member.javaName,
                parameterDescriptors = member.parameterDescriptors,
                returnTypeDescriptor = member.returnDescriptor ?: return@mapNotNull null,
                explicitTargetName = member.explicitTargetName,
                range = member.range,
                parseSource = member.parseSource,
                confidence = member.confidence,
                warnings = member.warnings,
            )
        }
    }

    private fun debugInfo(
        request: MixinFacadeRequest,
        command: String,
        languageId: String,
        context: AnnotationContext?,
        semanticContext: MixinCompletionContext?,
        fallbackAnnotationContextUsed: Boolean,
        fallbackAnnotationContextReason: String?,
        items: List<McCompletionItem>,
        warnings: List<String>,
    ): McdevCompletionDebugInfo {
        val semanticModel = request.semanticModel
        val parsedMethod = when (semanticContext) {
            is MixinCompletionContext.AtTarget -> semanticContext.methodName?.let {
                MethodTarget(it, semanticContext.methodDescriptor)
            }
            else -> context?.injectMethodName?.let(::parseMethodTarget)
        }
        val zeroReason = when {
            context == null -> "NO_COMPLETION_CONTEXT"
            fallbackAnnotationContextUsed && items.isEmpty() -> "FALLBACK_ANNOTATION_CONTEXT_USED"
            semanticModel?.targets.isNullOrEmpty() && context.annotation != MixinAnnotation.MIXIN -> "NO_MIXIN_TARGET"
            items.isEmpty() -> "NO_CANDIDATES"
            else -> null
        }
        return McdevCompletionDebugInfo(
            command = command,
            documentUri = request.documentUri,
            languageId = languageId,
            parseSource = semanticModel?.parseSource,
            parseConfidence = semanticModel?.confidence,
            usedCompilationUnit = semanticModel?.debugInfo?.usedCompilationUnit ?: false,
            usedJavaProject = semanticModel?.debugInfo?.usedJavaProject ?: false,
            bindingResolvedCount = semanticModel?.debugInfo?.bindingResolvedCount ?: 0,
            bindingFailedCount = semanticModel?.debugInfo?.bindingFailedCount ?: 0,
            fallbackReason = semanticModel?.debugInfo?.fallbackReason,
            semanticContextFound = semanticContext != null,
            fallbackAnnotationContextUsed = fallbackAnnotationContextUsed,
            fallbackAnnotationContextReason = fallbackAnnotationContextReason,
            semanticTargetCount = semanticModel?.targets?.size ?: 0,
            semanticMemberCount = semanticModel?.members?.size ?: 0,
            completionContextKind = semanticContext?.javaClass?.simpleName ?: context?.slot?.name,
            owner = semanticModel?.targets?.firstOrNull()?.internalName,
            methodName = parsedMethod?.name,
            methodDescriptor = parsedMethod?.descriptor,
            candidateCountBeforeFilter = items.size,
            candidateCountAfterFilter = items.size,
            zeroItemReason = zeroReason,
            warnings = warnings,
        )
    }
}
