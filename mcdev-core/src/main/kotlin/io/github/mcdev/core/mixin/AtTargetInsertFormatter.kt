package io.github.mcdev.core.mixin

import io.github.mcdev.core.descriptor.DescriptorParseResult
import io.github.mcdev.core.descriptor.DescriptorRenderer
import io.github.mcdev.core.descriptor.MemberTarget
import io.github.mcdev.core.descriptor.MemberTargetParser
import io.github.mcdev.core.mapping.FieldRef
import io.github.mcdev.core.mapping.MappingLookupResult
import io.github.mcdev.core.mapping.MappingResolver
import io.github.mcdev.core.mapping.MethodRef
import io.github.mcdev.core.model.MappingNamespace

class AtTargetInsertFormatter(
    private val targetFormatter: AtTargetCompletionService = AtTargetCompletionService(),
) {
    fun formatInsert(
        candidate: AtTargetCandidate,
        resolver: MappingResolver?,
        from: MappingNamespace = MappingNamespace.NAMED,
        to: MappingNamespace = MappingNamespace.INTERMEDIARY,
    ): String {
        val namedTarget = targetFormatter.formatTarget(candidate)
        if (resolver == null || from == to || candidate.kind == AtTargetKind.RETURN || candidate.kind == AtTargetKind.CONSTANT) {
            return namedTarget
        }
        return when (val parsed = MemberTargetParser.parse(namedTarget)) {
            is DescriptorParseResult.Success -> remapMemberTarget(parsed.value, resolver, from, to) ?: namedTarget
            is DescriptorParseResult.Failure -> namedTarget
        }
    }

    private fun remapMemberTarget(
        target: MemberTarget,
        resolver: MappingResolver,
        from: MappingNamespace,
        to: MappingNamespace,
    ): String? =
        when (target) {
            is MemberTarget.Method -> {
                when (
                    val remapped = resolver.remapMethod(
                        MethodRef(
                            owner = target.owner,
                            name = target.name,
                            descriptor = DescriptorRenderer.toDescriptor(target.descriptor),
                            namespace = from,
                        ),
                        to,
                    )
                ) {
                    is MappingLookupResult.Found ->
                        "L${remapped.value.owner};${remapped.value.name}${remapped.value.descriptor}"
                    else -> null
                }
            }
            is MemberTarget.Field -> {
                when (
                    val remapped = resolver.remapField(
                        FieldRef(
                            owner = target.owner,
                            name = target.name,
                            descriptor = DescriptorRenderer.toDescriptor(target.descriptor),
                            namespace = from,
                        ),
                        to,
                    )
                ) {
                    is MappingLookupResult.Found ->
                        "L${remapped.value.owner};${remapped.value.name}:${remapped.value.descriptor}"
                    else -> null
                }
            }
        }
}
