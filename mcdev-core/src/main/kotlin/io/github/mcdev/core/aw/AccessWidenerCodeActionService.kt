package io.github.mcdev.core.aw

import io.github.mcdev.core.codeaction.AddAccessWidenerDescriptorFix
import io.github.mcdev.core.codeaction.FixAccessWidenerDescriptorFix
import io.github.mcdev.core.codeaction.GenerateAccessWidenerEntryFix
import io.github.mcdev.core.codeaction.McFix
import io.github.mcdev.core.codeaction.McTextEdit
import io.github.mcdev.core.codeaction.RemapAccessWidenerEntryFix
import io.github.mcdev.core.codeaction.RemoveDuplicateAccessWidenerEntryFix
import io.github.mcdev.core.codeaction.WorkspaceEditFix
import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.core.mapping.ClassRef
import io.github.mcdev.core.mapping.FieldRef
import io.github.mcdev.core.mapping.MappingLookupResult
import io.github.mcdev.core.mapping.MethodRef
import io.github.mcdev.core.mapping.ProjectMappingContext
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.model.MappingNamespace

class AccessWidenerCodeActionService(
    private val editor: AccessWidenerEditor = AccessWidenerEditor,
) {
    fun fixesForDiagnostics(
        diagnostics: List<McDiagnostic>,
        documentUri: String,
        source: String,
        classIndex: ClassIndex? = null,
        mappingContext: ProjectMappingContext? = null,
    ): List<McFix> {
        val fixes = mutableListOf<McFix>()
        for (diagnostic in diagnostics) {
            fixes += when (diagnostic.code) {
                AwDiagnosticCodes.DUPLICATE_ENTRY -> {
                    val line = diagnostic.metadata["line"]?.toIntOrNull() ?: continue
                    listOf(
                        RemoveDuplicateAccessWidenerEntryFix(
                            title = "Remove duplicate access widener entry",
                            documentUri = documentUri,
                            lineNumber = line,
                        ),
                    )
                }
                AwDiagnosticCodes.NAMESPACE_MISMATCH -> {
                    if (classIndex == null || mappingContext == null) emptyList() else {
                        remapNamespaceFix(documentUri, source, diagnostic, classIndex, mappingContext)
                    }
                }
                AwDiagnosticCodes.INVALID_DESCRIPTOR -> {
                    if (classIndex == null) emptyList() else {
                        descriptorFix(documentUri, source, diagnostic, classIndex, fix = true)
                    }
                }
                AwDiagnosticCodes.UNRESOLVED_MEMBER -> {
                    if (classIndex == null) emptyList() else {
                        descriptorFix(documentUri, source, diagnostic, classIndex, fix = false)
                    }
                }
                else -> emptyList()
            }
        }
        return fixes
    }

    fun generateEntryFix(
        documentUri: String,
        entry: AccessWidenerEntry,
        insertLine: Int? = null,
    ): GenerateAccessWidenerEntryFix =
        GenerateAccessWidenerEntryFix(
            title = "Generate access widener entry",
            documentUri = documentUri,
            insertLine = insertLine ?: sourceLineCount(documentUri),
            entry = editor.formatEntry(entry),
        )

    fun applyGenerateEntryFix(fix: GenerateAccessWidenerEntryFix, currentContent: String): WorkspaceEditFix {
        val lines = currentContent.lineSequence().toList()
        val insertAt = fix.insertLine.coerceIn(1, lines.size)
        val newContent = buildString {
            lines.forEachIndexed { index, line ->
                if (index + 1 == insertAt) {
                    appendLine(fix.entry)
                }
                append(line)
                if (index < lines.lastIndex) append('\n')
            }
            if (insertAt > lines.size) {
                if (isNotEmpty()) append('\n')
                append(fix.entry)
            }
            if (currentContent.endsWith("\n")) append('\n')
        }
        return WorkspaceEditFix(
            title = fix.title,
            kind = fix.kind,
            documentUri = fix.documentUri,
            edits = listOf(McTextEdit(0, currentContent.length, newContent)),
        )
    }

    fun applyRemapEntryFix(fix: RemapAccessWidenerEntryFix, currentContent: String): WorkspaceEditFix =
        WorkspaceEditFix(
            title = fix.title,
            kind = fix.kind,
            documentUri = fix.documentUri,
            edits = listOf(
                McTextEdit(
                    0,
                    currentContent.length,
                    editor.replaceLine(currentContent, fix.lineNumber, fix.newLine),
                ),
            ),
        )

    fun applyDescriptorFix(
        fix: McFix,
        currentContent: String,
    ): WorkspaceEditFix? = when (fix) {
        is AddAccessWidenerDescriptorFix -> WorkspaceEditFix(
            title = fix.title,
            kind = fix.kind,
            documentUri = fix.documentUri,
            edits = listOf(McTextEdit(fix.startOffset, fix.endOffset, " ${fix.descriptor}")),
        )
        is FixAccessWidenerDescriptorFix -> WorkspaceEditFix(
            title = fix.title,
            kind = fix.kind,
            documentUri = fix.documentUri,
            edits = listOf(McTextEdit(fix.startOffset, fix.endOffset, fix.descriptor)),
        )
        else -> null
    }

    fun applyRemoveDuplicateFix(fix: RemoveDuplicateAccessWidenerEntryFix, currentContent: String): WorkspaceEditFix =
        WorkspaceEditFix(
            title = fix.title,
            kind = fix.kind,
            documentUri = fix.documentUri,
            edits = listOf(
                McTextEdit(
                    0,
                    currentContent.length,
                    editor.removeLine(currentContent, fix.lineNumber),
                ),
            ),
        )

    private fun remapNamespaceFix(
        documentUri: String,
        source: String,
        diagnostic: McDiagnostic,
        classIndex: ClassIndex,
        mappingContext: ProjectMappingContext,
    ): List<McFix> {
        val lineNumber = diagnostic.metadata["line"]?.toIntOrNull() ?: return emptyList()
        val parseResult = AccessWidenerParser.parse(source) as? AccessWidenerParseResult.Success ?: return emptyList()
        val entry = parseResult.file.entries.find { it.line == lineNumber } ?: return emptyList()
        val resolvedOwner = AwNamespaceHelper.resolveOwnerInIndex(
            entry.owner,
            parseResult.file.namespace,
            classIndex,
            mappingContext,
        ) ?: return emptyList()
        val targetNamespace = parseResult.file.namespace
        val remappedOwner = AwNamespaceHelper.insertOwner(
            resolvedOwner,
            targetNamespace,
            mappingContext.resolver,
        )
        val remappedName = entry.name?.let { name ->
            AwNamespaceHelper.insertMemberName(
                resolvedOwner,
                name,
                entry.descriptor.orEmpty(),
                entry.kind,
                targetNamespace,
                mappingContext.resolver,
            )
        }
        val remappedDescriptor = entry.descriptor?.let { descriptor ->
            AwNamespaceHelper.insertDescriptor(
                resolvedOwner,
                descriptor,
                entry.kind,
                targetNamespace,
                mappingContext.resolver,
            )
        }
        val newLine = when (entry.kind) {
            AccessWidenerKind.CLASS -> editor.formatClassEntry(entry.directive, remappedOwner)
            AccessWidenerKind.METHOD -> editor.formatMethodEntry(
                entry.directive,
                remappedOwner,
                remappedName ?: entry.name.orEmpty(),
                remappedDescriptor ?: entry.descriptor.orEmpty(),
            )
            AccessWidenerKind.FIELD -> editor.formatFieldEntry(
                entry.directive,
                remappedOwner,
                remappedName ?: entry.name.orEmpty(),
                remappedDescriptor ?: entry.descriptor.orEmpty(),
            )
        }
        return listOf(
            RemapAccessWidenerEntryFix(
                title = "Remap access widener entry to ${targetNamespace.name.lowercase()} namespace",
                documentUri = documentUri,
                lineNumber = lineNumber,
                newLine = newLine,
            ),
        )
    }

    private fun descriptorFix(
        documentUri: String,
        source: String,
        diagnostic: McDiagnostic,
        classIndex: ClassIndex,
        fix: Boolean,
    ): List<McFix> {
        val lineNumber = diagnostic.metadata["line"]?.toIntOrNull() ?: return emptyList()
        val parseResult = AccessWidenerParser.parse(source) as? AccessWidenerParseResult.Success ?: return emptyList()
        val entry = parseResult.file.entries.find { it.line == lineNumber } ?: return emptyList()
        val resolvedOwner = AwNamespaceHelper.resolveOwnerInIndex(
            entry.owner,
            parseResult.file.namespace,
            classIndex,
            null,
        ) ?: return emptyList()
        val name = entry.name ?: return emptyList()
        val descriptor = when (entry.kind) {
            AccessWidenerKind.METHOD -> classIndex.getMethods(resolvedOwner).firstOrNull { it.name == name }?.descriptor
            AccessWidenerKind.FIELD -> classIndex.getFields(resolvedOwner).firstOrNull { it.name == name }?.descriptor
            AccessWidenerKind.CLASS -> null
        } ?: return emptyList()
        val range = descriptorTokenOffsets(source, lineNumber) ?: return emptyList()
        val fixType = if (fix) {
            FixAccessWidenerDescriptorFix(
                title = "Fix access widener descriptor",
                documentUri = documentUri,
                startOffset = range.first,
                endOffset = range.second,
                descriptor = descriptor,
            )
        } else {
            AddAccessWidenerDescriptorFix(
                title = "Add missing access widener descriptor",
                documentUri = documentUri,
                startOffset = range.first,
                endOffset = range.second,
                descriptor = descriptor,
            )
        }
        return listOf(fixType)
    }

    private fun descriptorTokenOffsets(source: String, lineNumber: Int): Pair<Int, Int>? {
        val lineStart = lineStartOffset(source, lineNumber)
        val rawLine = source.lineSequence().elementAt(lineNumber - 1)
        val contentLine = rawLine.substringBefore('#').trim()
        val parts = contentLine.split(Regex("\\s+"))
        if (parts.size < 4) {
            val end = lineStart + rawLine.indexOf(contentLine) + contentLine.length
            return end to end
        }
        val before = parts.take(4).sumOf { it.length + 1 }
        val token = parts.getOrNull(4).orEmpty()
        val start = lineStart + rawLine.indexOf(contentLine) + before
        return if (token.isEmpty()) start to start else start to (start + token.length)
    }

    private fun lineStartOffset(source: String, lineNumber: Int): Int {
        var offset = 0
        repeat((lineNumber - 1).coerceAtLeast(0)) {
            val next = source.indexOf('\n', offset)
            offset = if (next < 0) source.length else next + 1
        }
        return offset
    }

    private fun sourceLineCount(documentUri: String): Int = 1
}
