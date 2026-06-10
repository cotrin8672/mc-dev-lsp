package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.mixin.AnnotationContext
import io.github.mcdev.core.mixin.AnnotationSlot
import io.github.mcdev.core.mixin.AtValueCompletionService
import io.github.mcdev.core.mixin.MixinAnnotation
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class ExpressionSupportTest {
    private val expressionSupport = ExpressionSupport()
    private val atValueService = AtValueCompletionService()

    @Test
    fun completesMixinExtrasExpressionAtValue() {
        val context = atValueContext("MIXINEXTRAS:EXPR")
        val coreItems = atValueService.complete(context)
        val extrasItems = expressionSupport.completeAtValue(context)
        assertTrue(coreItems.any { it.insertText == "MIXINEXTRAS:EXPRESSION" })
        assertEquals("MIXINEXTRAS:EXPRESSION", extrasItems.first().insertText)
        assertEquals("mixinextras.expressionAtValue", extrasItems.first().metadata.source)
    }

    @Test
    fun expressionAtValueCompletionFiltersByPrefix() {
        val context = atValueContext("MIXINEXTRAS")
        val items = expressionSupport.completeAtValue(context)
        assertEquals(1, items.size)
    }

    @Test
    fun completesExpressionAnnotationSnippets() {
        val context = AnnotationContext(
            annotation = MixinAnnotation.AT,
            slot = AnnotationSlot.VALUE,
            partialValue = "def",
            valueStartOffset = 0,
            valueEndOffset = 3,
            annotationStartOffset = 0,
            annotationEndOffset = 0,
        )
        val items = expressionSupport.completeExpressionAnnotations(context)
        assertTrue(items.any { it.insertText.startsWith("@Definition") })
    }

    @Test
    fun completesShareSnippet() {
        val items = expressionSupport.completeExpressionAnnotations(emptyPartialContext())
        assertTrue(items.any { it.metadata.name == "share" })
    }

    @Test
    fun completesLocalSnippet() {
        val items = expressionSupport.completeExpressionAnnotations(emptyPartialContext())
        assertTrue(items.any { it.metadata.name == "local" })
    }

    @Test
    fun isExpressionAtValueDetectsValue() {
        assertTrue(expressionSupport.isExpressionAtValue("MIXINEXTRAS:EXPRESSION"))
        assertTrue(!expressionSupport.isExpressionAtValue("INVOKE"))
    }

    @Test
    fun expressionAtValueReturnsEmptyForNonAtAnnotation() {
        val context = AnnotationContext(
            annotation = MixinAnnotation.INJECT,
            slot = AnnotationSlot.VALUE,
            partialValue = "MIXINEXTRAS",
            valueStartOffset = 0,
            valueEndOffset = 9,
            annotationStartOffset = 0,
            annotationEndOffset = 0,
        )
        assertTrue(expressionSupport.completeAtValue(context).isEmpty())
    }

    private fun atValueContext(partial: String) = AnnotationContext(
        annotation = MixinAnnotation.AT,
        slot = AnnotationSlot.VALUE,
        partialValue = partial,
        valueStartOffset = 0,
        valueEndOffset = partial.length,
        annotationStartOffset = 0,
        annotationEndOffset = 0,
    )

    private fun emptyPartialContext() = AnnotationContext(
        annotation = MixinAnnotation.AT,
        slot = AnnotationSlot.TARGET,
        partialValue = "",
        valueStartOffset = 0,
        valueEndOffset = 0,
        annotationStartOffset = 0,
        annotationEndOffset = 0,
    )
}
