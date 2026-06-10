package io.github.mcdev.jdtls.handler

import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata
import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.fixtures.FixtureResourceLoader
import io.github.mcdev.jdtls.mixin.MixinServiceFacade
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.support.JdtlsFixtureSupport
import io.github.mcdev.protocol.McdevCompletionResponse
import io.github.mcdev.protocol.McdevErrorCode
import io.github.mcdev.protocol.McdevProtocol
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Path

class McdevCompletionHandlerTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun returnsMixinClassCompletionsForFixture() {
        val handler = createHandler()
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_EXAMPLE_MIXIN)
        val (line, character) = JdtlsFixtureSupport.mixinCursorPosition(source, "SimpleTarget")
        val response = handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                    source = source,
                    line = line,
                    character = character,
                ),
            ),
        )
        val completion = assertIs<McdevCompletionResponse>(response.result)
        assertTrue(completion.items.isNotEmpty())
        assertTrue(completion.items.any { it.label == "SimpleTarget" })
        assertTrue(completion.items.any { it.insertText == "SimpleTarget.class" })
    }

    @Test
    fun returnsEmptyItemsWhenCursorIsOutsideAnnotation() {
        val handler = createHandler()
        val source = "package com.example;\npublic class Plain {}"
        val response = handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                    source = source,
                    line = 1,
                    character = 10,
                ),
            ),
        )
        val completion = assertIs<McdevCompletionResponse>(response.result)
        assertTrue(completion.items.isEmpty())
    }

    @Test
    fun missingWorkspaceRootReturnsIncompleteContextError() {
        val handler = McdevCompletionHandler()
        val response = handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = "",
                    source = "@Mixin(Simple)",
                    line = 0,
                    character = 8,
                ),
            ),
        )
        assertEquals(McdevErrorCode.INCOMPLETE_PROJECT_CONTEXT, response.error?.code)
    }

    @Test
    fun protocolMismatchReturnsStructuredError() {
        val handler = createHandler()
        val payload = completionPayload(
            workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
            source = "@Mixin(Simple)",
            line = 0,
            character = 8,
        ).toMutableMap()
        payload["protocolVersion"] = 99
        val response = handler.handle(listOf(payload))
        assertEquals(McdevErrorCode.PROTOCOL_MISMATCH, response.error?.code)
    }

    @Test
    fun completionItemsIncludeMetadataAndKind() {
        val handler = createHandler()
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_EXAMPLE_MIXIN)
        val (line, character) = JdtlsFixtureSupport.mixinCursorPosition(source, "SimpleTarget")
        val response = handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                    source = source,
                    line = line,
                    character = character,
                ),
            ),
        )
        val item = assertIs<McdevCompletionResponse>(response.result).items.first()
        assertEquals("class", item.kind)
        assertEquals("mixin.target", item.metadata["source"])
        assertNotNull(item.filterText)
    }

    @Test
    fun importModeMixinClassCompletionIncludesImportAdditionalEdit() {
        val handler = createHandler()
        val source = """
            package com.example.mixin;

            import org.spongepowered.asm.mixin.Mixin;

            @Mixin(Simple
        """.trimIndent()
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, source.length)
        val response = handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                    source = source,
                    line = line,
                    character = character,
                ),
            ),
        )
        val item = assertIs<McdevCompletionResponse>(response.result)
            .items
            .first { it.label == "SimpleTarget" }
        assertEquals(1, item.additionalEdits.size)
        assertTrue(item.additionalEdits.first().newText.contains("import com.example.target.SimpleTarget;"))
    }

    @Test
    fun atTargetCompletionRemapsInsertTextWithFixtureMappings() {
        val atTargetItem = McCompletionItem(
            label = "draw(String, float, float): void",
            detail = "SimpleTarget",
            documentation = "Lcom/example/target/SimpleTarget;draw(Ljava/lang/String;FF)V",
            filterText = "draw",
            insertText = "Lcom/example/target/SimpleTarget;draw(Ljava/lang/String;FF)V",
            kind = McCompletionKind.METHOD,
            sortKey = "0400_draw",
            metadata = McCompletionMetadata(
                source = "mixin.atTarget",
                owner = "com/example/target/SimpleTarget",
                name = "draw",
                descriptor = "(Ljava/lang/String;FF)V",
            ),
        )
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_BASIC, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        val handler = McdevCompletionHandler(
            projectService = FileBasedProjectContextService(),
            mixinFacade = MixinServiceFacade(completeOverride = { _, _, _, _, _ -> listOf(atTargetItem) }),
        )
        val source = """@At(value = "INVOKE", target = "draw")"""
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, source.indexOf("draw") + "draw".length)
        val response = handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                    source = source,
                    line = line,
                    character = character,
                ),
            ),
        )
        val item = assertIs<McdevCompletionResponse>(response.result).items.single()
        assertEquals("mixin.atTarget", item.metadata["source"])
        assertEquals(
            "Lcom/example/target/class_1;method_1(Ljava/lang/String;FF)V",
            item.insertText,
        )
        assertEquals(item.insertText, item.edit?.newText)
    }

    @Test
    fun returnsMixinExtrasMethodCompletionsThroughFacade() {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_MIXINEXTRAS, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        val handler = McdevCompletionHandler(
            projectService = FileBasedProjectContextService(),
            mixinFacade = MixinServiceFacade(),
        )
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_MIXINEXTRAS_MIXIN)
        val (line, character) = JdtlsFixtureSupport.memberCursorPosition(source, "dra")
        val response = handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                    source = source,
                    line = line,
                    character = character,
                ),
            ),
        )
        val completion = assertIs<McdevCompletionResponse>(response.result)
        assertTrue(completion.items.any { it.metadata["source"] == "mixinextras.injectMethod" })
        assertTrue(completion.items.any { it.insertText.startsWith("draw") })
    }

    @Test
    fun fqnInsertModeReturnsQualifiedClassName() {
        val handler = createHandler()
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_EXAMPLE_MIXIN)
        val (line, character) = JdtlsFixtureSupport.mixinCursorPosition(source, "SimpleTarget")
        val payload = completionPayload(
            workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
            source = source,
            line = line,
            character = character,
        ).toMutableMap()
        payload["options"] = mapOf(
            "preferredAtTarget" to "descriptor",
            "mixinClassInsert" to "fqn",
            "injectMethodDescriptor" to "auto",
        )
        val response = handler.handle(listOf(payload))
        val completion = assertIs<McdevCompletionResponse>(response.result)
        assertTrue(completion.items.any { it.insertText == "com.example.target.SimpleTarget.class" })
    }

    private fun createHandler(): McdevCompletionHandler {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_BASIC, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        return McdevCompletionHandler(projectService = FileBasedProjectContextService())
    }

    private fun completionPayload(
        workspaceRoot: String,
        source: String,
        line: Int,
        character: Int,
    ): Map<String, Any?> = mapOf(
        "protocolVersion" to McdevProtocol.VERSION,
        "workspaceRoot" to workspaceRoot,
        "documentUri" to "$workspaceRoot/src/main/java/com/example/mixin/ExampleMixin.java",
        "languageId" to "java",
        "position" to mapOf("line" to line, "character" to character),
        "bufferText" to source,
        "client" to mapOf("name" to "mcdev.nvim", "version" to "0.1.0"),
        "trigger" to mapOf("kind" to "manual", "character" to null),
        "options" to mapOf(
            "preferredAtTarget" to "descriptor",
            "mixinClassInsert" to "import",
            "injectMethodDescriptor" to "auto",
        ),
    )
}
