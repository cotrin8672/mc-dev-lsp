package io.github.mcdev.jdtls.handler

import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.fixtures.FixtureResourceLoader
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.support.JdtlsFixtureSupport
import io.github.mcdev.protocol.McdevReferencesResponse
import io.github.mcdev.protocol.McdevProtocol
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Path

class McdevReferencesHandlerTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun findsMixinClassReferencesInWorkspace() {
        val handler = createHandler()
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_EXAMPLE_MIXIN)
        val (line, character) = JdtlsFixtureSupport.mixinCursorPosition(source, "SimpleTarget")
        val response = handler.handle(
            listOf(referencesPayload(source, line, character)),
        )
        val result = assertIs<McdevReferencesResponse>(response.result)
        assertTrue(result.locations.isNotEmpty())
        assertTrue(result.locations.any { it.metadata["source"] == "mixin.class" })
    }

    @Test
    fun findsShadowFieldReferencesInBuffer() {
        val handler = createHandler()
        val source = """
            package com.example.mixin;
            import com.example.target.SimpleTarget;
            import org.spongepowered.asm.mixin.Mixin;
            import org.spongepowered.asm.mixin.Shadow;
            @Mixin(SimpleTarget.class)
            public abstract class ExampleMixin {
                @Shadow private int counter;
            }
        """.trimIndent()
        val (line, character) = JdtlsFixtureSupport.memberCursorPosition(source, "counter", offsetInName = 2)
        val response = handler.handle(listOf(referencesPayload(source, line, character)))
        val result = assertIs<McdevReferencesResponse>(response.result)
        assertEquals(1, result.locations.size)
        assertEquals("mixin.shadow", result.locations.first().metadata["source"])
    }

    @Test
    fun returnsEmptyWhenDefinitionTargetMissing() {
        val handler = createHandler()
        val source = "package com.example;\npublic class Plain {}"
        val response = handler.handle(listOf(referencesPayload(source, 1, 10)))
        val result = assertIs<McdevReferencesResponse>(response.result)
        assertTrue(result.locations.isEmpty())
    }

    private fun createHandler(): McdevReferencesHandler {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_BASIC, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        return McdevReferencesHandler(projectService = FileBasedProjectContextService())
    }

    private fun referencesPayload(
        source: String,
        line: Int,
        character: Int,
    ): Map<String, Any?> = mapOf(
        "context" to mapOf(
            "protocolVersion" to McdevProtocol.VERSION,
            "workspaceRoot" to JdtlsFixtureSupport.workspaceUri(tempDir),
            "documentUri" to "${JdtlsFixtureSupport.workspaceUri(tempDir)}/src/main/java/com/example/mixin/ExampleMixin.java",
            "languageId" to "java",
            "position" to mapOf("line" to line, "character" to character),
            "bufferText" to source,
            "client" to mapOf("name" to "mcdev.nvim", "version" to "0.1.0"),
        ),
    )
}
