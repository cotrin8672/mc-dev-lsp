package io.github.mcdev.core.aw

import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata
import io.github.mcdev.core.mapping.ProjectMappingContext
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.model.MappingNamespace

data class AccessWidenerCompletionOptions(
    val includeDescriptorsForMembers: Boolean = true,
)

class AccessWidenerCompletionService(
    private val classIndex: ClassIndex,
) {
    fun complete(
        context: AwAnnotationContext,
        mappingContext: ProjectMappingContext? = null,
        options: AccessWidenerCompletionOptions = AccessWidenerCompletionOptions(),
    ): List<McCompletionItem> = when (context.slot) {
        AwSyntaxSlot.HEADER_NAMESPACE -> completeNamespaces(context)
        AwSyntaxSlot.DIRECTIVE -> completeDirectives(context)
        AwSyntaxSlot.KIND -> completeKinds(context)
        AwSyntaxSlot.OWNER -> completeOwners(context, mappingContext)
        AwSyntaxSlot.NAME -> completeMembers(context, mappingContext, options)
        AwSyntaxSlot.DESCRIPTOR -> completeDescriptors(context, mappingContext)
    }

    private fun completeNamespaces(context: AwAnnotationContext): List<McCompletionItem> =
        listOf("named", "intermediary", "official")
            .filter { it.startsWith(context.partialValue.lowercase()) }
            .map { token ->
                McCompletionItem(
                    label = token,
                    detail = "Access widener namespace",
                    documentation = null,
                    filterText = token,
                    insertText = token,
                    kind = McCompletionKind.VALUE,
                    sortKey = "0000_$token",
                    metadata = McCompletionMetadata(source = "aw.namespace", namespace = token.uppercase()),
                )
            }

    private fun completeDirectives(context: AwAnnotationContext): List<McCompletionItem> =
        listOf("accessible", "extendable", "mutable", "natural")
            .filter { it.startsWith(context.partialValue) }
            .map { directive ->
                McCompletionItem(
                    label = directive,
                    detail = "Access widener directive",
                    documentation = null,
                    filterText = directive,
                    insertText = directive,
                    kind = McCompletionKind.KEYWORD,
                    sortKey = "0100_$directive",
                    metadata = McCompletionMetadata(source = "aw.directive", name = directive),
                )
            }

    private fun completeKinds(context: AwAnnotationContext): List<McCompletionItem> =
        listOf("class", "method", "field")
            .filter { kind ->
                kind.startsWith(context.partialValue) &&
                    when (context.directive) {
                        AccessWidenerDirective.MUTABLE -> kind == "field"
                        AccessWidenerDirective.EXTENDABLE -> kind == "class"
                        else -> true
                    }
            }
            .map { kind ->
                McCompletionItem(
                    label = kind,
                    detail = "Access widener kind",
                    documentation = null,
                    filterText = kind,
                    insertText = kind,
                    kind = McCompletionKind.KEYWORD,
                    sortKey = "0200_$kind",
                    metadata = McCompletionMetadata(source = "aw.kind", name = kind),
                )
            }

    private fun completeOwners(
        context: AwAnnotationContext,
        mappingContext: ProjectMappingContext?,
    ): List<McCompletionItem> {
        val prefix = context.partialValue.substringAfterLast('/')
        val targetNamespace = context.fileNamespace ?: mappingContext?.awNamespace
        return classIndex.findClasses(prefix).map { entry ->
            val insertOwner = AwNamespaceHelper.insertOwner(
                entry.internalName,
                targetNamespace,
                mappingContext?.resolver,
            )
            McCompletionItem(
                label = entry.simpleName,
                detail = entry.internalName,
                documentation = entry.fqn,
                filterText = "${entry.simpleName} ${entry.fqn} ${entry.internalName}",
                insertText = insertOwner,
                kind = McCompletionKind.CLASS,
                sortKey = "0300_${entry.simpleName}",
                metadata = McCompletionMetadata(
                    source = "aw.class",
                    owner = entry.internalName,
                    name = entry.simpleName,
                    namespace = targetNamespace?.name,
                ),
            )
        }
    }

    private fun completeMembers(
        context: AwAnnotationContext,
        mappingContext: ProjectMappingContext?,
        options: AccessWidenerCompletionOptions,
    ): List<McCompletionItem> {
        val owner = context.owner ?: return emptyList()
        val kind = context.kind ?: return emptyList()
        val resolvedOwner = AwNamespaceHelper.resolveOwnerInIndex(
            owner,
            context.fileNamespace,
            classIndex,
            mappingContext,
        ) ?: return emptyList()
        val targetNamespace = context.fileNamespace ?: mappingContext?.awNamespace
        val prefix = context.partialValue
        return when (kind) {
            AccessWidenerKind.METHOD -> classIndex.getMethods(resolvedOwner)
                .filter { it.name.startsWith(prefix) }
                .map { method -> memberCompletion(resolvedOwner, method.name, method.descriptor, method.readableSignature, kind, targetNamespace, mappingContext, options, McCompletionKind.METHOD, context.slot) }
            AccessWidenerKind.FIELD -> classIndex.getFields(resolvedOwner)
                .filter { it.name.startsWith(prefix) }
                .map { field ->
                    memberCompletion(
                        resolvedOwner,
                        field.name,
                        field.descriptor,
                        "${field.name}: ${field.readableType}",
                        kind,
                        targetNamespace,
                        mappingContext,
                        options,
                        McCompletionKind.FIELD,
                        context.slot,
                    )
                }
            AccessWidenerKind.CLASS -> emptyList()
        }
    }

    private fun memberCompletion(
        resolvedOwner: String,
        name: String,
        descriptor: String,
        label: String,
        kind: AccessWidenerKind,
        targetNamespace: MappingNamespace?,
        mappingContext: ProjectMappingContext?,
        options: AccessWidenerCompletionOptions,
        completionKind: McCompletionKind,
        slot: AwSyntaxSlot,
    ): McCompletionItem {
        val insertName = AwNamespaceHelper.insertMemberName(
            resolvedOwner,
            name,
            descriptor,
            kind,
            targetNamespace,
            mappingContext?.resolver,
        )
        val insertDescriptor = AwNamespaceHelper.insertDescriptor(
            resolvedOwner,
            descriptor,
            kind,
            targetNamespace,
            mappingContext?.resolver,
        )
        val insertText = if (options.includeDescriptorsForMembers && slot == AwSyntaxSlot.DESCRIPTOR) {
            "$insertName $insertDescriptor"
        } else {
            insertName
        }
        return McCompletionItem(
            label = label,
            detail = resolvedOwner,
            documentation = descriptor,
            filterText = "$name $label",
            insertText = insertText,
            kind = completionKind,
            sortKey = "0400_$name",
            metadata = McCompletionMetadata(
                source = if (kind == AccessWidenerKind.METHOD) "aw.method" else "aw.field",
                owner = resolvedOwner,
                name = name,
                descriptor = descriptor,
                namespace = targetNamespace?.name,
            ),
        )
    }

    private fun completeDescriptors(
        context: AwAnnotationContext,
        mappingContext: ProjectMappingContext?,
    ): List<McCompletionItem> {
        val owner = context.owner ?: return emptyList()
        val name = context.name ?: return emptyList()
        val kind = context.kind ?: return emptyList()
        val resolvedOwner = AwNamespaceHelper.resolveOwnerInIndex(
            owner,
            context.fileNamespace,
            classIndex,
            mappingContext,
        ) ?: return emptyList()
        val targetNamespace = context.fileNamespace ?: mappingContext?.awNamespace
        val partial = context.partialValue
        return when (kind) {
            AccessWidenerKind.METHOD -> classIndex.getMethods(resolvedOwner)
                .filter { it.name == name && it.descriptor.startsWith(partial) }
                .map { method ->
                    descriptorCompletion(resolvedOwner, name, method.descriptor, method.readableSignature, kind, targetNamespace, mappingContext, McCompletionKind.METHOD)
                }
            AccessWidenerKind.FIELD -> classIndex.getFields(resolvedOwner)
                .filter { it.name == name && it.descriptor.startsWith(partial) }
                .map { field ->
                    descriptorCompletion(resolvedOwner, name, field.descriptor, field.readableType, kind, targetNamespace, mappingContext, McCompletionKind.FIELD)
                }
            AccessWidenerKind.CLASS -> emptyList()
        }
    }

    private fun descriptorCompletion(
        resolvedOwner: String,
        name: String,
        descriptor: String,
        detail: String,
        kind: AccessWidenerKind,
        targetNamespace: MappingNamespace?,
        mappingContext: ProjectMappingContext?,
        completionKind: McCompletionKind,
    ): McCompletionItem {
        val insertDescriptor = AwNamespaceHelper.insertDescriptor(
            resolvedOwner,
            descriptor,
            kind,
            targetNamespace,
            mappingContext?.resolver,
        )
        return McCompletionItem(
            label = descriptor,
            detail = detail,
            documentation = null,
            filterText = descriptor,
            insertText = insertDescriptor,
            kind = completionKind,
            sortKey = "0500_$descriptor",
            metadata = McCompletionMetadata(
                source = "aw.descriptor",
                owner = resolvedOwner,
                name = name,
                descriptor = descriptor,
            ),
        )
    }
}
