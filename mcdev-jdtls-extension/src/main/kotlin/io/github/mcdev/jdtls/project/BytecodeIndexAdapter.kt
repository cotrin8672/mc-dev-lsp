package io.github.mcdev.jdtls.project

import io.github.mcdev.core.bytecode.AtTargetCandidate as BytecodeAtTargetCandidate
import io.github.mcdev.core.bytecode.AtTargetKind as BytecodeAtTargetKind
import io.github.mcdev.core.bytecode.InstructionExtractor
import io.github.mcdev.core.mixin.AnnotationContextExtractor
import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.AtTargetKind
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.model.MappingNamespace

class BytecodeIndexAdapter(
    private val provider: ClasspathClassBytesProvider,
    private val classIndex: ClassIndex,
) : BytecodeIndex {
    private val candidateCache = mutableMapOf<String, List<AtTargetCandidate>>()
    private val returnCountCache = mutableMapOf<String, Int>()

    override fun getAtTargetCandidates(
        ownerInternalName: String,
        methodName: String,
        methodDescriptor: String?,
        atValue: String,
    ): List<AtTargetCandidate> {
        val cacheKey = "$ownerInternalName#$methodName#${methodDescriptor.orEmpty()}#$atValue"
        return candidateCache.getOrPut(cacheKey) {
            val descriptor = methodDescriptor ?: resolveMethodDescriptor(ownerInternalName, methodName) ?: "()V"
            extractCandidates(ownerInternalName, methodName, descriptor)
                .filter { matchesAtValue(it, atValue) }
                .map { convertCandidate(it) }
        }
    }

    override fun getReturnOrdinalCount(
        ownerInternalName: String,
        methodName: String,
        methodDescriptor: String?,
    ): Int {
        val cacheKey = "$ownerInternalName#$methodName#${methodDescriptor.orEmpty()}"
        return returnCountCache.getOrPut(cacheKey) {
            val descriptor = methodDescriptor ?: resolveMethodDescriptor(ownerInternalName, methodName) ?: "()V"
            extractCandidates(ownerInternalName, methodName, descriptor)
                .count { it.kind == BytecodeAtTargetKind.RETURN }
                .coerceAtLeast(1)
        }
    }

    fun clearCache() {
        candidateCache.clear()
        returnCountCache.clear()
    }

    private fun extractCandidates(
        ownerInternalName: String,
        methodName: String,
        methodDescriptor: String,
    ): List<BytecodeAtTargetCandidate> {
        val classBytes = provider.getClassBytes(ownerInternalName) ?: return emptyList()
        return InstructionExtractor.extract(classBytes, methodName, methodDescriptor)
    }

    private fun resolveMethodDescriptor(ownerInternalName: String, methodName: String): String? =
        classIndex.getMethods(ownerInternalName)
            .firstOrNull { it.name == methodName }
            ?.descriptor

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
        )
    }

    companion object {
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
