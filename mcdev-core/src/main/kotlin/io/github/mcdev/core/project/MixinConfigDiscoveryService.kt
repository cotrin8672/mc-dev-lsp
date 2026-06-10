package io.github.mcdev.core.project

import com.google.gson.JsonObject
import com.google.gson.JsonParser
import java.nio.file.Files
import java.nio.file.Path
import kotlin.io.path.exists
import kotlin.io.path.extension
import kotlin.io.path.name
import kotlin.io.path.readText

object MixinConfigDiscoveryService {
    fun discover(root: Path): List<MixinConfigRef> {
        if (!root.exists()) return emptyList()
        val paths = Files.walk(root).use { stream ->
            stream
                .filter { Files.isRegularFile(it) }
                .filter { isMixinConfigFile(it) }
                .filter { path -> !ProjectPathFilters.isUnderExcludedDirectory(path) }
                .sorted()
                .toList()
        }
        return paths.mapNotNull { path -> parse(path) }.sortedBy { it.path.toString() }
    }

    fun isMixinConfigFile(path: Path): Boolean {
        val name = path.name.lowercase()
        return name == "mixins.json" ||
            name.endsWith(".mixins.json") ||
            (path.extension.lowercase() == "json" && name.contains("mixin"))
    }

    fun parse(path: Path): MixinConfigRef? {
        return try {
            parseContent(path, path.readText())
        } catch (_: Exception) {
            null
        }
    }

    fun parseContent(path: Path, content: String): MixinConfigRef? {
        return try {
            parseJsonContent(path, content)
        } catch (_: Exception) {
            null
        }
    }

    private fun parseJsonContent(path: Path, content: String): MixinConfigRef? {
        val root = JsonParser.parseString(content)
        if (!root.isJsonObject) return null
        val json = root.asJsonObject

        return MixinConfigRef(
            path = path,
            packageName = json.getStringOrNull("package"),
            mixins = json.getStringList("mixins"),
            client = json.getStringList("client"),
            server = json.getStringList("server"),
            common = json.getStringList("common"),
        )
    }

    private fun JsonObject.getStringOrNull(key: String): String? {
        val element = get(key) ?: return null
        return if (element.isJsonPrimitive && element.asJsonPrimitive.isString) {
            element.asString
        } else {
            null
        }
    }

    private fun JsonObject.getStringList(key: String): List<String> {
        val element = get(key) ?: return emptyList()
        if (!element.isJsonArray) return emptyList()
        return element.asJsonArray.mapNotNull { item ->
            if (item.isJsonPrimitive && item.asJsonPrimitive.isString) item.asString else null
        }
    }
}
