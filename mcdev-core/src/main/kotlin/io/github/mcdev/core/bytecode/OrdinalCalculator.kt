package io.github.mcdev.core.bytecode

object OrdinalCalculator {
    fun assignOrdinals(candidates: List<AtTargetCandidate>): List<AtTargetCandidate> {
        val counters = mutableMapOf<OrdinalKey, Int>()
        return candidates.map { candidate ->
            val key = OrdinalKey.from(candidate)
            val ordinal = counters.getOrDefault(key, 0)
            counters[key] = ordinal + 1
            candidate.copy(ordinal = ordinal)
        }
    }

    internal data class OrdinalKey(
        val kind: AtTargetKind,
        val owner: String,
        val name: String,
        val descriptor: String,
        val constantTag: String?,
    ) {
        companion object {
            fun from(candidate: AtTargetCandidate): OrdinalKey =
                OrdinalKey(
                    kind = candidate.kind,
                    owner = candidate.owner,
                    name = candidate.name,
                    descriptor = candidate.descriptor,
                    constantTag = candidate.constantValue?.let { constantTag(it) },
                )

            private fun constantTag(value: ConstantValue): String =
                when (value) {
                    is ConstantValue.StringValue -> "s:${value.value}"
                    is ConstantValue.IntValue -> "i:${value.value}"
                    is ConstantValue.LongValue -> "l:${value.value}"
                    is ConstantValue.FloatValue -> "f:${value.value}"
                    is ConstantValue.DoubleValue -> "d:${value.value}"
                    is ConstantValue.ClassLiteral -> "c:${value.internalName}"
                    ConstantValue.NullValue -> "null"
                }
        }
    }
}
