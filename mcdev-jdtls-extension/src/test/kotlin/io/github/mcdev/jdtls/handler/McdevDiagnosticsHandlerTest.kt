package io.github.mcdev.jdtls.handler

import io.github.mcdev.core.mixin.MixinDiagnosticCodes
import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.fixtures.FixtureResourceLoader
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.support.JdtlsFixtureSupport
import io.github.mcdev.protocol.McdevDiagnosticsResponse
import io.github.mcdev.protocol.McdevErrorCode
import io.github.mcdev.protocol.McdevProtocol
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Path

class McdevDiagnosticsHandlerTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun reportsUnresolvedInjectMethodForBrokenFixture() {
        val handler = createHandler(FixturePaths.BROKEN_DIAGNOSTICS)
        val source = FixtureResourceLoader.loadText(FixturePaths.BROKEN_DIAGNOSTICS_MIXIN)
        val response = handler.handle(listOf(contextPayload(source)))
        val diagnostics = assertIs<McdevDiagnosticsResponse>(response.result).diagnostics
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.UNRESOLVED_INJECT_METHOD })
    }

    @Test
    fun reportsUnresolvedAtTargetForBrokenFixture() {
        val handler = createHandler(FixturePaths.BROKEN_DIAGNOSTICS)
        val source = FixtureResourceLoader.loadText(FixturePaths.BROKEN_DIAGNOSTICS_MIXIN)
        val response = handler.handle(listOf(contextPayload(source)))
        val diagnostics = assertIs<McdevDiagnosticsResponse>(response.result).diagnostics
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.UNRESOLVED_AT_TARGET })
    }

    @Test
    fun cleanFixtureProducesNoUnresolvedMixinTarget() {
        val handler = createHandler(FixturePaths.FABRIC_BASIC)
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_EXAMPLE_MIXIN)
        val response = handler.handle(listOf(contextPayload(source)))
        val diagnostics = assertIs<McdevDiagnosticsResponse>(response.result).diagnostics
        assertTrue(diagnostics.none { it.code == MixinDiagnosticCodes.UNRESOLVED_MIXIN_TARGET })
    }

    @Test
    fun diagnosticsIncludeSeverityAndMessage() {
        val handler = createHandler(FixturePaths.BROKEN_DIAGNOSTICS)
        val source = FixtureResourceLoader.loadText(FixturePaths.BROKEN_DIAGNOSTICS_MIXIN)
        val response = handler.handle(listOf(contextPayload(source)))
        val diagnostic = assertIs<McdevDiagnosticsResponse>(response.result).diagnostics.first()
        assertTrue(diagnostic.severity.isNotBlank())
        assertTrue(diagnostic.message.isNotBlank())
    }

    @Test
    fun missingWorkspaceRootReturnsIncompleteContextError() {
        val handler = McdevDiagnosticsHandler()
        val response = handler.handle(
            listOf(
                mapOf(
                    "context" to mapOf(
                        "protocolVersion" to McdevProtocol.VERSION,
                        "workspaceRoot" to "",
                        "documentUri" to "file:///Mixin.java",
                        "languageId" to "java",
                        "position" to mapOf("line" to 0, "character" to 0),
                        "bufferText" to "@Mixin(Test.class)",
                        "client" to mapOf("name" to "mcdev.nvim", "version" to "0.1.0"),
                    ),
                ),
            ),
        )
        assertEquals(McdevErrorCode.INCOMPLETE_PROJECT_CONTEXT, response.error?.code)
    }

    private fun createHandler(fixtureRoot: String): McdevDiagnosticsHandler {
        JdtlsFixtureSupport.copyFixture(fixtureRoot, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        return McdevDiagnosticsHandler(projectService = FileBasedProjectContextService())
    }

    private fun contextPayload(source: String): Map<String, Any?> = mapOf(
        "context" to mapOf(
            "protocolVersion" to McdevProtocol.VERSION,
            "workspaceRoot" to JdtlsFixtureSupport.workspaceUri(tempDir),
            "documentUri" to "${JdtlsFixtureSupport.workspaceUri(tempDir)}/src/main/java/com/example/mixin/ExampleMixin.java",
            "languageId" to "java",
            "position" to mapOf("line" to 0, "character" to 0),
            "bufferText" to source,
            "client" to mapOf("name" to "mcdev.nvim", "version" to "0.1.0"),
        ),
    )
}
