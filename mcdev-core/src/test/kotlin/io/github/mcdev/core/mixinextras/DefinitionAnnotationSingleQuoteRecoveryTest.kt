package io.github.mcdev.core.mixinextras

import kotlin.test.Test
import kotlin.test.assertEquals

class DefinitionAnnotationSingleQuoteRecoveryTest {
    @Test
    fun invalidSingleQuotedValueDoesNotHideFollowingValidAttribute() {
        val parsed = DefinitionAnnotationParser.parse(
            "id = \"good\", method = 'bad,still', field = \"Lcom/example/Foo;value:I\"",
        )

        assertEquals(listOf("Lcom/example/Foo;value:I"), parsed.rawFieldReferences)
        assertEquals(listOf("method"), parsed.parseIssues.map { it.attribute })
        assertEquals("'bad,still'", parsed.parseIssues.single().rawValue)
    }
}
