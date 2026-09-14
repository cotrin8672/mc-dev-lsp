package io.github.mcdev.core.mixinextras

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

class OfficialExpressionParserTest {
    @Test
    fun parseSuccessCases() {
        for ((category, source) in successCases) {
            val result = OfficialExpressionParser.parse(source)
            assertIs<OfficialExpressionParseResult.Success>(
                result,
                "expected success for $category: $source",
            )
        }
    }

    @Test
    fun parseFailureCases() {
        for ((label, source) in failureCases) {
            val result = OfficialExpressionParser.parse(source)
            val failure = assertIs<OfficialExpressionParseResult.SyntaxFailure>(
                result,
                "expected failure for $label: $source",
            )
            assertTrue(failure.message.isNotBlank(), "message for $label")
            assertNotNull(failure.cause, "cause for $label")
        }
    }

    @Test
    fun preservesOfficialErrorMessageAndCause() {
        for ((_, source) in failureCases) {
            val failure = assertIs<OfficialExpressionParseResult.SyntaxFailure>(
                OfficialExpressionParser.parse(source),
            )
            val cause = assertIs<RuntimeException>(failure.cause)
            assertEquals(cause.message, failure.message)
        }
    }

    private companion object {
        val successCases = listOf(
            "member assignment" to "this.foo=x",
            "array store" to "arr[i]=value",
            "identifier assignment" to "name=value",
            "return" to "return x",
            "throw" to "throw x",
            "capture" to "@(this.foo)",
            "wildcard" to "?",
            "this" to "this",
            "negative int literal" to "-23",
            "negative float literal" to "-1.5",
            "boolean literal" to "true",
            "null literal" to "null",
            "single-quoted text" to "'hello'",
            "identifier" to "foo",
            "class literal" to "Type.class",
            "array access" to "arr[i]",
            "field member" to "this.foo",
            "super method call" to "super.foo(x)",
            "member method call" to "this.foo(x)",
            "method call" to "foo(x)",
            "bound method reference" to "this::foo",
            "unbound method reference" to "::foo",
            "constructor reference" to "Type::new",
            "bitwise not" to "~x",
            "new object" to "new Type(x)",
            "new array initializer" to "new Type[]{x,y}",
            "new multidimensional array" to "new Type[x][]",
            "cast" to "(Type)x",
            "multiply" to "x*y",
            "add" to "x+y",
            "left shift" to "x<<y",
            "less than" to "x<y",
            "instanceof" to "x instanceof Type",
            "equality" to "x==y",
            "bitwise and" to "x&y",
            "bitwise xor" to "x^y",
            "bitwise or" to "x|y",
        )

        val failureCases = listOf(
            "empty input" to "",
            "malformed paren" to "foo(",
            "malformed operator" to "1 + )",
            "incomplete binary" to "a +",
            "incomplete increment" to "++",
            "malformed capture" to "@)",
        )
    }
}
