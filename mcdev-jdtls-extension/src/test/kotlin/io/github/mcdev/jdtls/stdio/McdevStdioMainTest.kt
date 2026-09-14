package io.github.mcdev.jdtls.stdio

import com.google.gson.Gson
import com.google.gson.JsonObject
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.nio.charset.StandardCharsets
import java.util.Collections
import java.util.concurrent.TimeUnit
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class McdevStdioMainTest {
    @Test
    fun shadedJarKeepsLineProtocolAliveAcrossProvisionalAndFallbackRequests() {
        val jar = extensionJar()
        val javaName = if (System.getProperty("os.name").startsWith("Windows")) "java.exe" else "java"
        val java = Paths.get(System.getProperty("java.home"), "bin", javaName)
        val process = ProcessBuilder(
            java.toString(),
            "-cp",
            jar.toString(),
            "io.github.mcdev.jdtls.stdio.McdevStdioMain",
        ).start()

        val gson = Gson()
        val requests = listOf(
            request(id = 1, source = "@Inject(meth", character = 12),
            request(id = 2, source = "@Expression(\"{ ret", character = 18),
            "{malformed",
            request(id = 3, source = "@Inject(meth", character = 12),
            nonEligibleRequest(),
        )
        val responses = Collections.synchronizedList(mutableListOf<String>())
        val reader = Thread {
            process.inputStream.bufferedReader(StandardCharsets.UTF_8).useLines { lines ->
                lines.forEach(responses::add)
            }
        }.apply {
            isDaemon = true
            start()
        }
        try {
            process.outputStream.bufferedWriter(StandardCharsets.UTF_8).use { writer ->
                requests.forEach { line ->
                    writer.append(line).append('\n')
                }
            }
            assertTrue(process.waitFor(20, TimeUnit.SECONDS), "stdio helper did not exit after EOF")
            assertEquals(0, process.exitValue())
        } finally {
            if (process.isAlive) process.destroyForcibly()
            reader.join(1_000)
        }

        assertEquals(requests.size, responses.size)

        val first = gson.fromJson(responses[0], JsonObject::class.java)
        assertEquals(1, first.get("id").asInt)
        assertTrue(first.get("handled").asBoolean)
        val firstItem = first.getAsJsonObject("response")
            .getAsJsonObject("result")
            .getAsJsonArray("items")
            .first()
            .asJsonObject
        assertEquals(8, firstItem.getAsJsonObject("edit").getAsJsonObject("range").getAsJsonObject("start").get("character").asInt)
        assertEquals(12, firstItem.getAsJsonObject("edit").getAsJsonObject("range").getAsJsonObject("end").get("character").asInt)

        val second = gson.fromJson(responses[1], JsonObject::class.java)
        assertEquals(2, second.get("id").asInt)
        assertTrue(second.get("handled").asBoolean)
        val secondItem = second.getAsJsonObject("response")
            .getAsJsonObject("result")
            .getAsJsonArray("items")
            .first()
            .asJsonObject
        assertTrue(secondItem.getAsJsonObject("edit").get("newText").asString.isNotEmpty())

        val malformed = gson.fromJson(responses[2], JsonObject::class.java)
        assertEquals(false, malformed.get("handled").asBoolean)
        assertEquals("PARSE_ERROR", malformed.getAsJsonObject("error").get("code").asString)

        val recovered = gson.fromJson(responses[3], JsonObject::class.java)
        assertEquals(3, recovered.get("id").asInt)
        assertTrue(recovered.get("handled").asBoolean)
        assertTrue(recovered.getAsJsonObject("response").getAsJsonObject("result").getAsJsonArray("items").size() > 0)

        val nonEligible = gson.fromJson(responses[4], JsonObject::class.java)
        assertEquals(4, nonEligible.get("id").asInt)
        assertEquals(false, nonEligible.get("handled").asBoolean)
    }

    private fun request(id: Int, source: String, character: Int): String =
        Gson().toJson(
            mapOf(
                "id" to id,
                "payload" to mapOf(
                    "context" to mapOf(
                        "protocolVersion" to 1,
                        "workspaceRoot" to "file:///workspace",
                        "documentUri" to "file:///workspace/Mixin.java",
                        "languageId" to "java",
                        "position" to mapOf("line" to 0, "character" to character),
                        "bufferText" to source,
                        "documentVersion" to 1,
                        "client" to mapOf("name" to "stdio-test", "version" to "1"),
                    ),
                    "trigger" to mapOf("kind" to "manual"),
                    "options" to mapOf(
                        "preferredAtTarget" to "smart",
                        "mixinClassInsert" to "import",
                        "injectMethodDescriptor" to "auto",
                    ),
                ),
            ),
        )

    private fun nonEligibleRequest(): String {
        val source = "@Inject(method = \"tick\", at = @At(value = \"INVOKE\", target = \"\"))"
        return request(
            id = 4,
            source = source,
            character = source.indexOf("target = \"") + "target = \"".length,
        )
    }

    private fun extensionJar(): Path {
        val roots = listOf(
            Paths.get("build/libs"),
            Paths.get("mcdev-jdtls-extension/build/libs"),
        )
        return roots.asSequence()
            .filter(Files::isDirectory)
            .flatMap { root ->
                Files.list(root).use { paths -> paths.toList().asSequence() }
            }
            .filter { path ->
                val name = path.fileName.toString()
                name.startsWith("io.github.mcdev.jdtls-") &&
                    name.endsWith(".jar") &&
                    !name.endsWith("-plain.jar") &&
                    !name.endsWith("-sources.jar")
            }
            .maxByOrNull { Files.getLastModifiedTime(it) }
            ?: error("built mcdev JDT LS jar not found")
    }
}
