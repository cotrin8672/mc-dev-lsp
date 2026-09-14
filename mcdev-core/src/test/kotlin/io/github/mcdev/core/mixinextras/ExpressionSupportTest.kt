package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.bytecode.BytecodeFixtureCompiler
import io.github.mcdev.core.codeaction.McTextEdit
import io.github.mcdev.core.mixin.AnnotationContext
import io.github.mcdev.core.mixin.AnnotationContextExtractor
import io.github.mcdev.core.mixin.AnnotationSlot
import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FakeBytecodeIndex
import io.github.mcdev.core.mixin.FakeClassIndex
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.mixin.AtValueCompletionService
import io.github.mcdev.core.mixin.MixinAnnotation
import io.github.mcdev.core.mixinextras.ExpressionCompletionPosition
import java.util.concurrent.atomic.AtomicInteger
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import org.objectweb.asm.Opcodes
import org.objectweb.asm.Type

class ExpressionSupportTest {
    private val expressionSupport = ExpressionSupport(classIndex = wrapWithConditionClassIndex())
    private val atValueService = AtValueCompletionService()

    @Test
    fun completesMixinExtrasExpressionAtValue() {
        val context = atValueContext("MIXINEXTRAS:EXPR")
        val coreItems = atValueService.complete(context)
        val extrasItems = expressionSupport.completeAtValue(context)
        assertTrue(coreItems.any { it.insertText == "MIXINEXTRAS:EXPRESSION" })
        assertEquals("MIXINEXTRAS:EXPRESSION", extrasItems.first().insertText)
        assertEquals("mixinextras.expressionAtValue", extrasItems.first().metadata.source)
    }

    @Test
    fun expressionAtValueCompletionFiltersByPrefix() {
        val context = atValueContext("MIXINEXTRAS")
        val items = expressionSupport.completeAtValue(context)
        assertEquals(1, items.size)
    }

    @Test
    fun completesExpressionAnnotationSnippets() {
        val context = AnnotationContext(
            annotation = MixinAnnotation.AT,
            slot = AnnotationSlot.VALUE,
            partialValue = "def",
            valueStartOffset = 0,
            valueEndOffset = 3,
            annotationStartOffset = 0,
            annotationEndOffset = 0,
        )
        val items = expressionSupport.completeExpressionAnnotations(context)
        assertTrue(items.any { it.insertText.startsWith("Definition") })
        assertTrue(items.any { it.metadata.source == "mixinextras.feature" })
    }

    @Test
    fun completesShareSnippet() {
        val items = expressionSupport.completeExpressionAnnotations(emptyPartialContext())
        assertTrue(items.any { it.metadata.name == "share" })
    }

    @Test
    fun completesLocalSnippet() {
        val items = expressionSupport.completeExpressionAnnotations(emptyPartialContext())
        assertTrue(items.any { it.metadata.name == "local" })
    }

    @Test
    fun completesCancellableSnippet() {
        val items = expressionSupport.completeExpressionAnnotations(emptyPartialContext())
        assertTrue(items.any { it.metadata.name == "cancellable" })
    }

    @Test
    fun completesAllMixinExtrasFeatureSnippets() {
        val items = expressionSupport.completeExpressionAnnotations(emptyPartialContext())
        val names = items.map { it.metadata.name }.toSet()

        assertTrue(names.containsAll(
            setOf(
                "modifyexpressionvalue",
                "modifyreturnvalue",
                "modifyreceiver",
                "wrapoperation",
                "wrapwithcondition",
                "wrapmethod",
                "definition",
                "definitions",
                "expression",
                "expressions",
                "share",
                "sharenamespace",
                "local",
                "localordinal",
                "localindex",
                "localname",
                "localargsonly",
                "localtype",
                "cancellable",
            ),
        ))
    }

    @Test
    fun completesFeatureAnnotationsAtAtSign() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            class ExampleMixin {
                @Expr
            }
        """.trimIndent()
        val offset = source.indexOf("@Expr") + "@Expr".length
        val (line, character) = offsetToLineCharacter(source, offset)
        val items = expressionSupport.completeFeatureAnnotations(source, line, character)

        assertTrue(items.any { it.insertText == "Expression(\"${'$'}{1}\")${'$'}0" })
        assertTrue(items.any { it.insertText == "Expressions({ ${'$'}{1} })${'$'}0" })
    }

    @Test
    fun argumentBearingCoreAnnotationSnippetsExposeUsefulAttributes() {
        val source = "package demo;\n\n@"
        val (line, character) = offsetToLineCharacter(source, source.length)
        val items = expressionSupport.completeFeatureAnnotationsWithRange(source, line, character)
            .items
            .associateBy { it.label }

        assertTrue(items.getValue("Group").insertText.contains("name ="))
        assertTrue(items.getValue("Group").insertText.contains("min ="))
        assertTrue(items.getValue("Debug").insertText.contains("export ="))
        assertTrue(items.getValue("Dynamic").insertText.contains("mixin ="))
        assertTrue(items.getValue("Unique").insertText.contains("silent ="))
        assertTrue(items.getValue("Intrinsic").insertText.contains("displace ="))
        assertTrue(items.getValue("Implements").insertText.contains("@Interface("))
        assertTrue(items.getValue("Interface").insertText.contains("prefix ="))
        assertTrue(items.getValue("Interface").insertText.contains("prefix\\${'$'}"))
        assertTrue(items.getValue("Debug").insertText.contains("${'$'}{1:false}"))
        assertTrue(items.getValue("Unique").insertText.contains("${'$'}{1:false}"))
        assertTrue(items.getValue("Intrinsic").insertText.contains("${'$'}{1:false}"))
        assertTrue(!items.getValue("Implements").insertText.contains("unique ="))
        assertTrue(!items.getValue("Implements").insertText.contains("remap ="))
        assertTrue(!items.getValue("Interface").insertText.contains("unique ="))
        assertTrue(!items.getValue("Interface").insertText.contains("remap ="))
        assertTrue(items.getValue("Desc").insertText.contains("args ="))
        assertTrue(items.getValue("Descriptors").insertText.contains("@Desc("))
        assertTrue(items.values
            .filter { it.label in setOf("Group", "Debug", "Dynamic", "Unique", "Intrinsic", "Implements", "Interface", "Desc", "Descriptors") }
            .all { it.insertText.endsWith("${'$'}0") && it.metadata.source == "mixin.annotation" })
    }

    @Test
    fun nestedCoreAnnotationSnippetsAddImportsAndRespectSimpleNameConflicts() {
        val source = "import other.Interface;\n\n@Impl"
        val (line, character) = offsetToLineCharacter(source, source.length)
        val completion = expressionSupport.completeFeatureAnnotationsWithRange(source, line, character)
        val item = completion.items.first { it.label == "Implements" }

        assertTrue(item.insertText.contains("@org.spongepowered.asm.mixin.Interface("))
        assertTrue(item.additionalEdits.any { it.newText.contains("import org.spongepowered.asm.mixin.Implements;") })
        assertTrue(item.additionalEdits.none { it.newText.contains("import org.spongepowered.asm.mixin.Interface;") })
        val updated = applyCompletion(source, assertNotNull(completion.replacementRange), item)
        assertTrue(updated.contains("@Implements({ @org.spongepowered.asm.mixin.Interface("))
        assertTrue(updated.contains("import org.spongepowered.asm.mixin.Implements;"))

        val groupedSource = "package demo;\n\n@Descr"
        val (groupedLine, groupedCharacter) = offsetToLineCharacter(groupedSource, groupedSource.length)
        val groupedCompletion = expressionSupport.completeFeatureAnnotationsWithRange(groupedSource, groupedLine, groupedCharacter)
        val grouped = groupedCompletion.items
            .first { it.label == "Descriptors" }
        assertTrue(grouped.insertText.contains("@Desc("))
        assertTrue(grouped.additionalEdits.any { it.newText.contains("import org.spongepowered.asm.mixin.injection.Descriptors;") })
        assertTrue(grouped.additionalEdits.any { it.newText.contains("import org.spongepowered.asm.mixin.injection.Desc;") })
        val groupedUpdated = applyCompletion(
            groupedSource,
            assertNotNull(groupedCompletion.replacementRange),
            grouped,
        )
        assertTrue(groupedUpdated.contains("@Descriptors({ @Desc("))
        assertTrue(groupedUpdated.contains("import org.spongepowered.asm.mixin.injection.Descriptors;"))
        assertTrue(groupedUpdated.contains("import org.spongepowered.asm.mixin.injection.Desc;"))
    }

    @Test
    fun coreAnnotationSnippetReplacesWholeNameAndPreservesExistingArguments() {
        val source = "@Groupx"
        val offset = source.indexOf("Group") + 2
        val (line, character) = offsetToLineCharacter(source, offset)
        val completion = expressionSupport.completeFeatureAnnotationsWithRange(source, line, character)
        val range = assertNotNull(completion.replacementRange)
        val item = completion.items.first { it.label == "Group" }

        assertEquals(source.indexOf("Group"), range.startOffset)
        assertEquals(source.length, range.endOffset)
        val updated = applyCompletion(source, range, item)
        assertTrue(updated.contains("@Group("))
        assertTrue(!updated.contains("@Groupx"))
        assertTrue(updated.contains("import org.spongepowered.asm.mixin.injection.Group;"))

        val existing = "@Group(existingArgs)"
        val existingOffset = existing.indexOf("Group") + 2
        val (existingLine, existingCharacter) = offsetToLineCharacter(existing, existingOffset)
        val existingCompletion = expressionSupport.completeFeatureAnnotationsWithRange(existing, existingLine, existingCharacter)
        assertTrue(existingCompletion.items.isEmpty())
        assertEquals(null, existingCompletion.replacementRange)
    }

    @Test
    fun annotationSnippetsApplyOverOnlyTheTypedNameAndAddImports() {
        val cases = listOf(
            "@Mix" to ("Mixin" to "org.spongepowered.asm.mixin.Mixin"),
            "@Inject" to ("Inject" to "org.spongepowered.asm.mixin.injection.Inject"),
            "@At" to ("At" to "org.spongepowered.asm.mixin.injection.At"),
            "@Wrap" to ("WrapOperation" to "com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation"),
            "@Expr" to ("Expression" to "com.llamalad7.mixinextras.expression.Expression"),
            "@Definition" to ("Definition" to "com.llamalad7.mixinextras.expression.Definition"),
            "@Local" to ("Local" to "com.llamalad7.mixinextras.sugar.Local"),
            "@Cancellable" to ("Cancellable" to "com.llamalad7.mixinextras.sugar.Cancellable"),
        )

        for ((typed, expected) in cases) {
            val source = "package demo;\n\n$typed"
            val offset = source.length
            val (line, character) = offsetToLineCharacter(source, offset)
            val completion = expressionSupport.completeFeatureAnnotationsWithRange(source, line, character)
            val item = completion.items.first { it.label == expected.first }
            val range = assertNotNull(completion.replacementRange)
            assertEquals(source.length - typed.length + 1, range.startOffset)
            assertEquals(source.length, range.endOffset)

            val updated = applyCompletion(source, range, item)
            assertTrue(updated.contains("@${expected.first}"))
            assertTrue(updated.contains("import ${expected.second};"))
        }
    }

    @Test
    fun wrapWithConditionUsesIndexedVersionAndImport() {
        val v2Fqn = "com.llamalad7.mixinextras.injector.v2.WrapWithCondition"
        val index = FakeClassIndex(
            classes = listOf(ClassIndexEntry("WrapWithCondition", "com.llamalad7.mixinextras.injector.v2", "com/llamalad7/mixinextras/injector/v2/WrapWithCondition")),
        )
        val support = ExpressionSupport(classIndex = index)
        val source = "package demo;\n\n@Wrap"
        val (line, character) = offsetToLineCharacter(source, source.length)

        val item = support.completeFeatureAnnotationsWithRange(source, line, character)
            .items
            .first { it.label == "WrapWithCondition (v2)" }

        assertTrue(item.additionalEdits.any { it.newText.contains("import $v2Fqn;") })
        assertTrue(item.additionalEdits.none { it.newText.contains("injector.WrapWithCondition;") && !it.newText.contains("injector.v2") })
    }

    @Test
    fun featureCompletionIgnoresAtSignsInStringsAndComments() {
        val source = """
            class Example {
                String text = "@Expr";
                // @Definition
                @Expr
            }
        """.trimIndent()
        val offset = source.lastIndexOf("@Expr") + "@Expr".length
        val (line, character) = offsetToLineCharacter(source, offset)

        val completion = expressionSupport.completeFeatureAnnotationsWithRange(source, line, character)

        assertTrue(completion.items.any { it.label == "Expression" })
        assertEquals(source.lastIndexOf("Expr"), completion.replacementRange?.startOffset)
        assertEquals(offset, completion.replacementRange?.endOffset)
    }

    @Test
    fun conflictingAnnotationImportUsesQualifiedSnippetWithoutConflictingImport() {
        val source = "import other.Local;\n\n@Local"
        val (line, character) = offsetToLineCharacter(source, source.length)

        val completion = expressionSupport.completeFeatureAnnotationsWithRange(source, line, character)
        val item = completion.items.first { it.label == "Local" }

        assertTrue(item.insertText.startsWith("com.llamalad7.mixinextras.sugar.Local"))
        assertTrue(item.additionalEdits.none { it.newText.contains("com.llamalad7.mixinextras.sugar.Local;") })
        val updated = applyCompletion(source, assertNotNull(completion.replacementRange), item)
        assertTrue(updated.contains("@com.llamalad7.mixinextras.sugar.Local"))
    }

    @Test
    fun annotationSnippetReplacesWholeNameWhenCaretIsInsideIt() {
        val source = "@Inject"
        val offset = source.indexOf("Inject") + 2
        val (line, character) = offsetToLineCharacter(source, offset)

        val completion = expressionSupport.completeFeatureAnnotationsWithRange(source, line, character)
        val item = completion.items.first { it.label == "Inject" }
        val range = assertNotNull(completion.replacementRange)

        assertEquals(source.indexOf("Inject"), range.startOffset)
        assertEquals(source.length, range.endOffset)
        assertEquals("@${item.insertText}", applyCompletion(source, range, item).lineSequence().last())
    }

    @Test
    fun annotationSnippetDoesNotDuplicateExistingArgumentListWhenCaretIsInsideName() {
        val source = "@Inject(existingArgs)"
        val offset = source.indexOf("Inject") + 2
        val (line, character) = offsetToLineCharacter(source, offset)

        val completion = expressionSupport.completeFeatureAnnotationsWithRange(source, line, character)

        assertTrue(completion.items.isEmpty())
        assertEquals(null, completion.replacementRange)
    }

    @Test
    fun isExpressionAtValueDetectsValue() {
        assertTrue(expressionSupport.isExpressionAtValue("MIXINEXTRAS:EXPRESSION"))
        assertTrue(!expressionSupport.isExpressionAtValue("INVOKE"))
    }

    @Test
    fun expressionAtValueReturnsEmptyForNonAtAnnotation() {
        val context = AnnotationContext(
            annotation = MixinAnnotation.INJECT,
            slot = AnnotationSlot.VALUE,
            partialValue = "MIXINEXTRAS",
            valueStartOffset = 0,
            valueEndOffset = 9,
            annotationStartOffset = 0,
            annotationEndOffset = 0,
        )
        assertTrue(expressionSupport.completeAtValue(context).isEmpty())
    }

    @Test
    fun completeExpressionValueAtStatementStartOffersReturnAndThrow() {
        val items = expressionSupport.completeExpressionValue(
            expressionValueContext("", ExpressionCompletionPosition.STATEMENT_START),
        )
        assertTrue(items.any { it.insertText == "return" })
        assertTrue(items.any { it.insertText == "throw" })
        assertTrue(items.all { it.metadata.source == "mixinextras.expressionValue" })
    }

    @Test
    fun completeExpressionValueAtStatementStartAlsoOffersValueKeywords() {
        val items = expressionSupport.completeExpressionValue(
            expressionValueContext("", ExpressionCompletionPosition.STATEMENT_START),
        )
        assertTrue(items.any { it.insertText == "this" })
        assertTrue(items.any { it.insertText == "new" })
        assertTrue(items.any { it.insertText == "return" })
    }

    @Test
    fun completeExpressionValueAtValueStartOffersValueKeywordsOnly() {
        val items = expressionSupport.completeExpressionValue(
            expressionValueContext("", ExpressionCompletionPosition.VALUE_START),
        )
        assertEquals(listOf("this", "super", "true", "false", "null", "new"), items.map { it.insertText })
    }

    @Test
    fun completeExpressionValueFiltersByPrefix() {
        val items = expressionSupport.completeExpressionValue(
            expressionValueContext("th", ExpressionCompletionPosition.VALUE_START),
        )
        assertEquals(listOf("this"), items.map { it.insertText })
    }

    @Test
    fun completeExpressionValueRepeatedPrefixMatchesMultipleKeywords() {
        val items = expressionSupport.completeExpressionValue(
            expressionValueContext("t", ExpressionCompletionPosition.VALUE_START),
        )
        assertEquals(listOf("this", "true"), items.map { it.insertText })
    }

    @Test
    fun completeExpressionValueAfterMethodReferenceOffersNew() {
        val items = expressionSupport.completeExpressionValue(
            expressionValueContext("n", ExpressionCompletionPosition.AFTER_METHOD_REFERENCE),
        )
        assertEquals(listOf("new"), items.map { it.insertText })
    }

    @Test
    fun completeExpressionValueAfterDotReturnsEmpty() {
        val items = expressionSupport.completeExpressionValue(
            expressionValueContext("", ExpressionCompletionPosition.AFTER_DOT),
        )
        assertTrue(items.isEmpty())
    }

    @Test
    fun completeExpressionValueNoneReturnsEmpty() {
        val items = expressionSupport.completeExpressionValue(
            expressionValueContext("", ExpressionCompletionPosition.NONE),
        )
        assertTrue(items.isEmpty())
    }

    @Test
    fun completeExpressionValueRequiresExpressionAnnotation() {
        val context = expressionValueContext("th", ExpressionCompletionPosition.VALUE_START)
            .copy(annotation = MixinAnnotation.DEFINITION)
        assertTrue(expressionSupport.completeExpressionValue(context).isEmpty())
    }

    @Test
    fun completeExpressionValueOffersLexicalDefinitionIdsWithKeywords() {
        val source = expressionHandlerSource(
            prefix = """
                @Definition(id = "alpha")
                @Definition(id = "beta")
            """.trimIndent(),
            expressionValue = "",
        )
        val context = expressionValueContextFromSource(source)
        val items = expressionSupport.completeExpressionValue(source = source, context = context)
        assertEquals(
            listOf("this", "super", "true", "false", "null", "new", "alpha", "beta"),
            items.map { it.insertText },
        )
        assertTrue(items.take(6).all { it.metadata.source == "mixinextras.expressionValue" })
        assertTrue(items.drop(6).all { it.metadata.source == "mixinextras.definitionId" })
        assertTrue(items.drop(6).all { it.kind.name == "VALUE" })
    }

    @Test
    fun completeExpressionValueFiltersDefinitionIdsByPrefix() {
        val source = expressionHandlerSource(
            prefix = """
                @Definition(id = "alpha")
                @Definition(id = "beta")
            """.trimIndent(),
            expressionValue = "al",
        )
        val context = expressionValueContextFromSource(source, valuePrefix = "al")
        val items = expressionSupport.completeExpressionValue(source = source, context = context)
        assertEquals(listOf("alpha"), items.map { it.insertText })
        assertEquals("mixinextras.definitionId", items.single().metadata.source)
    }

    @Test
    fun completeExpressionValueDeduplicatesRepeatedDefinitionIds() {
        val source = expressionHandlerSource(
            prefix = """
                @Definition(id = "alpha")
                @Definition(id = "beta")
                @Definition(id = "alpha")
            """.trimIndent(),
            expressionValue = "",
        )
        val context = expressionValueContextFromSource(source)
        val definitionIds = expressionSupport.completeExpressionValue(source = source, context = context)
            .filter { it.metadata.source == "mixinextras.definitionId" }
            .map { it.insertText }
        assertEquals(listOf("alpha", "beta"), definitionIds)
    }

    @Test
    fun completeExpressionValuePreservesDefinitionIdDeclarationOrder() {
        val source = expressionHandlerSource(
            prefix = """
                @Definition(id = "first")
                @Definition(id = "second")
                @Definition(id = "third")
            """.trimIndent(),
            expressionValue = "",
        )
        val context = expressionValueContextFromSource(source)
        val definitionIds = expressionSupport.completeExpressionValue(source = source, context = context)
            .filter { it.metadata.source == "mixinextras.definitionId" }
            .map { it.insertText }
        assertEquals(listOf("first", "second", "third"), definitionIds)
    }

    @Test
    fun completeExpressionValueIsolatesDefinitionIdsByHandler() {
        val source = dualHandlerSource()
        val secondHandlerContext = expressionValueContextFromSource(
            source = source,
            valuePrefix = "",
            handlerIndex = 1,
        )
        val definitionIds = expressionSupport.completeExpressionValue(source = source, context = secondHandlerContext)
            .filter { it.metadata.source == "mixinextras.definitionId" }
            .map { it.insertText }
        assertEquals(listOf("second"), definitionIds)
    }

    @Test
    fun completeExpressionValueUsesAuthoritativeDefinitionIdsOverLexical() {
        val source = expressionHandlerSource(
            prefix = """
                @Definition(id = "lexicalFirst")
                @Definition(id = "lexicalSecond")
            """.trimIndent(),
            expressionValue = "",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val resolvedContexts = listOf(
            ResolvedMixinExtrasContext(
                handlerRange = site.handlerMethod?.range ?: site.annotationRange,
                context = ExpressionContext(
                    expressionIndex = MixinExtrasExpressionIndex(),
                    definitionIndex = MixinExtrasDefinitionIndex(
                        definitions = listOf(MixinExtrasDefinition(id = "authoritativeOnly")),
                    ),
                ),
            ),
        )
        val context = expressionValueContextFromSource(source)
        val definitionIds = expressionSupport.completeExpressionValue(
            source = source,
            context = context,
            resolvedContexts = resolvedContexts,
        )
            .filter { it.metadata.source == "mixinextras.definitionId" }
            .map { it.insertText }
        assertEquals(listOf("authoritativeOnly"), definitionIds)
    }

    @Test
    fun completeExpressionValueAuthoritativeEmptySuppressesLexicalDefinitionIds() {
        val source = expressionHandlerSource(
            prefix = """
                @Definition(id = "lexicalFirst")
                @Definition(id = "lexicalSecond")
            """.trimIndent(),
            expressionValue = "",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val resolvedContexts = listOf(
            ResolvedMixinExtrasContext(
                handlerRange = site.handlerMethod?.range ?: site.annotationRange,
                context = ExpressionContext(
                    expressionIndex = MixinExtrasExpressionIndex(),
                    definitionIndex = MixinExtrasDefinitionIndex(),
                ),
            ),
        )
        val context = expressionValueContextFromSource(source)
        val definitionIds = expressionSupport.completeExpressionValue(
            source = source,
            context = context,
            resolvedContexts = resolvedContexts,
        )
            .filter { it.metadata.source == "mixinextras.definitionId" }
        assertTrue(definitionIds.isEmpty())
        assertTrue(
            expressionSupport.completeExpressionValue(source = source, context = context, resolvedContexts = resolvedContexts)
                .any { it.metadata.source == "mixinextras.expressionValue" },
        )
    }

    @Test
    fun completeExpressionValueDoesNotOfferDefinitionIdsAfterDotEvenWithDefinitions() {
        val source = expressionHandlerSource(
            prefix = """@Definition(id = "alpha")""",
            expressionValue = "this.",
        )
        val context = expressionValueContextFromSource(source, valuePrefix = "this.")
            .copy(expressionCompletionPosition = ExpressionCompletionPosition.AFTER_DOT)
        assertTrue(expressionSupport.completeExpressionValue(source = source, context = context).isEmpty())
    }

    @Test
    fun completeExpressionValueAfterDotOffersFieldAndMethodMemberItems() {
        val owner = BytecodeFixtureCompiler.internalName("ExpressionMatchSamples")
        val service = expressionMemberCompletionService(owner)
        val support = ExpressionSupport(service)
        val source = memberExpressionHandlerSource(
            targetMethod = "readSampleField()I",
            expressionValue = "this.",
        )
        val context = memberExpressionValueContextFromSource(source, valuePrefix = "this.")

        val fieldItems = support.completeExpressionValue(
            source = source,
            context = context,
            mixinTargetOwners = listOf(owner),
        ).filter { it.metadata.source == "mixinextras.expressionMember" }

        assertEquals(1, fieldItems.size)
        val field = fieldItems.single()
        assertEquals(McCompletionKind.FIELD, field.kind)
        assertEquals("sampleField:I", field.label)
        assertEquals(owner, field.detail)
        assertEquals("I", field.metadata.descriptor)
        assertEquals("sampleField", field.metadata.name)
        assertEquals("sampleField", field.insertText)
        assertTrue(field.additionalEdits.any { it.newText.contains("@Definition(id = \"sampleField\"") })

        val methodSource = memberExpressionHandlerSource(
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
            expressionValue = "value.",
        )
        val methodContext = memberExpressionValueContextFromSource(methodSource, valuePrefix = "value.")
        val methodItems = support.completeExpressionValue(
            source = methodSource,
            context = methodContext,
            mixinTargetOwners = listOf(owner),
        ).filter { it.metadata.source == "mixinextras.expressionMember" }

        assertEquals(1, methodItems.size)
        val method = methodItems.single()
        assertEquals(McCompletionKind.METHOD, method.kind)
        assertEquals("trim()Ljava/lang/String;", method.label)
        assertEquals("java/lang/String", method.detail)
        assertEquals("()Ljava/lang/String;", method.metadata.descriptor)
        assertEquals("trim()", method.insertText)
    }

    @Test
    fun completeExpressionValueAfterDotAddsDefinitionAndImportEditsForNewMembers() {
        val owner = BytecodeFixtureCompiler.internalName("ExpressionMatchSamples")
        val support = ExpressionSupport(expressionMemberCompletionService(owner))
        val source = """
            package com.example.mixin;

            @Mixin(ExpressionMatchSamples.class)
            abstract class ExampleMixin {
                @ModifyExpressionValue(method = "readSampleField()I", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
                @Expression("this.")
                private Object mcdev${'$'}handler(Object original) { return original; }
            }
        """.trimIndent()
        val context = memberExpressionValueContextFromSource(source, valuePrefix = "this.")
        val item = support.completeExpressionValue(
            source = source,
            context = context,
            mixinTargetOwners = listOf(owner),
        ).single { it.metadata.source == "mixinextras.expressionMember" }

        assertTrue(item.additionalEdits.any { it.newText.contains("@Definition(id = \"sampleField\"") })
        assertTrue(
            item.additionalEdits.any {
                it.newText.contains("import com.llamalad7.mixinextras.expression.Definition;")
            },
        )
    }

    @Test
    fun completeExpressionValueAfterDotReusesExactDefinitionWithoutAdditionalEdits() {
        val owner = BytecodeFixtureCompiler.internalName("ExpressionMatchSamples")
        val sampleFieldSelector = "L$owner;sampleField:I"
        val support = ExpressionSupport(expressionMemberCompletionService(owner))
        val source = memberExpressionHandlerSource(
            prefix = """@Definition(id = "sampleField", field = "$sampleFieldSelector")""",
            targetMethod = "readSampleField()I",
            expressionValue = "this.",
        )
        val context = memberExpressionValueContextFromSource(source, valuePrefix = "this.")
        val item = support.completeExpressionValue(
            source = source,
            context = context,
            mixinTargetOwners = listOf(owner),
        ).single { it.metadata.source == "mixinextras.expressionMember" }

        assertEquals("sampleField", item.insertText)
        assertTrue(item.additionalEdits.isEmpty())
    }

    @Test
    fun completeExpressionValueAfterDotOmitsUnreachableCandidates() {
        val owner = BytecodeFixtureCompiler.internalName("ExpressionMatchSamples")
        val support = ExpressionSupport(expressionMemberCompletionService(owner))
        val source = memberExpressionHandlerSource(
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
            expressionValue = "value.",
        )
        val context = memberExpressionValueContextFromSource(source, valuePrefix = "value.")
        val names = support.completeExpressionValue(
            source = source,
            context = context,
            mixinTargetOwners = listOf(owner),
        ).map { it.metadata.name }

        assertEquals(listOf("trim"), names)
        assertTrue(names.none { it == "sampleField" })
        assertTrue(names.none { it == "readSampleField" })
    }

    @Test
    fun completeExpressionValueAfterDotDistinguishesOverloadedMethods() {
        val owner = "com/example/target/SimpleTarget"
        val support = ExpressionSupport(overloadedDrawMemberCompletionService(owner))
        val source = memberExpressionHandlerSource(
            mixinTarget = "SimpleTarget",
            targetMethod = "draw(Ljava/lang/String;FF)V",
            expressionValue = "this.",
        )
        val context = memberExpressionValueContextFromSource(source, valuePrefix = "this.")
        val items = support.completeExpressionValue(
            source = source,
            context = context,
            mixinTargetOwners = listOf(owner),
        ).filter { it.metadata.name == "draw" }

        assertEquals(2, items.size)
        assertEquals(
            setOf(
                "draw(Ljava/lang/String;FF)V",
                "draw(I)V",
            ),
            items.map { it.label }.toSet(),
        )
        assertEquals(
            setOf(
                "Lcom/example/target/SimpleTarget;draw(Ljava/lang/String;FF)V",
                "Lcom/example/target/SimpleTarget;draw(I)V",
            ),
            items.flatMap { edit ->
                edit.additionalEdits
                    .filter { it.newText.contains("@Definition") }
                    .map { it.newText.substringAfter("method = \"").substringBefore("\"") }
            }.toSet(),
        )
    }

    @Test
    fun completeExpressionValueAfterDotReturnsEmptyWhenMemberServiceUnavailable() {
        val owner = BytecodeFixtureCompiler.internalName("ExpressionMatchSamples")
        val missingBytesIndex = object : BytecodeIndex by expressionMatchSamplesBytecodeIndex(owner) {
            override fun getClassBytes(ownerInternalName: String): ByteArray? = null
        }
        val support = ExpressionSupport(
            ExpressionMemberCompletionService(
                expressionMatchSamplesClassIndex(owner),
                missingBytesIndex,
            ),
        )
        val source = memberExpressionHandlerSource(
            targetMethod = "readSampleField()I",
            expressionValue = "this.",
        )
        val context = memberExpressionValueContextFromSource(source, valuePrefix = "this.")

        assertTrue(
            support.completeExpressionValue(
                source = source,
                context = context,
                mixinTargetOwners = listOf(owner),
            ).isEmpty(),
        )
    }

    @Test
    fun completeExpressionValueKeywordAndDefinitionIdCompletionDoesNotInvokeMemberService() {
        val owner = BytecodeFixtureCompiler.internalName("ExpressionMatchSamples")
        val invocationCount = AtomicInteger(0)
        val support = ExpressionSupport(
            ExpressionMemberCompletionService(
                classIndex = expressionMatchSamplesClassIndex(owner),
                bytecodeIndex = expressionMatchSamplesBytecodeIndex(owner),
                matcherRunner = ExpressionMemberMatcherRunner { classBytes, methodName, methodDescriptor, receiverExpression, contextType, identifierPool, commonSuperClass, cancellationChecker ->
                    invocationCount.incrementAndGet()
                    OfficialExpressionMatcher.completeReceiverMembers(
                        classBytes = classBytes,
                        methodName = methodName,
                        methodDescriptor = methodDescriptor,
                        receiverExpression = receiverExpression,
                        contextType = contextType,
                        identifierPool = identifierPool,
                        commonSuperClass = commonSuperClass,
                        cancellationChecker = cancellationChecker,
                    )
                },
            ),
        )
        val source = expressionHandlerSource(
            prefix = """
                @Definition(id = "alpha")
                @Definition(id = "beta")
            """.trimIndent(),
            expressionValue = "",
        )
        val context = expressionValueContextFromSource(source)

        val items = support.completeExpressionValue(
            source = source,
            context = context,
            mixinTargetOwners = listOf(owner),
        )
        assertEquals(
            listOf("this", "super", "true", "false", "null", "new", "alpha", "beta"),
            items.map { it.insertText },
        )
        assertEquals(0, invocationCount.get())
        assertTrue(items.none { it.metadata.source == "mixinextras.expressionMember" })
    }

    private fun expressionMemberCompletionService(owner: String): ExpressionMemberCompletionService =
        ExpressionMemberCompletionService(
            classIndex = expressionMatchSamplesClassIndex(owner),
            bytecodeIndex = expressionMatchSamplesBytecodeIndex(owner),
        )

    private fun overloadedDrawMemberCompletionService(owner: String): ExpressionMemberCompletionService =
        ExpressionMemberCompletionService(
            classIndex = expressionMatchSamplesClassIndex(owner),
            bytecodeIndex = expressionMatchSamplesBytecodeIndex(owner),
            matcherRunner = ExpressionMemberMatcherRunner { _, _, _, _, _, _, _, _ ->
                OfficialExpressionMemberCompletionResult.Available(
                    listOf(
                        OfficialExpressionMemberCandidate.MethodInvocation(
                            ownerInternalName = owner,
                            name = "draw",
                            descriptor = "(Ljava/lang/String;FF)V",
                            isInterface = false,
                            originalInstructionOpcode = Opcodes.INVOKEVIRTUAL,
                            originalInstructionIndex = 0,
                        ),
                        OfficialExpressionMemberCandidate.MethodInvocation(
                            ownerInternalName = owner,
                            name = "draw",
                            descriptor = "(I)V",
                            isInterface = false,
                            originalInstructionOpcode = Opcodes.INVOKEVIRTUAL,
                            originalInstructionIndex = 1,
                        ),
                    ),
                )
            },
        )

    private fun expressionMatchSamplesClassIndex(owner: String): ClassIndex =
        FakeClassIndex(
            classes = FakeClassIndex.defaultClasses() + listOf(
                ClassIndexEntry(
                    "ExpressionMatchSamples",
                    "io.github.mcdev.core.bytecode.fixtures",
                    owner,
                ),
                ClassIndexEntry("SimpleTarget", "com.example.target", "com/example/target/SimpleTarget"),
            ),
            methods = FakeClassIndex.defaultMethods() + mapOf(
                owner to listOf(
                    MethodIndexEntry("readSampleField", "()I", false, "readSampleField(): int"),
                    MethodIndexEntry(
                        "trim",
                        "(Ljava/lang/String;)Ljava/lang/String;",
                        false,
                        "trim(String): String",
                    ),
                ),
                "com/example/target/SimpleTarget" to listOf(
                    MethodIndexEntry("draw", "(Ljava/lang/String;FF)V", false, "draw(String, float, float): void"),
                    MethodIndexEntry("draw", "(I)V", false, "draw(int): void"),
                ),
            ),
        )

    private fun expressionMatchSamplesBytecodeIndex(owner: String): BytecodeIndex {
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
                if (ownerInternalName == owner) classBytes else delegate.getClassBytes(ownerInternalName)

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

    private fun memberExpressionHandlerSource(
        mixinTarget: String = "ExpressionMatchSamples",
        prefix: String = "",
        targetMethod: String,
        expressionValue: String,
    ): String = """
        @Mixin($mixinTarget.class)
        abstract class ExampleMixin {
            $prefix
            @ModifyExpressionValue(method = "$targetMethod", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
            @Expression("$expressionValue")
            private Object mcdev${'$'}handler(Object original) { return original; }
        }
    """.trimIndent()

    private fun memberExpressionValueContextFromSource(
        source: String,
        valuePrefix: String,
    ): AnnotationContext {
        val annotationStart = source.indexOf("@Expression")
        val valueStart = source.indexOf('"', annotationStart) + 1
        val cursor = valueStart + valuePrefix.length
        return AnnotationContextExtractor.extractAtOffset(source, cursor)
            ?: error("failed to extract annotation context at offset $cursor")
    }

    private fun expressionValueContextFromSource(
        source: String,
        expressionMarker: String = "@Expression",
        valuePrefix: String = "",
        handlerIndex: Int = 0,
    ): AnnotationContext {
        var searchFrom = 0
        repeat(handlerIndex) {
            searchFrom = source.indexOf(expressionMarker, searchFrom) + expressionMarker.length
        }
        val annotationStart = source.indexOf(expressionMarker, searchFrom)
        val valueStart = source.indexOf('"', annotationStart) + 1
        val cursor = valueStart + valuePrefix.length
        return AnnotationContextExtractor.extractAtOffset(source, cursor)
            ?: error("failed to extract annotation context at offset $cursor")
    }

    private fun expressionHandlerSource(
        prefix: String,
        expressionValue: String = "",
    ): String = """
        @Mixin(SimpleTarget.class)
        abstract class ExampleMixin {
            $prefix
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
            @Expression("$expressionValue")
            private int mcdev${'$'}handler(int original) { return original; }
        }
    """.trimIndent()

    private fun dualHandlerSource(): String = """
        @Mixin(SimpleTarget.class)
        abstract class ExampleMixin {
            @Definition(id = "first")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
            @Expression("")
            private int mcdev${'$'}handler1(int original) { return original; }

            @Definition(id = "second")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
            @Expression("")
            private int mcdev${'$'}handler2(int original) { return original; }
        }
    """.trimIndent()

    private fun expressionValueContext(
        partial: String,
        position: ExpressionCompletionPosition,
    ) = AnnotationContext(
        annotation = MixinAnnotation.EXPRESSION,
        slot = AnnotationSlot.VALUE,
        partialValue = partial,
        valueStartOffset = 0,
        valueEndOffset = partial.length,
        annotationStartOffset = 0,
        annotationEndOffset = 0,
        expressionCompletionPosition = position,
    )

    private fun atValueContext(partial: String) = AnnotationContext(
        annotation = MixinAnnotation.AT,
        slot = AnnotationSlot.VALUE,
        partialValue = partial,
        valueStartOffset = 0,
        valueEndOffset = partial.length,
        annotationStartOffset = 0,
        annotationEndOffset = 0,
    )

    private fun emptyPartialContext() = AnnotationContext(
        annotation = MixinAnnotation.AT,
        slot = AnnotationSlot.TARGET,
        partialValue = "",
        valueStartOffset = 0,
        valueEndOffset = 0,
        annotationStartOffset = 0,
        annotationEndOffset = 0,
    )

    private fun offsetToLineCharacter(source: String, offset: Int): Pair<Int, Int> {
        var line = 0
        var character = 0
        var index = 0
        while (index < offset && index < source.length) {
            if (source[index] == '\n') {
                line++
                character = 0
            } else {
                character++
            }
            index++
        }
        return line to character
    }

    private fun wrapWithConditionClassIndex() = FakeClassIndex(
        classes = listOf(
            ClassIndexEntry(
                "WrapWithCondition",
                "com.llamalad7.mixinextras.injector",
                "com/llamalad7/mixinextras/injector/WrapWithCondition",
            ),
            ClassIndexEntry(
                "WrapWithCondition",
                "com.llamalad7.mixinextras.injector.v2",
                "com/llamalad7/mixinextras/injector/v2/WrapWithCondition",
            ),
        ),
    )

    private fun applyCompletion(
        source: String,
        range: io.github.mcdev.core.completion.McCompletionReplacementRange,
        item: io.github.mcdev.core.completion.McCompletionItem,
    ): String {
        val edits = item.additionalEdits + McTextEdit(
            startOffset = range.startOffset,
            endOffset = range.endOffset,
            newText = item.insertText,
        )
        var updated = source
        edits.sortedByDescending { it.startOffset }.forEach { edit ->
            updated = updated.replaceRange(edit.startOffset, edit.endOffset, edit.newText)
        }
        return updated
    }
}
