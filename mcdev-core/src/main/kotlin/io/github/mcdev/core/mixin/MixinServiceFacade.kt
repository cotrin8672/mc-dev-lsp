package io.github.mcdev.core.mixin

import io.github.mcdev.core.codeaction.McFix
import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata
import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.core.mixinextras.ExpressionSupport
import io.github.mcdev.core.mixinextras.MixinExtrasCodeActionService
import io.github.mcdev.core.mixinextras.MixinExtrasCompletionService
import io.github.mcdev.core.mixinextras.MixinExtrasDiagnosticRequest
import io.github.mcdev.core.mixinextras.MixinExtrasDiagnosticsService

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
)

class MixinServiceFacade(
    private val classIndex: ClassIndex,
    private val bytecodeIndex: BytecodeIndex,
    private val mixinTargetCompletion: MixinTargetCompletionService = MixinTargetCompletionService(classIndex),
    private val injectMethodCompletion: InjectMethodCompletionService = InjectMethodCompletionService(classIndex),
    private val atValueCompletion: AtValueCompletionService = AtValueCompletionService(),
    private val atTargetCompletion: AtTargetCompletionService = AtTargetCompletionService(),
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
    private val mixinExtrasCodeActions: MixinExtrasCodeActionService = MixinExtrasCodeActionService(classIndex),
    private val expressionSupport: ExpressionSupport = ExpressionSupport(),
) {
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
        val items = routedItems.ifEmpty {
            expressionSupport.completeFeatureAnnotations(
                effectiveRequest.bufferText,
                effectiveRequest.line,
                effectiveRequest.character,
            )
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
            ),
        )
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
        )
        return (mixinFixes + extrasFixes).distinctBy { it.title }
    }

    private fun routeCompletion(
        request: MixinFacadeRequest,
        context: AnnotationContext,
        options: MixinCompletionOptions,
    ): List<McCompletionItem> {
        val source = request.bufferText
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
                AnnotationSlot.VALUE -> {
                    val expressionItems = expressionSupport.completeAtValue(context)
                    if (expressionItems.isNotEmpty()) expressionItems else atValueCompletion.complete(context)
                }
                AnnotationSlot.TARGET -> {
                    val extrasItems = mixinExtrasCompletion.complete(context.withResolvedMixinTargets(request), options)
                    if (extrasItems.isNotEmpty()) extrasItems else completeAtTarget(request, context)
                }
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
            else -> emptyList()
        }
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
        val owner = owners.firstOrNull() ?: return emptyList()
        val methodTarget = parseMethodTarget(
            context.injectMethodName
            ?: return emptyList(),
        )
        val atValue = context.atValue
            ?: return emptyList()
        val candidates = bytecodeIndex.getAtTargetCandidates(
            owner,
            methodTarget.name,
            methodTarget.descriptor,
            atValue,
        )
        return atTargetCompletion.complete(context, candidates)
    }

    private data class MethodTarget(
        val name: String,
        val descriptor: String?,
    )

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
        request.semanticModel?.targets
            ?.mapNotNull {
                MixinTargetResolver.resolveTarget(it.internalName, classIndex)
                    ?: it.internalName.takeIf { name -> '/' in name }
            }
            ?.takeIf { it.isNotEmpty() }
            ?.let { return it }
        val rawTargets = context.mixinTargetInternalNames.ifEmpty {
            AnnotationContextExtractor.resolveRawMixinTargets(source, context.valueStartOffset)
        }
        return MixinTargetResolver.resolveTargets(rawTargets, classIndex, JavaTypeDescriptorResolver.importsFor(source))
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
                kind = McCompletionKind.METHOD,
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
