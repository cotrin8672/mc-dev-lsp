package io.github.mcdev.core.mapping

import io.github.mcdev.core.model.MappingNamespace

class CompositeMappingResolver(
    private val resolvers: List<MappingResolver>,
) : MappingResolver {
    init {
        require(resolvers.isNotEmpty()) { "composite resolver requires at least one delegate" }
    }

    override val namespaces: Set<MappingNamespace> = resolvers.flatMap { it.namespaces }.toSet()

    override fun remapClass(ref: ClassRef, to: MappingNamespace): MappingLookupResult<ClassRef> =
        firstFound { it.remapClass(ref, to) }

    override fun remapField(ref: FieldRef, to: MappingNamespace): MappingLookupResult<FieldRef> =
        firstFound { it.remapField(ref, to) }

    override fun remapMethod(ref: MethodRef, to: MappingNamespace): MappingLookupResult<MethodRef> =
        firstFound { it.remapMethod(ref, to) }

    override fun remapDescriptor(
        descriptor: String,
        from: MappingNamespace,
        to: MappingNamespace,
    ): MappingLookupResult<String> = firstFound { it.remapDescriptor(descriptor, from, to) }

    private inline fun <T> firstFound(
        action: (MappingResolver) -> MappingLookupResult<T>,
    ): MappingLookupResult<T> {
        var lastMissing: MappingLookupResult.Missing? = null
        for (resolver in resolvers) {
            when (val result = action(resolver)) {
                is MappingLookupResult.Found -> return result
                is MappingLookupResult.InvalidDescriptor -> return result
                is MappingLookupResult.Missing -> lastMissing = result
            }
        }
        return lastMissing ?: error("composite resolver delegates returned no lookup results")
    }
}

fun List<MappingSet>.asCompositeResolver(): MappingResolver = when (size) {
    0 -> throw IllegalArgumentException("cannot build resolver from empty mapping sets")
    1 -> first().asResolver()
    else -> CompositeMappingResolver(map { it.asResolver() })
}
