package io.github.mcdev.core.mixinextras

object DefinitionsAnnotationParser {
    /**
     * @param parsedTextBaseOffset absolute file offset of [body]'s index 0.
     */
    fun parse(body: String, parsedTextBaseOffset: Int = 0): MixinExtrasDefinitionIndex {
        var index = skipForwardWhitespaceAndComments(body, 0)
        if (index >= body.length) {
            return MixinExtrasDefinitionIndex()
        }

        return when {
            body[index] == '@' || body[index] == '{' -> {
                val parsed = parseDefinitionOrDefinitionArray(body, index, parsedTextBaseOffset)
                    ?: return MixinExtrasDefinitionIndex()
                if (skipForwardWhitespaceAndComments(body, parsed.end) < body.length) {
                    return MixinExtrasDefinitionIndex()
                }
                MixinExtrasDefinitionIndex(parsed.values)
            }
            else -> parseNamedAttributes(body, index, parsedTextBaseOffset)
        }
    }

    fun parseDefinitionAt(
        source: String,
        definitionAtOffset: Int,
        parsedTextBaseOffset: Int,
    ): MixinExtrasDefinition? =
        readDefinitionAnnotation(source, definitionAtOffset, parsedTextBaseOffset)?.value

    private fun parseNamedAttributes(body: String, start: Int, parsedTextBaseOffset: Int): MixinExtrasDefinitionIndex {
        var index = skipForwardWhitespaceAndComments(body, start)
        if (index >= body.length) {
            return MixinExtrasDefinitionIndex()
        }

        val attribute = readIdentifier(body, index) ?: return MixinExtrasDefinitionIndex()
        if (attribute != "value") {
            return MixinExtrasDefinitionIndex()
        }
        index += attribute.length

        index = skipForwardWhitespaceAndComments(body, index)
        if (body.getOrNull(index) != '=') {
            return MixinExtrasDefinitionIndex()
        }
        index++

        index = skipForwardWhitespaceAndComments(body, index)
        val parsed = parseDefinitionOrDefinitionArray(body, index, parsedTextBaseOffset)
            ?: return MixinExtrasDefinitionIndex()
        index = skipForwardWhitespaceAndComments(body, parsed.end)
        if (index < body.length) {
            return MixinExtrasDefinitionIndex()
        }

        return MixinExtrasDefinitionIndex(parsed.values)
    }

    private fun parseDefinitionOrDefinitionArray(
        source: String,
        start: Int,
        parsedTextBaseOffset: Int,
    ): ParsedValue<MixinExtrasDefinition>? {
        val index = skipForwardWhitespaceAndComments(source, start)
        if (index >= source.length) return null
        return when (source[index]) {
            '@' -> readDefinitionAnnotation(source, index, parsedTextBaseOffset)?.let { ParsedValue(listOf(it.value), it.end) }
            '{' -> parseDefinitionArray(source, index, parsedTextBaseOffset)
            else -> null
        }
    }

    private fun parseDefinitionArray(
        source: String,
        openBrace: Int,
        parsedTextBaseOffset: Int,
    ): ParsedValue<MixinExtrasDefinition>? {
        val closeBrace = findMatchingCloseBrace(source, openBrace) ?: return null
        val values = mutableListOf<MixinExtrasDefinition>()
        var index = openBrace + 1
        while (index < closeBrace) {
            index = skipForwardWhitespaceAndComments(source, index)
            if (index >= closeBrace) break
            if (source.getOrNull(index) != '@') {
                index++
                continue
            }
            val definition = readDefinitionAnnotation(source, index, parsedTextBaseOffset)
            if (definition == null) {
                index++
                continue
            }
            val afterDefinition = skipForwardWhitespaceAndComments(source, definition.end)
            if (afterDefinition >= closeBrace || source.getOrNull(afterDefinition) == ',') {
                values += definition.value
                index = if (source.getOrNull(afterDefinition) == ',') afterDefinition + 1 else afterDefinition
            } else {
                index = skipToNextTopLevelComma(source, definition.end, closeBrace)
            }
        }
        return ParsedValue(values, closeBrace + 1)
    }

    private fun skipToNextTopLevelComma(source: String, start: Int, closeBrace: Int): Int {
        var index = start
        var parenDepth = 0
        var braceDepth = 0
        var inString = false
        while (index < closeBrace) {
            when {
                inString -> {
                    if (source[index] == '\\' && index + 1 < source.length) {
                        index += 2
                        continue
                    }
                    if (source[index] == '"') inString = false
                    index++
                }
                isLineCommentStart(source, index) -> index = skipLineComment(source, index)
                isBlockCommentStart(source, index) -> index = skipBlockComment(source, index)
                source[index] == '"' -> {
                    inString = true
                    index++
                }
                source[index] == '(' -> {
                    parenDepth++
                    index++
                }
                source[index] == ')' -> {
                    parenDepth = (parenDepth - 1).coerceAtLeast(0)
                    index++
                }
                source[index] == '{' -> {
                    braceDepth++
                    index++
                }
                source[index] == '}' -> {
                    braceDepth = (braceDepth - 1).coerceAtLeast(0)
                    index++
                }
                source[index] == ',' && parenDepth == 0 && braceDepth == 0 -> return index + 1
                else -> index++
            }
        }
        return closeBrace
    }

    private fun readDefinitionAnnotation(
        source: String,
        start: Int,
        parsedTextBaseOffset: Int,
    ): ParsedScalar<MixinExtrasDefinition>? {
        if (source.getOrNull(start) != '@') return null
        var index = start + 1
        val nameStart = index
        while (index < source.length && (source[index].isLetterOrDigit() || source[index] in "._$")) {
            index++
        }
        if (index == nameStart) return null
        val qualifiedName = source.substring(nameStart, index)
        if (!isDefinitionAnnotationName(qualifiedName)) return null

        index = skipForwardWhitespaceAndComments(source, index)
        if (source.getOrNull(index) != '(') return null
        val closeParen = findMatchingCloseParen(source, index) ?: return null
        val bodyStart = index + 1
        val definitionBody = source.substring(bodyStart, closeParen)
        val definition = DefinitionAnnotationParser.parse(definitionBody)
        val end = skipForwardWhitespaceAndComments(source, closeParen + 1)
        return ParsedScalar(
            definition.withIdSourceRange(parsedTextBaseOffset + bodyStart),
            end,
        )
    }

    private fun isDefinitionAnnotationName(qualifiedName: String): Boolean =
        qualifiedName == "Definition" ||
            qualifiedName == "com.llamalad7.mixinextras.expression.Definition"

    private data class ParsedValue<T>(val values: List<T>, val end: Int)

    private data class ParsedScalar<T>(val value: T, val end: Int)

    private data class StringLiteral(val value: String, val end: Int)

    private fun readIdentifier(source: String, start: Int): String? {
        if (start >= source.length || !source[start].isJavaIdentifierStart()) return null
        var index = start + 1
        while (index < source.length && source[index].isJavaIdentifierPart()) {
            index++
        }
        return source.substring(start, index)
    }

    private fun splitTopLevelCommas(value: String): List<String> {
        val results = mutableListOf<String>()
        var start = 0
        var genericDepth = 0
        var parenDepth = 0
        var braceDepth = 0
        var inString = false
        var index = 0
        while (index < value.length) {
            when {
                inString -> {
                    if (value[index] == '\\' && index + 1 < value.length) {
                        index += 2
                        continue
                    }
                    if (value[index] == '"') inString = false
                }
                isLineCommentStart(value, index) -> {
                    index = skipLineComment(value, index)
                    continue
                }
                isBlockCommentStart(value, index) -> {
                    index = skipBlockComment(value, index)
                    continue
                }
                value[index] == '"' -> inString = true
                value[index] == '<' -> genericDepth++
                value[index] == '>' -> genericDepth = (genericDepth - 1).coerceAtLeast(0)
                value[index] == '(' -> parenDepth++
                value[index] == ')' -> parenDepth = (parenDepth - 1).coerceAtLeast(0)
                value[index] == '{' -> braceDepth++
                value[index] == '}' -> braceDepth = (braceDepth - 1).coerceAtLeast(0)
                value[index] == ',' && genericDepth == 0 && parenDepth == 0 && braceDepth == 0 && !inString -> {
                    results += value.substring(start, index)
                    start = index + 1
                }
            }
            index++
        }
        results += value.substring(start)
        return results
    }

    private fun skipForwardWhitespaceAndComments(source: String, start: Int): Int {
        var index = start
        while (index < source.length) {
            when {
                source[index].isWhitespace() -> index++
                isLineCommentStart(source, index) -> index = skipLineComment(source, index)
                isBlockCommentStart(source, index) -> index = skipBlockComment(source, index)
                else -> break
            }
        }
        return index
    }

    private fun isLineCommentStart(source: String, index: Int): Boolean =
        index + 1 < source.length && source[index] == '/' && source[index + 1] == '/'

    private fun isBlockCommentStart(source: String, index: Int): Boolean =
        index + 1 < source.length && source[index] == '/' && source[index + 1] == '*'

    private fun skipLineComment(source: String, index: Int): Int {
        var current = index + 2
        while (current < source.length && source[current] != '\n') {
            current++
        }
        return current
    }

    private fun skipBlockComment(source: String, index: Int): Int {
        var current = index + 2
        while (current + 1 < source.length) {
            if (source[current] == '*' && source[current + 1] == '/') {
                return current + 2
            }
            current++
        }
        return source.length
    }

    private fun readStringLiteral(source: String, start: Int): StringLiteral? {
        if (source.getOrNull(start) != '"') return null
        val builder = StringBuilder()
        var index = start + 1
        while (index < source.length) {
            when {
                source[index] == '\\' -> {
                    if (index + 1 >= source.length) return null
                    when (source[index + 1]) {
                        '\\' -> builder.append('\\')
                        '"' -> builder.append('"')
                        else -> {
                            builder.append('\\')
                            builder.append(source[index + 1])
                        }
                    }
                    index += 2
                }
                source[index] == '"' -> return StringLiteral(builder.toString(), index + 1)
                else -> {
                    builder.append(source[index])
                    index++
                }
            }
        }
        return null
    }

    private fun findMatchingCloseParen(source: String, openIndex: Int): Int? {
        if (source.getOrNull(openIndex) != '(') return null
        var depth = 0
        var index = openIndex
        while (index < source.length) {
            when {
                isLineCommentStart(source, index) -> index = skipLineComment(source, index)
                isBlockCommentStart(source, index) -> index = skipBlockComment(source, index)
                source[index] == '"' -> {
                    val literal = readStringLiteral(source, index) ?: return null
                    index = literal.end
                }
                source[index] == '(' -> {
                    depth++
                    index++
                }
                source[index] == ')' -> {
                    depth--
                    if (depth == 0) return index
                    index++
                }
                else -> index++
            }
        }
        return null
    }

    private fun findMatchingCloseBrace(source: String, openIndex: Int): Int? {
        if (source.getOrNull(openIndex) != '{') return null
        var depth = 0
        var index = openIndex
        while (index < source.length) {
            when {
                isLineCommentStart(source, index) -> index = skipLineComment(source, index)
                isBlockCommentStart(source, index) -> index = skipBlockComment(source, index)
                source[index] == '"' -> {
                    val literal = readStringLiteral(source, index) ?: return null
                    index = literal.end
                }
                source[index] == '{' -> {
                    depth++
                    index++
                }
                source[index] == '}' -> {
                    depth--
                    if (depth == 0) return index
                    index++
                }
                else -> index++
            }
        }
        return null
    }

    private fun Char.isJavaIdentifierStart(): Boolean = isLetter() || this == '_' || this == '$'

    private fun Char.isJavaIdentifierPart(): Boolean = isLetterOrDigit() || this == '_' || this == '$'
}
