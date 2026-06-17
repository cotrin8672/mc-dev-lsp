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
    val debug: McdevCompletionDebugInfo? = null,
)

data class McdevCompletionDebugInfo(
    val command: String,
    val documentUri: String,
    val languageId: String,
    val parseSource: String?,
    val parseConfidence: String?,
    val usedCompilationUnit: Boolean,
    val usedJavaProject: Boolean,
    val bindingResolvedCount: Int,
    val bindingFailedCount: Int,
    val fallbackReason: String?,
    val semanticTargetCount: Int,
    val semanticMemberCount: Int,
    val completionContextKind: String?,
    val owner: String?,
    val methodName: String?,
    val methodDescriptor: String?,
    val candidateCountBeforeFilter: Int,
    val candidateCountAfterFilter: Int,
    val zeroItemReason: String?,
    val warnings: List<String>,
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
