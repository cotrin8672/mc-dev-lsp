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
import org.objectweb.asm.tree.MethodInsnNode

class OfficialExpressionMatcherTest {
    @Test
    fun typedLocalDefinitionsResolveOrdinalNameIndexAndArgsOnlyAgainstLiveFrames() {
        val writer = ClassWriter(ClassWriter.COMPUTE_MAXS)
        writer.visit(Opcodes.V21, Opcodes.ACC_PUBLIC, "LocalDefinitions", null, "java/lang/Object", null)
        val method = writer.visitMethod(Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC, "run", "(II)I", null, null)
        val start = Label()
        val stored = Label()
        val end = Label()
        method.visitCode()
        method.visitLabel(start)
        method.visitVarInsn(Opcodes.ILOAD, 0)
        method.visitVarInsn(Opcodes.ILOAD, 1)
        method.visitInsn(Opcodes.IADD)
        method.visitVarInsn(Opcodes.ISTORE, 2)
        method.visitLabel(stored)
        method.visitVarInsn(Opcodes.ILOAD, 2)
        method.visitInsn(Opcodes.IRETURN)
        method.visitLabel(end)
        method.visitLocalVariable("left", "I", null, start, end, 0)
        method.visitLocalVariable("right", "I", null, start, end, 1)
        method.visitLocalVariable("sum", "I", null, stored, end, 2)
        method.visitMaxs(2, 3)
        method.visitEnd()
        writer.visitEnd()
        val cases = listOf(
            HandlerParameterSugarSpec.Local(index = 2) to setOf(3, 4),
            HandlerParameterSugarSpec.Local(ordinal = 2) to setOf(3, 4),
            HandlerParameterSugarSpec.Local(names = setOf("sum")) to setOf(3, 4),
            HandlerParameterSugarSpec.Local(ordinal = 1, argsOnly = true) to setOf(1),
            HandlerParameterSugarSpec.Local(index = 2, argsOnly = true) to emptySet(),
            HandlerParameterSugarSpec.Local() to emptySet(),
        )
        for ((spec, expected) in cases) {
            val built = OfficialExpressionIdentifierPoolBuilder.build(MixinExtrasDefinitionIndex(listOf(
                MixinExtrasDefinition(id = "selected", localSpecs = listOf(spec.copy(typeClassName = "int"))),
            )))
            assertTrue(built.issues.isEmpty())
            val result = matchWithClassBytes(writer.toByteArray(), "run", "(II)I",
                listOf("@(selected)", "selected = ?"), identifierPool = built.pool)
            val actual = assertIs<OfficialExpressionMatchResult.Available>(result, result.toString())
                .matches.map { it.originalInstructionIndex }.toSet()
            assertEquals(expected, actual, spec.toString())
        }
    }
    private val unaryIntConditionalOpcodes = setOf(
        Opcodes.IFEQ,
        Opcodes.IFNE,
        Opcodes.IFLT,
        Opcodes.IFGE,
        Opcodes.IFGT,
        Opcodes.IFLE,
    )

    @Test
    fun capturesArithmeticExpression() {
        val result = match(
            methodName = "add",
            methodDescriptor = "(II)I",
            expressions = listOf("@(?+?)"),
            contextType = OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,
        )

        val available = requireAvailable(result)
        val match = available.matches.single()
        assertEquals(0, match.expressionIndex)
        assertEquals(Opcodes.IADD, match.originalInstructionOpcode)
        assertEquals(OfficialExpressionTypeConstraint.Exact("I"), match.capturedType)
        assertEquals(
            OfficialExpressionMatchDecorationValue.TypeConstraint(OfficialExpressionTypeConstraint.Exact("I")),
            match.decorations[OfficialExpressionMatchDecorationKey.SIMPLE_EXPRESSION_TYPE],
        )
    }

    @Test
    fun capturesArrayLoadExpression() {
        val result = match(
            methodName = "arrayAccess",
            methodDescriptor = "([II)V",
            expressions = listOf("@(?[?])"),
            contextType = OfficialExpressionMatchContextType.WRAP_OPERATION,
        )

        val available = requireAvailable(result)
        val match = available.matches.single()
        assertEquals(Opcodes.IALOAD, match.originalInstructionOpcode)
        assertEquals(
            OfficialExpressionMatchDecorationValue.TypeConstraints(
                listOf(
                    OfficialExpressionTypeConstraint.Exact("[I"),
                    OfficialExpressionTypeConstraint.Exact("I"),
                ),
            ),
            match.decorations[OfficialExpressionMatchDecorationKey.SIMPLE_OPERATION_ARGS],
        )
        assertEquals(
            OfficialExpressionMatchDecorationValue.ParamNames(listOf("array", "index")),
            match.decorations[OfficialExpressionMatchDecorationKey.SIMPLE_OPERATION_PARAM_NAMES],
        )
        assertEquals(
            OfficialExpressionMatchDecorationValue.TypeConstraint(OfficialExpressionTypeConstraint.Exact("I")),
            match.decorations[OfficialExpressionMatchDecorationKey.SIMPLE_OPERATION_RETURN_TYPE],
        )
    }

    @Test
    fun capturesArrayStoreExpression() {
        val result = match(
            methodName = "arrayStore",
            methodDescriptor = "([III)V",
            expressions = listOf("?[?]=?"),
            contextType = OfficialExpressionMatchContextType.WRAP_OPERATION,
        )

        val available = requireAvailable(result)
        val match = available.matches.single()
        assertEquals(Opcodes.IASTORE, match.originalInstructionOpcode)
        assertEquals(
            OfficialExpressionMatchDecorationValue.TypeConstraints(
                listOf(
                    OfficialExpressionTypeConstraint.Exact("[I"),
                    OfficialExpressionTypeConstraint.Exact("I"),
                    OfficialExpressionTypeConstraint.Exact("I"),
                ),
            ),
            match.decorations[OfficialExpressionMatchDecorationKey.SIMPLE_OPERATION_ARGS],
        )
        assertEquals(
            OfficialExpressionMatchDecorationValue.TypeConstraint(OfficialExpressionTypeConstraint.Exact("V")),
            match.decorations[OfficialExpressionMatchDecorationKey.SIMPLE_OPERATION_RETURN_TYPE],
        )
    }

    @Test
    fun capturesStringLiteralExpression() {
        val result = match(
            methodName = "assignString",
            methodDescriptor = "()V",
            expressions = listOf("@('hello')"),
            contextType = OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,
        )

        val available = requireAvailable(result)
        val match = available.matches.single { it.originalInstructionOpcode == Opcodes.LDC }
        assertEquals(
            OfficialExpressionMatchDecorationValue.TypeConstraint(
                OfficialExpressionTypeConstraint.Exact("Ljava/lang/String;"),
            ),
            match.decorations[OfficialExpressionMatchDecorationKey.SIMPLE_EXPRESSION_TYPE],
        )
    }

    @Test
    fun preservesExpressionAndBytecodeOrderForMultipleExpressions() {
        val results = (1..5).map {
            requireAvailable(
                match(
                    methodName = "addAndMultiply",
                    methodDescriptor = "(III)I",
                    expressions = listOf("@(?+?)", "@(?*?)"),
                    contextType = OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,
                ),
            )
        }

        val expectedOrder = results.first().matches.map { match ->
            listOf(
                match.expressionIndex,
                match.originalInstructionIndex,
                match.expressionStartOffset,
                match.originalInstructionOpcode,
            )
        }
        for (result in results) {
            assertEquals(2, result.matches.size)
            assertEquals(expectedOrder, result.matches.map { match ->
                listOf(
                    match.expressionIndex,
                    match.originalInstructionIndex,
                    match.expressionStartOffset,
                    match.originalInstructionOpcode,
                )
            })
        }

        val available = results.first()
        assertEquals(listOf(0, 1), available.matches.map { it.expressionIndex })
        assertEquals(listOf(Opcodes.IADD, Opcodes.IMUL), available.matches.map { it.originalInstructionOpcode })
        assertTrue(available.matches[0].originalInstructionIndex < available.matches[1].originalInstructionIndex)
    }

    @Test
    fun returnsUnavailableForSyntaxFailure() {
        val result = match(
            methodName = "add",
            methodDescriptor = "(II)I",
            expressions = listOf("a +"),
            contextType = OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,
        )

        val unavailable = assertIs<OfficialExpressionMatchResult.Unavailable>(result)
        assertTrue(unavailable.reason.contains("Failed to parse expression"))
        assertTrue(unavailable.cause != null)
    }

    @Test
    fun returnsUnavailableForEmptyClassBytes() {
        val result = matchWithClassBytes(
            classBytes = byteArrayOf(),
            methodName = "add",
            methodDescriptor = "(II)I",
            expressions = listOf("@(?+?)"),
        )

        val unavailable = assertIs<OfficialExpressionMatchResult.Unavailable>(result)
        assertEquals("Failed to parse class bytecode", unavailable.reason)
        assertNotNull(unavailable.cause)
    }

    @Test
    fun returnsUnavailableForTruncatedClassBytes() {
        val validBytes = BytecodeFixtureCompiler.classBytes("ExpressionMatchSamples")
        val result = matchWithClassBytes(
            classBytes = validBytes.copyOf(validBytes.size / 2),
            methodName = "add",
            methodDescriptor = "(II)I",
            expressions = listOf("@(?+?)"),
        )

        val unavailable = assertIs<OfficialExpressionMatchResult.Unavailable>(result)
        assertEquals("Failed to parse class bytecode", unavailable.reason)
        assertNotNull(unavailable.cause)
    }

    @Test
    fun returnsUnavailableForCorruptClassBytes() {
        val result = matchWithClassBytes(
            classBytes = CORRUPT_CLASS_BYTES,
            methodName = "add",
            methodDescriptor = "(II)I",
            expressions = listOf("@(?+?)"),
        )

        val unavailable = assertIs<OfficialExpressionMatchResult.Unavailable>(result)
        assertEquals("Failed to parse class bytecode", unavailable.reason)
        assertNotNull(unavailable.cause)
    }

    @Test
    fun returnsUnavailableForMissingMethod() {
        val result = match(
            methodName = "missing",
            methodDescriptor = "()V",
            expressions = listOf("?"),
            contextType = OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,
        )

        val unavailable = assertIs<OfficialExpressionMatchResult.Unavailable>(result)
        assertEquals("Method not found: missing()V", unavailable.reason)
        assertEquals(null, unavailable.cause)
    }

    @Test
    fun extractsModifyExpressionValueDecorations() {
        val result = match(
            methodName = "add",
            methodDescriptor = "(II)I",
            expressions = listOf("@(?+?)"),
            contextType = OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,
        )

        val match = requireAvailable(result).matches.single()
        assertEquals(
            OfficialExpressionMatchDecorationValue.TypeConstraint(OfficialExpressionTypeConstraint.Exact("I")),
            match.decorations[OfficialExpressionMatchDecorationKey.SIMPLE_EXPRESSION_TYPE],
        )
        assertEquals(null, match.decorations[OfficialExpressionMatchDecorationKey.SIMPLE_OPERATION_ARGS])
    }

    @Test
    fun capturesComparisonVirtualExpansion() {
        val result = match(
            methodName = "intEqualsZero",
            methodDescriptor = "(I)Z",
            expressions = listOf("? == 0"),
            contextType = OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,
        )

        val available = requireAvailable(result)
        val match = available.matches.single()
        assertTrue(
            match.originalInstructionOpcode in unaryIntConditionalOpcodes,
            "expected unary int conditional opcode but got ${match.originalInstructionOpcode}",
        )
        assertTrue(match.originalInstructionIndex >= 0)
        assertEquals(
            OfficialExpressionMatchDecorationValue.TypeConstraint(OfficialExpressionTypeConstraint.Exact("Z")),
            match.decorations[OfficialExpressionMatchDecorationKey.SIMPLE_EXPRESSION_TYPE],
        )
    }

    @Test
    fun capturesComparisonWrapOperation() {
        val binaryIntEqualityConditionalOpcodes = setOf(
            Opcodes.IF_ICMPEQ,
            Opcodes.IF_ICMPNE,
        )

        val result = match(
            methodName = "intComparison",
            methodDescriptor = "(II)Z",
            expressions = listOf("? == ?"),
            contextType = OfficialExpressionMatchContextType.WRAP_OPERATION,
        )

        val available = requireAvailable(result)
        val match = available.matches.single()
        assertTrue(
            match.originalInstructionOpcode in binaryIntEqualityConditionalOpcodes,
            "expected binary int equality conditional opcode but got ${match.originalInstructionOpcode}",
        )
        assertEquals(
            OfficialExpressionMatchDecorationValue.TypeConstraints(
                listOf(
                    OfficialExpressionTypeConstraint.Exact("I"),
                    OfficialExpressionTypeConstraint.Exact("I"),
                ),
            ),
            match.decorations[OfficialExpressionMatchDecorationKey.SIMPLE_OPERATION_ARGS],
        )
        assertEquals(
            OfficialExpressionMatchDecorationValue.TypeConstraint(OfficialExpressionTypeConstraint.Exact("Z")),
            match.decorations[OfficialExpressionMatchDecorationKey.SIMPLE_OPERATION_RETURN_TYPE],
        )
    }

    @Test
    fun capturesStringConcatVirtualExpansion() {
        val result = match(
            methodName = "stringConcat",
            methodDescriptor = "(I)Ljava/lang/String;",
            expressions = listOf("? + ?"),
            contextType = OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,
        )

        val available = requireAvailable(result)
        assertTrue(available.matches.isNotEmpty())
        val match = available.matches.first { it.originalInstructionOpcode == Opcodes.INVOKEDYNAMIC }
        assertEquals(
            OfficialExpressionMatchDecorationValue.TypeConstraint(
                OfficialExpressionTypeConstraint.Exact("Ljava/lang/String;"),
            ),
            match.decorations[OfficialExpressionMatchDecorationKey.SIMPLE_EXPRESSION_TYPE],
        )
    }

    @Test
    fun capturesFieldReadMetadata() {
        val result = match(
            methodName = "readSampleField",
            methodDescriptor = "()I",
            expressions = listOf("@(?.?)"),
            contextType = OfficialExpressionMatchContextType.WRAP_OPERATION,
        )

        val match = requireAvailable(result).matches.single()
        assertEquals(Opcodes.GETFIELD, match.originalInstructionOpcode)
        val metadata = assertIs<OfficialExpressionMatchInstructionMetadata.FieldAccess>(match.instructionMetadata)
        assertEquals("io/github/mcdev/core/bytecode/fixtures/ExpressionMatchSamples", metadata.ownerInternalName)
        assertEquals("sampleField", metadata.name)
        assertEquals("I", metadata.descriptor)
    }

    @Test
    fun capturesFieldWriteMetadata() {
        val result = match(
            methodName = "writeSampleField",
            methodDescriptor = "(I)V",
            expressions = listOf("?.?=?"),
            contextType = OfficialExpressionMatchContextType.WRAP_OPERATION,
        )

        val match = requireAvailable(result).matches.single()
        assertEquals(Opcodes.PUTFIELD, match.originalInstructionOpcode)
        val metadata = assertIs<OfficialExpressionMatchInstructionMetadata.FieldAccess>(match.instructionMetadata)
        assertEquals("io/github/mcdev/core/bytecode/fixtures/ExpressionMatchSamples", metadata.ownerInternalName)
        assertEquals("sampleField", metadata.name)
        assertEquals("I", metadata.descriptor)
    }

    @Test
    fun capturesNewTypeOperationMetadata() {
        val result = match(
            methodName = "newString",
            methodDescriptor = "()Ljava/lang/String;",
            expressions = listOf("@(new ?())"),
            contextType = OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,
        )

        val match = requireAvailable(result).matches.single { it.originalInstructionOpcode == Opcodes.NEW }
        assertEquals(Opcodes.NEW, match.originalInstructionOpcode)
        val metadata = assertIs<OfficialExpressionMatchInstructionMetadata.TypeOperation>(match.instructionMetadata)
        assertEquals("java/lang/String", metadata.typeInternalName)
    }

    @Test
    fun capturesCheckcastTypeOperationMetadata() {
        val result = match(
            methodName = "castString",
            methodDescriptor = "(Ljava/lang/Object;)Ljava/lang/String;",
            expressions = listOf("@((?)?)"),
            contextType = OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,
        )

        val match = requireAvailable(result).matches.single { it.originalInstructionOpcode == Opcodes.CHECKCAST }
        assertEquals(Opcodes.CHECKCAST, match.originalInstructionOpcode)
        val metadata = assertIs<OfficialExpressionMatchInstructionMetadata.TypeOperation>(match.instructionMetadata)
        assertEquals("java/lang/String", metadata.typeInternalName)
    }

    @Test
    fun capturesInstanceofTypeOperationMetadata() {
        val result = match(
            methodName = "isString",
            methodDescriptor = "(Ljava/lang/Object;)Z",
            expressions = listOf("@(? instanceof ?)"),
            contextType = OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,
        )

        val match = requireAvailable(result).matches.single { it.originalInstructionOpcode == Opcodes.INSTANCEOF }
        assertEquals(Opcodes.INSTANCEOF, match.originalInstructionOpcode)
        val metadata = assertIs<OfficialExpressionMatchInstructionMetadata.TypeOperation>(match.instructionMetadata)
        assertEquals("java/lang/String", metadata.typeInternalName)
    }

    @Test
    fun capturesMethodInvocationMetadata() {
        val result = match(
            methodName = "trim",
            methodDescriptor = "(Ljava/lang/String;)Ljava/lang/String;",
            expressions = listOf("@(?.?())"),
            contextType = OfficialExpressionMatchContextType.WRAP_OPERATION,
        )

        val match = requireAvailable(result).matches.single()
        assertEquals(Opcodes.INVOKEVIRTUAL, match.originalInstructionOpcode)
        val metadata = assertIs<OfficialExpressionMatchInstructionMetadata.MethodInvocation>(match.instructionMetadata)
        assertEquals("java/lang/String", metadata.ownerInternalName)
        assertEquals("trim", metadata.name)
        assertEquals("()Ljava/lang/String;", metadata.descriptor)
        assertEquals(false, metadata.isInterface)
        assertEquals(null, match.decorations[OfficialExpressionMatchDecorationKey.SIMPLE_OPERATION_ARGS])
        assertEquals(null, match.decorations[OfficialExpressionMatchDecorationKey.SIMPLE_OPERATION_RETURN_TYPE])
    }

    @Test
    fun matchesDefinedMethodIdentifierFromBuiltPool() {
        val built = OfficialExpressionIdentifierPoolBuilder.build(
            MixinExtrasDefinitionIndex(
                listOf(
                    MixinExtrasDefinition(
                        id = "trimCall",
                        rawMethodReferences = listOf("Ljava/lang/String;trim()Ljava/lang/String;"),
                    ),
                ),
            ),
        )
        assertEquals(emptyList(), built.issues)

        val result = match(
            methodName = "trim",
            methodDescriptor = "(Ljava/lang/String;)Ljava/lang/String;",
            expressions = listOf("@(?.trimCall())"),
            contextType = OfficialExpressionMatchContextType.WRAP_OPERATION,
            identifierPool = built.pool,
        )

        val match = requireAvailable(result).matches.single()
        assertEquals(Opcodes.INVOKEVIRTUAL, match.originalInstructionOpcode)
        val metadata = assertIs<OfficialExpressionMatchInstructionMetadata.MethodInvocation>(match.instructionMetadata)
        assertEquals("java/lang/String", metadata.ownerInternalName)
        assertEquals("trim", metadata.name)
        assertEquals("()Ljava/lang/String;", metadata.descriptor)
    }

    @Test
    fun matchesDefinedFieldIdentifierFromBuiltPool() {
        val built = OfficialExpressionIdentifierPoolBuilder.build(
            MixinExtrasDefinitionIndex(
                listOf(
                    MixinExtrasDefinition(
                        id = "sampleFieldRef",
                        rawFieldReferences = listOf(
                            "Lio/github/mcdev/core/bytecode/fixtures/ExpressionMatchSamples;sampleField:I",
                        ),
                    ),
                ),
            ),
        )
        assertEquals(emptyList(), built.issues)

        val result = match(
            methodName = "readSampleField",
            methodDescriptor = "()I",
            expressions = listOf("@(?.sampleFieldRef)"),
            contextType = OfficialExpressionMatchContextType.WRAP_OPERATION,
            identifierPool = built.pool,
        )

        val match = requireAvailable(result).matches.single()
        assertEquals(Opcodes.GETFIELD, match.originalInstructionOpcode)
        val metadata = assertIs<OfficialExpressionMatchInstructionMetadata.FieldAccess>(match.instructionMetadata)
        assertEquals("io/github/mcdev/core/bytecode/fixtures/ExpressionMatchSamples", metadata.ownerInternalName)
        assertEquals("sampleField", metadata.name)
        assertEquals("I", metadata.descriptor)
    }

    @Test
    fun matchesDefinedTypeIdentifierFromBuiltPool() {
        val built = OfficialExpressionIdentifierPoolBuilder.build(
            MixinExtrasDefinitionIndex(
                listOf(
                    MixinExtrasDefinition(
                        id = "stringType",
                        classLiteralTypeNames = listOf("java.lang.String"),
                    ),
                ),
            ),
        )
        assertEquals(emptyList(), built.issues)

        val result = match(
            methodName = "newString",
            methodDescriptor = "()Ljava/lang/String;",
            expressions = listOf("@(new stringType())"),
            contextType = OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,
            identifierPool = built.pool,
        )

        val match = requireAvailable(result).matches.single { it.originalInstructionOpcode == Opcodes.NEW }
        assertEquals(Opcodes.NEW, match.originalInstructionOpcode)
        val metadata = assertIs<OfficialExpressionMatchInstructionMetadata.TypeOperation>(match.instructionMetadata)
        assertEquals("java/lang/String", metadata.typeInternalName)
    }

    @Test
    fun returnsUnavailableForUndeclaredMemberIdentifier() {
        val result = match(
            methodName = "trim",
            methodDescriptor = "(Ljava/lang/String;)Ljava/lang/String;",
            expressions = listOf("@(?.undeclaredMember())"),
            contextType = OfficialExpressionMatchContextType.WRAP_OPERATION,
        )

        val unavailable = assertIs<OfficialExpressionMatchResult.Unavailable>(result)
        assertTrue(unavailable.reason.contains("expression at index 0"))
        assertNotNull(unavailable.cause)
    }

    @Test
    fun returnsUnavailableForUndeclaredTypeIdentifier() {
        val result = match(
            methodName = "newString",
            methodDescriptor = "()Ljava/lang/String;",
            expressions = listOf("@(new UndeclaredType())"),
            contextType = OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,
        )

        val unavailable = assertIs<OfficialExpressionMatchResult.Unavailable>(result)
        assertTrue(unavailable.reason.contains("expression at index 0"))
        assertNotNull(unavailable.cause)
    }

    @Test
    fun propagatesCancellationFromGuardedExpressionMatches() {
        val cancellation = ExpressionMatchCancellationMarker()

        val thrown = assertFailsWith<ExpressionMatchCancellationMarker> {
            match(
                methodName = "add",
                methodDescriptor = "(II)I",
                expressions = listOf("@(?+?)"),
                contextType = OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,
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
    fun propagatesCancellationFromMatchFailureRecheck() {
        val recheckCancellation = MatchFailureRecheckCancellationMarker()
        var directMatchCancellationChecks = 0

        val thrown = assertFailsWith<MatchFailureRecheckCancellationMarker> {
            match(
                methodName = "trim",
                methodDescriptor = "(Ljava/lang/String;)Ljava/lang/String;",
                expressions = listOf("@(?.undeclaredMember())"),
                contextType = OfficialExpressionMatchContextType.WRAP_OPERATION,
                cancellationChecker = OfficialExpressionCancellationChecker {
                    val fromMatchWithoutFlowGuard = Thread.currentThread().stackTrace.any { frame ->
                        frame.className.endsWith("OfficialExpressionMatcher") &&
                            frame.methodName.contains("match")
                    } && Thread.currentThread().stackTrace.none { frame ->
                        frame.className.endsWith("OfficialExpressionMatcher") &&
                            frame.methodName == "checkFlowCancellation"
                    }
                    if (fromMatchWithoutFlowGuard) {
                        directMatchCancellationChecks++
                        if (directMatchCancellationChecks == 2) {
                            throw recheckCancellation
                        }
                    }
                },
            )
        }

        assertTrue(thrown === recheckCancellation)
    }

    @Test
    fun propagatesCancellationFromFlowInterpreter() {
        val cancellation = FlowInterpreterCancellationMarker()

        val thrown = assertFailsWith<FlowInterpreterCancellationMarker> {
            match(
                methodName = "assignString",
                methodDescriptor = "()V",
                expressions = listOf("?"),
                contextType = OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,
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

    @Test
    fun labelLineAndFrameNodesDoNotShiftPhysicalInstructionIndex() {
        val classBytes = classBytesWithDebugPseudoNodes()
        val expectedPhysicalIndex = physicalInstructionIndexOfOpcode(
            classBytes = classBytes,
            methodName = "withDebug",
            methodDescriptor = "(I)I",
            opcode = Opcodes.IADD,
        )
        val insnListIndex = insnListIndexOfOpcode(
            classBytes = classBytes,
            methodName = "withDebug",
            methodDescriptor = "(I)I",
            opcode = Opcodes.IADD,
        )

        val result = matchWithClassBytes(
            classBytes = classBytes,
            methodName = "withDebug",
            methodDescriptor = "(I)I",
            expressions = listOf("@(?+?)"),
            contextType = OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,
        )

        val match = requireAvailable(result).matches.single()
        assertEquals(Opcodes.IADD, match.originalInstructionOpcode)
        assertEquals(expectedPhysicalIndex, match.originalInstructionIndex)
        assertTrue(insnListIndex > match.originalInstructionIndex)
    }

    @Test
    fun extractsWrapOperationDecorations() {
        val result = match(
            methodName = "arrayAccess",
            methodDescriptor = "([II)V",
            expressions = listOf("@(?[?])"),
            contextType = OfficialExpressionMatchContextType.WRAP_OPERATION,
        )

        val match = requireAvailable(result).matches.single()
        assertTrue(match.decorations.containsKey(OfficialExpressionMatchDecorationKey.SIMPLE_OPERATION_ARGS))
        assertTrue(match.decorations.containsKey(OfficialExpressionMatchDecorationKey.SIMPLE_OPERATION_PARAM_NAMES))
        assertTrue(match.decorations.containsKey(OfficialExpressionMatchDecorationKey.SIMPLE_OPERATION_RETURN_TYPE))
    }

    private fun requireAvailable(result: OfficialExpressionMatchResult): OfficialExpressionMatchResult.Available =
        when (result) {
            is OfficialExpressionMatchResult.Available -> result
            is OfficialExpressionMatchResult.Unavailable -> {
                val causeDetail = result.cause?.let { cause ->
                    "${cause.javaClass.name}: ${cause.message}"
                } ?: "none"
                fail("expected Available but got Unavailable: reason=${result.reason}, cause=$causeDetail")
            }
        }

    private fun match(
        methodName: String,
        methodDescriptor: String,
        expressions: List<String>,
        contextType: OfficialExpressionMatchContextType,
        identifierPool: OfficialExpressionIdentifierPool = OfficialExpressionIdentifierPool.EMPTY,
        cancellationChecker: OfficialExpressionCancellationChecker = OfficialExpressionCancellationChecker.NONE,
    ): OfficialExpressionMatchResult =
        matchWithClassBytes(
            classBytes = BytecodeFixtureCompiler.classBytes("ExpressionMatchSamples"),
            methodName = methodName,
            methodDescriptor = methodDescriptor,
            expressions = expressions,
            contextType = contextType,
            identifierPool = identifierPool,
            cancellationChecker = cancellationChecker,
        )

    private fun matchWithClassBytes(
        classBytes: ByteArray,
        methodName: String,
        methodDescriptor: String,
        expressions: List<String>,
        contextType: OfficialExpressionMatchContextType = OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,
        identifierPool: OfficialExpressionIdentifierPool = OfficialExpressionIdentifierPool.EMPTY,
        cancellationChecker: OfficialExpressionCancellationChecker = OfficialExpressionCancellationChecker.NONE,
    ): OfficialExpressionMatchResult =
        OfficialExpressionMatcher.match(
            classBytes = classBytes,
            methodName = methodName,
            methodDescriptor = methodDescriptor,
            expressions = expressions,
            contextType = contextType,
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

    private class ExpressionMatchCancellationMarker : RuntimeException("expression match cancellation")

    private class MatchFailureRecheckCancellationMarker : RuntimeException("match failure recheck cancellation")

    private class FlowInterpreterCancellationMarker : RuntimeException("flow interpreter cancellation")

    private fun physicalInstructionIndexOfInvoke(
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
                if (insn is MethodInsnNode && insn.owner == owner && insn.name == name) {
                    if (seen == occurrence) {
                        return physicalIndex
                    }
                    seen++
                }
                physicalIndex++
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

    private fun physicalInstructionIndexOfOpcode(
        classBytes: ByteArray,
        methodName: String,
        methodDescriptor: String,
        opcode: Int,
        occurrence: Int = 0,
    ): Int {
        val method = loadMethod(classBytes, methodName, methodDescriptor)

        var seen = 0
        var physicalIndex = 0
        var insn: AbstractInsnNode? = method.instructions.first
        while (insn != null) {
            if (insn.opcode >= 0) {
                if (insn.opcode == opcode) {
                    if (seen == occurrence) {
                        return physicalIndex
                    }
                    seen++
                }
                physicalIndex++
            }
            insn = insn.next
        }
        error("opcode not found: $opcode in $methodName$methodDescriptor")
    }

    private fun insnListIndexOfOpcode(
        classBytes: ByteArray,
        methodName: String,
        methodDescriptor: String,
        opcode: Int,
        occurrence: Int = 0,
    ): Int {
        val method = loadMethod(classBytes, methodName, methodDescriptor)

        var seen = 0
        var insnListIndex = 0
        var insn: AbstractInsnNode? = method.instructions.first
        while (insn != null) {
            if (insn.opcode == opcode) {
                if (seen == occurrence) {
                    return insnListIndex
                }
                seen++
            }
            insnListIndex++
            insn = insn.next
        }
        error("opcode not found: $opcode in $methodName$methodDescriptor")
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
