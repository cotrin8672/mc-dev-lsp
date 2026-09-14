package io.github.mcdev.jdtls.mixin

import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FieldIndexEntry
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.mixinextras.ClassLiteralTypeNameResolver
import io.github.mcdev.core.mixinextras.ExpressionContext
import io.github.mcdev.core.mixinextras.HandlerParameterSugarSpec
import io.github.mcdev.core.mixinextras.MixinExtrasDefinition
import io.github.mcdev.core.mixinextras.MixinExtrasDefinitionIndex
import io.github.mcdev.core.mixinextras.MixinExtrasExpression
import io.github.mcdev.core.mixinextras.MixinExtrasExpressionIndex
import io.github.mcdev.core.mixinextras.OfficialExpressionIdentifierPoolBuilder
import io.github.mcdev.core.mixinextras.ResolvedMixinExtrasContext
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue

class JdtMixinExtrasResolvedContextExtractorTest {
    private val handlerRange = McTextRange(McTextPosition(1, 4), McTextPosition(1, 20))

    @Test
    fun returnsNotMixinExtrasHandlerWhenNoRecognizedHandlerBindingExists() {
        val method = method(
            NormalAnnotation(
                fqn = "com.example.ModifyExpressionValue",
                typeName = "ModifyExpressionValue",
                members = mapOf("method" to StringLiteral("draw()V")),
            ),
        )

        assertEquals(
            JdtMixinExtrasResolvedContextResult.NotMixinExtrasHandler,
            JdtMixinExtrasResolvedContextExtractor.extract(method, handlerRange),
        )
    }

    @Test
    fun rejectsHandlerBySimpleNameWhenBindingFqnDoesNotMatchOfficialPackage() {
        val method = method(
            NormalAnnotation(
                fqn = "com.example.fake.ModifyExpressionValue",
                typeName = "ModifyExpressionValue",
                members = emptyMap(),
            ),
        )

        assertEquals(
            JdtMixinExtrasResolvedContextResult.NotMixinExtrasHandler,
            JdtMixinExtrasResolvedContextExtractor.extract(method, handlerRange),
        )
    }

    @Test
    fun returnsUnavailableWhenHandlerBindingIsUnresolved() {
        val method = method(
            NormalAnnotation(
                fqn = null,
                typeName = "ModifyExpressionValue",
                members = emptyMap(),
            ),
        )

        assertEquals(
            JdtMixinExtrasResolvedContextResult.Unavailable("unresolved handler annotation binding"),
            JdtMixinExtrasResolvedContextExtractor.extract(method, handlerRange),
        )
    }

    @Test
    fun returnsUnavailableWhenDirectExpressionBindingIsUnresolvedBesideValidHandler() {
        val method = method(
            handlerAnnotation(),
            NormalAnnotation(
                fqn = null,
                typeName = "Expression",
                members = mapOf("value" to StringLiteral("text.length()")),
            ),
        )

        assertEquals(
            JdtMixinExtrasResolvedContextResult.Unavailable("unresolved context annotation binding"),
            JdtMixinExtrasResolvedContextExtractor.extract(method, handlerRange),
        )
    }

    @Test
    fun returnsUnavailableWhenOneUnresolvedExpressionSiblingExistsBesideResolvedExpression() {
        val method = method(
            handlerAnnotation(),
            SingleMemberAnnotation(
                fqn = EXPRESSION_FQN,
                typeName = "Expression",
                value = StringLiteral("valid"),
            ),
            NormalAnnotation(
                fqn = null,
                typeName = "Expression",
                members = mapOf("value" to StringLiteral("broken")),
            ),
        )

        assertEquals(
            JdtMixinExtrasResolvedContextResult.Unavailable("unresolved context annotation binding"),
            JdtMixinExtrasResolvedContextExtractor.extract(method, handlerRange),
        )
    }

    @Test
    fun returnsUnavailableWhenOfficialHandlerAnnotationIsRecovered() {
        val method = method(
            NormalAnnotation(
                fqn = "com.llamalad7.mixinextras.injector.ModifyExpressionValue",
                typeName = "ModifyExpressionValue",
                members = mapOf("method" to StringLiteral("draw()V")),
                recovered = true,
            ),
        )

        assertEquals(
            JdtMixinExtrasResolvedContextResult.Unavailable("recovered or malformed handler annotation"),
            JdtMixinExtrasResolvedContextExtractor.extract(method, handlerRange),
        )
    }

    @Test
    fun resolvesFoundEmptyHandlerWithEmptyIndexes() {
        val method = method(handlerAnnotation())

        val result = JdtMixinExtrasResolvedContextExtractor.extract(method, handlerRange)

        assertEquals(
            JdtMixinExtrasResolvedContextResult.Resolved(
                ResolvedMixinExtrasContext(
                    handlerRange = handlerRange,
                    context = ExpressionContext(
                        expressionIndex = MixinExtrasExpressionIndex(),
                        definitionIndex = MixinExtrasDefinitionIndex(),
                    ),
                ),
            ),
            result,
        )
    }

    @Test
    fun extractsDirectAndContainerAnnotationsInDeclarationOrderUsingConstantResolution() {
        val method = method(
            handlerAnnotation(),
            NormalAnnotation(
                fqn = DEFINITION_FQN,
                members = mapOf(
                    "id" to StringLiteral("first-def"),
                    "method" to StringLiteral("Lcom/example/Foo;run()V"),
                ),
            ),
            SingleMemberAnnotation(
                fqn = EXPRESSION_FQN,
                value = StringLiteral("text.length()"),
            ),
            NormalAnnotation(
                fqn = EXPRESSIONS_FQN,
                members = mapOf(
                    "value" to ArrayInitializer(
                        NormalAnnotation(
                            fqn = EXPRESSION_FQN,
                            members = mapOf(
                                "id" to StringLiteral("named"),
                                "value" to StringLiteral("(this.a)"),
                            ),
                        ),
                        NormalAnnotation(
                            fqn = EXPRESSION_FQN,
                            members = mapOf("value" to StringLiteral("(this.b)")),
                        ),
                    ),
                ),
            ),
            NormalAnnotation(
                fqn = DEFINITIONS_FQN,
                members = mapOf(
                    "value" to ArrayInitializer(
                        NormalAnnotation(
                            fqn = DEFINITION_FQN,
                            members = mapOf("id" to StringLiteral("second-def")),
                        ),
                        NormalAnnotation(
                            fqn = DEFINITION_FQN,
                            members = mapOf(
                                "id" to StringLiteral("third-def"),
                                "field" to StringLiteral("Lcom/example/Foo;count:I"),
                            ),
                        ),
                    ),
                ),
            ),
        )

        val result = JdtMixinExtrasResolvedContextExtractor.extract(method, handlerRange)
        val resolved = assertIs<JdtMixinExtrasResolvedContextResult.Resolved>(result)

        assertEquals(
            listOf(
                MixinExtrasExpression(values = listOf("text.length()")),
                MixinExtrasExpression(id = "named", values = listOf("(this.a)")),
                MixinExtrasExpression(values = listOf("(this.b)")),
            ),
            resolved.context.context.expressionIndex.expressions,
        )
        assertEquals(
            listOf(
                MixinExtrasDefinition(
                    id = "first-def",
                    rawMethodReferences = listOf("Lcom/example/Foo;run()V"),
                ),
                MixinExtrasDefinition(id = "second-def"),
                MixinExtrasDefinition(
                    id = "third-def",
                    rawFieldReferences = listOf("Lcom/example/Foo;count:I"),
                ),
            ),
            resolved.context.context.definitionIndex.definitions,
        )
    }

    @Test
    fun extractsTypeLiteralDescriptorsIncludingMultidimensionalAndLocalTypes() {
        val method = method(
            handlerAnnotation(),
            NormalAnnotation(
                fqn = DEFINITION_FQN,
                members = mapOf(
                    "id" to StringLiteral("typed"),
                    "type" to ArrayInitializer(
                        TypeLiteral(ClassBinding("java.lang.String")),
                        TypeLiteral(
                            ArrayBinding(
                                ArrayBinding(ClassBinding("java.lang.String"), dims = 1),
                                dims = 1,
                            ),
                        ),
                        TypeLiteral(ClassBinding("com.example.Outer\$Inner")),
                    ),
                ),
            ),
        )

        val result = JdtMixinExtrasResolvedContextExtractor.extract(method, handlerRange)
        val resolved = assertIs<JdtMixinExtrasResolvedContextResult.Resolved>(result)

        assertEquals(
            listOf(
                "Ljava/lang/String;",
                "[[Ljava/lang/String;",
                "Lcom/example/Outer\$Inner;",
            ),
            resolved.context.context.definitionIndex.definitions.single().classLiteralTypeNames,
        )
    }

    @Test
    fun extractsNestedLocalAnnotationMembersFromBindingsAndConstants() {
        val method = method(
            handlerAnnotation(),
            NormalAnnotation(
                fqn = DEFINITION_FQN,
                members = mapOf(
                    "id" to StringLiteral("locals"),
                    "local" to ArrayInitializer(
                        NormalAnnotation(
                            fqn = LOCAL_FQN,
                            members = mapOf(
                                "argsOnly" to BooleanLiteral(true),
                                "index" to IntLiteral(16),
                                "ordinal" to IntLiteral(-1),
                                "name" to ArrayInitializer(
                                    StringLiteral("alpha"),
                                    StringLiteral("beta"),
                                ),
                                "print" to BooleanLiteral(true),
                                "type" to TypeLiteral(ClassBinding("java.lang.String")),
                            ),
                        ),
                        MarkerAnnotation(fqn = LOCAL_FQN),
                    ),
                ),
            ),
        )

        val result = JdtMixinExtrasResolvedContextExtractor.extract(method, handlerRange)
        val resolved = assertIs<JdtMixinExtrasResolvedContextResult.Resolved>(result)

        assertEquals(
            listOf(
                HandlerParameterSugarSpec.Local(
                    argsOnly = true,
                    index = 16,
                    ordinal = null,
                    names = setOf("alpha", "beta"),
                    print = true,
                    typeClassName = "Ljava/lang/String;",
                ),
                HandlerParameterSugarSpec.Local(),
            ),
            resolved.context.context.definitionIndex.definitions.single().localSpecs,
        )
    }

    @Test
    fun extractsJdtPrimitiveAndDescriptorTypesForSourceResolverAndPool() {
        val primitiveNames = listOf(
            "void",
            "boolean",
            "byte",
            "char",
            "short",
            "int",
            "long",
            "float",
            "double",
        )
        val nonVoidPrimitiveNames = primitiveNames.drop(1)
        val typeNodes = primitiveNames.map { name -> TypeLiteral(PrimitiveBinding(name)) } + listOf(
            TypeLiteral(ClassBinding("java.lang.String")),
            TypeLiteral(ArrayBinding(PrimitiveBinding("int"), dims = 1)),
            TypeLiteral(
                ArrayBinding(
                    ArrayBinding(ClassBinding("java.lang.String"), dims = 1),
                    dims = 1,
                ),
            ),
        )
        val localNodes = nonVoidPrimitiveNames.map { name ->
            NormalAnnotation(
                fqn = LOCAL_FQN,
                members = mapOf("type" to TypeLiteral(PrimitiveBinding(name))),
            )
        } + listOf(
            NormalAnnotation(
                fqn = LOCAL_FQN,
                members = mapOf("type" to TypeLiteral(ClassBinding("java.lang.String"))),
            ),
            NormalAnnotation(
                fqn = LOCAL_FQN,
                members = mapOf("type" to TypeLiteral(ArrayBinding(PrimitiveBinding("int"), dims = 1))),
            ),
        )
        val method = method(
            handlerAnnotation(),
            NormalAnnotation(
                fqn = DEFINITION_FQN,
                members = mapOf(
                    "id" to StringLiteral("all-types"),
                    "type" to ArrayInitializer(*typeNodes.toTypedArray()),
                    "local" to ArrayInitializer(*localNodes.toTypedArray()),
                ),
            ),
        )

        val result = JdtMixinExtrasResolvedContextExtractor.extract(method, handlerRange)
        val resolved = assertIs<JdtMixinExtrasResolvedContextResult.Resolved>(result)
        val definition = resolved.context.context.definitionIndex.definitions.single()

        assertEquals(
            primitiveNames + listOf(
                "Ljava/lang/String;",
                "[I",
                "[[Ljava/lang/String;",
            ),
            definition.classLiteralTypeNames,
        )
        assertEquals(
            nonVoidPrimitiveNames + listOf("Ljava/lang/String;", "[I"),
            definition.localSpecs.map { it.typeClassName },
        )

        val build = OfficialExpressionIdentifierPoolBuilder.build(
            resolved.context.context.definitionIndex,
            ClassLiteralTypeNameResolver.forSource(
                source = "import java.lang.String;",
                classIndex = emptyClassIndex(),
            ),
        )
        assertTrue(build.issues.isEmpty(), "unexpected identifier-pool issues: ${build.issues}")
    }

    @Test
    fun retainsDefinitionWhenTypeLiteralBindingIsUnresolvedForDiagnostic() {
        val method = method(
            handlerAnnotation(),
            NormalAnnotation(
                fqn = DEFINITION_FQN,
                members = mapOf(
                    "id" to StringLiteral("typed"),
                    "type" to TypeLiteral(null),
                ),
            ),
        )

        val result = JdtMixinExtrasResolvedContextExtractor.extract(method, handlerRange)
        val resolved = assertIs<JdtMixinExtrasResolvedContextResult.Resolved>(result)

        assertEquals("typed", resolved.context.context.definitionIndex.definitions.single().id)
        val build = OfficialExpressionIdentifierPoolBuilder.build(resolved.context.context.definitionIndex)
        assertEquals(
            listOf("type"),
            build.issues.map { it.attribute },
        )
        assertTrue(build.issues.single().message.contains("expected a class literal"))
    }

    @Test
    fun returnsUnavailableWhenOneSiblingExpressionHasUnresolvedConstant() {
        val method = method(
            handlerAnnotation(),
            SingleMemberAnnotation(
                fqn = EXPRESSION_FQN,
                value = StringLiteral("valid"),
            ),
            NormalAnnotation(
                fqn = EXPRESSION_FQN,
                members = mapOf("value" to StringLiteral(null)),
            ),
        )

        val result = JdtMixinExtrasResolvedContextExtractor.extract(method, handlerRange)

        assertIs<JdtMixinExtrasResolvedContextResult.Unavailable>(result)
        assertTrue((result as JdtMixinExtrasResolvedContextResult.Unavailable).reason.contains("@Expression"))
    }

    @Test
    fun retainsValidDefinitionAndAttributeSiblingsBesideMalformedMembersForDiagnostics() {
        val method = method(
            handlerAnnotation(),
            NormalAnnotation(
                fqn = DEFINITIONS_FQN,
                members = mapOf(
                    "value" to ArrayInitializer(
                        NormalAnnotation(
                            fqn = DEFINITION_FQN,
                            members = mapOf("id" to StringLiteral("ok")),
                        ),
                        NormalAnnotation(
                            fqn = DEFINITION_FQN,
                            members = mapOf(
                                "id" to StringLiteral("broken"),
                                "method" to IntLiteral(123),
                                "field" to StringLiteral("Lcom/example/Foo;kept:I"),
                                "type" to StringLiteral("NotAClassLiteral"),
                                "local" to StringLiteral("not a @Local"),
                            ),
                        ),
                        NormalAnnotation(
                            fqn = DEFINITION_FQN,
                            members = mapOf(
                                "id" to StringLiteral("last"),
                                "field" to StringLiteral("Lcom/example/Foo;count:I"),
                            ),
                        ),
                    ),
                ),
            ),
        )

        val result = JdtMixinExtrasResolvedContextExtractor.extract(method, handlerRange)
        val resolved = assertIs<JdtMixinExtrasResolvedContextResult.Resolved>(result)

        assertEquals(
            listOf("ok", "broken", "last"),
            resolved.context.context.definitionIndex.definitions.map { it.id },
        )
        assertEquals(
            listOf("Lcom/example/Foo;kept:I"),
            resolved.context.context.definitionIndex.definitions[1].rawFieldReferences,
        )
        assertEquals(
            listOf("Lcom/example/Foo;count:I"),
            resolved.context.context.definitionIndex.definitions[2].rawFieldReferences,
        )
        val build = OfficialExpressionIdentifierPoolBuilder.build(resolved.context.context.definitionIndex)
        assertEquals(listOf("method", "type", "local"), build.issues.map { it.attribute })
        assertTrue(build.issues.all { it.message.startsWith("expected") })
    }

    @Test
    fun retainsUsableDefinitionSiblingsWhenDefinitionsArrayContainsNonAnnotationElement() {
        val method = method(
            handlerAnnotation(),
            NormalAnnotation(
                fqn = DEFINITIONS_FQN,
                members = mapOf(
                    "value" to ArrayInitializer(
                        NormalAnnotation(
                            fqn = DEFINITION_FQN,
                            members = mapOf("id" to StringLiteral("first")),
                        ),
                        IntLiteral(123),
                        NormalAnnotation(
                            fqn = DEFINITION_FQN,
                            members = mapOf("id" to StringLiteral("last")),
                        ),
                    ),
                ),
            ),
        )

        val result = JdtMixinExtrasResolvedContextExtractor.extract(method, handlerRange)
        val resolved = assertIs<JdtMixinExtrasResolvedContextResult.Resolved>(result)

        assertEquals(
            listOf("first", null, "last"),
            resolved.context.context.definitionIndex.definitions.map { it.id },
        )
        val build = OfficialExpressionIdentifierPoolBuilder.build(resolved.context.context.definitionIndex)
        assertEquals("id", build.issues.single().attribute)
        assertEquals("expected @Definition annotation", build.issues.single().message)
    }

    @Test
    fun retainsDirectDefinitionMissingIdForDiagnostics() {
        val source = "@Definition(method = \"Lcom/example/Foo;run()V\")"
        val method = method(
            handlerAnnotation(),
            NormalAnnotation(
                fqn = DEFINITION_FQN,
                members = mapOf("method" to StringLiteral("Lcom/example/Foo;run()V")),
                startPosition = 0,
                length = source.length,
            ),
        )

        val result = JdtMixinExtrasResolvedContextExtractor.extract(method, handlerRange, source)
        val resolved = assertIs<JdtMixinExtrasResolvedContextResult.Resolved>(result)

        assertEquals(
            listOf("Lcom/example/Foo;run()V"),
            resolved.context.context.definitionIndex.definitions.single().rawMethodReferences,
        )
        val build = OfficialExpressionIdentifierPoolBuilder.build(resolved.context.context.definitionIndex)
        assertEquals("method", build.issues.single().attribute)
        assertEquals("method definition requires id", build.issues.single().message)
        assertEquals(
            source.indexOf('(') + 1 until source.lastIndexOf(')'),
            build.issues.single().sourceRange,
        )
    }

    @Test
    fun mapsJdtDefinitionParseIssueToOffendingValueRange() {
        val source = "@Definition(id = \"broken\", method = 123)"
        val annotationStart = source.indexOf("@Definition")
        val annotationLength = source.length - annotationStart
        val invalidValueStart = source.indexOf("123")
        val invalidValue = IntLiteral(
            123,
            startPosition = invalidValueStart,
            length = "123".length,
        )
        val method = method(
            handlerAnnotation(),
            NormalAnnotation(
                fqn = DEFINITION_FQN,
                members = mapOf(
                    "id" to StringLiteral("broken"),
                    "method" to invalidValue,
                ),
                startPosition = annotationStart,
                length = annotationLength,
            ),
        )

        val result = JdtMixinExtrasResolvedContextExtractor.extract(method, handlerRange, source)
        val resolved = assertIs<JdtMixinExtrasResolvedContextResult.Resolved>(result)
        val definition = resolved.context.context.definitionIndex.definitions.single()
        val build = OfficialExpressionIdentifierPoolBuilder.build(resolved.context.context.definitionIndex)

        assertEquals(
            source.indexOf('(') + 1 until source.lastIndexOf(')'),
            definition.sourceRange,
        )
        assertEquals(
            invalidValueStart until invalidValueStart + "123".length,
            build.issues.single().sourceRange,
        )
    }

    @Test
    fun recognizesEveryOfficialExpressionHandlerAndRetainsExpressionContext() {
        val handlerFqns = listOf(
            "com.llamalad7.mixinextras.injector.ModifyExpressionValue",
            "com.llamalad7.mixinextras.injector.ModifyReturnValue",
            "com.llamalad7.mixinextras.injector.ModifyReceiver",
            "com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation",
            "com.llamalad7.mixinextras.injector.WrapWithCondition",
            "com.llamalad7.mixinextras.injector.v2.WrapWithCondition",
            "com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod",
            "org.spongepowered.asm.mixin.injection.Inject",
            "org.spongepowered.asm.mixin.injection.Redirect",
            "org.spongepowered.asm.mixin.injection.ModifyArg",
            "org.spongepowered.asm.mixin.injection.ModifyArgs",
            "org.spongepowered.asm.mixin.injection.ModifyVariable",
            "org.spongepowered.asm.mixin.injection.ModifyConstant",
        )

        for (handlerFqn in handlerFqns) {
            val method = method(
                NormalAnnotation(
                    fqn = handlerFqn,
                    typeName = handlerFqn.substringAfterLast('.'),
                    members = mapOf("method" to StringLiteral("draw()V")),
                ),
                SingleMemberAnnotation(
                    fqn = EXPRESSION_FQN,
                    value = StringLiteral("this.value"),
                ),
            )

            val result = JdtMixinExtrasResolvedContextExtractor.extract(method, handlerRange)
            val resolved = assertIs<JdtMixinExtrasResolvedContextResult.Resolved>(result)

            assertEquals(
                listOf(MixinExtrasExpression(values = listOf("this.value"))),
                resolved.context.context.expressionIndex.expressions,
                "handler $handlerFqn should retain its expression context",
            )
        }
    }

    @Test
    fun returnsUnavailableWhenExpressionsContainerMissingValue() {
        val method = method(
            handlerAnnotation(),
            NormalAnnotation(fqn = EXPRESSIONS_FQN, members = emptyMap()),
        )

        val result = JdtMixinExtrasResolvedContextExtractor.extract(method, handlerRange)

        assertEquals(
            JdtMixinExtrasResolvedContextResult.Unavailable("unresolved @Expressions member"),
            result,
        )
    }

    @Test
    fun malformedDefinitionsContainerKeepsExpressionAndReportsItsIssue() {
        val method = method(
            handlerAnnotation(),
            SingleMemberAnnotation(fqn = EXPRESSION_FQN, value = StringLiteral("this.foo")),
            MarkerAnnotation(fqn = DEFINITIONS_FQN),
        )

        val result = JdtMixinExtrasResolvedContextExtractor.extract(method, handlerRange)

        val context = assertIs<JdtMixinExtrasResolvedContextResult.Resolved>(result).context.context
        assertEquals(listOf("this.foo"), context.expressionIndex.expressions.single().values)
        val issues = OfficialExpressionIdentifierPoolBuilder.build(context.definitionIndex).issues
        assertEquals(1, issues.size)
        assertEquals("unresolved @Definitions member", issues.single().message)
    }

    @Test
    fun acceptsExplicitlyEmptyExpressionsContainerValue() {
        val method = method(
            handlerAnnotation(),
            NormalAnnotation(
                fqn = EXPRESSIONS_FQN,
                members = mapOf("value" to ArrayInitializer()),
            ),
        )

        val result = JdtMixinExtrasResolvedContextExtractor.extract(method, handlerRange)
        val resolved = assertIs<JdtMixinExtrasResolvedContextResult.Resolved>(result)

        assertEquals(emptyList(), resolved.context.context.expressionIndex.expressions)
    }

    @Test
    fun returnsUnavailableWhenNormalAnnotationHasUnreadableMemberPair() {
        val method = method(
            handlerAnnotation(),
            NormalAnnotation(
                fqn = EXPRESSION_FQN,
                members = mapOf("value" to StringLiteral("ok")),
                unreadablePair = true,
            ),
        )

        val result = JdtMixinExtrasResolvedContextExtractor.extract(method, handlerRange)

        assertEquals(
            JdtMixinExtrasResolvedContextResult.Unavailable("unresolved @Expression member"),
            result,
        )
    }

    @Test
    fun returnsUnavailableWhenNormalAnnotationHasDuplicateMembers() {
        val method = method(
            handlerAnnotation(),
            NormalAnnotation(
                fqn = EXPRESSION_FQN,
                members = mapOf("value" to StringLiteral("first")),
                duplicateMember = "value",
            ),
        )

        val result = JdtMixinExtrasResolvedContextExtractor.extract(method, handlerRange)

        assertEquals(
            JdtMixinExtrasResolvedContextResult.Unavailable("unresolved @Expression member"),
            result,
        )
    }

    @Test
    fun returnsUnavailableWhenDirectRelevantAnnotationIsRecovered() {
        val method = method(
            handlerAnnotation(),
            NormalAnnotation(
                fqn = EXPRESSION_FQN,
                members = mapOf("value" to StringLiteral("recovered")),
                recovered = true,
            ),
        )

        val result = JdtMixinExtrasResolvedContextExtractor.extract(method, handlerRange)

        assertEquals(
            JdtMixinExtrasResolvedContextResult.Unavailable("unresolved @Expression member"),
            result,
        )
    }

    @Test
    fun ignoresUnrelatedAnnotationsThatShareSimpleNamesWithOfficialOnes() {
        val method = method(
            handlerAnnotation(),
            NormalAnnotation(
                fqn = "com.example.Expression",
                typeName = "Expression",
                members = mapOf("value" to StringLiteral("ignored")),
            ),
            SingleMemberAnnotation(
                fqn = EXPRESSION_FQN,
                value = StringLiteral("kept"),
            ),
        )

        val result = JdtMixinExtrasResolvedContextExtractor.extract(method, handlerRange)
        val resolved = assertIs<JdtMixinExtrasResolvedContextResult.Resolved>(result)

        assertEquals(
            listOf(MixinExtrasExpression(values = listOf("kept"))),
            resolved.context.context.expressionIndex.expressions,
        )
    }

    private fun method(vararg modifiers: Any): MethodDeclaration =
        MethodDeclaration(modifiers.toList())

    private fun emptyClassIndex(): ClassIndex =
        object : ClassIndex {
            override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> = emptyList()

            override fun findClass(internalName: String): ClassIndexEntry? = null

            override fun findClassByFqn(fqn: String): ClassIndexEntry? = null

            override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> = emptyList()

            override fun getFields(ownerInternalName: String): List<FieldIndexEntry> = emptyList()
        }

    private fun handlerAnnotation(): NormalAnnotation =
        NormalAnnotation(
            fqn = "com.llamalad7.mixinextras.injector.ModifyExpressionValue",
            typeName = "ModifyExpressionValue",
            members = mapOf("method" to StringLiteral("draw()V")),
        )

    private class MethodDeclaration(
        private val modifierList: List<Any>,
    ) {
        fun modifiers(): List<Any> = modifierList
    }

    private open class DomNode(
        private val recovered: Boolean = false,
        private val malformed: Boolean = false,
        private val startPosition: Int = -1,
        private val nodeLength: Int = 0,
    ) {
        fun isRecovered(): Boolean = recovered

        fun isMalformed(): Boolean = malformed

        fun getStartPosition(): Int = startPosition

        fun getLength(): Int = nodeLength
    }

    private class TypeBinding(
        private val qualifiedName: String?,
    ) {
        fun getQualifiedName(): String? = qualifiedName

        fun getBinaryName(): String? = qualifiedName
    }

    private class AnnotationBinding(
        private val typeBinding: TypeBinding?,
    ) {
        fun getAnnotationType(): TypeBinding? = typeBinding
    }

    private open class AnnotationNode(
        private val fqn: String?,
        private val typeName: String? = fqn?.substringAfterLast('.'),
        recovered: Boolean = false,
        malformed: Boolean = false,
        startPosition: Int = -1,
        length: Int = 0,
    ) : DomNode(recovered, malformed, startPosition, length) {
        fun resolveAnnotationBinding(): AnnotationBinding? = fqn?.let { AnnotationBinding(TypeBinding(it)) }

        fun getTypeName(): SimpleName = SimpleName(typeName ?: "Unknown")
    }

    private class MarkerAnnotation(
        fqn: String?,
        typeName: String? = fqn?.substringAfterLast('.'),
    ) : AnnotationNode(fqn, typeName)

    private class SingleMemberAnnotation(
        fqn: String?,
        private val value: Any,
        typeName: String? = fqn?.substringAfterLast('.'),
    ) : AnnotationNode(fqn, typeName) {
        fun getValue(): Any = value
    }

    private class NormalAnnotation(
        fqn: String?,
        private val members: Map<String, Any>,
        typeName: String? = fqn?.substringAfterLast('.'),
        private val unreadablePair: Boolean = false,
        private val duplicateMember: String? = null,
        recovered: Boolean = false,
        startPosition: Int = -1,
        length: Int = 0,
    ) : AnnotationNode(fqn, typeName, recovered, startPosition = startPosition, length = length) {
        fun values(): List<Any> {
            val pairs = members.map { (name, value) -> MemberValuePair(name, value) }
            if (unreadablePair) {
                return pairs + UnreadableMemberValuePair()
            }
            if (duplicateMember != null) {
                return pairs + MemberValuePair(duplicateMember, StringLiteral("duplicate"))
            }
            return pairs
        }
    }

    private class MemberValuePair(
        private val name: String,
        private val value: Any,
    ) {
        fun getName(): SimpleName = SimpleName(name)

        fun getValue(): Any = value
    }

    private class UnreadableMemberValuePair {
        fun getName(): SimpleName? = null

        fun getValue(): Any? = null
    }

    private class SimpleName(
        private val identifier: String,
    ) {
        fun getIdentifier(): String = identifier

        override fun toString(): String = identifier
    }

    private class StringLiteral(
        private val constant: String?,
    ) : DomNode() {
        fun resolveConstantExpressionValue(): String? = constant
    }

    private class BooleanLiteral(
        private val constant: Boolean,
    ) : DomNode() {
        fun resolveConstantExpressionValue(): Boolean = constant
    }

    private class IntLiteral(
        private val constant: Int,
        startPosition: Int = -1,
        length: Int = 0,
    ) : DomNode(startPosition = startPosition, nodeLength = length) {
        fun resolveConstantExpressionValue(): Int = constant
    }

    private class ArrayInitializer(
        vararg expressions: Any,
    ) : DomNode() {
        private val values: List<Any> = expressions.toList()

        fun expressions(): List<Any> = values
    }

    private class TypeLiteral(
        private val binding: Any?,
    ) : DomNode() {
        fun getType(): TypeNode = TypeNode(binding)

        fun resolveTypeBinding(): Any = ClassBinding("java.lang.Class")
    }

    private class TypeNode(
        private val binding: Any?,
    ) {
        fun resolveBinding(): Any? = binding
    }

    private class PrimitiveBinding(
        private val name: String,
    ) {
        fun isPrimitive(): Boolean = true

        fun isArray(): Boolean = false

        fun getName(): String = name
    }

    private class ClassBinding(
        private val binaryName: String,
    ) {
        fun isPrimitive(): Boolean = false

        fun isArray(): Boolean = false

        fun getErasure(): Any = this

        fun getBinaryName(): String = binaryName

        fun getQualifiedName(): String = binaryName
    }

    private class ArrayBinding(
        private val component: Any,
        private val dims: Int,
    ) {
        fun isPrimitive(): Boolean = false

        fun isArray(): Boolean = true

        fun getComponentType(): Any = component

        fun getDimensions(): Int = dims
    }

    private companion object {
        private const val EXPRESSION_FQN = "com.llamalad7.mixinextras.expression.Expression"
        private const val EXPRESSIONS_FQN = "com.llamalad7.mixinextras.expression.Expressions"
        private const val DEFINITION_FQN = "com.llamalad7.mixinextras.expression.Definition"
        private const val DEFINITIONS_FQN = "com.llamalad7.mixinextras.expression.Definitions"
        private const val LOCAL_FQN = "com.llamalad7.mixinextras.sugar.Local"
    }
}
