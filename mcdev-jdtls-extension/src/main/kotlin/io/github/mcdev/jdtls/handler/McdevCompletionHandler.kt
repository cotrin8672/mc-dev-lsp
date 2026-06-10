package io.github.mcdev.jdtls.handler

import io.github.mcdev.jdtls.awat.AwAtServiceFacade
import io.github.mcdev.jdtls.convert.CompletionConvertContext
import io.github.mcdev.jdtls.convert.CompletionItemConverter
import io.github.mcdev.jdtls.mixin.MixinServiceFacade
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.protocol.ProtocolDecodeException
import io.github.mcdev.jdtls.protocol.ProtocolPayloadDecoder
import io.github.mcdev.protocol.McdevCompletionRequest
import io.github.mcdev.protocol.McdevCompletionResponse
import io.github.mcdev.protocol.McdevError
import io.github.mcdev.protocol.McdevErrorCode
import io.github.mcdev.protocol.McdevProtocol
import io.github.mcdev.protocol.McdevResponseEnvelope

class McdevCompletionHandler(
    private val projectService: FileBasedProjectContextService = FileBasedProjectContextService(),
    private val mixinFacade: MixinServiceFacade = MixinServiceFacade(),
    private val awAtFacade: AwAtServiceFacade = AwAtServiceFacade(),
    private val decoder: ProtocolPayloadDecoder = ProtocolPayloadDecoder(),
) {
    fun handle(arguments: List<Any?>): McdevResponseEnvelope<McdevCompletionResponse> =
        try {
            val request = decoder.decodeCompletionRequest(arguments)
            handle(request)
        } catch (error: ProtocolDecodeException) {
            errorEnvelope(McdevErrorCode.PARSE_ERROR, error.message ?: "invalid completion payload")
        }

    fun handle(request: McdevCompletionRequest): McdevResponseEnvelope<McdevCompletionResponse> {
        if (request.context.protocolVersion != McdevProtocol.VERSION) {
            return typedProtocolMismatch(request.context.protocolVersion)
        }
        if (request.context.workspaceRoot.isBlank()) {
            return incompleteContext("workspace root is required")
        }

        val session = projectService.loadSession(request.context.workspaceRoot)
        val awAtFileType = awAtFacade.detectFileType(
            languageId = request.context.languageId,
            documentUri = request.context.documentUri,
        )
        if (awAtFileType != null) {
            val items = awAtFacade.complete(
                session = session,
                source = request.context.bufferText,
                line = request.context.position.line,
                character = request.context.position.character,
                fileType = awAtFileType,
                documentUri = request.context.documentUri,
            )
            return McdevResponseEnvelope(
                capabilities = setOf("completion"),
                result = McdevCompletionResponse(
                    items = CompletionItemConverter.toDtos(
                        items = items,
                        annotationContext = null,
                        source = request.context.bufferText,
                        convertContext = CompletionConvertContext(
                            source = request.context.bufferText,
                            annotationContext = null,
                            mappingResolver = session.context.mappings.resolver,
                            sourceNamespace = session.context.mappings.sourceNamespace,
                            runtimeNamespace = session.context.mappings.runtimeNamespace,
                        ),
                    ),
                ),
            )
        }

        val options = mixinFacade.toCompletionOptions(
            mixinClassInsert = request.options.mixinClassInsert,
            injectMethodDescriptor = request.options.injectMethodDescriptor,
            preferredAtTarget = request.options.preferredAtTarget,
        )
        val annotationContext = CompletionItemConverter.extractAnnotationContext(
            source = request.context.bufferText,
            line = request.context.position.line,
            character = request.context.position.character,
        )
        val items = mixinFacade.complete(
            session = session,
            source = request.context.bufferText,
            line = request.context.position.line,
            character = request.context.position.character,
            options = options,
        )
        return McdevResponseEnvelope(
            capabilities = setOf("completion"),
            result = McdevCompletionResponse(
                items = CompletionItemConverter.toDtos(
                    items = items,
                    annotationContext = annotationContext,
                    source = request.context.bufferText,
                    convertContext = CompletionConvertContext(
                        source = request.context.bufferText,
                        annotationContext = annotationContext,
                        classInsertMode = options.classInsertMode,
                        preferredAtTarget = options.preferredAtTarget,
                        mappingResolver = session.context.mappings.resolver,
                        sourceNamespace = session.context.mappings.sourceNamespace,
                        runtimeNamespace = session.context.mappings.runtimeNamespace,
                    ),
                ),
            ),
        )
    }

    private fun errorEnvelope(code: McdevErrorCode, message: String): McdevResponseEnvelope<McdevCompletionResponse> =
        McdevResponseEnvelope(error = McdevError(code = code, message = message))

    private fun incompleteContext(message: String): McdevResponseEnvelope<McdevCompletionResponse> =
        errorEnvelope(McdevErrorCode.INCOMPLETE_PROJECT_CONTEXT, message)
}
