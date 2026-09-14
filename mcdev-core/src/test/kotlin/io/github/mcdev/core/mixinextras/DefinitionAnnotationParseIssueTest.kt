package io.github.mcdev.core.mixinextras

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class DefinitionAnnotationParseIssueTest {
    @Test
    fun retainsMalformedKnownAttributesAndContinuesParsingValidSiblings() {
        val body = """
            method = 123,
            field = "Lcom/example/Foo;value:I",
            type = NotAClassLiteral,
            local = 123,
            remap = maybe,
            id = "good"
        """.trimIndent()

        val parsed = DefinitionAnnotationParser.parse(body)

        assertEquals("good", parsed.id)
        assertEquals(listOf("Lcom/example/Foo;value:I"), parsed.rawFieldReferences)
        assertEquals(listOf("method", "type", "local", "remap"), parsed.parseIssues.map { it.attribute })
        assertEquals(listOf("123", "NotAClassLiteral", "123", "maybe"), parsed.parseIssues.map { it.rawValue })
        assertEquals(
            listOf(
                "expected a string or string array",
                "expected a class literal or class literal array",
                "expected @Local or @Local array",
                "expected a boolean literal",
            ),
            parsed.parseIssues.map { it.message },
        )
        assertTrue(parsed.parseIssues.all { it.bodyRange.first <= it.bodyRange.last })
    }

    @Test
    fun mapsParseIssueToSourceAndKeepsValidIdentifierBinding() {
        val source = "@Definition(id = \"good\", method = 123, field = \"Lcom/example/Foo;value:I\")"
        val definition = DefinitionsAnnotationParser.parse(source).definitions.single()

        val built = OfficialExpressionIdentifierPoolBuilder.build(
            MixinExtrasDefinitionIndex(listOf(definition)),
        )

        assertEquals(listOf("method"), built.issues.map { it.attribute })
        assertEquals("123", built.issues.single().rawValue)
        val invalidStart = source.indexOf("123")
        assertEquals(invalidStart until invalidStart + 3, built.issues.single().sourceRange)
        assertTrue(built.pool.delegate.memberExists("good"))
    }

    @Test
    fun malformedIdProducesOneParseIssueInsteadOfDuplicateMissingIdIssues() {
        val definition = DefinitionsAnnotationParser.parse(
            "@Definition(id = notAString)",
        ).definitions.single()

        val built = OfficialExpressionIdentifierPoolBuilder.build(
            MixinExtrasDefinitionIndex(listOf(definition)),
        )

        assertEquals(listOf("id"), built.issues.map { it.attribute })
        assertEquals("notAString", built.issues.single().rawValue)
    }

    @Test
    fun recoversFromMissingEqualsBeforeValidSiblingAttribute() {
        val parsed = DefinitionAnnotationParser.parse(
            "method 123, field = \"Lcom/example/Foo;value:I\"",
        )

        assertEquals(listOf("Lcom/example/Foo;value:I"), parsed.rawFieldReferences)
        assertEquals(listOf("method"), parsed.parseIssues.map { it.attribute })
    }
}
