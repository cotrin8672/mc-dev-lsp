package io.github.mcdev.core.mixin

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class MixinTargetCompletionServiceTest {
    private val classIndex = FakeClassIndex()
    private val service = MixinTargetCompletionService(classIndex)

    @Test
    fun completesMixinClassWithImportMode() {
        val context = context(MixinAnnotation.MIXIN, AnnotationSlot.CLASS, "Minecraft")
        val items = service.complete(context, MixinCompletionOptions(MixinClassInsertMode.IMPORT))
        assertTrue(items.isNotEmpty())
        assertEquals("MinecraftClient.class", items.first().insertText)
        assertEquals("MinecraftClient", items.first().label)
    }

    @Test
    fun completesMixinClassWithFqnMode() {
        val context = context(MixinAnnotation.MIXIN, AnnotationSlot.CLASS, "Minecraft")
        val items = service.complete(context, MixinCompletionOptions(MixinClassInsertMode.FQN))
        assertEquals("net.minecraft.client.MinecraftClient.class", items.first().insertText)
    }

    @Test
    fun classCompletionPreservesLabelAndDetail() {
        val context = context(MixinAnnotation.MIXIN, AnnotationSlot.CLASS, "Game")
        val item = service.complete(context).first { it.label == "GameRenderer" }
        assertEquals("net.minecraft.client.render", item.detail)
    }

    @Test
    fun classCompletionMetadataContainsOwner() {
        val context = context(MixinAnnotation.MIXIN, AnnotationSlot.CLASS, "Draw")
        val item = service.complete(context).first()
        assertEquals("mixin.target", item.metadata.source)
        assertEquals("net/minecraft/client/gui/DrawContext", item.metadata.owner)
    }

    @Test
    fun completesTargetsStringWithFqn() {
        val context = context(MixinAnnotation.MIXIN, AnnotationSlot.TARGETS, "net.minecraft.client.Mine")
        val items = service.complete(context)
        assertEquals("net.minecraft.client.MinecraftClient", items.first().insertText)
    }

    @Test
    fun targetsCompletionLabelIsSimpleName() {
        val context = context(MixinAnnotation.MIXIN, AnnotationSlot.TARGETS, "Game")
        val item = service.complete(context).first { it.label == "GameRenderer" }
        assertEquals("net.minecraft.client.render.GameRenderer", item.detail)
    }

    @Test
    fun returnsEmptyForNonMixinAnnotation() {
        val context = context(MixinAnnotation.INJECT, AnnotationSlot.CLASS, "Mine")
        assertTrue(service.complete(context).isEmpty())
    }

    @Test
    fun returnsEmptyForWrongSlot() {
        val context = context(MixinAnnotation.MIXIN, AnnotationSlot.METHOD, "tick")
        assertTrue(service.complete(context).isEmpty())
    }

    @Test
    fun filtersClassesByPrefix() {
        val context = context(MixinAnnotation.MIXIN, AnnotationSlot.CLASS, "Text")
        val items = service.complete(context)
        assertEquals(1, items.size)
        assertEquals("TextRenderer", items.first().label)
    }

    @Test
    fun sortKeyIsStable() {
        val context = context(MixinAnnotation.MIXIN, AnnotationSlot.CLASS, "M")
        val items = service.complete(context)
        assertTrue(items.all { it.sortKey.startsWith("0100_") })
    }

    private fun context(annotation: MixinAnnotation, slot: AnnotationSlot, partial: String) = AnnotationContext(
        annotation = annotation,
        slot = slot,
        partialValue = partial,
        valueStartOffset = 0,
        valueEndOffset = partial.length,
        annotationStartOffset = 0,
        annotationEndOffset = 0,
    )
}
