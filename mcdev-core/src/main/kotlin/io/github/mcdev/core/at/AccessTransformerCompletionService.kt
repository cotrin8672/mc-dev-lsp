package io.github.mcdev.core.at

import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata
import io.github.mcdev.core.mapping.ProjectMappingContext
import io.github.mcdev.core.mixin.ClassIndex

class AccessTransformerCompletionService(
    private val classIndex: ClassIndex,
    private val mappingContext: ProjectMappingContext? = null,
    private val insertFormatter: AtMemberInsertFormatter = AtMemberInsertFormatter(),
) {
    fun complete(context: AtContext, mappingContext: ProjectMappingContext? = null): List<McCompletionItem> =
        completeInternal(context, mappingContext ?: this.mappingContext)

    private fun completeInternal(context: AtContext, mappingContext: ProjectMappingContext?): List<McCompletionItem> = when (context.slot) {
        AtSlot.MODIFIER -> completeModifiers(context)
        AtSlot.OWNER -> completeOwners(context)
        AtSlot.MEMBER_NAME -> completeMembers(context, mappingContext)
        AtSlot.MEMBER_DESCRIPTOR -> emptyList()
    }

    private fun completeModifiers(context: AtContext): List<McCompletionItem> {
        val prefix = context.partialValue
        return AccessTransformerModifier.entries
            .filter { it.token.startsWith(prefix) }
            .map { modifier ->
                McCompletionItem(
                    label = modifier.token,
                    detail = "access modifier",
                    documentation = null,
                    filterText = modifier.token,
                    insertText = modifier.token,
                    kind = McCompletionKind.KEYWORD,
                    sortKey = "0100_${modifier.token}",
                    metadata = McCompletionMetadata(source = "at.modifier"),
                )
            }
    }

    private fun completeOwners(context: AtContext): List<McCompletionItem> {
        val prefix = context.partialValue.substringAfterLast('.')
        return classIndex.findClasses(prefix).map { entry ->
            McCompletionItem(
                label = entry.simpleName,
                detail = entry.fqn,
                documentation = entry.internalName,
                filterText = "${entry.simpleName} ${entry.fqn}",
                insertText = insertFormatter.formatOwnerInsert(entry),
                kind = McCompletionKind.CLASS,
                sortKey = "0200_${entry.simpleName}",
                metadata = McCompletionMetadata(
                    source = "at.class",
                    owner = entry.internalName,
                    name = entry.simpleName,
                    namespace = "NAMED",
                ),
            )
        }
    }

    private fun completeMembers(context: AtContext, mappingContext: ProjectMappingContext?): List<McCompletionItem> {
        val ownerFqn = context.owner ?: return emptyList()
        val ownerInternal = classIndex.findClassByFqn(ownerFqn)?.internalName
            ?: classIndex.findClass(ownerFqn.replace('.', '/'))?.internalName
            ?: return emptyList()
        val prefix = context.partialValue
        val methods = classIndex.getMethods(ownerInternal)
            .filter { it.name.startsWith(prefix) }
            .map { method ->
                val insert = insertFormatter.formatMethodInsert(method, ownerInternal, mappingContext)
                McCompletionItem(
                    label = method.readableSignature,
                    detail = "named: ${method.name}",
                    documentation = insert.insertText,
                    filterText = "${method.name} ${method.readableSignature}",
                    insertText = insert.insertText,
                    kind = McCompletionKind.METHOD,
                    sortKey = "0300_${method.name}",
                    metadata = McCompletionMetadata(
                        source = "at.member.method",
                        owner = ownerInternal,
                        name = method.name,
                        descriptor = method.descriptor,
                        namespace = mappingContext?.atNamespace?.name,
                    ),
                )
            }
        val fields = classIndex.getFields(ownerInternal)
            .filter { it.name.startsWith(prefix) }
            .map { field ->
                val insert = insertFormatter.formatFieldInsert(field, ownerInternal, mappingContext)
                McCompletionItem(
                    label = "${field.name}: ${field.readableType}",
                    detail = "named: ${field.name}",
                    documentation = insert.insertText,
                    filterText = "${field.name} ${field.readableType}",
                    insertText = insert.insertText,
                    kind = McCompletionKind.FIELD,
                    sortKey = "0400_${field.name}",
                    metadata = McCompletionMetadata(
                        source = "at.member.field",
                        owner = ownerInternal,
                        name = field.name,
                        descriptor = field.descriptor,
                        namespace = mappingContext?.atNamespace?.name,
                    ),
                )
            }
        return methods + fields
    }
}
