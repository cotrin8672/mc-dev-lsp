package io.github.mcdev.protocol

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull

class McdevProtocolTest {
    @Test
    fun protocolMismatchUsesStructuredError() {
        val response = protocolMismatch(99)
        assertEquals(McdevErrorCode.PROTOCOL_MISMATCH, response.error?.code)
        assertEquals("99", response.error?.details?.get("client"))
        assertEquals(McdevProtocol.VERSION.toString(), response.error?.details?.get("server"))
    }

    @Test
    fun projectContextObservabilityCommandsUseStableIds() {
        assertEquals("mcdev.reloadProjectContext", McdevCommands.RELOAD_PROJECT_CONTEXT)
        assertEquals("mcdev.dumpContext", McdevCommands.DUMP_CONTEXT)
        assertEquals("mcdev.hover", McdevCommands.HOVER)
        assertEquals("mcdev.diagnostics", McdevCommands.DIAGNOSTICS)
    }

    @Test
    fun completionItemPreservesDisplayAndInsertionSeparately() {
        val item = McdevCompletionItemDto(
            label = "setScreen(Screen): void",
            detail = "MinecraftClient",
            documentation = null,
            filterText = "setScreen MinecraftClient Screen",
            insertText = "m_91152_(Lnet/minecraft/client/gui/screens/Screen;)V",
            kind = "method",
            sortKey = "0200_setScreen",
            edit = null,
        )
        assertEquals("setScreen(Screen): void", item.label)
        assertEquals("m_91152_(Lnet/minecraft/client/gui/screens/Screen;)V", item.insertText)
        assertNotNull(item.filterText)
    }
}
