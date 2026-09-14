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

/**
 * Source offsets that should be replaced when a completion item is applied.
 *
 * The range is exclusive at [endOffset], matching the text-edit model used by
 * the protocol adapters.
 */
data class McCompletionReplacementRange(
    val startOffset: Int,
    val endOffset: Int,
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
