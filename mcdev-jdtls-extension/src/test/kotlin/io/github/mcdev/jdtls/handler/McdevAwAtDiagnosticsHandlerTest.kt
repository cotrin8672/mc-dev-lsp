package io.github.mcdev.jdtls.handler

import io.github.mcdev.core.aw.AwDiagnosticCodes
import io.github.mcdev.core.at.AtDiagnosticCodes
import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.fixtures.FixtureResourceLoader
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.support.JdtlsFixtureSupport
import io.github.mcdev.protocol.McdevDiagnosticsResponse
import io.github.mcdev.protocol.McdevProtocol
import kotlin.test.Test
import kotlin.test.assertIs
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Path

class McdevAwAtDiagnosticsHandlerTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun reportsUnresolvedAccessWidenerClassForBrokenBuffer() {
        val handler = createHandler()
        val source = """
            accessWidener v2 named
            accessible class com/example/missing/Missing
        """.trimIndent()
        val response = handler.handle(listOf(contextPayload(source, "accesswidener", "mod.accesswidener")))
        val diagnostics = assertIs<McdevDiagnosticsResponse>(response.result).diagnostics
        assertTrue(diagnostics.any { it.code == AwDiagnosticCodes.UNRESOLVED_CLASS })
    }

    @Test
    fun cleanAccessWidenerFixtureProducesNoUnresolvedTargets() {
        val handler = createHandler()
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_AW_AT_ACCESS_WIDENER)
        val response = handler.handle(listOf(contextPayload(source, "accesswidener", "mod.accesswidener")))
        val diagnostics = assertIs<McdevDiagnosticsResponse>(response.result).diagnostics
        assertTrue(diagnostics.none { it.code == AwDiagnosticCodes.UNRESOLVED_CLASS })
        assertTrue(diagnostics.none { it.code == AwDiagnosticCodes.UNRESOLVED_MEMBER })
    }

    @Test
    fun cleanAccessTransformerFixtureProducesNoUnresolvedTargets() {
        val handler = createHandler()
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_AW_AT_ACCESS_TRANSFORMER)
        val response = handler.handle(listOf(contextPayload(source, "accesstransformer", "mod_at.cfg")))
        val diagnostics = assertIs<McdevDiagnosticsResponse>(response.result).diagnostics
        assertTrue(diagnostics.none { it.code == AtDiagnosticCodes.UNRESOLVED_CLASS })
        assertTrue(diagnostics.none { it.code == AtDiagnosticCodes.UNRESOLVED_MEMBER })
    }

    @Test
    fun detectsAccessTransformerBufferByFileExtension() {
        val handler = createHandler()
        val source = """
            public com.example.missing.Missing
        """.trimIndent()
        val response = handler.handle(listOf(contextPayload(source, "java", "mod_at.cfg")))
        val diagnostics = assertIs<McdevDiagnosticsResponse>(response.result).diagnostics
        assertTrue(diagnostics.any { it.code == AtDiagnosticCodes.UNRESOLVED_CLASS })
    }

    private fun createHandler(): McdevDiagnosticsHandler {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_AW_AT, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        return McdevDiagnosticsHandler(projectService = FileBasedProjectContextService())
    }

    private fun contextPayload(
        source: String,
        languageId: String,
        fileName: String,
    ): Map<String, Any?> = mapOf(
        "context" to mapOf(
            "protocolVersion" to McdevProtocol.VERSION,
            "workspaceRoot" to JdtlsFixtureSupport.workspaceUri(tempDir),
            "documentUri" to "${JdtlsFixtureSupport.workspaceUri(tempDir)}/$fileName",
            "languageId" to languageId,
            "position" to mapOf("line" to 0, "character" to 0),
            "bufferText" to source,
            "client" to mapOf("name" to "mcdev.nvim", "version" to "0.1.0"),
        ),
    )
}
