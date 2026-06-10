package io.github.mcdev.core.awat

import io.github.mcdev.core.at.AccessTransformerCodeActionService
import io.github.mcdev.core.at.AccessTransformerCompletionService
import io.github.mcdev.core.at.AtContextExtractor
import io.github.mcdev.core.at.AtDiagnosticRequest
import io.github.mcdev.core.at.AccessTransformerDiagnosticsService
import io.github.mcdev.core.aw.AccessWidenerCodeActionService
import io.github.mcdev.core.aw.AccessWidenerCompletionService
import io.github.mcdev.core.aw.AccessWidenerDiagnosticRequest
import io.github.mcdev.core.aw.AccessWidenerDiagnosticsService
import io.github.mcdev.core.aw.AwContextExtractor
import io.github.mcdev.core.codeaction.McFix
import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.core.mapping.ProjectMappingContext
import io.github.mcdev.core.mixin.ClassIndex

data class AwAtFacadeRequest(
    val bufferText: String,
    val line: Int,
    val character: Int,
    val documentUri: String,
    val fileType: AwAtFileType,
)

class AwAtServiceFacade(
    private val classIndex: ClassIndex,
    private val mappingContext: ProjectMappingContext? = null,
    private val awCompletion: AccessWidenerCompletionService = AccessWidenerCompletionService(classIndex),
    private val awDiagnostics: AccessWidenerDiagnosticsService = AccessWidenerDiagnosticsService(classIndex),
    private val awCodeActions: AccessWidenerCodeActionService = AccessWidenerCodeActionService(),
    private val atCompletion: AccessTransformerCompletionService = AccessTransformerCompletionService(classIndex),
    private val atDiagnostics: AccessTransformerDiagnosticsService = AccessTransformerDiagnosticsService(classIndex, mappingContext),
    private val atCodeActions: AccessTransformerCodeActionService = AccessTransformerCodeActionService(classIndex, mappingContext),
) {
    fun complete(request: AwAtFacadeRequest): List<McCompletionItem> =
        when (request.fileType) {
            AwAtFileType.ACCESS_WIDENER -> {
                val offset = AwContextExtractor.toOffset(request.bufferText, request.line, request.character)
                    ?: return emptyList()
                val context = AwContextExtractor.extractAtOffset(request.bufferText, offset) ?: return emptyList()
                awCompletion.complete(context, mappingContext)
            }
            AwAtFileType.ACCESS_TRANSFORMER -> {
                val context = AtContextExtractor.extract(request.bufferText, request.line, request.character)
                    ?: return emptyList()
                atCompletion.complete(context, mappingContext)
            }
        }

    fun diagnose(request: AwAtFacadeRequest): List<McDiagnostic> =
        when (request.fileType) {
            AwAtFileType.ACCESS_WIDENER -> awDiagnostics.analyze(
                AccessWidenerDiagnosticRequest(
                    source = request.bufferText,
                    documentUri = request.documentUri,
                    mappingContext = mappingContext,
                ),
            )
            AwAtFileType.ACCESS_TRANSFORMER -> atDiagnostics.analyze(
                AtDiagnosticRequest(
                    source = request.bufferText,
                    documentUri = request.documentUri,
                    mappingContext = mappingContext,
                    classIndex = classIndex,
                ),
            )
        }

    fun codeActions(
        request: AwAtFacadeRequest,
        diagnosticCode: String? = null,
    ): List<McFix> {
        val diagnostics = diagnose(request).filter { diagnosticCode == null || it.code == diagnosticCode }
        return when (request.fileType) {
            AwAtFileType.ACCESS_WIDENER -> awCodeActions.fixesForDiagnostics(
                diagnostics = diagnostics,
                documentUri = request.documentUri,
                source = request.bufferText,
                classIndex = classIndex,
                mappingContext = mappingContext,
            )
            AwAtFileType.ACCESS_TRANSFORMER -> atCodeActions.fixesForDiagnostics(
                diagnostics = diagnostics,
                documentUri = request.documentUri,
                source = request.bufferText,
                classIndex = classIndex,
                mappingContext = mappingContext,
            )
        }
    }
}
