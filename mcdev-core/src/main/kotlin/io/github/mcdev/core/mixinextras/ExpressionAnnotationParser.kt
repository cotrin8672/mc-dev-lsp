package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.text.JavaStringContentDecoder

object ExpressionAnnotationParser {
    fun parse(body: String): MixinExtrasExpression? {
        var index = skipForwardWhitespaceAndComments(body, 0)
        if (index >= body.length) return null

        return when {
            body[index] == '"' || body[index] == '{' -> parseValueShorthand(body, index)
            else -> parseNamedAttributes(body, index)
        }
    }

    private fun parseValueShorthand(body: String, start: Int): MixinExtrasExpression? {
        val parsed = parseStringOrStringArrayStrict(body, start) ?: return null
        val end = skipForwardWhitespaceAndComments(body, parsed.end)
        if (end < body.length) return null
        return MixinExtrasExpression(values = parsed.values)
    }

    private fun parseNamedAttributes(body: String, start: Int): MixinExtrasExpression? {
        var index = skipForwardWhitespaceAndComments(body, start)
        var id: String? = null
        var idExplicit = false
        var values: List<String>? = null

        while (index < body.length) {
            index = skipForwardWhitespaceAndComments(body, index)
            if (index >= body.length) break

            val attribute = readIdentifier(body, index) ?: return null
            index += attribute.length
            index = skipForwardWhitespaceAndComments(body, index)
            if (body.getOrNull(index) != '=') return null
            index++
            index = skipForwardWhitespaceAndComments(body, index)

            when (attribute) {
                "id" -> {
                    if (idExplicit) return null
                    val literal = readStringLiteral(body, index) ?: return null
                    id = literal.value
                    idExplicit = true
                    index = literal.end
                }
                "value" -> {
                    if (values != null) return null
                    val parsed = parseStringOrStringArrayStrict(body, index) ?: return null
                    values = parsed.values
                    index = parsed.end
                }
                else -> return null
            }

            index = skipForwardWhitespaceAndComments(body, index)
            if (body.getOrNull(index) == ',') {
                index++
            }
        }

        if (values == null) return null
        index = skipForwardWhitespaceAndComments(body, index)
        if (index < body.length) return null
        return MixinExtrasExpression(id = id ?: "", values = values)
    }

    private fun parseStringOrStringArrayStrict(source: String, start: Int): ParsedValue<String>? {
        val index = skipForwardWhitespaceAndComments(source, start)
        if (index >= source.length) return null
        return when (source[index]) {
            '"' -> readStringLiteral(source, index)?.let { ParsedValue(listOf(it.value), it.end) }
            '{' -> parseStringArrayStrict(source, index)
            else -> null
        }
    }

    private fun parseStringArrayStrict(source: String, openBrace: Int): ParsedValue<String>? {
        val closeBrace = findMatchingCloseBrace(source, openBrace) ?: return null
        val values = mutableListOf<String>()
        for (element in splitTopLevelCommas(source.substring(openBrace + 1, closeBrace))) {
            val trimmedStart = skipForwardWhitespaceAndComments(element, 0)
            if (trimmedStart >= element.length) continue
            val literal = readStringLiteral(element, trimmedStart) ?: return null
            if (skipForwardWhitespaceAndComments(element, literal.end) < element.length) return null
            values += literal.value
        }
        return ParsedValue(values, closeBrace + 1)
    }

    private data class ParsedValue<T>(val values: List<T>, val end: Int)

    private data class StringLiteral(val value: String, val end: Int)

    private fun readStringLiteral(source: String, start: Int): StringLiteral? {
        if (source.getOrNull(start) != '"') return null
        val builder = StringBuilder()
        var index = start + 1
        while (index < source.length) {
            when (val char = source[index]) {
                '\\' -> {
                    if (index + 1 >= source.length) return null
                    when (val escape = source[index + 1]) {
                        'b' -> builder.append('\b')
                        't' -> builder.append('\t')
                        'n' -> builder.append('\n')
                        'f' -> builder.append('\u000C')
                        'r' -> builder.append('\r')
                        '"' -> builder.append('"')
                        '\'' -> builder.append('\'')
                        '\\' -> builder.append('\\')
                        's' -> builder.append(' ')
                        'u' -> {
                            val decoded = JavaStringContentDecoder.decodeUnicodeEscape(source, index, source.length)
                                ?: return null
                            if (decoded.char == '"' || decoded.char == '\\' || decoded.char == '\r' || decoded.char == '\n') {
                                return null
                            }
                            builder.append(decoded.char)
                            index = decoded.nextIndex
                            continue
                        }
                        in '0'..'7' -> {
                            var value = escape - '0'
                            var next = index + 2
                            if (next < source.length && source[next] in '0'..'7') {
                                value = value * 8 + (source[next] - '0')
                                next++
                                if (escape <= '3' && next < source.length && source[next] in '0'..'7') {
                                    value = value * 8 + (source[next] - '0')
                                    next++
                                }
                            }
                            if (value > 0xFFFF) return null
                            builder.append(value.toChar())
                            index = next
                            continue
                        }
                        else -> return null
                    }
                    index += 2
                }
                '"' -> return StringLiteral(builder.toString(), index + 1)
                else -> {
                    builder.append(char)
                    index++
                }
            }
        }
        return null
    }

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
