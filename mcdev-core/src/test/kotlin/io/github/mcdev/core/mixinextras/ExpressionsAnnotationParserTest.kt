package io.github.mcdev.core.mixinextras

import kotlin.test.Test
import kotlin.test.assertEquals

class ExpressionsAnnotationParserTest {
    @Test
    fun emptyBodyReturnsEmptyIndex() {
        assertEquals(MixinExtrasExpressionIndex(), ExpressionsAnnotationParser.parse(""))
    }

    @Test
    fun parsesSingleExpressionViaValueEquals() {
        val parsed = ExpressionsAnnotationParser.parse(
            """value = @Expression(value = "(this.foo)")""",
        )
        assertEquals(
            MixinExtrasExpressionIndex(listOf(MixinExtrasExpression(values = listOf("(this.foo)")))),
            parsed,
        )
    }

    @Test
    fun parsesSingleExpressionViaUnnamedShorthand() {
        val parsed = ExpressionsAnnotationParser.parse(
            """@Expression(value = "(this.foo)")""",
        )
        assertEquals(
            MixinExtrasExpressionIndex(listOf(MixinExtrasExpression(values = listOf("(this.foo)")))),
            parsed,
        )
    }

    @Test
    fun parsesExpressionArrayPreservingOrder() {
        val body = """
            value = {
                @Expression(id = "first", value = "(this.a)"),
                @Expression(value = "(this.b)"),
                @Expression(id = "third", value = "(this.c)")
            }
        """.trimIndent()
        val parsed = ExpressionsAnnotationParser.parse(body)
        assertEquals(
            MixinExtrasExpressionIndex(
                listOf(
                    MixinExtrasExpression(id = "first", values = listOf("(this.a)")),
                    MixinExtrasExpression(values = listOf("(this.b)")),
                    MixinExtrasExpression(id = "third", values = listOf("(this.c)")),
                ),
            ),
            parsed,
        )
    }

    @Test
    fun parsesArrayViaUnnamedShorthand() {
        val body = """
            {
                @Expression(id = "a", value = "(this.a)"),
                @Expression(id = "b", value = "(this.b)")
            }
        """.trimIndent()
        val parsed = ExpressionsAnnotationParser.parse(body)
        assertEquals(
            MixinExtrasExpressionIndex(
                listOf(
                    MixinExtrasExpression(id = "a", values = listOf("(this.a)")),
                    MixinExtrasExpression(id = "b", values = listOf("(this.b)")),
                ),
            ),
            parsed,
        )
    }

    @Test
    fun duplicateIdExpressionsAreBothRetainedInDocumentOrder() {
        val body = """
            value = {
                @Expression(id = "main", value = "(this.a)"),
                @Expression(id = "main", value = "(this.b)")
            }
        """.trimIndent()
        val parsed = ExpressionsAnnotationParser.parse(body)
        assertEquals(
            MixinExtrasExpressionIndex(
                listOf(
                    MixinExtrasExpression(id = "main", values = listOf("(this.a)")),
                    MixinExtrasExpression(id = "main", values = listOf("(this.b)")),
                ),
            ),
            parsed,
        )
    }

    @Test
    fun parsesEmptyArray() {
        assertEquals(MixinExtrasExpressionIndex(), ExpressionsAnnotationParser.parse("value = { }"))
        assertEquals(MixinExtrasExpressionIndex(), ExpressionsAnnotationParser.parse("{}"))
    }

    @Test
    fun parsesFullyQualifiedExpression() {
        val parsed = ExpressionsAnnotationParser.parse(
            """value = @com.llamalad7.mixinextras.expression.Expression(value = "(this.foo)")""",
        )
        assertEquals(
            MixinExtrasExpressionIndex(listOf(MixinExtrasExpression(values = listOf("(this.foo)")))),
            parsed,
        )
    }

    @Test
    fun bareExpressionWithoutParenthesesIsExcluded() {
        assertEquals(
            MixinExtrasExpressionIndex(),
            ExpressionsAnnotationParser.parse("value = @Expression"),
        )
    }

    @Test
    fun expressionWithoutValueIsExcluded() {
        assertEquals(
            MixinExtrasExpressionIndex(),
            ExpressionsAnnotationParser.parse("""value = @Expression(id = "main")"""),
        )
        assertEquals(
            MixinExtrasExpressionIndex(),
            ExpressionsAnnotationParser.parse("value = @Expression()"),
        )
    }

    @Test
    fun malformedIdInsideExpressionIsExcluded() {
        assertEquals(
            MixinExtrasExpressionIndex(),
            ExpressionsAnnotationParser.parse(
                """value = @Expression(id = not-a-string, value = "(this.foo)")""",
            ),
        )
    }

    @Test
    fun malformedValueInsideExpressionIsExcluded() {
        assertEquals(
            MixinExtrasExpressionIndex(),
            ExpressionsAnnotationParser.parse(
                """value = @Expression(value = { "(good)", broken })""",
            ),
        )
    }

    @Test
    fun nonOfficialExpressionFqnIsExcluded() {
        assertEquals(
            MixinExtrasExpressionIndex(),
            ExpressionsAnnotationParser.parse(
                """value = @foo.Expression(value = "(this.foo)")""",
            ),
        )
    }

    @Test
    fun trailingGarbageAtTopLevelFailsClosed() {
        assertEquals(
            MixinExtrasExpressionIndex(),
            ExpressionsAnnotationParser.parse(
                """value = @Expression(value = "(this.foo)") garbage""",
            ),
        )
    }

    @Test
    fun handlesCommentsWhitespaceAndNestedExpressionAttributes() {
        val body = """
            // leading
            value = { /* one */ @Expression(id = "main", value = { "(this.a)", "(this.b)" }) }
        """.trimIndent()
        val parsed = ExpressionsAnnotationParser.parse(body)
        assertEquals(
            MixinExtrasExpressionIndex(
                listOf(
                    MixinExtrasExpression(
                        id = "main",
                        values = listOf("(this.a)", "(this.b)"),
                    ),
                ),
            ),
            parsed,
        )
    }

    @Test
    fun ignoresWrongAnnotationSiblings() {
        val parsed = ExpressionsAnnotationParser.parse(
            """
            value = {
                @Expression(id = "main", value = "(this.foo)"),
                @Share("slot"),
                @Definition(id = "foo"),
                @Expression(id = "other")
            }
            """.trimIndent(),
        )
        assertEquals(
            MixinExtrasExpressionIndex(
                listOf(MixinExtrasExpression(id = "main", values = listOf("(this.foo)"))),
            ),
            parsed,
        )
    }

    @Test
    fun malformedExpressionElementsAreIgnoredWithoutDiscardingValidSiblings() {
        val parsed = ExpressionsAnnotationParser.parse(
            """
            value = {
                @Expression(id = "first", value = "(this.a)"),
                broken,
                @Share("x"),
                @Expression(value = { "(good)", broken }),
                @Expression(id = "last", value = "(this.b)")
            }
            """.trimIndent(),
        )
        assertEquals(
            MixinExtrasExpressionIndex(
                listOf(
                    MixinExtrasExpression(id = "first", values = listOf("(this.a)")),
                    MixinExtrasExpression(id = "last", values = listOf("(this.b)")),
                ),
            ),
            parsed,
        )
    }

    @Test
    fun failClosedOnUnclosedArrayBrace() {
        assertEquals(
            MixinExtrasExpressionIndex(),
            ExpressionsAnnotationParser.parse(
                """value = { @Expression(value = "(this.foo)")""",
            ),
        )
        assertEquals(
            MixinExtrasExpressionIndex(),
            ExpressionsAnnotationParser.parse(
                """{ @Expression(value = "(this.foo)")""",
            ),
        )
    }

    @Test
    fun failClosedOnUnclosedExpressionParenInSingleValue() {
        assertEquals(
            MixinExtrasExpressionIndex(),
            ExpressionsAnnotationParser.parse(
                """value = @Expression(value = "(this.foo"""",
            ),
        )
        assertEquals(
            MixinExtrasExpressionIndex(),
            ExpressionsAnnotationParser.parse(
                """@Expression(value = "(this.foo"""",
            ),
        )
    }

    @Test
    fun failClosedOnRootArrayTrailingGarbage() {
        assertEquals(
            MixinExtrasExpressionIndex(),
            ExpressionsAnnotationParser.parse(
                """
                {
                    @Expression(id = "a", value = "(this.a)"),
                    @Expression(id = "b", value = "(this.b)")
                } garbage
                """.trimIndent(),
            ),
        )
    }

    @Test
    fun failClosedOnNamedArrayTrailingGarbage() {
        assertEquals(
            MixinExtrasExpressionIndex(),
            ExpressionsAnnotationParser.parse(
                """
                value = {
                    @Expression(id = "a", value = "(this.a)"),
                    @Expression(id = "b", value = "(this.b)")
                } garbage
                """.trimIndent(),
            ),
        )
    }

    @Test
    fun failClosedOnUnknownMember() {
        assertEquals(
            MixinExtrasExpressionIndex(),
            ExpressionsAnnotationParser.parse(
                """other = 1, value = @Expression(value = "(this.foo)")""",
            ),
        )
        assertEquals(
            MixinExtrasExpressionIndex(),
            ExpressionsAnnotationParser.parse(
                """value = @Expression(value = "(this.foo)"), other = 1""",
            ),
        )
    }

    @Test
    fun failClosedOnDuplicateValue() {
        assertEquals(
            MixinExtrasExpressionIndex(),
            ExpressionsAnnotationParser.parse(
                """
                value = @Expression(value = "(this.a)"),
                value = @Expression(value = "(this.b)")
                """.trimIndent(),
            ),
        )
        assertEquals(
            MixinExtrasExpressionIndex(),
            ExpressionsAnnotationParser.parse(
                """
                value = {
                    @Expression(value = "(this.a)")
                },
                value = {
                    @Expression(value = "(this.b)")
                }
                """.trimIndent(),
            ),
        )
    }

    @Test
    fun expressionWithValueShorthandInsideAnnotation() {
        val parsed = ExpressionsAnnotationParser.parse(
            """value = @Expression("(this.foo)")""",
        )
        assertEquals(
            MixinExtrasExpressionIndex(listOf(MixinExtrasExpression(values = listOf("(this.foo)")))),
            parsed,
        )
    }
}
