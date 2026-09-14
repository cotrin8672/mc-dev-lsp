package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata
import io.github.mcdev.core.mixin.ClassIndex

class LocalCompletionService(
    classIndex: ClassIndex,
) {
    private val typeResolver = LocalCaptureTypeResolver(classIndex)

    fun complete(
        source: String,
        parameter: HandlerParameterDeclaration,
        results: List<LocalCaptureValidationService.Result>,
        attributeName: String,
        partialPrefix: String = "",
    ): List<McCompletionItem> {
        if (results.isEmpty()) return emptyList()
        if (results.any { it is LocalCaptureValidationService.Result.Unavailable }) return emptyList()
        if (results.any { it.snapshots.isEmpty() }) return emptyList()

        val spec = parameter.sugarSpec as? HandlerParameterSugarSpec.Local ?: return emptyList()
        val targetDescriptor = typeResolver.resolveParameter(source, parameter) ?: return emptyList()

        val snapshots = results.flatMap(LocalCaptureValidationService.Result::snapshots)
        val firstFiltered = filterCandidates(spec, targetDescriptor, snapshots.first().candidates)

        return when (attributeName) {
            "ordinal" -> completeOrdinals(spec, targetDescriptor, snapshots, firstFiltered, partialPrefix)
            "index" -> completeIndices(spec, targetDescriptor, snapshots, firstFiltered, partialPrefix)
            "name" -> completeNames(spec, targetDescriptor, snapshots, firstFiltered, partialPrefix)
            else -> emptyList()
        }
    }

    private fun completeOrdinals(
        spec: HandlerParameterSugarSpec.Local,
        targetDescriptor: String,
        snapshots: List<LocalCaptureSnapshot>,
        firstFiltered: List<LocalCaptureCandidate>,
        partialPrefix: String,
    ): List<McCompletionItem> =
        (0 until firstFiltered.size)
            .asSequence()
            .filter { ordinal ->
                resolvesUniquelyInAllSnapshots(
                    spec = spec.copy(ordinal = ordinal, index = null, names = emptySet()),
                    targetDescriptor = targetDescriptor,
                    snapshots = snapshots,
                )
            }
            .map(Int::toString)
            .filter { it.startsWith(partialPrefix) }
            .sortedBy { it.toInt() }
            .mapIndexed { index, value ->
                valueItem(
                    attributeName = "ordinal",
                    value = value,
                    sortKey = "0200_${index.toString().padStart(4, '0')}_$value",
                )
            }
            .toList()

    private fun completeIndices(
        spec: HandlerParameterSugarSpec.Local,
        targetDescriptor: String,
        snapshots: List<LocalCaptureSnapshot>,
        firstFiltered: List<LocalCaptureCandidate>,
        partialPrefix: String,
    ): List<McCompletionItem> =
        firstFiltered
            .map(LocalCaptureCandidate::slotIndex)
            .distinct()
            .asSequence()
            .filter { slotIndex ->
                resolvesUniquelyInAllSnapshots(
                    spec = spec.copy(index = slotIndex, ordinal = null, names = emptySet()),
                    targetDescriptor = targetDescriptor,
                    snapshots = snapshots,
                )
            }
            .map(Int::toString)
            .filter { it.startsWith(partialPrefix) }
            .sortedBy { it.toInt() }
            .mapIndexed { index, value ->
                valueItem(
                    attributeName = "index",
                    value = value,
                    sortKey = "0200_${index.toString().padStart(4, '0')}_$value",
                )
            }
            .toList()

    private fun completeNames(
        spec: HandlerParameterSugarSpec.Local,
        targetDescriptor: String,
        snapshots: List<LocalCaptureSnapshot>,
        firstFiltered: List<LocalCaptureCandidate>,
        partialPrefix: String,
    ): List<McCompletionItem> =
        firstFiltered
            .mapNotNull(LocalCaptureCandidate::name)
            .distinct()
            .asSequence()
            .filter { name ->
                resolvesUniquelyInAllSnapshots(
                    spec = spec.copy(names = setOf(name), ordinal = null, index = null),
                    targetDescriptor = targetDescriptor,
                    snapshots = snapshots,
                )
            }
            .filter { it.startsWith(partialPrefix, ignoreCase = true) }
            .sortedBy { it.lowercase() }
            .mapIndexed { index, value ->
                valueItem(
                    attributeName = "name",
                    value = value,
                    sortKey = "0100_${index.toString().padStart(4, '0')}_$value",
                )
            }
            .toList()

    private fun resolvesUniquelyInAllSnapshots(
        spec: HandlerParameterSugarSpec.Local,
        targetDescriptor: String,
        snapshots: List<LocalCaptureSnapshot>,
    ): Boolean =
        snapshots.all { snapshot ->
            LocalDiscriminatorResolver.resolve(spec, targetDescriptor, snapshot.candidates) is
                LocalDiscriminatorResolution.Resolved
        }

    private fun filterCandidates(
        spec: HandlerParameterSugarSpec.Local,
        targetDescriptor: String,
        candidates: List<LocalCaptureCandidate>,
    ): List<LocalCaptureCandidate> =
        candidates
            .asSequence()
            .filter { it.descriptor == targetDescriptor }
            .filter { !spec.argsOnly || it.isArgument }
            .sortedBy { it.slotIndex }
            .toList()

    private fun valueItem(
        attributeName: String,
        value: String,
        sortKey: String,
    ): McCompletionItem =
        McCompletionItem(
            label = value,
            detail = "@Local $attributeName",
            documentation = null,
            filterText = value,
            insertText = value,
            kind = McCompletionKind.VALUE,
            sortKey = sortKey,
            metadata = McCompletionMetadata(source = "mixinextras.local", name = attributeName),
        )
}
