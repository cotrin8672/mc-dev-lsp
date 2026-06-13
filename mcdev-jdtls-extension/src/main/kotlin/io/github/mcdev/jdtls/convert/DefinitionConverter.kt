package io.github.mcdev.jdtls.convert

import io.github.mcdev.core.definition.McReferenceLocation
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.jdtls.definition.ResolvedDefinition
import io.github.mcdev.protocol.McdevDefinitionResolution
import io.github.mcdev.protocol.McdevLocation
import io.github.mcdev.protocol.McdevPosition
import io.github.mcdev.protocol.McdevRange

object DefinitionConverter {
    fun toLocation(resolved: ResolvedDefinition): McdevLocation {
        val target = resolved.target
        val documentUri = when (resolved.resolution) {
            McdevDefinitionResolution.SOURCE,
            McdevDefinitionResolution.JDT,
            -> resolved.documentUri
            else -> ""
        }
        val range = when (resolved.resolution) {
            McdevDefinitionResolution.SOURCE,
            McdevDefinitionResolution.JDT,
            -> resolved.range.toProtocolRange()
            else -> target.sourceRange?.toProtocolRange() ?: zeroRange()
        }
        return McdevLocation(
            documentUri = documentUri,
            range = range,
            metadata = buildMetadata(target),
            resolution = resolved.resolution,
            resolutionMessage = resolved.resolutionMessage,
        )
    }

    fun toLocations(resolved: List<ResolvedDefinition>): List<McdevLocation> =
        resolved.map(::toLocation)

    fun referenceToLocation(reference: McReferenceLocation): McdevLocation =
        McdevLocation(
            documentUri = reference.documentUri,
            range = reference.range.toProtocolRange(),
            metadata = reference.metadata,
            resolution = McdevDefinitionResolution.SOURCE,
        )

    fun referencesToLocations(references: List<McReferenceLocation>): List<McdevLocation> =
        references.map(::referenceToLocation)

    private fun buildMetadata(target: io.github.mcdev.core.definition.McDefinitionTarget): Map<String, String> {
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
