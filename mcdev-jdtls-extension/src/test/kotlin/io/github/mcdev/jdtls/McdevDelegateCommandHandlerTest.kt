package io.github.mcdev.jdtls

import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.core.mixinextras.OfficialExpressionMatcher
import io.github.mcdev.jdtls.support.JdtlsFixtureSupport
import io.github.mcdev.jdtls.command.McdevCommandDispatcher
import io.github.mcdev.protocol.McdevCommands
import io.github.mcdev.protocol.McdevProtocol
import org.eclipse.core.runtime.NullProgressMonitor
import kotlin.test.AfterTest
import kotlin.test.BeforeTest
import kotlin.test.Test
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertTrue
import java.nio.file.Path
import java.util.concurrent.CancellationException
import org.junit.jupiter.api.io.TempDir

class McdevDelegateCommandHandlerTest {
    @TempDir
    lateinit var tempDir: Path

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

    @Test
    fun expressionCancellationPropagatesAndNextRequestSucceeds() {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_BASIC, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        val source = """
            package com.example.mixin;
            import com.example.target.SimpleTarget;
            import com.llamalad7.mixinextras.expression.Expression;
            import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
            import org.spongepowered.asm.mixin.Mixin;
            import org.spongepowered.asm.mixin.injection.At;
            @Mixin(SimpleTarget.class)
            public abstract class ExpressionMixin {
                @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
                @Expression("text.length()")
                private int mcdev${'$'}handler(int original) { return original; }
            }
        """.trimIndent()
        val handler = McdevDelegateCommandHandler()
        val monitor = CancelAfterChecksMonitor(cancelAfter = 1)

        assertFailsWith<CancellationException> {
            handler.executeCommand(
                McdevCommands.DIAGNOSTICS,
                listOf(contextPayload(source)),
                monitor,
            )
        }
        assertTrue(monitor.checks > 1)
        assertTrue(
            monitor.cancellationStackTrace.orEmpty().any {
                it.className == OfficialExpressionMatcher::class.qualifiedName
            },
        )

        @Suppress("UNCHECKED_CAST")
        val result = handler.executeCommand(
            McdevCommands.DIAGNOSTICS,
            listOf(contextPayload(source)),
            NullProgressMonitor(),
        ) as Map<String, Any?>
        assertTrue(result.containsKey("result"))
    }

    private fun contextPayload(source: String): Map<String, Any?> = mapOf(
        "context" to mapOf(
            "protocolVersion" to McdevProtocol.VERSION,
            "workspaceRoot" to JdtlsFixtureSupport.workspaceUri(tempDir),
            "documentUri" to "${JdtlsFixtureSupport.workspaceUri(tempDir)}/src/main/java/com/example/mixin/ExpressionMixin.java",
            "languageId" to "java",
            "position" to mapOf("line" to 0, "character" to 0),
            "bufferText" to source,
            "client" to mapOf("name" to "test", "version" to "0.1.0"),
        ),
    )

    private class CancelAfterChecksMonitor(
        private val cancelAfter: Int,
    ) : NullProgressMonitor() {
        var checks: Int = 0
            private set
        var cancellationStackTrace: Array<StackTraceElement>? = null
            private set

        override fun isCanceled(): Boolean {
            checks++
            if (checks > cancelAfter) {
                cancellationStackTrace = Throwable().stackTrace
                return true
            }
            return false
        }
    }
}
