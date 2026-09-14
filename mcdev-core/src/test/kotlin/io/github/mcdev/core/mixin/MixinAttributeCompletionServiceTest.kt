package io.github.mcdev.core.mixin

import io.github.mcdev.core.completion.McCompletionInsertTextFormat
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

class MixinAttributeCompletionServiceTest {
    private val service = MixinAttributeCompletionService()

    @Test
    fun methodAttributeInsertsQuotedSnippetWithCursorInside() {
        val items = service.complete(context(MixinAnnotation.INJECT, "meth"))
        val method = items.single { it.metadata.name == "method" }
        assertEquals("method = \"${'$'}{1}\"${'$'}0", method.insertText)
        assertEquals(McCompletionInsertTextFormat.SNIPPET, method.insertTextFormat)
        assertEquals("method = \"…\"", method.label)
    }

    @Test
    fun atAttributeBuildsNestedAtSnippet() {
        val at = service.complete(context(MixinAnnotation.INJECT, "at"))
            .single { it.metadata.name == "at" }
        assertEquals("at = @At(\"${'$'}{1}\")${'$'}0", at.insertText)
    }

    @Test
    fun excludesOnlyAttributesAlreadyPresentOnTheSameAnnotation() {
        val items = service.complete(
            context(MixinAnnotation.INJECT, "re").copy(existingAttributes = setOf("method", "at")),
        )
        assertTrue(items.any { it.metadata.name == "remap" })
        assertFalse(items.any { it.metadata.name == "method" })
        assertFalse(items.any { it.metadata.name == "at" })
    }

    @Test
    fun vanillaInjectStillSuggestsConstraintsAttribute() {
        val names = attributeNames(MixinAnnotation.INJECT, "")
        assertTrue("constraints" in names)
    }

    @Test
    fun mixinExtrasInjectorDoesNotSuggestConstraintsAttribute() {
        val names = attributeNames(MixinAnnotation.MODIFY_EXPRESSION_VALUE, "")
        assertFalse("constraints" in names)
        assertTrue("require" in names)
        assertTrue("expect" in names)
        assertTrue("allow" in names)
        assertTrue("remap" in names)
        assertTrue("order" in names)
    }

    @Test
    fun wrapWithConditionOrderDependsOnResolvedOfficialFqn() {
        data class Case(val source: String, val expectedOrder: Boolean)

        val v1Fqn = "com.llamalad7.mixinextras.injector.WrapWithCondition"
        val v2Fqn = "com.llamalad7.mixinextras.injector.v2.WrapWithCondition"
        val cases = listOf(
            Case("@$v1Fqn()", false),
            Case("import $v1Fqn;\n@WrapWithCondition()", false),
            Case("@$v2Fqn()", true),
            Case("import $v2Fqn;\n@WrapWithCondition()", true),
            Case("@WrapWithCondition()", true),
        )

        cases.forEach { (source, expectedOrder) ->
            val cursor = source.lastIndexOf(')')
            val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
            val names = attributeNames(context)
            assertEquals(expectedOrder, "order" in names, "order completion for $source")
            assertTrue("method" in names)
            assertTrue("at" in names)
        }
    }

    @Test
    fun wrapOperationDoesNotSuggestConstraintsAndKeepsAtConstantExclusivity() {
        val names = attributeNames(MixinAnnotation.WRAP_OPERATION, "")
        assertFalse("constraints" in names)
        assertTrue("constant" in names)
        assertTrue("order" in names)

        val withAt = service.complete(
            context(MixinAnnotation.WRAP_OPERATION, "").copy(existingAttributes = setOf("at")),
        )
        assertFalse(withAt.any { it.metadata.name == "constant" })
        assertTrue(withAt.any { it.metadata.name == "order" })

        val withConstant = service.complete(
            context(MixinAnnotation.WRAP_OPERATION, "").copy(existingAttributes = setOf("constant")),
        )
        assertFalse(withConstant.any { it.metadata.name == "at" })
    }

    @Test
    fun wrapMethodDoesNotSuggestConstraintsAttribute() {
        val names = attributeNames(MixinAnnotation.WRAP_METHOD, "")
        assertFalse("constraints" in names)
        assertTrue("require" in names)
        assertTrue("order" in names)
        assertFalse("at" in names)
    }

    @Test
    fun localSuggestsOnlyValidMixinExtrasAttributes() {
        val names = attributeNames(MixinAnnotation.LOCAL, "")
        assertEquals(
            setOf("print", "ordinal", "index", "name", "argsOnly", "type"),
            names,
        )
    }

    @Test
    fun localAttributePrefixFiltersCandidates() {
        val names = attributeNames(MixinAnnotation.LOCAL, "ord")
        assertEquals(setOf("ordinal"), names)
    }

    @Test
    fun localExcludesExistingAttributes() {
        val items = service.complete(
            context(MixinAnnotation.LOCAL, "").copy(existingAttributes = setOf("ordinal", "print")),
        )
        assertFalse(items.any { it.metadata.name == "ordinal" })
        assertFalse(items.any { it.metadata.name == "print" })
        assertTrue(items.any { it.metadata.name == "index" })
    }

    @Test
    fun localUsesExpectedSnippetKinds() {
        val items = service.complete(context(MixinAnnotation.LOCAL, ""))
        assertEquals("print = true/false", items.single { it.metadata.name == "print" }.label)
        assertEquals("ordinal = …", items.single { it.metadata.name == "ordinal" }.label)
        assertEquals("name = { \"…\" }", items.single { it.metadata.name == "name" }.label)
        assertEquals("type = ….class", items.single { it.metadata.name == "type" }.label)
    }

    @Test
    fun shareSuggestsOnlyValueAndNamespace() {
        val names = attributeNames(MixinAnnotation.SHARE, "")
        assertEquals(setOf("value", "namespace"), names)
    }

    @Test
    fun shareAttributePrefixFiltersCandidates() {
        val names = attributeNames(MixinAnnotation.SHARE, "nam")
        assertEquals(setOf("namespace"), names)
    }

    @Test
    fun shareExcludesExistingAttributes() {
        val items = service.complete(
            context(MixinAnnotation.SHARE, "").copy(existingAttributes = setOf("value")),
        )
        assertFalse(items.any { it.metadata.name == "value" })
        assertTrue(items.any { it.metadata.name == "namespace" })
    }

    @Test
    fun definitionSuggestsOnlyValidMixinExtrasAttributes() {
        val names = attributeNames(MixinAnnotation.DEFINITION, "")
        assertEquals(setOf("id", "method", "field", "type", "local", "remap"), names)
    }

    @Test
    fun definitionAttributePrefixFiltersCandidates() {
        val names = attributeNames(MixinAnnotation.DEFINITION, "met")
        assertEquals(setOf("method"), names)
    }

    @Test
    fun definitionExcludesExistingAttributes() {
        val items = service.complete(
            context(MixinAnnotation.DEFINITION, "").copy(existingAttributes = setOf("id", "remap")),
        )
        assertFalse(items.any { it.metadata.name == "id" })
        assertFalse(items.any { it.metadata.name == "remap" })
        assertTrue(items.any { it.metadata.name == "method" })
    }

    @Test
    fun expressionSuggestsOnlyValueAndId() {
        val names = attributeNames(MixinAnnotation.EXPRESSION, "")
        assertEquals(setOf("value", "id"), names)
    }

    @Test
    fun expressionAttributePrefixFiltersCandidates() {
        val names = attributeNames(MixinAnnotation.EXPRESSION, "id")
        assertEquals(setOf("id"), names)
    }

    @Test
    fun expressionExcludesExistingAttributes() {
        val items = service.complete(
            context(MixinAnnotation.EXPRESSION, "").copy(existingAttributes = setOf("value")),
        )
        assertFalse(items.any { it.metadata.name == "value" })
        assertTrue(items.any { it.metadata.name == "id" })
    }

    @Test
    fun definitionsSuggestsOnlyValueAttribute() {
        val names = attributeNames(MixinAnnotation.DEFINITIONS, "")
        assertEquals(setOf("value"), names)
    }

    @Test
    fun expressionsSuggestsOnlyValueAttribute() {
        val names = attributeNames(MixinAnnotation.EXPRESSIONS, "")
        assertEquals(setOf("value"), names)
    }

    @Test
    fun definitionUsesExpectedSnippetKinds() {
        val items = service.complete(context(MixinAnnotation.DEFINITION, ""))
        assertEquals("id = \"…\"", items.single { it.metadata.name == "id" }.label)
        assertEquals("method = { \"…\" }", items.single { it.metadata.name == "method" }.label)
        assertEquals("field = { \"…\" }", items.single { it.metadata.name == "field" }.label)
        assertEquals("type = { ….class }", items.single { it.metadata.name == "type" }.label)
        assertEquals("local = { @Local(…) }", items.single { it.metadata.name == "local" }.label)
        assertEquals("remap = true/false", items.single { it.metadata.name == "remap" }.label)
    }

    @Test
    fun definitionsAndExpressionsUseNestedAnnotationSnippets() {
        val definitions = service.complete(context(MixinAnnotation.DEFINITIONS, "")).single()
        assertEquals("value = { @Definition(…) }", definitions.label)
        assertEquals("value = { @Definition(id = \"${'$'}{1}\") }${'$'}0", definitions.insertText)

        val expressions = service.complete(context(MixinAnnotation.EXPRESSIONS, "")).single()
        assertEquals("value = { @Expression(…) }", expressions.label)
        assertEquals("value = { @Expression(\"${'$'}{1}\") }${'$'}0", expressions.insertText)
    }

    @Test
    fun constantSuggestsExactAttributeNamesWithPrefixAndExistingFiltering() {
        val names = attributeNames(MixinAnnotation.CONSTANT, "")
        assertEquals(
            setOf(
                "nullValue",
                "intValue",
                "floatValue",
                "longValue",
                "doubleValue",
                "stringValue",
                "classValue",
                "ordinal",
                "slice",
                "expandZeroConditions",
                "log",
            ),
            names,
        )
        assertEquals(setOf("intValue"), attributeNames(MixinAnnotation.CONSTANT, "int"))
        val filtered = service.complete(
            context(MixinAnnotation.CONSTANT, "").copy(existingAttributes = setOf("intValue", "log")),
        )
        assertFalse(filtered.any { it.metadata.name == "intValue" })
        assertFalse(filtered.any { it.metadata.name == "log" })
        assertTrue(filtered.any { it.metadata.name == "stringValue" })
    }

    @Test
    fun sliceSuggestsExactAttributeNamesWithPrefixAndExistingFiltering() {
        val names = attributeNames(MixinAnnotation.SLICE, "")
        assertEquals(setOf("id", "from", "to"), names)
        assertEquals(setOf("from"), attributeNames(MixinAnnotation.SLICE, "fr"))
        val filtered = service.complete(
            context(MixinAnnotation.SLICE, "").copy(existingAttributes = setOf("from")),
        )
        assertFalse(filtered.any { it.metadata.name == "from" })
        assertTrue(filtered.any { it.metadata.name == "to" })
    }

    private fun attributeNames(annotation: MixinAnnotation, partial: String): Set<String> =
        service.complete(context(annotation, partial)).mapNotNull { it.metadata.name }.toSet()

    private fun attributeNames(context: AnnotationContext): Set<String> =
        service.complete(context).mapNotNull { it.metadata.name }.toSet()

    private fun context(annotation: MixinAnnotation, partial: String) = AnnotationContext(
        annotation = annotation,
        slot = AnnotationSlot.ATTRIBUTE,
        partialValue = partial,
        valueStartOffset = 0,
        valueEndOffset = partial.length,
        annotationStartOffset = 0,
        annotationEndOffset = partial.length,
    )
}
