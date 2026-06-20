package io.github.mcdev.core.mixin

object AnnotationContextExtractor {
    private val injectorAnnotations = setOf(
        MixinAnnotation.INJECT,
        MixinAnnotation.REDIRECT,
        MixinAnnotation.MODIFY_ARG,
        MixinAnnotation.MODIFY_ARGS,
        MixinAnnotation.MODIFY_VARIABLE,
        MixinAnnotation.MODIFY_CONSTANT,
        MixinAnnotation.MODIFY_EXPRESSION_VALUE,
        MixinAnnotation.MODIFY_RETURN_VALUE,
        MixinAnnotation.MODIFY_RECEIVER,
        MixinAnnotation.WRAP_OPERATION,
        MixinAnnotation.WRAP_WITH_CONDITION,
        MixinAnnotation.WRAP_METHOD,
    )

    fun extract(source: String, line: Int, character: Int): AnnotationContext? {
        val cursorOffset = toOffset(source, line, character) ?: return null
        return extractAtOffset(source, cursorOffset)
    }

    fun extractAtOffset(source: String, cursorOffset: Int): AnnotationContext? {
        if (cursorOffset < 0 || cursorOffset > source.length) return null
        val mixinTargets = resolveMixinTargets(source, cursorOffset)
        val annotationStart = findEnclosingAnnotationStart(source, cursorOffset) ?: return null
        val annotation = parseAnnotationName(source, annotationStart) ?: return null
        val annotationEndWithoutParens = skipAnnotationName(source, annotationStart)
        val parenStart = annotationEndWithoutParens.takeIf { source.getOrNull(it) == '(' }
        val bodyStart = parenStart ?: annotationEndWithoutParens
        val bodyEnd = if (parenStart != null) {
            findMatchingParen(source, parenStart) ?: source.length
        } else {
            annotationEndWithoutParens
        }
        val annotationEnd = if (parenStart != null) bodyEnd + 1 else annotationEndWithoutParens
        if (cursorOffset <= bodyStart) {
            if (annotation == MixinAnnotation.MIXIN) {
                return buildClassSlotContext(source, annotation, annotationStart, bodyStart, cursorOffset, mixinTargets)
            }
            if (annotation == MixinAnnotation.OVERWRITE) {
                return AnnotationContext(
                    annotation = annotation,
                    slot = AnnotationSlot.OVERWRITE_METHOD,
                    partialValue = "",
                    valueStartOffset = bodyEnd,
                    valueEndOffset = bodyEnd,
                    annotationStartOffset = annotationStart,
                    annotationEndOffset = annotationEnd,
                    mixinTargetInternalNames = mixinTargets,
                )
            }
            return null
        }
        val slotContext = findSlotAtCursor(source, annotation, bodyStart, bodyEnd, cursorOffset)
            ?: if (annotation == MixinAnnotation.MIXIN && cursorOffset in (bodyStart + 1)..bodyEnd) {
                buildClassSlotContext(source, annotation, annotationStart, bodyStart, cursorOffset, mixinTargets)
            } else {
                return null
            }
        val context = slotContext ?: return null
        val enrichedContext = if (annotation == MixinAnnotation.AT) {
            context.enrichAtContextFromParentInjector(source, annotationStart)
        } else {
            context
        }
        return enrichedContext.copy(
            annotationStartOffset = annotationStart,
            annotationEndOffset = annotationEnd,
            mixinTargetInternalNames = mixinTargets,
        )
    }

    private fun buildClassSlotContext(
        source: String,
        annotation: MixinAnnotation,
        annotationStart: Int,
        bodyStart: Int,
        cursorOffset: Int,
        mixinTargets: List<String>,
    ): AnnotationContext? {
        if (annotation != MixinAnnotation.MIXIN) return null
        val afterParen = bodyStart + 1
        if (cursorOffset < afterParen) return null
        val partial = extractPartialClassReference(source, afterParen, cursorOffset)
        return AnnotationContext(
            annotation = annotation,
            slot = AnnotationSlot.CLASS,
            partialValue = partial,
            valueStartOffset = afterParen,
            valueEndOffset = cursorOffset,
            annotationStartOffset = annotationStart,
            annotationEndOffset = bodyStart + 1,
            mixinTargetInternalNames = mixinTargets,
        )
    }

    private fun findEnclosingAnnotationStart(source: String, cursorOffset: Int): Int? {
        var searchFrom = cursorOffset
        while (searchFrom >= 0) {
            val at = source.lastIndexOf('@', searchFrom)
            if (at < 0) return null
            val annotation = parseAnnotationName(source, at)
            if (annotation != null) {
                val nameEnd = skipAnnotationName(source, at)
                val immediateParen = if (source.getOrNull(nameEnd) == '(') nameEnd else -1
                if (immediateParen < 0) {
                    if (annotationSupportsBareForm(annotation) && cursorOffset >= at) {
                        return at
                    }
                    searchFrom = at - 1
                    continue
                }
                if (immediateParen > cursorOffset) {
                    searchFrom = at - 1
                    continue
                }
                val close = findMatchingParen(source, immediateParen)
                if (close == null || cursorOffset <= close) {
                    return at
                }
                if (
                    annotationSupportsBareForm(annotation) &&
                    isCursorInAnnotatedMemberDeclaration(source, close + 1, cursorOffset)
                ) {
                    return at
                }
            }
            searchFrom = at - 1
        }
        return null
    }

    private fun annotationSupportsBareForm(annotation: MixinAnnotation): Boolean =
        annotation == MixinAnnotation.SHADOW || annotation == MixinAnnotation.OVERWRITE

    private fun isCursorInAnnotatedMemberDeclaration(source: String, annotationEnd: Int, cursorOffset: Int): Boolean {
        if (cursorOffset < annotationEnd || cursorOffset > source.length) return false
        val between = source.substring(annotationEnd, cursorOffset)
        return between.none { it == ';' || it == '{' || it == '}' }
    }

    private fun skipAnnotationName(source: String, atOffset: Int): Int {
        if (source.getOrNull(atOffset) != '@') return atOffset
        var end = atOffset + 1
        while (end < source.length && isAnnotationNameChar(source[end])) {
            end++
        }
        return end
    }

    private fun parseAnnotationName(source: String, atOffset: Int): MixinAnnotation? {
        if (source.getOrNull(atOffset) != '@') return null
        val nameEnd = skipAnnotationName(source, atOffset)
        if (nameEnd <= atOffset + 1) return null
        val qualifiedName = source.substring(atOffset + 1, nameEnd)
        return MixinAnnotation.fromSimpleName(qualifiedName.substringAfterLast('.'))
    }

    private fun isAnnotationNameChar(ch: Char): Boolean =
        ch.isLetterOrDigit() || ch == '_' || ch == '.'

    private fun findSlotAtCursor(
        source: String,
        annotation: MixinAnnotation,
        bodyStart: Int,
        bodyEnd: Int,
        cursorOffset: Int,
    ): AnnotationContext? {
        var index = bodyStart + 1
        while (index < bodyEnd) {
            index = skipWhitespace(source, index, bodyEnd)
            if (index >= bodyEnd) break
            val shorthand = readShorthandValue(source, index, bodyEnd)
            if (shorthand != null) {
                if (cursorOffset in shorthand.contentStart..shorthand.contentEnd) {
                    return buildContextForShorthand(annotation, shorthand, source, bodyStart, bodyEnd, cursorOffset)
                }
                index = shorthand.end
                if (index < bodyEnd && source[index] == ',') index++
                continue
            }
            if (source[index] == '@') {
                val nestedStart = index
                val nameEnd = skipAnnotationName(source, nestedStart)
                val nestedParen = if (source.getOrNull(nameEnd) == '(') nameEnd else break
                val nestedClose = findMatchingParen(source, nestedParen) ?: break
                if (cursorOffset in nestedParen..nestedClose) {
                    val nestedAnnotation = parseAnnotationName(source, nestedStart) ?: return null
                    return findSlotAtCursor(source, nestedAnnotation, nestedParen, nestedClose, cursorOffset)
                }
                index = nestedClose + 1
                continue
            }
            val attrName = readIdentifier(source, index, bodyEnd) ?: break
            index += attrName.length
            index = skipWhitespace(source, index, bodyEnd)
            if (index >= bodyEnd || source[index] != '=') break
            index++
            index = skipWhitespace(source, index, bodyEnd)
            if (index >= bodyEnd) break

            val valueInfo = readValue(source, index, bodyEnd) ?: break
            if (cursorOffset in valueInfo.contentStart..valueInfo.contentEnd) {
                return buildContextForAttribute(
                    annotation,
                    attrName,
                    valueInfo,
                    source,
                    bodyStart,
                    bodyEnd,
                    cursorOffset,
                )
            }
            index = valueInfo.end
            if (index < bodyEnd && source[index] == ',') {
                index++
            }
        }

        if (annotation == MixinAnnotation.SHADOW && cursorOffset > bodyStart) {
            return buildBareMemberContext(source, annotation, AnnotationSlot.SHADOW_MEMBER, bodyEnd, cursorOffset)
        }
        if (annotation == MixinAnnotation.OVERWRITE && cursorOffset > bodyStart) {
            return buildBareMemberContext(source, annotation, AnnotationSlot.OVERWRITE_METHOD, bodyEnd, cursorOffset)
        }
        return null
    }

    private fun buildBareMemberContext(
        source: String,
        annotation: MixinAnnotation,
        slot: AnnotationSlot,
        annotationBodyEnd: Int,
        cursorOffset: Int,
    ): AnnotationContext {
        val safeCursor = cursorOffset.coerceIn(0, source.length)
        val tokenStart = javaIdentifierStartBefore(source, safeCursor)
        val start = if (tokenStart >= annotationBodyEnd) tokenStart else safeCursor
        return AnnotationContext(
            annotation = annotation,
            slot = slot,
            partialValue = source.substring(start, safeCursor),
            valueStartOffset = start,
            valueEndOffset = safeCursor,
            annotationStartOffset = 0,
            annotationEndOffset = 0,
        )
    }

    private fun AnnotationContext.enrichAtContextFromParentInjector(
        source: String,
        atAnnotationStart: Int,
    ): AnnotationContext {
        if (annotation != MixinAnnotation.AT) return this
        val parent = findParentInjectorBody(source, atAnnotationStart) ?: return this
        return copy(
            injectMethodName = injectMethodName ?: findMethodAttribute(source, parent.bodyStart, parent.bodyEnd),
        )
    }

    private data class AnnotationBody(
        val bodyStart: Int,
        val bodyEnd: Int,
    )

    private fun findParentInjectorBody(source: String, nestedAnnotationStart: Int): AnnotationBody? {
        var searchFrom = nestedAnnotationStart - 1
        while (searchFrom >= 0) {
            val at = source.lastIndexOf('@', searchFrom)
            if (at < 0) return null
            val annotation = parseAnnotationName(source, at)
            val nameEnd = skipAnnotationName(source, at)
            val paren = if (source.getOrNull(nameEnd) == '(') nameEnd else -1
            if (annotation in injectorAnnotations && paren >= 0) {
                val close = findMatchingParen(source, paren) ?: source.length
                if (nestedAnnotationStart in paren..close) {
                    return AnnotationBody(paren + 1, close)
                }
            }
            searchFrom = at - 1
        }
        return null
    }

    private fun javaIdentifierStartBefore(source: String, cursorOffset: Int): Int {
        var index = cursorOffset.coerceIn(0, source.length)
        while (index > 0 && isJavaIdentifierPart(source[index - 1])) {
            index--
        }
        return index
    }

    private fun readShorthandValue(source: String, start: Int, bodyEnd: Int): ValueInfo? {
        if (start >= bodyEnd) return null
        if (source[start] == '"') return readValue(source, start, bodyEnd)
        val classRef = readClassReference(source, start, bodyEnd) ?: return null
        return ValueInfo(classRef.contentStart, classRef.contentEnd, classRef.replaceStart, classRef.end)
    }

    private fun readClassReference(source: String, start: Int, bodyEnd: Int): ValueInfo? {
        if (start >= bodyEnd || !isJavaIdentifierStart(source[start])) return null
        var i = start + 1
        while (i < bodyEnd) {
            when {
                source[i].isLetterOrDigit() || source[i] == '_' -> i++
                source[i] == '.' -> {
                    if (isClassLiteralSuffix(source, i, bodyEnd)) break
                    if (i + 1 < bodyEnd && isJavaIdentifierStart(source[i + 1])) i++
                    else break
                }
                else -> break
            }
        }
        val next = skipWhitespace(source, i, bodyEnd)
        if (next >= bodyEnd || !source.startsWith(".class", next)) return null
        val end = next + ".class".length
        return ValueInfo(start, end, start, end)
    }

    private fun isClassLiteralSuffix(source: String, dotIndex: Int, bodyEnd: Int): Boolean {
        if (!source.startsWith(".class", dotIndex)) return false
        val after = dotIndex + ".class".length
        if (after >= bodyEnd) return true
        return when (source[after]) {
            ',', ')', '}' -> true
            else -> source[after].isWhitespace()
        }
    }

    private fun isJavaIdentifierStart(ch: Char): Boolean = ch.isLetter() || ch == '_'

    private fun isJavaIdentifierPart(ch: Char): Boolean = ch.isLetterOrDigit() || ch == '_' || ch == '$'

    private fun buildContextForShorthand(
        annotation: MixinAnnotation,
        valueInfo: ValueInfo,
        source: String,
        bodyStart: Int,
        bodyEnd: Int,
        cursorOffset: Int,
    ): AnnotationContext {
        val partial = source.substring(valueInfo.contentStart, cursorOffset.coerceAtMost(valueInfo.contentEnd))
        val slot = when (annotation) {
            MixinAnnotation.ACCESSOR -> AnnotationSlot.ACCESSOR_VALUE
            MixinAnnotation.INVOKER -> AnnotationSlot.INVOKER_VALUE
            MixinAnnotation.AT -> AnnotationSlot.VALUE
            MixinAnnotation.MIXIN -> AnnotationSlot.CLASS
            else -> AnnotationSlot.VALUE
        }
        val cleanedPartial = if (slot == AnnotationSlot.CLASS) classSlotPartial(partial) else partial.trim('"')
        return AnnotationContext(
            annotation = annotation,
            slot = slot,
            partialValue = cleanedPartial,
            valueStartOffset = valueInfo.replaceStart,
            valueEndOffset = cursorOffset,
            annotationStartOffset = 0,
            annotationEndOffset = 0,
            injectMethodName = findMethodAttribute(source, bodyStart, bodyEnd),
            atValue = if (annotation == MixinAnnotation.AT) partial.trim('"') else findAtValue(source, bodyStart, bodyEnd),
        )
    }

    private fun buildContextForAttribute(
        annotation: MixinAnnotation,
        attrName: String,
        valueInfo: ValueInfo,
        source: String,
        bodyStart: Int,
        bodyEnd: Int,
        cursorOffset: Int,
    ): AnnotationContext {
        val partial = source.substring(valueInfo.contentStart, cursorOffset.coerceAtMost(valueInfo.contentEnd))
        val injectMethod = if (annotation in injectorAnnotations && attrName == "method") {
            partial.trim('"')
        } else {
            findMethodAttribute(source, bodyStart, bodyEnd)
        }
        val atValue = when {
            annotation == MixinAnnotation.AT && attrName == "value" -> partial.trim('"')
            annotation == MixinAnnotation.AT -> findAtValueInAtBody(source, bodyStart, bodyEnd)
            else -> findAtValue(source, bodyStart, bodyEnd)
        }
        val slot = when {
            annotation == MixinAnnotation.MIXIN && attrName in setOf("targets", "target") -> AnnotationSlot.TARGETS
            annotation == MixinAnnotation.MIXIN && attrName == "value" -> AnnotationSlot.CLASS
            annotation in injectorAnnotations && attrName == "method" -> AnnotationSlot.METHOD
            annotation == MixinAnnotation.AT && attrName == "value" -> AnnotationSlot.VALUE
            annotation == MixinAnnotation.AT && attrName == "target" -> AnnotationSlot.TARGET
            annotation == MixinAnnotation.ACCESSOR -> AnnotationSlot.ACCESSOR_VALUE
            annotation == MixinAnnotation.INVOKER -> AnnotationSlot.INVOKER_VALUE
            annotation == MixinAnnotation.SHADOW && attrName == "prefix" -> AnnotationSlot.PREFIX
            annotation == MixinAnnotation.SHADOW && attrName == "remap" -> AnnotationSlot.REMAP
            else -> AnnotationSlot.VALUE
        }
        val shadowPrefix = if (annotation == MixinAnnotation.SHADOW) {
            findStringAttribute(source, bodyStart, bodyEnd, "prefix")
        } else {
            null
        }
        val shadowRemap = if (annotation == MixinAnnotation.SHADOW) {
            findBooleanAttribute(source, bodyStart, bodyEnd, "remap") ?: true
        } else {
            true
        }
        val cleanedPartial = if (slot == AnnotationSlot.CLASS) classSlotPartial(partial) else partial
        return AnnotationContext(
            annotation = annotation,
            slot = slot,
            partialValue = cleanedPartial,
            valueStartOffset = valueInfo.replaceStart,
            valueEndOffset = cursorOffset,
            annotationStartOffset = 0,
            annotationEndOffset = 0,
            injectMethodName = injectMethod,
            atValue = atValue,
            shadowPrefix = shadowPrefix,
            shadowRemap = shadowRemap,
        )
    }

    private data class ValueInfo(
        val contentStart: Int,
        val contentEnd: Int,
        val replaceStart: Int,
        val end: Int,
    )

    private fun readValue(source: String, start: Int, bodyEnd: Int): ValueInfo? {
        if (start >= bodyEnd) return null
        return when (source[start]) {
            '"' -> {
                val contentStart = start + 1
                var i = contentStart
                while (i < bodyEnd) {
                    if (source[i] == '\\') {
                        i += 2
                        continue
                    }
                    if (source[i] == '"') {
                        return ValueInfo(contentStart, i, start, i + 1)
                    }
                    i++
                }
                ValueInfo(contentStart, bodyEnd.coerceAtMost(source.length), start, bodyEnd.coerceAtMost(source.length))
            }
            '{' -> {
                val close = findMatchingBrace(source, start) ?: bodyEnd
                ValueInfo(start + 1, close, start, close + 1)
            }
            else -> {
                val end = readBareValueEnd(source, start, bodyEnd)
                ValueInfo(start, end, start, end)
            }
        }
    }

    private fun readBareValueEnd(source: String, start: Int, bodyEnd: Int): Int {
        var i = start
        while (i < bodyEnd) {
            val ch = source[i]
            if (ch == ',' || ch == ')') break
            i++
        }
        return i
    }

    private fun extractPartialClassReference(source: String, start: Int, cursorOffset: Int): String {
        val slice = source.substring(start, cursorOffset)
        val cleaned = slice
            .replace(".class", "")
            .replace(Regex("[\\s{},]"), "")
        val parts = cleaned.split('.')
        return parts.lastOrNull { it.isNotEmpty() } ?: cleaned
    }

    fun resolveMixinTargets(source: String, cursorOffset: Int): List<String> =
        resolveRawMixinTargets(source, cursorOffset)

    fun resolveRawMixinTargets(source: String, cursorOffset: Int): List<String> {
        val classDecl = findEnclosingClassDeclaration(source, cursorOffset) ?: return emptyList()
        val mixinAnnotation = findMixinAnnotationOnClass(source, classDecl) ?: return emptyList()
        return parseMixinTargetValues(source, mixinAnnotation)
    }

    private fun findEnclosingClassDeclaration(source: String, cursorOffset: Int): Int? {
        val before = source.substring(0, cursorOffset)
        val declPattern = Regex(
            """\b(?:public\s+|private\s+|protected\s+)?(?:abstract\s+|final\s+|static\s+)?(?:class|interface|enum|record)\s+\w+""",
        )
        return declPattern.findAll(before).lastOrNull()?.range?.first
    }

    private fun findMixinAnnotationOnClass(source: String, classDeclOffset: Int): Int? {
        var pos = classDeclOffset
        var mixinAt: Int? = null
        while (pos > 0) {
            pos = skipBackwardWhitespaceAndComments(source, pos)
            if (pos <= 0) break
            val annotationStart = findAnnotationImmediatelyBefore(source, pos) ?: break
            if (parseAnnotationName(source, annotationStart) == MixinAnnotation.MIXIN) {
                mixinAt = annotationStart
            }
            pos = annotationStart
        }
        return mixinAt
    }

    fun findAnnotationOffsets(source: String, annotation: MixinAnnotation): List<Int> {
        val results = mutableListOf<Int>()
        var search = 0
        while (search < source.length) {
            val at = source.indexOf('@', search)
            if (at < 0) break
            if (parseAnnotationName(source, at) == annotation) {
                results += at
            }
            search = at + 1
        }
        return results
    }

    fun annotationEndOffset(source: String, atOffset: Int): Int {
        val nameEnd = skipAnnotationName(source, atOffset)
        return if (source.getOrNull(nameEnd) == '(') {
            findMatchingParen(source, nameEnd)?.plus(1) ?: nameEnd
        } else {
            nameEnd
        }
    }

    fun parseMixinTargetValues(source: String, mixinAtOffset: Int): List<String> {
        val paren = source.indexOf('(', mixinAtOffset)
        if (paren < 0) return emptyList()
        val close = findMatchingParen(source, paren) ?: return emptyList()
        return parseMixinTargetBody(source, paren + 1, close)
    }

    private fun parseMixinTargetBody(source: String, bodyStart: Int, bodyEnd: Int): List<String> {
        val results = mutableListOf<String>()
        var index = bodyStart
        while (index < bodyEnd) {
            index = skipWhitespace(source, index, bodyEnd)
            if (index >= bodyEnd) break

            val shorthand = readShorthandValue(source, index, bodyEnd)
            if (shorthand != null) {
                results += classRefToTarget(source.substring(shorthand.contentStart, shorthand.contentEnd))
                index = shorthand.end
                if (index < bodyEnd && source[index] == ',') index++
                continue
            }

            if (source[index] == '{') {
                val arrayClose = findMatchingBrace(source, index) ?: break
                results += parseMixinTargetArray(source, index + 1, arrayClose)
                index = arrayClose + 1
                if (index < bodyEnd && source[index] == ',') index++
                continue
            }

            val attrName = readIdentifier(source, index, bodyEnd) ?: break
            if (attrName !in setOf("value", "targets", "target")) {
                index = skipUnknownAttribute(source, index, bodyEnd) ?: break
                continue
            }
            index += attrName.length
            index = skipWhitespace(source, index, bodyEnd)
            if (index >= bodyEnd || source[index] != '=') break
            index++
            index = skipWhitespace(source, index, bodyEnd)
            if (index >= bodyEnd) break

            when {
                source[index].isLetter() || source[index] == '_' -> {
                    val valueInfo = readClassReference(source, index, bodyEnd) ?: break
                    results += classRefToTarget(source.substring(valueInfo.contentStart, valueInfo.contentEnd))
                    index = valueInfo.end
                }
                source[index] == '"' -> {
                    val valueInfo = readValue(source, index, bodyEnd) ?: break
                    results += source.substring(valueInfo.contentStart, valueInfo.contentEnd)
                    index = valueInfo.end
                }
                source[index] == '{' -> {
                    val arrayClose = findMatchingBrace(source, index) ?: break
                    results += parseMixinTargetArray(source, index + 1, arrayClose)
                    index = arrayClose + 1
                }
                else -> break
            }
            if (index < bodyEnd && source[index] == ',') index++
        }
        return results
    }

    private fun parseMixinTargetArray(source: String, start: Int, end: Int): List<String> {
        val results = mutableListOf<String>()
        var index = start
        while (index < end) {
            index = skipWhitespace(source, index, end)
            if (index >= end) break
            val shorthand = readShorthandValue(source, index, end)
            if (shorthand != null) {
                results += classRefToTarget(source.substring(shorthand.contentStart, shorthand.contentEnd))
                index = shorthand.end
            } else if (source[index] == '"') {
                val valueInfo = readValue(source, index, end) ?: break
                results += source.substring(valueInfo.contentStart, valueInfo.contentEnd)
                index = valueInfo.end
            } else {
                break
            }
            if (index < end && source[index] == ',') index++
        }
        return results
    }

    private fun skipUnknownAttribute(source: String, start: Int, bodyEnd: Int): Int? {
        val attrName = readIdentifier(source, start, bodyEnd) ?: return null
        var index = start + attrName.length
        index = skipWhitespace(source, index, bodyEnd)
        if (index >= bodyEnd || source[index] != '=') return null
        index++
        index = skipWhitespace(source, index, bodyEnd)
        if (index >= bodyEnd) return null
        val valueInfo = readValue(source, index, bodyEnd) ?: return null
        index = valueInfo.end
        if (index < bodyEnd && source[index] == ',') index++
        return index
    }

    private fun classRefToTarget(text: String): String {
        val cleaned = stripClassSuffix(text.trim())
        return if ('.' in cleaned) fqnToInternal(cleaned) else cleaned
    }

    private fun skipBackwardWhitespaceAndComments(source: String, end: Int): Int {
        var pos = end
        while (pos > 0) {
            while (pos > 0 && source[pos - 1].isWhitespace()) pos--
            if (pos <= 0) break
            if (pos >= 2 && source[pos - 1] == '/' && source[pos - 2] == '*') {
                val open = source.lastIndexOf("/*", pos - 2)
                if (open < 0) break
                pos = open
                continue
            }
            val lineStart = source.lastIndexOf('\n', pos - 1).let { if (it < 0) 0 else it + 1 }
            if (source.substring(lineStart, pos).trimStart().startsWith("//")) {
                pos = lineStart
                continue
            }
            break
        }
        return pos
    }

    private fun findAnnotationImmediatelyBefore(source: String, end: Int): Int? {
        var pos = end
        while (pos > 0 && source[pos - 1].isWhitespace()) pos--
        if (pos <= 0) return null

        if (source.getOrNull(pos - 1) != ')') {
            val at = source.lastIndexOf('@', pos - 1)
            if (at >= 0) {
                val nameEnd = skipAnnotationName(source, at)
                if (source.getOrNull(nameEnd) == '(') {
                    val close = findMatchingParen(source, nameEnd)
                    if (close != null && pos <= close) {
                        pos = close + 1
                        while (pos > 0 && source[pos - 1].isWhitespace()) pos--
                    }
                }
            }
        }
        if (pos <= 0) return null

        val annotationEnd = when {
            source[pos - 1] == ')' -> {
                val close = pos - 1
                val open = findMatchingOpenParen(source, close) ?: return null
                val at = source.lastIndexOf('@', open)
                if (at < 0) return null
                val nameEnd = skipAnnotationName(source, at)
                if (source.getOrNull(nameEnd) != '(' || nameEnd != open) return null
                close + 1
            }
            else -> {
                val at = source.lastIndexOf('@', pos - 1)
                if (at < 0) return null
                val nameEnd = skipAnnotationName(source, at)
                if (source.getOrNull(nameEnd) == '(') return null
                if (nameEnd != pos) return null
                pos
            }
        }

        val at = source.lastIndexOf('@', annotationEnd - 1)
        if (at < 0) return null
        val nameEnd = skipAnnotationName(source, at)
        val computedEnd = if (source.getOrNull(nameEnd) == '(') {
            findMatchingParen(source, nameEnd)?.plus(1) ?: return null
        } else {
            nameEnd
        }
        if (computedEnd > annotationEnd) return null
        if (computedEnd < annotationEnd && !source.substring(computedEnd, annotationEnd).all { it.isWhitespace() }) {
            return null
        }
        return at
    }

    private fun findMatchingOpenParen(source: String, closeIndex: Int): Int? {
        if (source.getOrNull(closeIndex) != ')') return null
        var depth = 0
        var inString = false
        var i = closeIndex
        while (i >= 0) {
            when {
                inString -> {
                    if (source[i] == '\\') {
                        i -= 2
                        continue
                    }
                    if (source[i] == '"') inString = false
                }
                source[i] == '"' -> inString = true
                source[i] == ')' -> depth++
                source[i] == '(' -> {
                    depth--
                    if (depth == 0) return i
                }
            }
            i--
        }
        return null
    }

    private fun findMethodAttribute(source: String, bodyStart: Int, bodyEnd: Int): String? {
        val body = source.substring(bodyStart.coerceIn(0, source.length), (bodyEnd + 1).coerceIn(0, source.length))
        val match = Regex("""method\s*=\s*"([^"]*)""").find(body) ?: return null
        return match.groupValues[1]
    }

    private fun findAtValue(source: String, bodyStart: Int, bodyEnd: Int): String? {
        var search = bodyStart + 1
        while (search < bodyEnd) {
            val at = source.indexOf('@', search)
            if (at < 0 || at >= bodyEnd) break
            if (parseAnnotationName(source, at) != MixinAnnotation.AT) {
                search = at + 1
                continue
            }
            val nameEnd = skipAnnotationName(source, at)
            val paren = if (source.getOrNull(nameEnd) == '(') nameEnd else break
            val close = findMatchingParen(source, paren) ?: break
            if (close > bodyEnd) break
            val atBodyStart = paren + 1
            val atBodyEnd = close
            findStringAttribute(source, atBodyStart, atBodyEnd, "value")?.let { return it }
            readShorthandString(source, atBodyStart, atBodyEnd)?.let { return it }
            search = close + 1
        }
        return null
    }

    private fun findAtValueInAtBody(source: String, bodyStart: Int, bodyEnd: Int): String? =
        findStringAttribute(source, bodyStart, bodyEnd, "value")
            ?: readShorthandString(source, bodyStart, bodyEnd)

    private fun readShorthandString(source: String, start: Int, end: Int): String? {
        val index = skipWhitespace(source, start, end)
        if (index >= end || source[index] != '"') return null
        val valueInfo = readValue(source, index, end) ?: return null
        return source.substring(valueInfo.contentStart, valueInfo.contentEnd)
    }

    private fun findStringAttribute(source: String, bodyStart: Int, bodyEnd: Int, name: String): String? {
        val body = source.substring(bodyStart.coerceIn(0, source.length), (bodyEnd + 1).coerceIn(0, source.length))
        val match = Regex("""$name\s*=\s*"([^"]*)""").find(body) ?: return null
        return match.groupValues[1]
    }

    private fun findBooleanAttribute(source: String, bodyStart: Int, bodyEnd: Int, name: String): Boolean? {
        val body = source.substring(bodyStart.coerceIn(0, source.length), (bodyEnd + 1).coerceIn(0, source.length))
        val match = Regex("""$name\s*=\s*(true|false)""").find(body) ?: return null
        return match.groupValues[1] == "true"
    }

    private fun readIdentifier(source: String, start: Int, end: Int): String? {
        if (start >= end || !source[start].isLetter() && source[start] != '_') return null
        var i = start
        while (i < end && (source[i].isLetterOrDigit() || source[i] == '_')) i++
        return source.substring(start, i)
    }

    private fun skipWhitespace(source: String, start: Int, end: Int): Int {
        var i = start
        while (i < end && source[i].isWhitespace()) i++
        return i
    }

    private fun findMatchingParen(source: String, openIndex: Int): Int? {
        if (source.getOrNull(openIndex) != '(') return null
        var depth = 0
        var inString = false
        var i = openIndex
        while (i < source.length) {
            when {
                inString -> {
                    if (source[i] == '\\') {
                        i += 2
                        continue
                    }
                    if (source[i] == '"') inString = false
                }
                source[i] == '"' -> inString = true
                source[i] == '(' -> depth++
                source[i] == ')' -> {
                    depth--
                    if (depth == 0) return i
                }
            }
            i++
        }
        return null
    }

    private fun findMatchingBrace(source: String, openIndex: Int): Int? {
        if (source.getOrNull(openIndex) != '{') return null
        var depth = 0
        var inString = false
        var i = openIndex
        while (i < source.length) {
            when {
                inString -> {
                    if (source[i] == '\\') {
                        i += 2
                        continue
                    }
                    if (source[i] == '"') inString = false
                }
                source[i] == '"' -> inString = true
                source[i] == '{' -> depth++
                source[i] == '}' -> {
                    depth--
                    if (depth == 0) return i
                }
            }
            i++
        }
        return null
    }

    fun toOffset(source: String, line: Int, character: Int): Int? {
        if (line < 0 || character < 0) return null
        var currentLine = 0
        var offset = 0
        while (offset < source.length && currentLine < line) {
            if (source[offset] == '\n') currentLine++
            offset++
        }
        val result = offset + character
        return if (result <= source.length) result else null
    }

    fun findInjectorAnnotationOffsets(source: String): List<Int> {
        val results = mutableListOf<Int>()
        var search = 0
        while (search < source.length) {
            val at = source.indexOf('@', search)
            if (at < 0) break
            val annotation = parseAnnotationName(source, at)
            if (annotation != null && annotation in injectorAnnotations) {
                results += at
            }
            search = at + 1
        }
        return results
    }

    fun extractAtAnnotationBodies(source: String): List<String> {
        val bodies = mutableListOf<String>()
        findAnnotationOffsets(source, MixinAnnotation.AT).forEach { at ->
            val nameEnd = skipAnnotationName(source, at)
            val paren = if (source.getOrNull(nameEnd) == '(') nameEnd else return@forEach
            val close = findMatchingParen(source, paren) ?: return@forEach
            bodies += source.substring(paren + 1, close)
        }
        return bodies
    }

    private fun stripClassSuffix(value: String): String = value.removeSuffix(".class").trim()

    private fun classSlotPartial(partial: String): String =
        extractPartialClassReference(partial, 0, partial.length)

    fun fqnToInternal(fqn: String): String = fqn.replace('.', '/')

    fun internalToFqn(internal: String): String = internal.replace('/', '.')
}
