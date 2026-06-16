package io.github.mcdev.jdtls.command

import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.fixtures.FixtureResourceLoader
import io.github.mcdev.core.mixin.MixinDiagnosticCodes
import io.github.mcdev.jdtls.handler.McdevCodeActionHandler
import io.github.mcdev.jdtls.handler.McdevCompletionHandler
import io.github.mcdev.jdtls.handler.McdevDefinitionHandler
import io.github.mcdev.jdtls.handler.McdevDiagnosticsHandler
import io.github.mcdev.jdtls.handler.McdevHoverHandler
import io.github.mcdev.jdtls.handler.McdevInfoHandler
import io.github.mcdev.jdtls.handler.McdevProjectContextHandler
import io.github.mcdev.jdtls.handler.McdevReferencesHandler
import io.github.mcdev.jdtls.handler.McdevReindexHandler
import io.github.mcdev.jdtls.mixin.MixinServiceFacade
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.support.JdtlsFixtureSupport
import io.github.mcdev.protocol.McdevCommands
import io.github.mcdev.protocol.McdevCodeActionResponse
import io.github.mcdev.protocol.McdevCompletionResponse
import io.github.mcdev.protocol.McdevDefinitionResponse
import io.github.mcdev.protocol.McdevDiagnosticsResponse
import io.github.mcdev.protocol.McdevDumpContextResponse
import io.github.mcdev.protocol.McdevErrorCode
import io.github.mcdev.protocol.McdevHoverResponse
import io.github.mcdev.protocol.McdevInfoResponse
import io.github.mcdev.protocol.McdevProtocol
import io.github.mcdev.protocol.McdevReferencesResponse
import io.github.mcdev.protocol.McdevReloadProjectContextResponse
import io.github.mcdev.protocol.McdevReindexResponse
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Path

class McdevCommandDispatcherTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun infoReturnsStructuredLines() {
        val dispatcher = createDispatcher()
        val response = dispatcher.execute(McdevCommands.INFO, listOf(contextPayload("")))
        val info = assertIs<McdevInfoResponse>(response.result)
        assertTrue(info.lines.any { it.startsWith("Protocol:") })
        assertTrue(info.lines.any { it.startsWith("Platform: Fabric") })
    }

    @Test
    fun completionReturnsNonEmptyMixinItems() {
        val dispatcher = createDispatcher()
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_EXAMPLE_MIXIN)
        val (line, character) = JdtlsFixtureSupport.mixinCursorPosition(source, "SimpleTarget")
        val response = dispatcher.execute(
            McdevCommands.COMPLETION,
            listOf(completionPayload(source, line, character)),
        )
        val completion = assertIs<McdevCompletionResponse>(response.result)
        assertTrue(completion.items.isNotEmpty())
    }

    @Test
    fun diagnosticsReturnsDiagnosticsEnvelope() {
        val dispatcher = createDispatcher()
        val source = FixtureResourceLoader.loadText(FixturePaths.BROKEN_DIAGNOSTICS_MIXIN)
        val response = dispatcher.execute(
            McdevCommands.DIAGNOSTICS,
            listOf(contextPayload(source)),
        )
        val diagnostics = assertIs<McdevDiagnosticsResponse>(response.result)
        assertTrue(diagnostics.diagnostics.isNotEmpty())
    }

    @Test
    fun contextAliasReturnsDiagnosticsEnvelope() {
        val dispatcher = createDispatcher()
        val source = FixtureResourceLoader.loadText(FixturePaths.BROKEN_DIAGNOSTICS_MIXIN)
        val response = dispatcher.execute(
            McdevCommands.CONTEXT,
            listOf(contextPayload(source)),
        )
        val diagnostics = assertIs<McdevDiagnosticsResponse>(response.result)
        assertTrue(diagnostics.diagnostics.isNotEmpty())
    }

    @Test
    fun reindexReturnsStructuredStatus() {
        val dispatcher = createDispatcher()
        val response = dispatcher.execute(McdevCommands.REINDEX, listOf(contextPayload("")))
        val result = assertIs<McdevReindexResponse>(response.result)
        assertEquals("reindex complete", result.status)
    }

    @Test
    fun reloadProjectContextReturnsStructuredStatus() {
        val dispatcher = createDispatcher()
        val response = dispatcher.execute(McdevCommands.RELOAD_PROJECT_CONTEXT, listOf(contextPayload("")))
        val result = assertIs<McdevReloadProjectContextResponse>(response.result)
        assertEquals("project context reloaded", result.status)
        assertEquals("ready", result.indexState)
        assertTrue(result.lines.any { it.startsWith("Project:") })
    }

    @Test
    fun dumpContextReturnsStructuredProjectDetails() {
        val dispatcher = createDispatcher()
        val response = dispatcher.execute(McdevCommands.DUMP_CONTEXT, listOf(contextPayload("")))
        val result = assertIs<McdevDumpContextResponse>(response.result)
        assertEquals("fabric", result.platform)
        assertEquals("ready", result.indexState)
        assertTrue(result.classpath.entryCount >= 1)
        assertTrue(result.lines.any { it.startsWith("Platform: Fabric") })
    }

    @Test
    fun unknownCommandReturnsStructuredError() {
        val dispatcher = McdevCommandDispatcher()
        val response = dispatcher.execute("mcdev.unknown", emptyList())
        assertEquals(McdevErrorCode.PARSE_ERROR, response.error?.code)
    }

    @Test
    fun definitionReturnsMixinClassTarget() {
        val dispatcher = createDispatcher()
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_EXAMPLE_MIXIN)
        val (line, character) = JdtlsFixtureSupport.mixinCursorPosition(source, "SimpleTarget")
        val response = dispatcher.execute(
            McdevCommands.DEFINITION,
            listOf(definitionPayload(source, line, character)),
        )
        val result = assertIs<McdevDefinitionResponse>(response.result)
        assertEquals("com/example/target/SimpleTarget", result.locations.first().metadata["owner"])
    }

    @Test
    fun codeActionReturnsMixinConfigFix() {
        val dispatcher = createDispatcher()
        val source = """
            package com.example.mixin;
            import com.example.target.SimpleTarget;
            import org.spongepowered.asm.mixin.Mixin;
            @Mixin(SimpleTarget.class)
            public abstract class UnlistedMixin {}
        """.trimIndent()
        val response = dispatcher.execute(
            McdevCommands.CODE_ACTION,
            listOf(
                codeActionPayload(
                    source,
                    listOf(MixinDiagnosticCodes.MIXIN_CLASS_NOT_LISTED_IN_CONFIG),
                ),
            ),
        )
        val result = assertIs<McdevCodeActionResponse>(response.result)
        assertTrue(result.actions.any { it.kind == "quickfix.mixin.config" })
    }

    @Test
    fun referencesReturnsMixinUsages() {
        val dispatcher = createDispatcher()
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_EXAMPLE_MIXIN)
        val (line, character) = JdtlsFixtureSupport.mixinCursorPosition(source, "SimpleTarget")
        val response = dispatcher.execute(
            McdevCommands.REFERENCES,
            listOf(definitionPayload(source, line, character)),
        )
        val result = assertIs<McdevReferencesResponse>(response.result)
        assertTrue(result.locations.isNotEmpty())
    }

    @Test
    fun hoverReturnsMixinClassDetails() {
        val dispatcher = createDispatcher()
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_EXAMPLE_MIXIN)
        val (line, character) = JdtlsFixtureSupport.mixinCursorPosition(source, "SimpleTarget")
        val response = dispatcher.execute(
            McdevCommands.HOVER,
            listOf(definitionPayload(source, line, character)),
        )
        val result = assertIs<McdevHoverResponse>(response.result)
        assertTrue(result.contents.any { it.contains("com.example.target.SimpleTarget") })
        assertTrue(result.contents.any { it.contains("owner: com/example/target/SimpleTarget") })
    }

    @Test
    fun registryListsImplementedCommands() {
        assertTrue(McdevCommandRegistry.implementedCommandIds.contains(McdevCommands.COMPLETION))
        assertTrue(McdevCommandRegistry.implementedCommandIds.contains(McdevCommands.DIAGNOSTICS))
        assertTrue(McdevCommandRegistry.implementedCommandIds.contains(McdevCommands.CONTEXT))
        assertTrue(McdevCommandRegistry.implementedCommandIds.contains(McdevCommands.INFO))
        assertTrue(McdevCommandRegistry.implementedCommandIds.contains(McdevCommands.REINDEX))
        assertTrue(McdevCommandRegistry.implementedCommandIds.contains(McdevCommands.RELOAD_PROJECT_CONTEXT))
        assertTrue(McdevCommandRegistry.implementedCommandIds.contains(McdevCommands.DUMP_CONTEXT))
        assertTrue(McdevCommandRegistry.implementedCommandIds.contains(McdevCommands.DEFINITION))
        assertTrue(McdevCommandRegistry.implementedCommandIds.contains(McdevCommands.REFERENCES))
        assertTrue(McdevCommandRegistry.implementedCommandIds.contains(McdevCommands.HOVER))
        assertTrue(McdevCommandRegistry.implementedCommandIds.contains(McdevCommands.CODE_ACTION))
    }

    @Test
    fun completionWithMissingWorkspaceReturnsError() {
        val dispatcher = createDispatcher()
        val response = dispatcher.execute(
            McdevCommands.COMPLETION,
            listOf(
                completionPayload(
                    source = "@Mixin(Simple)",
                    line = 0,
                    character = 8,
                    workspaceRoot = "",
                ),
            ),
        )
        assertEquals(McdevErrorCode.INCOMPLETE_PROJECT_CONTEXT, response.error?.code)
    }

    @Test
    fun endToEndDecodeAndExecuteRoundtrip() {
        val dispatcher = createDispatcher()
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_EXAMPLE_MIXIN)
        val (line, character) = JdtlsFixtureSupport.mixinCursorPosition(source, "SimpleTarget")
        val payload = completionPayload(source, line, character)
        val response = dispatcher.execute(McdevCommands.COMPLETION, listOf(payload))
        val completion = assertIs<McdevCompletionResponse>(response.result)
        assertEquals(McdevProtocol.VERSION, response.protocolVersion)
        assertTrue(completion.items.any { it.label == "SimpleTarget" })
    }

    private fun createDispatcher(): McdevCommandDispatcher {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_BASIC, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        val projectService = FileBasedProjectContextService()
        val mixinFacade = MixinServiceFacade()
        return McdevCommandDispatcher(
            completionHandler = McdevCompletionHandler(projectService = projectService, mixinFacade = mixinFacade),
            diagnosticsHandler = McdevDiagnosticsHandler(projectService = projectService, mixinFacade = mixinFacade),
            infoHandler = McdevInfoHandler(projectService = projectService),
            reindexHandler = McdevReindexHandler(projectService = projectService),
            projectContextHandler = McdevProjectContextHandler(projectService = projectService),
            codeActionHandler = McdevCodeActionHandler(projectService = projectService, mixinFacade = mixinFacade),
            definitionHandler = McdevDefinitionHandler(projectService = projectService, mixinFacade = mixinFacade),
            referencesHandler = McdevReferencesHandler(projectService = projectService, mixinFacade = mixinFacade),
            hoverHandler = McdevHoverHandler(projectService = projectService, mixinFacade = mixinFacade),
        )
    }

    private fun contextPayload(source: String): Map<String, Any?> = mapOf(
        "context" to mapOf(
            "protocolVersion" to McdevProtocol.VERSION,
            "workspaceRoot" to workspaceRoot(),
            "documentUri" to "${workspaceRoot()}/src/main/java/com/example/mixin/ExampleMixin.java",
            "languageId" to "java",
            "position" to mapOf("line" to 0, "character" to 0),
            "bufferText" to source,
            "client" to mapOf("name" to "mcdev.nvim", "version" to "0.1.0"),
        ),
    )

    private fun completionPayload(
        source: String,
        line: Int,
        character: Int,
        workspaceRoot: String = workspaceRoot(),
    ): Map<String, Any?> = contextPayload(source) + mapOf(
        "trigger" to mapOf("kind" to "manual", "character" to null),
        "options" to mapOf(
            "preferredAtTarget" to "descriptor",
            "mixinClassInsert" to "import",
            "injectMethodDescriptor" to "auto",
        ),
        "context" to (contextPayload(source)["context"] as Map<String, Any?>).toMutableMap().apply {
            put("position", mapOf("line" to line, "character" to character))
            put("workspaceRoot", workspaceRoot)
        },
    )

    private fun definitionPayload(
        source: String,
        line: Int,
        character: Int,
    ): Map<String, Any?> = mapOf(
        "context" to (contextPayload(source)["context"] as Map<String, Any?>).toMutableMap().apply {
            put("position", mapOf("line" to line, "character" to character))
        },
    )

    private fun codeActionPayload(
        source: String,
        diagnosticCodes: List<String>,
    ): Map<String, Any?> = mapOf(
        "context" to mapOf(
            "protocolVersion" to McdevProtocol.VERSION,
            "workspaceRoot" to workspaceRoot(),
            "documentUri" to "${workspaceRoot()}/src/main/java/com/example/mixin/UnlistedMixin.java",
            "languageId" to "java",
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

    private fun workspaceRoot(): String = JdtlsFixtureSupport.workspaceUri(tempDir)
}
