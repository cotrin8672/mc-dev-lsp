package io.github.mcdev.core.bytecode

import io.github.mcdev.core.bytecode.OccurrenceResultClassification
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.objectweb.asm.ClassWriter
import org.objectweb.asm.Label
import org.objectweb.asm.MethodVisitor
import org.objectweb.asm.Opcodes

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

    @Test
    fun voidStaticInvokeIsClassifiedAsVoid() {
        val candidate = singleInvokeCandidate {
            visitMethodInsn(Opcodes.INVOKESTATIC, "java/lang/System", "gc", "()V", false)
        }
        assertEquals(OccurrenceResultClassification.VOID, candidate.occurrenceResultClassification)
        assertEquals(0, candidate.instructionOccurrenceIndex)
    }

    @Test
    fun voidInstanceInvokeIsClassifiedAsVoid() {
        val candidate = singleInvokeCandidate(matcher = { it.name == "clear" }) {
            visitTypeInsn(Opcodes.NEW, "java/util/ArrayList")
            visitInsn(Opcodes.DUP)
            visitMethodInsn(Opcodes.INVOKESPECIAL, "java/util/ArrayList", "<init>", "()V", false)
            visitMethodInsn(Opcodes.INVOKEVIRTUAL, "java/util/ArrayList", "clear", "()V", false)
        }
        assertEquals(OccurrenceResultClassification.VOID, candidate.occurrenceResultClassification)
    }

    @Test
    fun intInvokeFollowedByPopIsImmediatelyPopped() {
        val candidate = singleInvokeCandidate {
            visitIntInsn(Opcodes.BIPUSH, 1)
            visitMethodInsn(Opcodes.INVOKESTATIC, "java/lang/Math", "abs", "(I)I", false)
            visitInsn(Opcodes.POP)
        }
        assertEquals(OccurrenceResultClassification.IMMEDIATELY_POPPED, candidate.occurrenceResultClassification)
    }

    @Test
    fun objectInvokeFollowedByPopIsImmediatelyPopped() {
        val candidate = singleInvokeCandidate {
            visitIntInsn(Opcodes.BIPUSH, 1)
            visitMethodInsn(Opcodes.INVOKESTATIC, "java/lang/Integer", "valueOf", "(I)Ljava/lang/Integer;", false)
            visitInsn(Opcodes.POP)
        }
        assertEquals(OccurrenceResultClassification.IMMEDIATELY_POPPED, candidate.occurrenceResultClassification)
    }

    @Test
    fun longInvokeFollowedByPop2IsImmediatelyPopped() {
        val candidate = singleInvokeCandidate {
            visitInsn(Opcodes.LCONST_0)
            visitMethodInsn(Opcodes.INVOKESTATIC, "java/lang/Math", "abs", "(J)J", false)
            visitInsn(Opcodes.POP2)
        }
        assertEquals(OccurrenceResultClassification.IMMEDIATELY_POPPED, candidate.occurrenceResultClassification)
    }

    @Test
    fun doubleInvokeFollowedByPop2IsImmediatelyPopped() {
        val candidate = singleInvokeCandidate {
            visitInsn(Opcodes.DCONST_0)
            visitMethodInsn(Opcodes.INVOKESTATIC, "java/lang/Math", "abs", "(D)D", false)
            visitInsn(Opcodes.POP2)
        }
        assertEquals(OccurrenceResultClassification.IMMEDIATELY_POPPED, candidate.occurrenceResultClassification)
    }

    @Test
    fun longInvokeFollowedByWrongPopIsRetained() {
        val candidate = singleInvokeCandidate {
            visitInsn(Opcodes.LCONST_0)
            visitMethodInsn(Opcodes.INVOKESTATIC, "java/lang/Math", "abs", "(J)J", false)
            visitInsn(Opcodes.POP)
        }
        assertEquals(OccurrenceResultClassification.RETAINED, candidate.occurrenceResultClassification)
    }

    @Test
    fun invokeResultStoredIsRetained() {
        val candidate = singleInvokeCandidate {
            visitIntInsn(Opcodes.BIPUSH, 1)
            visitMethodInsn(Opcodes.INVOKESTATIC, "java/lang/Math", "abs", "(I)I", false)
            visitVarInsn(Opcodes.ISTORE, 1)
        }
        assertEquals(OccurrenceResultClassification.RETAINED, candidate.occurrenceResultClassification)
    }

    @Test
    fun invokeResultReturnedIsRetained() {
        val candidate = singleInvokeCandidate(
            methodDescriptor = "()I",
            finish = {
                visitInsn(Opcodes.IRETURN)
                visitMaxs(0, 0)
            },
        ) {
            visitIntInsn(Opcodes.BIPUSH, 1)
            visitMethodInsn(Opcodes.INVOKESTATIC, "java/lang/Math", "abs", "(I)I", false)
        }
        assertEquals(OccurrenceResultClassification.RETAINED, candidate.occurrenceResultClassification)
    }

    @Test
    fun fieldGetFollowedByPopIsRetained() {
        val candidate = singleFieldCandidate {
            visitVarInsn(Opcodes.ALOAD, 0)
            visitFieldInsn(Opcodes.GETFIELD, SAMPLE_OWNER, "value", "I")
            visitInsn(Opcodes.POP)
        }
        assertEquals(OccurrenceResultClassification.RETAINED, candidate.occurrenceResultClassification)
    }

    @Test
    fun fieldPutIsClassifiedAsVoid() {
        val candidate = singleFieldCandidate {
            visitVarInsn(Opcodes.ALOAD, 0)
            visitIntInsn(Opcodes.BIPUSH, 1)
            visitFieldInsn(Opcodes.PUTFIELD, SAMPLE_OWNER, "value", "I")
        }
        assertEquals(OccurrenceResultClassification.VOID, candidate.occurrenceResultClassification)
    }

    @Test
    fun labelBetweenInvokeAndPopIsRetained() {
        val candidate = singleInvokeCandidate(
            finish = { visitMaxs(0, 0) },
        ) {
            val tryStart = Label()
            val afterInvoke = Label()
            val tryEnd = Label()
            val handler = Label()
            visitTryCatchBlock(tryStart, afterInvoke, handler, "java/lang/RuntimeException")
            visitLabel(tryStart)
            visitIntInsn(Opcodes.BIPUSH, 1)
            visitMethodInsn(Opcodes.INVOKESTATIC, "java/lang/Math", "abs", "(I)I", false)
            visitLabel(afterInvoke)
            visitInsn(Opcodes.POP)
            visitLabel(tryEnd)
            visitInsn(Opcodes.RETURN)
            visitLabel(handler)
            visitInsn(Opcodes.POP)
            visitInsn(Opcodes.RETURN)
        }
        assertEquals(OccurrenceResultClassification.RETAINED, candidate.occurrenceResultClassification)
    }

    @Test
    fun frameBetweenLongInvokeAndPop2IsRetained() {
        val candidate = singleInvokeCandidate(
            useComputeMaxs = false,
            finish = {
                visitInsn(Opcodes.RETURN)
                visitMaxs(2, 2)
            },
        ) {
            visitInsn(Opcodes.LCONST_0)
            visitMethodInsn(Opcodes.INVOKESTATIC, "java/lang/Math", "abs", "(J)J", false)
            visitFrame(Opcodes.F_SAME, 0, null, 0, null)
            visitInsn(Opcodes.POP2)
        }
        assertEquals(OccurrenceResultClassification.RETAINED, candidate.occurrenceResultClassification)
    }

    @Test
    fun instructionOccurrenceIndexCountsRealInstructionsOnly() {
        val candidate = singleInvokeCandidate(
            finish = { visitMaxs(0, 0) },
        ) {
            val tryStart = Label()
            val tryEnd = Label()
            val handler = Label()
            visitTryCatchBlock(tryStart, tryEnd, handler, "java/lang/RuntimeException")
            visitLabel(tryStart)
            visitIntInsn(Opcodes.BIPUSH, 1)
            visitMethodInsn(Opcodes.INVOKESTATIC, "java/lang/Math", "abs", "(I)I", false)
            visitInsn(Opcodes.POP)
            visitLabel(tryEnd)
            visitInsn(Opcodes.RETURN)
            visitLabel(handler)
            visitInsn(Opcodes.POP)
            visitInsn(Opcodes.RETURN)
        }
        // BIPUSH=0, invoke=1 — labels must not shift the occurrence index
        assertEquals(1, candidate.instructionOccurrenceIndex)
    }

    @Test
    fun extractsUnaryZeroComparisonConditionsAndExcludesOtherJumps() {
        val ifltTarget = Label()
        val ifgeTarget = Label()
        val ifeqTarget = Label()
        val ifIcmpneTarget = Label()
        val bytes = classBytesWithMethod(
            methodDescriptor = "()V",
            useComputeMaxs = true,
            finish = {
                visitInsn(Opcodes.RETURN)
                visitMaxs(0, 0)
            },
        ) {
            visitInsn(Opcodes.ICONST_M1)
            visitJumpInsn(Opcodes.IFLT, ifltTarget)
            visitLabel(ifltTarget)
            visitInsn(Opcodes.ICONST_0)
            visitJumpInsn(Opcodes.IFGE, ifgeTarget)
            visitLabel(ifgeTarget)
            visitInsn(Opcodes.ICONST_0)
            visitJumpInsn(Opcodes.IFEQ, ifeqTarget)
            visitLabel(ifeqTarget)
            visitInsn(Opcodes.ICONST_1)
            visitJumpInsn(Opcodes.IF_ICMPNE, ifIcmpneTarget)
            visitLabel(ifIcmpneTarget)
        }

        val candidates = InstructionExtractor.extract(bytes, "run", "()V")
        val conditions = candidates.filter { it.conditionOpcode != null }

        assertEquals(listOf(Opcodes.IFLT, Opcodes.IFGE), conditions.map { it.conditionOpcode })
        assertEquals(listOf(1, 3), conditions.map { it.instructionOccurrenceIndex })
        assertTrue(conditions.all { it.kind == AtTargetKind.CONSTANT && it.constantValue == null })
        assertTrue(candidates.none { it.instructionOccurrenceIndex == 5 || it.instructionOccurrenceIndex == 7 })
    }

    @Test
    fun excludesUnaryConditionsImmediatelyAfterCompareInstructions() {
        val lcmpTarget = Label()
        val fcmplTarget = Label()
        val fcmpgTarget = Label()
        val dcmplTarget = Label()
        val dcmpgTarget = Label()
        val unaryTarget = Label()
        val bytes = classBytesWithMethod(
            methodDescriptor = "()V",
            useComputeMaxs = true,
            finish = {
                visitInsn(Opcodes.RETURN)
                visitMaxs(0, 0)
            },
        ) {
            visitInsn(Opcodes.LCONST_0)
            visitInsn(Opcodes.LCONST_1)
            visitInsn(Opcodes.LCMP)
            visitJumpInsn(Opcodes.IFEQ, lcmpTarget)
            visitLabel(lcmpTarget)
            visitInsn(Opcodes.FCONST_0)
            visitInsn(Opcodes.FCONST_1)
            visitInsn(Opcodes.FCMPL)
            visitJumpInsn(Opcodes.IFNE, fcmplTarget)
            visitLabel(fcmplTarget)
            visitInsn(Opcodes.FCONST_0)
            visitInsn(Opcodes.FCONST_1)
            visitInsn(Opcodes.FCMPG)
            visitJumpInsn(Opcodes.IFLT, fcmpgTarget)
            visitLabel(fcmpgTarget)
            visitInsn(Opcodes.DCONST_0)
            visitInsn(Opcodes.DCONST_1)
            visitInsn(Opcodes.DCMPL)
            visitJumpInsn(Opcodes.IFGE, dcmplTarget)
            visitLabel(dcmplTarget)
            visitInsn(Opcodes.DCONST_0)
            visitInsn(Opcodes.DCONST_1)
            visitInsn(Opcodes.DCMPG)
            visitJumpInsn(Opcodes.IFGT, dcmpgTarget)
            visitLabel(dcmpgTarget)
            visitInsn(Opcodes.ICONST_M1)
            visitJumpInsn(Opcodes.IFLT, unaryTarget)
            visitLabel(unaryTarget)
        }

        val conditions = InstructionExtractor.extract(bytes, "run", "()V")
            .filter { it.conditionOpcode != null }
        assertEquals(listOf(Opcodes.IFLT), conditions.map { it.conditionOpcode })
        assertEquals(listOf(21), conditions.map { it.instructionOccurrenceIndex })
    }

    private fun extract(methodName: String): List<AtTargetCandidate> =
        InstructionExtractor.extract(invokeBytes, methodName, "()V")

    private fun singleInvokeCandidate(
        methodDescriptor: String = "()V",
        useComputeMaxs: Boolean = true,
        matcher: (AtTargetCandidate) -> Boolean = { it.kind.name.startsWith("INVOKE_") },
        finish: MethodVisitor.() -> Unit = {
            visitInsn(Opcodes.RETURN)
            visitMaxs(0, 0)
        },
        body: MethodVisitor.() -> Unit,
    ): AtTargetCandidate {
        val bytes = classBytesWithMethod(methodDescriptor, useComputeMaxs, finish, body)
        return InstructionExtractor.extract(bytes, "run", methodDescriptor)
            .first(matcher)
    }

    private fun singleFieldCandidate(
        body: MethodVisitor.() -> Unit,
    ): AtTargetCandidate {
        val bytes = classBytesWithField(body)
        return InstructionExtractor.extract(bytes, "run", "()V")
            .first { it.kind.name.startsWith("FIELD_") && it.name == "value" }
    }

    private fun classBytesWithMethod(
        methodDescriptor: String,
        useComputeMaxs: Boolean,
        finish: MethodVisitor.() -> Unit,
        body: MethodVisitor.() -> Unit,
    ): ByteArray {
        val flags = if (useComputeMaxs) ClassWriter.COMPUTE_MAXS else 0
        val classWriter = ClassWriter(flags)
        classWriter.visit(Opcodes.V21, Opcodes.ACC_PUBLIC, SAMPLE_OWNER, null, "java/lang/Object", null)
        classWriter.visitMethod(Opcodes.ACC_PUBLIC, "run", methodDescriptor, null, null).apply {
            visitCode()
            body()
            finish()
            visitEnd()
        }
        classWriter.visitEnd()
        return classWriter.toByteArray()
    }

    private fun classBytesWithField(body: MethodVisitor.() -> Unit): ByteArray {
        val classWriter = ClassWriter(ClassWriter.COMPUTE_MAXS)
        classWriter.visit(Opcodes.V21, Opcodes.ACC_PUBLIC, SAMPLE_OWNER, null, "java/lang/Object", null)
        classWriter.visitField(Opcodes.ACC_PUBLIC, "value", "I", null, null)
        classWriter.visitMethod(Opcodes.ACC_PUBLIC, "run", "()V", null, null).apply {
            visitCode()
            body()
            visitInsn(Opcodes.RETURN)
            visitMaxs(0, 0)
            visitEnd()
        }
        classWriter.visitEnd()
        return classWriter.toByteArray()
    }

    private companion object {
        const val SAMPLE_OWNER = "test/OccurrenceSamples"
    }
}
