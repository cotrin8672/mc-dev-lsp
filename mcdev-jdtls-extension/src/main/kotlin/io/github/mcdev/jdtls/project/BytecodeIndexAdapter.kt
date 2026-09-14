package io.github.mcdev.jdtls.project

import io.github.mcdev.core.bytecode.AtTargetCandidate as BytecodeAtTargetCandidate
import io.github.mcdev.core.bytecode.AtTargetKind as BytecodeAtTargetKind
import io.github.mcdev.core.bytecode.ConstantValue
import io.github.mcdev.core.bytecode.InstructionExtractor
import io.github.mcdev.core.mixin.AnnotationContextExtractor
import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.AtTargetKind
import io.github.mcdev.core.mixin.AtTargetOperationKind
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixinextras.BytecodeCommonSuperClassResolver
import io.github.mcdev.core.model.MappingNamespace
import java.util.Collections
import java.util.LinkedHashMap

class BytecodeIndexAdapter(
    private val provider: ClasspathClassBytesProvider,
    private val classIndex: ClassIndex,
) : BytecodeIndex {
    private val candidateCache = Collections.synchronizedMap(
        object : LinkedHashMap<String, List<AtTargetCandidate>>(MAX_CANDIDATE_CACHE_ENTRIES, 0.75f, true) {
            override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, List<AtTargetCandidate>>?): Boolean =
                size > MAX_CANDIDATE_CACHE_ENTRIES
        },
    )
    private val returnCountCache = Collections.synchronizedMap(
        object : LinkedHashMap<String, Int>(MAX_CANDIDATE_CACHE_ENTRIES, 0.75f, true) {
            override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, Int>?): Boolean =
                size > MAX_CANDIDATE_CACHE_ENTRIES
        },
    )
    private val commonSuperClassResolver by lazy {
        BytecodeCommonSuperClassResolver(classBytesLookup = provider::getClassBytes)
    }

    override fun getAtTargetCandidates(
        ownerInternalName: String,
        methodName: String,
        methodDescriptor: String?,
        atValue: String,
    ): List<AtTargetCandidate> {
        val cacheKey = "$ownerInternalName#$methodName#${methodDescriptor.orEmpty()}#$atValue"
        synchronized(candidateCache) {
            candidateCache[cacheKey]?.let { return it }
        }
        val computed = resolveMethodDescriptors(ownerInternalName, methodName, methodDescriptor)
            .flatMap { descriptor -> extractCandidates(ownerInternalName, methodName, descriptor) }
            .filter { matchesAtValue(it, atValue) }
            .map { convertCandidate(it) }
            .distinctBy(::candidateKey)
            .sortedWith(candidateComparator)
        synchronized(candidateCache) {
            return candidateCache.getOrPut(cacheKey) { computed }
        }
    }

    override fun getClassBytes(ownerInternalName: String): ByteArray? =
        provider.getClassBytes(ownerInternalName)

    override fun resolveCommonSuperClass(type1Descriptor: String, type2Descriptor: String): String? =
        commonSuperClassResolver.resolve(type1Descriptor, type2Descriptor)

    override fun getReturnOrdinalCount(
        ownerInternalName: String,
        methodName: String,
        methodDescriptor: String?,
    ): Int {
        val cacheKey = "$ownerInternalName#$methodName#${methodDescriptor.orEmpty()}"
        return synchronized(returnCountCache) {
            returnCountCache.getOrPut(cacheKey) {
                val descriptor = methodDescriptor ?: resolveSingleMethodDescriptor(ownerInternalName, methodName) ?: "()V"
                extractCandidates(ownerInternalName, methodName, descriptor)
                    .count { it.kind == BytecodeAtTargetKind.RETURN }
                    .coerceAtLeast(1)
            }
        }
    }

    fun clearCache() {
        synchronized(candidateCache) {
            candidateCache.clear()
        }
        synchronized(returnCountCache) {
            returnCountCache.clear()
        }
    }

    private fun extractCandidates(
        ownerInternalName: String,
        methodName: String,
        methodDescriptor: String,
    ): List<BytecodeAtTargetCandidate> {
        val classBytes = provider.getClassBytes(ownerInternalName) ?: return emptyList()
        return InstructionExtractor.extract(classBytes, methodName, methodDescriptor)
    }

    private fun resolveSingleMethodDescriptor(ownerInternalName: String, methodName: String): String? =
        classIndex.getMethods(ownerInternalName)
            .firstOrNull { it.name == methodName }
            ?.descriptor

    private fun resolveMethodDescriptors(
        ownerInternalName: String,
        methodName: String,
        methodDescriptor: String?,
    ): List<String> {
        if (!methodDescriptor.isNullOrEmpty()) return listOf(methodDescriptor)
        val matching = classIndex.getMethods(ownerInternalName).filter { it.name == methodName }
        return when {
            matching.isEmpty() -> emptyList()
            else -> matching.map { it.descriptor }.sorted()
        }
    }

    private fun candidateKey(candidate: AtTargetCandidate): String =
        "${candidate.owner}#${candidate.name}#${candidate.descriptor}#${candidate.kind}#" +
            "${candidate.operationKind}#${candidate.ordinal}#${constantValueKey(candidate.constantValue)}#" +
            "${candidate.instructionOccurrenceIndex}#${candidate.occurrenceResultClassification}"

    private val candidateComparator = compareBy<AtTargetCandidate>(
        { it.owner },
        { it.name },
        { it.descriptor },
        { it.kind },
        { it.operationKind },
        { it.ordinal ?: Int.MIN_VALUE },
        { it.instructionOccurrenceIndex },
    )

    private fun matchesAtValue(candidate: BytecodeAtTargetCandidate, atValue: String): Boolean =
        when (atValue.uppercase()) {
            "INVOKE", "INVOKE_ASSIGN" -> candidate.kind in INVOKE_KINDS
            "FIELD" -> candidate.kind in FIELD_KINDS
            "NEW" -> candidate.kind == BytecodeAtTargetKind.NEW
            "RETURN" -> candidate.kind == BytecodeAtTargetKind.RETURN
            "CONSTANT" -> candidate.kind == BytecodeAtTargetKind.CONSTANT
            else -> false
        }

    private fun convertCandidate(candidate: BytecodeAtTargetCandidate): AtTargetCandidate {
        val kind = when (candidate.kind) {
            BytecodeAtTargetKind.INVOKE_VIRTUAL,
            BytecodeAtTargetKind.INVOKE_STATIC,
            BytecodeAtTargetKind.INVOKE_SPECIAL,
            BytecodeAtTargetKind.INVOKE_INTERFACE,
                -> AtTargetKind.INVOKE
            BytecodeAtTargetKind.FIELD_GET_INSTANCE,
            BytecodeAtTargetKind.FIELD_PUT_INSTANCE,
            BytecodeAtTargetKind.FIELD_GET_STATIC,
            BytecodeAtTargetKind.FIELD_PUT_STATIC,
                -> AtTargetKind.FIELD
            BytecodeAtTargetKind.NEW -> AtTargetKind.NEW
            BytecodeAtTargetKind.RETURN -> AtTargetKind.RETURN
            BytecodeAtTargetKind.CONSTANT -> AtTargetKind.CONSTANT
        }
        val ownerEntry = classIndex.findClass(candidate.owner)
        val detail = ownerEntry?.packageName?.ifEmpty { null }
            ?: AnnotationContextExtractor.internalToFqn(candidate.owner).substringBeforeLast('.')
        val displayLabel = when (kind) {
            AtTargetKind.FIELD -> "${candidate.name}: ${ClassMemberIndexAdapter.readableFieldType(candidate.descriptor)}"
            AtTargetKind.NEW -> if (candidate.name == "<init>") {
                "new ${ownerEntry?.simpleName ?: candidate.owner.substringAfterLast('/')}"
            } else {
                candidate.name
            }
            else -> {
                val signature = ClassMemberIndexAdapter.readableMethodSignature(candidate.name, candidate.descriptor)
                if (candidate.name == "<init>") "new ${ownerEntry?.simpleName ?: ""}$signature" else signature
            }
        }
        return AtTargetCandidate(
            owner = candidate.owner,
            name = candidate.name,
            descriptor = candidate.descriptor,
            displayLabel = displayLabel,
            detail = detail,
            kind = kind,
            ordinal = candidate.ordinal,
            namespace = MappingNamespace.NAMED,
            operationKind = toOperationKind(candidate.kind),
            constantValue = candidate.constantValue,
            instructionOccurrenceIndex = candidate.instructionOccurrenceIndex,
            occurrenceResultClassification = candidate.occurrenceResultClassification,
        )
    }

    private fun constantValueKey(value: ConstantValue?): String =
        when (value) {
            null -> ""
            is ConstantValue.StringValue -> "s:${value.value}"
            is ConstantValue.IntValue -> "i:${value.value}"
            is ConstantValue.LongValue -> "l:${value.value}"
            is ConstantValue.FloatValue -> "f:${value.value}"
            is ConstantValue.DoubleValue -> "d:${value.value}"
            is ConstantValue.ClassLiteral -> "c:${value.internalName}"
            ConstantValue.NullValue -> "null"
        }

    private fun toOperationKind(kind: BytecodeAtTargetKind): AtTargetOperationKind? =
        when (kind) {
            BytecodeAtTargetKind.INVOKE_VIRTUAL -> AtTargetOperationKind.INVOKE_VIRTUAL
            BytecodeAtTargetKind.INVOKE_STATIC -> AtTargetOperationKind.INVOKE_STATIC
            BytecodeAtTargetKind.INVOKE_SPECIAL -> AtTargetOperationKind.INVOKE_SPECIAL
            BytecodeAtTargetKind.INVOKE_INTERFACE -> AtTargetOperationKind.INVOKE_INTERFACE
            BytecodeAtTargetKind.FIELD_GET_INSTANCE -> AtTargetOperationKind.FIELD_GET_INSTANCE
            BytecodeAtTargetKind.FIELD_PUT_INSTANCE -> AtTargetOperationKind.FIELD_PUT_INSTANCE
            BytecodeAtTargetKind.FIELD_GET_STATIC -> AtTargetOperationKind.FIELD_GET_STATIC
            BytecodeAtTargetKind.FIELD_PUT_STATIC -> AtTargetOperationKind.FIELD_PUT_STATIC
            BytecodeAtTargetKind.NEW,
            BytecodeAtTargetKind.RETURN,
            BytecodeAtTargetKind.CONSTANT,
                -> null
        }

    companion object {
        private const val MAX_CANDIDATE_CACHE_ENTRIES = 256

        private val INVOKE_KINDS = setOf(
            BytecodeAtTargetKind.INVOKE_VIRTUAL,
            BytecodeAtTargetKind.INVOKE_STATIC,
            BytecodeAtTargetKind.INVOKE_SPECIAL,
            BytecodeAtTargetKind.INVOKE_INTERFACE,
        )

        private val FIELD_KINDS = setOf(
            BytecodeAtTargetKind.FIELD_GET_INSTANCE,
            BytecodeAtTargetKind.FIELD_PUT_INSTANCE,
            BytecodeAtTargetKind.FIELD_GET_STATIC,
            BytecodeAtTargetKind.FIELD_PUT_STATIC,
        )
    }
}
