package io.github.mcdev.jdtls.protocol

import io.github.mcdev.protocol.McdevCodeActionRequest
import io.github.mcdev.protocol.McdevCompletionRequest
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.definition.McDefinitionTarget
import io.github.mcdev.core.model.MemberKind
import io.github.mcdev.jdtls.convert.DefinitionConverter
import io.github.mcdev.jdtls.definition.ResolvedDefinition
import io.github.mcdev.protocol.McdevDefinitionRequest
import io.github.mcdev.protocol.McdevDefinitionResolution
import io.github.mcdev.protocol.McdevDefinitionResponse
import io.github.mcdev.protocol.McdevResponseEnvelope
import io.github.mcdev.protocol.McdevProtocol
import io.github.mcdev.protocol.McdevReferencesRequest
import io.github.mcdev.protocol.McdevRequestContext
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull

class ProtocolPayloadDecoderTest {
    private val decoder = ProtocolPayloadDecoder()

    @Test
    fun decodesCompletionRequestFromMapPayload() {
        val request = decoder.decodeCompletionRequest(listOf(completionPayload()))
        assertEquals("manual", request.trigger.kind)
        assertEquals("import", request.options.mixinClassInsert)
        assertEquals("file:///tmp/project", request.context.workspaceRoot)
        assertEquals("java", request.context.languageId)
    }

    @Test
    fun decodesFlatContextPayload() {
        val context = decoder.decodeContextRequest(listOf(contextPayload()))
        assertEquals(12, context.position.line)
        assertEquals(4, context.position.character)
        assertEquals("mcdev.nvim", context.client.name)
    }

    @Test
    fun decodesNestedContextPayload() {
        val request = decoder.decodeInfoRequest(listOf(mapOf("context" to contextPayload())))
        assertEquals("0.1.0", request.context.client.version)
    }

    @Test
    fun decodeRoundtripPreservesCompletionRequest() {
        val original = decoder.decodeCompletionRequest(listOf(completionPayload()))
        val encoded = decoder.encodeToMap(original)
        val restored = decoder.decodeFromMap(encoded, McdevCompletionRequest::class.java)
        assertEquals(original, restored)
    }

    @Test
    fun decodeRoundtripPreservesRequestContext() {
        val original = decoder.decodeContextRequest(listOf(contextPayload()))
        val encoded = decoder.encodeToMap(original)
        val restored = decoder.decodeFromMap(encoded, McdevRequestContext::class.java)
        assertEquals(original, restored)
    }

    @Test
    fun missingPayloadThrowsParseError() {
        assertFailsWith<ProtocolDecodeException> {
            decoder.decodeCompletionRequest(emptyList())
        }
    }

    @Test
    fun missingWorkspaceRootThrowsParseError() {
        val payload = contextPayload().toMutableMap()
        payload.remove("workspaceRoot")
        assertFailsWith<ProtocolDecodeException> {
            decoder.decodeContextRequest(listOf(payload))
        }
    }

    @Test
    fun missingPositionThrowsParseError() {
        val payload = contextPayload().toMutableMap()
        payload.remove("position")
        assertFailsWith<ProtocolDecodeException> {
            decoder.decodeContextRequest(listOf(payload))
        }
    }

    @Test
    fun defaultsProtocolVersionWhenMissing() {
        val payload = contextPayload().toMutableMap()
        payload.remove("protocolVersion")
        val context = decoder.decodeContextRequest(listOf(payload))
        assertEquals(McdevProtocol.VERSION, context.protocolVersion)
    }

    @Test
    fun decodeDiagnosticsRequestUsesContext() {
        val request = decoder.decodeDiagnosticsRequest(listOf(mapOf("context" to contextPayload())))
        assertNotNull(request.context.bufferText)
    }

    @Test
    fun decodesCodeActionRequestWithDiagnosticCodes() {
        val request = decoder.decodeCodeActionRequest(listOf(codeActionPayload()))
        assertEquals(1, request.diagnosticCodes.size)
        assertEquals("MIXIN_CLASS_NOT_LISTED_IN_CONFIG", request.diagnosticCodes.first())
    }

    @Test
    fun decodeRoundtripPreservesCodeActionRequest() {
        val original = decoder.decodeCodeActionRequest(listOf(codeActionPayload()))
        val encoded = decoder.encodeToMap(original)
        val restored = decoder.decodeFromMap(encoded, McdevCodeActionRequest::class.java)
        assertEquals(original, restored)
    }

    @Test
    fun decodeRoundtripPreservesDefinitionRequest() {
        val original = decoder.decodeDefinitionRequest(listOf(mapOf("context" to contextPayload())))
        val encoded = decoder.encodeToMap(original)
        val restored = decoder.decodeFromMap(encoded, McdevDefinitionRequest::class.java)
        assertEquals(original, restored)
    }

    @Test
    fun encodeDefinitionResponseUsesLowercaseResolutionWireValues() {
        val target = McDefinitionTarget(
            kind = MemberKind.CLASS,
            ownerInternalName = "com/example/target/SimpleTarget",
            ownerFqn = "com.example.target.SimpleTarget",
        )
        val envelope = McdevResponseEnvelope(
            capabilities = setOf("definition"),
            result = McdevDefinitionResponse(
                locations = listOf(
                    DefinitionConverter.toLocation(
                        ResolvedDefinition(
                            target = target,
                            documentUri = "file:///mapped-sources/SimpleTarget.java",
                            range = McTextRange(McTextPosition(3, 0), McTextPosition(3, 20)),
                            resolution = McdevDefinitionResolution.SOURCE,
                        ),
                    ),
                ),
            ),
        )
        val encoded = decoder.encodeToMap(envelope)
        @Suppress("UNCHECKED_CAST")
        val locations = ((encoded["result"] as Map<String, Any?>)["locations"] as List<Map<String, Any?>>)
        assertEquals("source", locations.single()["resolution"])
    }

    @Test
    fun decodeRoundtripPreservesReferencesRequest() {
        val original = decoder.decodeReferencesRequest(listOf(mapOf("context" to contextPayload())))
        val encoded = decoder.encodeToMap(original)
        val restored = decoder.decodeFromMap(encoded, McdevReferencesRequest::class.java)
        assertEquals(original, restored)
    }

    private fun contextPayload(): Map<String, Any?> = mapOf(
        "protocolVersion" to 1,
        "workspaceRoot" to "file:///tmp/project",
        "documentUri" to "file:///tmp/project/src/Mixin.java",
        "languageId" to "java",
        "position" to mapOf("line" to 12, "character" to 4),
        "bufferText" to "@Mixin(Mine",
        "client" to mapOf("name" to "mcdev.nvim", "version" to "0.1.0"),
    )

    private fun codeActionPayload(): Map<String, Any?> = mapOf(
        "context" to contextPayload(),
        "range" to mapOf(
            "start" to mapOf("line" to 0, "character" to 0),
            "end" to mapOf("line" to 0, "character" to 0),
        ),
        "diagnosticCodes" to listOf("MIXIN_CLASS_NOT_LISTED_IN_CONFIG"),
    )

    private fun completionPayload(): Map<String, Any?> = contextPayload() + mapOf(
        "trigger" to mapOf("kind" to "manual", "character" to null),
        "options" to mapOf(
            "preferredAtTarget" to "descriptor",
            "mixinClassInsert" to "import",
            "injectMethodDescriptor" to "auto",
        ),
    )
}
