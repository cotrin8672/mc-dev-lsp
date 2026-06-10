package io.github.mcdev.core.mixin

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class InjectMethodCompletionServiceTest {
    private val classIndex = FakeClassIndex()
    private val service = InjectMethodCompletionService(classIndex)

    @Test
    fun completesInjectMethodWithoutDescriptorForSingleOverload() {
        val context = methodContext("ti", listOf("net/minecraft/client/MinecraftClient"))
        val items = service.complete(context)
        assertTrue(items.any { it.insertText == "tick" })
    }

    @Test
    fun completesInjectMethodWithDescriptorForOverloadsInAutoMode() {
        val context = methodContext("render", listOf("net/minecraft/client/MinecraftClient"))
        val items = service.complete(context)
        assertTrue(items.any { it.insertText == "render(FJZ)V" })
        assertTrue(items.any { it.insertText == "render(I)V" })
    }

    @Test
    fun alwaysModeInsertsDescriptorEvenForSingleOverload() {
        val context = methodContext("tick", listOf("net/minecraft/client/MinecraftClient"))
        val items = service.complete(context, MixinCompletionOptions(injectMethodDescriptorMode = InjectMethodDescriptorMode.ALWAYS))
        assertEquals("tick()V", items.first().insertText)
    }

    @Test
    fun neverModeInsertsNameOnlyEvenForOverloads() {
        val context = methodContext("render", listOf("net/minecraft/client/MinecraftClient"))
        val items = service.complete(context, MixinCompletionOptions(injectMethodDescriptorMode = InjectMethodDescriptorMode.NEVER))
        assertEquals(1, items.size)
        assertEquals("render", items.first().insertText)
    }

    @Test
    fun completionLabelShowsReadableSignature() {
        val context = methodContext("set", listOf("net/minecraft/client/MinecraftClient"))
        val item = service.complete(context).first { it.label.contains("setScreen") }
        assertEquals("setScreen(Screen): void", item.label)
    }

    @Test
    fun returnsEmptyWithoutMixinTargets() {
        val context = methodContext("tick", emptyList())
        assertTrue(service.complete(context).isEmpty())
    }

    @Test
    fun filtersByMethodPrefix() {
        val context = methodContext("set", listOf("net/minecraft/client/MinecraftClient"))
        val items = service.complete(context)
        assertTrue(items.all { it.metadata.name!!.startsWith("set") })
    }

    @Test
    fun metadataContainsDescriptor() {
        val context = methodContext("tick", listOf("net/minecraft/client/MinecraftClient"))
        val item = service.complete(context).first()
        assertEquals("()V", item.metadata.descriptor)
        assertEquals("mixin.injectMethod", item.metadata.source)
    }

    @Test
    fun returnsEmptyForNonMethodSlot() {
        val context = AnnotationContext(
            annotation = MixinAnnotation.INJECT,
            slot = AnnotationSlot.VALUE,
            partialValue = "HEAD",
            valueStartOffset = 0,
            valueEndOffset = 4,
            annotationStartOffset = 0,
            annotationEndOffset = 10,
            mixinTargetInternalNames = listOf("net/minecraft/client/MinecraftClient"),
        )
        assertTrue(service.complete(context).isEmpty())
    }

    @Test
    fun completesFromMultipleMixinTargets() {
        val context = methodContext("tick", listOf("net/minecraft/client/MinecraftClient", "net/minecraft/client/render/GameRenderer"))
        val items = service.complete(context)
        assertTrue(items.isNotEmpty())
    }

    private fun methodContext(partial: String, targets: List<String>) = AnnotationContext(
        annotation = MixinAnnotation.INJECT,
        slot = AnnotationSlot.METHOD,
        partialValue = partial,
        valueStartOffset = 0,
        valueEndOffset = partial.length,
        annotationStartOffset = 0,
        annotationEndOffset = 0,
        mixinTargetInternalNames = targets,
    )
}
