package io.github.mcdev.core.mixin

import io.github.mcdev.core.completion.McCompletionInsertTextFormat
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class MixinAttributeCompletionServiceTest {
    private val service = MixinAttributeCompletionService()

    @Test
    fun methodAttributeInsertsQuotedSnippetWithCursorInside() {
        val items = service.complete(context(MixinAnnotation.INJECT, "meth"))
        val method = items.single { it.metadata.name == "method" }
        assertEquals("method = \"${'$'}{1}\"${'$'}0", method.insertText)
        assertEquals(McCompletionInsertTextFormat.SNIPPET, method.insertTextFormat)
        assertEquals("method = \"…\"", method.label)
    }

    @Test
    fun atAttributeBuildsNestedAtSnippet() {
        val at = service.complete(context(MixinAnnotation.INJECT, "at"))
            .single { it.metadata.name == "at" }
        assertEquals("at = @At(\"${'$'}{1}\")${'$'}0", at.insertText)
    }

    @Test
    fun excludesOnlyAttributesAlreadyPresentOnTheSameAnnotation() {
        val items = service.complete(
            context(MixinAnnotation.INJECT, "re").copy(existingAttributes = setOf("method", "at")),
        )
        assertTrue(items.any { it.metadata.name == "remap" })
        assertFalse(items.any { it.metadata.name == "method" })
        assertFalse(items.any { it.metadata.name == "at" })
    }

    private fun context(annotation: MixinAnnotation, partial: String) = AnnotationContext(
        annotation = annotation,
        slot = AnnotationSlot.ATTRIBUTE,
        partialValue = partial,
        valueStartOffset = 0,
        valueEndOffset = partial.length,
        annotationStartOffset = 0,
        annotationEndOffset = partial.length,
    )
}
