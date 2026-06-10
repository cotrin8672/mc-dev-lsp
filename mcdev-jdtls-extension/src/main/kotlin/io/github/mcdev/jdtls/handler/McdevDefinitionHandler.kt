package io.github.mcdev.jdtls.handler

import io.github.mcdev.jdtls.convert.DefinitionConverter
import io.github.mcdev.jdtls.mixin.MixinServiceFacade
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.protocol.ProtocolDecodeException
import io.github.mcdev.jdtls.protocol.ProtocolPayloadDecoder
import io.github.mcdev.protocol.McdevDefinitionRequest
import io.github.mcdev.protocol.McdevDefinitionResponse
import io.github.mcdev.protocol.McdevError
import io.github.mcdev.protocol.McdevErrorCode
import io.github.mcdev.protocol.McdevProtocol
import io.github.mcdev.protocol.McdevResponseEnvelope

class McdevDefinitionHandler(
    private val projectService: FileBasedProjectContextService = FileBasedProjectContextService(),
    private val mixinFacade: MixinServiceFacade = MixinServiceFacade(),
    private val decoder: ProtocolPayloadDecoder = ProtocolPayloadDecoder(),
) {
    fun handle(arguments: List<Any?>): McdevResponseEnvelope<McdevDefinitionResponse> =
        try {
            val request = decoder.decodeDefinitionRequest(arguments)
            handle(request)
        } catch (error: ProtocolDecodeException) {
            errorEnvelope(McdevErrorCode.PARSE_ERROR, error.message ?: "invalid definition payload")
        }

    fun handle(request: McdevDefinitionRequest): McdevResponseEnvelope<McdevDefinitionResponse> {
        if (request.context.protocolVersion != McdevProtocol.VERSION) {
            return typedProtocolMismatch(request.context.protocolVersion)
        }
        if (request.context.workspaceRoot.isBlank()) {
            return incompleteContext("workspace root is required")
        }

        val session = projectService.loadSession(request.context.workspaceRoot)
        val targets = mixinFacade.definitions(
            session = session,
            source = request.context.bufferText,
            line = request.context.position.line,
            character = request.context.position.character,
        )
        return McdevResponseEnvelope(
            capabilities = setOf("definition"),
            result = McdevDefinitionResponse(
                locations = DefinitionConverter.toLocations(targets),
            ),
        )
    }

    private fun errorEnvelope(code: McdevErrorCode, message: String): McdevResponseEnvelope<McdevDefinitionResponse> =
        McdevResponseEnvelope(error = McdevError(code = code, message = message))

    private fun incompleteContext(message: String): McdevResponseEnvelope<McdevDefinitionResponse> =
        errorEnvelope(McdevErrorCode.INCOMPLETE_PROJECT_CONTEXT, message)
}
