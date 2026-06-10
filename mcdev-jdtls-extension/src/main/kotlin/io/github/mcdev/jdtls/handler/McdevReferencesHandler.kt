package io.github.mcdev.jdtls.handler

import io.github.mcdev.jdtls.convert.DefinitionConverter
import io.github.mcdev.jdtls.mixin.MixinServiceFacade
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.protocol.ProtocolDecodeException
import io.github.mcdev.jdtls.protocol.ProtocolPayloadDecoder
import io.github.mcdev.protocol.McdevError
import io.github.mcdev.protocol.McdevErrorCode
import io.github.mcdev.protocol.McdevProtocol
import io.github.mcdev.protocol.McdevReferencesRequest
import io.github.mcdev.protocol.McdevReferencesResponse
import io.github.mcdev.protocol.McdevResponseEnvelope

class McdevReferencesHandler(
    private val projectService: FileBasedProjectContextService = FileBasedProjectContextService(),
    private val mixinFacade: MixinServiceFacade = MixinServiceFacade(),
    private val decoder: ProtocolPayloadDecoder = ProtocolPayloadDecoder(),
) {
    fun handle(arguments: List<Any?>): McdevResponseEnvelope<McdevReferencesResponse> =
        try {
            val request = decoder.decodeReferencesRequest(arguments)
            handle(request)
        } catch (error: ProtocolDecodeException) {
            errorEnvelope(McdevErrorCode.PARSE_ERROR, error.message ?: "invalid references payload")
        }

    fun handle(request: McdevReferencesRequest): McdevResponseEnvelope<McdevReferencesResponse> {
        if (request.context.protocolVersion != McdevProtocol.VERSION) {
            return typedProtocolMismatch(request.context.protocolVersion)
        }
        if (request.context.workspaceRoot.isBlank()) {
            return incompleteContext("workspace root is required")
        }

        val session = projectService.loadSession(request.context.workspaceRoot)
        val definitionTargets = mixinFacade.definitions(
            session = session,
            source = request.context.bufferText,
            line = request.context.position.line,
            character = request.context.position.character,
        )
        if (definitionTargets.isEmpty()) {
            return McdevResponseEnvelope(
                capabilities = setOf("references"),
                result = McdevReferencesResponse(locations = emptyList()),
            )
        }

        val sources = mixinFacade.collectSourceEntries(
            projectContext = session.context,
            currentDocumentUri = request.context.documentUri,
            currentBufferText = request.context.bufferText,
        )
        val references = definitionTargets.flatMap { target ->
            mixinFacade.references(session, target, sources)
        }
        return McdevResponseEnvelope(
            capabilities = setOf("references"),
            result = McdevReferencesResponse(
                locations = DefinitionConverter.referencesToLocations(references),
            ),
        )
    }

    private fun errorEnvelope(code: McdevErrorCode, message: String): McdevResponseEnvelope<McdevReferencesResponse> =
        McdevResponseEnvelope(error = McdevError(code = code, message = message))

    private fun incompleteContext(message: String): McdevResponseEnvelope<McdevReferencesResponse> =
        errorEnvelope(McdevErrorCode.INCOMPLETE_PROJECT_CONTEXT, message)
}
