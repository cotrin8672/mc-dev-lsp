package io.github.mcdev.core.mixinextras

data class ParsedExpressionReceiver(
    val receiver: String,
    val memberPrefix: String,
)

object ExpressionReceiverParser {
    private val invalidReceiverRoots = setOf("new", "true", "false", "null")
    private val receiverBoundaryChars = setOf(
        '+', '-', '*', '/', '%', '&', '|', '^', '=', '!', '<', '>', '?', ':', ',', ';', '~',
    )
    private val receiverBoundaryKeywords = setOf("return", "throw")

    fun parse(expressionPrefix: String, cursorInExpression: Int): ParsedExpressionReceiver? {
        val cursor = cursorInExpression.coerceIn(0, expressionPrefix.length)
        val lexResult = ExpressionCompletionLexer.lex(expressionPrefix, cursor)
        if (lexResult.position != ExpressionCompletionPosition.AFTER_DOT) {
            return null
        }

        val memberAnchor = if (lexResult.hasIdentifierToken()) {
            lexResult.tokenStart
        } else {
            cursor
        }
        val dotIndex = dotBeforeMember(expressionPrefix, memberAnchor) ?: return null
        val memberPrefix = memberPrefixAt(expressionPrefix, dotIndex, cursor) ?: return null
        val receiver = receiverBeforeDot(expressionPrefix, dotIndex) ?: return null
        if (!isValidReceiverExpression(receiver)) {
            return null
        }

        return ParsedExpressionReceiver(receiver = receiver, memberPrefix = memberPrefix)
    }

    private fun dotBeforeMember(source: String, cursor: Int): Int? {
        var index = cursor
        while (index > 0 && source[index - 1].isWhitespace()) {
            index--
        }
        if (index <= 0 || source[index - 1] != '.') {
            return null
        }
        if (index >= 2 && source[index - 2] == ':') {
            return null
        }
        if (source.substring(0, index - 1).contains("::")) {
            return null
        }
        if (source.startsWith("class", index)) {
            return null
        }
        return index - 1
    }

    private fun memberPrefixAt(source: String, dotIndex: Int, cursor: Int): String? {
        var start = dotIndex + 1
        while (start < cursor && source[start].isWhitespace()) {
            start++
        }
        val end = cursor
        if (start > end) {
            return ""
        }
        val prefix = source.substring(start, end)
        if (prefix.isEmpty()) {
            return ""
        }
        if (!prefix.first().isJavaIdentifierStart()) {
            return null
        }
        if (prefix.any { !it.isJavaIdentifierPart() }) {
            return null
        }
        return prefix
    }

    /**
     * Returns the complete expression immediately to the left of the completion dot.
     *
     * A receiver can contain calls, array accesses, casts, and parenthesized expressions.
     * Scanning the prefix from the beginning lets us balance those constructs while still
     * discarding a preceding operand such as the `a +` in `a + value.trim().`.
     */
    private fun receiverBeforeDot(source: String, dotIndex: Int): String? {
        var end = dotIndex
        while (end > 0 && source[end - 1].isWhitespace()) end--
        if (end <= 0) {
            return null
        }

        val start = expressionStart(source, end) ?: return null
        if (start >= end) return null
        return source.substring(start, end).trim().takeIf { it.isNotEmpty() }
    }

    /**
     * Performs cheap structural checks here and lets the official parser decide whether a
     * complex expression is part of MixinExtras' actual grammar. This keeps completion
     * fail-closed for malformed input without maintaining a second expression grammar.
     */
    internal fun isValidReceiverExpression(receiverExpression: String): Boolean {
        val receiver = receiverExpression.trim()
        if (receiver.isEmpty()) {
            return false
        }
        if (receiver == "super") {
            return true
        }
        if (receiver in invalidReceiverRoots) {
            return false
        }
        if (receiver.contains("::")) {
            return false
        }
        if (receiver.first().isDigit() || receiver.first() == '\'' || receiver.first() == '"') {
            return false
        }

        return when (OfficialExpressionParser.parse("@($receiver.?)")) {
            is OfficialExpressionParseResult.Success -> true
            is OfficialExpressionParseResult.SyntaxFailure -> false
        }
    }

    private fun expressionStart(source: String, endExclusive: Int): Int? {
        val delimiters = mutableListOf<UnclosedDelimiter>()
        var start = 0
        var index = 0
        while (index < endExclusive) {
            when {
                source[index] == '\'' || source[index] == '"' -> {
                    index = skipQuotedLiteral(source, index, endExclusive) ?: return null
                    continue
                }
                source[index] == '/' && index + 1 < endExclusive && source[index + 1] == '/' -> {
                    index = skipLineComment(source, index, endExclusive)
                    continue
                }
                source[index] == '/' && index + 1 < endExclusive && source[index + 1] == '*' -> {
                    index = skipBlockComment(source, index, endExclusive) ?: return null
                    continue
                }
            }

            when (val ch = source[index]) {
                '(', '[', '{' -> delimiters += UnclosedDelimiter(
                    opening = ch,
                    index = index,
                )
                ')', ']', '}' -> {
                    if (delimiters.isEmpty() || !matches(delimiters.removeAt(delimiters.lastIndex).opening, ch)) {
                        return null
                    }
                }
                else -> if (delimiters.isEmpty()) {
                    if (ch in receiverBoundaryChars) {
                        start = index + 1
                    } else {
                        receiverBoundaryKeywords.firstOrNull { keyword ->
                            source.startsWith(keyword, index) &&
                                isKeywordBoundary(source, index, keyword, endExclusive)
                        }?.let { keyword ->
                            start = index + keyword.length
                            index += keyword.length - 1
                        }
                    }
                }
            }
            index++
        }
        if (delimiters.isEmpty()) return start

        val outerDelimiter = delimiters.last()
        val nestedSourceStart = outerDelimiter.index + 1
        val nestedStart = expressionStart(
            source.substring(nestedSourceStart, endExclusive),
            endExclusive - nestedSourceStart,
        ) ?: return null
        return nestedSourceStart + nestedStart
    }

    private data class UnclosedDelimiter(
        val opening: Char,
        val index: Int,
    )

    private fun skipQuotedLiteral(source: String, start: Int, endExclusive: Int): Int? {
        val quote = source[start]
        var index = start + 1
        while (index < endExclusive) {
            when {
                source[index] == '\\' -> index += 2
                source[index] == quote -> return index + 1
                else -> index++
            }
        }
        return null
    }

    private fun skipLineComment(source: String, start: Int, endExclusive: Int): Int {
        var index = start + 2
        while (index < endExclusive && source[index] != '\n') index++
        return index
    }

    private fun skipBlockComment(source: String, start: Int, endExclusive: Int): Int? {
        var index = start + 2
        while (index + 1 < endExclusive) {
            if (source[index] == '*' && source[index + 1] == '/') return index + 2
            index++
        }
        return null
    }

    private fun isKeywordBoundary(source: String, start: Int, keyword: String, endExclusive: Int): Boolean {
        if (start > 0 && (source[start - 1].isJavaIdentifierPart() || source[start - 1] == '.')) {
            return false
        }
        val end = start + keyword.length
        return end >= endExclusive || !source[end].isJavaIdentifierPart()
    }

    private fun matches(opening: Char, closing: Char): Boolean =
        (opening == '(' && closing == ')') ||
            (opening == '[' && closing == ']') ||
            (opening == '{' && closing == '}')

    private fun Char.isJavaIdentifierStart(): Boolean = isLetter() || this == '_' || this == '$'

    private fun Char.isJavaIdentifierPart(): Boolean = isLetterOrDigit() || this == '_' || this == '$'
}
