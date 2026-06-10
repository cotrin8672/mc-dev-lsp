package io.github.mcdev.core.index

import io.github.mcdev.core.bytecode.BytecodeFixtureCompiler
import io.github.mcdev.core.bytecode.BytecodeIndexService
import io.github.mcdev.core.mixin.AtTargetKind
import io.github.mcdev.core.mixin.e2e.MixinE2ETestSupport
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class BytecodeAtTargetAdapterTest {
    @Test
    fun simpleTargetReturnCandidates() {
        val adapter = BytecodeAtTargetAdapter(
            BytecodeIndexService(),
            MixinE2ETestSupport.simpleTargetProvider(),
        )
        val candidates = adapter.getAtTargetCandidates(
            MixinE2ETestSupport.SIMPLE_TARGET_INTERNAL,
            "draw",
            null,
            "RETURN",
        )
        assertTrue(candidates.isNotEmpty())
        assertTrue(candidates.all { it.kind == AtTargetKind.RETURN })
        assertTrue(candidates.all { it.displayLabel == "RETURN" })
    }

    @Test
    fun simpleTargetReturnOrdinalCount() {
        val adapter = BytecodeAtTargetAdapter(
            BytecodeIndexService(),
            MixinE2ETestSupport.simpleTargetProvider(),
        )
        val count = adapter.getReturnOrdinalCount(
            MixinE2ETestSupport.SIMPLE_TARGET_INTERNAL,
            "draw",
            null,
        )
        assertTrue(count >= 1)
    }

    @Test
    fun invokeSamplesProducesInvokeCandidates() {
        val provider = BytecodeFixtureCompiler.provider()
        val adapter = BytecodeAtTargetAdapter(BytecodeIndexService(), provider)
        val owner = BytecodeFixtureCompiler.internalName("InvokeSamples")
        val candidates = adapter.getAtTargetCandidates(owner, "virtualInvoke", "()V", "INVOKE")
        assertTrue(candidates.isNotEmpty())
        assertTrue(candidates.all { it.kind == AtTargetKind.INVOKE })
        assertTrue(candidates.all { it.displayLabel != it.owner })
    }

    @Test
    fun invokeCandidateLabelDiffersFromInsertDescriptor() {
        val provider = BytecodeFixtureCompiler.provider()
        val adapter = BytecodeAtTargetAdapter(BytecodeIndexService(), provider)
        val owner = BytecodeFixtureCompiler.internalName("InvokeSamples")
        val candidates = adapter.getAtTargetCandidates(owner, "virtualInvoke", "()V", "INVOKE")
        val first = candidates.first()
        assertTrue(first.displayLabel.contains("length") || first.displayLabel.contains("println"))
        assertTrue(first.owner.isNotEmpty())
    }

    @Test
    fun fieldSamplesProducesFieldCandidates() {
        val provider = BytecodeFixtureCompiler.provider()
        val adapter = BytecodeAtTargetAdapter(BytecodeIndexService(), provider)
        val owner = BytecodeFixtureCompiler.internalName("FieldSamples")
        val candidates = adapter.getAtTargetCandidates(owner, "accessFields", "()V", "FIELD")
        assertTrue(candidates.isNotEmpty())
        assertTrue(candidates.all { it.kind == AtTargetKind.FIELD })
        assertTrue(candidates.first().displayLabel.contains(':'))
    }

    @Test
    fun filtersByAtValue() {
        val provider = BytecodeFixtureCompiler.provider()
        val adapter = BytecodeAtTargetAdapter(BytecodeIndexService(), provider)
        val invokeOwner = BytecodeFixtureCompiler.internalName("InvokeSamples")
        val fieldOwner = BytecodeFixtureCompiler.internalName("FieldSamples")
        val invokeOnly = adapter.getAtTargetCandidates(invokeOwner, "virtualInvoke", "()V", "INVOKE")
        val fieldOnly = adapter.getAtTargetCandidates(fieldOwner, "accessFields", "()V", "FIELD")
        val crossFiltered = adapter.getAtTargetCandidates(invokeOwner, "staticInvoke", "()V", "FIELD")
        assertTrue(invokeOnly.isNotEmpty())
        assertTrue(fieldOnly.isNotEmpty())
        assertTrue(crossFiltered.isEmpty())
    }

    @Test
    fun returnsEmptyForMissingOwner() {
        val adapter = BytecodeAtTargetAdapter(
            BytecodeIndexService(),
            MixinE2ETestSupport.simpleTargetProvider(),
        )
        assertTrue(adapter.getAtTargetCandidates("missing/Class", "draw", null, "RETURN").isEmpty())
    }
}
