package io.github.mcdev.jdtls.mixin

import io.github.mcdev.core.mixin.ParseSource
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class JdtMixinSemanticModelParserTest {
    @Test
    fun fallsBackWithVisibleWarningWhenJdtAstIsUnavailableInUnitTests() {
        val source = """
            package com.example.mixin;
            @Mixin(SimpleTarget.class)
            class ExampleMixin {
                @Shadow private int counter;
            }
        """.trimIndent()

        val model = JdtMixinSemanticModelParser().parse(source, "file:///ExampleMixin.java")

        assertEquals("file:///ExampleMixin.java", model.sourceUri)
        assertEquals(ParseSource.HAND_WRITTEN_FALLBACK, model.parseSource)
        assertTrue(model.warnings.any { it.contains("JDT ASTParser is not available") })
    }
}
