package io.github.mcdev.jdtls.handler

import io.github.mcdev.core.model.MappingNamespace
import io.github.mcdev.core.project.InfoService
import io.github.mcdev.core.project.ProjectContext
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.protocol.ProtocolDecodeException
import io.github.mcdev.jdtls.protocol.ProtocolPayloadDecoder
import io.github.mcdev.protocol.McdevClasspathDto
import io.github.mcdev.protocol.McdevDumpContextRequest
import io.github.mcdev.protocol.McdevDumpContextResponse
import io.github.mcdev.protocol.McdevError
import io.github.mcdev.protocol.McdevErrorCode
import io.github.mcdev.protocol.McdevMappingContextDto
import io.github.mcdev.protocol.McdevProtocol
import io.github.mcdev.protocol.McdevReloadProjectContextRequest
import io.github.mcdev.protocol.McdevReloadProjectContextResponse
import io.github.mcdev.protocol.McdevRequestContext
import io.github.mcdev.protocol.McdevResponseEnvelope
import io.github.mcdev.protocol.McdevSourceSetDto
import java.nio.file.Path

class McdevProjectContextHandler(
    private val projectService: FileBasedProjectContextService = FileBasedProjectContextService(),
    private val decoder: ProtocolPayloadDecoder = ProtocolPayloadDecoder(),
) {
    fun reload(arguments: List<Any?>): McdevResponseEnvelope<McdevReloadProjectContextResponse> =
        try {
            val request = decoder.decodeReloadProjectContextRequest(arguments)
            reload(request)
        } catch (error: ProtocolDecodeException) {
            reloadError(McdevErrorCode.PARSE_ERROR, error.message ?: "invalid reload project context payload")
        }

    fun reload(request: McdevReloadProjectContextRequest): McdevResponseEnvelope<McdevReloadProjectContextResponse> =
        reload(request.context)

    fun reload(context: McdevRequestContext): McdevResponseEnvelope<McdevReloadProjectContextResponse> {
        if (context.protocolVersion != McdevProtocol.VERSION) {
            return typedProtocolMismatch(context.protocolVersion)
        }
        if (context.workspaceRoot.isBlank()) {
            return reloadError(McdevErrorCode.INCOMPLETE_PROJECT_CONTEXT, "workspace root is required")
        }

        projectService.clearCache()
        val session = projectService.reindex(context.workspaceRoot)
        val lines = infoLines(session.context)
        return McdevResponseEnvelope(
            capabilities = setOf("reloadProjectContext", "dumpContext", "info", "reindex"),
            result = McdevReloadProjectContextResponse(
                status = "project context reloaded",
                indexState = session.context.indexState.name.lowercase(),
                classpathEntries = session.context.classpath.entryCount,
                lines = lines,
            ),
        )
    }

    fun dump(arguments: List<Any?>): McdevResponseEnvelope<McdevDumpContextResponse> =
        try {
            val request = decoder.decodeDumpContextRequest(arguments)
            dump(request)
        } catch (error: ProtocolDecodeException) {
            dumpError(McdevErrorCode.PARSE_ERROR, error.message ?: "invalid dump context payload")
        }

    fun dump(request: McdevDumpContextRequest): McdevResponseEnvelope<McdevDumpContextResponse> =
        dump(request.context)

    fun dump(context: McdevRequestContext): McdevResponseEnvelope<McdevDumpContextResponse> {
        if (context.protocolVersion != McdevProtocol.VERSION) {
            return typedProtocolMismatch(context.protocolVersion)
        }
        if (context.workspaceRoot.isBlank()) {
            return dumpError(McdevErrorCode.INCOMPLETE_PROJECT_CONTEXT, "workspace root is required")
        }

        val session = projectService.loadSession(context.workspaceRoot)
        return McdevResponseEnvelope(
            capabilities = setOf("dumpContext", "info"),
            result = session.context.toDumpResponse(infoLines(session.context)),
        )
    }

    private fun infoLines(context: ProjectContext): List<String> =
        InfoService.formatLines(
            context = context,
            protocolVersion = McdevProtocol.VERSION,
            extensionVersion = McdevProtocol.SERVER_VERSION,
        )

    private fun ProjectContext.toDumpResponse(lines: List<String>): McdevDumpContextResponse =
        McdevDumpContextResponse(
            lines = lines,
            projectId = projectId,
            root = root.toString(),
            platform = platform.name.lowercase(),
            mappings = McdevMappingContextDto(
                sourceNamespace = mappings.sourceNamespace.protocolName(),
                runtimeNamespace = mappings.runtimeNamespace.protocolName(),
                awNamespace = mappings.awNamespace?.protocolName(),
                atNamespace = mappings.atNamespace?.protocolName(),
                availableNamespaces = mappings.availableNamespaces.map { it.protocolName() }.sorted(),
            ),
            classpath = McdevClasspathDto(
                entryCount = classpath.entryCount,
                projectOutputs = classpath.projectOutputs.map(Path::toString),
                dependencyJars = classpath.dependencyJars.map(Path::toString),
                minecraftJars = classpath.minecraftJars.map(Path::toString),
                generatedOutputs = classpath.generatedOutputs.map(Path::toString),
                capturedAt = classpath.capturedAt,
            ),
            sourceSets = sourceSets.map { sourceSet ->
                McdevSourceSetDto(
                    name = sourceSet.name,
                    sourceDirectories = sourceSet.sourceDirectories.map(Path::toString),
                    resourceDirectories = sourceSet.resourceDirectories.map(Path::toString),
                    outputDirectory = sourceSet.outputDirectory?.toString(),
                )
            },
            mixinConfigs = mixinConfigs.map { it.path.toString() },
            accessWideners = accessWideners.map { it.path.toString() },
            accessTransformers = accessTransformers.map { it.path.toString() },
            minecraftJars = minecraftJars.map(Path::toString),
            indexState = indexState.name.lowercase(),
        )

    private fun MappingNamespace.protocolName(): String = name.lowercase()

    private fun reloadError(
        code: McdevErrorCode,
        message: String,
    ): McdevResponseEnvelope<McdevReloadProjectContextResponse> =
        McdevResponseEnvelope(error = McdevError(code = code, message = message))

    private fun dumpError(code: McdevErrorCode, message: String): McdevResponseEnvelope<McdevDumpContextResponse> =
        McdevResponseEnvelope(error = McdevError(code = code, message = message))
}
