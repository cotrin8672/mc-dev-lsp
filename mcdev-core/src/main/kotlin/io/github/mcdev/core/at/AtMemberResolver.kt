package io.github.mcdev.core.at

import io.github.mcdev.core.mapping.FieldRef
import io.github.mcdev.core.mapping.MappingLookupResult
import io.github.mcdev.core.mapping.ProjectMappingContext
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.FieldIndexEntry
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.model.MemberKind
import io.github.mcdev.core.model.MappingNamespace

data class ResolvedAtMember(
    val kind: MemberKind,
    val namedName: String,
    val descriptor: String?,
    val method: MethodIndexEntry? = null,
    val field: FieldIndexEntry? = null,
)

sealed interface AtMemberResolution {
    data class Found(val member: ResolvedAtMember) : AtMemberResolution
    data object NotFound : AtMemberResolution
    data class WrongNamespace(
        val namedName: String,
        val expectedNamespace: MappingNamespace,
        val actualName: String,
    ) : AtMemberResolution
    data class MappingNotFound(
        val namedName: String,
        val kind: MemberKind,
    ) : AtMemberResolution
    data class DescriptorMismatch(
        val namedName: String,
        val descriptor: String,
        val candidates: List<MethodIndexEntry>,
    ) : AtMemberResolution
    data class MissingDescriptor(
        val namedName: String,
    ) : AtMemberResolution
}

class AtMemberResolver(
    private val insertFormatter: AtMemberInsertFormatter = AtMemberInsertFormatter(),
) {
    fun resolve(
        ownerInternalName: String,
        memberName: String,
        memberDescriptor: String?,
        classIndex: ClassIndex,
        mappingContext: ProjectMappingContext?,
    ): AtMemberResolution {
        val namedMethods = classIndex.getMethods(ownerInternalName).filter { it.name == memberName }
        if (namedMethods.isNotEmpty()) {
            return resolveNamedMethod(namedMethods, memberName, memberDescriptor, mappingContext, ownerInternalName)
        }
        val namedField = classIndex.getFields(ownerInternalName).find { it.name == memberName }
        if (namedField != null) {
            return resolveNamedField(namedField, memberName, mappingContext, ownerInternalName)
        }

        val mappings = mappingContext
        val targetNamespace = mappings?.atNamespace
        if (targetNamespace != null) {
            resolveByRemappedName(
                ownerInternalName = ownerInternalName,
                memberName = memberName,
                memberDescriptor = memberDescriptor,
                classIndex = classIndex,
                mappingContext = mappings,
                targetNamespace = targetNamespace,
            )?.let { return it }
        }
        return AtMemberResolution.NotFound
    }

    private fun resolveNamedMethod(
        methods: List<MethodIndexEntry>,
        memberName: String,
        memberDescriptor: String?,
        mappingContext: ProjectMappingContext?,
        ownerInternalName: String,
    ): AtMemberResolution {
        val method = selectNamedMethod(methods, memberDescriptor)
            ?: return if (memberDescriptor != null) {
                AtMemberResolution.DescriptorMismatch(memberName, memberDescriptor, methods)
            } else {
                AtMemberResolution.MissingDescriptor(memberName)
            }
        val targetNamespace = mappingContext?.atNamespace
        if (targetNamespace != null && targetNamespace != mappingContext.sourceNamespace) {
            val remapped = insertFormatter.remapMethodForEntry(ownerInternalName, method, mappingContext)
            if (!remapped.mappingFound) {
                return AtMemberResolution.MappingNotFound(namedName = memberName, kind = MemberKind.METHOD)
            }
            val remappedName = remapped.insertText.substringBefore('(')
            if (remappedName != memberName) {
                return AtMemberResolution.WrongNamespace(
                    namedName = memberName,
                    expectedNamespace = targetNamespace,
                    actualName = memberName,
                )
            }
        }
        if (memberDescriptor == null && method.descriptor.isNotEmpty()) {
            return AtMemberResolution.Found(
                ResolvedAtMember(
                    kind = MemberKind.METHOD,
                    namedName = memberName,
                    descriptor = null,
                    method = method,
                ),
            )
        }
        if (memberDescriptor != null && memberDescriptor != method.descriptor) {
            return AtMemberResolution.Found(
                ResolvedAtMember(
                    kind = MemberKind.METHOD,
                    namedName = memberName,
                    descriptor = memberDescriptor,
                    method = method,
                ),
            )
        }
        return AtMemberResolution.Found(
            ResolvedAtMember(
                kind = MemberKind.METHOD,
                namedName = memberName,
                descriptor = memberDescriptor ?: method.descriptor,
                method = method,
            ),
        )
    }

    private fun selectNamedMethod(
        methods: List<MethodIndexEntry>,
        memberDescriptor: String?,
    ): MethodIndexEntry? =
        when {
            memberDescriptor != null -> methods.find { it.descriptor == memberDescriptor }
            else -> methods.singleOrNull()
        }

    private fun resolveNamedField(
        field: FieldIndexEntry,
        memberName: String,
        mappingContext: ProjectMappingContext?,
        ownerInternalName: String,
    ): AtMemberResolution {
        val targetNamespace = mappingContext?.atNamespace
        if (targetNamespace != null && targetNamespace != mappingContext.sourceNamespace) {
            val remapped = insertFormatter.remapFieldForEntry(ownerInternalName, field, mappingContext)
            if (!remapped.mappingFound) {
                return AtMemberResolution.MappingNotFound(namedName = memberName, kind = MemberKind.FIELD)
            }
            if (remapped.insertText != memberName) {
                return AtMemberResolution.WrongNamespace(
                    namedName = memberName,
                    expectedNamespace = targetNamespace,
                    actualName = memberName,
                )
            }
        }
        return AtMemberResolution.Found(
            ResolvedAtMember(
                kind = MemberKind.FIELD,
                namedName = memberName,
                descriptor = field.descriptor,
                field = field,
            ),
        )
    }

    private fun resolveByRemappedName(
        ownerInternalName: String,
        memberName: String,
        memberDescriptor: String?,
        classIndex: ClassIndex,
        mappingContext: ProjectMappingContext,
        targetNamespace: MappingNamespace,
    ): AtMemberResolution? {
        for (method in classIndex.getMethods(ownerInternalName)) {
            val remapped = insertFormatter.remapMethodForEntry(ownerInternalName, method, mappingContext)
            val expected = remapped.insertText
            val expectedName = expected.substringBefore('(')
            val expectedDescriptor = expected.substringAfter('(', missingDelimiterValue = "").let {
                if (it.isEmpty()) null else "($it"
            }
            if (expectedName == memberName) {
                if (!remapped.mappingFound) {
                    return AtMemberResolution.MappingNotFound(namedName = method.name, kind = MemberKind.METHOD)
                }
                if (memberDescriptor != null && expectedDescriptor != null && memberDescriptor != expectedDescriptor) {
                    continue
                }
                return AtMemberResolution.Found(
                    ResolvedAtMember(
                        kind = MemberKind.METHOD,
                        namedName = method.name,
                        descriptor = memberDescriptor ?: method.descriptor,
                        method = method,
                    ),
                )
            }
        }
        for (field in classIndex.getFields(ownerInternalName)) {
            val remapped = insertFormatter.remapFieldForEntry(ownerInternalName, field, mappingContext)
            if (remapped.insertText == memberName) {
                if (!remapped.mappingFound) {
                    return AtMemberResolution.MappingNotFound(namedName = field.name, kind = MemberKind.FIELD)
                }
                return AtMemberResolution.Found(
                    ResolvedAtMember(
                        kind = MemberKind.FIELD,
                        namedName = field.name,
                        descriptor = field.descriptor,
                        field = field,
                    ),
                )
            }
        }
        return null
    }
}
