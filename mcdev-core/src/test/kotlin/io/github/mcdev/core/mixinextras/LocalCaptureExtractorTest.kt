package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.bytecode.BytecodeFixtureCompiler
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue
import org.objectweb.asm.ClassWriter
import org.objectweb.asm.Label
import org.objectweb.asm.MethodVisitor
import org.objectweb.asm.Opcodes
import org.objectweb.asm.tree.AbstractInsnNode
import org.objectweb.asm.tree.MethodInsnNode

class LocalCaptureExtractorTest {
    private val fixtureBytes = BytecodeFixtureCompiler.classBytes("LocalCaptureSamples")

    @Test
    fun instanceMethodExcludesReceiverAndMarksArguments() {
        val captureIndex = invokeIndex(
            methodName = "instanceWithArgs",
            methodDescriptor = "(Ljava/lang/String;I)I",
            owner = "java/lang/String",
            name = "length",
        )
        val result = extract(
            methodName = "instanceWithArgs",
            methodDescriptor = "(Ljava/lang/String;I)I",
            instructionOccurrenceIndices = setOf(captureIndex),
        )

        val snapshot = result.snapshots.single()
        assertEquals(captureIndex, snapshot.instructionOccurrenceIndex)
        assertEquals(
            listOf(
                candidate(1, "Ljava/lang/String;", ordinal = 0, isArgument = true),
                candidate(2, "I", ordinal = 0, isArgument = true),
                candidate(3, "I", ordinal = 1, isArgument = false),
            ),
            snapshot.candidates,
        )
    }

    @Test
    fun staticMethodCoversWideArgumentSlots() {
        val captureIndex = invokeIndex(
            methodName = "staticWithWide",
            methodDescriptor = "(JDI)V",
            owner = BytecodeFixtureCompiler.internalName("LocalCaptureSamples"),
            name = "staticWithWideMarker",
            occurrence = 2,
        )
        val result = extract(
            methodName = "staticWithWide",
            methodDescriptor = "(JDI)V",
            instructionOccurrenceIndices = setOf(captureIndex),
        )

        val snapshot = result.snapshots.single()
        assertEquals(
            listOf(
                candidate(0, "J", ordinal = 0, isArgument = true),
                candidate(2, "D", ordinal = 0, isArgument = true),
                candidate(4, "I", ordinal = 0, isArgument = true),
            ),
            snapshot.candidates,
        )
        assertTrue(snapshot.candidates.none { it.slotIndex == 1 || it.slotIndex == 3 })
    }

    @Test
    fun bodyLocalIsNotMarkedAsArgument() {
        val result = extract(
            methodName = "instanceWithArgs",
            methodDescriptor = "(Ljava/lang/String;I)I",
            instructionOccurrenceIndices = setOf(
                invokeIndex(
                    methodName = "instanceWithArgs",
                    methodDescriptor = "(Ljava/lang/String;I)I",
                    owner = "java/lang/String",
                    name = "length",
                ),
            ),
        )

        val body = result.snapshots.single().candidates.single { it.slotIndex == 3 }
        assertEquals("I", body.descriptor)
        assertEquals(false, body.isArgument)
    }

    @Test
    fun localVariableTableNameAttachedWhenLive() {
        val classBytes = classBytesWithNamedLocalVariableTable()
        val captureIndex = invokeIndex(
            classBytes = classBytes,
            methodName = "namedLocals",
            methodDescriptor = "(I)I",
            owner = "java/lang/Math",
            name = "abs",
        )
        val result = extract(
            classBytes = classBytes,
            methodName = "namedLocals",
            methodDescriptor = "(I)I",
            instructionOccurrenceIndices = setOf(captureIndex),
        )

        val candidates = result.snapshots.single().candidates
        assertEquals("arg", candidates.single { it.slotIndex == 0 }.name)
        assertEquals("early", candidates.single { it.slotIndex == 1 }.name)
        assertTrue(candidates.none { it.slotIndex == 2 })
    }

    @Test
    fun localVariableTableNameNullWhenDebugInfoAbsent() {
        val classBytes = classBytesWithoutLocalVariableTable()
        val captureIndex = invokeIndex(
            classBytes = classBytes,
            methodName = "noDebug",
            methodDescriptor = "(I)I",
            owner = "java/lang/Math",
            name = "abs",
        )
        val result = extract(
            classBytes = classBytes,
            methodName = "noDebug",
            methodDescriptor = "(I)I",
            instructionOccurrenceIndices = setOf(captureIndex),
        )

        val snapshot = result.snapshots.single()
        assertEquals(null, snapshot.candidates.single { it.slotIndex == 1 }.name)
        assertTrue(snapshot.candidates.all { it.name == null })
    }

    @Test
    fun multipleInstructionIndicesShareSingleAnalysis() {
        val methodName = "multipleIndices"
        val methodDescriptor = "(I)V"
        val firstAbs = invokeIndex(methodName, methodDescriptor, "java/lang/Math", "abs", occurrence = 0)
        val secondAbs = invokeIndex(methodName, methodDescriptor, "java/lang/Math", "abs", occurrence = 1)

        val result = extract(
            methodName = methodName,
            methodDescriptor = methodDescriptor,
            instructionOccurrenceIndices = setOf(secondAbs, firstAbs),
        )

        assertEquals(listOf(firstAbs, secondAbs), result.snapshots.map { it.instructionOccurrenceIndex })

        val firstSnapshot = result.snapshots[0]
        assertEquals(
            listOf(
                candidate(1, "I", ordinal = 0, isArgument = true),
                candidate(2, "I", ordinal = 1, isArgument = false),
                candidate(3, "I", ordinal = 2, isArgument = false),
            ),
            firstSnapshot.candidates,
        )

        val secondSnapshot = result.snapshots[1]
        assertEquals(
            listOf(
                candidate(1, "I", ordinal = 0, isArgument = true),
                candidate(2, "I", ordinal = 1, isArgument = false),
                candidate(3, "I", ordinal = 2, isArgument = false),
                candidate(4, "I", ordinal = 3, isArgument = false),
            ),
            secondSnapshot.candidates,
        )
    }

    @Test
    fun labelLineAndFrameNodesDoNotShiftRequestedOccurrence() {
        val classBytes = classBytesWithDebugPseudoNodes()
        val captureIndex = invokeIndex(
            classBytes = classBytes,
            methodName = "withDebug",
            methodDescriptor = "(I)I",
            owner = "java/lang/Math",
            name = "abs",
        )
        val insnListIndex = insnListIndexOfInvoke(
            classBytes = classBytes,
            methodName = "withDebug",
            methodDescriptor = "(I)I",
            owner = "java/lang/Math",
            name = "abs",
        )

        assertEquals(5, captureIndex)
        assertTrue(insnListIndex > captureIndex)

        val result = extract(
            classBytes = classBytes,
            methodName = "withDebug",
            methodDescriptor = "(I)I",
            instructionOccurrenceIndices = setOf(captureIndex),
        )

        val snapshot = result.snapshots.single()
        assertEquals(captureIndex, snapshot.instructionOccurrenceIndex)
        assertEquals(
            listOf(
                candidate(0, "I", ordinal = 0, isArgument = true),
                candidate(1, "I", ordinal = 1, isArgument = false),
            ),
            snapshot.candidates,
        )

        val shiftedResult = LocalCaptureExtractor.extract(
            classBytes = classBytes,
            methodName = "withDebug",
            methodDescriptor = "(I)I",
            instructionOccurrenceIndices = setOf(insnListIndex),
        )
        val shiftedFailure = assertIs<LocalCaptureResult.Failure>(shiftedResult)
        assertIs<LocalCaptureInvalidInstructionIndexError>(shiftedFailure.error)
    }

    @Test
    fun missingMethodFailsClosed() {
        val result = LocalCaptureExtractor.extract(
            classBytes = fixtureBytes,
            methodName = "missing",
            methodDescriptor = "()V",
            instructionOccurrenceIndices = setOf(0),
        )

        val failure = assertIs<LocalCaptureResult.Failure>(result)
        assertIs<LocalCaptureMethodNotFoundError>(failure.error)
    }

    @Test
    fun invalidInstructionIndexFailsClosed() {
        val result = LocalCaptureExtractor.extract(
            classBytes = fixtureBytes,
            methodName = "instanceWithArgs",
            methodDescriptor = "(Ljava/lang/String;I)I",
            instructionOccurrenceIndices = setOf(999),
        )

        val failure = assertIs<LocalCaptureResult.Failure>(result)
        assertIs<LocalCaptureInvalidInstructionIndexError>(failure.error)
    }

    @Test
    fun missingClassBytesFailClosed() {
        val result = LocalCaptureExtractor.extract(
            classBytes = null,
            methodName = "any",
            methodDescriptor = "()V",
            instructionOccurrenceIndices = setOf(0),
        )

        val failure = assertIs<LocalCaptureResult.Failure>(result)
        assertIs<LocalCaptureMissingClassBytesError>(failure.error)
    }

    @Test
    fun emptyClassBytesFailClosed() {
        val result = LocalCaptureExtractor.extract(
            classBytes = byteArrayOf(),
            methodName = "any",
            methodDescriptor = "()V",
            instructionOccurrenceIndices = setOf(0),
        )

        val failure = assertIs<LocalCaptureResult.Failure>(result)
        assertIs<LocalCaptureEmptyClassBytesError>(failure.error)
    }

    @Test
    fun corruptClassBytesFailClosed() {
        val result = LocalCaptureExtractor.extract(
            classBytes = byteArrayOf(0x00, 0x01, 0x02, 0x03),
            methodName = "any",
            methodDescriptor = "()V",
            instructionOccurrenceIndices = setOf(0),
        )

        val failure = assertIs<LocalCaptureResult.Failure>(result)
        assertIs<LocalCaptureCorruptClassBytesError>(failure.error)
    }

    @Test
    fun analysisFailureFailsClosed() {
        val result = LocalCaptureExtractor.extract(
            classBytes = unanalyzableClassBytes(),
            methodName = "broken",
            methodDescriptor = "()V",
            instructionOccurrenceIndices = setOf(1),
        )

        val failure = assertIs<LocalCaptureResult.Failure>(result)
        assertIs<LocalCaptureAnalysisError>(failure.error)
    }

    private fun extract(
        methodName: String,
        methodDescriptor: String,
        instructionOccurrenceIndices: Set<Int>,
        classBytes: ByteArray = fixtureBytes,
    ): LocalCaptureResult.Success {
        val result = LocalCaptureExtractor.extract(
            classBytes = classBytes,
            methodName = methodName,
            methodDescriptor = methodDescriptor,
            instructionOccurrenceIndices = instructionOccurrenceIndices,
        )
        return assertIs<LocalCaptureResult.Success>(result)
    }

    private fun candidate(
        slotIndex: Int,
        descriptor: String,
        name: String? = null,
        ordinal: Int? = null,
        isArgument: Boolean,
    ) = LocalCaptureCandidate(
        slotIndex = slotIndex,
        descriptor = descriptor,
        name = name,
        isArgument = isArgument,
        ordinal = ordinal,
    )

    private fun invokeIndex(
        methodName: String,
        methodDescriptor: String,
        owner: String,
        name: String,
        occurrence: Int = 0,
        classBytes: ByteArray = fixtureBytes,
    ): Int {
        val method = loadMethod(classBytes, methodName, methodDescriptor)

        var seen = 0
        var realInstructionIndex = 0
        var insn: AbstractInsnNode? = method.instructions.first
        while (insn != null) {
            if (insn.opcode >= 0) {
                if (insn is MethodInsnNode && insn.owner == owner && insn.name == name) {
                    if (seen == occurrence) {
                        return realInstructionIndex
                    }
                    seen++
                }
                realInstructionIndex++
            }
            insn = insn.next
        }
        error("invoke not found: $owner.$name in $methodName$methodDescriptor")
    }

    private fun insnListIndexOfInvoke(
        classBytes: ByteArray,
        methodName: String,
        methodDescriptor: String,
        owner: String,
        name: String,
        occurrence: Int = 0,
    ): Int {
        val method = loadMethod(classBytes, methodName, methodDescriptor)

        var seen = 0
        var insnListIndex = 0
        var insn: AbstractInsnNode? = method.instructions.first
        while (insn != null) {
            if (insn is MethodInsnNode && insn.owner == owner && insn.name == name) {
                if (seen == occurrence) {
                    return insnListIndex
                }
                seen++
            }
            insnListIndex++
            insn = insn.next
        }
        error("invoke not found: $owner.$name in $methodName$methodDescriptor")
    }

    private fun loadMethod(
        classBytes: ByteArray,
        methodName: String,
        methodDescriptor: String,
    ): org.objectweb.asm.tree.MethodNode {
        val reader = org.objectweb.asm.ClassReader(classBytes)
        val classNode = org.objectweb.asm.tree.ClassNode()
        reader.accept(classNode, org.objectweb.asm.ClassReader.SKIP_FRAMES)
        return classNode.methods.single { it.name == methodName && it.desc == methodDescriptor }
    }

    private fun classBytesWithNamedLocalVariableTable(): ByteArray {
        val classWriter = ClassWriter(ClassWriter.COMPUTE_MAXS)
        classWriter.visit(
            Opcodes.V21,
            Opcodes.ACC_PUBLIC,
            "NamedLocalVariableTable",
            null,
            "java/lang/Object",
            null,
        )

        val methodVisitor = classWriter.visitMethod(
            Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC,
            "namedLocals",
            "(I)I",
            null,
            null,
        )
        methodVisitor.visitCode()
        val start = Label()
        val beforeInvoke = Label()
        val afterInvoke = Label()
        val end = Label()
        methodVisitor.visitLabel(start)
        methodVisitor.visitVarInsn(Opcodes.ILOAD, 0)
        methodVisitor.visitInsn(Opcodes.ICONST_1)
        methodVisitor.visitInsn(Opcodes.IADD)
        methodVisitor.visitVarInsn(Opcodes.ISTORE, 1)
        methodVisitor.visitLabel(beforeInvoke)
        methodVisitor.visitVarInsn(Opcodes.ILOAD, 1)
        methodVisitor.visitMethodInsn(Opcodes.INVOKESTATIC, "java/lang/Math", "abs", "(I)I", false)
        methodVisitor.visitLabel(afterInvoke)
        methodVisitor.visitVarInsn(Opcodes.ISTORE, 2)
        methodVisitor.visitVarInsn(Opcodes.ILOAD, 2)
        methodVisitor.visitInsn(Opcodes.IRETURN)
        methodVisitor.visitLabel(end)
        methodVisitor.visitLocalVariable("arg", "I", null, start, end, 0)
        methodVisitor.visitLocalVariable("early", "I", null, start, afterInvoke, 1)
        methodVisitor.visitLocalVariable("late", "I", null, afterInvoke, end, 2)
        methodVisitor.visitMaxs(2, 3)
        methodVisitor.visitEnd()
        classWriter.visitEnd()
        return classWriter.toByteArray()
    }

    private fun classBytesWithoutLocalVariableTable(): ByteArray {
        val classWriter = ClassWriter(ClassWriter.COMPUTE_MAXS)
        classWriter.visit(
            Opcodes.V21,
            Opcodes.ACC_PUBLIC,
            "NoDebugLocals",
            null,
            "java/lang/Object",
            null,
        )

        val methodVisitor = classWriter.visitMethod(
            Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC,
            "noDebug",
            "(I)I",
            null,
            null,
        )
        methodVisitor.visitCode()
        writeNoDebugBody(methodVisitor)
        methodVisitor.visitMaxs(2, 2)
        methodVisitor.visitEnd()
        classWriter.visitEnd()
        return classWriter.toByteArray()
    }

    private fun writeNoDebugBody(methodVisitor: MethodVisitor) {
        methodVisitor.visitVarInsn(Opcodes.ILOAD, 0)
        methodVisitor.visitInsn(Opcodes.ICONST_1)
        methodVisitor.visitInsn(Opcodes.IADD)
        methodVisitor.visitVarInsn(Opcodes.ISTORE, 1)
        methodVisitor.visitVarInsn(Opcodes.ILOAD, 1)
        methodVisitor.visitMethodInsn(Opcodes.INVOKESTATIC, "java/lang/Math", "abs", "(I)I", false)
        methodVisitor.visitInsn(Opcodes.IRETURN)
    }

    private fun classBytesWithDebugPseudoNodes(): ByteArray {
        val classWriter = ClassWriter(ClassWriter.COMPUTE_MAXS)
        classWriter.visit(
            Opcodes.V21,
            Opcodes.ACC_PUBLIC,
            "DebugPseudoNodes",
            null,
            "java/lang/Object",
            null,
        )

        val methodVisitor = classWriter.visitMethod(
            Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC,
            "withDebug",
            "(I)I",
            null,
            null,
        )
        methodVisitor.visitCode()
        val entry = Label()
        val beforeInvoke = Label()
        methodVisitor.visitLabel(entry)
        methodVisitor.visitLineNumber(10, entry)
        methodVisitor.visitVarInsn(Opcodes.ILOAD, 0)
        methodVisitor.visitInsn(Opcodes.ICONST_1)
        methodVisitor.visitFrame(Opcodes.F_SAME, 0, null, 0, null)
        methodVisitor.visitInsn(Opcodes.IADD)
        methodVisitor.visitVarInsn(Opcodes.ISTORE, 1)
        methodVisitor.visitLabel(beforeInvoke)
        methodVisitor.visitLineNumber(11, beforeInvoke)
        methodVisitor.visitVarInsn(Opcodes.ILOAD, 1)
        methodVisitor.visitMethodInsn(Opcodes.INVOKESTATIC, "java/lang/Math", "abs", "(I)I", false)
        methodVisitor.visitInsn(Opcodes.IRETURN)
        methodVisitor.visitMaxs(2, 2)
        methodVisitor.visitEnd()
        classWriter.visitEnd()
        return classWriter.toByteArray()
    }

    private fun unanalyzableClassBytes(): ByteArray {
        val classWriter = ClassWriter(ClassWriter.COMPUTE_MAXS)
        classWriter.visit(
            Opcodes.V21,
            Opcodes.ACC_PUBLIC,
            "BrokenAnalysis",
            null,
            "java/lang/Object",
            null,
        )

        val methodVisitor = classWriter.visitMethod(
            Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC,
            "broken",
            "()V",
            null,
            null,
        )
        methodVisitor.visitCode()
        methodVisitor.visitInsn(Opcodes.ICONST_1)
        methodVisitor.visitInsn(Opcodes.POP)
        methodVisitor.visitInsn(Opcodes.IADD)
        methodVisitor.visitInsn(Opcodes.RETURN)
        methodVisitor.visitMaxs(1, 0)
        methodVisitor.visitEnd()
        classWriter.visitEnd()
        return classWriter.toByteArray()
    }
}
