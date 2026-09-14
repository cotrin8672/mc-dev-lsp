package io.github.mcdev.core.mixin

import com.google.gson.Gson
import com.google.gson.GsonBuilder
import com.google.gson.JsonArray
import com.google.gson.JsonElement
import com.google.gson.JsonObject
import com.google.gson.JsonParser
import io.github.mcdev.core.project.MixinConfigDiscoveryService

data class MixinConfigEntry(
    val path: String,
    val packageName: String?,
    val mixins: List<String>,
    val client: List<String>,
    val server: List<String>,
    val common: List<String> = emptyList(),
)

data class MixinConfigEditDelta(
    val startOffset: Int,
    val endOffset: Int,
    val newText: String,
)

data class MixinConfigEditResult(
    val content: String,
    val added: Boolean,
    val arrayName: String,
    val delta: MixinConfigEditDelta? = null,
)

class MixinConfigEditor {
    private val gson: Gson = GsonBuilder().setPrettyPrinting().create()

    fun parse(content: String, path: String = ""): MixinConfigEntry {
        val root = JsonParser.parseString(MixinConfigDiscoveryService.normalizeJsonContent(content))
        return when {
            root.isJsonObject -> parseObject(root.asJsonObject, path)
            root.isJsonArray -> MixinConfigEntry(
                path,
                null,
                root.asJsonArray.mapNotNull { it.asStringOrNull() },
                emptyList(),
                emptyList(),
                emptyList(),
            )
            else -> MixinConfigEntry(path, null, emptyList(), emptyList(), emptyList(), emptyList())
        }
    }

    fun containsEntry(content: String, mixinClassName: String): Boolean {
        val config = parse(content)
        return config.mixins.contains(mixinClassName) ||
            config.client.contains(mixinClassName) ||
            config.server.contains(mixinClassName) ||
            config.common.contains(mixinClassName)
    }

    fun addEntry(
        content: String,
        mixinClassName: String,
        arrayName: String = "mixins",
    ): MixinConfigEditResult {
        tryLosslessAppend(content, mixinClassName, arrayName)?.let { return it }

        val rootElement = JsonParser.parseString(MixinConfigDiscoveryService.normalizeJsonContent(content))
        val root = when {
            rootElement.isJsonObject -> rootElement.asJsonObject
            rootElement.isJsonArray -> {
                val obj = JsonObject()
                obj.add("mixins", rootElement.asJsonArray.deepCopy())
                obj
            }
            else -> JsonObject()
        }
        val array = root.getAsJsonArray(arrayName) ?: JsonArray().also { root.add(arrayName, it) }
        val existing = array.mapNotNull { it.asStringOrNull() }
        if (mixinClassName in existing) {
            return MixinConfigEditResult(content, added = false, arrayName = arrayName)
        }
        val updated = JsonArray()
        (existing + mixinClassName).sorted().forEach { updated.add(it) }
        root.add(arrayName, updated)
        val rendered = gson.toJson(root) + "\n"
        return MixinConfigEditResult(rendered, added = true, arrayName = arrayName)
    }

    fun listMixinClasses(content: String): List<String> {
        val config = parse(content)
        return (config.mixins + config.client + config.server + config.common).distinct().sorted()
    }

    private fun tryLosslessAppend(
        content: String,
        mixinClassName: String,
        arrayName: String,
    ): MixinConfigEditResult? {
        if (arrayName !in LOSSLESS_ARRAY_NAMES) return null
        val scanner = JsonTextScanner(content)
        val escapedEntry = gson.toJson(mixinClassName)
        return when (val lookup = scanner.lookupTopLevelArray(arrayName)) {
            is TopLevelArrayLookup.Found -> {
                val scanned = scanner.scanStringArrayElements(lookup.arrayStart) ?: return null
                if (mixinClassName in scanned.entries) {
                    return MixinConfigEditResult(content, added = false, arrayName = arrayName)
                }
                val insertion = scanner.buildArrayInsertion(lookup.arrayStart, scanned, escapedEntry)
                losslessResult(insertion, arrayName)
            }
            is TopLevelArrayLookup.Missing -> {
                val insertion = scanner.buildMissingArrayInsertion(lookup, arrayName, escapedEntry)
                losslessResult(insertion, arrayName)
            }
            null -> null
        }
    }

    private fun losslessResult(insertion: ArrayInsertion, arrayName: String): MixinConfigEditResult {
        val delta = MixinConfigEditDelta(
            startOffset = insertion.startOffset,
            endOffset = insertion.startOffset,
            newText = insertion.newText,
        )
        return MixinConfigEditResult(insertion.content, added = true, arrayName = arrayName, delta = delta)
    }

    private fun parseObject(obj: JsonObject, path: String): MixinConfigEntry {
        val packageName = obj.get("package")?.asStringOrNull()
        return MixinConfigEntry(
            path = path,
            packageName = packageName,
            mixins = readArray(obj, "mixins"),
            client = readArray(obj, "client"),
            server = readArray(obj, "server"),
            common = readArray(obj, "common"),
        )
    }

    private fun readArray(obj: JsonObject, key: String): List<String> =
        obj.getAsJsonArray(key)?.mapNotNull { it.asStringOrNull() } ?: emptyList()

    private fun JsonElement.asStringOrNull(): String? = if (isJsonPrimitive && asJsonPrimitive.isString) asString else null

    private companion object {
        val LOSSLESS_ARRAY_NAMES = setOf("mixins", "client", "server")
    }
}

private sealed class TopLevelArrayLookup {
    data class Found(val arrayStart: Int) : TopLevelArrayLookup()

    data class Missing(
        val insertOffset: Int,
        val propertyIndent: String,
        val entryIndent: String,
        val lineEnding: String,
        val needsLeadingComma: Boolean,
    ) : TopLevelArrayLookup()
}

private class JsonTextScanner(private val source: String) {
    private var index = 0

    fun lookupTopLevelArray(arrayName: String): TopLevelArrayLookup? {
        index = 0
        skipWhitespace()
        if (index >= source.length || source[index] != '{') return null
        val openBrace = index
        index++
        var propertyIndent: String? = null
        val lineEnding = detectLineEnding()
        var hasProperties = false
        while (index < source.length) {
            skipWhitespaceAndComments()
            if (index >= source.length) return null
            if (source[index] == '}') {
                val closeBrace = index
                val indent = propertyIndent ?: inferIndentAfter(openBrace) ?: DEFAULT_PROPERTY_INDENT
                return TopLevelArrayLookup.Missing(
                    insertOffset = whitespaceStartBefore(closeBrace),
                    propertyIndent = indent,
                    entryIndent = indent + DEFAULT_PROPERTY_INDENT,
                    lineEnding = lineEnding,
                    needsLeadingComma = hasProperties && !trailingCommaBefore(closeBrace, openBrace),
                )
            }
            val keyStart = index
            val key = readStringLiteral() ?: return null
            if (propertyIndent == null) {
                propertyIndent = indentBefore(keyStart)
            }
            skipWhitespaceAndComments()
            if (index >= source.length || source[index] != ':') return null
            index++
            skipWhitespaceAndComments()
            if (key == arrayName) {
                if (index < source.length && source[index] == '[') {
                    return TopLevelArrayLookup.Found(index)
                }
                return null
            }
            hasProperties = true
            skipValue()
            skipWhitespaceAndComments()
            if (index < source.length && source[index] == ',') {
                index++
            }
        }
        return null
    }

    fun scanStringArrayElements(arrayStart: Int): ScannedStringArray? {
        index = arrayStart + 1
        val entries = mutableListOf<String>()
        var hasValues = false
        skipWhitespaceAndComments()
        if (index < source.length && source[index] == ']') {
            return ScannedStringArray(entries, index, trailingComma = false, hasValues = false)
        }
        while (index < source.length) {
            skipWhitespaceAndComments()
            if (index < source.length && source[index] == ']') {
                return ScannedStringArray(
                    entries,
                    index,
                    trailingCommaBefore(index, arrayStart),
                    hasValues,
                )
            }
            if (index < source.length && source[index] == ',') {
                index++
                continue
            }
            if (index < source.length && (source[index] == '"' || source[index] == '\'')) {
                val value = readStringLiteral() ?: return null
                entries.add(value)
                hasValues = true
            } else {
                val before = index
                skipValue()
                if (index == before) return null
                hasValues = true
            }
            skipWhitespaceAndComments()
            if (index < source.length && source[index] == ',') {
                index++
            }
        }
        return null
    }

    fun buildMissingArrayInsertion(
        missing: TopLevelArrayLookup.Missing,
        arrayName: String,
        escapedEntry: String,
    ): ArrayInsertion {
        val newText = buildString {
            if (missing.needsLeadingComma) append(',')
            append(missing.lineEnding)
            append(missing.propertyIndent)
            append('"')
            append(arrayName)
            append("\": [")
            append(missing.lineEnding)
            append(missing.entryIndent)
            append(escapedEntry)
            append(missing.lineEnding)
            append(missing.propertyIndent)
            append(']')
            if (source.substring(missing.insertOffset).firstOrNull() == '}') {
                append(missing.lineEnding)
            }
        }
        val startOffset = missing.insertOffset
        return ArrayInsertion(
            content = source.substring(0, startOffset) + newText + source.substring(startOffset),
            startOffset = startOffset,
            newText = newText,
        )
    }

    fun buildArrayInsertion(arrayStart: Int, scanned: ScannedStringArray, escapedEntry: String): ArrayInsertion {
        if (!scanned.hasValues) {
            val startOffset = arrayStart + 1
            return ArrayInsertion(
                content = source.substring(0, startOffset) + escapedEntry + source.substring(startOffset),
                startOffset = startOffset,
                newText = escapedEntry,
            )
        }
        val newText = if (scanned.trailingComma) "$escapedEntry," else ", $escapedEntry"
        val startOffset = scanned.closeBracket
        return ArrayInsertion(
            content = source.substring(0, startOffset) + newText + source.substring(startOffset),
            startOffset = startOffset,
            newText = newText,
        )
    }

    private fun skipValue() {
        skipWhitespaceAndComments()
        if (index >= source.length) return
        when (source[index]) {
            '{' -> skipObject()
            '[' -> skipArray()
            '"', '\'' -> readStringLiteral()
            else -> {
                while (index < source.length && !isValueTerminator(source[index])) {
                    index++
                }
            }
        }
    }

    private fun skipObject() {
        if (index >= source.length || source[index] != '{') return
        index++
        while (index < source.length) {
            skipWhitespaceAndComments()
            if (index < source.length && source[index] == '}') {
                index++
                return
            }
            readStringLiteral()
            skipWhitespaceAndComments()
            if (index < source.length && source[index] == ':') {
                index++
            }
            skipValue()
            skipWhitespaceAndComments()
            if (index < source.length && source[index] == ',') {
                index++
            }
        }
    }

    private fun skipArray() {
        if (index >= source.length || source[index] != '[') return
        index++
        while (index < source.length) {
            skipWhitespaceAndComments()
            if (index < source.length && source[index] == ']') {
                index++
                return
            }
            skipValue()
            skipWhitespaceAndComments()
            if (index < source.length && source[index] == ',') {
                index++
            }
        }
    }

    private fun readStringLiteral(): String? {
        skipWhitespaceAndComments()
        if (index >= source.length) return null
        val delimiter = source[index]
        if (delimiter != '"' && delimiter != '\'') return null
        index++
        val out = StringBuilder()
        while (index < source.length) {
            val char = source[index]
            if (char == '\\' && index + 1 < source.length) {
                when (val escape = source[index + 1]) {
                    'u' -> {
                        if (index + 5 >= source.length) return null
                        val hex = source.substring(index + 2, index + 6)
                        if (!hex.all { it.isDigit() || it.lowercaseChar() in 'a'..'f' }) return null
                        out.append(hex.toInt(16).toChar())
                        index += 6
                        continue
                    }
                    else -> {
                        out.append(unescape(escape))
                        index += 2
                        continue
                    }
                }
            }
            if (char == delimiter) {
                index++
                return out.toString()
            }
            out.append(char)
            index++
        }
        return null
    }

    private fun unescape(char: Char): Char = when (char) {
        'b' -> '\b'
        'f' -> '\u000C'
        'n' -> '\n'
        'r' -> '\r'
        't' -> '\t'
        else -> char
    }

    private fun skipWhitespaceAndComments() {
        while (index < source.length) {
            when {
                source[index].isWhitespace() -> index++
                source[index] == '/' && index + 1 < source.length -> when (source[index + 1]) {
                    '/' -> {
                        index += 2
                        while (index < source.length && source[index] != '\n' && source[index] != '\r') {
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
                    else -> return
                }
                else -> return
            }
        }
    }

    private fun skipWhitespace() {
        while (index < source.length && source[index].isWhitespace()) {
            index++
        }
    }

    private fun trailingCommaBefore(closeBracket: Int, arrayStart: Int): Boolean {
        var pos = closeBracket - 1
        while (pos > arrayStart) {
            when {
                source[pos].isWhitespace() -> pos--
                source[pos] == '/' && pos + 1 < source.length && source[pos + 1] == '/' -> {
                    pos--
                    while (pos > arrayStart && source[pos] != '\n' && source[pos] != '\r') {
                        pos--
                    }
                }
                source[pos] == '/' && pos > 0 && source[pos - 1] == '*' -> {
                    pos -= 2
                    while (pos > arrayStart) {
                        if (source[pos] == '/' && pos + 1 < source.length && source[pos + 1] == '*') {
                            pos--
                            break
                        }
                        pos--
                    }
                }
                source[pos] == ',' -> return true
                else -> return false
            }
        }
        return false
    }

    private fun isValueTerminator(char: Char): Boolean =
        char == ',' || char == '}' || char == ']' || char.isWhitespace()

    private fun detectLineEnding(): String =
        if (source.contains("\r\n")) "\r\n" else "\n"

    private fun indentBefore(quoteIndex: Int): String {
        var pos = quoteIndex - 1
        while (pos >= 0 && source[pos].isWhitespace() && source[pos] != '\n' && source[pos] != '\r') {
            pos--
        }
        return source.substring(pos + 1, quoteIndex)
    }

    private fun inferIndentAfter(openBrace: Int): String? {
        index = openBrace + 1
        skipWhitespaceAndComments()
        if (index < source.length && source[index] == '"') {
            return indentBefore(index)
        }
        return null
    }

    private fun whitespaceStartBefore(closeIndex: Int): Int {
        var pos = closeIndex - 1
        while (pos >= 0 && source[pos].isWhitespace()) {
            pos--
        }
        return pos + 1
    }

    private companion object {
        const val DEFAULT_PROPERTY_INDENT = "  "
    }
}

private data class ScannedStringArray(
    val entries: List<String>,
    val closeBracket: Int,
    val trailingComma: Boolean,
    val hasValues: Boolean,
)

private data class ArrayInsertion(
    val content: String,
    val startOffset: Int,
    val newText: String,
)
