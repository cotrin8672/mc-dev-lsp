package io.github.mcdev.core.mixinextras

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class ExpressionReceiverParserTest {
    @Test
    fun thisDotEmptyMemberPrefix() {
        assertParsed("this.", 5, "this", "")
    }

    @Test
    fun thisDotPartialMemberPrefix() {
        assertParsed("this.fi", 7, "this", "fi")
    }

    @Test
    fun qualifiedReceiverWithPartialMemberPrefix() {
        assertParsed("foo.bar.baz", 11, "foo.bar", "baz")
    }

    @Test
    fun superDotEmptyMemberPrefix() {
        assertParsed("super.", 6, "super", "")
    }

    @Test
    fun whitespaceAfterDotAllowsEmptyMemberPrefix() {
        assertParsed("this. ", 6, "this", "")
    }

    @Test
    fun receiverChainStopsBeforePrecedingExpression() {
        assertParsed("a + this.fi", 11, "this", "fi")
    }

    @Test
    fun parsesOfficialCompoundReceivers() {
        assertParsed("value.trim().", "value.trim().".length, "value.trim()", "")
        assertParsed("this.getChild().", "this.getChild().".length, "this.getChild()", "")
        assertParsed("values[index].le", "values[index].le".length, "values[index]", "le")
        assertParsed("(String) value.", "(String) value.".length, "(String) value", "")
        assertParsed("((String)value).le", "((String)value).le".length, "((String)value)", "le")
        assertParsed("(value.trim()).le", "(value.trim()).le".length, "(value.trim())", "le")
        assertParsed("new String().", "new String().".length, "new String()", "")
    }

    @Test
    fun parsesReceiverInsideUnclosedOuterCallCaptureAndArrayIndex() {
        assertParsed("someCall(this.", "someCall(this.".length, "this", "")
        assertParsed("@(this.", "@(this.".length, "this", "")
        assertParsed("values[this.", "values[this.".length, "this", "")
        assertParsed("someCall(a + this.", "someCall(a + this.".length, "this", "")
        assertParsed("(this.", "(this.".length, "this", "")
    }

    @Test
    fun rejectsNewKeywordReceiver() {
        assertRejected("new.", 4)
    }

    @Test
    fun rejectsMethodReference() {
        assertRejected("foo::", 5)
        assertRejected("foo::name", 9)
        assertRejected("foo::name.", "foo::name.".length)
    }

    @Test
    fun rejectsNumericLiteralReceiver() {
        assertRejected("123.", 4)
    }

    @Test
    fun rejectsSingleQuotedLiteralReceiver() {
        assertRejected("'hello'.", 8)
    }

    @Test
    fun rejectsBooleanAndNullLiteralReceivers() {
        assertRejected("true.", 5)
        assertRejected("false.", 6)
        assertRejected("null.", 5)
    }

    @Test
    fun rejectsMissingDot() {
        assertRejected("this", 4)
        assertRejected("foo", 3)
    }

    @Test
    fun rejectsInvalidMemberPrefixTail() {
        assertRejected("this.fi(", 8)
        assertRejected("this.fi-", 8)
        assertRejected("this.2fi", 8)
    }

    @Test
    fun rejectsMalformedDelimiters() {
        assertRejected("this.(", 6)
        assertRejected("this..fi", 8)
        assertRejected("value.trim(", "value.trim(".length)
        assertParsed("values[index.", "values[index.".length, "index", "")
        assertParsed("((String)value.", "((String)value.".length, "(String)value", "")
    }

    @Test
    fun rejectsClassLiteralSuffix() {
        assertRejected("Type.class", 10)
    }

    @Test
    fun memberPrefixUsesCursorNotFullPrefix() {
        assertParsed("this.field", 5, "this", "")
    }

    private fun assertParsed(
        expressionPrefix: String,
        cursor: Int,
        expectedReceiver: String,
        expectedMemberPrefix: String,
    ) {
        val parsed = ExpressionReceiverParser.parse(expressionPrefix, cursor)
        assertEquals(
            ParsedExpressionReceiver(expectedReceiver, expectedMemberPrefix),
            parsed,
            "expressionPrefix=$expressionPrefix cursor=$cursor",
        )
    }

    private fun assertRejected(expressionPrefix: String, cursor: Int) {
        assertNull(
            ExpressionReceiverParser.parse(expressionPrefix, cursor),
            "expected rejection for expressionPrefix=$expressionPrefix cursor=$cursor",
        )
    }
}
