package io.github.mcdev.jdtls.mixin

import io.github.mcdev.core.mixin.MixinClassModel
import io.github.mcdev.core.mixin.MixinSemanticModelParser
import io.github.mcdev.core.mixin.ParseConfidence
import io.github.mcdev.core.mixin.ParseSource
import io.github.mcdev.core.mixin.SemanticParseDebugInfo
import java.util.concurrent.CancellationException
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertSame
import kotlin.test.assertTrue

class SemanticModelCacheTest {
    @Test
    fun transientJdtFallbackIsRecomputedSoRecoveryCanUseAstModel() {
        val transientFallback = model(
            parseSource = ParseSource.HAND_WRITTEN_FALLBACK,
            fallbackReason = "JDT AST parse failed: java.lang.IllegalStateException: unavailable",
        )
        val recovered = model(ParseSource.JDT_AST)
        var calls = 0
        val cache = SemanticModelCache { _, _ ->
            calls++
            if (calls == 1) transientFallback else recovered
        }

        val first = cache.get(SOURCE, URI, 1)
        val second = cache.get(SOURCE, URI, 1)
        val third = cache.get(SOURCE, URI, 1)

        assertEquals(ParseSource.HAND_WRITTEN_FALLBACK, first.model.parseSource)
        assertFalse(first.cacheHit)
        assertEquals(ParseSource.JDT_AST, second.model.parseSource)
        assertFalse(second.cacheHit)
        assertTrue(third.cacheHit)
        assertEquals(2, calls)
    }

    @Test
    fun canceledComputationLeavesNoEntryForTheNextRequest() {
        val cancellation = CancellationException("cancelled")
        val recovered = model(ParseSource.JDT_AST)
        var calls = 0
        val cache = SemanticModelCache { _, _ ->
            calls++
            if (calls == 1) throw cancellation else recovered
        }

        val thrown = assertFailsWith<CancellationException> {
            cache.get(SOURCE, URI, 2)
        }
        val next = cache.get(SOURCE, URI, 2)
        val cached = cache.get(SOURCE, URI, 2)

        assertSame(cancellation, thrown)
        assertEquals(ParseSource.JDT_AST, next.model.parseSource)
        assertFalse(next.cacheHit)
        assertTrue(cached.cacheHit)
        assertEquals(2, calls)
    }

    private fun model(parseSource: ParseSource, fallbackReason: String? = null): MixinClassModel =
        MixinSemanticModelParser.parse(
            source = SOURCE,
            sourceUri = URI,
            parseSource = parseSource,
            confidence = if (parseSource == ParseSource.JDT_AST) ParseConfidence.HIGH else ParseConfidence.LOW,
            warnings = fallbackReason?.let(::listOf) ?: emptyList(),
            debugInfo = SemanticParseDebugInfo(
                parseSource = parseSource,
                fallbackReason = fallbackReason,
            ),
        )

    private companion object {
        const val URI = "file:///ExampleMixin.java"
        const val SOURCE = "class ExampleMixin {}"
    }
}
