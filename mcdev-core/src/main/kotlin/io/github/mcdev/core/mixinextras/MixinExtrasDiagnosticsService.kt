package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.at.AtTextPositions
import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.core.diagnostics.McSeverity
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mixin.AnnotationContextExtractor
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.JavaTypeDescriptorResolver
import io.github.mcdev.core.mixin.MixinTargetResolver

data class MixinExtrasDiagnosticRequest(
    val source: String,
    val documentUri: String,
    val resolvedContexts: List<ResolvedMixinExtrasContext> = emptyList(),
)

class MixinExtrasDiagnosticsService(
    private val classIndex: ClassIndex,
    private val bytecodeIndex: BytecodeIndex,
    private val signatureService: HandlerSignatureService = HandlerSignatureService(classIndex, bytecodeIndex),
) {
    fun analyze(request: MixinExtrasDiagnosticRequest): List<McDiagnostic> {
        val sites = HandlerSignatureService.findAnnotationSites(request.source)
        val diagnostics = mutableListOf<McDiagnostic>()
        diagnostics += shareTypeConflictDiagnostics(
            request.source,
            HandlerSignatureService.findSugarHandlerAnnotationSites(request.source),
        )
        diagnostics += definitionDiagnostics(request)
        for (site in sites) {
            val siteTargets = AnnotationContextExtractor.toOffset(
                request.source,
                site.annotationRange.start.line,
                site.annotationRange.start.character,
            )?.let { AnnotationContextExtractor.resolveMixinClassScope(request.source, it) }
                ?.let { scope ->
                    MixinTargetResolver.resolveTargets(
                        scope.rawTargets,
                        classIndex,
                        JavaTypeDescriptorResolver.importsFor(request.source),
                    )
                }
                ?: emptyList()
            val selected = selectResolvedMixinExtrasContext(site, request.resolvedContexts)
            val resolvedContext = selected?.context
            if (site.atValue.equals("MIXINEXTRAS:EXPRESSION", ignoreCase = true)) {
                val handler = site.handlerMethod
                val expressionContext = resolvedContext
                    ?: ExpressionContextResolver.resolveExpressionContext(request.source, site)
                val expressionValues = expressionContext.expressionValuesForAtId(site.atId)
                if (expressionValues.isEmpty()) {
                    diagnostics += McDiagnostic(
                        code = MixinExtrasDiagnosticCodes.MISSING_EXPRESSION_ANNOTATION,
                        severity = McSeverity.ERROR,
                        message = "MixinExtras expression handler requires @Expression annotation",
                        range = handler?.range ?: site.annotationRange,
                    )
                } else if (site.annotation == MixinExtrasAnnotation.MODIFY_EXPRESSION_VALUE) {
                    val targetMethod = signatureService.resolveTargetMethod(siteTargets, site.methodAttribute)
                    if (targetMethod != null) {
                        val inferred = signatureService.inferExpressionValueType(
                            source = request.source,
                            site = site,
                            targetMethod = targetMethod,
                            mixinTargets = siteTargets,
                            resolvedContext = resolvedContext,
                        )
                        if (inferred == null) {
                            diagnostics += McDiagnostic(
                                code = MixinExtrasDiagnosticCodes.UNSUPPORTED_EXPRESSION_CONTEXT,
                                severity = McSeverity.WARNING,
                                message = "Could not infer expression type for: ${expressionValues.joinToString(", ")}",
                                range = handler?.range ?: site.annotationRange,
                            )
                        }
                    }
                }
            }
            if (site.annotation == MixinExtrasAnnotation.WRAP_WITH_CONDITION) {
                wrapWithConditionTargetDiagnostic(
                    site,
                    signatureService.wrapWithConditionTargetStatus(
                        site = site,
                        mixinTargets = siteTargets,
                        source = request.source,
                        resolvedContext = resolvedContext,
                    ),
                )?.let { diagnostics += it }
            }
            val handler = site.handlerMethod ?: continue
            val enriched = HandlerSignatureService.enrichHandlerTypes(handler, classIndex)
            if (site.annotation == MixinExtrasAnnotation.WRAP_OPERATION ||
                site.annotation == MixinExtrasAnnotation.WRAP_METHOD
            ) {
                val expectedSignature = signatureService.expectedSignature(
                    request.source,
                    site,
                    siteTargets,
                    resolvedContext,
                )
                val operationCallLayout = OperationCallValidator.resolveOperationCallLayout(
                    site,
                    enriched,
                    expectedSignature,
                )
                diagnostics += OperationCallValidator.validate(request.source, site, operationCallLayout).map { issue ->
                    McDiagnostic(
                        code = MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS,
                        severity = McSeverity.ERROR,
                        message = "Operation.call called with the wrong number of arguments",
                        range = issue.argumentRange,
                        metadata = siteDiagnosticMetadata(site) + mapOf(
                            "expectedArguments" to issue.expectedNames.joinToString(", "),
                        ),
                    )
                }
            }
            val issues = signatureService.validateHandler(
                request.source,
                site,
                siteTargets,
                enriched,
                resolvedContext = resolvedContext,
            )
            diagnostics += issues.map { issue ->
                McDiagnostic(
                    code = issue.code,
                    severity = McSeverity.ERROR,
                    message = issue.message,
                    range = issue.range,
                    metadata = siteDiagnosticMetadata(site),
                )
            }
        }
        return diagnostics.distinct()
    }

    private fun definitionDiagnostics(request: MixinExtrasDiagnosticRequest): List<McDiagnostic> {
        val resolver = ClassLiteralTypeNameResolver.forSource(request.source, classIndex)
        return HandlerSignatureService.findSugarHandlerAnnotationSites(request.source).flatMap { site ->
            val context = selectResolvedMixinExtrasContext(site, request.resolvedContexts)?.context
                ?: ExpressionContextResolver.resolveExpressionContext(request.source, site)
            context.definitionIndex.definitions.flatMap { definition ->
                val offsets = definition.idSourceRange ?: definition.sourceRange
                val range = offsets?.let {
                    AtTextPositions.rangeForOffsets(request.source, it.first, it.last + 1)
                } ?: site.annotationRange
                OfficialExpressionIdentifierPoolBuilder.build(
                    MixinExtrasDefinitionIndex(listOf(definition)), resolver,
                ).issues.map { issue ->
                    McDiagnostic(
                        code = MixinExtrasDiagnosticCodes.INVALID_DEFINITION,
                        severity = McSeverity.WARNING,
                        message = "Cannot resolve @Definition ${issue.attribute}: ${issue.message} (${issue.rawValue})",
                        range = issue.sourceRange?.let {
                            AtTextPositions.rangeForOffsets(request.source, it.first, it.last + 1)
                        } ?: range,
                    )
                }
            }
        }
    }

    private fun shareTypeConflictDiagnostics(
        source: String,
        sites: List<MixinExtrasAnnotationSite>,
    ): List<McDiagnostic> {
        val imports = JavaTypeDescriptorResolver.importsFor(source)
        val claims = linkedMapOf<String, MutableMap<McTextRange, String>>()
        for (site in sites) {
            val scope = AnnotationContextExtractor.toOffset(
                source,
                site.annotationRange.start.line,
                site.annotationRange.start.character,
            )?.let { AnnotationContextExtractor.resolveMixinClassScope(source, it) }
                ?: continue
            val siteTargets = MixinTargetResolver.resolveTargets(scope.rawTargets, classIndex, imports)
            val target = signatureService.resolveTargetMethod(siteTargets, site.methodAttribute) ?: continue
            val handler = site.handlerMethod ?: continue
            val enriched = HandlerSignatureService.enrichHandlerTypes(handler, classIndex)
            for (owner in siteTargets) {
                if (classIndex.getMethods(owner).count { it.name == target.name && it.descriptor == target.descriptor } != 1) {
                    continue
                }
                for (parameter in enriched.parameters) {
                    val share = parameter.sugarSpec as? HandlerParameterSugarSpec.Share ?: continue
                    val value = share.value ?: continue
                    val namespace = if (share.namespaceSpecified) share.namespace ?: continue
                    else scope.className.takeIf { it.isNotBlank() } ?: continue
                    val descriptor = HandlerSignatureService.shareRefValueDescriptor(
                        parameter.typeDescriptor ?: continue,
                    ) ?: continue
                    val range = parameter.sugarAnnotationRange ?: continue
                    val key = "$owner\u0000${target.name}\u0000${target.descriptor}\u0000$namespace\u0000$value"
                    claims.getOrPut(key) { linkedMapOf() }[range] = descriptor
                }
            }
        }
        return claims.values.mapNotNull { byRange ->
            if (byRange.values.toSet().size < 2) return@mapNotNull null
            val range = byRange.keys.maxWithOrNull(compareBy<McTextRange>({ it.start.line }, { it.start.character }))
                ?: return@mapNotNull null
            McDiagnostic(
                code = MixinExtrasDiagnosticCodes.SHARE_TYPE_CONFLICT,
                severity = McSeverity.ERROR,
                message = "Conflicting @Share reference types for the same target",
                range = range,
            )
        }
    }

    private fun wrapWithConditionTargetDiagnostic(
        site: MixinExtrasAnnotationSite,
        status: WrapWithConditionTargetStatus,
    ): McDiagnostic? = when (status) {
        WrapWithConditionTargetStatus.VALID_VOID,
        WrapWithConditionTargetStatus.UNRESOLVED,
        -> null
        WrapWithConditionTargetStatus.VALID_POPPED_NON_VOID -> McDiagnostic(
            code = MixinExtrasDiagnosticCodes.WRAP_WITH_CONDITION_POPPED_NON_VOID,
            severity = McSeverity.WARNING,
            message = "WrapWithCondition is targeting a non-void instruction",
            range = site.annotationRange,
            metadata = siteDiagnosticMetadata(site),
        )
        WrapWithConditionTargetStatus.INVALID_RETAINED_NON_VOID -> McDiagnostic(
            code = MixinExtrasDiagnosticCodes.WRAP_WITH_CONDITION_INVALID_TARGET,
            severity = McSeverity.ERROR,
            message = "WrapWithCondition cannot target a retained non-void invocation",
            range = site.annotationRange,
            metadata = siteDiagnosticMetadata(site),
        )
        WrapWithConditionTargetStatus.INVALID_INSTRUCTION -> McDiagnostic(
            code = MixinExtrasDiagnosticCodes.WRAP_WITH_CONDITION_INVALID_TARGET,
            severity = McSeverity.ERROR,
            message = "WrapWithCondition only supports void method invocations, immediately popped invocations, and field writes",
            range = site.annotationRange,
            metadata = siteDiagnosticMetadata(site),
        )
    }

    private fun siteDiagnosticMetadata(site: MixinExtrasAnnotationSite): Map<String, String> = mapOf(
        "annotation" to site.annotation.simpleName,
        "method" to site.methodAttribute,
    )
}
