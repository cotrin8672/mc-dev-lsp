package io.github.mcdev.protocol

data class McdevCompletionRequest(
    val context: McdevRequestContext,
    val trigger: McdevCompletionTrigger,
    val options: McdevCompletionOptions,
)

data class McdevCompletionTrigger(
    val kind: String,
    val character: String?,
)

data class McdevCompletionOptions(
    val preferredAtTarget: String = "smart",
    val mixinClassInsert: String = "import",
    val injectMethodDescriptor: String = "auto",
)

data class McdevCompletionResponse(
    val items: List<McdevCompletionItemDto>,
    val warnings: List<McdevWarning> = emptyList(),
)

data class McdevCompletionItemDto(
    val label: String,
    val detail: String?,
    val documentation: String?,
    val filterText: String,
    val insertText: String,
    val kind: String,
    val sortKey: String,
    val edit: McdevTextEdit?,
    val additionalEdits: List<McdevTextEdit> = emptyList(),
    val metadata: Map<String, String?> = emptyMap(),
)

data class McdevTextEdit(
    val range: McdevRange,
    val newText: String,
)

data class McdevWarning(
    val code: String,
    val message: String,
)
