package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.text.JavaStringContentDecoder

object DefinitionAnnotationParser {
    fun parse(body: String): MixinExtrasDefinition {
        var id: String? = null
        var idContentRange: IntRange? = null
        val methodReferences = mutableListOf<String>()
        val fieldReferences = mutableListOf<String>()
        val classLiteralTypeNames = mutableListOf<String>()
        val localSpecs = mutableListOf<HandlerParameterSugarSpec.Local>()
        var remap: Boolean? = null
        val parseIssues = mutableListOf<DefinitionAnnotationParseIssue>()

        var index = skipForwardWhitespaceAndComments(body, 0)
        while (index < body.length) {
            index = skipForwardWhitespaceAndComments(body, index)
            if (index >= body.length) break

            val attributeStart = index
            val attribute = readIdentifier(body, index) ?: break
            index += attribute.length
            index = skipForwardWhitespaceAndComments(body, index)
            if (body.getOrNull(index) != '=') {
                val valueEnd = skipAnnotationValue(body, index)
                parseIssues += DefinitionAnnotationParseIssue(
                    attribute = attribute,
                    rawValue = rawValue(body, index, valueEnd),
                    message = "expected '=' after attribute",
                    bodyRange = attributeStart until valueEnd,
                )
                index = valueEnd
                if (body.getOrNull(index) == ',') index++
                continue
            }
            index++
            index = skipForwardWhitespaceAndComments(body, index)
            val valueStart = index

            when (attribute) {
                "id" -> {
                    val literal = readStringLiteral(body, index)
                    if (literal != null) {
                        id = literal.value
                        idContentRange = (index + 1) until (literal.end - 1)
                        index = literal.end
                    } else {
                        index = malformedValue(
                            body,
                            valueStart,
                            attribute,
                            "expected a string literal",
                            parseIssues,
                        )
                    }
                }
                "method" -> {
                    val parsed = parseStringOrStringArray(body, index)
                    if (parsed != null) {
                        methodReferences += parsed.values
                        index = parsed.end
                    } else {
                        index = malformedValue(
                            body,
                            valueStart,
                            attribute,
                            "expected a string or string array",
                            parseIssues,
                        )
                    }
                }
                "field" -> {
                    val parsed = parseStringOrStringArray(body, index)
                    if (parsed != null) {
                        fieldReferences += parsed.values
                        index = parsed.end
                    } else {
                        index = malformedValue(
                            body,
                            valueStart,
                            attribute,
                            "expected a string or string array",
                            parseIssues,
                        )
                    }
                }
                "type" -> {
                    val parsed = parseClassLiteralOrArray(body, index)
                    if (parsed != null) {
                        classLiteralTypeNames += parsed.values
                        index = parsed.end
                    } else {
                        index = malformedValue(
                            body,
                            valueStart,
                            attribute,
                            "expected a class literal or class literal array",
                            parseIssues,
                        )
                    }
                }
                "remap" -> {
                    val parsed = readBooleanLiteral(body, index)
                    if (parsed != null) {
                        remap = parsed.value
                        index = parsed.end
                    } else {
                        index = malformedValue(
                            body,
                            valueStart,
                            attribute,
                            "expected a boolean literal",
                            parseIssues,
                        )
                    }
                }
                "local" -> {
                    val parsed = parseLocalOrLocalArray(body, index)
                    if (parsed != null) {
                        localSpecs += parsed.values
                        index = parsed.end
                    } else {
                        index = malformedValue(
                            body,
                            valueStart,
                            attribute,
                            "expected @Local or @Local array",
                            parseIssues,
                        )
                    }
                }
                else -> index = skipAnnotationValue(body, index)
            }

            index = skipForwardWhitespaceAndComments(body, index)
            if (body.getOrNull(index) == ',') {
                index++
            }
        }

        return MixinExtrasDefinition(
            id = id,
            rawMethodReferences = methodReferences,
            rawFieldReferences = fieldReferences,
            classLiteralTypeNames = classLiteralTypeNames,
            localSpecs = localSpecs,
            remap = remap,
        ).attachIdContentRange(idContentRange).withParseIssues(parseIssues).also { it.bodyLength = body.length }
    }

    private fun malformedValue(
        source: String,
        start: Int,
        attribute: String,
        message: String,
        issues: MutableList<DefinitionAnnotationParseIssue>,
    ): Int {
        val end = skipAnnotationValue(source, start)
        issues += DefinitionAnnotationParseIssue(
            attribute = attribute,
            rawValue = rawValue(source, start, end),
            message = message,
            bodyRange = start until end,
        )
        return end
    }

    private fun rawValue(source: String, start: Int, end: Int): String =
        source.substring(start.coerceIn(0, source.length), end.coerceIn(start, source.length)).trim()

    private fun parseLocalOrLocalArray(source: String, start: Int): ParsedValue<HandlerParameterSugarSpec.Local>? {
        val index = skipForwardWhitespaceAndComments(source, start)
        if (index >= source.length) return null
        return when (source[index]) {
            '@' -> readLocalAnnotation(source, index)?.let { ParsedValue(listOf(it.value), it.end) }
            '{' -> parseLocalAnnotationArray(source, index)
            else -> null
        }
    }

    private fun parseLocalAnnotationArray(source: String, openBrace: Int): ParsedValue<HandlerParameterSugarSpec.Local>? {
        val closeBrace = findMatchingCloseBrace(source, openBrace) ?: return null
        val elements = splitTopLevelCommas(source.substring(openBrace + 1, closeBrace))
        val values = mutableListOf<HandlerParameterSugarSpec.Local>()
        for (element in elements) {
            val trimmedStart = skipForwardWhitespaceAndComments(element, 0)
            if (trimmedStart >= element.length) {
                if (elements.size > 1) return null
                continue
            }
            val local = readLocalAnnotation(element, trimmedStart) ?: return null
            if (skipForwardWhitespaceAndComments(element, local.end) < element.length) return null
            values += local.value
        }
        return ParsedValue(values, closeBrace + 1)
    }

    private fun readLocalAnnotation(source: String, start: Int): ParsedScalar<HandlerParameterSugarSpec.Local>? {
        if (source.getOrNull(start) != '@') return null
        var index = start + 1
        val nameStart = index
        while (index < source.length && (source[index].isLetterOrDigit() || source[index] in "._$")) {
            index++
        }
        if (index == nameStart) return null
        val qualifiedName = source.substring(nameStart, index)
        if (!isLocalAnnotationName(qualifiedName)) return null

        index = skipForwardWhitespaceAndComments(source, index)
        if (source.getOrNull(index) == '(') {
            val closeParen = findMatchingCloseParen(source, index) ?: return null
            val body = source.substring(index + 1, closeParen)
            return ParsedScalar(parseLocalSugarSpec(body), closeParen + 1)
        }
        return ParsedScalar(HandlerParameterSugarSpec.Local(), index)
    }

    private fun isLocalAnnotationName(qualifiedName: String): Boolean =
        qualifiedName == "Local" || qualifiedName == "com.llamalad7.mixinextras.sugar.Local"

    private fun parseLocalSugarSpec(body: String): HandlerParameterSugarSpec.Local {
        var argsOnly = false
        var index: Int? = null
        var ordinal: Int? = null
        val names = mutableSetOf<String>()
        var print = false
        var typeClassName: String? = null
        if (body.isBlank()) {
            return HandlerParameterSugarSpec.Local()
        }
        for (member in splitTopLevelCommas(body)) {
            val trimmedStart = skipForwardWhitespaceAndComments(member, 0)
            if (trimmedStart >= member.length) continue
            val trimmed = member.substring(trimmedStart)
            val equalsIndex = findTopLevelEquals(trimmed)
            if (equalsIndex < 0) continue
            val key = trimmed.substring(0, equalsIndex).trim()
            val valueStart = skipForwardWhitespaceAndComments(trimmed, equalsIndex + 1)
            when (key) {
                "argsOnly" -> argsOnly = parseLocalBooleanLiteral(trimmed, valueStart)
                "index" -> index = parseLocalOptionalIntLiteral(trimmed, valueStart)
                "ordinal" -> ordinal = parseLocalOptionalIntLiteral(trimmed, valueStart)
                "name" -> names.addAll(parseLocalNameValue(trimmed, valueStart))
                "print" -> print = parseLocalBooleanLiteral(trimmed, valueStart)
                "type" -> readClassLiteral(trimmed, valueStart)?.value?.let { typeClassName = it }
            }
        }
        return HandlerParameterSugarSpec.Local(
            argsOnly = argsOnly,
            index = index,
            ordinal = ordinal,
            names = names,
            print = print,
            typeClassName = typeClassName,
        )
    }

    private fun findTopLevelEquals(value: String): Int {
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
                value[index] == '"' -> inString = true
                value[index] == '=' -> return index
            }
            index++
        }
        return -1
    }

    private fun parseLocalBooleanLiteral(source: String, start: Int): Boolean {
        val index = skipForwardWhitespaceAndComments(source, start)
        if (index >= source.length) return false
        when {
            source.startsWith("true", index) && !source.getOrNull(index + 4).isIdentifierPart() ->
                return true
            source.startsWith("false", index) && !source.getOrNull(index + 5).isIdentifierPart() ->
                return false
            else -> return false
        }
    }

    private fun parseLocalOptionalIntLiteral(source: String, start: Int): Int? {
        val index = skipForwardWhitespaceAndComments(source, start)
        if (index >= source.length) return null
        var end = index
        if (source[end] == '-' && end + 1 < source.length) end++
        while (end < source.length && source[end].isDigit()) end++
        if (end == index || (source[index] == '-' && end == index + 1)) return null
        if (source.getOrNull(end).isIdentifierPart()) return null
        val parsed = source.substring(index, end).toIntOrNull() ?: return null
        return if (parsed == -1) null else parsed
    }

    private fun parseLocalNameValue(source: String, start: Int): Set<String> {
        val parsed = parseStringOrStringArray(source, start) ?: return emptySet()
        return parsed.values.toSet()
    }

    private data class ParsedValue<T>(val values: List<T>, val end: Int)

    private data class ParsedScalar<T>(val value: T, val end: Int)

    private data class StringLiteral(val value: String, val end: Int)

    private val PRIMITIVE_CLASS_LITERAL_NAMES = setOf(
        "void", "boolean", "byte", "char", "short", "int", "long", "float", "double",
    )

    private fun parseStringOrStringArray(source: String, start: Int): ParsedValue<String>? {
        val index = skipForwardWhitespaceAndComments(source, start)
        if (index >= source.length) return null
        return when (source[index]) {
            '"' -> readStringLiteral(source, index)?.let { ParsedValue(listOf(it.value), it.end) }
            '{' -> parseStringArray(source, index)
            else -> null
        }
    }

    private fun parseStringArray(source: String, openBrace: Int): ParsedValue<String>? {
        val closeBrace = findMatchingCloseBrace(source, openBrace) ?: return null
        val elements = splitTopLevelCommas(source.substring(openBrace + 1, closeBrace))
        val values = mutableListOf<String>()
        for (element in elements) {
            val trimmedStart = skipForwardWhitespaceAndComments(element, 0)
            if (trimmedStart >= element.length) {
                if (elements.size > 1) return null
                continue
            }
            val literal = readStringLiteral(element, trimmedStart) ?: return null
            if (skipForwardWhitespaceAndComments(element, literal.end) < element.length) return null
            values += literal.value
        }
        return ParsedValue(values, closeBrace + 1)
    }

    private fun parseClassLiteralOrArray(source: String, start: Int): ParsedValue<String>? {
        val index = skipForwardWhitespaceAndComments(source, start)
        if (index >= source.length) return null
        return when (source[index]) {
            '{' -> parseClassLiteralArray(source, index)
            else -> readClassLiteral(source, index)?.let { ParsedValue(listOf(it.value), it.end) }
        }
    }

    private fun parseClassLiteralArray(source: String, openBrace: Int): ParsedValue<String>? {
        val closeBrace = findMatchingCloseBrace(source, openBrace) ?: return null
        val elements = splitTopLevelCommas(source.substring(openBrace + 1, closeBrace))
        val values = mutableListOf<String>()
        for (element in elements) {
            val trimmedStart = skipForwardWhitespaceAndComments(element, 0)
            if (trimmedStart >= element.length) {
                if (elements.size > 1) return null
                continue
            }
            val literal = readClassLiteral(element, trimmedStart) ?: return null
            if (skipForwardWhitespaceAndComments(element, literal.end) < element.length) return null
            values += literal.value
        }
        return ParsedValue(values, closeBrace + 1)
    }

    private fun readClassLiteral(source: String, start: Int): ParsedScalar<String>? {
        val index = skipForwardWhitespaceAndComments(source, start)
        if (index >= source.length) return null

        val parsedType = readClassLiteralTypeName(source, index) ?: return null
        var end = parsedType.end
        end = skipForwardWhitespaceAndComments(source, end)
        if (!source.startsWith(".class", end)) return null
        end += ".class".length
        if (!isValidClassLiteralTypeName(parsedType.value)) return null
        return ParsedScalar(parsedType.value, end)
    }

    private fun readClassLiteralTypeName(source: String, start: Int): ParsedScalar<String>? {
        var index = skipForwardWhitespaceAndComments(source, start)
        if (index >= source.length) return null

        val first = readIdentifier(source, index) ?: return null
        index += first.length
        val builder = StringBuilder(first)

        if (first !in PRIMITIVE_CLASS_LITERAL_NAMES) {
            while (true) {
                index = skipForwardWhitespaceAndComments(source, index)
                if (source.getOrNull(index) != '.') break
                if (isTerminalClassLiteralSuffix(source, index)) break
                index++
                index = skipForwardWhitespaceAndComments(source, index)
                val part = readIdentifier(source, index) ?: return null
                builder.append('.').append(part)
                index += part.length
            }
        }

        index = appendArraySuffixes(source, index, builder)
        val typeName = builder.toString()
        if (typeName.isEmpty()) return null
        return ParsedScalar(typeName, index)
    }

    private fun isTerminalClassLiteralSuffix(source: String, dotIndex: Int): Boolean {
        if (!source.startsWith(".class", dotIndex)) return false
        val charAfterClass = source.getOrNull(dotIndex + ".class".length)
        return charAfterClass == null || !charAfterClass.isJavaIdentifierPart()
    }

    private fun appendArraySuffixes(source: String, start: Int, builder: StringBuilder): Int {
        var index = start
        while (true) {
            index = skipForwardWhitespaceAndComments(source, index)
            if (source.startsWith("[]", index)) {
                builder.append("[]")
                index += 2
            } else {
                break
            }
        }
        return index
    }

    private fun isValidClassLiteralTypeName(name: String): Boolean {
        val base = name.substringBefore('[')
        if (base in PRIMITIVE_CLASS_LITERAL_NAMES) {
            return name.drop(base.length).all { it == '[' || it == ']' } &&
                name.drop(base.length).length % 2 == 0
        }
        val segments = base.split('$', '.')
        if (segments.any { it.isEmpty() }) return false
        for (segment in segments) {
            if (!segment.first().let { it.isLetter() || it == '_' }) return false
            if (segment.drop(1).any { !(it.isLetterOrDigit() || it == '_' || it == '$') }) return false
        }
        val suffix = name.drop(base.length)
        return suffix.all { it == '[' || it == ']' } && suffix.length % 2 == 0
    }

    private fun readBooleanLiteral(source: String, start: Int): ParsedScalar<Boolean>? {
        val index = skipForwardWhitespaceAndComments(source, start)
        if (index >= source.length) return null
        when {
            source.startsWith("true", index) && !source.getOrNull(index + 4).isIdentifierPart() ->
                return ParsedScalar(true, index + 4)
            source.startsWith("false", index) && !source.getOrNull(index + 5).isIdentifierPart() ->
                return ParsedScalar(false, index + 5)
            else -> return null
        }
    }

    private fun readStringLiteral(source: String, start: Int): StringLiteral? {
        if (source.getOrNull(start) != '"') return null
        val contentStart = start + 1
        var index = contentStart
        while (index < source.length) {
            when (source[index]) {
                '"' -> {
                    val value = JavaStringContentDecoder.decodeContent(source, contentStart, index) ?: return null
                    return StringLiteral(value, index + 1)
                }
                '\\' -> {
                    val decoded = JavaStringContentDecoder.decodeEscape(source, index, source.length) ?: return null
                    index = decoded.nextIndex
                }
                else -> index++
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

    private fun skipAnnotationValue(source: String, start: Int): Int {
        var index = skipForwardWhitespaceAndComments(source, start)
        if (index >= source.length) return index
        return when (source[index]) {
            '"' -> readStringLiteral(source, index)?.end ?: (index + 1)
            '\'' -> skipUnsupportedSingleQuotedLiteral(source, index)
            '{' -> findMatchingCloseBrace(source, index)?.plus(1) ?: (index + 1)
            '(' -> findMatchingCloseParen(source, index)?.plus(1) ?: (index + 1)
            '@' -> skipNestedAnnotation(source, index)
            else -> skipBareAnnotationValue(source, index)
        }
    }

    /** Single quotes are invalid annotation string syntax, but still form one recoverable value. */
    private fun skipUnsupportedSingleQuotedLiteral(source: String, start: Int): Int {
        var index = start + 1
        while (index < source.length) {
            when (source[index]) {
                '\\' -> index = (index + 2).coerceAtMost(source.length)
                '\'' -> return index + 1
                else -> index++
            }
        }
        return source.length
    }

    private fun skipNestedAnnotation(source: String, start: Int): Int {
        if (source.getOrNull(start) != '@') return start + 1
        var index = start + 1
        while (index < source.length && (source[index].isLetterOrDigit() || source[index] in "._$")) {
            index++
        }
        if (source.getOrNull(index) != '(') return index
        return findMatchingCloseParen(source, index)?.plus(1) ?: (index + 1)
    }

    private fun skipBareAnnotationValue(source: String, start: Int): Int {
        var index = start
        var genericDepth = 0
        var parenDepth = 0
        while (index < source.length) {
            when {
                isLineCommentStart(source, index) -> index = skipLineComment(source, index)
                isBlockCommentStart(source, index) -> index = skipBlockComment(source, index)
                source[index] == '<' -> {
                    genericDepth++
                    index++
                }
                source[index] == '>' -> {
                    genericDepth = (genericDepth - 1).coerceAtLeast(0)
                    index++
                }
                source[index] == '(' -> {
                    parenDepth++
                    index++
                }
                source[index] == ')' -> {
                    if (parenDepth == 0 && genericDepth == 0) return index
                    parenDepth = (parenDepth - 1).coerceAtLeast(0)
                    index++
                }
                source[index] == ',' && genericDepth == 0 && parenDepth == 0 -> return index
                else -> index++
            }
        }
        return index
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

    private fun Char?.isIdentifierPart(): Boolean = this != null && this.isJavaIdentifierPart()
}
