package io.github.mcdev.jdtls.convert

import io.github.mcdev.core.definition.McDefinitionTarget
import io.github.mcdev.core.definition.McReferenceLocation
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.model.MemberKind
import io.github.mcdev.protocol.McdevLocation
import io.github.mcdev.protocol.McdevPosition
import io.github.mcdev.protocol.McdevRange

object DefinitionConverter {
    fun toLocation(target: McDefinitionTarget): McdevLocation =
        McdevLocation(
            documentUri = "",
            range = target.sourceRange?.toProtocolRange() ?: zeroRange(),
            metadata = buildMetadata(target),
        )

    fun toLocations(targets: List<McDefinitionTarget>): List<McdevLocation> =
        targets.map(::toLocation)

    fun referenceToLocation(reference: McReferenceLocation): McdevLocation =
        McdevLocation(
            documentUri = reference.documentUri,
            range = reference.range.toProtocolRange(),
            metadata = reference.metadata,
        )

    fun referencesToLocations(references: List<McReferenceLocation>): List<McdevLocation> =
        references.map(::referenceToLocation)

    private fun buildMetadata(target: McDefinitionTarget): Map<String, String> {
        val metadata = linkedMapOf(
            "kind" to target.kind.name.lowercase(),
            "owner" to target.ownerInternalName,
        )
        target.ownerFqn?.let { metadata["fqn"] = it }
        target.name?.let { metadata["name"] = it }
        target.descriptor?.let { metadata["descriptor"] = it }
        metadata["namespace"] = target.namespace.name
        return metadata
    }

    private fun McTextRange.toProtocolRange(): McdevRange =
        McdevRange(
            start = start.toProtocolPosition(),
            end = end.toProtocolPosition(),
        )

    private fun McTextPosition.toProtocolPosition(): McdevPosition =
        McdevPosition(line = line, character = character)

    private fun zeroRange(): McdevRange =
        McdevRange(
            start = McdevPosition(0, 0),
            end = McdevPosition(0, 0),
        )
}
