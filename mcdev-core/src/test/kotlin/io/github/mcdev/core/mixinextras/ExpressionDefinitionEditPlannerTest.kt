package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.codeaction.McTextEdit
import io.github.mcdev.core.mixin.AnnotationContext
import io.github.mcdev.core.mixin.AnnotationContextExtractor
import io.github.mcdev.core.mixin.AnnotationSlot
import io.github.mcdev.core.mixin.MixinAnnotation
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue
import org.objectweb.asm.Opcodes

class ExpressionDefinitionEditPlannerTest {
    private val sampleOwner = "io/github/mcdev/core/bytecode/fixtures/ExpressionMatchSamples"
    private val sampleFieldSelector = "L$sampleOwner;sampleField:I"
    private val trimMethodSelector = "Ljava/lang/String;trim()Ljava/lang/String;"
    private val lengthMethodSelector = "Ljava/lang/String;length()I"

    @Test
    fun reusesExactFieldDefinitionWithoutAdditionalEdits() {
        val source = expressionHandlerSource(expressionValue = "this.")
        val candidate = fieldCandidate()
        val index = MixinExtrasDefinitionIndex(
            listOf(
                MixinExtrasDefinition(
                    id = "sampleField",
                    rawFieldReferences = listOf(sampleFieldSelector),
                ),
            ),
        )

        val result = plan(source, "this.", index, candidate)
        val available = assertIs<ExpressionDefinitionEditPlanResult.Available>(result)

        assertEquals("sampleField", available.insertText)
        assertTrue(available.additionalEdits.isEmpty())
    }

    @Test
    fun reusesExactMethodDefinitionWithoutAdditionalEdits() {
        val source = expressionHandlerSource(
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
            expressionValue = "value.",
        )
        val candidate = methodCandidate(
            ownerInternalName = "java/lang/String",
            name = "trim",
            descriptor = "()Ljava/lang/String;",
        )
        val index = MixinExtrasDefinitionIndex(
            listOf(
                MixinExtrasDefinition(
                    id = "trim",
                    rawMethodReferences = listOf(trimMethodSelector),
                ),
            ),
        )

        val result = plan(source, "value.", index, candidate)
        val available = assertIs<ExpressionDefinitionEditPlanResult.Available>(result)

        assertEquals("trim()", available.insertText)
        assertTrue(available.additionalEdits.isEmpty())
    }

    @Test
    fun overloadedMethodDescriptorDoesNotReuseWrongSelector() {
        val source = expressionHandlerSource(
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
            expressionValue = "value.",
        )
        val candidate = methodCandidate(
            ownerInternalName = "java/lang/String",
            name = "length",
            descriptor = "()I",
        )
        val index = MixinExtrasDefinitionIndex(
            listOf(
                MixinExtrasDefinition(
                    id = "trim",
                    rawMethodReferences = listOf(trimMethodSelector),
                ),
            ),
        )

        val result = plan(source, "value.", index, candidate)
        val available = assertIs<ExpressionDefinitionEditPlanResult.Available>(result)

        assertEquals("length()", available.insertText)
        assertTrue(
            available.additionalEdits.any {
                it.newText.contains("@Definition(id = \"length\", method = \"$lengthMethodSelector\")")
            },
        )
    }

    @Test
    fun conflictingSameIdGetsStableNumericSuffix() {
        val source = expressionHandlerSource(
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
            expressionValue = "value.",
        )
        val candidate = methodCandidate(
            ownerInternalName = "java/lang/String",
            name = "length",
            descriptor = "()I",
        )
        val index = MixinExtrasDefinitionIndex(
            listOf(
                MixinExtrasDefinition(
                    id = "length",
                    rawMethodReferences = listOf(trimMethodSelector),
                ),
            ),
        )

        val first = assertIs<ExpressionDefinitionEditPlanResult.Available>(plan(source, "value.", index, candidate))
        val second = assertIs<ExpressionDefinitionEditPlanResult.Available>(plan(source, "value.", index, candidate))

        assertEquals("length2()", first.insertText)
        assertEquals("length2()", second.insertText)
        assertTrue(first.additionalEdits.any { it.newText.contains("@Definition(id = \"length2\"") })
    }

    @Test
    fun duplicateAmbiguousIdIsNotReused() {
        val source = expressionHandlerSource(expressionValue = "this.")
        val candidate = fieldCandidate()
        val index = MixinExtrasDefinitionIndex(
            listOf(
                MixinExtrasDefinition(
                    id = "sampleField",
                    rawFieldReferences = listOf(sampleFieldSelector),
                ),
                MixinExtrasDefinition(
                    id = "sampleField",
                    rawFieldReferences = listOf("L$sampleOwner;otherField:I"),
                ),
            ),
        )

        val result = plan(source, "this.", index, candidate)
        val available = assertIs<ExpressionDefinitionEditPlanResult.Available>(result)

        assertEquals("sampleField2", available.insertText)
        assertTrue(available.additionalEdits.any { it.newText.contains("@Definition(id = \"sampleField2\"") })
    }

    @Test
    fun definitionInsertionUsesLfIndentation() {
        val source = expressionHandlerSource(expressionValue = "this.")
        val candidate = fieldCandidate()

        val result = plan(source, "this.", MixinExtrasDefinitionIndex(), candidate)
        val available = assertIs<ExpressionDefinitionEditPlanResult.Available>(result)

        val definitionEdit = available.additionalEdits.single { it.newText.contains("@Definition") }
        assertEquals(
            "@Definition(id = \"sampleField\", field = \"$sampleFieldSelector\")\n    ",
            definitionEdit.newText,
        )
        assertEquals(source.indexOf("@Expression"), definitionEdit.startOffset)
        val edited = source.replaceRange(definitionEdit.startOffset, definitionEdit.endOffset, definitionEdit.newText)
        assertTrue(edited.contains("    @Definition(id = \"sampleField\", field = \"$sampleFieldSelector\")\n    @Expression"))
    }

    @Test
    fun definitionInsertionUsesCrlfIndentation() {
        val source = expressionHandlerSource(expressionValue = "this.").replace("\n", "\r\n")
        val candidate = fieldCandidate()

        val result = plan(source, "this.", MixinExtrasDefinitionIndex(), candidate)
        val available = assertIs<ExpressionDefinitionEditPlanResult.Available>(result)

        val definitionEdit = available.additionalEdits.single { it.newText.contains("@Definition") }
        assertEquals(
            "@Definition(id = \"sampleField\", field = \"$sampleFieldSelector\")\r\n    ",
            definitionEdit.newText,
        )
        val edited = source.replaceRange(definitionEdit.startOffset, definitionEdit.endOffset, definitionEdit.newText)
        assertTrue(edited.contains("    @Definition(id = \"sampleField\", field = \"$sampleFieldSelector\")\r\n    @Expression"))
    }

    @Test
    fun importAddedOnceWhenMissing() {
        val source = """
            package com.example.mixin;

            @Mixin(ExpressionMatchSamples.class)
            abstract class ExampleMixin {
                @ModifyExpressionValue(method = "readSampleField()I", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
                @Expression("this.")
                private Object mcdev${'$'}handler(Object original) { return original; }
            }
        """.trimIndent()
        val candidate = fieldCandidate()

        val result = plan(source, "this.", MixinExtrasDefinitionIndex(), candidate)
        val available = assertIs<ExpressionDefinitionEditPlanResult.Available>(result)

        val importEdits = available.additionalEdits.filter {
            it.newText.contains("import com.llamalad7.mixinextras.expression.Definition;")
        }
        assertEquals(1, importEdits.size)
    }

    @Test
    fun importSkippedWhenAlreadyImported() {
        val source = """
            package com.example.mixin;

            import com.llamalad7.mixinextras.expression.Definition;

            @Mixin(ExpressionMatchSamples.class)
            abstract class ExampleMixin {
                @ModifyExpressionValue(method = "readSampleField()I", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
                @Expression("this.")
                private Object mcdev${'$'}handler(Object original) { return original; }
            }
        """.trimIndent()
        val candidate = fieldCandidate()

        val result = plan(source, "this.", MixinExtrasDefinitionIndex(), candidate)
        val available = assertIs<ExpressionDefinitionEditPlanResult.Available>(result)

        assertTrue(
            available.additionalEdits.none {
                it.newText.contains("import com.llamalad7.mixinextras.expression.Definition;")
            },
        )
    }

    @Test
    fun importSkippedWhenTargetSharesPackage() {
        val source = """
            package com.llamalad7.mixinextras.expression;

            @Mixin(ExpressionMatchSamples.class)
            abstract class ExampleMixin {
                @ModifyExpressionValue(method = "readSampleField()I", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
                @Expression("this.")
                private Object mcdev${'$'}handler(Object original) { return original; }
            }
        """.trimIndent()
        val candidate = fieldCandidate()

        val result = plan(source, "this.", MixinExtrasDefinitionIndex(), candidate)
        val available = assertIs<ExpressionDefinitionEditPlanResult.Available>(result)

        assertTrue(available.additionalEdits.none { it.newText.startsWith("import ") })
    }

    @Test
    fun appliedMainInsertPlusAdditionalEditsYieldsValidOrdering() {
        val source = expressionHandlerSource(expressionValue = "this.")
        val candidate = fieldCandidate()
        val context = expressionValueContextFromSource(source, "this.")

        val result = plan(source, "this.", MixinExtrasDefinitionIndex(), candidate)
        val available = assertIs<ExpressionDefinitionEditPlanResult.Available>(result)

        val mainEdit = McTextEdit(
            startOffset = context.valueStartOffset,
            endOffset = context.valueEndOffset,
            newText = available.insertText,
        )
        val updated = applyEdits(
            source,
            (listOf(mainEdit) + available.additionalEdits).sortedByDescending { it.startOffset },
        )

        val definitionOffset = updated.indexOf("@Definition")
        val expressionOffsetAfter = updated.indexOf("@Expression")
        assertTrue(definitionOffset >= 0)
        assertTrue(expressionOffsetAfter > definitionOffset)
        assertTrue(updated.contains("@Definition(id = \"sampleField\", field = \"$sampleFieldSelector\")"))
        assertTrue(updated.contains("@Expression(\"this.sampleField\")"))
    }

    @Test
    fun failsClosedForInvalidExpressionOffset() {
        val source = expressionHandlerSource(expressionValue = "this.")
        val context = expressionValueContextFromSource(source, "this.")

        val result = ExpressionDefinitionEditPlanner.plan(
            context = context,
            expressionAnnotationOffset = -1,
            source = source,
            definitionIndex = MixinExtrasDefinitionIndex(),
            candidate = fieldCandidate(),
        )
        assertEquals(
            ExpressionDefinitionEditPlanResult.Unavailable("invalid expression annotation offset"),
            result,
        )
    }

    @Test
    fun failsClosedWhenAnnotationOffsetDoesNotPointAtAt() {
        val source = expressionHandlerSource(expressionValue = "this.")
        val context = expressionValueContextFromSource(source, "this.")

        val result = ExpressionDefinitionEditPlanner.plan(
            context = context,
            expressionAnnotationOffset = source.indexOf("Expression"),
            source = source,
            definitionIndex = MixinExtrasDefinitionIndex(),
            candidate = fieldCandidate(),
        )
        assertEquals(
            ExpressionDefinitionEditPlanResult.Unavailable("expression annotation offset does not point at @"),
            result,
        )
    }

    @Test
    fun failsClosedWhenContextIsNotExpression() {
        val context = definitionContext()
        val result = ExpressionDefinitionEditPlanner.plan(
            context = context,
            expressionAnnotationOffset = 0,
            source = "@Definition(id = \"main\")",
            definitionIndex = MixinExtrasDefinitionIndex(),
            candidate = fieldCandidate(),
        )
        assertEquals(
            ExpressionDefinitionEditPlanResult.Unavailable("annotation is not Expression"),
            result,
        )
    }

    @Test
    fun failsClosedWhenContextIsNotValueSlot() {
        val source = expressionHandlerSource(expressionValue = "this.")
        val context = expressionValueContextFromSource(source, "this.")
            .copy(slot = AnnotationSlot.METHOD)

        val result = ExpressionDefinitionEditPlanner.plan(
            context = context,
            expressionAnnotationOffset = source.indexOf("@Expression"),
            source = source,
            definitionIndex = MixinExtrasDefinitionIndex(),
            candidate = fieldCandidate(),
        )
        assertEquals(
            ExpressionDefinitionEditPlanResult.Unavailable("expression completion context is not in VALUE slot"),
            result,
        )
    }

    @Test
    fun failsClosedWhenContextIsNotAfterDot() {
        val source = expressionHandlerSource(expressionValue = "")
        val context = expressionValueContextFromSource(source, "")

        val result = ExpressionDefinitionEditPlanner.plan(
            context = context,
            expressionAnnotationOffset = source.indexOf("@Expression"),
            source = source,
            definitionIndex = MixinExtrasDefinitionIndex(),
            candidate = fieldCandidate(),
        )
        assertEquals(
            ExpressionDefinitionEditPlanResult.Unavailable("expression is not in AFTER_DOT completion state"),
            result,
        )
    }

    @Test
    fun canonicalExactSelectorFormatsFieldAndMethod() {
        assertEquals(
            sampleFieldSelector,
            ExpressionDefinitionEditPlanner.canonicalExactSelector(fieldCandidate()),
        )
        assertEquals(
            trimMethodSelector,
            ExpressionDefinitionEditPlanner.canonicalExactSelector(
                methodCandidate(
                    ownerInternalName = "java/lang/String",
                    name = "trim",
                    descriptor = "()Ljava/lang/String;",
                ),
            ),
        )
    }

    private fun plan(
        source: String,
        valuePrefix: String,
        definitionIndex: MixinExtrasDefinitionIndex,
        candidate: OfficialExpressionMemberCandidate,
    ): ExpressionDefinitionEditPlanResult =
        ExpressionDefinitionEditPlanner.plan(
            context = expressionValueContextFromSource(source, valuePrefix),
            expressionAnnotationOffset = source.indexOf("@Expression"),
            source = source,
            definitionIndex = definitionIndex,
            candidate = candidate,
        )

    private fun expressionValueContextFromSource(
        source: String,
        valuePrefix: String,
    ): AnnotationContext {
        val annotationStart = source.indexOf("@Expression")
        val valueStart = source.indexOf('"', annotationStart) + 1
        val cursor = valueStart + valuePrefix.length
        return AnnotationContextExtractor.extractAtOffset(source, cursor)
            ?: error("failed to extract annotation context at offset $cursor")
    }

    private fun definitionContext(): AnnotationContext =
        AnnotationContext(
            annotation = MixinAnnotation.DEFINITION,
            slot = AnnotationSlot.VALUE,
            partialValue = "main",
            valueStartOffset = 17,
            valueEndOffset = 21,
            annotationStartOffset = 0,
            annotationEndOffset = 22,
        )

    private fun fieldCandidate(): OfficialExpressionMemberCandidate.FieldAccess =
        OfficialExpressionMemberCandidate.FieldAccess(
            ownerInternalName = sampleOwner,
            name = "sampleField",
            descriptor = "I",
            originalInstructionOpcode = Opcodes.GETFIELD,
            originalInstructionIndex = 0,
        )

    private fun methodCandidate(
        ownerInternalName: String,
        name: String,
        descriptor: String,
    ): OfficialExpressionMemberCandidate.MethodInvocation =
        OfficialExpressionMemberCandidate.MethodInvocation(
            ownerInternalName = ownerInternalName,
            name = name,
            descriptor = descriptor,
            isInterface = false,
            originalInstructionOpcode = Opcodes.INVOKEVIRTUAL,
            originalInstructionIndex = 0,
        )

    private fun expressionHandlerSource(
        targetMethod: String = "readSampleField()I",
        expressionValue: String,
    ): String = """
        @Mixin(ExpressionMatchSamples.class)
        abstract class ExampleMixin {
            @ModifyExpressionValue(method = "$targetMethod", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
            @Expression("$expressionValue")
            private Object mcdev${'$'}handler(Object original) { return original; }
        }
    """.trimIndent()

    private fun applyEdits(source: String, edits: List<McTextEdit>): String {
        var updated = source
        for (edit in edits) {
            updated = updated.substring(0, edit.startOffset) + edit.newText + updated.substring(edit.endOffset)
        }
        return updated
    }
}
