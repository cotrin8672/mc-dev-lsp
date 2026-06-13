package io.github.mcdev.core.at

import io.github.mcdev.core.definition.McDefinitionTarget
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mapping.ProjectMappingContext
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.model.MemberKind

class AccessTransformerDefinitionService(
    private val classIndex: ClassIndex,
    private val mappingContext: ProjectMappingContext? = null,
    private val memberResolver: AtMemberResolver = AtMemberResolver(),
) {
    fun definitionsAt(source: String, line: Int, character: Int): List<McDefinitionTarget> {
        val context = AtContextExtractor.extract(source, line, character) ?: return emptyList()
        return definitionsFromContext(source, context)
    }

    fun definitionsAtOffset(source: String, offset: Int): List<McDefinitionTarget> {
        val context = AtContextExtractor.extractAtOffset(source, offset) ?: return emptyList()
        return definitionsFromContext(source, context)
    }

    private fun definitionsFromContext(source: String, context: AtContext): List<McDefinitionTarget> {
        val range = offsetRange(source, context)
        return when (context.slot) {
            AtSlot.OWNER -> resolveOwnerDefinition(context, range)
            AtSlot.MEMBER_NAME, AtSlot.MEMBER_DESCRIPTOR -> resolveMemberDefinition(context, range)
            else -> emptyList()
        }
    }

    private fun resolveOwnerDefinition(
        context: AtContext,
        range: McTextRange,
    ): List<McDefinitionTarget> {
        val ownerFqn = context.owner?.trim().orEmpty()
        if (ownerFqn.isEmpty()) return emptyList()
        val ownerEntry = classIndex.findClassByFqn(ownerFqn)
            ?: classIndex.findClass(ownerFqn.replace('.', '/'))
            ?: return emptyList()
        return listOf(
            McDefinitionTarget(
                kind = MemberKind.CLASS,
                ownerInternalName = ownerEntry.internalName,
                ownerFqn = ownerEntry.fqn,
                sourceRange = range,
            ),
        )
    }

    private fun resolveMemberDefinition(
        context: AtContext,
        range: McTextRange,
    ): List<McDefinitionTarget> {
        val ownerFqn = context.owner?.trim().orEmpty()
        val memberName = context.memberName?.trim().orEmpty()
        if (ownerFqn.isEmpty() || memberName.isEmpty()) return emptyList()
        val ownerEntry = classIndex.findClassByFqn(ownerFqn)
            ?: classIndex.findClass(ownerFqn.replace('.', '/'))
            ?: return emptyList()
        val descriptor = context.memberDescriptor?.trim().orEmpty().ifEmpty { null }
        return when (
            val resolution = memberResolver.resolve(
                ownerInternalName = ownerEntry.internalName,
                memberName = memberName,
                memberDescriptor = descriptor,
                classIndex = classIndex,
                mappingContext = mappingContext,
            )
        ) {
            is AtMemberResolution.Found -> listOf(
                McDefinitionTarget(
                    kind = resolution.member.kind,
                    ownerInternalName = ownerEntry.internalName,
                    ownerFqn = ownerEntry.fqn,
                    name = resolution.member.namedName,
                    descriptor = resolution.member.descriptor,
                    sourceRange = range,
                ),
            )
            is AtMemberResolution.WrongNamespace -> resolveNamedMemberFromIndex(
                ownerEntry = ownerEntry,
                memberName = resolution.namedName,
                descriptor = descriptor,
                range = range,
            )
            else -> resolveNamedMemberFromIndex(
                ownerEntry = ownerEntry,
                memberName = memberName,
                descriptor = descriptor,
                range = range,
            )
        }
    }

    private fun resolveNamedMemberFromIndex(
        ownerEntry: io.github.mcdev.core.mixin.ClassIndexEntry,
        memberName: String,
        descriptor: String?,
        range: McTextRange,
    ): List<McDefinitionTarget> {
        classIndex.getMethods(ownerEntry.internalName)
            .filter { it.name == memberName }
            .let { methods ->
                val method = when {
                    descriptor != null -> methods.find { it.descriptor == descriptor } ?: methods.firstOrNull()
                    else -> methods.singleOrNull() ?: methods.firstOrNull()
                }
                if (method != null) {
                    return listOf(
                        McDefinitionTarget(
                            kind = MemberKind.METHOD,
                            ownerInternalName = ownerEntry.internalName,
                            ownerFqn = ownerEntry.fqn,
                            name = method.name,
                            descriptor = method.descriptor,
                            sourceRange = range,
                        ),
                    )
                }
            }
        classIndex.getFields(ownerEntry.internalName)
            .filter { it.name == memberName }
            .let { fields ->
                val field = when {
                    descriptor != null -> fields.find { it.descriptor == descriptor } ?: fields.firstOrNull()
                    else -> fields.singleOrNull() ?: fields.firstOrNull()
                }
                if (field != null) {
                    return listOf(
                        McDefinitionTarget(
                            kind = MemberKind.FIELD,
                            ownerInternalName = ownerEntry.internalName,
                            ownerFqn = ownerEntry.fqn,
                            name = field.name,
                            descriptor = field.descriptor,
                            sourceRange = range,
                        ),
                    )
                }
            }
        return emptyList()
    }

    private fun offsetRange(source: String, context: AtContext): McTextRange {
        val start = AtTextPositions.offsetToPosition(source, context.valueStartOffset)
        val end = AtTextPositions.offsetToPosition(source, context.valueEndOffset.coerceAtLeast(context.valueStartOffset))
        return McTextRange(start = start, end = end)
    }
}
