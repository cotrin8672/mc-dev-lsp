package io.github.mcdev.core.mixinextras

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class ExpressionCompletionLexerTest {
    @Test
    fun thTokenAtStartRetainsValueStartPosition() {
        val result = ExpressionCompletionLexer.lex("th", 2)
        assertEquals(ExpressionCompletionPosition.VALUE_START, result.position)
        assertEquals(0, result.tokenStart)
        assertEquals(2, result.tokenEnd)
        assertTrue(result.hasIdentifierToken())
    }

    @Test
    fun aPlusThRetainsValueStartPosition() {
        val result = ExpressionCompletionLexer.lex("a + th", 6)
        assertEquals(ExpressionCompletionPosition.VALUE_START, result.position)
        assertEquals(4, result.tokenStart)
        assertEquals(6, result.tokenEnd)
    }

    @Test
    fun thisDotEmptyIsAfterDot() {
        val result = ExpressionCompletionLexer.lex("this.", 5)
        assertEquals(ExpressionCompletionPosition.AFTER_DOT, result.position)
        assertEquals(5, result.tokenStart)
        assertEquals(5, result.tokenEnd)
    }

    @Test
    fun identifierAfterDotRetainsAfterDotPosition() {
        val result = ExpressionCompletionLexer.lex("this.fi", 7)
        assertEquals(ExpressionCompletionPosition.AFTER_DOT, result.position)
        assertEquals(5, result.tokenStart)
        assertEquals(7, result.tokenEnd)
    }

    @Test
    fun identifierAfterDotAtTokenStartUsesEmptyToken() {
        val result = ExpressionCompletionLexer.lex("this.foo", 5)
        assertEquals(ExpressionCompletionPosition.AFTER_DOT, result.position)
        assertEquals(5, result.tokenStart)
        assertEquals(5, result.tokenEnd)
    }

    @Test
    fun fooMethodReferenceEmptyIsAfterMethodReference() {
        val result = ExpressionCompletionLexer.lex("foo::", 5)
        assertEquals(ExpressionCompletionPosition.AFTER_METHOD_REFERENCE, result.position)
        assertEquals(5, result.tokenStart)
        assertEquals(5, result.tokenEnd)
    }

    @Test
    fun identifierAfterMethodReferenceRetainsAfterMethodReferencePosition() {
        val result = ExpressionCompletionLexer.lex("foo::n", 6)
        assertEquals(ExpressionCompletionPosition.AFTER_METHOD_REFERENCE, result.position)
        assertEquals(5, result.tokenStart)
        assertEquals(6, result.tokenEnd)
    }

    @Test
    fun blockStartIsStatementStart() {
        val result = ExpressionCompletionLexer.lex("{", 1)
        assertEquals(ExpressionCompletionPosition.STATEMENT_START, result.position)
        assertEquals(1, result.tokenStart)
        assertEquals(1, result.tokenEnd)
    }

    @Test
    fun blockAfterSemicolonIsStatementStart() {
        val result = ExpressionCompletionLexer.lex("{ return x; ", 12)
        assertEquals(ExpressionCompletionPosition.STATEMENT_START, result.position)
    }

    @Test
    fun blockBodyIdentifierPrefixRetainsStatementStart() {
        val result = ExpressionCompletionLexer.lex("{ ret", 5)
        assertEquals(ExpressionCompletionPosition.STATEMENT_START, result.position)
        assertEquals(2, result.tokenStart)
        assertEquals(5, result.tokenEnd)
    }

    @Test
    fun emptyExpressionStartIsValueStart() {
        val result = ExpressionCompletionLexer.lex("", 0)
        assertEquals(ExpressionCompletionPosition.VALUE_START, result.position)
    }

    @Test
    fun afterPlusIsValueStart() {
        val result = ExpressionCompletionLexer.lex("a + ", 4)
        assertEquals(ExpressionCompletionPosition.VALUE_START, result.position)
    }

    @Test
    fun afterCompleteIdentifierIsNone() {
        val result = ExpressionCompletionLexer.lex("return ", 7)
        assertEquals(ExpressionCompletionPosition.NONE, result.position)
    }

    @Test
    fun cursorInMiddleOfIdentifierUsesFullTokenBoundsAndValueStart() {
        val result = ExpressionCompletionLexer.lex("return x", 3)
        assertEquals(ExpressionCompletionPosition.VALUE_START, result.position)
        assertEquals(0, result.tokenStart)
        assertEquals(6, result.tokenEnd)
    }

    @Test
    fun insideSingleQuotedLiteralIsNone() {
        val result = ExpressionCompletionLexer.lex("'hel", 4)
        assertEquals(ExpressionCompletionPosition.NONE, result.position)
    }

    @Test
    fun incompleteBinaryPrefixIsValueStart() {
        val result = ExpressionCompletionLexer.lex("a +", 3)
        assertEquals(ExpressionCompletionPosition.VALUE_START, result.position)
    }
}
