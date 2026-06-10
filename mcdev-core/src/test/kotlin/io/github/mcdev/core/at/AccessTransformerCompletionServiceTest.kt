package io.github.mcdev.core.at

import io.github.mcdev.core.completion.McCompletionKind
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class AccessTransformerCompletionServiceTest {
    @Test
    fun completesAllModifiers() {
        val service = AccessTransformerCompletionService(AtTestFixtures.classIndex)
        val context = contextAt("public net.minecraft.client.MinecraftClient", 0, 0)!!.copy(
            slot = AtSlot.MODIFIER,
            partialValue = "",
        )
        val items = service.complete(context)
        assertEquals(AccessTransformerModifier.entries.size, items.size)
        assertTrue(items.all { it.kind == McCompletionKind.KEYWORD })
    }

    @Test
    fun filtersModifiersByPrefix() {
        val service = AccessTransformerCompletionService(AtTestFixtures.classIndex)
        val context = contextAt("pub", 0, 3)!!.copy(slot = AtSlot.MODIFIER, partialValue = "pub")
        val items = service.complete(context)
        assertTrue(items.all { it.label.startsWith("pub") })
        assertTrue(items.any { it.label == "public" })
        assertTrue(items.any { it.label == "public-f" })
    }

    @Test
    fun completesClassesWithNamedFqnInsert() {
        val service = AccessTransformerCompletionService(AtTestFixtures.classIndex)
        val context = contextAt("public net.minecraft.client.Mine", 0, 33)!!
        val items = service.complete(context)
        assertTrue(items.any { it.label == "MinecraftClient" })
        assertTrue(items.any { it.insertText == "net.minecraft.client.MinecraftClient" })
    }

    @Test
    fun completesMethodsWithNamedInsertOnFabric() {
        val service = AccessTransformerCompletionService(AtTestFixtures.classIndex, AtTestFixtures.fabricMappingContext)
        val context = contextAt("public net.minecraft.client.MinecraftClient set", 0, 48)!!
        val items = service.complete(context)
        val setScreen = items.first { it.label.contains("setScreen") }
        assertEquals("setScreen(Lnet/minecraft/client/gui/screen/Screen;)V", setScreen.insertText)
        assertEquals("named: setScreen", setScreen.detail)
    }

    @Test
    fun completesFieldsWithNamedInsertOnFabric() {
        val service = AccessTransformerCompletionService(AtTestFixtures.classIndex, AtTestFixtures.fabricMappingContext)
        val context = contextAt("public net.minecraft.client.MinecraftClient play", 0, 49)!!
        val items = service.complete(context)
        val player = items.first { it.label.startsWith("player:") }
        assertEquals("player", player.insertText)
    }

    @Test
    fun completesMethodsWithSrgInsertOnForge() {
        val service = AccessTransformerCompletionService(AtTestFixtures.classIndex, AtTestFixtures.forgeMappingContext)
        val context = contextAt("public net.minecraft.client.MinecraftClient set", 0, 48)!!
        val items = service.complete(context)
        val setScreen = items.first { it.detail == "named: setScreen" }
        assertEquals(
            "m_91152_(Lnet/minecraft/client/gui/screen/Screen;)V",
            setScreen.insertText,
        )
    }

    @Test
    fun completesFieldsWithSrgInsertOnForge() {
        val service = AccessTransformerCompletionService(AtTestFixtures.classIndex, AtTestFixtures.forgeMappingContext)
        val context = contextAt("public net.minecraft.client.MinecraftClient curr", 0, 49)!!
        val items = service.complete(context)
        val currentScreen = items.first { it.label.startsWith("currentScreen:") }
        assertEquals("f_91074_", currentScreen.insertText)
    }

    @Test
    fun separatesDisplayLabelFromInsertText() {
        val service = AccessTransformerCompletionService(AtTestFixtures.classIndex, AtTestFixtures.forgeMappingContext)
        val context = contextAt("public net.minecraft.client.MinecraftClient ", 0, 44)!!
        val items = service.complete(context)
        val method = items.first { it.detail == "named: setScreen" }
        assertTrue(method.label.contains("setScreen"))
        assertTrue(method.insertText.startsWith("m_"))
    }

    @Test
    fun returnsEmptyForDescriptorSlot() {
        val service = AccessTransformerCompletionService(AtTestFixtures.classIndex, AtTestFixtures.forgeMappingContext)
        val context = contextAt("public net.minecraft.client.MinecraftClient setScreen(", 0, 55)!!
        val items = service.complete(context)
        assertTrue(items.isEmpty())
    }

    @Test
    fun returnsEmptyWhenOwnerMissingForMemberCompletion() {
        val service = AccessTransformerCompletionService(AtTestFixtures.classIndex, AtTestFixtures.forgeMappingContext)
        val context = contextAt("public ", 0, 7)!!.copy(
            slot = AtSlot.MEMBER_NAME,
            owner = null,
            partialValue = "",
        )
        val items = service.complete(context)
        assertTrue(items.isEmpty())
    }

    private fun contextAt(source: String, line: Int, character: Int): AtContext? =
        AtContextExtractor.extract(source, line, character)
}
