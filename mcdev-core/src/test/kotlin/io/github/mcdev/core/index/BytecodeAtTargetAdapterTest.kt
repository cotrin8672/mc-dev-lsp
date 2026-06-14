package io.github.mcdev.core.index

import io.github.mcdev.core.bytecode.BytecodeFixtureCompiler
import io.github.mcdev.core.bytecode.BytecodeIndexService
import io.github.mcdev.core.bytecode.InMemoryClassBytesProvider
import io.github.mcdev.core.mixin.AtTargetKind
import io.github.mcdev.core.mixin.e2e.MixinE2ETestSupport
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.objectweb.asm.ClassWriter
import org.objectweb.asm.Opcodes

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
            "(Ljava/lang/String;FF)V",
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
            "(Ljava/lang/String;FF)V",
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

    @Test
    fun overloadedMethodWithoutDescriptorReturnsEmpty() {
        val owner = "test/OverloadSamples"
        val adapter = BytecodeAtTargetAdapter(
            BytecodeIndexService(),
            InMemoryClassBytesProvider(mapOf(owner to overloadedDrawClassBytes())),
        )
        assertTrue(adapter.getAtTargetCandidates(owner, "draw", null, "RETURN").isEmpty())
        assertEquals(0, adapter.getReturnOrdinalCount(owner, "draw", null))
    }

    @Test
    fun overloadedMethodWithExplicitDescriptorResolvesCorrectOverload() {
        val owner = "test/OverloadSamples"
        val adapter = BytecodeAtTargetAdapter(
            BytecodeIndexService(),
            InMemoryClassBytesProvider(mapOf(owner to overloadedDrawClassBytes())),
        )
        val voidCandidates = adapter.getAtTargetCandidates(owner, "draw", "()V", "RETURN")
        val intCandidates = adapter.getAtTargetCandidates(owner, "draw", "(I)V", "RETURN")
        assertTrue(voidCandidates.isNotEmpty())
        assertTrue(intCandidates.isNotEmpty())
        assertTrue(voidCandidates.all { it.kind == AtTargetKind.RETURN })
        assertTrue(intCandidates.all { it.kind == AtTargetKind.RETURN })
        assertEquals(1, adapter.getReturnOrdinalCount(owner, "draw", "()V"))
        assertEquals(1, adapter.getReturnOrdinalCount(owner, "draw", "(I)V"))
    }

    private fun overloadedDrawClassBytes(): ByteArray {
        val classWriter = ClassWriter(0)
        classWriter.visit(
            Opcodes.V21,
            Opcodes.ACC_PUBLIC,
            "test/OverloadSamples",
            null,
            "java/lang/Object",
            null,
        )
        classWriter.visitMethod(Opcodes.ACC_PUBLIC, "draw", "()V", null, null).apply {
            visitCode()
            visitInsn(Opcodes.RETURN)
            visitMaxs(0, 1)
            visitEnd()
        }
        classWriter.visitMethod(Opcodes.ACC_PUBLIC, "draw", "(I)V", null, null).apply {
            visitCode()
            visitInsn(Opcodes.RETURN)
            visitMaxs(0, 2)
            visitEnd()
        }
        classWriter.visitEnd()
        return classWriter.toByteArray()
    }
}
