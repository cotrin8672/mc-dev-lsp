package io.github.mcdev.core.mixin

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class SemanticCompletionContextExtractorTest {
    private val facade = MixinServiceFacade(FakeClassIndex(), FakeBytecodeIndex())

    @Test
    fun completesInjectMethodFromSemanticContextWithoutFallback() {
        val fixture = markedSource(
            """
            package com.example.mixin;

            @Mixin(net.minecraft.client.MinecraftClient.class)
            class TargetMixin {
                @Inject(method = "/*caret*/", at = @At("HEAD"))
                private void injected(CallbackInfo ci) {}
            }
            """.trimIndent(),
        )

        val result = facade.completeWithDebug(
            request = request(fixture),
            options = MixinCompletionOptions(),
        )

        assertTrue(result.items.any { it.insertText == "tick" })
        assertTrue(result.debug.semanticContextFound)
        assertFalse(result.debug.fallbackAnnotationContextUsed)
        assertEquals(null, result.debug.fallbackAnnotationContextReason)
        assertEquals(null, result.debug.zeroItemReason)
    }

    @Test
    fun completesAtTargetFromRecoveredSemanticContextWithoutFallback() {
        val fixture = markedSource(
            """
            package com.example.mixin;

            @Mixin(net.minecraft.client.MinecraftClient.class)
            class TargetMixin {
                @Inject(
                    method = {"tick", "render"},
                    at = {@At("HEAD"), @At(value = "INVOKE", target = "/*caret*/")}
                )
                private void injected(CallbackInfo ci) {}
            }
            """.trimIndent(),
        )

        val result = facade.completeWithDebug(
            request = request(fixture),
            options = MixinCompletionOptions(),
        )

        assertTrue(result.items.any { it.insertText.contains("draw") })
        assertTrue(result.debug.semanticContextFound)
        assertFalse(result.debug.fallbackAnnotationContextUsed)
        assertEquals(null, result.debug.fallbackAnnotationContextReason)
        assertEquals(null, result.debug.zeroItemReason)
    }

    private fun request(fixture: MarkedSource): MixinFacadeRequest =
        MixinFacadeRequest(
            bufferText = fixture.source,
            line = fixture.line,
            character = fixture.character,
            documentUri = "file:///TargetMixin.java",
            semanticModel = MixinSemanticModelParser.parse(fixture.source, "file:///TargetMixin.java"),
        )

    private data class MarkedSource(
        val source: String,
        val line: Int,
        val character: Int,
    )

    private fun markedSource(sourceWithMarker: String): MarkedSource {
        val marker = "/*caret*/"
        val offset = sourceWithMarker.indexOf(marker)
        require(offset >= 0) { "missing caret marker" }
        val source = sourceWithMarker.replace(marker, "")
        val before = sourceWithMarker.substring(0, offset)
        val line = before.count { it == '\n' }
        val character = before.substringAfterLast('\n').length
        return MarkedSource(source, line, character)
    }
}
