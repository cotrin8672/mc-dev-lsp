package io.github.mcdev.protocol

object McdevProtocol {
    const val VERSION: Int = 1
    const val SERVER_VERSION: String = "0.1.0"
}

object McdevCommands {
    const val COMPLETION: String = "mcdev.completion"
    const val DEFINITION: String = "mcdev.definition"
    const val REFERENCES: String = "mcdev.references"
    const val CODE_ACTION: String = "mcdev.codeAction"
    const val REINDEX: String = "mcdev.reindex"
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
