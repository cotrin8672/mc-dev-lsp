package io.github.mcdev.core.mixinextras

import kotlin.test.Test
import kotlin.test.assertEquals

class MixinExtrasExpressionIndexTest {
    @Test
    fun emptyIndexHasNoExpressions() {
        val index = MixinExtrasExpressionIndex()
        assertEquals(emptyList(), index.expressions)
        assertEquals(emptyList(), index.valuesForId("main"))
        assertEquals(emptyList(), index.valuesForId(""))
    }

    @Test
    fun preservesDocumentOrder() {
        val first = MixinExtrasExpression(id = "a", values = listOf("(this.one)"))
        val second = MixinExtrasExpression(values = listOf("(this.two)"))
        val third = MixinExtrasExpression(id = "c", values = listOf("(this.three)", "(this.four)"))
        val index = MixinExtrasExpressionIndex(listOf(first, second, third))
        assertEquals(listOf(first, second, third), index.expressions)
    }

    @Test
    fun valuesForIdReturnsAllMatchesFlatMappedInDocumentOrder() {
        val first = MixinExtrasExpression(id = "main", values = listOf("(this.a)", "(this.b)"))
        val other = MixinExtrasExpression(id = "other", values = listOf("(this.other)"))
        val second = MixinExtrasExpression(id = "main", values = listOf("(this.c)"))
        val index = MixinExtrasExpressionIndex(listOf(first, other, second))
        assertEquals(
            listOf("(this.a)", "(this.b)", "(this.c)"),
            index.valuesForId("main"),
        )
        assertEquals(listOf("(this.other)"), index.valuesForId("other"))
        assertEquals(emptyList(), index.valuesForId("missing"))
    }

    @Test
    fun valuesForIdIsCaseSensitive() {
        val lower = MixinExtrasExpression(id = "main", values = listOf("(this.lower)"))
        val upper = MixinExtrasExpression(id = "Main", values = listOf("(this.upper)"))
        val index = MixinExtrasExpressionIndex(listOf(lower, upper))
        assertEquals(listOf("(this.lower)"), index.valuesForId("main"))
        assertEquals(listOf("(this.upper)"), index.valuesForId("Main"))
        assertEquals(emptyList(), index.valuesForId("MAIN"))
    }

    @Test
    fun valuesForIdMatchesEmptyIdExactly() {
        val explicitEmpty = MixinExtrasExpression(id = "", values = listOf("(this.empty)"))
        val named = MixinExtrasExpression(id = "main", values = listOf("(this.main)"))
        val index = MixinExtrasExpressionIndex(listOf(explicitEmpty, named))
        assertEquals(listOf("(this.empty)"), index.valuesForId(""))
        assertEquals(listOf("(this.main)"), index.valuesForId("main"))
    }

    @Test
    fun integrationWithExpressionsAnnotationParserPreservesSameIdOrder() {
        val body = """
            value = {
                @Expression(id = "main", value = "(this.first)"),
                @Expression(id = "other", value = "(this.other)"),
                @Expression(id = "main", value = { "(this.second)", "(this.third)" })
            }
        """.trimIndent()
        val index = ExpressionsAnnotationParser.parse(body)
        assertEquals(
            listOf("(this.first)", "(this.second)", "(this.third)"),
            index.valuesForId("main"),
        )
        assertEquals(listOf("(this.other)"), index.valuesForId("other"))
    }
}
