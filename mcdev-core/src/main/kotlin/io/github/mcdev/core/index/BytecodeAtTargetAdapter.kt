package io.github.mcdev.core.index

import io.github.mcdev.core.bytecode.BytecodeIndexResult
import io.github.mcdev.core.bytecode.BytecodeIndexService
import io.github.mcdev.core.bytecode.ClassBytesProvider
import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixinextras.BytecodeCommonSuperClassResolver

class BytecodeAtTargetAdapter(
    private val bytecodeService: BytecodeIndexService,
    private val provider: ClassBytesProvider,
) : BytecodeIndex {
    private val memberIndex by lazy { bytecodeService.buildIndex(provider) }
    private val commonSuperClassResolver by lazy {
        BytecodeCommonSuperClassResolver(classBytesLookup = provider::getClassBytes)
    }

    override fun getClassBytes(ownerInternalName: String): ByteArray? =
        provider.getClassBytes(ownerInternalName)

    override fun resolveCommonSuperClass(type1Descriptor: String, type2Descriptor: String): String? =
        commonSuperClassResolver.resolve(type1Descriptor, type2Descriptor)

    override fun getAtTargetCandidates(
        ownerInternalName: String,
        methodName: String,
        methodDescriptor: String?,
        atValue: String,
    ): List<AtTargetCandidate> {
        if (methodName.isEmpty() || atValue.isEmpty()) return emptyList()
        val descriptor = resolveMethodDescriptor(ownerInternalName, methodName, methodDescriptor) ?: return emptyList()
        return when (
            val result = bytecodeService.extractInstructions(
                provider,
                ownerInternalName,
                methodName,
                descriptor,
            )
        ) {
            is BytecodeIndexResult.Success ->
                result.value
                    .filter { BytecodeIndexEntryMapper.matchesAtValue(it.kind, atValue) }
                    .map(BytecodeIndexEntryMapper::toMixinAtTarget)
            is BytecodeIndexResult.Failure -> emptyList()
        }
    }

    override fun getReturnOrdinalCount(
        ownerInternalName: String,
        methodName: String,
        methodDescriptor: String?,
    ): Int {
        if (methodName.isEmpty()) return 0
        val descriptor = resolveMethodDescriptor(ownerInternalName, methodName, methodDescriptor) ?: return 0
        return when (
            val result = bytecodeService.extractInstructions(
                provider,
                ownerInternalName,
                methodName,
                descriptor,
            )
        ) {
            is BytecodeIndexResult.Success ->
                result.value.count { BytecodeIndexEntryMapper.matchesAtValue(it.kind, "RETURN") }
            is BytecodeIndexResult.Failure -> 0
        }
    }

    private fun resolveMethodDescriptor(
        ownerInternalName: String,
        methodName: String,
        methodDescriptor: String?,
    ): String? {
        if (!methodDescriptor.isNullOrEmpty()) return methodDescriptor
        val methods = memberIndex.methodsByOwner[ownerInternalName].orEmpty().filter { it.name == methodName }
        return when (methods.size) {
            1 -> methods.first().descriptor
            else -> null
        }
    }
}
