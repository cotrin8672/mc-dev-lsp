package io.github.mcdev.core.mixin

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class AtCompletionServiceTest {
    private val valueService = AtValueCompletionService()
    private val targetService = AtTargetCompletionService()

    @Test
    fun completesAtValueHead() {
        val context = atValueContext("HE")
        val items = valueService.complete(context)
        assertTrue(items.any { it.insertText == "HEAD" })
    }

    @Test
    fun completesAllAtValues() {
        val context = atValueContext("")
        val items = valueService.complete(context)
        assertTrue(items.any { it.insertText == "INVOKE" })
        assertTrue(items.any { it.insertText == "FIELD" })
        assertTrue(items.any { it.insertText == "MIXINEXTRAS:EXPRESSION" })
    }

    @Test
    fun atValueCompletionFiltersByPrefix() {
        val context = atValueContext("RET")
        val items = valueService.complete(context)
        assertEquals(1, items.size)
        assertEquals("RETURN", items.first().insertText)
    }

    @Test
    fun completesAtInvokeTarget() {
        val context = atTargetContext("")
        val candidate = FakeBytecodeIndex.defaultCandidates().values.first().first()
        val items = targetService.complete(context, listOf(candidate))
        assertEquals(
            "Lnet/minecraft/client/font/TextRenderer;draw(Ljava/lang/String;FFI)I",
            items.first().insertText,
        )
    }

    @Test
    fun atTargetLabelDiffersFromInsertText() {
        val context = atTargetContext("")
        val candidate = FakeBytecodeIndex.defaultCandidates().values.first().first()
        val item = targetService.complete(context, listOf(candidate)).first()
        assertEquals("draw(String, float, float, int): int", item.label)
        assertTrue(item.insertText.startsWith("L"))
    }

    @Test
    fun completesAtFieldTarget() {
        val candidate = FakeBytecodeIndex.defaultCandidates()["net/minecraft/client/MinecraftClient#tick#FIELD"]!!.first()
        val items = targetService.complete(atTargetContext(""), listOf(candidate))
        assertEquals(
            "Lnet/minecraft/client/MinecraftClient;currentScreen:Lnet/minecraft/client/gui/screen/Screen;",
            items.first().insertText,
        )
    }

    @Test
    fun formatTargetForNewClassOnly() {
        val candidate = AtTargetCandidate(
            owner = "net/minecraft/client/gui/DrawContext",
            name = "",
            descriptor = "",
            displayLabel = "DrawContext",
            detail = "",
            kind = AtTargetKind.NEW,
        )
        assertEquals("Lnet/minecraft/client/gui/DrawContext;", targetService.formatTarget(candidate))
    }

    @Test
    fun formatTargetForConstructor() {
        val candidate = AtTargetCandidate(
            owner = "net/minecraft/client/gui/DrawContext",
            name = "<init>",
            descriptor = "()V",
            displayLabel = "DrawContext.<init>()",
            detail = "",
            kind = AtTargetKind.NEW,
        )
        assertEquals("Lnet/minecraft/client/gui/DrawContext;<init>()V", targetService.formatTarget(candidate))
    }

    @Test
    fun atTargetFiltersByPartial() {
        val candidate = FakeBytecodeIndex.defaultCandidates().values.first().first()
        val items = targetService.complete(atTargetContext("TextRenderer"), listOf(candidate))
        assertTrue(items.isNotEmpty())
        val empty = targetService.complete(atTargetContext("zzzzz"), listOf(candidate))
        assertTrue(empty.isEmpty())
    }

    @Test
    fun returnsEmptyForNonAtAnnotation() {
        val context = AnnotationContext(
            annotation = MixinAnnotation.INJECT,
            slot = AnnotationSlot.VALUE,
            partialValue = "HE",
            valueStartOffset = 0,
            valueEndOffset = 2,
            annotationStartOffset = 0,
            annotationEndOffset = 5,
        )
        assertTrue(valueService.complete(context).isEmpty())
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

    private fun atTargetContext(partial: String) = AnnotationContext(
        annotation = MixinAnnotation.AT,
        slot = AnnotationSlot.TARGET,
        partialValue = partial,
        valueStartOffset = 0,
        valueEndOffset = partial.length,
        annotationStartOffset = 0,
        annotationEndOffset = 0,
        atValue = "INVOKE",
    )
}
