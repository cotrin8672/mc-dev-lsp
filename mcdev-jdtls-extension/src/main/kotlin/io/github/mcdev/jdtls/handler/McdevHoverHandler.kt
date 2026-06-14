package io.github.mcdev.jdtls.handler

import io.github.mcdev.core.definition.McDefinitionTarget
import io.github.mcdev.core.model.MemberKind
import io.github.mcdev.jdtls.awat.AwAtServiceFacade
import io.github.mcdev.jdtls.mixin.MixinServiceFacade
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.protocol.ProtocolDecodeException
import io.github.mcdev.jdtls.protocol.ProtocolPayloadDecoder
import io.github.mcdev.protocol.McdevError
import io.github.mcdev.protocol.McdevErrorCode
import io.github.mcdev.protocol.McdevHoverRequest
import io.github.mcdev.protocol.McdevHoverResponse
import io.github.mcdev.protocol.McdevProtocol
import io.github.mcdev.protocol.McdevResponseEnvelope

class McdevHoverHandler(
    private val projectService: FileBasedProjectContextService = FileBasedProjectContextService(),
    private val mixinFacade: MixinServiceFacade = MixinServiceFacade(),
    private val awAtFacade: AwAtServiceFacade = AwAtServiceFacade(),
    private val decoder: ProtocolPayloadDecoder = ProtocolPayloadDecoder(),
) {
    fun handle(arguments: List<Any?>): McdevResponseEnvelope<McdevHoverResponse> =
        try {
            val request = decoder.decodeHoverRequest(arguments)
            handle(request)
        } catch (error: ProtocolDecodeException) {
            errorEnvelope(McdevErrorCode.PARSE_ERROR, error.message ?: "invalid hover payload")
        }

    fun handle(request: McdevHoverRequest): McdevResponseEnvelope<McdevHoverResponse> {
        if (request.context.protocolVersion != McdevProtocol.VERSION) {
            return typedProtocolMismatch(request.context.protocolVersion)
        }
        if (request.context.workspaceRoot.isBlank()) {
            return errorEnvelope(McdevErrorCode.INCOMPLETE_PROJECT_CONTEXT, "workspace root is required")
        }

        val session = projectService.loadSession(request.context.workspaceRoot)
        val awAtFileType = awAtFacade.detectFileType(
            request.context.languageId,
            request.context.documentUri,
        )
        val targets = if (awAtFileType != null) {
            awAtFacade.definitions(
                session = session,
                source = request.context.bufferText,
                line = request.context.position.line,
                character = request.context.position.character,
                fileType = awAtFileType,
                documentUri = request.context.documentUri,
            )
        } else {
            mixinFacade.definitions(
                session = session,
                source = request.context.bufferText,
                line = request.context.position.line,
                character = request.context.position.character,
            )
        }
        val contents = targets.distinctBy { "${it.kind}:${it.ownerInternalName}:${it.name}:${it.descriptor}" }
            .map(::formatTarget)
        return McdevResponseEnvelope(
            capabilities = setOf("hover"),
            result = McdevHoverResponse(
                contents = contents,
                range = targets.firstOrNull()?.sourceRange?.let {
                    io.github.mcdev.protocol.McdevRange(
                        start = io.github.mcdev.protocol.McdevPosition(it.start.line, it.start.character),
                        end = io.github.mcdev.protocol.McdevPosition(it.end.line, it.end.character),
                    )
                },
            ),
        )
    }

    private fun formatTarget(target: McDefinitionTarget): String {
        val symbol = when (target.kind) {
            MemberKind.CLASS -> target.ownerFqn ?: target.ownerInternalName.replace('/', '.')
            MemberKind.METHOD -> "${target.ownerFqn ?: target.ownerInternalName.replace('/', '.')}.${target.name}${target.descriptor.orEmpty()}"
            MemberKind.FIELD -> "${target.ownerFqn ?: target.ownerInternalName.replace('/', '.')}.${target.name}:${target.descriptor.orEmpty()}"
            MemberKind.CONSTRUCTOR -> "${target.ownerFqn ?: target.ownerInternalName.replace('/', '.')}.<init>${target.descriptor.orEmpty()}"
        }
        return buildString {
            appendLine("```mcdev")
            appendLine("${target.kind.name.lowercase()} $symbol")
            appendLine("owner: ${target.ownerInternalName}")
            target.descriptor?.let { appendLine("descriptor: $it") }
            appendLine("namespace: ${target.namespace.name.lowercase()}")
            append("```")
        }
    }

    private fun errorEnvelope(code: McdevErrorCode, message: String): McdevResponseEnvelope<McdevHoverResponse> =
        McdevResponseEnvelope(error = McdevError(code = code, message = message))
}
