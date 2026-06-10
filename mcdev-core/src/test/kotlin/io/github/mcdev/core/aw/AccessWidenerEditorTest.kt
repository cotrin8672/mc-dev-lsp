package io.github.mcdev.core.aw

import io.github.mcdev.core.aw.AccessWidenerDirective.ACCESSIBLE
import io.github.mcdev.core.aw.AccessWidenerKind.CLASS
import io.github.mcdev.core.aw.AccessWidenerKind.FIELD
import io.github.mcdev.core.aw.AccessWidenerKind.METHOD
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class AccessWidenerEditorTest {
    @Test
    fun formatsClassEntryDeterministically() {
        assertEquals(
            "accessible class net/minecraft/client/MinecraftClient",
            AccessWidenerEditor.formatClassEntry(ACCESSIBLE, "net/minecraft/client/MinecraftClient"),
        )
    }

    @Test
    fun formatsMethodEntryDeterministically() {
        assertEquals(
            "accessible method net/minecraft/client/MinecraftClient setScreen (Lnet/minecraft/client/gui/screen/Screen;)V",
            AccessWidenerEditor.formatMethodEntry(
                ACCESSIBLE,
                "net/minecraft/client/MinecraftClient",
                "setScreen",
                "(Lnet/minecraft/client/gui/screen/Screen;)V",
            ),
        )
    }

    @Test
    fun formatsFieldEntryDeterministically() {
        assertEquals(
            "mutable field net/minecraft/client/MinecraftClient currentScreen Lnet/minecraft/client/gui/screen/Screen;",
            AccessWidenerEditor.formatFieldEntry(
                AccessWidenerDirective.MUTABLE,
                "net/minecraft/client/MinecraftClient",
                "currentScreen",
                "Lnet/minecraft/client/gui/screen/Screen;",
            ),
        )
    }

    @Test
    fun insertEntryAppendsFormattedLine() {
        val content = "accessWidener v2 named\n"
        val entry = AccessWidenerEntry(ACCESSIBLE, CLASS, "net/minecraft/client/MinecraftClient", line = 2)
        val updated = AccessWidenerEditor.insertEntry(content, entry)
        assertTrue(updated.contains("accessible class net/minecraft/client/MinecraftClient"))
    }

    @Test
    fun removeLineDeletesTargetEntry() {
        val updated = AccessWidenerEditor.removeLine(AwTestFixtures.VALID_AW, 3)
        assertTrue(updated.lines().none { it.contains("setScreen") })
    }

    @Test
    fun entryKeyIgnoresClassMemberParts() {
        val entry = AccessWidenerEntry(ACCESSIBLE, CLASS, "net/minecraft/client/MinecraftClient", line = 2)
        assertEquals("ACCESSIBLE|CLASS|net/minecraft/client/MinecraftClient", AccessWidenerEditor.entryKey(entry))
    }

    @Test
    fun entryKeyIncludesMemberParts() {
        val entry = AccessWidenerEntry(
            ACCESSIBLE,
            METHOD,
            "net/minecraft/client/MinecraftClient",
            "setScreen",
            "(Lnet/minecraft/client/gui/screen/Screen;)V",
            3,
        )
        assertTrue(AccessWidenerEditor.entryKey(entry).contains("setScreen"))
    }
}
