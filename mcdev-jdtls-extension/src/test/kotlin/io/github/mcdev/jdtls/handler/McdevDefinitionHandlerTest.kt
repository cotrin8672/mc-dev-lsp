package io.github.mcdev.jdtls.handler

import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.fixtures.FixtureResourceLoader
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.support.JdtlsFixtureSupport
import io.github.mcdev.protocol.McdevDefinitionResponse
import io.github.mcdev.protocol.McdevErrorCode
import io.github.mcdev.protocol.McdevProtocol
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Path

class McdevDefinitionHandlerTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun resolvesMixinClassTarget() {
        val handler = createHandler()
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_EXAMPLE_MIXIN)
        val (line, character) = JdtlsFixtureSupport.mixinCursorPosition(source, "SimpleTarget")
        val response = handler.handle(
            listOf(definitionPayload(source, line, character)),
        )
        val result = assertIs<McdevDefinitionResponse>(response.result)
        assertEquals(1, result.locations.size)
        assertEquals("class", result.locations.first().metadata["kind"])
        assertEquals("com/example/target/SimpleTarget", result.locations.first().metadata["owner"])
        assertEquals("com.example.target.SimpleTarget", result.locations.first().metadata["fqn"])
    }

    @Test
    fun resolvesShadowFieldTarget() {
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
        val response = handler.handle(
            listOf(definitionPayload(source, line, character)),
        )
        val result = assertIs<McdevDefinitionResponse>(response.result)
        assertEquals(1, result.locations.size)
        assertEquals("field", result.locations.first().metadata["kind"])
        assertEquals("counter", result.locations.first().metadata["name"])
    }

    @Test
    fun returnsEmptyLocationsOutsideMixinContext() {
        val handler = createHandler()
        val source = "package com.example;\npublic class Plain {}"
        val response = handler.handle(listOf(definitionPayload(source, 1, 10)))
        val result = assertIs<McdevDefinitionResponse>(response.result)
        assertTrue(result.locations.isEmpty())
    }

    @Test
    fun missingWorkspaceRootReturnsIncompleteContextError() {
        val handler = McdevDefinitionHandler()
        val response = handler.handle(
            listOf(definitionPayload("@Mixin(Simple)", 0, 8, workspaceRoot = "")),
        )
        assertEquals(McdevErrorCode.INCOMPLETE_PROJECT_CONTEXT, response.error?.code)
    }

    @Test
    fun definitionResponseIncludesSourceRangeMetadata() {
        val handler = createHandler()
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_EXAMPLE_MIXIN)
        val (line, character) = JdtlsFixtureSupport.mixinCursorPosition(source, "SimpleTarget")
        val response = handler.handle(listOf(definitionPayload(source, line, character)))
        val location = assertIs<McdevDefinitionResponse>(response.result).locations.first()
        assertTrue(location.range.start.line >= 0)
        assertTrue(location.range.end.character > location.range.start.character)
    }

    private fun createHandler(): McdevDefinitionHandler {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_BASIC, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        return McdevDefinitionHandler(projectService = FileBasedProjectContextService())
    }

    private fun definitionPayload(
        source: String,
        line: Int,
        character: Int,
        workspaceRoot: String = JdtlsFixtureSupport.workspaceUri(tempDir),
    ): Map<String, Any?> = mapOf(
        "context" to mapOf(
            "protocolVersion" to McdevProtocol.VERSION,
            "workspaceRoot" to workspaceRoot,
            "documentUri" to "$workspaceRoot/src/main/java/com/example/mixin/ExampleMixin.java",
            "languageId" to "java",
            "position" to mapOf("line" to line, "character" to character),
            "bufferText" to source,
            "client" to mapOf("name" to "mcdev.nvim", "version" to "0.1.0"),
        ),
    )
}
