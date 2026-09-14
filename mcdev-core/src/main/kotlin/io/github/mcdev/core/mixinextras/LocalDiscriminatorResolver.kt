package io.github.mcdev.core.mixinextras

data class LocalCaptureCandidate(
    val slotIndex: Int,
    val descriptor: String,
    val name: String? = null,
    val isArgument: Boolean = false,
    /**
     * Mixin's ordinal for this descriptor in the complete local frame, including the receiver.
     * Hand-built candidates may leave this unset and retain the historical slot-order fallback.
     */
    val ordinal: Int? = null,
)

sealed interface LocalDiscriminatorResolution {
    data class Resolved(val candidate: LocalCaptureCandidate) : LocalDiscriminatorResolution

    data object NotFound : LocalDiscriminatorResolution

    data class Ambiguous(val candidates: List<LocalCaptureCandidate>) : LocalDiscriminatorResolution
}

object LocalDiscriminatorResolver {
    fun resolve(
        spec: HandlerParameterSugarSpec.Local,
        targetDescriptor: String,
        candidates: List<LocalCaptureCandidate>,
    ): LocalDiscriminatorResolution {
        val filtered = filterCandidates(spec, targetDescriptor, candidates)
        return when {
            isOrdinalSpecified(spec) -> resolveByOrdinal(spec.ordinal!!, filtered)
            isIndexSpecified(spec) -> resolveByIndex(spec.index!!, filtered)
            areNamesSpecified(spec) -> resolveByNames(spec.names, filtered)
            else -> resolveImplicit(filtered)
        }
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

    private fun isOrdinalSpecified(spec: HandlerParameterSugarSpec.Local): Boolean =
        spec.ordinal != null && spec.ordinal >= 0

    private fun isIndexSpecified(spec: HandlerParameterSugarSpec.Local): Boolean =
        spec.index != null && spec.index >= 0

    private fun areNamesSpecified(spec: HandlerParameterSugarSpec.Local): Boolean =
        spec.names.isNotEmpty()

    private fun resolveByOrdinal(
        ordinal: Int,
        filtered: List<LocalCaptureCandidate>,
    ): LocalDiscriminatorResolution {
        val candidate = filtered.firstOrNull { it.ordinal == ordinal }
            ?: if (filtered.all { it.ordinal == null }) filtered.getOrNull(ordinal) else null
        return if (candidate != null) {
            LocalDiscriminatorResolution.Resolved(candidate)
        } else {
            LocalDiscriminatorResolution.NotFound
        }
    }

    private fun resolveByIndex(
        index: Int,
        filtered: List<LocalCaptureCandidate>,
    ): LocalDiscriminatorResolution {
        val matches = filtered.filter { it.slotIndex == index }
        return when (matches.size) {
            0 -> LocalDiscriminatorResolution.NotFound
            1 -> LocalDiscriminatorResolution.Resolved(matches.single())
            else -> LocalDiscriminatorResolution.Ambiguous(matches)
        }
    }

    /** MixinExtras runtime picks the first name match in local-variable slot order. */
    private fun resolveByNames(
        names: Set<String>,
        filtered: List<LocalCaptureCandidate>,
    ): LocalDiscriminatorResolution =
        filtered.firstOrNull { candidate ->
            candidate.name != null && candidate.name in names
        }?.let(LocalDiscriminatorResolution::Resolved)
            ?: LocalDiscriminatorResolution.NotFound

    private fun resolveImplicit(
        filtered: List<LocalCaptureCandidate>,
    ): LocalDiscriminatorResolution =
        when (filtered.size) {
            0 -> LocalDiscriminatorResolution.NotFound
            1 -> LocalDiscriminatorResolution.Resolved(filtered.single())
            else -> LocalDiscriminatorResolution.Ambiguous(filtered)
        }
}
