package io.github.mcdev.jdtls.handler

import io.github.mcdev.jdtls.convert.DiagnosticConverter
import io.github.mcdev.jdtls.mixin.MixinServiceFacade
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.protocol.ProtocolDecodeException
import io.github.mcdev.jdtls.protocol.ProtocolPayloadDecoder
import io.github.mcdev.protocol.McdevDiagnosticsRequest
import io.github.mcdev.protocol.McdevDiagnosticsResponse
import io.github.mcdev.protocol.McdevError
import io.github.mcdev.protocol.McdevErrorCode
import io.github.mcdev.protocol.McdevProtocol
import io.github.mcdev.protocol.McdevResponseEnvelope
import io.github.mcdev.protocol.McdevRequestContext

class McdevDiagnosticsHandler(
    private val projectService: FileBasedProjectContextService = FileBasedProjectContextService(),
    private val mixinFacade: MixinServiceFacade = MixinServiceFacade(),
    private val decoder: ProtocolPayloadDecoder = ProtocolPayloadDecoder(),
) {
    fun handle(arguments: List<Any?>): McdevResponseEnvelope<McdevDiagnosticsResponse> =
        try {
            val request = decoder.decodeDiagnosticsRequest(arguments)
            handle(request)
        } catch (error: ProtocolDecodeException) {
            errorEnvelope(McdevErrorCode.PARSE_ERROR, error.message ?: "invalid diagnostics payload")
        }

    fun handle(request: McdevDiagnosticsRequest): McdevResponseEnvelope<McdevDiagnosticsResponse> =
        handle(request.context)

    fun handle(context: McdevRequestContext): McdevResponseEnvelope<McdevDiagnosticsResponse> {
        if (context.protocolVersion != McdevProtocol.VERSION) {
            return typedProtocolMismatch(context.protocolVersion)
        }
        if (context.workspaceRoot.isBlank()) {
            return incompleteContext("workspace root is required")
        }

        val session = projectService.loadSession(context.workspaceRoot)
        val diagnostics = mixinFacade.analyzeDiagnostics(
            session = session,
            projectContext = session.context,
            source = context.bufferText,
            documentUri = context.documentUri,
        )
        return McdevResponseEnvelope(
            capabilities = setOf("diagnostics"),
            result = McdevDiagnosticsResponse(
                diagnostics = DiagnosticConverter.toDtos(diagnostics),
            ),
        )
    }

    private fun errorEnvelope(code: McdevErrorCode, message: String): McdevResponseEnvelope<McdevDiagnosticsResponse> =
        McdevResponseEnvelope(error = McdevError(code = code, message = message))

    private fun incompleteContext(message: String): McdevResponseEnvelope<McdevDiagnosticsResponse> =
        errorEnvelope(McdevErrorCode.INCOMPLETE_PROJECT_CONTEXT, message)
}
