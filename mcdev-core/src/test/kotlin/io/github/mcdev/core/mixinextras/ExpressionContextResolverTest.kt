package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.bytecode.BytecodeFixtureCompiler
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.AtTargetKind
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FakeBytecodeIndex
import io.github.mcdev.core.mixin.FakeClassIndex
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.model.MappingNamespace
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.objectweb.asm.ClassWriter
import org.objectweb.asm.Label
import org.objectweb.asm.Opcodes
import org.objectweb.asm.Type

class ExpressionContextResolverTest {
    @Test
    fun infersInvokeTypeFromFullyQualifiedReceiver() {
        val inferred = ExpressionContextResolver.inferFromExpression(
            expression = "java.lang.String.length()",
            ownerInternalName = "com/example/target/SimpleTarget",
            targetMethod = MethodIndexEntry("draw", "(Ljava/lang/String;FF)V", false, "draw(String, float, float): void"),
            bytecodeIndex = MixinExtrasTestFixtures.bytecodeIndex,
            classIndex = MixinExtrasTestFixtures.classIndex,
        )

        assertEquals("I", inferred)
    }

    @Test
    fun fullyQualifiedReceiverDisambiguatesSameNamedInvokes() {
        val classIndex = FakeClassIndex(
            classes = FakeClassIndex.defaultClasses() + listOf(
                ClassIndexEntry("First", "com.example.target", "com/example/target/First"),
                ClassIndexEntry("Second", "com.example.target", "com/example/target/Second"),
            ),
        )
        val bytecodeIndex = FakeBytecodeIndex(
            candidates = mapOf(
                "com/example/target/SimpleTarget#draw#INVOKE" to listOf(
                    AtTargetCandidate(
                        owner = "com/example/target/First",
                        name = "value",
                        descriptor = "()F",
                        displayLabel = "value(): float",
                        detail = "First",
                        kind = AtTargetKind.INVOKE,
                        namespace = MappingNamespace.NAMED,
                    ),
                    AtTargetCandidate(
                        owner = "com/example/target/Second",
                        name = "value",
                        descriptor = "()I",
                        displayLabel = "value(): int",
                        detail = "Second",
                        kind = AtTargetKind.INVOKE,
                        namespace = MappingNamespace.NAMED,
                    ),
                ),
            ),
        )

        val inferred = ExpressionContextResolver.inferFromExpression(
            expression = "com.example.target.Second.value()",
            ownerInternalName = "com/example/target/SimpleTarget",
            targetMethod = MethodIndexEntry("draw", "()V", false, "draw(): void"),
            bytecodeIndex = bytecodeIndex,
            classIndex = classIndex,
        )

        assertEquals("I", inferred)
    }

    @Test
    fun handlerRegionPreservesFullBlockUpToHandlerStart() {
        val source = """
            @Mixin(SimpleTarget.class)
            abstract class ExampleMixin {
                @Definition(id = "tail")@ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
                private int mcdev${'$'}handler(int original) { return original; }
            }
        """.trimIndent()
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val handlerStart = handlerStartOffset(source, site)
        val blockStart = source.indexOf("@Definition")
        val expected = source.substring(blockStart, handlerStart)

        assertEquals(expected, ExpressionContextResolver.handlerRegion(source, site))
        assertEquals(source[handlerStart - 1], expected.last())
        assertEquals(')', expected.trimEnd().last())
    }

    @Test
    fun pairsExpressionBeforeInjectorAnnotation() {
        val source = expressionHandlerSource(
            prefix = """@Expression("text.length()")""",
            at = """@At(value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = "",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()

        val context = ExpressionContextResolver.resolveExpressionContext(source, site)

        assertEquals(listOf("text.length()"), context.expressionValuesForAtId(site.atId))
    }

    @Test
    fun pairsNamedExpressionByMatchingAtId() {
        val source = expressionHandlerSource(
            prefix = "",
            at = """@At(id = "main", value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = """@Expression(id = "main", value = "text.length()")""",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()

        val context = ExpressionContextResolver.resolveExpressionContext(source, site)

        assertEquals("main", site.atId)
        assertEquals(listOf("text.length()"), context.expressionValuesForAtId(site.atId))
    }

    @Test
    fun ignoresMismatchedExpressionSibling() {
        val source = expressionHandlerSource(
            prefix = """@Expression(id = "other", value = "other.length()")""",
            at = """@At(id = "main", value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = "",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()

        val context = ExpressionContextResolver.resolveExpressionContext(source, site)

        assertTrue(context.expressionValuesForAtId(site.atId).isEmpty())
    }

    @Test
    fun retainsAllMatchingExpressionValuesWhenMultipleShareId() {
        val source = expressionHandlerSource(
            prefix = """@Expression(id = "main", value = "wrong.length()")""",
            at = """@At(id = "main", value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = """@Expression(id = "main", value = "text.length()")""",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()

        val context = ExpressionContextResolver.resolveExpressionContext(source, site)

        assertEquals(
            listOf("wrong.length()", "text.length()"),
            context.expressionValuesForAtId(site.atId),
        )
    }

    @Test
    fun preservesInterleavedStandaloneAndContainerOrder() {
        val source = expressionHandlerSource(
            prefix = """
                @Expression(id = "main", value = "first.length()")
                @Expressions(value = {
                    @Expression(id = "main", value = "second.length()"),
                    @Expression(id = "other", value = "other.length()")
                })
                @Expression(id = "main", value = "third.length()")
            """.trimIndent(),
            at = """@At(id = "main", value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = "",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()

        val context = ExpressionContextResolver.resolveExpressionContext(source, site)

        assertEquals(
            listOf("first.length()", "second.length()", "third.length()"),
            context.expressionValuesForAtId("main"),
        )
        assertEquals(listOf("other.length()"), context.expressionValuesForAtId("other"))
    }

    @Test
    fun flattensExpressionArrayValuesInDocumentOrder() {
        val source = expressionHandlerSource(
            prefix = """@Expression(id = "main", value = { "one.length()", "two.length()" })""",
            at = """@At(id = "main", value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = "",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()

        val context = ExpressionContextResolver.resolveExpressionContext(source, site)

        assertEquals(
            listOf("one.length()", "two.length()"),
            context.expressionValuesForAtId(site.atId),
        )
    }

    @Test
    fun defaultAtIdMatchesEmptyExpressionId() {
        val source = expressionHandlerSource(
            prefix = """@Expression("text.length()")""",
            at = """@At(value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = "",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()

        val context = ExpressionContextResolver.resolveExpressionContext(source, site)

        assertNull(site.atId)
        assertEquals(listOf("text.length()"), context.expressionValuesForAtId(site.atId))
        assertEquals(listOf("text.length()"), context.expressionValuesForAtId(""))
    }

    @Test
    fun valuesForAtIdIsCaseSensitive() {
        val source = expressionHandlerSource(
            prefix = """
                @Expression(id = "main", value = "lower.length()")
                @Expression(id = "Main", value = "upper.length()")
            """.trimIndent(),
            at = """@At(id = "main", value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = "",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()

        val context = ExpressionContextResolver.resolveExpressionContext(source, site)

        assertEquals(listOf("lower.length()"), context.expressionValuesForAtId("main"))
        assertEquals(listOf("upper.length()"), context.expressionValuesForAtId("Main"))
        assertTrue(context.expressionValuesForAtId("MAIN").isEmpty())
    }

    @Test
    fun rejectsNonOfficialQualifiedExpressionAnnotationNames() {
        val source = expressionHandlerSource(
            prefix = """
                @foo.Expression("ignored.length()")
                @Expression("text.length()")
            """.trimIndent(),
            at = """@At(value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = "",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()

        val context = ExpressionContextResolver.resolveExpressionContext(source, site)

        assertEquals(listOf("text.length()"), context.expressionValuesForAtId(site.atId))
    }

    @Test
    fun acceptsOfficialFullyQualifiedExpressionAnnotation() {
        val source = expressionHandlerSource(
            prefix = """@com.llamalad7.mixinextras.expression.Expression("text.length()")""",
            at = """@At(value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = "",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()

        val context = ExpressionContextResolver.resolveExpressionContext(source, site)

        assertEquals(listOf("text.length()"), context.expressionValuesForAtId(site.atId))
    }

    @Test
    fun excludesStandaloneDefinitionsWithoutValidId() {
        val source = expressionHandlerSource(
            prefix = """
                @Definition(id = "first")
                @Definition()
                @Definition(id = not-a-string)
                @Definition(id = "second")
                @Definition(id = "second")
                @Definition(method = "Lcom/example/Foo;run()V")
            """.trimIndent(),
            at = """@At(value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = """@Expression("text.length()")""",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()

        val context = ExpressionContextResolver.resolveExpressionContext(source, site)

        assertEquals(
            listOf("first", "second", "second"),
            context.definitionIndex.definitions.mapNotNull { it.id },
        )
    }

    @Test
    fun collectsDefinitionsInLexicalOrder() {
        val source = expressionHandlerSource(
            prefix = """
                @Definition(id = "first")
                @Definitions(value = { @Definition(id = "second"), @Definition(id = "third") })
                @Definition(id = "fourth")
            """.trimIndent(),
            at = """@At(value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = """@Expression("text.length()")""",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()

        val context = ExpressionContextResolver.resolveExpressionContext(source, site)

        assertEquals(
            listOf("first", "second", "third", "fourth"),
            context.definitionIndex.definitions.mapNotNull { it.id },
        )
    }

    @Test
    fun recognizesExpressionAfterLiteralsContainingCommentMarkers() {
        val source = expressionHandlerSource(
            prefix = """
                @Decoy("text // not a comment /* also not a comment */ end")
                @Decoy('/')
                @Expression("text.length()")
            """.trimIndent(),
            at = """@At(value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = "",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()

        val context = ExpressionContextResolver.resolveExpressionContext(source, site)

        assertEquals(listOf("text.length()"), context.expressionValuesForAtId(site.atId))
    }

    @Test
    fun ignoresExpressionLikeTextInsideComments() {
        val source = expressionHandlerSource(
            prefix = "// @Expression(\"fake.length()\")\n/* @Expression(\"also.fake()\") */",
            at = """@At(value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = """@Expression("text.length()")""",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()

        val context = ExpressionContextResolver.resolveExpressionContext(source, site)

        assertEquals(listOf("text.length()"), context.expressionValuesForAtId(site.atId))
    }

    @Test
    fun pairsExpressionBeforeInjectorWithBareMarkerAnnotation() {
        val source = expressionHandlerSource(
            prefix = """
                @Expression(id = "main", value = "text.length()")
                @Deprecated
            """.trimIndent(),
            at = """@At(id = "main", value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = "",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()

        val context = ExpressionContextResolver.resolveExpressionContext(source, site)

        assertEquals(listOf("text.length()"), context.expressionValuesForAtId(site.atId))
    }

    @Test
    fun preservesShorthandExpressionAfterInjector() {
        val source = expressionHandlerSource(
            prefix = "",
            at = """@At(value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = """@Expression("text.length()")""",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()

        val context = ExpressionContextResolver.resolveExpressionContext(source, site)

        assertEquals(listOf("text.length()"), context.expressionValuesForAtId(site.atId))
    }

    @Test
    fun resolvesExpressionAdjacentToHandlerModifierWithoutTruncatingClosingParen() {
        val source = """
            @Mixin(SimpleTarget.class)
            abstract class ExampleMixin {
                @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "MIXINEXTRAS:EXPRESSION"))@Expression("text.length()")private int mcdev${'$'}handler(int original) { return original; }
            }
        """.trimIndent()
        val site = HandlerSignatureService.findAnnotationSites(source).single()

        val context = ExpressionContextResolver.resolveExpressionContext(source, site)

        assertEquals(listOf("text.length()"), context.expressionValuesForAtId(site.atId))
    }

    @Test
    fun infersTypeWhenAllMatchedExpressionsResolveToSameDescriptor() {
        val source = expressionHandlerSource(
            prefix = """
                @Expression(id = "main", value = "text.length()")
                @Expression(id = "main", value = "text.length()")
            """.trimIndent(),
            at = """@At(id = "main", value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = "",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val targetMethod = MethodIndexEntry("draw", "(Ljava/lang/String;FF)V", false, "draw(String, float, float): void")

        val inferred = ExpressionContextResolver.inferExpressionValueType(
            source = source,
            site = site,
            targetMethod = targetMethod,
            mixinTargets = listOf("com/example/target/SimpleTarget"),
            bytecodeIndex = MixinExtrasTestFixtures.bytecodeIndex,
            classIndex = MixinExtrasTestFixtures.classIndex,
        )

        assertEquals("I", inferred)
    }

    @Test
    fun returnsNullWhenMatchedExpressionsResolveToConflictingDescriptors() {
        val bytecodeIndex = FakeBytecodeIndex(
            candidates = mapOf(
                "com/example/target/SimpleTarget#draw#INVOKE" to listOf(
                    AtTargetCandidate(
                        owner = "java/lang/String",
                        name = "length",
                        descriptor = "()I",
                        displayLabel = "length(): int",
                        detail = "String",
                        kind = AtTargetKind.INVOKE,
                        namespace = MappingNamespace.NAMED,
                    ),
                ),
                "com/example/target/SimpleTarget#draw#FIELD" to listOf(
                    AtTargetCandidate(
                        owner = "com/example/target/SimpleTarget",
                        name = "label",
                        descriptor = "Ljava/lang/String;",
                        displayLabel = "label: String",
                        detail = "SimpleTarget",
                        kind = AtTargetKind.FIELD,
                        namespace = MappingNamespace.NAMED,
                    ),
                ),
            ),
        )
        val source = expressionHandlerSource(
            prefix = """
                @Expression(id = "main", value = "text.length()")
                @Expression(id = "main", value = "this.label")
            """.trimIndent(),
            at = """@At(id = "main", value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = "",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val targetMethod = MethodIndexEntry("draw", "(Ljava/lang/String;FF)V", false, "draw(String, float, float): void")

        val inferred = ExpressionContextResolver.inferExpressionValueType(
            source = source,
            site = site,
            targetMethod = targetMethod,
            mixinTargets = listOf("com/example/target/SimpleTarget"),
            bytecodeIndex = bytecodeIndex,
            classIndex = MixinExtrasTestFixtures.classIndex,
        )

        assertNull(inferred)
    }

    @Test
    fun providedResolvedContextOverridesContradictorySourceValues() {
        val owner = expressionMatchSamplesOwner()
        val source = expressionHandlerSource(
            prefix = """
                @Definition(id = "trimCall", method = "Ljava/lang/String;trim()Ljava/lang/String;")
                @Expression(id = "main", value = "this.label")
            """.trimIndent(),
            at = """@At(id = "main", value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = "",
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
            mixinTarget = "ExpressionMatchSamples",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val targetMethod = MethodIndexEntry("trim", "(Ljava/lang/String;)Ljava/lang/String;", false, "trim(String): String")
        val resolvedContext = ExpressionContext(
            expressionIndex = MixinExtrasExpressionIndex(
                expressions = listOf(MixinExtrasExpression(id = "main", values = listOf("@(?.trimCall())"))),
            ),
            definitionIndex = MixinExtrasDefinitionIndex(
                definitions = listOf(
                    MixinExtrasDefinition(
                        id = "trimCall",
                        rawMethodReferences = listOf("Ljava/lang/String;trim()Ljava/lang/String;"),
                    ),
                ),
            ),
        )

        val inferred = ExpressionContextResolver.inferExpressionValueType(
            source = source,
            site = site,
            targetMethod = targetMethod,
            mixinTargets = listOf(owner),
            bytecodeIndex = expressionMatchSamplesBytecodeIndex(),
            classIndex = MixinExtrasTestFixtures.classIndex,
            resolvedContext = resolvedContext,
        )

        assertEquals("Ljava/lang/String;", inferred)
    }

    @Test
    fun providedEmptyResolvedContextDoesNotFallBackToSourceExpression() {
        val source = expressionHandlerSource(
            prefix = """@Expression("text.length()")""",
            at = """@At(value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = "",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val targetMethod = MethodIndexEntry("draw", "(Ljava/lang/String;FF)V", false, "draw(String, float, float): void")
        val resolvedContext = ExpressionContext(
            expressionIndex = MixinExtrasExpressionIndex(),
            definitionIndex = MixinExtrasDefinitionIndex(),
        )

        val inferred = ExpressionContextResolver.inferExpressionValueType(
            source = source,
            site = site,
            targetMethod = targetMethod,
            mixinTargets = listOf("com/example/target/SimpleTarget"),
            bytecodeIndex = MixinExtrasTestFixtures.bytecodeIndex,
            classIndex = MixinExtrasTestFixtures.classIndex,
            resolvedContext = resolvedContext,
        )

        assertNull(inferred)
    }

    @Test
    fun officialMatcherResolvesStringFromDefinitionAndExpressionCapture() {
        val owner = expressionMatchSamplesOwner()
        val source = expressionHandlerSource(
            prefix = """
                @Definition(id = "trimCall", method = "Ljava/lang/String;trim()Ljava/lang/String;")
                @Expression("@(?.trimCall())")
            """.trimIndent(),
            mixinTarget = "ExpressionMatchSamples",
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val targetMethod = MethodIndexEntry("trim", "(Ljava/lang/String;)Ljava/lang/String;", false, "trim(String): String")

        val inferred = ExpressionContextResolver.inferExpressionValueType(
            source = source,
            site = site,
            targetMethod = targetMethod,
            mixinTargets = listOf(owner),
            bytecodeIndex = expressionMatchSamplesBytecodeIndex(),
            classIndex = MixinExtrasTestFixtures.classIndex,
        )

        assertEquals("Ljava/lang/String;", inferred)
    }

    @Test
    fun officialMatcherCollapsesMultipleExpressionsToSameDescriptor() {
        val owner = expressionMatchSamplesOwner()
        val source = expressionHandlerSource(
            prefix = """
                @Expression(id = "main", value = "@(?+?)")
                @Expression(id = "main", value = "@(?*?)")
            """.trimIndent(),
            at = """@At(id = "main", value = "MIXINEXTRAS:EXPRESSION")""",
            mixinTarget = "ExpressionMatchSamples",
            targetMethod = "addAndMultiply(III)I",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val targetMethod = MethodIndexEntry("addAndMultiply", "(III)I", false, "addAndMultiply(int, int, int): int")

        val inferred = ExpressionContextResolver.inferExpressionValueType(
            source = source,
            site = site,
            targetMethod = targetMethod,
            mixinTargets = listOf(owner),
            bytecodeIndex = expressionMatchSamplesBytecodeIndex(),
            classIndex = MixinExtrasTestFixtures.classIndex,
        )

        assertEquals("I", inferred)
    }

    @Test
    fun officialMatcherReturnsNullWhenExpressionDoesNotMatch() {
        val owner = expressionMatchSamplesOwner()
        val source = expressionHandlerSource(
            prefix = """@Expression("@('absent-literal')")""",
            mixinTarget = "ExpressionMatchSamples",
            targetMethod = "add(II)I",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val targetMethod = MethodIndexEntry("add", "(II)I", false, "add(int, int): int")

        val inferred = ExpressionContextResolver.inferExpressionValueType(
            source = source,
            site = site,
            targetMethod = targetMethod,
            mixinTargets = listOf(owner),
            bytecodeIndex = expressionMatchSamplesBytecodeIndex(),
            classIndex = MixinExtrasTestFixtures.classIndex,
        )

        assertNull(inferred)
    }

    @Test
    fun officialMatcherReturnsNullWhenMatchedExpressionsHaveConflictingTypes() {
        val owner = expressionMatchSamplesOwner()
        val source = expressionHandlerSource(
            prefix = """
                @Expression(id = "main", value = "@('prefix')")
                @Expression(id = "main", value = "@(?)")
            """.trimIndent(),
            mixinTarget = "ExpressionMatchSamples",
            targetMethod = "stringConcat(I)Ljava/lang/String;",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val targetMethod = MethodIndexEntry("stringConcat", "(I)Ljava/lang/String;", false, "stringConcat(int): String")

        val inferred = ExpressionContextResolver.inferExpressionValueType(
            source = source,
            site = site,
            targetMethod = targetMethod,
            mixinTargets = listOf(owner),
            bytecodeIndex = expressionMatchSamplesBytecodeIndex(),
            classIndex = MixinExtrasTestFixtures.classIndex,
        )

        assertNull(inferred)
    }

    @Test
    fun missingBytesFallsBackToRegexHeuristicWithoutResolvedContext() {
        val source = expressionHandlerSource(
            prefix = """@Expression("text.length()")""",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val targetMethod = MethodIndexEntry("draw", "(Ljava/lang/String;FF)V", false, "draw(String, float, float): void")

        val inferred = ExpressionContextResolver.inferExpressionValueType(
            source = source,
            site = site,
            targetMethod = targetMethod,
            mixinTargets = listOf("com/example/target/SimpleTarget"),
            bytecodeIndex = MixinExtrasTestFixtures.bytecodeIndex,
            classIndex = MixinExtrasTestFixtures.classIndex,
        )

        assertEquals("I", inferred)
    }

    @Test
    fun missingBytesWithResolvedContextDoesNotFallBackToRegexHeuristic() {
        val source = expressionHandlerSource(
            prefix = """@Expression("text.length()")""",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val targetMethod = MethodIndexEntry("draw", "(Ljava/lang/String;FF)V", false, "draw(String, float, float): void")
        val resolvedContext = ExpressionContext(
            expressionIndex = MixinExtrasExpressionIndex(
                expressions = listOf(MixinExtrasExpression(values = listOf("text.length()"))),
            ),
            definitionIndex = MixinExtrasDefinitionIndex(),
        )

        val inferred = ExpressionContextResolver.inferExpressionValueType(
            source = source,
            site = site,
            targetMethod = targetMethod,
            mixinTargets = listOf("com/example/target/SimpleTarget"),
            bytecodeIndex = MixinExtrasTestFixtures.bytecodeIndex,
            classIndex = MixinExtrasTestFixtures.classIndex,
            resolvedContext = resolvedContext,
        )

        assertNull(inferred)
    }

    @Test
    fun officialMatcherInvokesCommonSuperClassResolverForBranchMergeFixture() {
        val owner = branchMergeFixtureInternalName()
        var commonSuperInvocations = 0
        val bytecodeIndex = TrackingCommonSuperBytecodeIndex(
            delegate = branchMergeFixtureBytecodeIndex(),
            onResolveCommonSuperClass = { commonSuperInvocations++ },
        )
        val source = expressionHandlerSource(
            prefix = """@Expression("return @(?)")""",
            mixinTarget = "BranchMergeFixture",
            targetMethod = "branchMerge(ZLjava/lang/String;Ljava/lang/Integer;)Ljava/lang/Object;",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val targetMethod = MethodIndexEntry(
            "branchMerge",
            "(ZLjava/lang/String;Ljava/lang/Integer;)Ljava/lang/Object;",
            false,
            "branchMerge(boolean, String, Integer): Object",
        )

        val inferred = ExpressionContextResolver.inferExpressionValueType(
            source = source,
            site = site,
            targetMethod = targetMethod,
            mixinTargets = listOf(owner),
            bytecodeIndex = bytecodeIndex,
            classIndex = MixinExtrasTestFixtures.classIndex,
        )

        assertEquals("Ljava/lang/Object;", inferred)
        assertTrue(commonSuperInvocations > 0, "expected common super class resolver to be invoked")
    }

    @Test
    fun invalidPoolSiblingDoesNotPreventValidUsedDefinitionFromMatching() {
        val owner = expressionMatchSamplesOwner()
        val source = expressionHandlerSource(
            prefix = """
                @Definition(id = "broken", method = "not-a-method-selector")
                @Definition(id = "trimCall", method = "Ljava/lang/String;trim()Ljava/lang/String;")
                @Expression("@(?.trimCall())")
            """.trimIndent(),
            mixinTarget = "ExpressionMatchSamples",
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val targetMethod = MethodIndexEntry("trim", "(Ljava/lang/String;)Ljava/lang/String;", false, "trim(String): String")

        val inferred = ExpressionContextResolver.inferExpressionValueType(
            source = source,
            site = site,
            targetMethod = targetMethod,
            mixinTargets = listOf(owner),
            bytecodeIndex = expressionMatchSamplesBytecodeIndex(),
            classIndex = MixinExtrasTestFixtures.classIndex,
        )

        assertEquals("Ljava/lang/String;", inferred)
    }

    @Test
    fun undeclaredUsedDefinitionIdentifierYieldsNullWithoutRegexFallback() {
        val owner = expressionMatchSamplesOwner()
        val source = expressionHandlerSource(
            prefix = """@Expression("@(?.undeclaredMember())")""",
            mixinTarget = "ExpressionMatchSamples",
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val targetMethod = MethodIndexEntry("trim", "(Ljava/lang/String;)Ljava/lang/String;", false, "trim(String): String")

        val inferred = ExpressionContextResolver.inferExpressionValueType(
            source = source,
            site = site,
            targetMethod = targetMethod,
            mixinTargets = listOf(owner),
            bytecodeIndex = expressionMatchSamplesBytecodeIndex(),
            classIndex = MixinExtrasTestFixtures.classIndex,
        )

        assertNull(inferred)
    }

    private fun expressionMatchSamplesOwner(): String =
        BytecodeFixtureCompiler.internalName("ExpressionMatchSamples")

    private fun branchMergeFixtureInternalName(): String =
        "io/github/mcdev/core/mixinextras/testfixtures/BranchMergeFixture"

    private fun branchMergeFixtureClassBytes(): ByteArray {
        val internalName = branchMergeFixtureInternalName()
        val writer = ClassWriter(ClassWriter.COMPUTE_FRAMES or ClassWriter.COMPUTE_MAXS)
        writer.visit(Opcodes.V21, Opcodes.ACC_PUBLIC, internalName, null, "java/lang/Object", null)

        val method = writer.visitMethod(
            Opcodes.ACC_PUBLIC,
            "branchMerge",
            "(ZLjava/lang/String;Ljava/lang/Integer;)Ljava/lang/Object;",
            null,
            null,
        )
        method.visitCode()

        val mergeLabel = Label()
        val elseLabel = Label()

        method.visitVarInsn(Opcodes.ILOAD, 1)
        method.visitJumpInsn(Opcodes.IFEQ, elseLabel)
        method.visitVarInsn(Opcodes.ALOAD, 2)
        method.visitVarInsn(Opcodes.ASTORE, 4)
        method.visitJumpInsn(Opcodes.GOTO, mergeLabel)

        method.visitLabel(elseLabel)
        method.visitVarInsn(Opcodes.ALOAD, 3)
        method.visitVarInsn(Opcodes.ASTORE, 4)

        method.visitLabel(mergeLabel)
        method.visitVarInsn(Opcodes.ALOAD, 4)
        method.visitInsn(Opcodes.ARETURN)

        method.visitMaxs(0, 0)
        method.visitEnd()
        writer.visitEnd()
        return writer.toByteArray()
    }

    private fun branchMergeFixtureBytecodeIndex(): BytecodeIndex {
        val owner = branchMergeFixtureInternalName()
        val classBytes = branchMergeFixtureClassBytes()
        val commonSuperClassResolver = BytecodeCommonSuperClassResolver(
            classBytesLookup = { internalName ->
                if (internalName == owner) classBytes else null
            },
        )
        val delegate = FakeBytecodeIndex()
        return object : BytecodeIndex {
            override fun getAtTargetCandidates(
                ownerInternalName: String,
                methodName: String,
                methodDescriptor: String?,
                atValue: String,
            ): List<AtTargetCandidate> =
                delegate.getAtTargetCandidates(ownerInternalName, methodName, methodDescriptor, atValue)

            override fun getReturnOrdinalCount(
                ownerInternalName: String,
                methodName: String,
                methodDescriptor: String?,
            ): Int = delegate.getReturnOrdinalCount(ownerInternalName, methodName, methodDescriptor)

            override fun getClassBytes(ownerInternalName: String): ByteArray? =
                if (ownerInternalName == owner) classBytes else null

            override fun resolveCommonSuperClass(type1Descriptor: String, type2Descriptor: String): String? =
                commonSuperClassResolver.resolve(type1Descriptor, type2Descriptor)
                    ?: expressionMatchSamplesFallbackCommonSuper(type1Descriptor, type2Descriptor)
        }
    }

    private fun expressionMatchSamplesBytecodeIndex(): BytecodeIndex {
        val owner = expressionMatchSamplesOwner()
        val classBytes = BytecodeFixtureCompiler.classBytes("ExpressionMatchSamples")
        val commonSuperClassResolver = BytecodeCommonSuperClassResolver(
            classBytesLookup = { internalName ->
                if (internalName == owner) classBytes else null
            },
        )
        val delegate = FakeBytecodeIndex()
        return object : BytecodeIndex {
            override fun getAtTargetCandidates(
                ownerInternalName: String,
                methodName: String,
                methodDescriptor: String?,
                atValue: String,
            ): List<AtTargetCandidate> =
                delegate.getAtTargetCandidates(ownerInternalName, methodName, methodDescriptor, atValue)

            override fun getReturnOrdinalCount(
                ownerInternalName: String,
                methodName: String,
                methodDescriptor: String?,
            ): Int = delegate.getReturnOrdinalCount(ownerInternalName, methodName, methodDescriptor)

            override fun getClassBytes(ownerInternalName: String): ByteArray? =
                if (ownerInternalName == owner) classBytes else null

            override fun resolveCommonSuperClass(type1Descriptor: String, type2Descriptor: String): String? =
                commonSuperClassResolver.resolve(type1Descriptor, type2Descriptor)
                    ?: expressionMatchSamplesFallbackCommonSuper(type1Descriptor, type2Descriptor)
        }
    }

    private fun expressionMatchSamplesFallbackCommonSuper(type1Descriptor: String, type2Descriptor: String): String? {
        val type1 = Type.getType(type1Descriptor)
        val type2 = Type.getType(type2Descriptor)
        return when {
            type1 == type2 -> type1Descriptor
            type1.sort == Type.OBJECT && type2.sort == Type.OBJECT ->
                Type.getObjectType("java/lang/Object").descriptor
            else -> null
        }
    }

    private class TrackingCommonSuperBytecodeIndex(
        private val delegate: BytecodeIndex,
        private val onResolveCommonSuperClass: () -> Unit,
    ) : BytecodeIndex by delegate {
        override fun resolveCommonSuperClass(type1Descriptor: String, type2Descriptor: String): String? {
            onResolveCommonSuperClass()
            return delegate.resolveCommonSuperClass(type1Descriptor, type2Descriptor)
        }
    }

    @Test
    fun findEnclosingSiteReturnsSiteForOffsetInsideDefinitionPrefix() {
        val source = expressionHandlerSource(
            prefix = """@Definition(id = "trimCall")""",
            at = """@At(value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = """@Expression("text.length()")""",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val offset = source.indexOf("trimCall")

        assertEquals(site, ExpressionContextResolver.findEnclosingSite(source, offset))
    }

    @Test
    fun findEnclosingSiteDoesNotLeakAcrossAdjacentHandlers() {
        val source = dualHandlerSource()
        val sites = HandlerSignatureService.findAnnotationSites(source)
        assertEquals(2, sites.size)

        assertEquals(sites[0], ExpressionContextResolver.findEnclosingSite(source, source.indexOf("\"first\"")))
        assertEquals(sites[1], ExpressionContextResolver.findEnclosingSite(source, source.indexOf("\"second\"")))
    }

    @Test
    fun findEnclosingSiteResolvesStandardInjectHandlerWithExpression() {
        val source = """
            @Mixin(ExpressionMatchSamples.class)
            abstract class ExampleMixin {
                @Expression("this.")
                @Inject(method = "readSampleField()I", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
                private void mcdev${'$'}handler(CallbackInfo ci) {}
            }
        """.trimIndent()
        val site = HandlerSignatureService.findSugarHandlerAnnotationSites(source).single()
        val expressionOffset = source.indexOf("this.")

        assertEquals(MixinExtrasAnnotation.INJECT, site.annotation)
        assertEquals(site, ExpressionContextResolver.findEnclosingSite(source, expressionOffset))

        val context = ExpressionContextResolver.resolveExpressionContext(source, site)
        assertEquals(listOf("this."), context.expressionValuesForAtId(site.atId))
    }

    @Test
    fun findEnclosingSiteReturnsNullOutsideHandlerPrefixBlock() {
        val source = expressionHandlerSource(
            prefix = """@Definition(id = "trimCall")""",
            at = """@At(value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = """@Expression("text.length()")""",
        )
        val handlerBodyOffset = source.indexOf("return original")

        assertNull(ExpressionContextResolver.findEnclosingSite(source, handlerBodyOffset))
        assertNull(ExpressionContextResolver.findEnclosingSite(source, -1))
        assertNull(ExpressionContextResolver.findEnclosingSite(source, source.length))
    }

    @Test
    fun resolveExpressionContextForSiteFallsBackToLexicalParsingWhenResolvedContextsEmpty() {
        val source = expressionHandlerSource(
            prefix = """
                @Definition(id = "first")
                @Definition(id = "second")
            """.trimIndent(),
            at = """@At(value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = """@Expression("text.length()")""",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()

        val context = ExpressionContextResolver.resolveExpressionContextForSite(source, site, emptyList())

        assertEquals(
            listOf("first", "second"),
            context?.definitionIndex?.definitions?.mapNotNull { it.id },
        )
    }

    @Test
    fun resolveExpressionContextForSiteUsesAuthoritativeEmptyContextInsteadOfLexicalFallback() {
        val source = expressionHandlerSource(
            prefix = """
                @Definition(id = "first")
                @Definition(id = "second")
            """.trimIndent(),
            at = """@At(value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = """@Expression("text.length()")""",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val resolvedContexts = listOf(
            resolvedFor(site, emptyExpressionContext()),
        )

        val context = ExpressionContextResolver.resolveExpressionContextForSite(source, site, resolvedContexts)

        assertEquals(emptyList(), context?.definitionIndex?.definitions?.mapNotNull { it.id })
    }

    @Test
    fun resolveExpressionContextForSiteReturnsNullWhenAuthoritativeSelectionIsAmbiguous() {
        val source = expressionHandlerSource(
            prefix = """@Definition(id = "first")""",
            at = """@At(value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = """@Expression("text.length()")""",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val ambiguousContexts = listOf(
            resolvedFor(site, expressionContextWithDefinition("first")),
            resolvedFor(site, expressionContextWithDefinition("other")),
        )

        assertNull(ExpressionContextResolver.resolveExpressionContextForSite(source, site, ambiguousContexts))
    }

    private fun dualHandlerSource(): String = """
        @Mixin(SimpleTarget.class)
        abstract class ExampleMixin {
            @Definition(id = "first")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
            private int mcdev${'$'}handler1(int original) { return original; }

            @Definition(id = "second")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
            private int mcdev${'$'}handler2(int original) { return original; }
        }
    """.trimIndent()

    private fun resolvedFor(
        site: MixinExtrasAnnotationSite,
        context: ExpressionContext,
    ): ResolvedMixinExtrasContext = ResolvedMixinExtrasContext(
        handlerRange = site.handlerMethod?.range ?: site.annotationRange,
        context = context,
    )

    private fun emptyExpressionContext() = ExpressionContext(
        expressionIndex = MixinExtrasExpressionIndex(),
        definitionIndex = MixinExtrasDefinitionIndex(),
    )

    private fun expressionContextWithDefinition(id: String) = ExpressionContext(
        expressionIndex = MixinExtrasExpressionIndex(),
        definitionIndex = MixinExtrasDefinitionIndex(
            definitions = listOf(MixinExtrasDefinition(id = id)),
        ),
    )

    private fun handlerStartOffset(source: String, site: MixinExtrasAnnotationSite): Int =
        site.handlerMethod?.let { textPositionToOffset(source, it.range.start) }
            ?: textPositionToOffset(source, site.annotationRange.end)

    private fun textPositionToOffset(source: String, position: McTextPosition): Int {
        var line = 0
        var offset = 0
        while (offset < source.length && line < position.line) {
            if (source[offset] == '\n') line++
            offset++
        }
        return (offset + position.character).coerceAtMost(source.length)
    }

    private fun expressionHandlerSource(
        prefix: String,
        at: String = """@At(value = "MIXINEXTRAS:EXPRESSION")""",
        suffix: String = "",
        targetMethod: String = "draw(Ljava/lang/String;FF)V",
        mixinTarget: String = "SimpleTarget",
    ): String = """
        @Mixin($mixinTarget.class)
        abstract class ExampleMixin {
            $prefix
            @ModifyExpressionValue(method = "$targetMethod", at = $at)
            $suffix
            private int mcdev${'$'}handler(int original) { return original; }
        }
    """.trimIndent()
}
