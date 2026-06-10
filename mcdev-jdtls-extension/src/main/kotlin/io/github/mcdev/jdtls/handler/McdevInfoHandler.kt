package io.github.mcdev.jdtls.handler

import io.github.mcdev.core.project.InfoService
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.protocol.ProtocolDecodeException
import io.github.mcdev.jdtls.protocol.ProtocolPayloadDecoder
import io.github.mcdev.protocol.McdevError
import io.github.mcdev.protocol.McdevErrorCode
import io.github.mcdev.protocol.McdevInfoRequest
import io.github.mcdev.protocol.McdevInfoResponse
import io.github.mcdev.protocol.McdevProtocol
import io.github.mcdev.protocol.McdevResponseEnvelope
import io.github.mcdev.protocol.McdevRequestContext

class McdevInfoHandler(
    private val projectService: FileBasedProjectContextService = FileBasedProjectContextService(),
    private val decoder: ProtocolPayloadDecoder = ProtocolPayloadDecoder(),
) {
    fun handle(arguments: List<Any?>): McdevResponseEnvelope<McdevInfoResponse> =
        try {
            val request = decoder.decodeInfoRequest(arguments)
            handle(request)
        } catch (error: ProtocolDecodeException) {
            errorEnvelope(McdevErrorCode.PARSE_ERROR, error.message ?: "invalid info payload")
        }

    fun handle(request: McdevInfoRequest): McdevResponseEnvelope<McdevInfoResponse> =
        handle(request.context)

    fun handle(context: McdevRequestContext): McdevResponseEnvelope<McdevInfoResponse> {
        if (context.protocolVersion != McdevProtocol.VERSION) {
            return typedProtocolMismatch(context.protocolVersion)
        }
        if (context.workspaceRoot.isBlank()) {
            return incompleteContext("workspace root is required")
        }

        val session = projectService.loadSession(context.workspaceRoot)
        return McdevResponseEnvelope(
            capabilities = setOf("info", "completion", "diagnostics"),
            result = McdevInfoResponse(
                lines = InfoService.formatLines(
                    context = session.context,
                    protocolVersion = McdevProtocol.VERSION,
                    extensionVersion = McdevProtocol.SERVER_VERSION,
                ),
            ),
        )
    }

    private fun errorEnvelope(code: McdevErrorCode, message: String): McdevResponseEnvelope<McdevInfoResponse> =
        McdevResponseEnvelope(error = McdevError(code = code, message = message))

    private fun incompleteContext(message: String): McdevResponseEnvelope<McdevInfoResponse> =
        errorEnvelope(McdevErrorCode.INCOMPLETE_PROJECT_CONTEXT, message)
}
