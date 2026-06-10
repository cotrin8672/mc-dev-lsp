package io.github.mcdev.core.at

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull

class AtContextExtractorTest {
    @Test
    fun detectsModifierSlot() {
        val context = extract("pub|lic net.minecraft.client.MinecraftClient")
        assertEquals(AtSlot.MODIFIER, context?.slot)
        assertEquals("pub", context?.partialValue)
    }

    @Test
    fun detectsOwnerSlot() {
        val context = extract("public net.minecraft.client.Mine|")
        assertEquals(AtSlot.OWNER, context?.slot)
        assertEquals("net.minecraft.client.Mine", context?.partialValue)
        assertEquals(AccessTransformerModifier.PUBLIC, context?.modifier)
    }

    @Test
    fun detectsMemberNameSlot() {
        val context = extract("public net.minecraft.client.MinecraftClient set|")
        assertEquals(AtSlot.MEMBER_NAME, context?.slot)
        assertEquals("set", context?.partialValue)
        assertEquals("net.minecraft.client.MinecraftClient", context?.owner)
    }

    @Test
    fun detectsMemberDescriptorSlot() {
        val context = extract("public net.minecraft.client.MinecraftClient setScreen(|")
        assertEquals(AtSlot.MEMBER_DESCRIPTOR, context?.slot)
    }

    @Test
    fun detectsMemberSlotAfterOwnerOnTrailingCursor() {
        val context = extract("public net.minecraft.client.MinecraftClient |")
        assertEquals(AtSlot.MEMBER_NAME, context?.slot)
        assertEquals("", context?.partialValue)
    }

    @Test
    fun detectsOwnerSlotAfterModifierOnTrailingCursor() {
        val context = extract("public |")
        assertEquals(AtSlot.OWNER, context?.slot)
    }

    @Test
    fun ignoresCommentSuffix() {
        val context = extract("public net.minecraft.client.MinecraftClient # comment|")
        assertEquals(AtSlot.MEMBER_NAME, context?.slot)
    }

    @Test
    fun parsesLineTokens() {
        val line = AtContextExtractor.parseLine(
            "public-f net.minecraft.client.MinecraftClient f_91074_",
            lineNumber = 1,
        )
        assertNotNull(line)
        assertEquals("public-f", line.modifier?.text)
        assertEquals("net.minecraft.client.MinecraftClient", line.owner?.text)
        assertEquals("f_91074_", line.member?.text)
    }

    @Test
    fun returnsNullOutsideSource() {
        assertNull(AtContextExtractor.extract("line", line = 5, character = 0))
    }

    private fun extract(sourceWithCursor: String): AtContext? {
        val cursor = sourceWithCursor.indexOf('|')
        val source = sourceWithCursor.replace("|", "")
        var line = 0
        var character = cursor
        if (sourceWithCursor.substring(0, cursor).contains('\n')) {
            line = sourceWithCursor.substring(0, cursor).count { it == '\n' }
            character = cursor - sourceWithCursor.lastIndexOf('\n', cursor - 1) - 1
        }
        return AtContextExtractor.extract(source, line, character)
    }
}
