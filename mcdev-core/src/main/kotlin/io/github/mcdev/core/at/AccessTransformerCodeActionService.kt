package io.github.mcdev.core.at

import io.github.mcdev.core.codeaction.AddAccessTransformerEntryFix
import io.github.mcdev.core.codeaction.AddAtMethodDescriptorFix
import io.github.mcdev.core.codeaction.McFix
import io.github.mcdev.core.codeaction.McTextEdit
import io.github.mcdev.core.codeaction.RemapAccessTransformerEntryFix
import io.github.mcdev.core.codeaction.RemoveDuplicateAtEntryFix
import io.github.mcdev.core.codeaction.WorkspaceEditFix
import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.core.mapping.ProjectMappingContext
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.FieldIndexEntry
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.model.MemberKind

class AccessTransformerCodeActionService(
    private val classIndex: ClassIndex,
    private val mappingContext: ProjectMappingContext? = null,
    private val editor: AccessTransformerEditor = AccessTransformerEditor(),
    private val insertFormatter: AtMemberInsertFormatter = AtMemberInsertFormatter(),
    private val memberResolver: AtMemberResolver = AtMemberResolver(),
) {
    fun fixesForDiagnostics(
        diagnostics: List<McDiagnostic>,
        documentUri: String,
        source: String,
        classIndex: ClassIndex = this.classIndex,
        mappingContext: ProjectMappingContext? = this.mappingContext,
    ): List<McFix> {
        val fixes = mutableListOf<McFix>()
        for (diagnostic in diagnostics) {
            fixes += when (diagnostic.code) {
                AtDiagnosticCodes.MISSING_METHOD_DESCRIPTOR -> {
                    val descriptor = diagnostic.metadata["descriptor"] ?: continue
                    val member = diagnostic.metadata["member"] ?: continue
                    listOf(
                        AddAtMethodDescriptorFix(
                            title = "Add method descriptor",
                            documentUri = documentUri,
                            line = diagnostic.metadata["line"]?.toIntOrNull() ?: continue,
                            memberName = member,
                            descriptor = descriptor,
                        ),
                    )
                }
                AtDiagnosticCodes.WRONG_NAMESPACE,
                AtDiagnosticCodes.SRG_MAPPING_NOT_FOUND,
                -> {
                    val line = diagnostic.metadata["line"]?.toIntOrNull() ?: continue
                    if (
                        diagnostic.code == AtDiagnosticCodes.WRONG_NAMESPACE &&
                        remapEntryOnLine(source, line, classIndex, mappingContext) == null
                    ) {
                        emptyList()
                    } else {
                        listOf(
                            RemapAccessTransformerEntryFix(
                                title = "Remap AT entry namespace",
                                documentUri = documentUri,
                                line = line,
                            ),
                        )
                    }
                }
                AtDiagnosticCodes.DUPLICATE_ENTRY -> {
                    val line = diagnostic.metadata["line"]?.toIntOrNull() ?: continue
                    listOf(
                        RemoveDuplicateAtEntryFix(
                            title = "Remove duplicate entry",
                            documentUri = documentUri,
                            line = line,
                        ),
                    )
                }
                else -> emptyList()
            }
        }
        return fixes
    }

    fun generateEntryFix(
        context: AtContext,
        documentUri: String,
    ): AddAccessTransformerEntryFix? {
        val modifier = context.modifier ?: return null
        val owner = context.owner ?: return null
        return AddAccessTransformerEntryFix(
            title = "Generate Access Transformer entry",
            documentUri = documentUri,
            modifier = modifier.token,
            owner = owner,
            memberName = context.memberName,
            memberDescriptor = context.memberDescriptor,
            insertLine = context.lineNumber,
        )
    }

    fun applyAddEntryFix(
        fix: AddAccessTransformerEntryFix,
        currentContent: String,
    ): WorkspaceEditFix? {
        val modifier = AccessTransformerModifier.entries.firstOrNull { it.token == fix.modifier } ?: return null
        val entry = AccessTransformerEntry(
            modifier = modifier,
            owner = fix.owner,
            name = fix.memberName,
            descriptor = fix.memberDescriptor,
            line = fix.insertLine,
        )
        val result = editor.appendEntry(currentContent, entry)
        return WorkspaceEditFix(
            title = fix.title,
            kind = fix.kind,
            documentUri = fix.documentUri,
            edits = listOf(
                McTextEdit(
                    startOffset = 0,
                    endOffset = currentContent.length,
                    newText = result.content,
                ),
            ),
        )
    }

    fun applyMethodDescriptorFix(
        fix: AddAtMethodDescriptorFix,
        currentContent: String,
    ): WorkspaceEditFix? {
        val lineContext = AtContextExtractor.parseLine(currentContent, fix.line) ?: return null
        val memberToken = lineContext.member ?: return null
        val openParen = memberToken.text.indexOf('(')
        val baseName = if (openParen >= 0) memberToken.text.substring(0, openParen) else memberToken.text
        if (baseName != fix.memberName) return null
        val newMember = "${fix.memberName}${fix.descriptor}"
        val newLine = rebuildLine(lineContext, memberReplacement = newMember)
        val result = editor.replaceLine(currentContent, fix.line, newLine)
        if (!result.changed) return null
        return workspaceReplace(currentContent, fix.documentUri, fix.title, fix.kind, result.content)
    }

    fun applyRemapFix(
        fix: RemapAccessTransformerEntryFix,
        currentContent: String,
        classIndex: ClassIndex = this.classIndex,
        mappingContext: ProjectMappingContext? = this.mappingContext,
    ): WorkspaceEditFix? {
        if (mappingContext?.atNamespace == null) return null
        val parseResult = AccessTransformerParser.parse(currentContent)
        if (parseResult !is AccessTransformerParseResult.Success) return null
        val entry = parseResult.file.entries.find { it.line == fix.line } ?: return null
        val ownerEntry = classIndex.findClassByFqn(entry.owner)
            ?: classIndex.findClass(entry.owner.replace('.', '/'))
            ?: return null
        val remapped = remapEntry(entry, ownerEntry.internalName, classIndex, mappingContext) ?: return null
        val newLine = editor.formatEntry(remapped)
        val result = editor.replaceLine(currentContent, fix.line, newLine)
        if (!result.changed) return null
        return workspaceReplace(currentContent, fix.documentUri, fix.title, fix.kind, result.content)
    }

    fun applyRemoveDuplicateFix(
        fix: RemoveDuplicateAtEntryFix,
        currentContent: String,
    ): WorkspaceEditFix? {
        val result = editor.removeLine(currentContent, fix.line)
        if (!result.changed) return null
        return workspaceReplace(currentContent, fix.documentUri, fix.title, fix.kind, result.content)
    }

    private fun remapEntry(
        entry: AccessTransformerEntry,
        ownerInternalName: String,
        classIndex: ClassIndex,
        mappingContext: ProjectMappingContext?,
    ): AccessTransformerEntry? {
        val memberName = entry.name ?: return entry
        val resolution = memberResolver.resolve(
            ownerInternalName = ownerInternalName,
            memberName = memberName,
            memberDescriptor = entry.descriptor,
            classIndex = classIndex,
            mappingContext = mappingContext,
        )
        return when (resolution) {
            is AtMemberResolution.Found -> {
                when (resolution.member.kind) {
                    MemberKind.METHOD -> {
                        val method = resolution.member.method ?: return null
                        val insert = insertFormatter.remapMethodForEntry(ownerInternalName, method, mappingContext!!)
                        val descriptor = insert.insertText.substringAfter('(', missingDelimiterValue = "").let {
                            if (it.isEmpty()) null else "($it"
                        }
                        entry.copy(
                            name = insert.insertText.substringBefore('('),
                            descriptor = descriptor,
                        )
                    }
                    MemberKind.FIELD -> {
                        val field = resolution.member.field ?: return null
                        val insert = insertFormatter.remapFieldForEntry(ownerInternalName, field, mappingContext!!)
                        entry.copy(name = insert.insertText, descriptor = null)
                    }
                    else -> entry
                }
            }
            is AtMemberResolution.WrongNamespace -> {
                val method = resolveMethodForRemap(
                    ownerInternalName = ownerInternalName,
                    name = resolution.namedName,
                    descriptor = entry.descriptor,
                    classIndex = classIndex,
                )
                if (method != null) {
                    val insert = insertFormatter.remapMethodForEntry(ownerInternalName, method, mappingContext!!)
                    val descriptor = insert.insertText.substringAfter('(', missingDelimiterValue = "").let {
                        if (it.isEmpty()) null else "($it"
                    }
                    return entry.copy(
                        name = insert.insertText.substringBefore('('),
                        descriptor = descriptor,
                    )
                }
                val field = resolveFieldForRemap(
                    ownerInternalName = ownerInternalName,
                    name = resolution.namedName,
                    descriptor = entry.descriptor,
                    classIndex = classIndex,
                )
                if (field != null) {
                    val insert = insertFormatter.remapFieldForEntry(ownerInternalName, field, mappingContext!!)
                    return entry.copy(name = insert.insertText, descriptor = null)
                }
                null
            }
            else -> null
        }
    }

    private fun remapEntryOnLine(
        source: String,
        line: Int,
        classIndex: ClassIndex,
        mappingContext: ProjectMappingContext?,
    ): AccessTransformerEntry? {
        if (mappingContext?.atNamespace == null) return null
        val parseResult = AccessTransformerParser.parse(source)
        if (parseResult !is AccessTransformerParseResult.Success) return null
        val entry = parseResult.file.entries.find { it.line == line } ?: return null
        val ownerEntry = classIndex.findClassByFqn(entry.owner)
            ?: classIndex.findClass(entry.owner.replace('.', '/'))
            ?: return null
        return remapEntry(entry, ownerEntry.internalName, classIndex, mappingContext)
    }

    private fun resolveMethodForRemap(
        ownerInternalName: String,
        name: String,
        descriptor: String?,
        classIndex: ClassIndex,
    ): MethodIndexEntry? {
        val candidates = classIndex.getMethods(ownerInternalName).filter { it.name == name }
        return if (descriptor != null) {
            candidates.find { it.descriptor == descriptor }
        } else {
            candidates.singleOrNull()
        }
    }

    private fun resolveFieldForRemap(
        ownerInternalName: String,
        name: String,
        descriptor: String?,
        classIndex: ClassIndex,
    ): FieldIndexEntry? {
        val candidates = classIndex.getFields(ownerInternalName).filter { it.name == name }
        return if (descriptor != null) {
            candidates.find { it.descriptor == descriptor }
        } else {
            candidates.singleOrNull()
        }
    }

    private fun rebuildLine(lineContext: AtLineContext, memberReplacement: String): String {
        val modifier = lineContext.modifier?.text ?: return lineContext.content.trim()
        val owner = lineContext.owner?.text ?: return lineContext.content.trim()
        return "$modifier $owner $memberReplacement"
    }

    private fun workspaceReplace(
        currentContent: String,
        documentUri: String,
        title: String,
        kind: String,
        newContent: String,
    ): WorkspaceEditFix = WorkspaceEditFix(
        title = title,
        kind = kind,
        documentUri = documentUri,
        edits = listOf(
            McTextEdit(
                startOffset = 0,
                endOffset = currentContent.length,
                newText = newContent,
            ),
        ),
    )
}
