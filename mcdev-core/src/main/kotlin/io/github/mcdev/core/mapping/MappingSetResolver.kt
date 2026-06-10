package io.github.mcdev.core.mapping

import io.github.mcdev.core.model.MappingNamespace

class MappingSetResolver(private val mappings: MappingSet) : MappingResolver {
    override val namespaces: Set<MappingNamespace> = mappings.namespaces.toSet()

    override fun remapClass(ref: ClassRef, to: MappingNamespace): MappingLookupResult<ClassRef> {
        if (ref.namespace == to) return MappingLookupResult.Found(ref.copy(namespace = to))
        val name = mappings.className(ref.internalName, ref.namespace, to)
            ?: return MappingLookupResult.Missing(
                MappingSubject.CLASS,
                owner = ref.internalName,
                from = ref.namespace,
                to = to,
            )
        return MappingLookupResult.Found(ClassRef(name, to))
    }

    override fun remapField(ref: FieldRef, to: MappingNamespace): MappingLookupResult<FieldRef> {
        if (ref.namespace == to) return MappingLookupResult.Found(ref.copy(namespace = to))
        val owner = when (val result = remapClass(ClassRef(ref.owner, ref.namespace), to)) {
            is MappingLookupResult.Found -> result.value.internalName
            is MappingLookupResult.Missing -> return result
            is MappingLookupResult.InvalidDescriptor -> return result
        }
        val descriptor = when (val result = remapDescriptor(ref.descriptor, ref.namespace, to)) {
            is MappingLookupResult.Found -> result.value
            is MappingLookupResult.Missing -> ref.descriptor
            is MappingLookupResult.InvalidDescriptor -> return result
        }
        val name = mappings.fieldName(ref.owner, ref.name, ref.descriptor, ref.namespace, to)
            ?: return MappingLookupResult.Missing(
                MappingSubject.FIELD,
                owner = ref.owner,
                name = ref.name,
                descriptor = ref.descriptor,
                from = ref.namespace,
                to = to,
            )
        return MappingLookupResult.Found(FieldRef(owner, name, descriptor, to))
    }

    override fun remapMethod(ref: MethodRef, to: MappingNamespace): MappingLookupResult<MethodRef> {
        if (ref.namespace == to) return MappingLookupResult.Found(ref.copy(namespace = to))
        val owner = when (val result = remapClass(ClassRef(ref.owner, ref.namespace), to)) {
            is MappingLookupResult.Found -> result.value.internalName
            is MappingLookupResult.Missing -> return result
            is MappingLookupResult.InvalidDescriptor -> return result
        }
        val descriptor = when (val result = remapDescriptor(ref.descriptor, ref.namespace, to)) {
            is MappingLookupResult.Found -> result.value
            is MappingLookupResult.Missing -> ref.descriptor
            is MappingLookupResult.InvalidDescriptor -> return result
        }
        val name = mappings.methodName(ref.owner, ref.name, ref.descriptor, ref.namespace, to)
            ?: return MappingLookupResult.Missing(
                MappingSubject.METHOD,
                owner = ref.owner,
                name = ref.name,
                descriptor = ref.descriptor,
                from = ref.namespace,
                to = to,
            )
        return MappingLookupResult.Found(MethodRef(owner, name, descriptor, to))
    }

    override fun remapDescriptor(
        descriptor: String,
        from: MappingNamespace,
        to: MappingNamespace,
    ): MappingLookupResult<String> {
        if (from == to) return MappingLookupResult.Found(descriptor)
        return if (descriptor.startsWith("(")) {
            DescriptorRemapper.remapMethodDescriptor(descriptor, from, to, this)
        } else {
            DescriptorRemapper.remapFieldDescriptor(descriptor, from, to, this)
        }
    }
}

fun MappingSet.asResolver(): MappingResolver = MappingSetResolver(this)
