package io.github.mcdev.jdtls.handler

import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.fixtures.FixtureResourceLoader
import io.github.mcdev.jdtls.awat.AwAtServiceFacade
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
    fun findsAccessWidenerReferencesFromJavaField() {
        val handler = createAwAtHandler()
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
        assertTrue(result.locations.any { it.metadata["source"] == "mixin.shadow" })
        assertTrue(result.locations.any { it.metadata["source"] == "aw.field" })
    }

    @Test
    fun findsAccessWidenerReferencesFromAccessWidenerBuffer() {
        val handler = createAwAtHandler()
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_AW_AT_ACCESS_WIDENER)
        val marker = "mutable field com/example/target/SimpleTarget counter"
        val offset = source.indexOf(marker) + marker.indexOf("counter") + 3
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, offset)
        val response = handler.handle(
            listOf(
                referencesPayload(
                    source = source,
                    line = line,
                    character = character,
                    languageId = "accesswidener",
                    documentUri = "${JdtlsFixtureSupport.workspaceUri(tempDir)}/src/main/resources/mod.accesswidener",
                ),
            ),
        )
        val result = assertIs<McdevReferencesResponse>(response.result)
        assertTrue(result.locations.any { it.metadata["source"] == "aw.field" })
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

    private fun createAwAtHandler(): McdevReferencesHandler {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_AW_AT, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        return McdevReferencesHandler(
            projectService = FileBasedProjectContextService(),
            awAtFacade = AwAtServiceFacade(),
        )
    }

    private fun referencesPayload(
        source: String,
        line: Int,
        character: Int,
        languageId: String = "java",
        documentUri: String = "${JdtlsFixtureSupport.workspaceUri(tempDir)}/src/main/java/com/example/mixin/ExampleMixin.java",
    ): Map<String, Any?> = mapOf(
        "context" to mapOf(
            "protocolVersion" to McdevProtocol.VERSION,
            "workspaceRoot" to JdtlsFixtureSupport.workspaceUri(tempDir),
            "documentUri" to documentUri,
            "languageId" to languageId,
            "position" to mapOf("line" to line, "character" to character),
            "bufferText" to source,
            "client" to mapOf("name" to "mcdev.nvim", "version" to "0.1.0"),
        ),
    )
}
