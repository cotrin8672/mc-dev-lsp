package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.core.diagnostics.McSeverity
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.MixinTargetResolver

data class MixinExtrasDiagnosticRequest(
    val source: String,
    val documentUri: String,
)

class MixinExtrasDiagnosticsService(
    private val classIndex: ClassIndex,
    private val bytecodeIndex: BytecodeIndex,
    private val signatureService: HandlerSignatureService = HandlerSignatureService(classIndex, bytecodeIndex),
) {
    fun analyze(request: MixinExtrasDiagnosticRequest): List<McDiagnostic> {
        val mixinTargets = MixinTargetResolver.resolveTargetsFromSource(request.source, classIndex)
        val sites = HandlerSignatureService.findAnnotationSites(request.source)
        val diagnostics = mutableListOf<McDiagnostic>()
        for (site in sites) {
            if (site.atValue.equals("MIXINEXTRAS:EXPRESSION", ignoreCase = true)) {
                val handler = site.handlerMethod
                val expressionContext = ExpressionContextResolver.parseHandlerAnnotations(
                    ExpressionContextResolver.handlerRegion(request.source, site),
                )
                if (expressionContext.expression.isNullOrBlank()) {
                    diagnostics += McDiagnostic(
                        code = MixinExtrasDiagnosticCodes.MISSING_EXPRESSION_ANNOTATION,
                        severity = McSeverity.ERROR,
                        message = "MixinExtras expression handler requires @Expression annotation",
                        range = handler?.range ?: site.annotationRange,
                    )
                } else {
                    val targetMethod = signatureService.resolveTargetMethod(mixinTargets, site.methodAttribute)
                    if (targetMethod != null) {
                        val inferred = ExpressionContextResolver.inferExpressionValueType(
                            source = request.source,
                            site = site,
                            targetMethod = targetMethod,
                            mixinTargets = mixinTargets,
                            bytecodeIndex = bytecodeIndex,
                            classIndex = classIndex,
                        )
                        if (inferred == null) {
                            diagnostics += McDiagnostic(
                                code = MixinExtrasDiagnosticCodes.UNSUPPORTED_EXPRESSION_CONTEXT,
                                severity = McSeverity.WARNING,
                                message = "Could not infer expression type for: ${expressionContext.expression}",
                                range = handler?.range ?: site.annotationRange,
                            )
                        }
                    }
                }
            }
            val handler = site.handlerMethod ?: continue
            val enriched = HandlerSignatureService.enrichHandlerTypes(handler, classIndex)
            val issues = signatureService.validateHandler(request.source, site, mixinTargets, enriched)
            diagnostics += issues.map { issue ->
                McDiagnostic(
                    code = issue.code,
                    severity = McSeverity.ERROR,
                    message = issue.message,
                    range = issue.range,
                    metadata = mapOf(
                        "annotation" to site.annotation.simpleName,
                        "method" to site.methodAttribute,
                    ),
                )
            }
        }
        return diagnostics
    }
}
