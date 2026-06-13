package io.github.mcdev.jdtls.handler

import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.support.JdtlsFixtureSupport
import io.github.mcdev.protocol.McdevErrorCode
import io.github.mcdev.protocol.McdevInfoResponse
import io.github.mcdev.protocol.McdevProtocol
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Path

class McdevInfoHandlerTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun returnsRealFabricProjectState() {
        val handler = createHandler()
        val response = handler.handle(listOf(contextPayload()))
        val info = assertIs<McdevInfoResponse>(response.result)
        assertTrue(info.lines.any { it.startsWith("Platform: Fabric") })
        assertTrue(info.lines.any { it.startsWith("Mixin config:") })
        assertTrue(info.lines.any { it.startsWith("Mappings:") })
    }

    @Test
    fun includesProtocolAndExtensionVersions() {
        val handler = createHandler()
        val response = handler.handle(listOf(contextPayload()))
        val info = assertIs<McdevInfoResponse>(response.result)
        assertTrue(info.lines.any { it == "Protocol: ${McdevProtocol.VERSION}" })
        assertTrue(info.lines.any { it == "Extension: ${McdevProtocol.SERVER_VERSION}" })
    }

    @Test
    fun reportsAccessWidenerDiscoveryForAwAtFixture() {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_AW_AT, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        val handler = McdevInfoHandler(projectService = FileBasedProjectContextService())
        val response = handler.handle(listOf(contextPayload()))
        val info = assertIs<McdevInfoResponse>(response.result)
        assertTrue(info.lines.any { it == "Access Widener: 1 file" })
        assertTrue(info.lines.any { it == "Access Transformer: 1 file" })
    }

    @Test
    fun reportsClasspathEntryCount() {
        val handler = createHandler()
        val response = handler.handle(listOf(contextPayload()))
        val info = assertIs<McdevInfoResponse>(response.result)
        assertTrue(info.lines.any { it.startsWith("Classpath entries:") && !it.endsWith("0") })
    }

    @Test
    fun missingWorkspaceRootReturnsIncompleteContextError() {
        val handler = McdevInfoHandler()
        val response = handler.handle(
            listOf(
                mapOf(
                    "context" to mapOf(
                        "protocolVersion" to McdevProtocol.VERSION,
                        "workspaceRoot" to "",
                        "documentUri" to "file:///Mixin.java",
                        "languageId" to "java",
                        "position" to mapOf("line" to 0, "character" to 0),
                        "bufferText" to "",
                        "client" to mapOf("name" to "mcdev.nvim", "version" to "0.1.0"),
                    ),
                ),
            ),
        )
        assertEquals(McdevErrorCode.INCOMPLETE_PROJECT_CONTEXT, response.error?.code)
    }

    private fun createHandler(): McdevInfoHandler {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_BASIC, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        return McdevInfoHandler(projectService = FileBasedProjectContextService())
    }

    private fun contextPayload(): Map<String, Any?> = mapOf(
        "context" to mapOf(
            "protocolVersion" to McdevProtocol.VERSION,
            "workspaceRoot" to JdtlsFixtureSupport.workspaceUri(tempDir),
            "documentUri" to "${JdtlsFixtureSupport.workspaceUri(tempDir)}/src/main/java/com/example/mixin/ExampleMixin.java",
            "languageId" to "java",
            "position" to mapOf("line" to 0, "character" to 0),
            "bufferText" to "",
            "client" to mapOf("name" to "mcdev.nvim", "version" to "0.1.0"),
        ),
    )
}
