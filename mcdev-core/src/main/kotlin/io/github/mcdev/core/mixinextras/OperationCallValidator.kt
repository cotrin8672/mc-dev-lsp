package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mixin.AnnotationContextExtractor

data class OperationCallIssue(
    val argumentRange: McTextRange,
    val expectedNames: List<String>,
)

/** One expected Operation.call argument derived from a non-sugar handler parameter before Operation. */
data class OperationCallExpectedArg(
    val name: String,
    val descriptor: String?,
)

/**
 * Optional hook for future JDT-backed expression typing. Defaults to null and is unused in core.
 */
fun interface OperationCallArgumentTypeResolver {
    fun resolveType(expression: String): String?
}

/** Authoritative Operation.call arity context derived from expected handler layout. */
data class OperationCallValidationLayout(
    val operationParameterName: String,
    val expectedCallArgs: List<OperationCallExpectedArg>,
    val handlerParameterDescriptors: Map<String, String?>,
    val handlerRange: McTextRange,
) {
    val expectedCallArgNames: List<String> get() = expectedCallArgs.map { it.name }
}

private enum class DescriptorCompatibility {
    COMPATIBLE,
    INCOMPATIBLE,
    UNKNOWN,
}

object OperationCallValidator {
    /**
     * Resolves whether [handler] has an unambiguous required Operation parameter layout
     * matching [expected]. Returns null when the expected signature is unavailable or the
     * handler layout is unresolved/ambiguous (fail closed).
     */
    fun resolveOperationCallLayout(
        site: MixinExtrasAnnotationSite,
        handler: HandlerMethodDeclaration,
        expected: HandlerSignatureSpec?,
    ): OperationCallValidationLayout? {
        if (expected == null) return null
        if (site.annotation != MixinExtrasAnnotation.WRAP_OPERATION &&
            site.annotation != MixinExtrasAnnotation.WRAP_METHOD
        ) {
            return null
        }

        val operationParams = handler.parameters.filter { it.isOperation }
        if (operationParams.size != 1) return null

        val firstSugarIndex = handler.parameters.indexOfFirst { it.isSugar }
        if (firstSugarIndex >= 0 &&
            handler.parameters.subList(firstSugarIndex + 1, handler.parameters.size).any { !it.isSugar }
        ) {
            return null
        }

        val nonSugarParams = handler.parameters.filter { !it.isSugar }
        val operationIndex = nonSugarParams.indexOfFirst { it.isOperation }
        if (operationIndex < 0) return null

        val expectedRequired = expected.parameters
        val expectedOperationIndex = expectedRequired.indexOfLast { it.isOperation }
        if (expectedOperationIndex < 0) return null

        when (site.annotation) {
            MixinExtrasAnnotation.WRAP_METHOD -> {
                if (operationIndex != nonSugarParams.lastIndex) return null
                if (nonSugarParams.size != expectedRequired.size) return null
                if (operationIndex != expectedOperationIndex) return null
            }
            MixinExtrasAnnotation.WRAP_OPERATION -> {
                val requiredCount = expectedRequired.size
                if (nonSugarParams.size < requiredCount) return null
                if (operationIndex != requiredCount - 1) return null
                if (!nonSugarParams[requiredCount - 1].isOperation) return null
            }
            else -> return null
        }

        val expectedCallArgCount = expected.operationCallArgs.size
        val callArgParams = nonSugarParams.take(operationIndex)
        if (callArgParams.size != expectedCallArgCount) return null

        val expectedCallArgs = callArgParams.map { param ->
            OperationCallExpectedArg(name = param.name, descriptor = param.typeDescriptor)
        }
        if (expectedCallArgs.any { !isValidJavaIdentifier(it.name) }) return null

        val handlerParameterDescriptors = handler.parameters
            .filter { !it.isSugar }
            .associate { it.name to it.typeDescriptor }

        return OperationCallValidationLayout(
            operationParameterName = operationParams.single().name,
            expectedCallArgs = expectedCallArgs,
            handlerParameterDescriptors = handlerParameterDescriptors,
            handlerRange = handler.range,
        )
    }

    private fun isValidJavaIdentifier(name: String): Boolean {
        if (name.isEmpty()) return false
        if (!name[0].isJavaIdentifierStart()) return false
        for (index in 1 until name.length) {
            if (!name[index].isJavaIdentifierPart()) return false
        }
        return true
    }

    fun validate(
        source: String,
        site: MixinExtrasAnnotationSite,
        layout: OperationCallValidationLayout?,
        argumentTypeResolver: OperationCallArgumentTypeResolver? = null,
    ): List<OperationCallIssue> {
        if (site.handlerMethod == null || layout == null) return emptyList()
        if (site.annotation != MixinExtrasAnnotation.WRAP_OPERATION &&
            site.annotation != MixinExtrasAnnotation.WRAP_METHOD
        ) {
            return emptyList()
        }

        val bodyRange = findHandlerBodyContentRange(source, layout.handlerRange) ?: return emptyList()
        return scanForInvalidCalls(
            source,
            bodyRange,
            layout.operationParameterName,
            layout.expectedCallArgs,
            layout.handlerParameterDescriptors,
            argumentTypeResolver,
        )
    }

    private fun findHandlerBodyContentRange(source: String, handlerRange: McTextRange): IntRange? {
        val searchFrom = rangeEnd(source, handlerRange) ?: return null
        var index = searchFrom
        while (index < source.length) {
            when {
                isLineCommentStart(source, index) -> index = skipLineComment(source, index)
                isBlockCommentStart(source, index) -> {
                    val end = skipBlockComment(source, index)
                    index = if (end < 0) source.length else end
                }
                source[index].isWhitespace() -> index++
                source[index] == ';' -> return null
                source[index] == '{' -> {
                    val braceClose = findMatchingBrace(source, index) ?: return null
                    return (index + 1) until braceClose
                }
                else -> index++
            }
        }
        return null
    }

    private fun scanForInvalidCalls(
        source: String,
        bodyRange: IntRange,
        operationName: String,
        expectedArgs: List<OperationCallExpectedArg>,
        handlerParameterDescriptors: Map<String, String?>,
        argumentTypeResolver: OperationCallArgumentTypeResolver?,
    ): List<OperationCallIssue> {
        val expectedNames = expectedArgs.map { it.name }
        val issues = mutableListOf<OperationCallIssue>()
        var index = bodyRange.first
        val bodyEndExclusive = bodyRange.last + 1
        while (index < bodyEndExclusive) {
            when {
                isLineCommentStart(source, index) -> index = skipLineComment(source, index)
                isBlockCommentStart(source, index) -> {
                    val end = skipBlockComment(source, index)
                    index = if (end < 0) bodyEndExclusive else end
                }
                source[index] == '"' -> {
                    val end = skipStringLiteral(source, index)
                    index = if (end < 0) bodyEndExclusive else end
                }
                source[index] == '\'' -> {
                    val end = skipCharLiteral(source, index)
                    index = if (end < 0) bodyEndExclusive else end
                }
                source[index] == '@' -> {
                    val end = skipAnnotation(source, index)
                    index = end ?: (index + 1)
                }
                source[index].isJavaIdentifierStart() -> {
                    val identifierStart = index
                    val identifierEnd = readIdentifierEnd(source, index)
                    index = identifierEnd
                    if (identifierEnd - identifierStart == operationName.length &&
                        source.regionMatches(identifierStart, operationName, 0, operationName.length) &&
                        isUnqualifiedOperationReceiver(source, identifierStart)
                    ) {
                        val openParen = matchOperationCallOpenParen(source, identifierEnd)
                        if (openParen != null) {
                            val closeParen = findMatchingParen(source, openParen)
                            if (closeParen != null && closeParen < bodyEndExclusive) {
                                val contentStart = openParen + 1
                                val contentEnd = closeParen
                                val actualArgs = topLevelArguments(source, contentStart, contentEnd)
                                if (actualArgs.size != expectedArgs.size) {
                                    issues += OperationCallIssue(
                                        argumentRange = argumentContentRange(source, contentStart, contentEnd),
                                        expectedNames = expectedNames,
                                    )
                                } else if (hasIncompatibleArgumentTypes(
                                        actualArgs,
                                        expectedArgs,
                                        handlerParameterDescriptors,
                                        argumentTypeResolver,
                                    )
                                ) {
                                    issues += OperationCallIssue(
                                        argumentRange = argumentContentRange(source, contentStart, contentEnd),
                                        expectedNames = expectedNames,
                                    )
                                }
                                index = closeParen + 1
                                continue
                            }
                        }
                    }
                }
                else -> index++
            }
        }
        return issues
    }

    private fun matchOperationCallOpenParen(source: String, identifierEnd: Int): Int? {
        var index = skipForwardWhitespace(source, identifierEnd)
        if (source.getOrNull(index) != '.') return null
        index++
        index = skipForwardWhitespace(source, index)
        if (!source.regionMatches(index, "call", 0, 4)) return null
        index += 4
        if (index < source.length && source[index].isJavaIdentifierPart()) return null
        index = skipForwardWhitespace(source, index)
        if (source.getOrNull(index) != '(') return null
        return index
    }

    private fun isUnqualifiedOperationReceiver(source: String, identifierStart: Int): Boolean {
        var index = identifierStart - 1
        while (index >= 0 && source[index].isWhitespace()) {
            index--
        }
        if (index < 0) return true
        return when (source[index]) {
            '.', ')', ']', '>' -> false
            '(' -> false
            else -> true
        }
    }

    private fun topLevelArguments(source: String, contentStart: Int, contentEnd: Int): List<String> {
        if (contentStart >= contentEnd) return emptyList()
        val content = source.substring(contentStart, contentEnd)
        if (content.isBlank()) return emptyList()
        return splitTopLevelCommas(content).map { it.trim() }.filter { it.isNotEmpty() }
    }

    private fun hasIncompatibleArgumentTypes(
        actualArgs: List<String>,
        expectedArgs: List<OperationCallExpectedArg>,
        handlerParameterDescriptors: Map<String, String?>,
        argumentTypeResolver: OperationCallArgumentTypeResolver?,
    ): Boolean {
        var sawIncompatible = false
        for (index in expectedArgs.indices) {
            when (
                argumentTypeCompatibility(
                    expectedArgs[index],
                    actualArgs[index],
                    handlerParameterDescriptors,
                    argumentTypeResolver,
                )
            ) {
                DescriptorCompatibility.INCOMPATIBLE -> sawIncompatible = true
                DescriptorCompatibility.UNKNOWN,
                DescriptorCompatibility.COMPATIBLE,
                -> Unit
            }
        }
        return sawIncompatible
    }

    private fun argumentTypeCompatibility(
        expected: OperationCallExpectedArg,
        actualExpression: String,
        handlerParameterDescriptors: Map<String, String?>,
        argumentTypeResolver: OperationCallArgumentTypeResolver?,
    ): DescriptorCompatibility {
        val expectedDescriptor = expected.descriptor ?: return DescriptorCompatibility.UNKNOWN

        val actualDescriptor = resolveActualArgumentDescriptor(
            actualExpression,
            handlerParameterDescriptors,
            argumentTypeResolver,
        ) ?: return DescriptorCompatibility.UNKNOWN

        return descriptorCompatibility(expectedDescriptor, actualDescriptor)
    }

    private fun resolveActualArgumentDescriptor(
        actualExpression: String,
        handlerParameterDescriptors: Map<String, String?>,
        argumentTypeResolver: OperationCallArgumentTypeResolver?,
    ): String? {
        val handlerParamName = singleHandlerParameterIdentifier(actualExpression) ?: run {
            return argumentTypeResolver?.resolveType(actualExpression)
        }
        val descriptor = handlerParameterDescriptors[handlerParamName] ?: return null
        return descriptor ?: argumentTypeResolver?.resolveType(actualExpression)
    }

    private fun singleHandlerParameterIdentifier(expression: String): String? {
        if (expression.isEmpty() || !expression[0].isJavaIdentifierStart()) return null
        var index = 1
        while (index < expression.length && expression[index].isJavaIdentifierPart()) {
            index++
        }
        return if (index == expression.length) expression else null
    }

    private fun descriptorCompatibility(expected: String, actual: String): DescriptorCompatibility {
        if (expected == actual) return DescriptorCompatibility.COMPATIBLE

        val expectedPrimitive = asPrimitiveDescriptor(expected)
        val actualPrimitive = asPrimitiveDescriptor(actual)
        val expectedWrapper = asWrapperDescriptor(expected)
        val actualWrapper = asWrapperDescriptor(actual)

        if (expectedPrimitive != null && actualWrapper == wrapperForPrimitive(expectedPrimitive)) {
            return DescriptorCompatibility.COMPATIBLE
        }
        if (actualPrimitive != null && expectedWrapper == wrapperForPrimitive(actualPrimitive)) {
            return DescriptorCompatibility.COMPATIBLE
        }

        if (expectedPrimitive != null && actualPrimitive != null) {
            return DescriptorCompatibility.INCOMPATIBLE
        }
        if (expectedWrapper != null && actualWrapper != null) {
            return DescriptorCompatibility.INCOMPATIBLE
        }

        val expectedInNumericFamily = expectedPrimitive != null || expectedWrapper != null
        val actualInNumericFamily = actualPrimitive != null || actualWrapper != null
        if (expectedInNumericFamily || actualInNumericFamily) {
            return DescriptorCompatibility.INCOMPATIBLE
        }

        if (expected.startsWith('L') && actual.startsWith('L')) {
            return DescriptorCompatibility.UNKNOWN
        }

        return DescriptorCompatibility.UNKNOWN
    }

    private fun asPrimitiveDescriptor(descriptor: String): String? =
        if (descriptor.length == 1 && descriptor[0] in PRIMITIVE_DESCRIPTORS) descriptor else null

    private fun asWrapperDescriptor(descriptor: String): String? =
        if (descriptor.startsWith('L') && descriptor.endsWith(';')) descriptor else null

    private fun wrapperForPrimitive(primitive: String): String? = when (primitive) {
        "Z" -> "Ljava/lang/Boolean;"
        "B" -> "Ljava/lang/Byte;"
        "C" -> "Ljava/lang/Character;"
        "S" -> "Ljava/lang/Short;"
        "I" -> "Ljava/lang/Integer;"
        "J" -> "Ljava/lang/Long;"
        "F" -> "Ljava/lang/Float;"
        "D" -> "Ljava/lang/Double;"
        else -> null
    }

    private val PRIMITIVE_DESCRIPTORS = setOf('Z', 'B', 'C', 'S', 'I', 'J', 'F', 'D')

    private fun argumentContentRange(source: String, contentStart: Int, contentEnd: Int): McTextRange {
        if (contentStart >= contentEnd) {
            return offsetRange(source, contentStart, contentStart)
        }
        var start = contentStart
        var end = contentEnd
        while (start < end && source[start].isWhitespace()) start++
        while (end > start && source[end - 1].isWhitespace()) end--
        if (start >= end) {
            return offsetRange(source, contentStart, contentStart)
        }
        return offsetRange(source, start, end)
    }

    private fun splitTopLevelCommas(value: String): List<String> {
        val results = mutableListOf<String>()
        var start = 0
        var angleDepth = 0
        var parenDepth = 0
        var braceDepth = 0
        var bracketDepth = 0
        var inString = false
        var inChar = false
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
                inChar -> {
                    if (value[index] == '\\' && index + 1 < value.length) {
                        index += 2
                        continue
                    }
                    if (value[index] == '\'') inChar = false
                }
                isLineCommentStart(value, index) -> index = skipLineComment(value, index)
                isBlockCommentStart(value, index) -> {
                    val end = skipBlockComment(value, index)
                    index = if (end < 0) value.length else end
                }
                value[index] == '"' -> inString = true
                value[index] == '\'' -> inChar = true
                value[index] == '<' -> {
                    if (hasMatchingGenericCloseAngle(value, index)) {
                        angleDepth++
                    }
                }
                value[index] == '>' && angleDepth > 0 -> angleDepth--
                value[index] == '(' -> parenDepth++
                value[index] == ')' -> parenDepth = (parenDepth - 1).coerceAtLeast(0)
                value[index] == '{' -> braceDepth++
                value[index] == '}' -> braceDepth = (braceDepth - 1).coerceAtLeast(0)
                value[index] == '[' -> bracketDepth++
                value[index] == ']' -> bracketDepth = (bracketDepth - 1).coerceAtLeast(0)
                value[index] == ',' &&
                    angleDepth == 0 &&
                    parenDepth == 0 &&
                    braceDepth == 0 &&
                    bracketDepth == 0 &&
                    !inString &&
                    !inChar -> {
                    results += value.substring(start, index)
                    start = index + 1
                }
            }
            index++
        }
        results += value.substring(start)
        return results
    }

    /**
     * Treat `<` as a generic opener only when it is followed by a balanced `>` that closes
     * type arguments, not a relational comparison chain such as `a < b > c`.
     */
    private fun hasMatchingGenericCloseAngle(value: String, openIndex: Int): Boolean {
        if (value.getOrNull(openIndex) != '<') return false
        val closeIndex = findBalancedGenericCloseAngle(value, openIndex) ?: return false
        return isGenericAnglePair(value, openIndex, closeIndex)
    }

    private fun findBalancedGenericCloseAngle(value: String, openIndex: Int): Int? {
        var depth = 1
        var index = openIndex + 1
        var inString = false
        var inChar = false
        while (index < value.length) {
            when {
                inString -> {
                    if (value[index] == '\\' && index + 1 < value.length) {
                        index += 2
                        continue
                    }
                    if (value[index] == '"') inString = false
                }
                inChar -> {
                    if (value[index] == '\\' && index + 1 < value.length) {
                        index += 2
                        continue
                    }
                    if (value[index] == '\'') inChar = false
                }
                value[index] == '"' -> inString = true
                value[index] == '\'' -> inChar = true
                value[index] == '<' -> depth++
                value[index] == '>' -> {
                    depth--
                    if (depth == 0) return index
                }
            }
            index++
        }
        return null
    }

    private fun isGenericAnglePair(value: String, openIndex: Int, closeIndex: Int): Boolean {
        var before = openIndex - 1
        while (before >= 0 && value[before].isWhitespace()) {
            before--
        }
        if (before < 0) return false

        when (value[before]) {
            '.', ',', '?', '>' -> return true
        }

        if (value[before].isJavaIdentifierPart()) {
            if (isNewKeywordEndingAt(value, before)) return true

            val identifierEnd = before + 1
            val gap = value.substring(identifierEnd, openIndex)
            if (gap.isEmpty()) {
                var after = closeIndex + 1
                while (after < value.length && value[after].isWhitespace()) {
                    after++
                }
                val inTypeArgContext = when (value.getOrNull(after)) {
                    '(', ')', '.', '[', ';' -> true
                    null -> true
                    else -> false
                }
                return inTypeArgContext
            }
            return false
        }

        return false
    }

    private fun isNewKeywordEndingAt(value: String, endInclusive: Int): Boolean {
        val start = endInclusive - 2
        return start >= 0 && value.regionMatches(start, "new", 0, 3) &&
            (start == 0 || !value[start - 1].isJavaIdentifierPart())
    }

    private fun findMatchingBrace(source: String, openIndex: Int): Int? =
        findMatchingDelimiters(source, openIndex, '{', '}')

    private fun findMatchingParen(source: String, openIndex: Int): Int? =
        findMatchingDelimiters(source, openIndex, '(', ')')

    private fun findMatchingDelimiters(source: String, openIndex: Int, open: Char, close: Char): Int? {
        if (source.getOrNull(openIndex) != open) return null
        var depth = 0
        var index = openIndex
        while (index < source.length) {
            when {
                isLineCommentStart(source, index) -> index = skipLineComment(source, index)
                isBlockCommentStart(source, index) -> {
                    val end = skipBlockComment(source, index)
                    index = if (end < 0) source.length else end
                }
                source[index] == '"' -> {
                    val end = skipStringLiteral(source, index)
                    index = if (end < 0) source.length else end
                }
                source[index] == '\'' -> {
                    val end = skipCharLiteral(source, index)
                    index = if (end < 0) source.length else end
                }
                source[index] == open -> {
                    depth++
                    index++
                }
                source[index] == close -> {
                    depth--
                    if (depth == 0) return index
                    index++
                }
                else -> index++
            }
        }
        return null
    }

    private fun skipAnnotation(source: String, atOffset: Int): Int? {
        if (source.getOrNull(atOffset) != '@') return null
        var end = atOffset + 1
        while (end < source.length && (source[end].isLetterOrDigit() || source[end] in "._$")) {
            end++
        }
        if (source.getOrNull(end) != '(') return end
        return findMatchingParen(source, end)?.plus(1)
    }

    private fun readIdentifierEnd(source: String, start: Int): Int {
        var end = start + 1
        while (end < source.length && source[end].isJavaIdentifierPart()) {
            end++
        }
        return end
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
        return -1
    }

    private fun skipStringLiteral(source: String, start: Int): Int {
        if (source.getOrNull(start) != '"') return start
        var index = start + 1
        while (index < source.length) {
            when {
                source[index] == '\\' -> index += 2
                source[index] == '"' -> return index + 1
                else -> index++
            }
        }
        return -1
    }

    private fun skipCharLiteral(source: String, start: Int): Int {
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

    private fun skipForwardWhitespace(source: String, index: Int): Int {
        var current = index
        while (current < source.length && source[current].isWhitespace()) {
            current++
        }
        return current
    }

    private fun offsetRange(source: String, start: Int, end: Int): McTextRange =
        McTextRange(offsetToPosition(source, start), offsetToPosition(source, end))

    private fun offsetToPosition(source: String, offset: Int): McTextPosition {
        var line = 0
        var character = 0
        var index = 0
        while (index < offset && index < source.length) {
            if (source[index] == '\n') {
                line++
                character = 0
            } else {
                character++
            }
            index++
        }
        return McTextPosition(line, character)
    }

    private fun rangeEnd(source: String, range: McTextRange): Int? =
        AnnotationContextExtractor.toOffset(source, range.end.line, range.end.character)

    private fun Char.isJavaIdentifierStart(): Boolean = isLetter() || this == '_' || this == '$'

    private fun Char.isJavaIdentifierPart(): Boolean = isLetterOrDigit() || this == '_' || this == '$'
}
