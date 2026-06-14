package io.github.mcdev.jdtls.handler

import io.github.mcdev.core.aw.AwDiagnosticCodes
import io.github.mcdev.core.at.AtDiagnosticCodes
import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.support.JdtlsFixtureSupport
import io.github.mcdev.protocol.McdevCodeActionResponse
import io.github.mcdev.protocol.McdevProtocol
import kotlin.test.Test
import kotlin.test.assertIs
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Path

class McdevAwAtCodeActionHandlerTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun returnsRemoveDuplicateFixForAccessWidenerBuffer() {
        val handler = createHandler()
        val source = """
            accessWidener v2 named
            accessible class com/example/target/SimpleTarget
            accessible class com/example/target/SimpleTarget
        """.trimIndent()
        val response = handler.handle(
            listOf(
                codeActionPayload(
                    source = source,
                    languageId = "accesswidener",
                    fileName = "mod.accesswidener",
                    diagnosticCodes = listOf(AwDiagnosticCodes.DUPLICATE_ENTRY),
                ),
            ),
        )
        val result = assertIs<McdevCodeActionResponse>(response.result)
        assertTrue(result.actions.isNotEmpty())
        assertTrue(result.actions.any { it.kind == "quickfix.aw.removeDuplicate" })
        assertTrue(result.actions.first().edits.first().documentUri.endsWith("mod.accesswidener"))
    }

    @Test
    fun returnsAddDescriptorFixForAccessTransformerBuffer() {
        val handler = createHandler()
        val source = "public com.example.target.SimpleTarget draw"
        val response = handler.handle(
            listOf(
                codeActionPayload(
                    source = source,
                    languageId = "accesstransformer",
                    fileName = "mod_at.cfg",
                    diagnosticCodes = listOf(AtDiagnosticCodes.MISSING_METHOD_DESCRIPTOR),
                ),
            ),
        )
        val result = assertIs<McdevCodeActionResponse>(response.result)
        assertTrue(result.actions.none { it.title.contains("descriptor", ignoreCase = true) })
    }

    private fun createHandler(): McdevCodeActionHandler {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_AW_AT, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        return McdevCodeActionHandler(projectService = FileBasedProjectContextService())
    }

    private fun codeActionPayload(
        source: String,
        languageId: String,
        fileName: String,
        diagnosticCodes: List<String>,
    ): Map<String, Any?> {
        val workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir)
        return mapOf(
            "context" to mapOf(
                "protocolVersion" to McdevProtocol.VERSION,
                "workspaceRoot" to workspaceRoot,
                "documentUri" to "$workspaceRoot/$fileName",
                "languageId" to languageId,
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
}
