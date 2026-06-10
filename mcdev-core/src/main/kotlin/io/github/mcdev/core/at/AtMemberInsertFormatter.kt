package io.github.mcdev.core.at

import io.github.mcdev.core.mapping.FieldRef
import io.github.mcdev.core.mapping.MappingLookupResult
import io.github.mcdev.core.mapping.MappingResolver
import io.github.mcdev.core.mapping.MethodRef
import io.github.mcdev.core.mapping.ProjectMappingContext
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FieldIndexEntry
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.model.MappingNamespace

data class AtInsertResult(
    val insertText: String,
    val mappingFound: Boolean,
)

class AtMemberInsertFormatter {
    fun formatOwnerInsert(classEntry: ClassIndexEntry): String = classEntry.fqn

    fun formatMethodInsert(
        method: MethodIndexEntry,
        ownerInternalName: String,
        mappingContext: ProjectMappingContext?,
    ): AtInsertResult {
        val targetNamespace = mappingContext?.atNamespace
        if (targetNamespace == null || mappingContext.sourceNamespace == targetNamespace) {
            return AtInsertResult("${method.name}${method.descriptor}", mappingFound = true)
        }
        return remapMethodInsert(
            ownerInternalName = ownerInternalName,
            method = method,
            resolver = mappingContext.resolver,
            sourceNamespace = mappingContext.sourceNamespace,
            targetNamespace = targetNamespace,
        )
    }

    fun formatFieldInsert(
        field: FieldIndexEntry,
        ownerInternalName: String,
        mappingContext: ProjectMappingContext?,
    ): AtInsertResult {
        val targetNamespace = mappingContext?.atNamespace
        if (targetNamespace == null || mappingContext.sourceNamespace == targetNamespace) {
            return AtInsertResult(field.name, mappingFound = true)
        }
        return remapFieldInsert(
            ownerInternalName = ownerInternalName,
            field = field,
            resolver = mappingContext.resolver,
            sourceNamespace = mappingContext.sourceNamespace,
            targetNamespace = targetNamespace,
        )
    }

    fun remapMethodForEntry(
        ownerInternalName: String,
        method: MethodIndexEntry,
        mappingContext: ProjectMappingContext,
    ): AtInsertResult = remapMethodInsert(
        ownerInternalName = ownerInternalName,
        method = method,
        resolver = mappingContext.resolver,
        sourceNamespace = mappingContext.sourceNamespace,
        targetNamespace = mappingContext.atNamespace ?: mappingContext.sourceNamespace,
    )

    fun remapFieldForEntry(
        ownerInternalName: String,
        field: FieldIndexEntry,
        mappingContext: ProjectMappingContext,
    ): AtInsertResult = remapFieldInsert(
        ownerInternalName = ownerInternalName,
        field = field,
        resolver = mappingContext.resolver,
        sourceNamespace = mappingContext.sourceNamespace,
        targetNamespace = mappingContext.atNamespace ?: mappingContext.sourceNamespace,
    )

    private fun remapMethodInsert(
        ownerInternalName: String,
        method: MethodIndexEntry,
        resolver: MappingResolver,
        sourceNamespace: MappingNamespace,
        targetNamespace: MappingNamespace,
    ): AtInsertResult {
        val remapped = resolver.remapMethod(
            MethodRef(
                owner = ownerInternalName,
                name = method.name,
                descriptor = method.descriptor,
                namespace = sourceNamespace,
            ),
            targetNamespace,
        )
        return when (remapped) {
            is MappingLookupResult.Found ->
                AtInsertResult("${remapped.value.name}${remapped.value.descriptor}", mappingFound = true)
            else -> AtInsertResult("${method.name}${method.descriptor}", mappingFound = false)
        }
    }

    private fun remapFieldInsert(
        ownerInternalName: String,
        field: FieldIndexEntry,
        resolver: MappingResolver,
        sourceNamespace: MappingNamespace,
        targetNamespace: MappingNamespace,
    ): AtInsertResult {
        val remapped = resolver.remapField(
            FieldRef(
                owner = ownerInternalName,
                name = field.name,
                descriptor = field.descriptor,
                namespace = sourceNamespace,
            ),
            targetNamespace,
        )
        return when (remapped) {
            is MappingLookupResult.Found -> AtInsertResult(remapped.value.name, mappingFound = true)
            else -> AtInsertResult(field.name, mappingFound = false)
        }
    }
}
