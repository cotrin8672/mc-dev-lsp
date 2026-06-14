package io.github.mcdev.jdtls.handler

import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.support.JdtlsFixtureSupport
import io.github.mcdev.protocol.McdevDumpContextResponse
import io.github.mcdev.protocol.McdevErrorCode
import io.github.mcdev.protocol.McdevProtocol
import io.github.mcdev.protocol.McdevReloadProjectContextResponse
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Path

class McdevProjectContextHandlerTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun reloadClearsAndRebuildsCachedProjectContext() {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_BASIC, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        val projectService = FileBasedProjectContextService()
        val handler = McdevProjectContextHandler(projectService = projectService)
        val uri = JdtlsFixtureSupport.workspaceUri(tempDir)

        projectService.loadSession(uri)
        val response = handler.reload(listOf(contextPayload()))

        val result = assertIs<McdevReloadProjectContextResponse>(response.result)
        assertEquals("project context reloaded", result.status)
        assertEquals("ready", result.indexState)
        assertTrue(result.classpathEntries >= 1)
        assertTrue(result.lines.any { it.startsWith("Classpath entries:") })
        assertEquals("ready", projectService.loadSession(uri).context.indexState.name.lowercase())
    }

    @Test
    fun dumpReturnsStructuredProjectContextDetails() {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_BASIC, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        val handler = McdevProjectContextHandler(projectService = FileBasedProjectContextService())

        val response = handler.dump(listOf(contextPayload()))

        val result = assertIs<McdevDumpContextResponse>(response.result)
        assertEquals("fabric", result.platform)
        assertEquals("ready", result.indexState)
        assertTrue(result.classpath.entryCount >= 1)
        assertTrue(result.sourceSets.any { it.name == "main" })
        assertTrue(result.lines.any { it.startsWith("Protocol:") })
    }

    @Test
    fun dumpWithMissingWorkspaceRootReturnsIncompleteContextError() {
        val handler = McdevProjectContextHandler()
        val response = handler.dump(
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
