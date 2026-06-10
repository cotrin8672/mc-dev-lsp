package io.github.mcdev.core.at

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs

class AccessTransformerParserTest {
    @Test
    fun parsesClassFieldAndMethodEntries() {
        val result = assertIs<AccessTransformerParseResult.Success>(
            AccessTransformerParser.parse(
                """
                public net.minecraft.client.Minecraft
                public-f net.minecraft.client.Minecraft f_91074_
                protected net.minecraft.client.Minecraft m_91152_(Lnet/minecraft/client/gui/screens/Screen;)V
                """.trimIndent(),
            ),
        )

        assertEquals(3, result.file.entries.size)
        assertEquals("m_91152_", result.file.entries[2].name)
    }

    @Test
    fun rejectsInvalidModifier() {
        val result = AccessTransformerParser.parse("wider net.minecraft.client.Minecraft")
        assertIs<AccessTransformerParseResult.Failure>(result)
    }
}
