package io.github.mcdev.jdtls.handler

import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.support.JdtlsFixtureSupport
import io.github.mcdev.protocol.McdevErrorCode
import io.github.mcdev.protocol.McdevProtocol
import io.github.mcdev.protocol.McdevReindexResponse
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Path

class McdevReindexHandlerTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun reindexReturnsReadyState() {
        val projectService = FileBasedProjectContextService()
        val handler = createHandler(projectService)
        val response = handler.handle(listOf(contextPayload()))
        val result = assertIs<McdevReindexResponse>(response.result)
        assertEquals("reindex complete", result.status)
        assertEquals("ready", result.indexState)
        assertTrue(result.classpathEntries >= 1)
    }

    @Test
    fun reindexUpdatesCachedSession() {
        val projectService = FileBasedProjectContextService()
        val handler = createHandler(projectService)
        val uri = JdtlsFixtureSupport.workspaceUri(tempDir)
        projectService.loadSession(uri)
        handler.handle(listOf(contextPayload()))
        val session = projectService.loadSession(uri)
        assertEquals("ready", session.context.indexState.name.lowercase())
    }

    @Test
    fun missingWorkspaceRootReturnsIncompleteContextError() {
        val handler = McdevReindexHandler()
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

    private fun createHandler(projectService: FileBasedProjectContextService): McdevReindexHandler {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_BASIC, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        return McdevReindexHandler(projectService = projectService)
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
