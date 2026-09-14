package io.github.mcdev.core.mixinextras

object ExpressionsAnnotationParser {
    fun parse(body: String): MixinExtrasExpressionIndex {
        var index = skipForwardWhitespaceAndComments(body, 0)
        if (index >= body.length) {
            return MixinExtrasExpressionIndex()
        }

        return when {
            body[index] == '@' || body[index] == '{' -> {
                val parsed = parseExpressionOrExpressionArray(body, index)
                    ?: return MixinExtrasExpressionIndex()
                if (skipForwardWhitespaceAndComments(body, parsed.end) < body.length) {
                    return MixinExtrasExpressionIndex()
                }
                MixinExtrasExpressionIndex(parsed.values)
            }
            else -> parseNamedAttributes(body, index)
        }
    }

    private fun parseNamedAttributes(body: String, start: Int): MixinExtrasExpressionIndex {
        var index = skipForwardWhitespaceAndComments(body, start)
        if (index >= body.length) {
            return MixinExtrasExpressionIndex()
        }

        val attribute = readIdentifier(body, index) ?: return MixinExtrasExpressionIndex()
        if (attribute != "value") {
            return MixinExtrasExpressionIndex()
        }
        index += attribute.length

        index = skipForwardWhitespaceAndComments(body, index)
        if (body.getOrNull(index) != '=') {
            return MixinExtrasExpressionIndex()
        }
        index++

        index = skipForwardWhitespaceAndComments(body, index)
        val parsed = parseExpressionOrExpressionArray(body, index)
            ?: return MixinExtrasExpressionIndex()
        index = skipForwardWhitespaceAndComments(body, parsed.end)
        if (index < body.length) {
            return MixinExtrasExpressionIndex()
        }

        return MixinExtrasExpressionIndex(parsed.values)
    }

    private fun parseExpressionOrExpressionArray(source: String, start: Int): ParsedValue<MixinExtrasExpression>? {
        val index = skipForwardWhitespaceAndComments(source, start)
        if (index >= source.length) return null
        return when (source[index]) {
            '@' -> readExpressionAnnotation(source, index)?.let { ParsedValue(listOf(it.value), it.end) }
            '{' -> parseExpressionArray(source, index)
            else -> null
        }
    }

    private fun parseExpressionArray(source: String, openBrace: Int): ParsedValue<MixinExtrasExpression>? {
        val closeBrace = findMatchingCloseBrace(source, openBrace) ?: return null
        val values = mutableListOf<MixinExtrasExpression>()
        for (element in splitTopLevelCommas(source.substring(openBrace + 1, closeBrace))) {
            val trimmedStart = skipForwardWhitespaceAndComments(element, 0)
            if (trimmedStart >= element.length) continue
            val expression = readExpressionAnnotation(element, trimmedStart) ?: continue
            values += expression.value
        }
        return ParsedValue(values, closeBrace + 1)
    }

    private fun readExpressionAnnotation(source: String, start: Int): ParsedScalar<MixinExtrasExpression>? {
        if (source.getOrNull(start) != '@') return null
        var index = start + 1
        val nameStart = index
        while (index < source.length && (source[index].isLetterOrDigit() || source[index] in "._$")) {
            index++
        }
        if (index == nameStart) return null
        val qualifiedName = source.substring(nameStart, index)
        if (!isExpressionAnnotationName(qualifiedName)) return null

        index = skipForwardWhitespaceAndComments(source, index)
        if (source.getOrNull(index) != '(') return null
        val closeParen = findMatchingCloseParen(source, index) ?: return null
        val expressionBody = source.substring(index + 1, closeParen)
        val expression = ExpressionAnnotationParser.parse(expressionBody) ?: return null
        var end = closeParen + 1
        end = skipForwardWhitespaceAndComments(source, end)
        if (end < source.length) return null
        return ParsedScalar(expression, end)
    }

    private fun isExpressionAnnotationName(qualifiedName: String): Boolean =
        qualifiedName == "Expression" ||
            qualifiedName == "com.llamalad7.mixinextras.expression.Expression"

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
