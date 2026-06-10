package io.github.mcdev.core.bytecode

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class InstructionExtractorInvokeTest {
    private val invokeClass = BytecodeFixtureCompiler.internalName("InvokeSamples")
    private val invokeBytes = BytecodeFixtureCompiler.classBytes("InvokeSamples")

    @Test
    fun extractsVirtualInvoke() {
        val candidates = extract("virtualInvoke")
        val virtual = candidates.filter { it.kind == AtTargetKind.INVOKE_VIRTUAL }
        assertTrue(virtual.any { it.owner == "java/lang/String" && it.name == "length" })
        assertTrue(virtual.any { it.owner == "java/io/PrintStream" && it.name == "println" })
    }

    @Test
    fun extractsStaticInvoke() {
        val candidates = extract("staticInvoke")
        val static = candidates.filter { it.kind == AtTargetKind.INVOKE_STATIC }
        assertTrue(static.any { it.owner == "java/lang/Math" && it.name == "abs" })
        assertTrue(static.any { it.owner == "java/lang/Integer" && it.name == "valueOf" })
    }

    @Test
    fun extractsSpecialInvoke() {
        val candidates = extract("specialInvoke")
        val special = candidates.filter { it.kind == AtTargetKind.INVOKE_SPECIAL }
        assertTrue(special.any { it.owner == "java/lang/Object" && it.name == "toString" })
        assertTrue(special.any { it.owner == "java/lang/String" && it.name == "<init>" })
    }

    @Test
    fun extractsInterfaceInvoke() {
        val candidates = extract("interfaceInvoke")
        val iface = candidates.filter { it.kind == AtTargetKind.INVOKE_INTERFACE }
        assertTrue(iface.any { it.owner == "java/lang/Runnable" && it.name == "run" })
    }

    @Test
    fun invokeVirtualHasMethodDescriptor() {
        val candidates = extract("virtualInvoke")
        val length = candidates.first { it.name == "length" }
        assertEquals("()I", length.descriptor)
    }

    @Test
    fun invokeStaticHasMethodDescriptor() {
        val candidates = extract("staticInvoke")
        val abs = candidates.first { it.name == "abs" }
        assertEquals("(I)I", abs.descriptor)
    }

    @Test
    fun constructorInvokeIsSpecial() {
        val candidates = extract("specialInvoke")
        val ctor = candidates.first { it.name == "<init>" }
        assertEquals(AtTargetKind.INVOKE_SPECIAL, ctor.kind)
        assertEquals("(Ljava/lang/String;)V", ctor.descriptor)
    }

    @Test
    fun duplicateInvokesReceiveDistinctOrdinals() {
        val candidates = extract("duplicateInvokes")
            .filter { it.kind == AtTargetKind.INVOKE_STATIC && it.name == "abs" }
        assertEquals(3, candidates.size)
        assertEquals(listOf(0, 1, 2), candidates.map { it.ordinal })
    }

    @Test
    fun duplicateInvokesShareOwnerAndDescriptor() {
        val candidates = extract("duplicateInvokes")
            .filter { it.kind == AtTargetKind.INVOKE_STATIC && it.name == "abs" }
        assertTrue(candidates.all { it.owner == "java/lang/Math" && it.descriptor == "(I)I" })
    }

    @Test
    fun returnsEmptyListForMissingMethod() {
        val candidates = InstructionExtractor.extract(invokeBytes, "missing", "()V")
        assertTrue(candidates.isEmpty())
    }

    @Test
    fun returnsEmptyListForWrongDescriptor() {
        val candidates = InstructionExtractor.extract(invokeBytes, "virtualInvoke", "(I)V")
        assertTrue(candidates.isEmpty())
    }

    private fun extract(methodName: String): List<AtTargetCandidate> =
        InstructionExtractor.extract(invokeBytes, methodName, "()V")
}
