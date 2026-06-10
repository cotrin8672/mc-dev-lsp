package io.github.mcdev.core.aw

import io.github.mcdev.core.model.MappingNamespace
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs

class AccessWidenerParserTest {
    @Test
    fun parsesHeaderAndEntries() {
        val result = assertIs<AccessWidenerParseResult.Success>(
            AccessWidenerParser.parse(
                """
                accessWidener v2 named
                accessible class net/minecraft/client/MinecraftClient
                accessible method net/minecraft/client/MinecraftClient setScreen (Lnet/minecraft/client/gui/screen/Screen;)V
                mutable field net/minecraft/client/MinecraftClient currentScreen Lnet/minecraft/client/gui/screen/Screen;
                """.trimIndent(),
            ),
        )

        assertEquals(MappingNamespace.NAMED, result.file.namespace)
        assertEquals(3, result.file.entries.size)
        assertEquals("setScreen", result.file.entries[1].name)
    }

    @Test
    fun rejectsInvalidDescriptor() {
        val result = AccessWidenerParser.parse(
            """
            accessWidener v2 named
            accessible method net/minecraft/client/MinecraftClient setScreen (Lbad
            """.trimIndent(),
        )
        assertIs<AccessWidenerParseResult.Failure>(result)
    }
}
