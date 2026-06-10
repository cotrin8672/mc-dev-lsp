package io.github.mcdev.protocol

data class McdevCodeActionRequest(
    val context: McdevRequestContext,
    val range: McdevRange,
    val diagnosticCodes: List<String>,
)

data class McdevCodeActionResponse(
    val actions: List<McdevCodeActionDto>,
)

data class McdevCodeActionDto(
    val title: String,
    val kind: String,
    val edits: List<McdevWorkspaceEdit>,
    val metadata: Map<String, String> = emptyMap(),
)

data class McdevWorkspaceEdit(
    val documentUri: String,
    val edits: List<McdevTextEdit>,
)

data class McdevDefinitionRequest(
    val context: McdevRequestContext,
)

data class McdevDefinitionResponse(
    val locations: List<McdevLocation>,
)

data class McdevReferencesRequest(
    val context: McdevRequestContext,
)

data class McdevReferencesResponse(
    val locations: List<McdevLocation>,
)

data class McdevLocation(
    val documentUri: String,
    val range: McdevRange,
    val metadata: Map<String, String> = emptyMap(),
)

data class McdevInfoRequest(
    val context: McdevRequestContext,
)

data class McdevInfoResponse(
    val lines: List<String>,
)
