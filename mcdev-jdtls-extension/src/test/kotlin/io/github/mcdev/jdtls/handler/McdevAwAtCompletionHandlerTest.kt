package io.github.mcdev.jdtls.handler

import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.jdtls.awat.AwAtServiceFacade
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.support.JdtlsFixtureSupport
import io.github.mcdev.protocol.McdevCompletionResponse
import io.github.mcdev.protocol.McdevProtocol
import kotlin.test.Test
import kotlin.test.assertEquals
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
            accessible class com/example/target/SimpleTarget
        """.trimIndent()
        val cursorOffset = source.indexOf("SimpleTarget") + "Simp".length
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, cursorOffset)
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
        val item = completion.items.first { it.insertText == "com/example/target/SimpleTarget" }
        assertEquals("com/example/target/SimpleTarget".length, item.edit?.range?.end?.character?.minus(item.edit!!.range.start.character))
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
    fun returnsAccessWidenerDescriptorCompletionsAtDescriptorSlot() {
        val handler = createHandler()
        val source = """
            accessWidener v2 named
            accessible method com/example/target/SimpleTarget draw (Ljava/lang/Str
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
        assertTrue(completion.items.any { it.insertText == "(Ljava/lang/String;FF)V" })
        assertTrue(completion.items.any { it.metadata["source"] == "aw.descriptor" })
    }

    @Test
    fun returnsAccessTransformerFieldCompletionsWithIntermediaryInsertText() {
        val handler = createHandler()
        val source = "public com.example.target.SimpleTarget cou"
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
        assertTrue(
            completion.items.any {
                (it.insertText == "field_1" || it.insertText == "counter") &&
                    it.metadata["source"] == "at.member.field"
            },
        )
    }

    @Test
    fun returnsAccessTransformerMemberCompletionsWithMappedInsertText() {
        val handler = createHandler()
        val source = "public com.example.target.SimpleTarget draw(Ljava/lang/String;FF)V"
        val cursor = source.indexOf("draw") + "dr".length
        val response = handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                    source = source,
                    line = 0,
                    character = cursor,
                    languageId = "accesstransformer",
                    documentUri = "${JdtlsFixtureSupport.workspaceUri(tempDir)}/mod_at.cfg",
                ),
            ),
        )
        val completion = assertIs<McdevCompletionResponse>(response.result)
        val method = completion.items.first { it.insertText == "draw(Ljava/lang/String;FF)V" }
        assertEquals("value", method.kind)
        assertEquals(source.indexOf("dr"), method.edit?.range?.start?.character)
        assertEquals(source.length, method.edit?.range?.end?.character)
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
