package io.github.mcdev.core.aw

import io.github.mcdev.core.definition.McDefinitionTarget
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mapping.ProjectMappingContext
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.model.MemberKind

class AccessWidenerDefinitionService(
    private val classIndex: ClassIndex,
    private val mappingContext: ProjectMappingContext? = null,
) {
    fun definitionsAt(source: String, line: Int, character: Int): List<McDefinitionTarget> {
        val context = AwContextExtractor.extract(source, line, character) ?: return emptyList()
        return definitionsFromContext(source, context)
    }

    fun definitionsAtOffset(source: String, offset: Int): List<McDefinitionTarget> {
        val context = AwContextExtractor.extractAtOffset(source, offset) ?: return emptyList()
        return definitionsFromContext(source, context)
    }

    private fun definitionsFromContext(source: String, context: AwAnnotationContext): List<McDefinitionTarget> {
        if (context.isHeaderLine) return emptyList()
        val range = offsetRange(source, context.valueStartOffset, context.valueEndOffset)
        return when (context.slot) {
            AwSyntaxSlot.OWNER -> resolveOwnerDefinition(context, range)
            AwSyntaxSlot.NAME -> resolveMemberDefinition(context, range, requireDescriptor = false)
            AwSyntaxSlot.DESCRIPTOR -> resolveMemberDefinition(context, range, requireDescriptor = true)
            else -> emptyList()
        }
    }

    private fun resolveOwnerDefinition(
        context: AwAnnotationContext,
        range: McTextRange,
    ): List<McDefinitionTarget> {
        val owner = context.owner?.trim().orEmpty()
        if (owner.isEmpty()) return emptyList()
        val resolvedOwner = AwNamespaceHelper.resolveOwnerInIndex(
            owner = owner,
            fileNamespace = context.fileNamespace,
            classIndex = classIndex,
            mappingContext = mappingContext,
        ) ?: return emptyList()
        return listOf(
            McDefinitionTarget(
                kind = MemberKind.CLASS,
                ownerInternalName = resolvedOwner,
                ownerFqn = fqnFor(resolvedOwner),
                sourceRange = range,
            ),
        )
    }

    private fun resolveMemberDefinition(
        context: AwAnnotationContext,
        range: McTextRange,
        requireDescriptor: Boolean,
    ): List<McDefinitionTarget> {
        val kind = context.kind ?: return emptyList()
        if (kind == AccessWidenerKind.CLASS) return emptyList()
        val owner = context.owner?.trim().orEmpty()
        val name = context.name?.trim().orEmpty()
        if (owner.isEmpty() || name.isEmpty()) return emptyList()
        val resolvedOwner = AwNamespaceHelper.resolveOwnerInIndex(
            owner = owner,
            fileNamespace = context.fileNamespace,
            classIndex = classIndex,
            mappingContext = mappingContext,
        ) ?: return emptyList()
        val descriptor = context.descriptor?.trim().orEmpty().ifEmpty { null }
        if (requireDescriptor && descriptor.isNullOrEmpty()) return emptyList()

        return when (kind) {
            AccessWidenerKind.METHOD -> {
                val methods = classIndex.getMethods(resolvedOwner).filter { it.name == name }
                val method = when {
                    descriptor != null -> methods.find { it.descriptor == descriptor } ?: methods.firstOrNull()
                    else -> methods.singleOrNull() ?: methods.firstOrNull()
                } ?: return emptyList()
                listOf(
                    McDefinitionTarget(
                        kind = MemberKind.METHOD,
                        ownerInternalName = resolvedOwner,
                        ownerFqn = fqnFor(resolvedOwner),
                        name = method.name,
                        descriptor = method.descriptor,
                        sourceRange = range,
                    ),
                )
            }
            AccessWidenerKind.FIELD -> {
                val fields = classIndex.getFields(resolvedOwner).filter { it.name == name }
                val field = when {
                    descriptor != null -> fields.find { it.descriptor == descriptor } ?: fields.firstOrNull()
                    else -> fields.singleOrNull() ?: fields.firstOrNull()
                } ?: return emptyList()
                listOf(
                    McDefinitionTarget(
                        kind = MemberKind.FIELD,
                        ownerInternalName = resolvedOwner,
                        ownerFqn = fqnFor(resolvedOwner),
                        name = field.name,
                        descriptor = field.descriptor,
                        sourceRange = range,
                    ),
                )
            }
            AccessWidenerKind.CLASS -> emptyList()
        }
    }

    private fun fqnFor(internalName: String): String? =
        classIndex.findClass(internalName)?.fqn ?: internalName.replace('/', '.')

    private fun offsetRange(source: String, startOffset: Int, endOffset: Int): McTextRange =
        McTextRange(
            start = offsetToPosition(source, startOffset),
            end = offsetToPosition(source, endOffset.coerceAtLeast(startOffset)),
        )

    private fun offsetToPosition(source: String, offset: Int): McTextPosition {
        val (line, character) = AwContextExtractor.offsetToPosition(source, offset.coerceIn(0, source.length))
        return McTextPosition(line = line, character = character)
    }
}
