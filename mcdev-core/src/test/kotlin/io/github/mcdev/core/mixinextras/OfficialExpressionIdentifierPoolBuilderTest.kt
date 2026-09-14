package io.github.mcdev.core.mixinextras

import com.llamalad7.mixinextras.expression.impl.flow.FlowValue
import io.github.mcdev.core.descriptor.DescriptorParseResult
import io.github.mcdev.core.descriptor.FieldSelector
import io.github.mcdev.core.descriptor.MethodSelector
import io.github.mcdev.core.descriptor.parseFieldSelector
import io.github.mcdev.core.descriptor.parseMethodSelector
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertTrue
import org.objectweb.asm.Handle
import org.objectweb.asm.Opcodes
import org.objectweb.asm.Type
import org.objectweb.asm.tree.AbstractInsnNode
import org.objectweb.asm.tree.FieldInsnNode
import org.objectweb.asm.tree.InsnNode
import org.objectweb.asm.tree.MethodInsnNode
import org.objectweb.asm.tree.VarInsnNode

class OfficialExpressionIdentifierPoolBuilderTest {
    private val sampleFieldOwner = "io/github/mcdev/core/bytecode/fixtures/ExpressionMatchSamples"
    private val sampleFieldExactSelector =
        "Lio/github/mcdev/core/bytecode/fixtures/ExpressionMatchSamples;sampleField:I"

    @Test
    fun registersTypedLocalDiscriminatorsAndRejectsMissingTypes() {
        val built = OfficialExpressionIdentifierPoolBuilder.build(
            MixinExtrasDefinitionIndex(
                listOf(
                    MixinExtrasDefinition(
                        id = "localRef",
                        localSpecs = listOf(
                            HandlerParameterSugarSpec.Local(index = 3, typeClassName = "int"),
                            HandlerParameterSugarSpec.Local(ordinal = 1, print = true, typeClassName = "int"),
                            HandlerParameterSugarSpec.Local(names = setOf("value"), argsOnly = true, typeClassName = "int"),
                        ),
                    ),
                ),
            ),
        )

        assertTrue(built.issues.isEmpty())
        assertEquals(3, built.pool.locals.size)
        assertTrue(built.pool.delegate.memberExists("localRef"))

        val unsupportedSpecs = listOf(
            HandlerParameterSugarSpec.Local(index = -1),
            HandlerParameterSugarSpec.Local(index = 0, ordinal = 0),
            HandlerParameterSugarSpec.Local(index = 0, names = setOf("value")),
            HandlerParameterSugarSpec.Local(index = 0, typeClassName = "void"),
            HandlerParameterSugarSpec.Local(index = 0, argsOnly = true),
        )
        for (spec in unsupportedSpecs) {
            val unsupported = build(
                MixinExtrasDefinition(
                    id = "localRef",
                    localSpecs = listOf(spec),
                ),
            )
            assertEquals(1, unsupported.issues.size)
            assertEquals("local", unsupported.issues.single().attribute)
            assertTrue(unsupported.issues.single().message.contains("non-void type"))
            assertFalse(unsupported.pool.delegate.memberExists("localRef"))
        }

        val missingId = build(
            MixinExtrasDefinition(
                localSpecs = listOf(
                    HandlerParameterSugarSpec.Local(index = 0),
                    HandlerParameterSugarSpec.Local(index = 1),
                ),
            ),
        )
        assertEquals(listOf("local", "local"), missingId.issues.map { it.attribute })
        assertFalse(missingId.pool.delegate.memberExists("localRef"))
    }

    @Test
    fun registersExactFieldSelector() {
        val built = build(
            MixinExtrasDefinition(
                id = "sampleFieldRef",
                rawFieldReferences = listOf(sampleFieldExactSelector),
            ),
        )

        assertEquals(emptyList(), built.issues)
        assertTrue(
            matchesFieldInsn(
                built,
                "sampleFieldRef",
                FieldInsnNode(
                    Opcodes.GETFIELD,
                    sampleFieldOwner,
                    "sampleField",
                    "I",
                ),
            ),
        )
        assertFalse(
            matchesFieldInsn(
                built,
                "sampleFieldRef",
                FieldInsnNode(
                    Opcodes.GETFIELD,
                    sampleFieldOwner,
                    "otherField",
                    "I",
                ),
            ),
        )
    }

    @Test
    fun matchesOmittedOwnerAndDescriptorForField() {
        val built = build(
            MixinExtrasDefinition(
                id = "sampleFieldRef",
                rawFieldReferences = listOf("sampleField"),
            ),
        )

        assertEquals(emptyList(), built.issues)
        assertTrue(
            matchesFieldInsn(
                built,
                "sampleFieldRef",
                FieldInsnNode(
                    Opcodes.GETFIELD,
                    sampleFieldOwner,
                    "sampleField",
                    "I",
                ),
            ),
        )
        assertTrue(
            matchesFieldInsn(
                built,
                "sampleFieldRef",
                FieldInsnNode(
                    Opcodes.GETSTATIC,
                    "java/lang/Integer",
                    "sampleField",
                    "I",
                ),
            ),
        )
    }

    @Test
    fun appliesTerminalNameStarAsExactNameWithAnyDescriptorForField() {
        val built = build(
            MixinExtrasDefinition(
                id = "sampleFieldLike",
                rawFieldReferences = listOf("sampleField*"),
            ),
        )

        assertEquals(emptyList(), built.issues)
        assertTrue(
            matchesFieldInsn(
                built,
                "sampleFieldLike",
                FieldInsnNode(
                    Opcodes.GETFIELD,
                    sampleFieldOwner,
                    "sampleField",
                    "I",
                ),
            ),
        )
        assertTrue(
            matchesFieldInsn(
                built,
                "sampleFieldLike",
                FieldInsnNode(
                    Opcodes.GETFIELD,
                    sampleFieldOwner,
                    "sampleField",
                    "J",
                ),
            ),
        )
        assertFalse(
            matchesFieldInsn(
                built,
                "sampleFieldLike",
                FieldInsnNode(
                    Opcodes.GETFIELD,
                    sampleFieldOwner,
                    "sampleFieldOther",
                    "I",
                ),
            ),
        )
        assertFalse(
            matchesFieldInsn(
                built,
                "sampleFieldLike",
                FieldInsnNode(
                    Opcodes.GETFIELD,
                    sampleFieldOwner,
                    "otherField",
                    "I",
                ),
            ),
        )
    }

    @Test
    fun combinesMultipleSameIdFieldAlternativesInDeclarationOrder() {
        val built = OfficialExpressionIdentifierPoolBuilder.build(
            MixinExtrasDefinitionIndex(
                listOf(
                    MixinExtrasDefinition(
                        id = "fieldRef",
                        rawFieldReferences = listOf(sampleFieldExactSelector),
                    ),
                    MixinExtrasDefinition(
                        id = "fieldRef",
                        rawFieldReferences = listOf("otherField"),
                    ),
                ),
            ),
        )

        assertEquals(emptyList(), built.issues)
        assertTrue(
            matchesFieldInsn(
                built,
                "fieldRef",
                FieldInsnNode(
                    Opcodes.GETFIELD,
                    sampleFieldOwner,
                    "sampleField",
                    "I",
                ),
            ),
        )
        assertTrue(
            matchesFieldInsn(
                built,
                "fieldRef",
                FieldInsnNode(
                    Opcodes.GETSTATIC,
                    "java/lang/String",
                    "otherField",
                    "Ljava/lang/String;",
                ),
            ),
        )
        assertFalse(
            matchesFieldInsn(
                built,
                "fieldRef",
                FieldInsnNode(
                    Opcodes.GETFIELD,
                    sampleFieldOwner,
                    "missingField",
                    "I",
                ),
            ),
        )
    }

    @Test
    fun reportsMalformedFieldSelectorWithoutRegisteringBroadMatch() {
        val built = OfficialExpressionIdentifierPoolBuilder.build(
            MixinExtrasDefinitionIndex(
                listOf(
                    MixinExtrasDefinition(
                        id = "fieldRef",
                        rawFieldReferences = listOf(
                            "Lcom/example/Foo;run()V",
                            sampleFieldExactSelector,
                        ),
                    ),
                ),
            ),
        )

        assertEquals(
            listOf(
                OfficialExpressionIdentifierPoolBuildIssue(
                    definitionId = "fieldRef",
                    attribute = "field",
                    rawValue = "Lcom/example/Foo;run()V",
                    message = assertIs<DescriptorParseResult.Failure>(parseFieldSelector("Lcom/example/Foo;run()V")).error.message,
                ),
            ),
            built.issues,
        )
        assertTrue(built.pool.delegate.memberExists("fieldRef"))
        assertTrue(
            matchesFieldInsn(
                built,
                "fieldRef",
                FieldInsnNode(
                    Opcodes.GETFIELD,
                    sampleFieldOwner,
                    "sampleField",
                    "I",
                ),
            ),
        )
        assertFalse(
            matchesFieldInsn(
                built,
                "fieldRef",
                FieldInsnNode(
                    Opcodes.GETFIELD,
                    "com/example/Foo",
                    "anything",
                    "I",
                ),
            ),
        )
    }

    @Test
    fun reportsMissingIdForFieldValuesWithoutRegistering() {
        val built = build(
            MixinExtrasDefinition(
                rawFieldReferences = listOf(sampleFieldExactSelector),
            ),
        )

        assertEquals(
            listOf(
                OfficialExpressionIdentifierPoolBuildIssue(
                    definitionId = null,
                    attribute = "field",
                    rawValue = sampleFieldExactSelector,
                    message = "field definition requires id",
                ),
            ),
            built.issues,
        )
        assertFalse(built.pool.delegate.memberExists("sampleFieldRef"))
    }

    @Test
    fun rejectsMethodInsnAndAllHandleTagsForFieldDefinition() {
        val selector = assertIs<DescriptorParseResult.Success<FieldSelector>>(
            parseFieldSelector(sampleFieldExactSelector),
        ).value
        val definition = FieldSelectorMemberDefinition(selector)

        assertTrue(
            definition.matches(
                FieldInsnNode(
                    Opcodes.GETFIELD,
                    sampleFieldOwner,
                    "sampleField",
                    "I",
                ),
            ),
        )
        assertFalse(
            definition.matches(
                MethodInsnNode(
                    Opcodes.INVOKEVIRTUAL,
                    sampleFieldOwner,
                    "sampleField",
                    "()I",
                    false,
                ),
            ),
        )

        val rejectedHandles = listOf(
            Handle(Opcodes.H_INVOKEVIRTUAL, sampleFieldOwner, "sampleField", "()I", false),
            Handle(Opcodes.H_INVOKESTATIC, sampleFieldOwner, "sampleField", "()I", false),
            Handle(Opcodes.H_INVOKESPECIAL, sampleFieldOwner, "sampleField", "()I", false),
            Handle(Opcodes.H_INVOKEINTERFACE, sampleFieldOwner, "sampleField", "()I", false),
            Handle(Opcodes.H_NEWINVOKESPECIAL, sampleFieldOwner, "<init>", "()V", false),
            Handle(Opcodes.H_GETFIELD, sampleFieldOwner, "sampleField", "I", false),
            Handle(Opcodes.H_PUTFIELD, sampleFieldOwner, "sampleField", "I", false),
            Handle(Opcodes.H_GETSTATIC, sampleFieldOwner, "sampleField", "I", false),
            Handle(Opcodes.H_PUTSTATIC, sampleFieldOwner, "sampleField", "I", false),
        )
        for (handle in rejectedHandles) {
            assertFalse(definition.matches(handle), "expected handle tag ${handle.tag} to be rejected")
        }
    }

    @Test
    fun registersExactMethodSelector() {
        val built = build(
            MixinExtrasDefinition(
                id = "trimCall",
                rawMethodReferences = listOf("Ljava/lang/String;trim()Ljava/lang/String;"),
            ),
        )

        assertEquals(emptyList(), built.issues)
        assertTrue(
            matchesMethodInsn(
                built,
                "trimCall",
                MethodInsnNode(
                    Opcodes.INVOKEVIRTUAL,
                    "java/lang/String",
                    "trim",
                    "()Ljava/lang/String;",
                    false,
                ),
            ),
        )
        assertFalse(
            matchesMethodInsn(
                built,
                "trimCall",
                MethodInsnNode(
                    Opcodes.INVOKEVIRTUAL,
                    "java/lang/String",
                    "toLowerCase",
                    "()Ljava/lang/String;",
                    false,
                ),
            ),
        )
    }

    @Test
    fun matchesOmittedOwnerAndDescriptor() {
        val built = build(
            MixinExtrasDefinition(
                id = "trimCall",
                rawMethodReferences = listOf("trim"),
            ),
        )

        assertEquals(emptyList(), built.issues)
        assertTrue(
            matchesMethodInsn(
                built,
                "trimCall",
                MethodInsnNode(
                    Opcodes.INVOKEVIRTUAL,
                    "java/lang/String",
                    "trim",
                    "()Ljava/lang/String;",
                    false,
                ),
            ),
        )
        assertTrue(
            matchesMethodInsn(
                built,
                "trimCall",
                MethodInsnNode(
                    Opcodes.INVOKEVIRTUAL,
                    "java/lang/Integer",
                    "trim",
                    "()V",
                    false,
                ),
            ),
        )
    }

    @Test
    fun appliesTerminalNameStarAsExactNameWithAnyDescriptor() {
        val built = build(
            MixinExtrasDefinition(
                id = "trimLike",
                rawMethodReferences = listOf("trim*"),
            ),
        )

        assertEquals(emptyList(), built.issues)
        assertTrue(
            matchesMethodInsn(
                built,
                "trimLike",
                MethodInsnNode(
                    Opcodes.INVOKEVIRTUAL,
                    "java/lang/String",
                    "trim",
                    "()Ljava/lang/String;",
                    false,
                ),
            ),
        )
        assertTrue(
            matchesMethodInsn(
                built,
                "trimLike",
                MethodInsnNode(
                    Opcodes.INVOKEVIRTUAL,
                    "java/lang/String",
                    "trim",
                    "(Z)Ljava/lang/String;",
                    false,
                ),
            ),
        )
        assertFalse(
            matchesMethodInsn(
                built,
                "trimLike",
                MethodInsnNode(
                    Opcodes.INVOKEVIRTUAL,
                    "java/lang/String",
                    "trimOther",
                    "()Ljava/lang/String;",
                    false,
                ),
            ),
        )
        assertFalse(
            matchesMethodInsn(
                built,
                "trimLike",
                MethodInsnNode(
                    Opcodes.INVOKEVIRTUAL,
                    "java/lang/String",
                    "trimToSize",
                    "()Ljava/lang/String;",
                    false,
                ),
            ),
        )
        assertFalse(
            matchesMethodInsn(
                built,
                "trimLike",
                MethodInsnNode(
                    Opcodes.INVOKEVIRTUAL,
                    "java/lang/String",
                    "toString",
                    "()Ljava/lang/String;",
                    false,
                ),
            ),
        )
    }

    @Test
    fun combinesMultipleSameIdAlternativesInDeclarationOrder() {
        val built = OfficialExpressionIdentifierPoolBuilder.build(
            MixinExtrasDefinitionIndex(
                listOf(
                    MixinExtrasDefinition(
                        id = "call",
                        rawMethodReferences = listOf("Ljava/lang/String;trim()Ljava/lang/String;"),
                    ),
                    MixinExtrasDefinition(
                        id = "call",
                        rawMethodReferences = listOf("Ljava/lang/String;toLowerCase()Ljava/lang/String;"),
                    ),
                ),
            ),
        )

        assertEquals(emptyList(), built.issues)
        assertTrue(
            matchesMethodInsn(
                built,
                "call",
                MethodInsnNode(
                    Opcodes.INVOKEVIRTUAL,
                    "java/lang/String",
                    "trim",
                    "()Ljava/lang/String;",
                    false,
                ),
            ),
        )
        assertTrue(
            matchesMethodInsn(
                built,
                "call",
                MethodInsnNode(
                    Opcodes.INVOKEVIRTUAL,
                    "java/lang/String",
                    "toLowerCase",
                    "()Ljava/lang/String;",
                    false,
                ),
            ),
        )
        assertFalse(
            matchesMethodInsn(
                built,
                "call",
                MethodInsnNode(
                    Opcodes.INVOKEVIRTUAL,
                    "java/lang/String",
                    "substring",
                    "(II)Ljava/lang/String;",
                    false,
                ),
            ),
        )
    }

    @Test
    fun reportsMalformedSelectorWithoutRegisteringBroadMatch() {
        val built = OfficialExpressionIdentifierPoolBuilder.build(
            MixinExtrasDefinitionIndex(
                listOf(
                    MixinExtrasDefinition(
                        id = "call",
                        rawMethodReferences = listOf(
                            "Lcom/example/Foo;run(I)",
                            "Lcom/example/Foo;valid()V",
                        ),
                    ),
                ),
            ),
        )

        assertEquals(
            listOf(
                OfficialExpressionIdentifierPoolBuildIssue(
                    definitionId = "call",
                    attribute = "method",
                    rawValue = "Lcom/example/Foo;run(I)",
                    message = assertIs<DescriptorParseResult.Failure>(parseMethodSelector("Lcom/example/Foo;run(I)")).error.message,
                ),
            ),
            built.issues,
        )
        assertTrue(built.pool.delegate.memberExists("call"))
        assertTrue(
            matchesMethodInsn(
                built,
                "call",
                MethodInsnNode(
                    Opcodes.INVOKEVIRTUAL,
                    "com/example/Foo",
                    "valid",
                    "()V",
                    false,
                ),
            ),
        )
        assertFalse(
            matchesMethodInsn(
                built,
                "call",
                MethodInsnNode(
                    Opcodes.INVOKEVIRTUAL,
                    "com/example/Foo",
                    "anything",
                    "()V",
                    false,
                ),
            ),
        )
    }

    @Test
    fun reportsMissingIdForMethodValuesWithoutRegistering() {
        val built = build(
            MixinExtrasDefinition(
                rawMethodReferences = listOf("Ljava/lang/String;trim()Ljava/lang/String;"),
            ),
        )

        assertEquals(
            listOf(
                OfficialExpressionIdentifierPoolBuildIssue(
                    definitionId = null,
                    attribute = "method",
                    rawValue = "Ljava/lang/String;trim()Ljava/lang/String;",
                    message = "method definition requires id",
                ),
            ),
            built.issues,
        )
        assertFalse(built.pool.delegate.memberExists("trimCall"))
    }

    @Test
    fun matchesAllowedHandleTagsOnly() {
        val selector = assertIs<DescriptorParseResult.Success<MethodSelector>>(
            parseMethodSelector("Ljava/lang/String;trim()Ljava/lang/String;"),
        ).value
        val definition = MethodSelectorMemberDefinition(selector)

        val matchingHandle = Handle(
            Opcodes.H_INVOKEVIRTUAL,
            "java/lang/String",
            "trim",
            "()Ljava/lang/String;",
            false,
        )
        assertTrue(definition.matches(matchingHandle))

        val rejectedHandles = listOf(
            Handle(Opcodes.H_NEWINVOKESPECIAL, "java/lang/String", "<init>", "()V", false),
            Handle(Opcodes.H_GETFIELD, "java/lang/String", "value", "[C", false),
            Handle(Opcodes.H_PUTFIELD, "java/lang/String", "value", "[C", false),
            Handle(Opcodes.H_GETSTATIC, "java/lang/String", "CASE_INSENSITIVE_ORDER", "Ljava/util/Comparator;", false),
            Handle(Opcodes.H_PUTSTATIC, "java/lang/String", "CASE_INSENSITIVE_ORDER", "Ljava/util/Comparator;", false),
        )
        for (handle in rejectedHandles) {
            assertFalse(definition.matches(handle), "expected handle tag ${handle.tag} to be rejected")
        }
    }

    @Test
    fun combinesMultipleMethodValuesForSameDefinitionAsAlternatives() {
        val built = build(
            MixinExtrasDefinition(
                id = "call",
                rawMethodReferences = listOf(
                    "Ljava/lang/String;trim()Ljava/lang/String;",
                    "trim",
                ),
            ),
        )

        assertEquals(emptyList(), built.issues)
        assertTrue(
            matchesMethodInsn(
                built,
                "call",
                MethodInsnNode(
                    Opcodes.INVOKEVIRTUAL,
                    "java/lang/String",
                    "trim",
                    "()Ljava/lang/String;",
                    false,
                ),
            ),
        )
        assertTrue(
            matchesMethodInsn(
                built,
                "call",
                MethodInsnNode(
                    Opcodes.INVOKESTATIC,
                    "java/lang/String",
                    "trim",
                    "()V",
                    false,
                ),
            ),
        )
    }

    @Test
    fun registersExactPrimitiveType() {
        val built = build(
            MixinExtrasDefinition(
                id = "intType",
                classLiteralTypeNames = listOf("int"),
            ),
        )

        assertEquals(emptyList(), built.issues)
        assertTrue(matchesType(built, "intType", Type.INT_TYPE))
        assertFalse(matchesType(built, "intType", Type.LONG_TYPE))
    }

    @Test
    fun registersExactPrimitiveArrayType() {
        val built = build(
            MixinExtrasDefinition(
                id = "intArrayType",
                classLiteralTypeNames = listOf("int[]"),
            ),
        )

        assertEquals(emptyList(), built.issues)
        assertTrue(matchesType(built, "intArrayType", Type.getType("[I")))
        assertFalse(matchesType(built, "intArrayType", Type.INT_TYPE))
    }

    @Test
    fun registersExactFullyQualifiedObjectType() {
        val built = build(
            MixinExtrasDefinition(
                id = "stringType",
                classLiteralTypeNames = listOf("java.lang.String"),
            ),
        )

        assertEquals(emptyList(), built.issues)
        assertTrue(matchesType(built, "stringType", Type.getObjectType("java/lang/String")))
        assertFalse(matchesType(built, "stringType", Type.getObjectType("java/lang/Object")))
    }

    @Test
    fun registersExactNestedBinaryObjectType() {
        val built = build(
            MixinExtrasDefinition(
                id = "nestedType",
                classLiteralTypeNames = listOf("com/example/Outer\$Inner"),
            ),
        )

        assertEquals(emptyList(), built.issues)
        assertTrue(matchesType(built, "nestedType", Type.getObjectType("com/example/Outer\$Inner")))
        assertFalse(matchesType(built, "nestedType", Type.getObjectType("com/example/Outer")))
    }

    @Test
    fun rejectsInjectedResolverReturningMethodType() {
        val built = OfficialExpressionIdentifierPoolBuilder.build(
            MixinExtrasDefinitionIndex(
                listOf(
                    MixinExtrasDefinition(
                        id = "methodType",
                        classLiteralTypeNames = listOf("Runnable.run"),
                    ),
                ),
            ),
            typeNameResolver = ClassLiteralTypeNameResolver { _ ->
                Type.getMethodType("(Ljava/lang/Runnable;)V")
            },
        )

        assertEquals(
            listOf(
                OfficialExpressionIdentifierPoolBuildIssue(
                    definitionId = "methodType",
                    attribute = "type",
                    rawValue = "Runnable.run",
                    message = "invalid resolved type",
                ),
            ),
            built.issues,
        )
        assertFalse(built.pool.delegate.typeExists("methodType"))
    }

    @Test
    fun resolvesSimpleNameWithInjectedResolver() {
        val built = OfficialExpressionIdentifierPoolBuilder.build(
            MixinExtrasDefinitionIndex(
                listOf(
                    MixinExtrasDefinition(
                        id = "stringType",
                        classLiteralTypeNames = listOf("String"),
                    ),
                ),
            ),
            typeNameResolver = ClassLiteralTypeNameResolver { typeName ->
                if (typeName == "String") Type.getObjectType("java/lang/String") else null
            },
        )

        assertEquals(emptyList(), built.issues)
        assertTrue(matchesType(built, "stringType", Type.getObjectType("java/lang/String")))
    }

    @Test
    fun rejectsExactTypeMismatch() {
        val built = build(
            MixinExtrasDefinition(
                id = "stringType",
                classLiteralTypeNames = listOf("java.lang.String"),
            ),
        )

        assertEquals(emptyList(), built.issues)
        assertFalse(matchesType(built, "stringType", Type.getObjectType("java/lang/Integer")))
    }

    @Test
    fun combinesMultipleSameIdTypeAlternativesInDeclarationOrder() {
        val built = OfficialExpressionIdentifierPoolBuilder.build(
            MixinExtrasDefinitionIndex(
                listOf(
                    MixinExtrasDefinition(
                        id = "typeRef",
                        classLiteralTypeNames = listOf("java.lang.String"),
                    ),
                    MixinExtrasDefinition(
                        id = "typeRef",
                        classLiteralTypeNames = listOf("int"),
                    ),
                ),
            ),
        )

        assertEquals(emptyList(), built.issues)
        assertTrue(matchesType(built, "typeRef", Type.getObjectType("java/lang/String")))
        assertTrue(matchesType(built, "typeRef", Type.INT_TYPE))
        assertFalse(matchesType(built, "typeRef", Type.LONG_TYPE))
    }

    @Test
    fun combinesMultipleTypeValuesForSameDefinitionAsAlternatives() {
        val built = build(
            MixinExtrasDefinition(
                id = "typeRef",
                classLiteralTypeNames = listOf("java.lang.String", "int"),
            ),
        )

        assertEquals(emptyList(), built.issues)
        assertTrue(matchesType(built, "typeRef", Type.getObjectType("java/lang/String")))
        assertTrue(matchesType(built, "typeRef", Type.INT_TYPE))
    }

    @Test
    fun reportsMixedValidAndUnresolvedTypeValuesWithoutBlockingValidSiblings() {
        val built = OfficialExpressionIdentifierPoolBuilder.build(
            MixinExtrasDefinitionIndex(
                listOf(
                    MixinExtrasDefinition(
                        id = "typeRef",
                        classLiteralTypeNames = listOf(
                            "String",
                            "int",
                            "Lcom/example/Foo;run()V",
                        ),
                    ),
                ),
            ),
        )

        assertEquals(
            listOf(
                OfficialExpressionIdentifierPoolBuildIssue(
                    definitionId = "typeRef",
                    attribute = "type",
                    rawValue = "String",
                    message = "unresolved type name",
                ),
                OfficialExpressionIdentifierPoolBuildIssue(
                    definitionId = "typeRef",
                    attribute = "type",
                    rawValue = "Lcom/example/Foo;run()V",
                    message = "method descriptor is not a type",
                ),
            ),
            built.issues,
        )
        assertTrue(built.pool.delegate.typeExists("typeRef"))
        assertTrue(matchesType(built, "typeRef", Type.INT_TYPE))
        assertFalse(matchesType(built, "typeRef", Type.getObjectType("java/lang/String")))
    }

    @Test
    fun reportsMissingIdForTypeValuesWithoutRegistering() {
        val built = build(
            MixinExtrasDefinition(
                classLiteralTypeNames = listOf("java.lang.String"),
            ),
        )

        assertEquals(
            listOf(
                OfficialExpressionIdentifierPoolBuildIssue(
                    definitionId = null,
                    attribute = "type",
                    rawValue = "java.lang.String",
                    message = "type definition requires id",
                ),
            ),
            built.issues,
        )
        assertFalse(built.pool.delegate.typeExists("stringType"))
    }

    @Test
    fun reportsInvalidTypeDescriptorWithoutRegistering() {
        val built = build(
            MixinExtrasDefinition(
                id = "typeRef",
                classLiteralTypeNames = listOf("Lcom/example/Foo"),
            ),
        )

        assertEquals(
            listOf(
                OfficialExpressionIdentifierPoolBuildIssue(
                    definitionId = "typeRef",
                    attribute = "type",
                    rawValue = "Lcom/example/Foo",
                    message = "invalid type descriptor",
                ),
            ),
            built.issues,
        )
        assertFalse(built.pool.delegate.typeExists("typeRef"))
    }

    private fun build(definition: MixinExtrasDefinition): OfficialExpressionIdentifierPoolBuildResult =
        OfficialExpressionIdentifierPoolBuilder.build(MixinExtrasDefinitionIndex(listOf(definition)))

    private fun matchesFieldInsn(
        built: OfficialExpressionIdentifierPoolBuildResult,
        id: String,
        insn: FieldInsnNode,
    ): Boolean {
        val fieldType = Type.getType(insn.desc)
        val flowValue = FlowValue(fieldType, insn)
        return built.pool.delegate.matchesMember(id, flowValue)
    }

    private fun matchesMethodInsn(
        built: OfficialExpressionIdentifierPoolBuildResult,
        id: String,
        insn: MethodInsnNode,
    ): Boolean {
        val returnType = Type.getReturnType(insn.desc)
        val flowValue = FlowValue(returnType, insn)
        return built.pool.delegate.matchesMember(id, flowValue)
    }

    private fun matchesInstruction(
        built: OfficialExpressionIdentifierPoolBuildResult,
        id: String,
        insn: AbstractInsnNode,
    ): Boolean = built.pool.delegate.matchesMember(id, FlowValue(Type.INT_TYPE, insn))

    private fun matchesType(
        built: OfficialExpressionIdentifierPoolBuildResult,
        id: String,
        type: Type,
    ): Boolean = built.pool.delegate.matchesType(id, type)
}
