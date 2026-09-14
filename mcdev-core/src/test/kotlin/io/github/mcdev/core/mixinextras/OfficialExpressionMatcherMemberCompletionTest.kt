package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.bytecode.BytecodeFixtureCompiler
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import kotlin.test.fail
import org.objectweb.asm.ClassWriter
import org.objectweb.asm.Label
import org.objectweb.asm.Opcodes
import org.objectweb.asm.Type
import org.objectweb.asm.tree.AbstractInsnNode
import org.objectweb.asm.tree.FieldInsnNode

class OfficialExpressionMatcherMemberCompletionTest {
    @Test
    fun matchesMembersAfterCompoundOfficialReceivers() {
        val classBytes = classBytesWithCompoundReceivers()

        assertLengthMethodCandidate("method call receiver",
            completeWithClassBytes(
                classBytes = classBytes,
                methodName = "trimLength",
                methodDescriptor = "(Ljava/lang/String;)I",
                receiverExpression = "value.trim()",
            ),
        )
        assertLengthMethodCandidate("this method call receiver",
            completeWithClassBytes(
                classBytes = classBytes,
                methodName = "childLength",
                methodDescriptor = "()I",
                receiverExpression = "this.getChild()",
            ),
        )
        assertLengthMethodCandidate("array receiver",
            completeWithClassBytes(
                classBytes = classBytes,
                methodName = "arrayElementLength",
                methodDescriptor = "([Ljava/lang/String;I)I",
                receiverExpression = "values[index]",
            ),
        )
        assertLengthMethodCandidate("cast receiver",
            completeWithClassBytes(
                classBytes = classBytes,
                methodName = "castLength",
                methodDescriptor = "(Ljava/lang/Object;)I",
                receiverExpression = "(String) value",
            ),
        )
        assertLengthMethodCandidate("nested cast receiver",
            completeWithClassBytes(
                classBytes = classBytes,
                methodName = "nestedCastLength",
                methodDescriptor = "(Ljava/lang/Object;)I",
                receiverExpression = "((String) value)",
            ),
        )
        assertLengthMethodCandidate("parenthesized receiver",
            completeWithClassBytes(
                classBytes = classBytes,
                methodName = "parenthesizedLength",
                methodDescriptor = "(Ljava/lang/String;)I",
                receiverExpression = "(value)",
            ),
        )
    }

    @Test
    fun compoundProbesWildcardOnlyUndeclaredNames() {
        val probes = requireNotNull(OfficialExpressionMemberCompletionProbes.build("value.trim()"))
        assertEquals(
            listOf("@((?.?()).?)", "@((?.?()).?())"),
            probes.map { it.source },
        )

        val arrayProbes = requireNotNull(OfficialExpressionMemberCompletionProbes.build("values[index]"))
        assertEquals(
            listOf("@((?[?]).?)", "@((?[?]).?())"),
            arrayProbes.map { it.source },
        )

        val castProbes = requireNotNull(OfficialExpressionMemberCompletionProbes.build("(String) value"))
        assertEquals(
            listOf("@(((?) ?).?)", "@(((?) ?).?())"),
            castProbes.map { it.source },
        )
    }

    @Test
    fun compoundProbeBuilderRejectsMalformedOrLiteralReceivers() {
        assertEquals(null, OfficialExpressionMemberCompletionProbes.build("value.trim("))
        assertEquals(null, OfficialExpressionMemberCompletionProbes.build("values[index"))
        assertEquals(null, OfficialExpressionMemberCompletionProbes.build("'literal'"))
    }

    @Test
    fun malformedCompoundReceiverNeverProducesBytecodeCandidates() {
        val result = completeWithClassBytes(
            classBytes = classBytesWithCompoundReceivers(),
            methodName = "childLength",
            methodDescriptor = "()I",
            receiverExpression = "this.getChild(",
        )

        val unavailable = assertIs<OfficialExpressionMemberCompletionResult.Unavailable>(result)
        assertTrue(unavailable.reason.contains("cannot be represented in probe syntax"))
    }

    @Test
    fun returnsMethodCandidateForMatchingReceiver() {
        val result = complete(
            methodName = "trim",
            methodDescriptor = "(Ljava/lang/String;)Ljava/lang/String;",
            receiverExpression = "value",
        )

        val available = requireAvailable(result)
        assertEquals(1, available.candidates.size)
        val candidate = assertIs<OfficialExpressionMemberCandidate.MethodInvocation>(available.candidates.single())
        assertEquals("trim", candidate.name)
        assertEquals("()Ljava/lang/String;", candidate.descriptor)
        assertEquals("java/lang/String", candidate.ownerInternalName)
        assertEquals(Opcodes.INVOKEVIRTUAL, candidate.originalInstructionOpcode)
    }

    @Test
    fun returnsCandidatesForUnresolvedReceiverIdentifier() {
        val result = complete(
            methodName = "trim",
            methodDescriptor = "(Ljava/lang/String;)Ljava/lang/String;",
            receiverExpression = "other",
        )

        val available = requireAvailable(result)
        assertEquals(1, available.candidates.size)
        val candidate = assertIs<OfficialExpressionMemberCandidate.MethodInvocation>(available.candidates.single())
        assertEquals("trim", candidate.name)
    }

    @Test
    fun preservesDefinedReceiverIdentifierAndExcludesMismatches() {
        val identifierPool = OfficialExpressionIdentifierPoolBuilder.build(
            MixinExtrasDefinitionIndex(
                listOf(
                    MixinExtrasDefinition(
                        id = "stringValueRef",
                        rawFieldReferences = listOf("Ljava/lang/String;value:[C"),
                    ),
                ),
            ),
        ).pool
        assertTrue(identifierPool.delegate.memberExists("stringValueRef"))

        val exactMismatch = complete(
            methodName = "readSampleField",
            methodDescriptor = "()I",
            receiverExpression = "this.stringValueRef",
            identifierPool = identifierPool,
        )
        assertTrue(requireAvailable(exactMismatch).candidates.isEmpty())

        val unresolvedFallback = complete(
            methodName = "readSampleField",
            methodDescriptor = "()I",
            receiverExpression = "other",
            identifierPool = identifierPool,
        )
        val candidate = assertIs<OfficialExpressionMemberCandidate.FieldAccess>(
            requireAvailable(unresolvedFallback).candidates.single(),
        )
        assertEquals("sampleField", candidate.name)
    }

    @Test
    fun preservesDefinedCallIdentifierAndExcludesOtherReachablePath() {
        val identifierPool = OfficialExpressionIdentifierPoolBuilder.build(
            MixinExtrasDefinitionIndex(
                listOf(
                    MixinExtrasDefinition(
                        id = "getChild",
                        rawMethodReferences = listOf(
                            "LCompoundReceiverSamples;getChildA()Ljava/lang/String;",
                        ),
                    ),
                ),
            ),
        ).pool

        val available = requireAvailable(
            completeWithClassBytes(
                classBytes = classBytesWithCompoundReceivers(),
                methodName = "childMemberChoices",
                methodDescriptor = "()I",
                receiverExpression = "this.getChild()",
                identifierPool = identifierPool,
            ),
        )

        assertEquals(listOf("length"), available.candidates.map { it.name })
    }

    @Test
    fun filtersCandidatesByMemberPrefix() {
        val matching = requireAvailable(
            complete(
                methodName = "trim",
                methodDescriptor = "(Ljava/lang/String;)Ljava/lang/String;",
                receiverExpression = "value",
                memberPrefix = "tr",
            ),
        )
        assertEquals(1, matching.candidates.size)
        assertEquals("trim", matching.candidates.single().name)

        val nonMatching = requireAvailable(
            complete(
                methodName = "trim",
                methodDescriptor = "(Ljava/lang/String;)Ljava/lang/String;",
                receiverExpression = "value",
                memberPrefix = "xyz",
            ),
        )
        assertTrue(nonMatching.candidates.isEmpty())
    }

    @Test
    fun returnsFieldCandidateWithExactMetadata() {
        val result = complete(
            methodName = "readSampleField",
            methodDescriptor = "()I",
            receiverExpression = "this",
        )

        val available = requireAvailable(result)
        val candidate = assertIs<OfficialExpressionMemberCandidate.FieldAccess>(available.candidates.single())
        assertEquals(OfficialExpressionMemberKind.FIELD, candidate.kind)
        assertEquals("io/github/mcdev/core/bytecode/fixtures/ExpressionMatchSamples", candidate.ownerInternalName)
        assertEquals("sampleField", candidate.name)
        assertEquals("I", candidate.descriptor)
        assertEquals(Opcodes.GETFIELD, candidate.originalInstructionOpcode)
        assertTrue(candidate.originalInstructionIndex >= 0)
    }

    @Test
    fun returnsMethodCandidateWithExactMetadata() {
        val result = complete(
            methodName = "trim",
            methodDescriptor = "(Ljava/lang/String;)Ljava/lang/String;",
            receiverExpression = "value",
        )

        val candidate = assertIs<OfficialExpressionMemberCandidate.MethodInvocation>(
            requireAvailable(result).candidates.single(),
        )
        assertEquals(OfficialExpressionMemberKind.METHOD, candidate.kind)
        assertEquals("java/lang/String", candidate.ownerInternalName)
        assertEquals("trim", candidate.name)
        assertEquals("()Ljava/lang/String;", candidate.descriptor)
        assertEquals(false, candidate.isInterface)
        assertEquals(Opcodes.INVOKEVIRTUAL, candidate.originalInstructionOpcode)
        assertTrue(candidate.originalInstructionIndex >= 0)
        assertEquals(null, candidate.decorations[OfficialExpressionMatchDecorationKey.SIMPLE_OPERATION_ARGS])
        assertEquals(null, candidate.decorations[OfficialExpressionMatchDecorationKey.SIMPLE_OPERATION_RETURN_TYPE])
    }

    @Test
    fun deduplicatesCandidatesBySemanticKey() {
        val results = (1..5).map {
            requireAvailable(
                complete(
                    methodName = "readSampleField",
                    methodDescriptor = "()I",
                    receiverExpression = "this",
                ),
            )
        }

        for (available in results) {
            assertEquals(1, available.candidates.size)
            val keys = available.candidates.map { candidate ->
                listOf(
                    candidate.kind,
                    candidate.ownerInternalName,
                    candidate.name,
                    candidate.descriptor,
                )
            }
            assertEquals(keys, keys.distinct())
        }

        val expected = results.first().candidates.map { candidate ->
            listOf(
                candidate.kind,
                candidate.ownerInternalName,
                candidate.name,
                candidate.descriptor,
                candidate.originalInstructionIndex,
                candidate.originalInstructionOpcode,
            )
        }
        for (available in results.drop(1)) {
            assertEquals(
                expected,
                available.candidates.map { candidate ->
                    listOf(
                        candidate.kind,
                        candidate.ownerInternalName,
                        candidate.name,
                        candidate.descriptor,
                        candidate.originalInstructionIndex,
                        candidate.originalInstructionOpcode,
                    )
                },
            )
        }
    }

    @Test
    fun preservesStableBytecodeOrderAcrossRepeatedCalls() {
        val results = (1..5).map {
            requireAvailable(
                complete(
                    methodName = "trim",
                    methodDescriptor = "(Ljava/lang/String;)Ljava/lang/String;",
                    receiverExpression = "value",
                ),
            )
        }

        val expectedOrder = results.first().candidates.map { candidate ->
            listOf(
                candidate.originalInstructionIndex,
                candidate.originalInstructionOpcode,
                candidate.kind,
                candidate.name,
                candidate.descriptor,
            )
        }
        for (available in results) {
            assertEquals(
                expectedOrder,
                available.candidates.map { candidate ->
                    listOf(
                        candidate.originalInstructionIndex,
                        candidate.originalInstructionOpcode,
                        candidate.kind,
                        candidate.name,
                        candidate.descriptor,
                    )
                },
            )
        }
    }

    @Test
    fun labelLineAndFrameNodesDoNotShiftPhysicalInstructionIndex() {
        val classBytes = classBytesWithDebugPseudoNodesAndFieldRead()
        val expectedPhysicalIndex = physicalInstructionIndexOfFieldRead(
            classBytes = classBytes,
            methodName = "readField",
            methodDescriptor = "()I",
            owner = "DebugPseudoField",
            name = "value",
        )
        val insnListIndex = insnListIndexOfFieldRead(
            classBytes = classBytes,
            methodName = "readField",
            methodDescriptor = "()I",
            owner = "DebugPseudoField",
            name = "value",
        )

        val candidate = assertIs<OfficialExpressionMemberCandidate.FieldAccess>(
            requireAvailable(
                completeWithClassBytes(
                    classBytes = classBytes,
                    methodName = "readField",
                    methodDescriptor = "()I",
                    receiverExpression = "this",
                ),
            ).candidates.single(),
        )

        assertEquals(Opcodes.GETFIELD, candidate.originalInstructionOpcode)
        assertEquals(expectedPhysicalIndex, candidate.originalInstructionIndex)
        assertTrue(insnListIndex > candidate.originalInstructionIndex)
    }

    @Test
    fun returnsUnavailableForMalformedReceiver() {
        val result = complete(
            methodName = "trim",
            methodDescriptor = "(Ljava/lang/String;)Ljava/lang/String;",
            receiverExpression = "new",
        )

        val unavailable = assertIs<OfficialExpressionMemberCompletionResult.Unavailable>(result)
        assertTrue(unavailable.reason.contains("cannot be represented in probe syntax"))
    }

    @Test
    fun returnsUnavailableForMissingMethod() {
        val result = complete(
            methodName = "missing",
            methodDescriptor = "()V",
            receiverExpression = "this",
        )

        val unavailable = assertIs<OfficialExpressionMemberCompletionResult.Unavailable>(result)
        assertEquals("Method not found: missing()V", unavailable.reason)
    }

    @Test
    fun returnsUnavailableForEmptyClassBytes() {
        val result = completeWithClassBytes(
            classBytes = byteArrayOf(),
            methodName = "trim",
            methodDescriptor = "(Ljava/lang/String;)Ljava/lang/String;",
            receiverExpression = "value",
        )

        val unavailable = assertIs<OfficialExpressionMemberCompletionResult.Unavailable>(result)
        assertEquals("Failed to parse class bytecode", unavailable.reason)
        assertNotNull(unavailable.cause)
    }

    @Test
    fun returnsUnavailableForCorruptClassBytes() {
        val result = completeWithClassBytes(
            classBytes = CORRUPT_CLASS_BYTES,
            methodName = "trim",
            methodDescriptor = "(Ljava/lang/String;)Ljava/lang/String;",
            receiverExpression = "value",
        )

        val unavailable = assertIs<OfficialExpressionMemberCompletionResult.Unavailable>(result)
        assertEquals("Failed to parse class bytecode", unavailable.reason)
        assertNotNull(unavailable.cause)
    }

    @Test
    fun propagatesCancellationFromGuardedExpressionMatches() {
        val cancellation = MemberCompletionCancellationMarker()

        val thrown = assertFailsWith<MemberCompletionCancellationMarker> {
            complete(
                methodName = "trim",
                methodDescriptor = "(Ljava/lang/String;)Ljava/lang/String;",
                receiverExpression = "value",
                cancellationChecker = OfficialExpressionCancellationChecker {
                    if (Thread.currentThread().stackTrace.any { frame ->
                            frame.className.contains("MatchSinkState") &&
                                (frame.methodName == "reportMatchStatus" ||
                                    frame.methodName == "reportPartialMatch")
                        }
                    ) {
                        throw cancellation
                    }
                },
            )
        }

        assertTrue(thrown === cancellation)
    }

    @Test
    fun propagatesCancellationFromFlowInterpreter() {
        val cancellation = FlowInterpreterCancellationMarker()

        val thrown = assertFailsWith<FlowInterpreterCancellationMarker> {
            complete(
                methodName = "assignString",
                methodDescriptor = "()V",
                receiverExpression = "text",
                cancellationChecker = OfficialExpressionCancellationChecker {
                    if (Thread.currentThread().stackTrace.any { frame ->
                            frame.className.endsWith("CancellableFlowInterpreter") &&
                                frame.methodName == "newOperation"
                        }
                    ) {
                        throw cancellation
                    }
                },
            )
        }

        assertTrue(thrown === cancellation)
    }

    private fun requireAvailable(
        result: OfficialExpressionMemberCompletionResult,
    ): OfficialExpressionMemberCompletionResult.Available =
        when (result) {
            is OfficialExpressionMemberCompletionResult.Available -> result
            is OfficialExpressionMemberCompletionResult.Unavailable -> {
                val causeDetail = result.cause?.let { cause ->
                    "${cause.javaClass.name}: ${cause.message}"
                } ?: "none"
                fail("expected Available but got Unavailable: reason=${result.reason}, cause=$causeDetail")
            }
        }

    private fun assertLengthMethodCandidate(label: String, result: OfficialExpressionMemberCompletionResult) {
        val available = requireAvailable(result)
        assertEquals(1, available.candidates.size, label)
        val candidate = assertIs<OfficialExpressionMemberCandidate.MethodInvocation>(
            available.candidates.single(),
            label,
        )
        assertEquals("java/lang/String", candidate.ownerInternalName)
        assertEquals("length", candidate.name)
        assertEquals("()I", candidate.descriptor)
        assertEquals(Opcodes.INVOKEVIRTUAL, candidate.originalInstructionOpcode)
    }

    private fun complete(
        methodName: String,
        methodDescriptor: String,
        receiverExpression: String,
        memberPrefix: String = "",
        identifierPool: OfficialExpressionIdentifierPool = OfficialExpressionIdentifierPool.EMPTY,
        cancellationChecker: OfficialExpressionCancellationChecker = OfficialExpressionCancellationChecker.NONE,
    ): OfficialExpressionMemberCompletionResult =
        completeWithClassBytes(
            classBytes = BytecodeFixtureCompiler.classBytes("ExpressionMatchSamples"),
            methodName = methodName,
            methodDescriptor = methodDescriptor,
            receiverExpression = receiverExpression,
            memberPrefix = memberPrefix,
            identifierPool = identifierPool,
            cancellationChecker = cancellationChecker,
        )

    private fun completeWithClassBytes(
        classBytes: ByteArray,
        methodName: String,
        methodDescriptor: String,
        receiverExpression: String,
        memberPrefix: String = "",
        identifierPool: OfficialExpressionIdentifierPool = OfficialExpressionIdentifierPool.EMPTY,
        cancellationChecker: OfficialExpressionCancellationChecker = OfficialExpressionCancellationChecker.NONE,
    ): OfficialExpressionMemberCompletionResult =
        OfficialExpressionMatcher.completeReceiverMembers(
            classBytes = classBytes,
            methodName = methodName,
            methodDescriptor = methodDescriptor,
            receiverExpression = receiverExpression,
            memberPrefix = memberPrefix,
            identifierPool = identifierPool,
            commonSuperClass = CommonSuperClassResolver { left, right ->
                val type1 = Type.getType(left)
                val type2 = Type.getType(right)
                when {
                    type1 == type2 -> left
                    type1.sort == Type.OBJECT && type2.sort == Type.OBJECT ->
                        Type.getObjectType("java/lang/Object").descriptor
                    else -> null
                }
            },
            cancellationChecker = cancellationChecker,
        )

    private class MemberCompletionCancellationMarker : RuntimeException("member completion cancellation")

    private class FlowInterpreterCancellationMarker : RuntimeException("flow interpreter cancellation")

    private fun physicalInstructionIndexOfFieldRead(
        classBytes: ByteArray,
        methodName: String,
        methodDescriptor: String,
        owner: String,
        name: String,
        occurrence: Int = 0,
    ): Int {
        val method = loadMethod(classBytes, methodName, methodDescriptor)

        var seen = 0
        var physicalIndex = 0
        var insn: AbstractInsnNode? = method.instructions.first
        while (insn != null) {
            if (insn.opcode >= 0) {
                if (insn is FieldInsnNode && insn.owner == owner && insn.name == name && insn.opcode == Opcodes.GETFIELD) {
                    if (seen == occurrence) {
                        return physicalIndex
                    }
                    seen++
                }
                physicalIndex++
            }
            insn = insn.next
        }
        error("field read not found: $owner.$name in $methodName$methodDescriptor")
    }

    private fun insnListIndexOfFieldRead(
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
            if (insn is FieldInsnNode && insn.owner == owner && insn.name == name && insn.opcode == Opcodes.GETFIELD) {
                if (seen == occurrence) {
                    return insnListIndex
                }
                seen++
            }
            insnListIndex++
            insn = insn.next
        }
        error("field read not found: $owner.$name in $methodName$methodDescriptor")
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

    private fun classBytesWithDebugPseudoNodesAndFieldRead(): ByteArray {
        val classWriter = ClassWriter(ClassWriter.COMPUTE_MAXS)
        classWriter.visit(
            Opcodes.V21,
            Opcodes.ACC_PUBLIC,
            "DebugPseudoField",
            null,
            "java/lang/Object",
            null,
        )
        classWriter.visitField(Opcodes.ACC_PRIVATE, "value", "I", null, null).visitEnd()

        val methodVisitor = classWriter.visitMethod(
            Opcodes.ACC_PUBLIC,
            "readField",
            "()I",
            null,
            null,
        )
        methodVisitor.visitCode()
        val entry = Label()
        val beforeField = Label()
        methodVisitor.visitLabel(entry)
        methodVisitor.visitLineNumber(1, entry)
        methodVisitor.visitVarInsn(Opcodes.ALOAD, 0)
        methodVisitor.visitFrame(Opcodes.F_SAME, 0, null, 0, null)
        methodVisitor.visitLabel(beforeField)
        methodVisitor.visitLineNumber(2, beforeField)
        methodVisitor.visitFieldInsn(Opcodes.GETFIELD, "DebugPseudoField", "value", "I")
        methodVisitor.visitInsn(Opcodes.IRETURN)
        methodVisitor.visitMaxs(1, 1)
        methodVisitor.visitEnd()
        classWriter.visitEnd()
        return classWriter.toByteArray()
    }

    private fun classBytesWithCompoundReceivers(): ByteArray {
        val classWriter = ClassWriter(ClassWriter.COMPUTE_MAXS)
        classWriter.visit(
            Opcodes.V21,
            Opcodes.ACC_PUBLIC,
            "CompoundReceiverSamples",
            null,
            "java/lang/Object",
            null,
        )

        val trimLength = classWriter.visitMethod(
            Opcodes.ACC_PUBLIC,
            "trimLength",
            "(Ljava/lang/String;)I",
            null,
            null,
        )
        trimLength.visitCode()
        trimLength.visitVarInsn(Opcodes.ALOAD, 1)
        trimLength.visitMethodInsn(Opcodes.INVOKEVIRTUAL, "java/lang/String", "trim", "()Ljava/lang/String;", false)
        trimLength.visitMethodInsn(Opcodes.INVOKEVIRTUAL, "java/lang/String", "length", "()I", false)
        trimLength.visitInsn(Opcodes.IRETURN)
        trimLength.visitMaxs(2, 2)
        trimLength.visitEnd()

        val arrayElementLength = classWriter.visitMethod(
            Opcodes.ACC_PUBLIC,
            "arrayElementLength",
            "([Ljava/lang/String;I)I",
            null,
            null,
        )
        arrayElementLength.visitCode()
        arrayElementLength.visitVarInsn(Opcodes.ALOAD, 1)
        arrayElementLength.visitVarInsn(Opcodes.ILOAD, 2)
        arrayElementLength.visitInsn(Opcodes.AALOAD)
        arrayElementLength.visitMethodInsn(Opcodes.INVOKEVIRTUAL, "java/lang/String", "length", "()I", false)
        arrayElementLength.visitInsn(Opcodes.IRETURN)
        arrayElementLength.visitMaxs(2, 3)
        arrayElementLength.visitEnd()

        val castLength = classWriter.visitMethod(
            Opcodes.ACC_PUBLIC,
            "castLength",
            "(Ljava/lang/Object;)I",
            null,
            null,
        )
        castLength.visitCode()
        castLength.visitVarInsn(Opcodes.ALOAD, 1)
        castLength.visitTypeInsn(Opcodes.CHECKCAST, "java/lang/String")
        castLength.visitMethodInsn(Opcodes.INVOKEVIRTUAL, "java/lang/String", "length", "()I", false)
        castLength.visitInsn(Opcodes.IRETURN)
        castLength.visitMaxs(1, 2)
        castLength.visitEnd()

        val nestedCastLength = classWriter.visitMethod(
            Opcodes.ACC_PUBLIC,
            "nestedCastLength",
            "(Ljava/lang/Object;)I",
            null,
            null,
        )
        nestedCastLength.visitCode()
        nestedCastLength.visitVarInsn(Opcodes.ALOAD, 1)
        nestedCastLength.visitTypeInsn(Opcodes.CHECKCAST, "java/lang/String")
        nestedCastLength.visitMethodInsn(Opcodes.INVOKEVIRTUAL, "java/lang/String", "length", "()I", false)
        nestedCastLength.visitInsn(Opcodes.IRETURN)
        nestedCastLength.visitMaxs(1, 2)
        nestedCastLength.visitEnd()

        val getChild = classWriter.visitMethod(
            Opcodes.ACC_PUBLIC,
            "getChild",
            "()Ljava/lang/String;",
            null,
            null,
        )
        getChild.visitCode()
        getChild.visitInsn(Opcodes.ACONST_NULL)
        getChild.visitInsn(Opcodes.ARETURN)
        getChild.visitMaxs(1, 1)
        getChild.visitEnd()

        for (name in listOf("getChildA", "getChildB")) {
            val child = classWriter.visitMethod(
                Opcodes.ACC_PUBLIC,
                name,
                "()Ljava/lang/String;",
                null,
                null,
            )
            child.visitCode()
            child.visitInsn(Opcodes.ACONST_NULL)
            child.visitInsn(Opcodes.ARETURN)
            child.visitMaxs(1, 1)
            child.visitEnd()
        }

        val childLength = classWriter.visitMethod(
            Opcodes.ACC_PUBLIC,
            "childLength",
            "()I",
            null,
            null,
        )
        childLength.visitCode()
        childLength.visitVarInsn(Opcodes.ALOAD, 0)
        childLength.visitMethodInsn(
            Opcodes.INVOKEVIRTUAL,
            "CompoundReceiverSamples",
            "getChild",
            "()Ljava/lang/String;",
            false,
        )
        childLength.visitMethodInsn(Opcodes.INVOKEVIRTUAL, "java/lang/String", "length", "()I", false)
        childLength.visitInsn(Opcodes.IRETURN)
        childLength.visitMaxs(1, 1)
        childLength.visitEnd()

        val childMemberChoices = classWriter.visitMethod(
            Opcodes.ACC_PUBLIC,
            "childMemberChoices",
            "()I",
            null,
            null,
        )
        childMemberChoices.visitCode()
        childMemberChoices.visitVarInsn(Opcodes.ALOAD, 0)
        childMemberChoices.visitMethodInsn(
            Opcodes.INVOKEVIRTUAL,
            "CompoundReceiverSamples",
            "getChildA",
            "()Ljava/lang/String;",
            false,
        )
        childMemberChoices.visitMethodInsn(Opcodes.INVOKEVIRTUAL, "java/lang/String", "length", "()I", false)
        childMemberChoices.visitInsn(Opcodes.POP)
        childMemberChoices.visitVarInsn(Opcodes.ALOAD, 0)
        childMemberChoices.visitMethodInsn(
            Opcodes.INVOKEVIRTUAL,
            "CompoundReceiverSamples",
            "getChildB",
            "()Ljava/lang/String;",
            false,
        )
        childMemberChoices.visitMethodInsn(Opcodes.INVOKEVIRTUAL, "java/lang/String", "hashCode", "()I", false)
        childMemberChoices.visitInsn(Opcodes.IRETURN)
        childMemberChoices.visitMaxs(1, 1)
        childMemberChoices.visitEnd()

        val parenthesizedLength = classWriter.visitMethod(
            Opcodes.ACC_PUBLIC,
            "parenthesizedLength",
            "(Ljava/lang/String;)I",
            null,
            null,
        )
        parenthesizedLength.visitCode()
        parenthesizedLength.visitVarInsn(Opcodes.ALOAD, 1)
        parenthesizedLength.visitMethodInsn(Opcodes.INVOKEVIRTUAL, "java/lang/String", "length", "()I", false)
        parenthesizedLength.visitInsn(Opcodes.IRETURN)
        parenthesizedLength.visitMaxs(1, 2)
        parenthesizedLength.visitEnd()

        classWriter.visitEnd()
        return classWriter.toByteArray()
    }

    private companion object {
        val CORRUPT_CLASS_BYTES = byteArrayOf(
            0xCA.toByte(),
            0xFE.toByte(),
            0xBA.toByte(),
            0xBE.toByte(),
            0x00,
            0x00,
            0x00,
            0x00,
        )
    }
}
