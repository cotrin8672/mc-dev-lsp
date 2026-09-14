package io.github.mcdev.core.mixinextras

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class ExpressionAnnotationParserTest {
    @Test
    fun emptyBodyFailsWithoutRequiredValue() {
        assertNull(ExpressionAnnotationParser.parse(""))
    }

    @Test
    fun missingValueAttributeFails() {
        assertNull(ExpressionAnnotationParser.parse("""id = "main""""))
        assertNull(ExpressionAnnotationParser.parse("id = \"main\", other = 1"))
    }

    @Test
    fun parsesValueShorthandSingleString() {
        val parsed = ExpressionAnnotationParser.parse(""""(this.foo)"""")
        assertEquals(MixinExtrasExpression(values = listOf("(this.foo)")), parsed)
    }

    @Test
    fun decodesUnicodeEscapeInValueShorthand() {
        val parsed = ExpressionAnnotationParser.parse("\"\\u0064rawCall\"")
        assertEquals(MixinExtrasExpression(values = listOf("drawCall")), parsed)
    }

    @Test
    fun parsesValueShorthandArrayPreservingOrder() {
        val body = """
            {
                "(this.a)",
                "(this.b)"
            }
        """.trimIndent()
        val parsed = ExpressionAnnotationParser.parse(body)
        assertEquals(
            MixinExtrasExpression(values = listOf("(this.a)", "(this.b)")),
            parsed,
        )
    }

    @Test
    fun parsesNamedValueEquals() {
        val parsed = ExpressionAnnotationParser.parse("""value = "(this.foo)"""")
        assertEquals(MixinExtrasExpression(values = listOf("(this.foo)")), parsed)
    }

    @Test
    fun parsesNamedValueArray() {
        val parsed = ExpressionAnnotationParser.parse(
            """value = { "(this.a)", "(this.b)" }""",
        )
        assertEquals(
            MixinExtrasExpression(values = listOf("(this.a)", "(this.b)")),
            parsed,
        )
    }

    @Test
    fun idDefaultsToEmptyWhenOmitted() {
        val parsed = ExpressionAnnotationParser.parse("""value = "(this.foo)"""")
        assertEquals("", parsed?.id)
    }

    @Test
    fun parsesExplicitEmptyStringId() {
        val parsed = ExpressionAnnotationParser.parse("""id = "", value = "(this.foo)"""")
        assertEquals("", parsed?.id)
    }

    @Test
    fun parsesIdAndValueInEitherOrder() {
        val forward = ExpressionAnnotationParser.parse("""id = "main", value = "(this.foo)"""")
        val reverse = ExpressionAnnotationParser.parse("""value = "(this.foo)", id = "main"""")
        assertEquals(MixinExtrasExpression(id = "main", values = listOf("(this.foo)")), forward)
        assertEquals(MixinExtrasExpression(id = "main", values = listOf("(this.foo)")), reverse)
    }

    @Test
    fun parsesFullyQualifiedStyleStringsWithSpecialCharacters() {
        val parsed = ExpressionAnnotationParser.parse(
            """value = "(this.foo, bar), {baz} // comment-like / slash seq"""",
        )
        assertEquals(
            MixinExtrasExpression(values = listOf("(this.foo, bar), {baz} // comment-like / slash seq")),
            parsed,
        )
    }

    @Test
    fun unknownNamedAttributeFailsWholeExpression() {
        val cases = listOf(
            """other = 1, value = "(this.foo)"""",
            """value = "(this.foo)", other = 1""",
            """id = "main", other = 1, value = "(this.foo)"""",
            """id = "main", value = "(this.foo)", remap = true""",
        )
        for (body in cases) {
            assertNull(ExpressionAnnotationParser.parse(body), "expected null for: $body")
        }
    }

    @Test
    fun decodesValidJavaBasicEscapes() {
        val cases = listOf(
            "\\b" to "\u0008",
            "\\t" to "\t",
            "\\n" to "\n",
            "\\f" to "\u000C",
            "\\r" to "\r",
            "\\\"" to "\"",
            "\\'" to "'",
            "\\\\" to "\\",
            "\\s" to " ",
        )
        for ((escape, expectedChar) in cases) {
            val body = """value = "$escape""""
            val parsed = ExpressionAnnotationParser.parse(body)
            assertEquals(
                MixinExtrasExpression(values = listOf(expectedChar)),
                parsed,
                "escape sequence $escape",
            )
        }
    }

    @Test
    fun decodesValidJavaOctalEscapes() {
        val cases = listOf(
            """\0""" to "\u0000",
            """\7""" to "\u0007",
            """\77""" to "?",
            """\377""" to "\u00FF",
            """\123""" to "S",
            """\12""" to "\n",
            """\128""" to "\n8",
        )
        for ((escape, expected) in cases) {
            val body = """value = "$escape""""
            val parsed = ExpressionAnnotationParser.parse(body)
            assertEquals(
                MixinExtrasExpression(values = listOf(expected)),
                parsed,
                "octal escape $escape",
            )
        }
    }

    @Test
    fun invalidAndUnicodeEscapesFailClosed() {
        val cases = listOf(
            """value = "\z"""",
            """value = "\x"""",
            """value = "\"""",
            """value = "\u"""",
            """value = "\u041"""",
            """value = "\u004G"""",
            """value = "a\qb"""",
        )
        for (body in cases) {
            assertNull(ExpressionAnnotationParser.parse(body), "expected null for: $body")
        }
    }

    @Test
    fun parsesEscapedQuoteAndBackslashInExpressionValue() {
        val parsed = ExpressionAnnotationParser.parse("""value = "a\\\"b\\\\c"""")
        assertEquals(MixinExtrasExpression(values = listOf("""a\"b\\c""")), parsed)
    }

    @Test
    fun emptyValueArrayRemainsEmpty() {
        val parsed = ExpressionAnnotationParser.parse("value = { }")
        assertEquals(MixinExtrasExpression(values = emptyList()), parsed)
    }

    @Test
    fun handlesCommentsWhitespaceAndTrailingComma() {
        val body = """
            // leading
            id = /* inline */ "main",
            value = {
                "(this.a)", // trailing on element
                "(this.b)",
            },
        """.trimIndent()
        val parsed = ExpressionAnnotationParser.parse(body)
        assertEquals(
            MixinExtrasExpression(id = "main", values = listOf("(this.a)", "(this.b)")),
            parsed,
        )
    }

    @Test
    fun malformedExplicitIdFailsWholeExpression() {
        assertNull(ExpressionAnnotationParser.parse("""id = not-a-string, value = "(this.foo)""""))
        assertNull(ExpressionAnnotationParser.parse("""id = "main", id = "other", value = "(this.foo)""""))
    }

    @Test
    fun malformedRequiredValueFailsWholeExpression() {
        assertNull(ExpressionAnnotationParser.parse("value = not-a-string"))
        assertNull(ExpressionAnnotationParser.parse("""value = "(unclosed"""))
    }

    @Test
    fun malformedInnerStringArrayElementFailsWithoutPartialList() {
        assertNull(ExpressionAnnotationParser.parse("""value = { "(good)", not-a-string }"""))
        assertNull(ExpressionAnnotationParser.parse("""value = { "(good)", "(bad""" + "\""))
        assertNull(ExpressionAnnotationParser.parse("""value = { "(good)", broken }"""))
    }

    @Test
    fun duplicateValueAttributeFails() {
        assertNull(
            ExpressionAnnotationParser.parse(
                """value = "(first)", value = "(second)"""",
            ),
        )
    }

    @Test
    fun valueShorthandWithTrailingGarbageFails() {
        assertNull(ExpressionAnnotationParser.parse(""""(this.foo)" , id = "main""""))
    }

    @Test
    fun unclosedValueArrayFails() {
        assertNull(ExpressionAnnotationParser.parse("""value = { "(this.foo)""""))
        assertNull(ExpressionAnnotationParser.parse("""{ "(this.foo)""""))
    }
}
