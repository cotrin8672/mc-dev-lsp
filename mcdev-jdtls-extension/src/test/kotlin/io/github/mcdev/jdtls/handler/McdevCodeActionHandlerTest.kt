package io.github.mcdev.jdtls.handler

import io.github.mcdev.core.mixin.MixinDiagnosticCodes
import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.fixtures.FixtureResourceLoader
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.support.JdtlsFixtureSupport
import io.github.mcdev.protocol.McdevCodeActionResponse
import io.github.mcdev.protocol.McdevErrorCode
import io.github.mcdev.protocol.McdevProtocol
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Path

class McdevCodeActionHandlerTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun returnsMixinConfigAddFix() {
        val handler = createHandler()
        val source = """
            package com.example.mixin;
            import com.example.target.SimpleTarget;
            import org.spongepowered.asm.mixin.Mixin;
            @Mixin(SimpleTarget.class)
            public abstract class UnlistedMixin {}
        """.trimIndent()
        val response = handler.handle(
            listOf(
                codeActionPayload(
                    source = source,
                    diagnosticCodes = listOf(MixinDiagnosticCodes.MIXIN_CLASS_NOT_LISTED_IN_CONFIG),
                ),
            ),
        )
        val result = assertIs<McdevCodeActionResponse>(response.result)
        assertTrue(result.actions.isNotEmpty())
        assertTrue(result.actions.any { it.title.contains("UnlistedMixin") })
        assertTrue(result.actions.any { it.kind == "quickfix.mixin.config" })
        assertTrue(result.actions.first().edits.isNotEmpty())
    }

    @Test
    fun missingWorkspaceRootReturnsIncompleteContextError() {
        val handler = McdevCodeActionHandler()
        val response = handler.handle(
            listOf(
                codeActionPayload(
                    source = "@Mixin(Simple)",
                    workspaceRoot = "",
                ),
            ),
        )
        assertEquals(McdevErrorCode.INCOMPLETE_PROJECT_CONTEXT, response.error?.code)
    }

    @Test
    fun protocolMismatchReturnsStructuredError() {
        val handler = createHandler()
        val payload = codeActionPayload(source = "@Mixin(Simple)").toMutableMap()
        val context = (payload["context"] as Map<String, Any?>).toMutableMap()
        context["protocolVersion"] = 99
        payload["context"] = context
        val response = handler.handle(listOf(payload))
        assertEquals(McdevErrorCode.PROTOCOL_MISMATCH, response.error?.code)
    }

    @Test
    fun codeActionEditsIncludeWorkspaceDocumentUri() {
        val handler = createHandler()
        val source = """
            package com.example.mixin;
            import com.example.target.SimpleTarget;
            import org.spongepowered.asm.mixin.Mixin;
            @Mixin(SimpleTarget.class)
            public abstract class UnlistedMixin {}
        """.trimIndent()
        val response = handler.handle(
            listOf(
                codeActionPayload(
                    source = source,
                    diagnosticCodes = listOf(MixinDiagnosticCodes.MIXIN_CLASS_NOT_LISTED_IN_CONFIG),
                ),
            ),
        )
        val action = assertIs<McdevCodeActionResponse>(response.result).actions.first()
        assertTrue(action.edits.first().documentUri.contains("mixins.json"))
    }

    private fun createHandler(): McdevCodeActionHandler {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_BASIC, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        return McdevCodeActionHandler(projectService = FileBasedProjectContextService())
    }

    private fun codeActionPayload(
        source: String,
        diagnosticCodes: List<String> = emptyList(),
        workspaceRoot: String = JdtlsFixtureSupport.workspaceUri(tempDir),
    ): Map<String, Any?> = mapOf(
        "context" to mapOf(
            "protocolVersion" to McdevProtocol.VERSION,
            "workspaceRoot" to workspaceRoot,
            "documentUri" to "$workspaceRoot/src/main/java/com/example/mixin/UnlistedMixin.java",
            "languageId" to "java",
            "position" to mapOf("line" to 0, "character" to 0),
            "bufferText" to source,
            "client" to mapOf("name" to "mcdev.nvim", "version" to "0.1.0"),
        ),
        "range" to mapOf(
            "start" to mapOf("line" to 0, "character" to 0),
            "end" to mapOf("line" to 0, "character" to 0),
        ),
        "diagnosticCodes" to diagnosticCodes,
    )
}
