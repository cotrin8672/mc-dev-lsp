package io.github.mcdev.core.at

import io.github.mcdev.core.mixin.MethodIndexEntry
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class AtMemberInsertFormatterTest {
    private val formatter = AtMemberInsertFormatter()

    @Test
    fun insertsNamedMethodOnFabric() {
        val method = MethodIndexEntry(
            name = "setScreen",
            descriptor = "(Lnet/minecraft/client/gui/screen/Screen;)V",
            isStatic = false,
            readableSignature = "setScreen(Screen): void",
        )
        val result = formatter.formatMethodInsert(
            method = method,
            ownerInternalName = "net/minecraft/client/MinecraftClient",
            mappingContext = AtTestFixtures.fabricMappingContext,
        )
        assertEquals("setScreen(Lnet/minecraft/client/gui/screen/Screen;)V", result.insertText)
        assertTrue(result.mappingFound)
    }

    @Test
    fun insertsSrgMethodOnForge() {
        val method = MethodIndexEntry(
            name = "setScreen",
            descriptor = "(Lnet/minecraft/client/gui/screen/Screen;)V",
            isStatic = false,
            readableSignature = "setScreen(Screen): void",
        )
        val result = formatter.formatMethodInsert(
            method = method,
            ownerInternalName = "net/minecraft/client/MinecraftClient",
            mappingContext = AtTestFixtures.forgeMappingContext,
        )
        assertEquals("m_91152_(Lnet/minecraft/client/gui/screen/Screen;)V", result.insertText)
        assertTrue(result.mappingFound)
    }

    @Test
    fun reportsMissingMappingForUnknownMethod() {
        val method = MethodIndexEntry(
            name = "render",
            descriptor = "(I)V",
            isStatic = false,
            readableSignature = "render(int): void",
        )
        val result = formatter.formatMethodInsert(
            method = method,
            ownerInternalName = "net/minecraft/client/MinecraftClient",
            mappingContext = AtTestFixtures.forgeMappingContext,
        )
        assertFalse(result.mappingFound)
        assertEquals("render(I)V", result.insertText)
    }

    @Test
    fun formatsOwnerAsFqn() {
        val entry = AtTestFixtures.classIndex.findClassByFqn("net.minecraft.client.MinecraftClient")
        assertEquals("net.minecraft.client.MinecraftClient", formatter.formatOwnerInsert(entry!!))
    }
}
