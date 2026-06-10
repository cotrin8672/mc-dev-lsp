package io.github.mcdev.core.at

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class AccessTransformerEditorTest {
    private val editor = AccessTransformerEditor()

    @Test
    fun formatsClassEntry() {
        val line = editor.formatEntry(
            AccessTransformerEntry(
                modifier = AccessTransformerModifier.PUBLIC,
                owner = "net.minecraft.client.MinecraftClient",
                line = 1,
            ),
        )
        assertEquals("public net.minecraft.client.MinecraftClient", line)
    }

    @Test
    fun formatsFieldEntry() {
        val line = editor.formatEntry(
            AccessTransformerEntry(
                modifier = AccessTransformerModifier.PUBLIC_REMOVE_FINAL,
                owner = "net.minecraft.client.MinecraftClient",
                name = "f_91074_",
                line = 1,
            ),
        )
        assertEquals("public-f net.minecraft.client.MinecraftClient f_91074_", line)
    }

    @Test
    fun formatsMethodEntryWithDescriptor() {
        val line = editor.formatEntry(
            AccessTransformerEntry(
                modifier = AccessTransformerModifier.PROTECTED,
                owner = "net.minecraft.client.MinecraftClient",
                name = "m_91152_",
                descriptor = "(Lnet/minecraft/client/gui/screen/Screen;)V",
                line = 1,
            ),
        )
        assertEquals(
            "protected net.minecraft.client.MinecraftClient m_91152_(Lnet/minecraft/client/gui/screen/Screen;)V",
            line,
        )
    }

    @Test
    fun appendEntryIsStable() {
        val result = editor.appendEntry(
            "public net.minecraft.client.MinecraftClient\n",
            AccessTransformerEntry(
                modifier = AccessTransformerModifier.PUBLIC,
                owner = "net.minecraft.client.MinecraftClient",
                name = "currentScreen",
                line = 2,
            ),
        )
        assertTrue(result.changed)
        assertTrue(result.content.contains("public net.minecraft.client.MinecraftClient currentScreen"))
        assertEquals(2, result.content.lines().count { it.isNotBlank() })
    }

    @Test
    fun replaceLineNoOpWhenUnchanged() {
        val content = "public net.minecraft.client.MinecraftClient\n"
        val result = editor.replaceLine(content, 1, "public net.minecraft.client.MinecraftClient")
        assertFalse(result.changed)
    }

    @Test
    fun entryKeyIncludesDescriptor() {
        val key = editor.entryKey(
            AccessTransformerEntry(
                modifier = AccessTransformerModifier.PUBLIC,
                owner = "net.minecraft.client.MinecraftClient",
                name = "setScreen",
                descriptor = "(Lnet/minecraft/client/gui/screen/Screen;)V",
                line = 1,
            ),
        )
        assertTrue(key.contains("setScreen"))
        assertTrue(key.contains("(Lnet/minecraft/client/gui/screen/Screen;)V"))
    }
}
