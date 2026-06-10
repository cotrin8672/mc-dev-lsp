package io.github.mcdev.core.mapping

import io.github.mcdev.core.model.MappingNamespace

data class ClassRef(
    val internalName: String,
    val namespace: MappingNamespace,
)

data class FieldRef(
    val owner: String,
    val name: String,
    val descriptor: String,
    val namespace: MappingNamespace,
)

data class MethodRef(
    val owner: String,
    val name: String,
    val descriptor: String,
    val namespace: MappingNamespace,
)

enum class MappingSubject {
    CLASS,
    FIELD,
    METHOD,
    DESCRIPTOR,
}

sealed interface MappingLookupResult<out T> {
    data class Found<T>(val value: T) : MappingLookupResult<T>

    data class Missing(
        val subject: MappingSubject,
        val owner: String? = null,
        val name: String? = null,
        val descriptor: String? = null,
        val from: MappingNamespace,
        val to: MappingNamespace,
    ) : MappingLookupResult<Nothing>

    data class InvalidDescriptor(
        val descriptor: String,
        val message: String,
        val offset: Int,
    ) : MappingLookupResult<Nothing>
}

interface MappingResolver {
    val namespaces: Set<MappingNamespace>

    fun remapClass(ref: ClassRef, to: MappingNamespace): MappingLookupResult<ClassRef>

    fun remapField(ref: FieldRef, to: MappingNamespace): MappingLookupResult<FieldRef>

    fun remapMethod(ref: MethodRef, to: MappingNamespace): MappingLookupResult<MethodRef>

    fun remapDescriptor(descriptor: String, from: MappingNamespace, to: MappingNamespace): MappingLookupResult<String>
}

data class ProjectMappingContext(
    val sourceNamespace: MappingNamespace,
    val runtimeNamespace: MappingNamespace,
    val awNamespace: MappingNamespace?,
    val atNamespace: MappingNamespace?,
    val availableNamespaces: Set<MappingNamespace>,
    val resolver: MappingResolver,
)
