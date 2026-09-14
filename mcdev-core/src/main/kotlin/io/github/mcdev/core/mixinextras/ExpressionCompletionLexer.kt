package io.github.mcdev.core.mixinextras

enum class ExpressionCompletionPosition {
    STATEMENT_START,
    VALUE_START,
    AFTER_DOT,
    AFTER_METHOD_REFERENCE,
    NONE,
}

data class ExpressionCompletionLexResult(
    val position: ExpressionCompletionPosition,
    val tokenStart: Int,
    val tokenEnd: Int,
) {
    fun hasIdentifierToken(): Boolean = tokenEnd > tokenStart
}

object ExpressionCompletionLexer {
    fun lex(expressionPrefix: String, cursorInExpression: Int): ExpressionCompletionLexResult {
        val cursor = cursorInExpression.coerceIn(0, expressionPrefix.length)
        if (cursorInsideLiteralOrComment(expressionPrefix, cursor)) {
            return emptyToken(cursor, ExpressionCompletionPosition.NONE)
        }

        val position = resolvePositionAt(expressionPrefix, cursor)
        if (position == ExpressionCompletionPosition.AFTER_DOT ||
            position == ExpressionCompletionPosition.AFTER_METHOD_REFERENCE
        ) {
            return emptyToken(cursor, position)
        }

        identifierBoundsAt(expressionPrefix, cursor)?.let { bounds ->
            val start = bounds.first
            val end = bounds.last + 1
            if (end > start) {
                return ExpressionCompletionLexResult(
                    resolvePositionAt(expressionPrefix, start),
                    start,
                    end,
                )
            }
        }

        return emptyToken(cursor, position)
    }

    private fun resolvePositionAt(source: String, index: Int): ExpressionCompletionPosition {
        if (isAfterMethodReference(source, index)) {
            return ExpressionCompletionPosition.AFTER_METHOD_REFERENCE
        }
        if (isAfterDot(source, index)) {
            return ExpressionCompletionPosition.AFTER_DOT
        }
        if (isStatementStart(source, index)) {
            return ExpressionCompletionPosition.STATEMENT_START
        }
        if (isValueStart(source, index)) {
            return ExpressionCompletionPosition.VALUE_START
        }
        return ExpressionCompletionPosition.NONE
    }

    private fun emptyToken(cursor: Int, position: ExpressionCompletionPosition): ExpressionCompletionLexResult =
        ExpressionCompletionLexResult(position, cursor, cursor)

    private fun cursorInsideLiteralOrComment(source: String, cursor: Int): Boolean {
        var index = 0
        while (index < cursor) {
            when {
                isLineCommentStart(source, index) -> {
                    index = skipLineComment(source, index)
                    if (index >= cursor) return true
                }
                isBlockCommentStart(source, index) -> {
                    val end = skipBlockCommentEnd(source, index)
                    if (end < 0 || end >= cursor) return true
                    index = end
                }
                source[index] == '\'' -> {
                    val end = skipSingleQuotedLiteral(source, index)
                    if (end < 0 || end >= cursor) return true
                    index = end
                }
                else -> index++
            }
        }
        return false
    }

    private fun identifierBoundsAt(source: String, cursor: Int): IntRange? {
        val hasPartBefore = cursor > 0 && source[cursor - 1].isJavaIdentifierPart()
        val hasPartAt = cursor < source.length && source[cursor].isJavaIdentifierStart()
        if (!hasPartBefore && !hasPartAt) return null

        var start = cursor
        if (hasPartBefore) {
            start = cursor - 1
            while (start > 0 && source[start - 1].isJavaIdentifierPart()) {
                start--
            }
        }
        var end = if (hasPartBefore) cursor else cursor + 1
        while (end < source.length && source[end].isJavaIdentifierPart()) {
            end++
        }
        return start until end
    }

    private fun isAfterMethodReference(source: String, cursor: Int): Boolean {
        val index = skipBackwardWhitespace(source, cursor)
        return index >= 2 && source.startsWith("::", index - 2)
    }

    private fun isAfterDot(source: String, cursor: Int): Boolean {
        val index = skipBackwardWhitespace(source, cursor)
        if (index <= 0 || source[index - 1] != '.') return false
        if (index >= 2 && source[index - 2] == ':') return false
        if (source.startsWith("class", index)) return false
        return true
    }

    private fun isStatementStart(source: String, cursor: Int): Boolean {
        if (blockDepthAt(source, cursor) <= 0) return false
        val index = skipBackwardWhitespace(source, cursor)
        if (index <= 0) return false
        return when (source[index - 1]) {
            '{', ';' -> true
            else -> false
        }
    }

    private fun isValueStart(source: String, cursor: Int): Boolean {
        if (cursor == 0) return true
        if (isAfterEndOfExpression(source, cursor)) return false
        val index = skipBackwardWhitespace(source, cursor)
        if (index <= 0) return true
        val ch = source[index - 1]
        return ch in "+-*/%&|^!=<>?:,([" || ch == '=' || ch == '@'
    }

    private fun isAfterEndOfExpression(source: String, cursor: Int): Boolean {
        val index = skipBackwardWhitespace(source, cursor)
        if (index <= 0) return false
        if (source[index - 1] in ")]}?") return true
        if (endsWithClassSuffix(source, index)) return true
        if (endsWithKeyword(source, index, "true") || endsWithKeyword(source, index, "false")) return true
        if (endsWithKeyword(source, index, "null")) return true
        if (endsWithNumericLiteral(source, index)) return true
        if (endsWithSingleQuotedLiteral(source, index)) return true
        return endsWithIdentifier(source, index)
    }

    private fun blockDepthAt(source: String, cursor: Int): Int {
        var depth = 0
        var index = 0
        while (index < cursor) {
            when {
                isLineCommentStart(source, index) -> index = skipLineComment(source, index)
                isBlockCommentStart(source, index) -> {
                    val end = skipBlockCommentEnd(source, index)
                    index = if (end < 0) cursor else end
                }
                source[index] == '\'' -> {
                    val end = skipSingleQuotedLiteral(source, index)
                    index = if (end < 0) cursor else end
                }
                source[index] == '{' -> {
                    depth++
                    index++
                }
                source[index] == '}' -> {
                    depth = (depth - 1).coerceAtLeast(0)
                    index++
                }
                else -> index++
            }
        }
        return depth
    }

    private fun skipBackwardWhitespace(source: String, cursor: Int): Int {
        var index = cursor.coerceIn(0, source.length)
        while (index > 0 && source[index - 1].isWhitespace()) {
            index--
        }
        return index
    }

    private fun endsWithIdentifier(source: String, endExclusive: Int): Boolean {
        if (endExclusive <= 0 || !source[endExclusive - 1].isJavaIdentifierPart()) return false
        var start = endExclusive - 1
        while (start > 0 && source[start - 1].isJavaIdentifierPart()) {
            start--
        }
        return source[start].isJavaIdentifierStart()
    }

    private fun endsWithKeyword(source: String, endExclusive: Int, keyword: String): Boolean {
        val start = endExclusive - keyword.length
        if (start < 0) return false
        if (!source.regionMatches(start, keyword, 0, keyword.length)) return false
        if (start > 0 && source[start - 1].isJavaIdentifierPart()) return false
        if (endExclusive < source.length && source[endExclusive].isJavaIdentifierPart()) return false
        return true
    }

    private fun endsWithClassSuffix(source: String, endExclusive: Int): Boolean {
        val suffix = "class"
        val start = endExclusive - suffix.length
        if (start <= 0 || !source.regionMatches(start, suffix, 0, suffix.length)) return false
        return source[start - 1] == '.'
    }

    private fun endsWithNumericLiteral(source: String, endExclusive: Int): Boolean {
        var index = endExclusive - 1
        if (index < 0) return false
        if (source[index] !in "0123456789.eEfFdDlL") return false
        while (index >= 0 && source[index] in "0123456789.eEfFdDlLxXabcdefABCDEF_") {
            index--
        }
        return index + 1 < endExclusive
    }

    private fun endsWithSingleQuotedLiteral(source: String, endExclusive: Int): Boolean {
        if (endExclusive <= 0 || source[endExclusive - 1] != '\'') return false
        var index = endExclusive - 2
        while (index >= 0) {
            if (source[index] == '\'' && (index == 0 || source[index - 1] != '\\')) {
                return true
            }
            index--
        }
        return false
    }

    private fun skipSingleQuotedLiteral(source: String, start: Int): Int {
        if (source.getOrNull(start) != '\'') return start
        var index = start + 1
        while (index < source.length) {
            when {
                source[index] == '\\' -> index += 2
                source[index] == '\'' -> return index + 1
                else -> index++
            }
        }
        return -1
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

    private fun skipBlockCommentEnd(source: String, index: Int): Int {
        var current = index + 2
        while (current + 1 < source.length) {
            if (source[current] == '*' && source[current + 1] == '/') {
                return current + 2
            }
            current++
        }
        return -1
    }

    private fun Char.isJavaIdentifierStart(): Boolean = isLetter() || this == '_' || this == '$'

    private fun Char.isJavaIdentifierPart(): Boolean = isLetterOrDigit() || this == '_' || this == '$'
}
