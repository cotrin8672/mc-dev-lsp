package io.github.mcdev.core.mixinextras

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertTrue
import org.objectweb.asm.ClassWriter
import org.objectweb.asm.Label
import org.objectweb.asm.Opcodes
import org.objectweb.asm.Type
import org.objectweb.asm.tree.AbstractInsnNode
import org.objectweb.asm.tree.ClassNode
import org.objectweb.asm.tree.MethodNode
import org.objectweb.asm.tree.VarInsnNode

class LocalCaptureRuntimeSemanticsTest {
    @Test
    fun preservesBooleanByteCharAndShortDescriptorsForLoadsAndStores() {
        val classBytes = typedPrimitiveClassBytes()
        val method = loadMethod(classBytes, "run", "(ZBCS)I")
        val loadIndices = varIndices(method).filter { it.second.opcode == Opcodes.ILOAD && it.second.`var` >= 4 }
        val result = LocalCaptureExtractor.extract(
            classBytes = classBytes,
            methodName = "run",
            methodDescriptor = "(ZBCS)I",
            instructionOccurrenceIndices = loadIndices.map { it.first }.toSet(),
        )

        val candidatesBySlot = assertIs<LocalCaptureResult.Success>(result).snapshots
            .flatMap { it.candidates }
            .associateBy { it.slotIndex }
        assertEquals("Z", candidatesBySlot.getValue(4).descriptor)
        assertEquals("B", candidatesBySlot.getValue(5).descriptor)
        assertEquals("C", candidatesBySlot.getValue(6).descriptor)
        assertEquals("S", candidatesBySlot.getValue(7).descriptor)
    }

    @Test
    fun preservesIntLikeDescriptorsFromTheDeclaredMethodSignatureWithoutLvt() {
        val classBytes = typedPrimitiveArgumentsWithoutLvtClassBytes()
        val method = loadMethod(classBytes, "run", "(ZBCS)I")
        val loadIndices = varIndices(method).filter { it.second.opcode == Opcodes.ILOAD }
        val result = LocalCaptureExtractor.extract(
            classBytes = classBytes,
            methodName = "run",
            methodDescriptor = "(ZBCS)I",
            instructionOccurrenceIndices = loadIndices.map { it.first }.toSet(),
        )

        val candidatesBySlot = assertIs<LocalCaptureResult.Success>(result).snapshots
            .flatMap { it.candidates }
            .associateBy { it.slotIndex }
        assertEquals("Z", candidatesBySlot.getValue(0).descriptor)
        assertEquals("B", candidatesBySlot.getValue(1).descriptor)
        assertEquals("C", candidatesBySlot.getValue(2).descriptor)
        assertEquals("S", candidatesBySlot.getValue(3).descriptor)
    }

    @Test
    fun typedPrimitiveDefinitionsMatchTheRealStoreAndLoadInstructions() {
        val classBytes = typedPrimitiveClassBytes()
        val method = loadMethod(classBytes, "run", "(ZBCS)I")
        val stores = varIndices(method).filter { it.second.opcode == Opcodes.ISTORE && it.second.`var` >= 4 }
        val loads = varIndices(method).filter { it.second.opcode == Opcodes.ILOAD && it.second.`var` >= 4 }
        val expectedTypes = listOf("boolean", "byte", "char", "short")

        for ((offset, typeName) in expectedTypes.withIndex()) {
            val slot = offset + 4
            val expected = setOf(
                stores.single { it.second.`var` == slot }.first,
                loads.single { it.second.`var` == slot }.first,
            )
            val built = OfficialExpressionIdentifierPoolBuilder.build(
                MixinExtrasDefinitionIndex(
                    listOf(
                        MixinExtrasDefinition(
                            id = "selected",
                            localSpecs = listOf(
                                HandlerParameterSugarSpec.Local(
                                    index = slot,
                                    typeClassName = typeName,
                                ),
                            ),
                        ),
                    ),
                ),
            )
            assertTrue(built.issues.isEmpty(), built.issues.toString())

            val result = OfficialExpressionMatcher.match(
                classBytes = classBytes,
                methodName = "run",
                methodDescriptor = "(ZBCS)I",
                expressions = listOf("@(selected)", "selected = ?"),
                contextType = OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,
                identifierPool = built.pool,
                commonSuperClass = trivialCommonSuperClassResolver,
            )
            val actual = assertIs<OfficialExpressionMatchResult.Available>(result, result.toString())
                .matches
                .map { it.originalInstructionIndex }
                .toSet()
            assertEquals(expected, actual, typeName)
        }
    }

    @Test
    fun instanceLocalOrdinalIncludesReceiverButDoesNotSelectIt() {
        val classBytes = sameTypeInstanceClassBytes()
        for ((ordinal, expected) in listOf(0 to emptySet(), 1 to setOf(3, 4))) {
            val built = OfficialExpressionIdentifierPoolBuilder.build(
                MixinExtrasDefinitionIndex(
                    listOf(
                        MixinExtrasDefinition(
                            id = "selected",
                            localSpecs = listOf(
                                HandlerParameterSugarSpec.Local(
                                    ordinal = ordinal,
                                    typeClassName = "SameTypeLocals",
                                ),
                            ),
                        ),
                    ),
                ),
                typeNameResolver = ClassLiteralTypeNameResolver { typeName ->
                    if (typeName == "SameTypeLocals") Type.getObjectType("SameTypeLocals") else null
                },
            )
            assertTrue(built.issues.isEmpty(), built.issues.toString())

            val result = OfficialExpressionMatcher.match(
                classBytes = classBytes,
                methodName = "run",
                methodDescriptor = "()LSameTypeLocals;",
                expressions = listOf("@(selected)", "selected = ?"),
                contextType = OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,
                identifierPool = built.pool,
                commonSuperClass = trivialCommonSuperClassResolver,
            )
            val actual = assertIs<OfficialExpressionMatchResult.Available>(result, result.toString())
                .matches
                .map { it.originalInstructionIndex }
                .toSet()
            assertEquals(expected, actual, "ordinal=$ordinal")
        }
    }

    @Test
    fun analyzerCancellationEscapesAndTheNextRequestStillSucceeds() {
        val classBytes = typedPrimitiveClassBytes()
        val marker = LocalCaptureCancellationMarker()
        val cancellationChecker = OfficialExpressionCancellationChecker {
            if (Thread.currentThread().stackTrace.any { frame ->
                    frame.className.contains("LocalCaptureExtractor") &&
                        frame.className.contains("ExactDescriptorInterpreter")
                }
            ) {
                throw marker
            }
        }

        assertFailsWith<LocalCaptureCancellationMarker> {
            LocalCaptureExtractor.extract(
                classBytes = classBytes,
                methodName = "run",
                methodDescriptor = "(ZBCS)I",
                instructionOccurrenceIndices = setOf(8),
                cancellationChecker = cancellationChecker,
            )
        }

        val built = OfficialExpressionIdentifierPoolBuilder.build(
            MixinExtrasDefinitionIndex(
                listOf(
                    MixinExtrasDefinition(
                        id = "selected",
                        localSpecs = listOf(
                            HandlerParameterSugarSpec.Local(index = 4, typeClassName = "boolean"),
                        ),
                    ),
                ),
            ),
        )
        assertFailsWith<LocalCaptureCancellationMarker> {
            OfficialExpressionMatcher.match(
                classBytes = classBytes,
                methodName = "run",
                methodDescriptor = "(ZBCS)I",
                expressions = listOf("@(selected)"),
                contextType = OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,
                identifierPool = built.pool,
                commonSuperClass = trivialCommonSuperClassResolver,
                cancellationChecker = cancellationChecker,
            )
        }

        val next = LocalCaptureExtractor.extract(
            classBytes = classBytes,
            methodName = "run",
            methodDescriptor = "(ZBCS)I",
            instructionOccurrenceIndices = setOf(8),
        )
        assertIs<LocalCaptureResult.Success>(next)
    }

    private fun typedPrimitiveClassBytes(): ByteArray {
        val writer = ClassWriter(ClassWriter.COMPUTE_MAXS)
        writer.visit(Opcodes.V21, Opcodes.ACC_PUBLIC, "TypedPrimitiveLocals", null, "java/lang/Object", null)
        val method = writer.visitMethod(
            Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC,
            "run",
            "(ZBCS)I",
            null,
            null,
        )
        val start = Label()
        val end = Label()
        method.visitCode()
        method.visitLabel(start)
        for (slot in 0..3) {
            method.visitVarInsn(Opcodes.ILOAD, slot)
            method.visitVarInsn(Opcodes.ISTORE, slot + 4)
        }
        method.visitVarInsn(Opcodes.ILOAD, 4)
        method.visitVarInsn(Opcodes.ILOAD, 5)
        method.visitInsn(Opcodes.IADD)
        method.visitVarInsn(Opcodes.ILOAD, 6)
        method.visitInsn(Opcodes.IADD)
        method.visitVarInsn(Opcodes.ILOAD, 7)
        method.visitInsn(Opcodes.IADD)
        method.visitInsn(Opcodes.IRETURN)
        method.visitLabel(end)
        method.visitLocalVariable("bool", "Z", null, start, end, 0)
        method.visitLocalVariable("byteValue", "B", null, start, end, 1)
        method.visitLocalVariable("charValue", "C", null, start, end, 2)
        method.visitLocalVariable("shortValue", "S", null, start, end, 3)
        method.visitLocalVariable("localBool", "Z", null, start, end, 4)
        method.visitLocalVariable("localByte", "B", null, start, end, 5)
        method.visitLocalVariable("localChar", "C", null, start, end, 6)
        method.visitLocalVariable("localShort", "S", null, start, end, 7)
        method.visitMaxs(2, 8)
        method.visitEnd()
        writer.visitEnd()
        return writer.toByteArray()
    }

    private fun sameTypeInstanceClassBytes(): ByteArray {
        val writer = ClassWriter(ClassWriter.COMPUTE_MAXS)
        writer.visit(Opcodes.V21, Opcodes.ACC_PUBLIC, "SameTypeLocals", null, "java/lang/Object", null)
        val constructor = writer.visitMethod(Opcodes.ACC_PUBLIC, "<init>", "()V", null, null)
        constructor.visitCode()
        constructor.visitVarInsn(Opcodes.ALOAD, 0)
        constructor.visitMethodInsn(Opcodes.INVOKESPECIAL, "java/lang/Object", "<init>", "()V", false)
        constructor.visitInsn(Opcodes.RETURN)
        constructor.visitMaxs(1, 1)
        constructor.visitEnd()
        val method = writer.visitMethod(
            Opcodes.ACC_PUBLIC,
            "run",
            "()LSameTypeLocals;",
            null,
            null,
        )
        val start = Label()
        val end = Label()
        method.visitCode()
        method.visitLabel(start)
        method.visitTypeInsn(Opcodes.NEW, "SameTypeLocals")
        method.visitInsn(Opcodes.DUP)
        method.visitMethodInsn(Opcodes.INVOKESPECIAL, "SameTypeLocals", "<init>", "()V", false)
        method.visitVarInsn(Opcodes.ASTORE, 1)
        method.visitVarInsn(Opcodes.ALOAD, 1)
        method.visitInsn(Opcodes.ARETURN)
        method.visitLabel(end)
        method.visitLocalVariable("this", "LSameTypeLocals;", null, start, end, 0)
        method.visitLocalVariable("value", "LSameTypeLocals;", null, start, end, 1)
        method.visitMaxs(2, 2)
        method.visitEnd()
        writer.visitEnd()
        return writer.toByteArray()
    }

    private fun typedPrimitiveArgumentsWithoutLvtClassBytes(): ByteArray {
        val writer = ClassWriter(ClassWriter.COMPUTE_MAXS)
        writer.visit(Opcodes.V21, Opcodes.ACC_PUBLIC, "TypedPrimitiveArguments", null, "java/lang/Object", null)
        val method = writer.visitMethod(
            Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC,
            "run",
            "(ZBCS)I",
            null,
            null,
        )
        method.visitCode()
        method.visitVarInsn(Opcodes.ILOAD, 0)
        method.visitVarInsn(Opcodes.ILOAD, 1)
        method.visitInsn(Opcodes.IADD)
        method.visitVarInsn(Opcodes.ILOAD, 2)
        method.visitInsn(Opcodes.IADD)
        method.visitVarInsn(Opcodes.ILOAD, 3)
        method.visitInsn(Opcodes.IADD)
        method.visitInsn(Opcodes.IRETURN)
        method.visitMaxs(2, 4)
        method.visitEnd()
        writer.visitEnd()
        return writer.toByteArray()
    }

    private fun loadMethod(classBytes: ByteArray, name: String, descriptor: String): MethodNode {
        val classNode = ClassNode()
        org.objectweb.asm.ClassReader(classBytes).accept(classNode, org.objectweb.asm.ClassReader.SKIP_FRAMES)
        return classNode.methods.single { it.name == name && it.desc == descriptor }
    }

    private fun varIndices(method: MethodNode): List<Pair<Int, VarInsnNode>> {
        val indices = mutableListOf<Pair<Int, VarInsnNode>>()
        var realIndex = 0
        var instruction: AbstractInsnNode? = method.instructions.first
        while (instruction != null) {
            if (instruction.opcode >= 0) {
                if (instruction is VarInsnNode) indices += realIndex to instruction
                realIndex++
            }
            instruction = instruction.next
        }
        return indices
    }

    private companion object {
        val trivialCommonSuperClassResolver = CommonSuperClassResolver { left, right ->
            if (left == right) left else "Ljava/lang/Object;"
        }
    }

    private class LocalCaptureCancellationMarker : IllegalStateException()
}
