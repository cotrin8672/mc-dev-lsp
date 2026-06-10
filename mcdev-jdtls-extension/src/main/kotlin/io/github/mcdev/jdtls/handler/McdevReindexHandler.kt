package io.github.mcdev.jdtls.handler

import io.github.mcdev.core.project.ProjectIndexState
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.protocol.ProtocolDecodeException
import io.github.mcdev.jdtls.protocol.ProtocolPayloadDecoder
import io.github.mcdev.protocol.McdevError
import io.github.mcdev.protocol.McdevErrorCode
import io.github.mcdev.protocol.McdevProtocol
import io.github.mcdev.protocol.McdevReindexRequest
import io.github.mcdev.protocol.McdevReindexResponse
import io.github.mcdev.protocol.McdevResponseEnvelope
import io.github.mcdev.protocol.McdevRequestContext

class McdevReindexHandler(
    private val projectService: FileBasedProjectContextService = FileBasedProjectContextService(),
    private val decoder: ProtocolPayloadDecoder = ProtocolPayloadDecoder(),
) {
    fun handle(arguments: List<Any?>): McdevResponseEnvelope<McdevReindexResponse> =
        try {
            val request = decoder.decodeReindexRequest(arguments)
            handle(request)
        } catch (error: ProtocolDecodeException) {
            errorEnvelope(McdevErrorCode.PARSE_ERROR, error.message ?: "invalid reindex payload")
        }

    fun handle(request: McdevReindexRequest): McdevResponseEnvelope<McdevReindexResponse> =
        handle(request.context)

    fun handle(context: McdevRequestContext): McdevResponseEnvelope<McdevReindexResponse> {
        if (context.protocolVersion != McdevProtocol.VERSION) {
            return typedProtocolMismatch(context.protocolVersion)
        }
        if (context.workspaceRoot.isBlank()) {
            return incompleteContext("workspace root is required")
        }

        val session = projectService.reindex(context.workspaceRoot)
        return McdevResponseEnvelope(
            capabilities = setOf("reindex"),
            result = McdevReindexResponse(
                status = "reindex complete",
                indexState = session.context.indexState.name.lowercase(),
                classpathEntries = session.context.classpath.entryCount,
            ),
        )
    }

    private fun errorEnvelope(code: McdevErrorCode, message: String): McdevResponseEnvelope<McdevReindexResponse> =
        McdevResponseEnvelope(error = McdevError(code = code, message = message))

    private fun incompleteContext(message: String): McdevResponseEnvelope<McdevReindexResponse> =
        errorEnvelope(McdevErrorCode.INCOMPLETE_PROJECT_CONTEXT, message)
}
