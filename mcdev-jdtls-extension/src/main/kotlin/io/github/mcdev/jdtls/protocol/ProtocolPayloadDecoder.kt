package io.github.mcdev.jdtls.protocol

import com.google.gson.Gson
import com.google.gson.GsonBuilder
import com.google.gson.JsonElement
import com.google.gson.JsonObject
import com.google.gson.TypeAdapter
import com.google.gson.stream.JsonReader
import com.google.gson.stream.JsonWriter
import io.github.mcdev.protocol.McdevDefinitionResolution
import io.github.mcdev.protocol.McdevClientInfo
import io.github.mcdev.protocol.McdevCodeActionRequest
import io.github.mcdev.protocol.McdevCompletionOptions
import io.github.mcdev.protocol.McdevCompletionRequest
import io.github.mcdev.protocol.McdevCompletionTrigger
import io.github.mcdev.protocol.McdevDefinitionRequest
import io.github.mcdev.protocol.McdevDiagnosticsRequest
import io.github.mcdev.protocol.McdevDumpContextRequest
import io.github.mcdev.protocol.McdevHoverRequest
import io.github.mcdev.protocol.McdevInfoRequest
import io.github.mcdev.protocol.McdevPosition
import io.github.mcdev.protocol.McdevProtocol
import io.github.mcdev.protocol.McdevRange
import io.github.mcdev.protocol.McdevReferencesRequest
import io.github.mcdev.protocol.McdevReloadProjectContextRequest
import io.github.mcdev.protocol.McdevReindexRequest
import io.github.mcdev.protocol.McdevRequestContext

class ProtocolPayloadDecoder(
    private val gson: Gson = createGson(),
) {
    fun decodeCompletionRequest(arguments: List<Any?>): McdevCompletionRequest {
        val root = decodeRoot(arguments)
        val context = decodeContext(root)
        val triggerObject = root.getAsJsonObjectOrEmpty("trigger")
        val optionsObject = root.getAsJsonObjectOrEmpty("options")
        return McdevCompletionRequest(
            context = context,
            trigger = McdevCompletionTrigger(
                kind = triggerObject.getStringOrDefault("kind", "manual"),
                character = triggerObject.getNullableString("character"),
            ),
            options = McdevCompletionOptions(
                preferredAtTarget = optionsObject.getStringOrDefault("preferredAtTarget", "smart"),
                mixinClassInsert = optionsObject.getStringOrDefault("mixinClassInsert", "import"),
                injectMethodDescriptor = optionsObject.getStringOrDefault("injectMethodDescriptor", "auto"),
            ),
        )
    }

    fun decodeContextRequest(arguments: List<Any?>): McdevRequestContext =
        decodeContext(decodeRoot(arguments))

    fun decodeInfoRequest(arguments: List<Any?>): McdevInfoRequest =
        McdevInfoRequest(context = decodeContextRequest(arguments))

    fun decodeDiagnosticsRequest(arguments: List<Any?>): McdevDiagnosticsRequest =
        McdevDiagnosticsRequest(context = decodeContextRequest(arguments))

    fun decodeReindexRequest(arguments: List<Any?>): McdevReindexRequest =
        McdevReindexRequest(context = decodeContextRequest(arguments))

    fun decodeReloadProjectContextRequest(arguments: List<Any?>): McdevReloadProjectContextRequest =
        McdevReloadProjectContextRequest(context = decodeContextRequest(arguments))

    fun decodeDumpContextRequest(arguments: List<Any?>): McdevDumpContextRequest =
        McdevDumpContextRequest(context = decodeContextRequest(arguments))

    fun decodeCodeActionRequest(arguments: List<Any?>): McdevCodeActionRequest {
        val root = decodeRoot(arguments)
        val rangeObject = root.getAsJsonObjectOrEmpty("range")
        return McdevCodeActionRequest(
            context = decodeContext(root),
            range = if (rangeObject.has("start") && rangeObject.has("end")) {
                decodeRange(rangeObject)
            } else {
                McdevRange(
                    start = McdevPosition(0, 0),
                    end = McdevPosition(0, 0),
                )
            },
            diagnosticCodes = decodeStringList(root, "diagnosticCodes"),
        )
    }

    fun decodeDefinitionRequest(arguments: List<Any?>): McdevDefinitionRequest =
        McdevDefinitionRequest(context = decodeContextRequest(arguments))

    fun decodeReferencesRequest(arguments: List<Any?>): McdevReferencesRequest =
        McdevReferencesRequest(context = decodeContextRequest(arguments))

    fun decodeHoverRequest(arguments: List<Any?>): McdevHoverRequest =
        McdevHoverRequest(context = decodeContextRequest(arguments))

    fun encodeToMap(value: Any): Map<String, Any?> =
        gson.fromJson(gson.toJsonTree(value), Map::class.java) as Map<String, Any?>

    fun <T> decodeFromMap(map: Map<String, Any?>, type: Class<T>): T =
        gson.fromJson(gson.toJson(map), type)

    private fun decodeRoot(arguments: List<Any?>): JsonObject {
        val payload = arguments.firstOrNull()
            ?: throw ProtocolDecodeException("missing command payload")
        return when (payload) {
            is JsonObject -> payload
            is Map<*, *> -> gson.toJsonTree(payload).asJsonObject
            else -> gson.toJsonTree(payload).asJsonObject
        }
    }

    private fun decodeContext(root: JsonObject): McdevRequestContext {
        val contextObject = when {
            root.has("context") -> root.getAsJsonObject("context")
            root.has("workspaceRoot") -> root
            else -> throw ProtocolDecodeException("missing request context")
        }
        val protocolVersion = contextObject.getIntOrDefault("protocolVersion", McdevProtocol.VERSION)
        val workspaceRoot = contextObject.getRequiredString("workspaceRoot")
        val documentUri = contextObject.getRequiredString("documentUri")
        val languageId = contextObject.getStringOrDefault("languageId", "java")
        val directBufferText = contextObject.getNullableString("bufferText")
        val fallbackBufferText = contextObject.getNullableString("bufferTextFallback")
        val bufferText = directBufferText ?: fallbackBufferText.orEmpty()
        val documentVersion = contextObject.getLongOrNull("documentVersion")
            ?: contextObject.getLongOrNull("changedtick")
        val positionObject = contextObject.getAsJsonObject("position")
            ?: throw ProtocolDecodeException("missing cursor position")
        val clientObject = contextObject.getAsJsonObject("client")
        return McdevRequestContext(
            protocolVersion = protocolVersion,
            workspaceRoot = workspaceRoot,
            documentUri = documentUri,
            languageId = languageId,
            position = McdevPosition(
                line = positionObject.getIntOrDefault("line", 0),
                character = positionObject.getIntOrDefault("character", 0),
            ),
            bufferText = bufferText,
            documentVersion = documentVersion,
            bufferTextFallbackUsed = directBufferText == null && fallbackBufferText != null,
            client = McdevClientInfo(
                name = clientObject?.getStringOrDefault("name", "unknown") ?: "unknown",
                version = clientObject?.getStringOrDefault("version", "0.0.0") ?: "0.0.0",
            ),
        )
    }

    private fun JsonObject.getAsJsonObjectOrEmpty(name: String): JsonObject =
        if (has(name) && get(name).isJsonObject) getAsJsonObject(name) else JsonObject()

    private fun JsonObject.getRequiredString(name: String): String =
        getNullableString(name) ?: throw ProtocolDecodeException("missing field: $name")

    private fun JsonObject.getStringOrDefault(name: String, defaultValue: String): String =
        getNullableString(name) ?: defaultValue

    private fun JsonObject.getNullableString(name: String): String? {
        if (!has(name) || get(name).isJsonNull) return null
        val element: JsonElement = get(name)
        return if (element.isJsonPrimitive) element.asString else element.toString()
    }

    private fun JsonObject.getIntOrDefault(name: String, defaultValue: Int): Int {
        if (!has(name) || get(name).isJsonNull) return defaultValue
        return get(name).asInt
    }

    private fun JsonObject.getLongOrNull(name: String): Long? {
        if (!has(name) || get(name).isJsonNull) return null
        return runCatching { get(name).asLong }.getOrNull()
    }

    private fun decodeRange(rangeObject: JsonObject): McdevRange {
        val startObject = rangeObject.getAsJsonObject("start")
            ?: throw ProtocolDecodeException("missing range start")
        val endObject = rangeObject.getAsJsonObject("end")
            ?: throw ProtocolDecodeException("missing range end")
        return McdevRange(
            start = McdevPosition(
                line = startObject.getIntOrDefault("line", 0),
                character = startObject.getIntOrDefault("character", 0),
            ),
            end = McdevPosition(
                line = endObject.getIntOrDefault("line", 0),
                character = endObject.getIntOrDefault("character", 0),
            ),
        )
    }

    private fun decodeStringList(root: JsonObject, name: String): List<String> {
        if (!root.has(name) || !root.get(name).isJsonArray) return emptyList()
        return root.getAsJsonArray(name)
            .mapNotNull { element ->
                if (element.isJsonNull) null else element.asString
            }
    }
}

class ProtocolDecodeException(message: String) : RuntimeException(message)

private fun createGson(): Gson =
    GsonBuilder()
        .registerTypeAdapter(McdevDefinitionResolution::class.java, McdevDefinitionResolutionAdapter())
        .create()

private class McdevDefinitionResolutionAdapter : TypeAdapter<McdevDefinitionResolution>() {
    override fun write(out: JsonWriter, value: McdevDefinitionResolution?) {
        if (value == null) {
            out.nullValue()
            return
        }
        out.value(value.name.lowercase())
    }

    override fun read(reader: JsonReader): McdevDefinitionResolution {
        val raw = reader.nextString()
        return McdevDefinitionResolution.valueOf(raw.uppercase())
    }
}
