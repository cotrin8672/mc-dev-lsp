package io.github.mcdev.jdtls.convert

import io.github.mcdev.core.codeaction.McTextEdit
import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.mapping.MappingResolver
import io.github.mcdev.core.mixin.AnnotationContext
import io.github.mcdev.core.mixin.AnnotationContextExtractor
import io.github.mcdev.core.mixin.AnnotationSlot
import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.AtTargetInsertFormatter
import io.github.mcdev.core.mixin.AtTargetKind
import io.github.mcdev.core.mixin.MixinAnnotation
import io.github.mcdev.core.mixin.MixinClassInsertMode
import io.github.mcdev.core.mixin.MixinImportEditBuilder
import io.github.mcdev.core.model.MappingNamespace
import io.github.mcdev.protocol.McdevCompletionItemDto
import io.github.mcdev.protocol.McdevPosition
import io.github.mcdev.protocol.McdevRange
import io.github.mcdev.protocol.McdevTextEdit

data class CompletionConvertContext(
    val source: String,
    val annotationContext: AnnotationContext?,
    val classInsertMode: MixinClassInsertMode = MixinClassInsertMode.IMPORT,
    val preferredAtTarget: String = "descriptor",
    val mappingResolver: MappingResolver? = null,
    val sourceNamespace: MappingNamespace = MappingNamespace.NAMED,
    val runtimeNamespace: MappingNamespace = MappingNamespace.INTERMEDIARY,
)

object CompletionItemConverter {
    private val atTargetInsertFormatter = AtTargetInsertFormatter()

    fun toDto(
        item: McCompletionItem,
        annotationContext: AnnotationContext?,
        source: String,
        convertContext: CompletionConvertContext = CompletionConvertContext(
            source = source,
            annotationContext = annotationContext,
        ),
    ): McdevCompletionItemDto {
        val insertText = resolveInsertText(item, convertContext)
        val edit = annotationContext?.let { context ->
            val range = offsetRange(
                source = source,
                startOffset = context.valueStartOffset,
                endOffset = context.valueEndOffset.coerceAtLeast(context.valueStartOffset),
            )
            McdevTextEdit(
                range = range,
                newText = insertText,
            )
        }
        return McdevCompletionItemDto(
            label = item.label,
            detail = item.detail,
            documentation = item.documentation,
            filterText = item.filterText,
            insertText = insertText,
            kind = item.kind.toProtocolKind(),
            sortKey = item.sortKey,
            edit = edit,
            additionalEdits = buildAdditionalEdits(item, convertContext),
            metadata = buildMetadata(item),
        )
    }

    fun toDtos(
        items: List<McCompletionItem>,
        annotationContext: AnnotationContext?,
        source: String,
        convertContext: CompletionConvertContext = CompletionConvertContext(
            source = source,
            annotationContext = annotationContext,
        ),
    ): List<McdevCompletionItemDto> = items.map { toDto(it, annotationContext, source, convertContext) }

    private fun resolveInsertText(item: McCompletionItem, context: CompletionConvertContext): String {
        if (context.preferredAtTarget.equals("descriptor", ignoreCase = true) &&
            item.metadata.source == "mixin.atTarget" &&
            context.mappingResolver != null
        ) {
            val candidate = atTargetCandidateFromItem(item) ?: return item.insertText
            return atTargetInsertFormatter.formatInsert(
                candidate = candidate,
                resolver = context.mappingResolver,
                from = context.sourceNamespace,
                to = context.runtimeNamespace,
            )
        }
        return item.insertText
    }

    private fun buildAdditionalEdits(
        item: McCompletionItem,
        context: CompletionConvertContext,
    ): List<McdevTextEdit> {
        val edits = item.additionalEdits.map { it.toProtocolEdit(context.source) }.toMutableList()
        if (context.classInsertMode == MixinClassInsertMode.IMPORT &&
            item.metadata.source == "mixin.target" &&
            context.annotationContext?.annotation == MixinAnnotation.MIXIN &&
            context.annotationContext.slot == AnnotationSlot.CLASS
        ) {
            val fqn = item.metadata.owner?.replace('/', '.')
            if (fqn != null) {
                MixinImportEditBuilder.buildImportEdit(context.source, fqn)
                    ?.toProtocolEdit(context.source)
                    ?.let { importEdit ->
                        if (edits.none { it.range == importEdit.range && it.newText == importEdit.newText }) {
                            edits += importEdit
                        }
                    }
            }
        }
        return edits
    }

    private fun atTargetCandidateFromItem(item: McCompletionItem): AtTargetCandidate? {
        val owner = item.metadata.owner ?: return null
        val name = item.metadata.name ?: return null
        val descriptor = item.metadata.descriptor ?: return null
        return AtTargetCandidate(
            owner = owner,
            name = name,
            descriptor = descriptor,
            displayLabel = item.label,
            detail = item.detail.orEmpty(),
            kind = inferAtTargetKind(item.insertText, item.kind),
        )
    }

    private fun inferAtTargetKind(insertText: String, kind: McCompletionKind): AtTargetKind = when {
        insertText == "RETURN" -> AtTargetKind.RETURN
        insertText.startsWith('"') -> AtTargetKind.CONSTANT
        insertText.contains("<init>") -> AtTargetKind.NEW
        kind == McCompletionKind.FIELD -> AtTargetKind.FIELD
        insertText.endsWith(";") && !insertText.substringAfter(';').contains('(') -> AtTargetKind.NEW
        else -> AtTargetKind.INVOKE
    }

    private fun McTextEdit.toProtocolEdit(source: String): McdevTextEdit =
        McdevTextEdit(
            range = offsetRange(source, startOffset, endOffset),
            newText = newText,
        )

    private fun buildMetadata(item: McCompletionItem): Map<String, String?> =
        mapOf(
            "source" to item.metadata.source,
            "owner" to item.metadata.owner,
            "name" to item.metadata.name,
            "descriptor" to item.metadata.descriptor,
            "namespace" to item.metadata.namespace,
        )

    private fun McCompletionKind.toProtocolKind(): String = when (this) {
        McCompletionKind.CLASS -> "class"
        McCompletionKind.METHOD -> "method"
        McCompletionKind.FIELD -> "field"
        McCompletionKind.KEYWORD -> "keyword"
        McCompletionKind.VALUE -> "value"
    }

    private fun offsetRange(source: String, startOffset: Int, endOffset: Int): McdevRange =
        McdevRange(
            start = offsetToPosition(source, startOffset),
            end = offsetToPosition(source, endOffset),
        )

    private fun offsetToPosition(source: String, offset: Int): McdevPosition {
        val safeOffset = offset.coerceIn(0, source.length)
        var line = 0
        var lastLineStart = 0
        var index = 0
        while (index < safeOffset) {
            if (source[index] == '\n') {
                line++
                lastLineStart = index + 1
            }
            index++
        }
        return McdevPosition(line = line, character = safeOffset - lastLineStart)
    }

    fun extractAnnotationContext(source: String, line: Int, character: Int): AnnotationContext? =
        AnnotationContextExtractor.extract(source, line, character)
}
