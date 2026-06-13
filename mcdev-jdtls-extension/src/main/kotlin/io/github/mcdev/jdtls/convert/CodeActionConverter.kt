package io.github.mcdev.jdtls.convert

import io.github.mcdev.core.at.AccessTransformerCodeActionService
import io.github.mcdev.core.aw.AccessWidenerCodeActionService
import io.github.mcdev.core.codeaction.AddAccessTransformerEntryFix
import io.github.mcdev.core.codeaction.AddAccessWidenerDescriptorFix
import io.github.mcdev.core.codeaction.AddAtMethodDescriptorFix
import io.github.mcdev.core.codeaction.AddMethodDescriptorFix
import io.github.mcdev.core.codeaction.AddMixinConfigEntryFix
import io.github.mcdev.core.codeaction.FixAccessWidenerDescriptorFix
import io.github.mcdev.core.codeaction.GenerateAccessWidenerEntryFix
import io.github.mcdev.core.codeaction.GenerateAccessorMethodFix
import io.github.mcdev.core.codeaction.GenerateInvokerMethodFix
import io.github.mcdev.core.codeaction.McFix
import io.github.mcdev.core.codeaction.McTextEdit
import io.github.mcdev.core.codeaction.RemapAccessTransformerEntryFix
import io.github.mcdev.core.codeaction.RemapAccessWidenerEntryFix
import io.github.mcdev.core.codeaction.RemoveDuplicateAccessWidenerEntryFix
import io.github.mcdev.core.codeaction.RemoveDuplicateAtEntryFix
import io.github.mcdev.core.codeaction.WorkspaceEditFix
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.MixinCodeActionService
import io.github.mcdev.protocol.McdevCodeActionDto
import io.github.mcdev.protocol.McdevPosition
import io.github.mcdev.protocol.McdevRange
import io.github.mcdev.protocol.McdevTextEdit
import io.github.mcdev.protocol.McdevWorkspaceEdit

object CodeActionConverter {
    fun toDto(
        fix: McFix,
        source: String,
        mixinConfigContent: String?,
        codeActionService: MixinCodeActionService = MixinCodeActionService(),
        awCodeActionService: AccessWidenerCodeActionService = AccessWidenerCodeActionService(),
        atCodeActionService: AccessTransformerCodeActionService = AccessTransformerCodeActionService(EmptyClassIndex),
    ): McdevCodeActionDto? {
        val workspaceEdit = resolveWorkspaceEdit(
            fix = fix,
            source = source,
            mixinConfigContent = mixinConfigContent,
            codeActionService = codeActionService,
            awCodeActionService = awCodeActionService,
            atCodeActionService = atCodeActionService,
        ) ?: return null
        return McdevCodeActionDto(
            title = fix.title,
            kind = fix.kind,
            edits = listOf(workspaceEdit),
            metadata = metadataFor(fix),
        )
    }

    fun toDtos(
        fixes: List<McFix>,
        source: String,
        mixinConfigContent: String?,
        codeActionService: MixinCodeActionService = MixinCodeActionService(),
        awCodeActionService: AccessWidenerCodeActionService = AccessWidenerCodeActionService(),
        atCodeActionService: AccessTransformerCodeActionService = AccessTransformerCodeActionService(EmptyClassIndex),
    ): List<McdevCodeActionDto> =
        fixes.mapNotNull {
            toDto(
                fix = it,
                source = source,
                mixinConfigContent = mixinConfigContent,
                codeActionService = codeActionService,
                awCodeActionService = awCodeActionService,
                atCodeActionService = atCodeActionService,
            )
        }

    private fun resolveWorkspaceEdit(
        fix: McFix,
        source: String,
        mixinConfigContent: String?,
        codeActionService: MixinCodeActionService,
        awCodeActionService: AccessWidenerCodeActionService,
        atCodeActionService: AccessTransformerCodeActionService,
    ): McdevWorkspaceEdit? =
        when (fix) {
            is WorkspaceEditFix -> workspaceEditFrom(source, fix)
            is RemoveDuplicateAccessWidenerEntryFix ->
                workspaceEditFrom(source, awCodeActionService.applyRemoveDuplicateFix(fix, source))
            is RemapAccessWidenerEntryFix ->
                workspaceEditFrom(source, awCodeActionService.applyRemapEntryFix(fix, source))
            is GenerateAccessWidenerEntryFix ->
                workspaceEditFrom(source, awCodeActionService.applyGenerateEntryFix(fix, source))
            is AddAccessWidenerDescriptorFix,
            is FixAccessWidenerDescriptorFix,
            -> awCodeActionService.applyDescriptorFix(fix, source)?.let { workspaceEditFrom(source, it) }
            is AddAtMethodDescriptorFix ->
                atCodeActionService.applyMethodDescriptorFix(fix, source)?.let { workspaceEditFrom(source, it) }
            is RemapAccessTransformerEntryFix ->
                atCodeActionService.applyRemapFix(fix, source)?.let { workspaceEditFrom(source, it) }
            is RemoveDuplicateAtEntryFix ->
                atCodeActionService.applyRemoveDuplicateFix(fix, source)?.let { workspaceEditFrom(source, it) }
            is AddAccessTransformerEntryFix ->
                atCodeActionService.applyAddEntryFix(fix, source)?.let { workspaceEditFrom(source, it) }
            is AddMixinConfigEntryFix -> {
                val content = mixinConfigContent ?: return null
                val applied = codeActionService.applyMixinConfigFix(fix, content) ?: return null
                McdevWorkspaceEdit(
                    documentUri = applied.documentUri,
                    edits = applied.edits.map { edit ->
                        McdevTextEdit(
                            range = fullDocumentRange(content),
                            newText = edit.newText,
                        )
                    },
                )
            }
            is AddMethodDescriptorFix -> {
                val applied = codeActionService.applyMethodDescriptorFix(fix, source)
                McdevWorkspaceEdit(
                    documentUri = applied.documentUri,
                    edits = applied.edits.map { toTextEdit(source, it) },
                )
            }
            is GenerateAccessorMethodFix -> McdevWorkspaceEdit(
                documentUri = fix.documentUri,
                edits = listOf(toTextEdit(source, McTextEdit(fix.insertOffset, fix.insertOffset, fix.methodSource))),
            )
            is GenerateInvokerMethodFix -> McdevWorkspaceEdit(
                documentUri = fix.documentUri,
                edits = listOf(toTextEdit(source, McTextEdit(fix.insertOffset, fix.insertOffset, fix.methodSource))),
            )
            else -> null
        }

    private fun workspaceEditFrom(source: String, fix: WorkspaceEditFix): McdevWorkspaceEdit =
        McdevWorkspaceEdit(
            documentUri = fix.documentUri,
            edits = fix.edits.map { toTextEdit(source, it) },
        )

    private fun metadataFor(fix: McFix): Map<String, String> =
        when (fix) {
            is WorkspaceEditFix -> fix.metadata
            is AddMixinConfigEntryFix -> buildMap {
                put("mixinClass", fix.mixinClassName)
                put("configPath", fix.configPath)
                fix.mixinPackage?.let { put("mixinPackage", it) }
            }
            is GenerateAccessorMethodFix -> mapOf("field" to fix.fieldName)
            is GenerateInvokerMethodFix -> mapOf("method" to fix.targetMethodName)
            else -> emptyMap()
        }

    private fun toTextEdit(source: String, edit: McTextEdit): McdevTextEdit =
        McdevTextEdit(
            range = offsetRange(source, edit.startOffset, edit.endOffset),
            newText = edit.newText,
        )

    private fun offsetRange(source: String, startOffset: Int, endOffset: Int): McdevRange =
        McdevRange(
            start = offsetToPosition(source, startOffset),
            end = offsetToPosition(source, endOffset.coerceAtLeast(startOffset)),
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

    private fun fullDocumentRange(content: String): McdevRange =
        McdevRange(
            start = McdevPosition(0, 0),
            end = offsetToPosition(content, content.length),
        )

    private object EmptyClassIndex : ClassIndex {
        override fun findClasses(prefix: String, limit: Int): List<io.github.mcdev.core.mixin.ClassIndexEntry> = emptyList()

        override fun findClass(internalName: String): io.github.mcdev.core.mixin.ClassIndexEntry? = null

        override fun findClassByFqn(fqn: String): io.github.mcdev.core.mixin.ClassIndexEntry? = null

        override fun getMethods(ownerInternalName: String): List<io.github.mcdev.core.mixin.MethodIndexEntry> = emptyList()

        override fun getFields(ownerInternalName: String): List<io.github.mcdev.core.mixin.FieldIndexEntry> = emptyList()
    }
}
