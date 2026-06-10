package io.github.mcdev.core.bytecode

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

class BytecodeIndexServiceTest {
    private val provider = BytecodeFixtureCompiler.provider()
    private val service = BytecodeIndexService()

    @Test
    fun buildsMemberIndexFromProvider() {
        val index = service.buildIndex(provider)
        assertTrue(index.classes.isNotEmpty())
    }

    @Test
    fun cachesIndexByClasspathHash() {
        val first = service.buildIndex(provider)
        val second = service.buildIndex(provider)
        assertEquals(first, second)
        assertNotNull(service.getCachedIndex(provider.classpathHash()))
    }

    @Test
    fun extractsInvokeInstructions() {
        val result = service.extractInstructions(
            provider,
            BytecodeFixtureCompiler.internalName("InvokeSamples"),
            "virtualInvoke",
            "()V",
        )
        val candidates = assertIs<BytecodeIndexResult.Success<List<AtTargetCandidate>>>(result).value
        assertTrue(candidates.any { it.kind == AtTargetKind.INVOKE_VIRTUAL })
    }

    @Test
    fun extractsFieldInstructions() {
        val result = service.extractInstructions(
            provider,
            BytecodeFixtureCompiler.internalName("FieldSamples"),
            "accessFields",
            "()V",
        )
        val candidates = assertIs<BytecodeIndexResult.Success<List<AtTargetCandidate>>>(result).value
        assertTrue(candidates.any { it.kind == AtTargetKind.FIELD_GET_STATIC })
    }

    @Test
    fun extractsNewInstructions() {
        val result = service.extractInstructions(
            provider,
            BytecodeFixtureCompiler.internalName("NewSamples"),
            "createObjects",
            "()V",
        )
        val candidates = assertIs<BytecodeIndexResult.Success<List<AtTargetCandidate>>>(result).value
        assertTrue(candidates.any { it.kind == AtTargetKind.NEW })
    }

    @Test
    fun extractsConstantInstructions() {
        val result = service.extractInstructions(
            provider,
            BytecodeFixtureCompiler.internalName("ConstantSamples"),
            "constants",
            "()V",
        )
        val candidates = assertIs<BytecodeIndexResult.Success<List<AtTargetCandidate>>>(result).value
        assertTrue(candidates.any { it.kind == AtTargetKind.CONSTANT })
    }

    @Test
    fun extractsReturnInstructions() {
        val result = service.extractInstructions(
            provider,
            BytecodeFixtureCompiler.internalName("ReturnSamples"),
            "multipleReturns",
            "(I)I",
        )
        val candidates = assertIs<BytecodeIndexResult.Success<List<AtTargetCandidate>>>(result).value
        assertTrue(candidates.count { it.kind == AtTargetKind.RETURN } >= 2)
    }

    @Test
    fun cachesExtractedInstructions() {
        val owner = BytecodeFixtureCompiler.internalName("InvokeSamples")
        val first = service.extractInstructions(provider, owner, "staticInvoke", "()V")
        val second = service.extractInstructions(provider, owner, "staticInvoke", "()V")
        assertEquals(first, second)
    }

    @Test
    fun returnsClassBytesMissingError() {
        val result = service.extractInstructions(provider, "missing/Class", "run", "()V")
        val error = assertIs<BytecodeIndexResult.Failure>(result).error
        assertIs<ClassBytesMissingError>(error)
        assertEquals("missing/Class", error.internalName)
    }

    @Test
    fun returnsMethodNotFoundError() {
        val owner = BytecodeFixtureCompiler.internalName("InvokeSamples")
        val result = service.extractInstructions(provider, owner, "missingMethod", "()V")
        val error = assertIs<BytecodeIndexResult.Failure>(result).error
        assertIs<MethodNotFoundError>(error)
        assertEquals("missingMethod", error.methodName)
    }

    @Test
    fun methodNotFoundWhenDescriptorMismatch() {
        val owner = BytecodeFixtureCompiler.internalName("InvokeSamples")
        val result = service.extractInstructions(provider, owner, "virtualInvoke", "(I)V")
        val error = assertIs<BytecodeIndexResult.Failure>(result).error
        assertIs<MethodNotFoundError>(error)
    }

    @Test
    fun duplicateInvokeOrdinalsThroughService() {
        val owner = BytecodeFixtureCompiler.internalName("InvokeSamples")
        val result = service.extractInstructions(provider, owner, "duplicateInvokes", "()V")
        val candidates = assertIs<BytecodeIndexResult.Success<List<AtTargetCandidate>>>(result).value
            .filter { it.kind == AtTargetKind.INVOKE_STATIC && it.name == "abs" }
        assertEquals(listOf(0, 1, 2), candidates.map { it.ordinal })
    }
}
