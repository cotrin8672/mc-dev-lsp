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
        val extension = path.extension.lowercase()
        return name == "mixins.json" ||
            name == "mixins.json5" ||
            name.endsWith(".mixins.json") ||
            name.endsWith(".mixins.json5") ||
            (extension == "json" && name.contains("mixin")) ||
            (extension == "json5" && name.contains("mixin"))
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

    fun normalizeJsonContent(content: String): String = removeTrailingCommas(stripJson5Comments(content))

    fun readContent(path: Path): String? =
        runCatching { normalizeJsonContent(path.readText()) }.getOrNull()

    fun selectForMixin(
        configs: List<MixinConfigRef>,
        mixinClassName: String?,
        mixinPackage: String?,
    ): MixinConfigRef? {
        if (configs.isEmpty()) return null
        val sorted = configs.sortedBy { it.path.toString() }

        if (mixinClassName != null) {
            val listingConfigs = sorted.filter { it.listsMixinClass(mixinClassName) }
            if (listingConfigs.isNotEmpty()) {
                return listingConfigs
                    .sortedWith(
                        compareBy<MixinConfigRef> { !it.packageMatches(mixinPackage) }
                            .thenBy { it.path.toString() },
                    )
                    .first()
            }
        }

        if (mixinPackage != null) {
            val packageConfigs = sorted.filter { it.packageName == mixinPackage }
            if (packageConfigs.isNotEmpty()) {
                return packageConfigs.first()
            }
        }

        return sorted.first()
    }

    private fun parseJsonContent(path: Path, content: String): MixinConfigRef? {
        val root = JsonParser.parseString(normalizeJsonContent(content))
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

    private fun MixinConfigRef.listsMixinClass(className: String): Boolean =
        className in mixins || className in client || className in server || className in common

    private fun MixinConfigRef.packageMatches(packageName: String?): Boolean =
        packageName != null && this.packageName == packageName

    private fun stripJson5Comments(source: String): String {
        val out = StringBuilder()
        var index = 0
        var inString = false
        var stringDelimiter = '"'
        while (index < source.length) {
            val char = source[index]
            if (inString) {
                out.append(char)
                if (char == '\\' && index + 1 < source.length) {
                    out.append(source[index + 1])
                    index += 2
                    continue
                }
                if (char == stringDelimiter) {
                    inString = false
                }
                index++
                continue
            }
            when (char) {
                '"', '\'' -> {
                    inString = true
                    stringDelimiter = char
                    out.append(char)
                    index++
                }
                '/' -> if (index + 1 < source.length) {
                    when (source[index + 1]) {
                        '/' -> {
                            index += 2
                            while (index < source.length && source[index] != '\n') {
                                index++
                            }
                        }
                        '*' -> {
                            index += 2
                            while (index + 1 < source.length && !(source[index] == '*' && source[index + 1] == '/')) {
                                index++
                            }
                            index += 2
                        }
                        else -> {
                            out.append(char)
                            index++
                        }
                    }
                } else {
                    out.append(char)
                    index++
                }
                else -> {
                    out.append(char)
                    index++
                }
            }
        }
        return out.toString()
    }

    private fun removeTrailingCommas(json: String): String {
        val out = StringBuilder()
        var index = 0
        var inString = false
        var stringDelimiter = '"'
        while (index < json.length) {
            val char = json[index]
            if (inString) {
                out.append(char)
                if (char == '\\' && index + 1 < json.length) {
                    out.append(json[index + 1])
                    index += 2
                    continue
                }
                if (char == stringDelimiter) {
                    inString = false
                }
                index++
                continue
            }
            if (char == '"' || char == '\'') {
                inString = true
                stringDelimiter = char
                out.append(char)
                index++
                continue
            }
            if (char == ',') {
                var lookahead = index + 1
                while (lookahead < json.length && json[lookahead].isWhitespace()) {
                    lookahead++
                }
                if (lookahead < json.length && (json[lookahead] == '}' || json[lookahead] == ']')) {
                    index++
                    continue
                }
            }
            out.append(char)
            index++
        }
        return out.toString()
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
