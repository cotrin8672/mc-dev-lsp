package io.github.mcdev.jdtls

import io.github.mcdev.jdtls.command.McdevCommandDispatcher
import io.github.mcdev.protocol.McdevCommands
import org.eclipse.core.runtime.NullProgressMonitor
import kotlin.test.AfterTest
import kotlin.test.BeforeTest
import kotlin.test.Test
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertTrue

class McdevDelegateCommandHandlerTest {
    @BeforeTest
    fun setUp() {
        McdevServices.dispatcher = McdevCommandDispatcher()
    }

    @AfterTest
    fun tearDown() {
        McdevServices.dispatcher = null
    }

    @Test
    fun executeCommandReturnsEncodedEnvelope() {
        val handler = McdevDelegateCommandHandler()
        @Suppress("UNCHECKED_CAST")
        val result = handler.executeCommand(
            McdevCommands.INFO,
            listOf(
                mapOf(
                    "context" to mapOf(
                        "protocolVersion" to 1,
                        "workspaceRoot" to "file:///tmp/project",
                        "documentUri" to "file:///tmp/project/Mixin.java",
                        "languageId" to "java",
                        "position" to mapOf("line" to 0, "character" to 0),
                        "bufferText" to "@Mixin(Foo.class)\nclass Mixin {}",
                        "client" to mapOf("name" to "test", "version" to "0.1.0"),
                    ),
                ),
            ),
            NullProgressMonitor(),
        ) as Map<String, Any?>

        assertTrue(result.containsKey("protocolVersion"))
        val info = assertIs<Map<*, *>>(result["result"])
        val lines = assertIs<List<*>>(info["lines"])
        assertTrue(lines.isNotEmpty())
    }

    @Test
    fun unknownCommandThrowsUnsupportedOperationException() {
        val handler = McdevDelegateCommandHandler()
        assertFailsWith<UnsupportedOperationException> {
            handler.executeCommand("mcdev.unknown", emptyList<Any>(), NullProgressMonitor())
        }
    }
}
