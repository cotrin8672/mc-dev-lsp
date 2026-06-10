package io.github.mcdev.core.project

import io.github.mcdev.core.mapping.ClassRef
import io.github.mcdev.core.mapping.FieldRef
import io.github.mcdev.core.mapping.MappingLookupResult
import io.github.mcdev.core.mapping.MappingResolver
import io.github.mcdev.core.mapping.MappingSubject
import io.github.mcdev.core.mapping.MethodRef
import io.github.mcdev.core.model.MappingNamespace

object EmptyMappingResolver : MappingResolver {
    override val namespaces: Set<MappingNamespace> = emptySet()

    override fun remapClass(ref: ClassRef, to: MappingNamespace): MappingLookupResult<ClassRef> =
        MappingLookupResult.Missing(MappingSubject.CLASS, owner = ref.internalName, from = ref.namespace, to = to)

    override fun remapField(ref: FieldRef, to: MappingNamespace): MappingLookupResult<FieldRef> =
        MappingLookupResult.Missing(
            MappingSubject.FIELD,
            owner = ref.owner,
            name = ref.name,
            descriptor = ref.descriptor,
            from = ref.namespace,
            to = to,
        )

    override fun remapMethod(ref: MethodRef, to: MappingNamespace): MappingLookupResult<MethodRef> =
        MappingLookupResult.Missing(
            MappingSubject.METHOD,
            owner = ref.owner,
            name = ref.name,
            descriptor = ref.descriptor,
            from = ref.namespace,
            to = to,
        )

    override fun remapDescriptor(
        descriptor: String,
        from: MappingNamespace,
        to: MappingNamespace,
    ): MappingLookupResult<String> =
        MappingLookupResult.Missing(MappingSubject.DESCRIPTOR, from = from, to = to)
}
