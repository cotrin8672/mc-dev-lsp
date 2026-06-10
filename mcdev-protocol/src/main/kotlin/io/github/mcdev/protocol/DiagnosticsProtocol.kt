package io.github.mcdev.protocol

data class McdevDiagnosticsRequest(
    val context: McdevRequestContext,
)

data class McdevDiagnosticsResponse(
    val diagnostics: List<McdevDiagnosticDto>,
)

data class McdevDiagnosticDto(
    val code: String,
    val severity: String,
    val message: String,
    val range: McdevRange,
    val metadata: Map<String, String> = emptyMap(),
)

data class McdevReindexRequest(
    val context: McdevRequestContext,
)

data class McdevReindexResponse(
    val status: String,
    val indexState: String,
    val classpathEntries: Int,
)
