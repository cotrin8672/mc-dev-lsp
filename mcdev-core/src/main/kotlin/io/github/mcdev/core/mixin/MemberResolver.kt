package io.github.mcdev.core.mixin

enum class AmbiguityPolicy {
    EXACT_DESCRIPTOR_REQUIRED,
    SINGLE_CANDIDATE_IF_DESCRIPTOR_MISSING,
    ALLOW_AMBIGUOUS_FOR_COMPLETION,
}

sealed interface MethodResolution {
    data class Resolved(val method: MethodIndexEntry) : MethodResolution
    data class MissingDescriptor(val candidates: List<MethodIndexEntry>) : MethodResolution
    data class DescriptorMismatch(val candidates: List<MethodIndexEntry>) : MethodResolution
    data class Ambiguous(val candidates: List<MethodIndexEntry>) : MethodResolution
    data class UnresolvedOwner(val owner: String) : MethodResolution
    data object NotFound : MethodResolution
}

sealed interface FieldResolution {
    data class Resolved(val field: FieldIndexEntry) : FieldResolution
    data class MissingDescriptor(val candidates: List<FieldIndexEntry>) : FieldResolution
    data class DescriptorMismatch(val candidates: List<FieldIndexEntry>) : FieldResolution
    data class Ambiguous(val candidates: List<FieldIndexEntry>) : FieldResolution
    data class UnresolvedOwner(val owner: String) : FieldResolution
    data object NotFound : FieldResolution
}

interface MemberResolver {
    fun resolveMethod(
        owner: String,
        name: String,
        descriptor: String?,
        policy: AmbiguityPolicy,
    ): MethodResolution

    fun resolveField(
        owner: String,
        name: String,
        descriptor: String?,
        policy: AmbiguityPolicy,
    ): FieldResolution
}

class ClassIndexMemberResolver(
    private val classIndex: ClassIndex,
) : MemberResolver {
    override fun resolveMethod(
        owner: String,
        name: String,
        descriptor: String?,
        policy: AmbiguityPolicy,
    ): MethodResolution {
        if (classIndex.findClasses(owner.substringAfterLast('/'), limit = 1).isEmpty() &&
            classIndex.getMethods(owner).isEmpty() &&
            classIndex.getFields(owner).isEmpty()
        ) {
            return MethodResolution.UnresolvedOwner(owner)
        }
        val candidates = classIndex.getMethods(owner).filter { it.name == name }
        if (candidates.isEmpty()) return MethodResolution.NotFound
        if (descriptor != null) {
            return candidates.firstOrNull { it.descriptor == descriptor }?.let(MethodResolution::Resolved)
                ?: MethodResolution.DescriptorMismatch(candidates)
        }
        return when (policy) {
            AmbiguityPolicy.EXACT_DESCRIPTOR_REQUIRED -> MethodResolution.MissingDescriptor(candidates)
            AmbiguityPolicy.SINGLE_CANDIDATE_IF_DESCRIPTOR_MISSING ->
                candidates.singleOrNull()?.let(MethodResolution::Resolved) ?: MethodResolution.Ambiguous(candidates)
            AmbiguityPolicy.ALLOW_AMBIGUOUS_FOR_COMPLETION -> MethodResolution.Ambiguous(candidates)
        }
    }

    override fun resolveField(
        owner: String,
        name: String,
        descriptor: String?,
        policy: AmbiguityPolicy,
    ): FieldResolution {
        if (classIndex.findClasses(owner.substringAfterLast('/'), limit = 1).isEmpty() &&
            classIndex.getMethods(owner).isEmpty() &&
            classIndex.getFields(owner).isEmpty()
        ) {
            return FieldResolution.UnresolvedOwner(owner)
        }
        val candidates = classIndex.getFields(owner).filter { it.name == name }
        if (candidates.isEmpty()) return FieldResolution.NotFound
        if (descriptor != null) {
            return candidates.firstOrNull { it.descriptor == descriptor }?.let(FieldResolution::Resolved)
                ?: FieldResolution.DescriptorMismatch(candidates)
        }
        return when (policy) {
            AmbiguityPolicy.EXACT_DESCRIPTOR_REQUIRED -> FieldResolution.MissingDescriptor(candidates)
            AmbiguityPolicy.SINGLE_CANDIDATE_IF_DESCRIPTOR_MISSING ->
                candidates.singleOrNull()?.let(FieldResolution::Resolved) ?: FieldResolution.Ambiguous(candidates)
            AmbiguityPolicy.ALLOW_AMBIGUOUS_FOR_COMPLETION -> FieldResolution.Ambiguous(candidates)
        }
    }
}
