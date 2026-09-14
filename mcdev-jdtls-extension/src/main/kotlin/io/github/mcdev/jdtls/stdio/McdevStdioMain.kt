package io.github.mcdev.jdtls.stdio

import com.google.gson.Gson
import com.google.gson.JsonElement
import com.google.gson.JsonObject
import com.google.gson.JsonParseException
import io.github.mcdev.jdtls.handler.McdevCompletionHandler
import io.github.mcdev.jdtls.protocol.ProtocolDecodeException
import io.github.mcdev.jdtls.protocol.ProtocolPayloadDecoder
import java.io.BufferedReader
import java.io.BufferedWriter
import java.io.InputStreamReader
import java.io.OutputStreamWriter
import java.io.Reader
import java.io.Writer
import java.nio.charset.StandardCharsets

/** Small no-index completion transport used alongside the JDT LS command path. */
object McdevStdioMain {
    @JvmStatic
    fun main(args: Array<String>) {
        run(
            input = InputStreamReader(System.`in`, StandardCharsets.UTF_8).buffered(),
            output = OutputStreamWriter(System.out, StandardCharsets.UTF_8).buffered(),
        )
    }

    internal fun run(input: Reader, output: Writer) {
        val reader = input as? BufferedReader ?: input.buffered()
        val writer = output as? BufferedWriter ?: output.buffered()
        val gson = Gson()
        val decoder = ProtocolPayloadDecoder()
        val handler = McdevCompletionHandler()
        while (true) {
            val line = reader.readLine() ?: break
            val response = processLine(line, gson, decoder, handler)
            writer.append(gson.toJson(response)).append('\n')
            writer.flush()
        }
    }

    private fun processLine(
        line: String,
        gson: Gson,
        decoder: ProtocolPayloadDecoder,
        handler: McdevCompletionHandler,
    ): StdioResponse {
        var id: JsonElement? = null
        return try {
            val root = gson.fromJson(line, JsonObject::class.java)
            id = root?.get("id")?.takeUnless { it.isJsonNull }
            val payload = root?.get("payload")
                ?.takeIf { it.isJsonObject }
                ?: return parseFailure(id)
            @Suppress("UNCHECKED_CAST")
            val payloadMap = gson.fromJson(payload, Map::class.java) as Map<String, Any?>
            val request = decoder.decodeCompletionRequest(listOf(payloadMap))
            val envelope = handler.handleBufferOnly(request)
            if (envelope?.result?.items?.isNotEmpty() == true) {
                StdioResponse(
                    id = id,
                    handled = true,
                    response = decoder.encodeToMap(envelope),
                )
            } else {
                StdioResponse(id = id, handled = false)
            }
        } catch (_: JsonParseException) {
            parseFailure(id)
        } catch (_: ProtocolDecodeException) {
            parseFailure(id)
        }
    }

    private fun parseFailure(id: JsonElement?): StdioResponse =
        StdioResponse(
            id = id,
            handled = false,
            error = StdioError(code = "PARSE_ERROR", message = "invalid helper request"),
        )

    private data class StdioResponse(
        val id: JsonElement?,
        val handled: Boolean,
        val response: Map<String, Any?>? = null,
        val error: StdioError? = null,
    )

    private data class StdioError(
        val code: String,
        val message: String,
    )
}
