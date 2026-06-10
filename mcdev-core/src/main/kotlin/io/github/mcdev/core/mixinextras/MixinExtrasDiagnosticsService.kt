package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.core.diagnostics.McSeverity
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.MixinTargetResolver

data class MixinExtrasDiagnosticRequest(
    val source: String,
    val documentUri: String,
)

class MixinExtrasDiagnosticsService(
    private val classIndex: ClassIndex,
    private val signatureService: HandlerSignatureService = HandlerSignatureService(classIndex),
) {
    fun analyze(request: MixinExtrasDiagnosticRequest): List<McDiagnostic> {
        val mixinTargets = MixinTargetResolver.resolveTargetsFromSource(request.source, classIndex)
        val sites = HandlerSignatureService.findAnnotationSites(request.source)
        val diagnostics = mutableListOf<McDiagnostic>()
        for (site in sites) {
            if (site.atValue.equals("MIXINEXTRAS:EXPRESSION", ignoreCase = true) && site.handlerMethod != null) {
                diagnostics += McDiagnostic(
                    code = MixinExtrasDiagnosticCodes.UNSUPPORTED_EXPRESSION_CONTEXT,
                    severity = McSeverity.WARNING,
                    message = "Expression handler validation is limited for MIXINEXTRAS:EXPRESSION",
                    range = site.handlerMethod.range,
                )
            }
            val handler = site.handlerMethod ?: continue
            val enriched = HandlerSignatureService.enrichHandlerTypes(handler, classIndex)
            val issues = signatureService.validateHandler(site, mixinTargets, enriched)
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
