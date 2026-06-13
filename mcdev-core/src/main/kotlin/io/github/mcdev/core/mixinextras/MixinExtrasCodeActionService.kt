package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.codeaction.McFix
import io.github.mcdev.core.codeaction.McTextEdit
import io.github.mcdev.core.codeaction.WorkspaceEditFix
import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.MixinTargetResolver

class MixinExtrasCodeActionService(
    private val classIndex: ClassIndex,
    private val signatureService: HandlerSignatureService = HandlerSignatureService(classIndex),
) {
    fun fixesForDiagnostics(
        diagnostics: List<McDiagnostic>,
        documentUri: String,
        source: String,
    ): List<McFix> {
        val mixinTargets = MixinTargetResolver.resolveTargetsFromSource(source, classIndex)
        val sites = HandlerSignatureService.findAnnotationSites(source)
        val fixes = mutableListOf<McFix>()
        for (diagnostic in diagnostics) {
            val site = sites.find { site ->
                site.handlerMethod?.range == diagnostic.range ||
                    diagnostic.metadata["method"] == site.methodAttribute
            } ?: sites.firstOrNull { diagnostic.metadata["annotation"] == it.annotation.simpleName }
            if (site == null) continue
            when (diagnostic.code) {
                MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH,
                MixinExtrasDiagnosticCodes.MISSING_OPERATION_PARAMETER,
                MixinExtrasDiagnosticCodes.WRONG_OPERATION_GENERIC,
                MixinExtrasDiagnosticCodes.WRONG_ORIGINAL_VALUE_TYPE,
                MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE,
                    -> {
                    val fix = fixHandlerSignatureFix(documentUri, source, site, mixinTargets)
                    if (fix != null) fixes += fix
                }
            }
            if (site.handlerMethod == null) {
                val fix = generateHandlerFix(documentUri, source, site, mixinTargets)
                if (fix != null) fixes += fix
            }
        }
        return fixes.distinctBy { it.title }
    }

    fun generateHandlerFixes(
        documentUri: String,
        source: String,
        site: MixinExtrasAnnotationSite,
        mixinTargets: List<String>,
    ): List<McFix> = listOfNotNull(generateHandlerFix(documentUri, source, site, mixinTargets))

    fun applyHandlerFix(
        fix: WorkspaceEditFix,
        currentSource: String,
    ): WorkspaceEditFix = fix

    private fun generateHandlerFix(
        documentUri: String,
        source: String,
        site: MixinExtrasAnnotationSite,
        mixinTargets: List<String>,
    ): WorkspaceEditFix? {
        val stub = signatureService.generateHandlerStub(source, site, mixinTargets) ?: return null
        val insertOffset = findInsertOffset(source, site)
        val title = when (site.annotation) {
            MixinExtrasAnnotation.WRAP_OPERATION -> "Generate WrapOperation handler"
            MixinExtrasAnnotation.MODIFY_EXPRESSION_VALUE -> "Generate ModifyExpressionValue handler"
            MixinExtrasAnnotation.MODIFY_RETURN_VALUE -> "Generate ModifyReturnValue handler"
            MixinExtrasAnnotation.WRAP_WITH_CONDITION -> "Generate WrapWithCondition handler"
            MixinExtrasAnnotation.WRAP_METHOD -> "Generate WrapMethod handler"
            else -> "Generate MixinExtras handler"
        }
        return WorkspaceEditFix(
            title = title,
            kind = "quickfix.mixinextras.generateHandler",
            documentUri = documentUri,
            edits = listOf(McTextEdit(insertOffset, insertOffset, "\n$stub")),
            metadata = mapOf("annotation" to site.annotation.simpleName),
        )
    }

    private fun fixHandlerSignatureFix(
        documentUri: String,
        source: String,
        site: MixinExtrasAnnotationSite,
        mixinTargets: List<String>,
    ): WorkspaceEditFix? {
        val handler = site.handlerMethod ?: return null
        val stub = signatureService.generateHandlerStub(source, site, mixinTargets, handler.methodName) ?: return null
        val title = when (site.annotation) {
            MixinExtrasAnnotation.WRAP_OPERATION -> "Fix WrapOperation handler signature"
            MixinExtrasAnnotation.MODIFY_EXPRESSION_VALUE -> "Fix ModifyExpressionValue handler signature"
            MixinExtrasAnnotation.MODIFY_RETURN_VALUE -> "Fix ModifyReturnValue handler signature"
            MixinExtrasAnnotation.WRAP_WITH_CONDITION -> "Fix WrapWithCondition handler signature"
            MixinExtrasAnnotation.WRAP_METHOD -> "Fix WrapMethod handler signature"
            else -> "Fix MixinExtras handler signature"
        }
        val start = rangeStart(source, handler.range)
        val end = findMethodBodyEnd(source, start)
        return WorkspaceEditFix(
            title = title,
            kind = "quickfix.mixinextras.fixHandlerSignature",
            documentUri = documentUri,
            edits = listOf(McTextEdit(start, end, stub.trimEnd() + "\n")),
            metadata = mapOf("annotation" to site.annotation.simpleName),
        )
    }

    private fun findInsertOffset(source: String, site: MixinExtrasAnnotationSite): Int {
        val annotationEnd = rangeEnd(source, site.annotationRange)
        return annotationEnd
    }

    private fun findMethodBodyEnd(source: String, methodStart: Int): Int {
        val brace = source.indexOf('{', methodStart)
        if (brace < 0) return methodStart
        var depth = 0
        var i = brace
        while (i < source.length) {
            when (source[i]) {
                '{' -> depth++
                '}' -> {
                    depth--
                    if (depth == 0) return i + 1
                }
            }
            i++
        }
        return source.length
    }

    private fun rangeStart(source: String, range: io.github.mcdev.core.diagnostics.McTextRange): Int {
        val lineStart = source.lineSequence().take(range.start.line).sumOf { it.length + 1 }
        return lineStart + range.start.character
    }

    private fun rangeEnd(source: String, range: io.github.mcdev.core.diagnostics.McTextRange): Int {
        val lineStart = source.lineSequence().take(range.end.line).sumOf { it.length + 1 }
        return lineStart + range.end.character
    }
}
