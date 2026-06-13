package io.github.mcdev.jdtls.handler

import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.fixtures.FixtureResourceLoader
import io.github.mcdev.jdtls.awat.AwAtServiceFacade
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.support.JdtlsFixtureSupport
import io.github.mcdev.protocol.McdevDefinitionResolution
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
        val location = result.locations.first()
        assertEquals("class", location.metadata["kind"])
        assertEquals("com/example/target/SimpleTarget", location.metadata["owner"])
        assertEquals("com.example.target.SimpleTarget", location.metadata["fqn"])
        assertEquals(McdevDefinitionResolution.SOURCE, location.resolution)
        assertTrue(location.documentUri.contains("SimpleTarget.java"))
        assertEquals(2, location.range.start.line)
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
        val location = result.locations.first()
        assertEquals("field", location.metadata["kind"])
        assertEquals("counter", location.metadata["name"])
        assertEquals(McdevDefinitionResolution.SOURCE, location.resolution)
        assertTrue(location.documentUri.contains("SimpleTarget.java"))
        assertEquals(3, location.range.start.line)
    }

    @Test
    fun resolvesAccessWidenerOwnerToClassDefinition() {
        val handler = createAwAtHandler()
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_AW_AT_ACCESS_WIDENER)
        val marker = "accessible class com/example/target/Simple"
        val offset = source.indexOf(marker) + marker.length
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, offset)
        val response = handler.handle(
            listOf(
                definitionPayload(
                    source = source,
                    line = line,
                    character = character,
                    languageId = "accesswidener",
                    documentUri = "${JdtlsFixtureSupport.workspaceUri(tempDir)}/src/main/resources/mod.accesswidener",
                ),
            ),
        )
        val result = assertIs<McdevDefinitionResponse>(response.result)
        assertEquals(1, result.locations.size)
        val location = result.locations.first()
        assertEquals("class", location.metadata["kind"])
        assertEquals("com/example/target/SimpleTarget", location.metadata["owner"])
        assertEquals(McdevDefinitionResolution.SOURCE, location.resolution)
        assertTrue(location.documentUri.contains("SimpleTarget.java"))
    }

    @Test
    fun resolvesAccessTransformerMemberToFieldDefinition() {
        val handler = createAwAtHandler()
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_AW_AT_ACCESS_TRANSFORMER)
        val marker = "public com.example.target.SimpleTarget counter"
        val offset = source.indexOf(marker) + marker.indexOf("counter") + 3
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, offset)
        val response = handler.handle(
            listOf(
                definitionPayload(
                    source = source,
                    line = line,
                    character = character,
                    languageId = "accesstransformer",
                    documentUri = "${JdtlsFixtureSupport.workspaceUri(tempDir)}/src/main/resources/mod_at.cfg",
                ),
            ),
        )
        val result = assertIs<McdevDefinitionResponse>(response.result)
        assertEquals(1, result.locations.size)
        val location = result.locations.first()
        assertEquals("field", location.metadata["kind"])
        assertEquals("counter", location.metadata["name"])
        assertEquals(McdevDefinitionResolution.SOURCE, location.resolution)
        assertTrue(location.documentUri.contains("SimpleTarget.java"))
        assertEquals(3, location.range.start.line)
    }

    @Test
    fun resolvesLoomMixinTargetFromMappedSources() {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_LOOM_E2E, tempDir)
        JdtlsFixtureSupport.installLoomRemappedJar(tempDir)
        val handler = McdevDefinitionHandler(projectService = FileBasedProjectContextService())
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_LOOM_E2E_EXAMPLE_MIXIN)
        val mixinLine = source.lineSequence().toList().indexOfFirst { it.contains("@Mixin") }
        require(mixinLine >= 0)
        val response = handler.handle(listOf(definitionPayload(source, mixinLine, 8)))
        val result = assertIs<McdevDefinitionResponse>(response.result)
        val location = result.locations.single()
        assertEquals("class", location.metadata["kind"])
        assertEquals(McdevDefinitionResolution.SOURCE, location.resolution)
        assertTrue(location.documentUri.contains("mapped-sources"), location.documentUri)
        assertTrue(location.documentUri.contains("SimpleTarget.java"), location.documentUri)
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

    private fun createAwAtHandler(): McdevDefinitionHandler {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_AW_AT, tempDir)
        JdtlsFixtureSupport.copyFixtureResource(
            "${FixturePaths.FABRIC_BASIC}/src/main/java/com/example/target/SimpleTarget.java",
            tempDir.resolve("src/main/java/com/example/target/SimpleTarget.java"),
        )
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        return McdevDefinitionHandler(
            projectService = FileBasedProjectContextService(),
            awAtFacade = AwAtServiceFacade(),
        )
    }

    private fun definitionPayload(
        source: String,
        line: Int,
        character: Int,
        workspaceRoot: String = JdtlsFixtureSupport.workspaceUri(tempDir),
        languageId: String = "java",
        documentUri: String = "$workspaceRoot/src/main/java/com/example/mixin/ExampleMixin.java",
    ): Map<String, Any?> = mapOf(
        "context" to mapOf(
            "protocolVersion" to McdevProtocol.VERSION,
            "workspaceRoot" to workspaceRoot,
            "documentUri" to documentUri,
            "languageId" to languageId,
            "position" to mapOf("line" to line, "character" to character),
            "bufferText" to source,
            "client" to mapOf("name" to "mcdev.nvim", "version" to "0.1.0"),
        ),
    )
}
