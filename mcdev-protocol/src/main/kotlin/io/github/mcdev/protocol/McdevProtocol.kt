package io.github.mcdev.protocol

object McdevProtocol {
    const val VERSION: Int = 1
    const val SERVER_VERSION: String = "0.1.0"
}

object McdevCommands {
    const val COMPLETION: String = "mcdev.completion"
    const val DEFINITION: String = "mcdev.definition"
    const val REFERENCES: String = "mcdev.references"
    const val HOVER: String = "mcdev.hover"
    const val CODE_ACTION: String = "mcdev.codeAction"
    const val REINDEX: String = "mcdev.reindex"
    const val RELOAD_PROJECT_CONTEXT: String = "mcdev.reloadProjectContext"
    const val DUMP_CONTEXT: String = "mcdev.dumpContext"
    const val CONTEXT: String = "mcdev.context"
    const val INFO: String = "mcdev.info"
}

data class McdevClientInfo(
    val name: String,
    val version: String,
)

data class McdevPosition(
    val line: Int,
    val character: Int,
)

data class McdevRange(
    val start: McdevPosition,
    val end: McdevPosition,
)

data class McdevRequestContext(
    val protocolVersion: Int,
    val workspaceRoot: String,
    val documentUri: String,
    val languageId: String,
    val position: McdevPosition,
    val bufferText: String,
    val client: McdevClientInfo,
)

data class McdevResponseEnvelope<T>(
    val protocolVersion: Int = McdevProtocol.VERSION,
    val serverVersion: String = McdevProtocol.SERVER_VERSION,
    val capabilities: Set<String> = emptySet(),
    val result: T? = null,
    val error: McdevError? = null,
)

data class McdevReloadProjectContextRequest(
    val context: McdevRequestContext,
)

data class McdevReloadProjectContextResponse(
    val status: String,
    val indexState: String,
    val classpathEntries: Int,
    val lines: List<String>,
)

data class McdevDumpContextRequest(
    val context: McdevRequestContext,
)

data class McdevDumpContextResponse(
    val lines: List<String>,
    val projectId: String,
    val root: String,
    val platform: String,
    val mappings: McdevMappingContextDto,
    val classpath: McdevClasspathDto,
    val sourceSets: List<McdevSourceSetDto>,
    val mixinConfigs: List<String>,
    val accessWideners: List<String>,
    val accessTransformers: List<String>,
    val minecraftJars: List<String>,
    val indexState: String,
)

data class McdevMappingContextDto(
    val sourceNamespace: String,
    val runtimeNamespace: String,
    val awNamespace: String?,
    val atNamespace: String?,
    val availableNamespaces: List<String>,
)

data class McdevClasspathDto(
    val entryCount: Int,
    val projectOutputs: List<String>,
    val dependencyJars: List<String>,
    val minecraftJars: List<String>,
    val generatedOutputs: List<String>,
    val capturedAt: Long,
)

data class McdevSourceSetDto(
    val name: String,
    val sourceDirectories: List<String>,
    val resourceDirectories: List<String>,
    val outputDirectory: String?,
)

data class McdevError(
    val code: McdevErrorCode,
    val message: String,
    val details: Map<String, String> = emptyMap(),
)

enum class McdevErrorCode {
    PROTOCOL_MISMATCH,
    NO_APPLICABLE_CONTEXT,
    INCOMPLETE_PROJECT_CONTEXT,
    MISSING_MAPPING_DATA,
    MISSING_CLASSPATH_ENTRY,
    BYTECODE_UNAVAILABLE,
    PARSE_ERROR,
    INTERNAL_ERROR,
}

fun protocolMismatch(clientVersion: Int): McdevResponseEnvelope<Nothing> =
    McdevResponseEnvelope(
        error = McdevError(
            code = McdevErrorCode.PROTOCOL_MISMATCH,
            message = "protocol mismatch, client=$clientVersion server=${McdevProtocol.VERSION}",
            details = mapOf("client" to clientVersion.toString(), "server" to McdevProtocol.VERSION.toString()),
        ),
    )
