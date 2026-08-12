package io.github.mcdev.core.completion

import io.github.mcdev.core.codeaction.McTextEdit

enum class McCompletionKind {
    CLASS,
    METHOD,
    FIELD,
    KEYWORD,
    VALUE,
}

enum class McCompletionInsertTextFormat {
    PLAIN_TEXT,
    SNIPPET,
}

data class McCompletionMetadata(
    val source: String,
    val owner: String? = null,
    val name: String? = null,
    val descriptor: String? = null,
    val namespace: String? = null,
)

data class McCompletionItem(
    val label: String,
    val detail: String?,
    val documentation: String?,
    val filterText: String,
    val insertText: String,
    val kind: McCompletionKind,
    val sortKey: String,
    val metadata: McCompletionMetadata,
    val additionalEdits: List<McTextEdit> = emptyList(),
    val insertTextFormat: McCompletionInsertTextFormat = McCompletionInsertTextFormat.PLAIN_TEXT,
)
