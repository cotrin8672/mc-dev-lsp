package io.github.mcdev.jdtls.handler

import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.jdtls.awat.AwAtServiceFacade
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.support.JdtlsFixtureSupport
import io.github.mcdev.protocol.McdevCompletionResponse
import io.github.mcdev.protocol.McdevProtocol
import kotlin.test.Test
import kotlin.test.assertIs
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Path

class McdevAwAtCompletionHandlerTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun returnsAccessWidenerDirectiveCompletionsForLanguageId() {
        val handler = createHandler()
        val source = """
            accessWidener v2 named
            access
        """.trimIndent()
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, source.length)
        val response = handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                    source = source,
                    line = line,
                    character = character,
                    languageId = "accesswidener",
                    documentUri = "${JdtlsFixtureSupport.workspaceUri(tempDir)}/mod.accesswidener",
                ),
            ),
        )
        val completion = assertIs<McdevCompletionResponse>(response.result)
        assertTrue(completion.items.any { it.label == "accessible" })
        assertTrue(completion.items.any { it.metadata["source"] == "aw.directive" })
    }

    @Test
    fun returnsAccessWidenerClassCompletionsByFileExtension() {
        val handler = createHandler()
        val source = """
            accessWidener v2 named
            accessible class com/example/target/Simp
        """.trimIndent()
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, source.length)
        val response = handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                    source = source,
                    line = line,
                    character = character,
                    languageId = "plaintext",
                    documentUri = "${JdtlsFixtureSupport.workspaceUri(tempDir)}/mod.aw",
                ),
            ),
        )
        val completion = assertIs<McdevCompletionResponse>(response.result)
        assertTrue(completion.items.any { it.insertText == "com/example/target/SimpleTarget" })
    }

    @Test
    fun returnsAccessTransformerModifierCompletionsForAtCfgBuffer() {
        val handler = createHandler()
        val source = "pub"
        val response = handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                    source = source,
                    line = 0,
                    character = source.length,
                    languageId = "accesstransformer",
                    documentUri = "${JdtlsFixtureSupport.workspaceUri(tempDir)}/mod_at.cfg",
                ),
            ),
        )
        val completion = assertIs<McdevCompletionResponse>(response.result)
        assertTrue(completion.items.any { it.insertText == "public" })
        assertTrue(completion.items.any { it.metadata["source"] == "at.modifier" })
    }

    @Test
    fun returnsAccessTransformerMemberCompletionsWithMappedInsertText() {
        val handler = createHandler()
        val source = "public com.example.target.SimpleTarget dr"
        val response = handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                    source = source,
                    line = 0,
                    character = source.length,
                    languageId = "accesstransformer",
                    documentUri = "${JdtlsFixtureSupport.workspaceUri(tempDir)}/mod_at.cfg",
                ),
            ),
        )
        val completion = assertIs<McdevCompletionResponse>(response.result)
        assertTrue(completion.items.any { it.insertText == "draw(Ljava/lang/String;FF)V" })
    }

    private fun createHandler(): McdevCompletionHandler {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_AW_AT, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        return McdevCompletionHandler(
            projectService = FileBasedProjectContextService(),
            awAtFacade = AwAtServiceFacade(),
        )
    }

    private fun completionPayload(
        workspaceRoot: String,
        source: String,
        line: Int,
        character: Int,
        languageId: String,
        documentUri: String,
    ): Map<String, Any?> = mapOf(
        "protocolVersion" to McdevProtocol.VERSION,
        "workspaceRoot" to workspaceRoot,
        "documentUri" to documentUri,
        "languageId" to languageId,
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
