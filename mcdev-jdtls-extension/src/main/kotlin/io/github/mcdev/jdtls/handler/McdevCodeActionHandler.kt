package io.github.mcdev.jdtls.handler

import io.github.mcdev.core.at.AccessTransformerCodeActionService
import io.github.mcdev.core.aw.AccessWidenerCodeActionService
import io.github.mcdev.jdtls.awat.AwAtServiceFacade
import io.github.mcdev.jdtls.convert.CodeActionConverter
import io.github.mcdev.jdtls.mixin.MixinServiceFacade
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.protocol.ProtocolDecodeException
import io.github.mcdev.jdtls.protocol.ProtocolPayloadDecoder
import io.github.mcdev.protocol.McdevCodeActionRequest
import io.github.mcdev.protocol.McdevCodeActionResponse
import io.github.mcdev.protocol.McdevError
import io.github.mcdev.protocol.McdevErrorCode
import io.github.mcdev.protocol.McdevProtocol
import io.github.mcdev.protocol.McdevResponseEnvelope

class McdevCodeActionHandler(
    private val projectService: FileBasedProjectContextService = FileBasedProjectContextService(),
    private val mixinFacade: MixinServiceFacade = MixinServiceFacade(),
    private val awAtFacade: AwAtServiceFacade = AwAtServiceFacade(),
    private val decoder: ProtocolPayloadDecoder = ProtocolPayloadDecoder(),
) {
    fun handle(arguments: List<Any?>): McdevResponseEnvelope<McdevCodeActionResponse> =
        try {
            val request = decoder.decodeCodeActionRequest(arguments)
            handle(request)
        } catch (error: ProtocolDecodeException) {
            errorEnvelope(McdevErrorCode.PARSE_ERROR, error.message ?: "invalid code action payload")
        }

    fun handle(request: McdevCodeActionRequest): McdevResponseEnvelope<McdevCodeActionResponse> {
        if (request.context.protocolVersion != McdevProtocol.VERSION) {
            return typedProtocolMismatch(request.context.protocolVersion)
        }
        if (request.context.workspaceRoot.isBlank()) {
            return incompleteContext("workspace root is required")
        }

        val session = projectService.loadSession(request.context.workspaceRoot)
        val mixinConfigContent = mixinFacade.selectedMixinConfigContent(
            projectContext = session.context,
            source = request.context.bufferText,
        )
        val awAtFileType = awAtFacade.detectFileType(request.context.languageId, request.context.documentUri)
        val filteredFixes = if (awAtFileType != null) {
            if (request.diagnosticCodes.isEmpty()) {
                awAtFacade.codeActions(
                    session = session,
                    source = request.context.bufferText,
                    documentUri = request.context.documentUri,
                    fileType = awAtFileType,
                    diagnosticCode = null,
                )
            } else {
                request.diagnosticCodes.flatMap { code ->
                    awAtFacade.codeActions(
                        session = session,
                        source = request.context.bufferText,
                        documentUri = request.context.documentUri,
                        fileType = awAtFileType,
                        diagnosticCode = code,
                    )
                }.distinctBy { "${it.kind}:${it.title}" }
            }
        } else if (request.diagnosticCodes.isEmpty()) {
            mixinFacade.codeActions(
                session = session,
                projectContext = session.context,
                source = request.context.bufferText,
                documentUri = request.context.documentUri,
                diagnosticCode = null,
            )
        } else {
            request.diagnosticCodes.flatMap { code ->
                mixinFacade.codeActions(
                    session = session,
                    projectContext = session.context,
                    source = request.context.bufferText,
                    documentUri = request.context.documentUri,
                    diagnosticCode = code,
                )
            }.distinctBy { "${it.kind}:${it.title}" }
        }

        return McdevResponseEnvelope(
            capabilities = setOf("codeAction"),
            result = McdevCodeActionResponse(
                actions = CodeActionConverter.toDtos(
                    fixes = filteredFixes,
                    source = request.context.bufferText,
                    mixinConfigContent = mixinConfigContent,
                    awCodeActionService = AccessWidenerCodeActionService(),
                    atCodeActionService = AccessTransformerCodeActionService(
                        classIndex = session.classIndex,
                        mappingContext = session.context.mappings,
                    ),
                ),
            ),
        )
    }

    private fun errorEnvelope(code: McdevErrorCode, message: String): McdevResponseEnvelope<McdevCodeActionResponse> =
        McdevResponseEnvelope(error = McdevError(code = code, message = message))

    private fun incompleteContext(message: String): McdevResponseEnvelope<McdevCodeActionResponse> =
        errorEnvelope(McdevErrorCode.INCOMPLETE_PROJECT_CONTEXT, message)
}
