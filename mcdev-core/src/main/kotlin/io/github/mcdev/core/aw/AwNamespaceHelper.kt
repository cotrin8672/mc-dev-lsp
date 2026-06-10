package io.github.mcdev.core.aw

import io.github.mcdev.core.mapping.ClassRef
import io.github.mcdev.core.mapping.FieldRef
import io.github.mcdev.core.mapping.MappingLookupResult
import io.github.mcdev.core.mapping.MappingResolver
import io.github.mcdev.core.mapping.MethodRef
import io.github.mcdev.core.mapping.ProjectMappingContext
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.model.MappingNamespace

internal object AwNamespaceHelper {
    fun resolveOwnerInIndex(
        owner: String,
        fileNamespace: MappingNamespace?,
        classIndex: ClassIndex,
        mappingContext: ProjectMappingContext?,
    ): String? {
        if (classIndex.findClass(owner) != null) return owner
        val resolver = mappingContext?.resolver ?: return null
        val namespaces = buildList {
            fileNamespace?.let { add(it) }
            add(MappingNamespace.NAMED)
            resolver.namespaces.forEach { add(it) }
        }.distinct()
        for (namespace in namespaces) {
            val namedOwner = remapOwnerTo(owner, namespace, MappingNamespace.NAMED, resolver) ?: continue
            if (classIndex.findClass(namedOwner) != null) return namedOwner
        }
        return null
    }

    fun hasNamespaceMismatch(
        owner: String,
        fileNamespace: MappingNamespace?,
        classIndex: ClassIndex,
        mappingContext: ProjectMappingContext?,
    ): Boolean {
        if (fileNamespace == null) return false
        if (classIndex.findClass(owner) != null) return false
        val resolver = mappingContext?.resolver ?: return false
        for (namespace in resolver.namespaces) {
            if (namespace == fileNamespace) continue
            val namedOwner = remapOwnerTo(owner, namespace, MappingNamespace.NAMED, resolver) ?: continue
            if (classIndex.findClass(namedOwner) != null) return true
        }
        return false
    }

    fun insertOwner(
        internalName: String,
        targetNamespace: MappingNamespace?,
        resolver: MappingResolver?,
    ): String {
        if (targetNamespace == null || targetNamespace == MappingNamespace.NAMED || resolver == null) {
            return internalName
        }
        return when (val result = resolver.remapClass(ClassRef(internalName, MappingNamespace.NAMED), targetNamespace)) {
            is MappingLookupResult.Found -> result.value.internalName
            else -> internalName
        }
    }

    fun insertMemberName(
        owner: String,
        name: String,
        descriptor: String,
        kind: AccessWidenerKind,
        targetNamespace: MappingNamespace?,
        resolver: MappingResolver?,
    ): String {
        if (targetNamespace == null || targetNamespace == MappingNamespace.NAMED || resolver == null) {
            return name
        }
        return when (kind) {
            AccessWidenerKind.METHOD -> when (
                val result = resolver.remapMethod(
                    MethodRef(owner, name, descriptor, MappingNamespace.NAMED),
                    targetNamespace,
                )
            ) {
                is MappingLookupResult.Found -> result.value.name
                else -> name
            }
            AccessWidenerKind.FIELD -> when (
                val result = resolver.remapField(
                    FieldRef(owner, name, descriptor, MappingNamespace.NAMED),
                    targetNamespace,
                )
            ) {
                is MappingLookupResult.Found -> result.value.name
                else -> name
            }
            AccessWidenerKind.CLASS -> name
        }
    }

    fun insertDescriptor(
        owner: String,
        descriptor: String,
        kind: AccessWidenerKind,
        targetNamespace: MappingNamespace?,
        resolver: MappingResolver?,
    ): String {
        if (targetNamespace == null || targetNamespace == MappingNamespace.NAMED || resolver == null) {
            return descriptor
        }
        return when (val result = resolver.remapDescriptor(descriptor, MappingNamespace.NAMED, targetNamespace)) {
            is MappingLookupResult.Found -> result.value
            else -> descriptor
        }
    }

    private fun remapOwnerTo(
        owner: String,
        from: MappingNamespace,
        to: MappingNamespace,
        resolver: MappingResolver,
    ): String? = when (val result = resolver.remapClass(ClassRef(owner, from), to)) {
        is MappingLookupResult.Found -> result.value.internalName
        else -> null
    }
}
