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
        val bodyStart = source.indexOf('(', annotationStart).takeIf { it >= 0 } ?: return null
        if (cursorOffset <= bodyStart) {
            return buildClassSlotContext(source, annotation, annotationStart, bodyStart, cursorOffset, mixinTargets)
        }
        val bodyEnd = findMatchingParen(source, bodyStart) ?: source.length
        val annotationEnd = bodyEnd + 1
        val slotContext = findSlotAtCursor(source, annotation, bodyStart, bodyEnd, cursorOffset)
            ?: if (annotation == MixinAnnotation.MIXIN && cursorOffset in (bodyStart + 1)..bodyEnd) {
                buildClassSlotContext(source, annotation, annotationStart, bodyStart, cursorOffset, mixinTargets)
            } else {
                return null
            }
        return slotContext?.copy(
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
            var nameEnd = at + 1
            while (nameEnd < source.length && (source[nameEnd].isLetterOrDigit() || source[nameEnd] == '_')) {
                nameEnd++
            }
            val name = source.substring(at + 1, nameEnd)
            if (MixinAnnotation.fromSimpleName(name) != null) {
                val paren = source.indexOf('(', at)
                if (paren < 0 || paren > cursorOffset) {
                    searchFrom = at - 1
                    continue
                }
                val close = findMatchingParen(source, paren)
                if (close == null || cursorOffset <= close) {
                    return at
                }
            }
            searchFrom = at - 1
        }
        return null
    }

    private fun parseAnnotationName(source: String, atOffset: Int): MixinAnnotation? {
        if (source.getOrNull(atOffset) != '@') return null
        var end = atOffset + 1
        while (end < source.length && (source[end].isLetterOrDigit() || source[end] == '_')) {
            end++
        }
        return MixinAnnotation.fromSimpleName(source.substring(atOffset + 1, end))
    }

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
                val nestedParen = source.indexOf('(', nestedStart)
                if (nestedParen < 0 || nestedParen >= bodyEnd) break
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
            return AnnotationContext(
                annotation = annotation,
                slot = AnnotationSlot.SHADOW_MEMBER,
                partialValue = "",
                valueStartOffset = bodyEnd,
                valueEndOffset = bodyEnd,
                annotationStartOffset = 0,
                annotationEndOffset = 0,
            )
        }
        return null
    }

    private fun readShorthandValue(source: String, start: Int, bodyEnd: Int): ValueInfo? {
        if (start >= bodyEnd) return null
        if (source[start] == '"') return readValue(source, start, bodyEnd)
        val identifier = readIdentifier(source, start, bodyEnd) ?: return null
        var next = skipWhitespace(source, start + identifier.length, bodyEnd)
        if (next < bodyEnd && source.startsWith(".class", next)) {
            val end = next + ".class".length
            return ValueInfo(start, end, start, end)
        }
        return null
    }

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
        val cleanedPartial = if (slot == AnnotationSlot.CLASS) stripClassSuffix(partial.trim('"')) else partial.trim('"')
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
        val atValue = if (annotation == MixinAnnotation.AT && attrName == "value") {
            partial.trim('"')
        } else {
            findAtValue(source, bodyStart, bodyEnd)
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
        val cleanedPartial = if (slot == AnnotationSlot.CLASS) stripClassSuffix(partial.trim('"')) else partial
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
        val classPattern = Regex("""\b(?:public\s+|private\s+|protected\s+)?(?:abstract\s+|final\s+)?class\s+\w+""")
        return classPattern.findAll(before).lastOrNull()?.range?.first
    }

    private fun findMixinAnnotationOnClass(source: String, classDeclOffset: Int): Int? {
        val searchStart = (classDeclOffset - 500).coerceAtLeast(0)
        val region = source.substring(searchStart, classDeclOffset)
        val atPattern = Regex("""@Mixin\s*\(""")
        return atPattern.findAll(region).lastOrNull()?.range?.first?.plus(searchStart)
    }

    fun parseMixinTargetValues(source: String, mixinAtOffset: Int): List<String> {
        val paren = source.indexOf('(', mixinAtOffset)
        if (paren < 0) return emptyList()
        val close = findMatchingParen(source, paren) ?: return emptyList()
        val body = source.substring(paren + 1, close)
        val targetsMatch = Regex("""targets\s*=\s*("([^"]*)"|(\{[^}]*\}))""").find(body)
        if (targetsMatch != null) {
            val value = targetsMatch.groupValues[1]
            return if (value.startsWith("{")) {
                Regex("""["']([^"']+)["']""").findAll(value).map { it.groupValues[1] }.toList()
            } else {
                listOf(targetsMatch.groupValues[2])
            }
        }
        val classRefs = Regex("""([\w.]+)\s*\.class""").findAll(body).map {
            fqnToInternal(it.groupValues[1])
        }.toList()
        return classRefs
    }

    private fun findMethodAttribute(source: String, bodyStart: Int, bodyEnd: Int): String? {
        val body = source.substring(bodyStart, bodyEnd + 1)
        val match = Regex("""method\s*=\s*"([^"]*)""").find(body) ?: return null
        return match.groupValues[1]
    }

    private fun findAtValue(source: String, bodyStart: Int, bodyEnd: Int): String? {
        val body = source.substring(bodyStart, bodyEnd + 1)
        val atMatch = Regex("""@At\s*\([^)]*value\s*=\s*"([^"]*)""").find(body) ?: return null
        return atMatch.groupValues[1]
    }

    private fun findStringAttribute(source: String, bodyStart: Int, bodyEnd: Int, name: String): String? {
        val body = source.substring(bodyStart, bodyEnd + 1)
        val match = Regex("""$name\s*=\s*"([^"]*)""").find(body) ?: return null
        return match.groupValues[1]
    }

    private fun findBooleanAttribute(source: String, bodyStart: Int, bodyEnd: Int, name: String): Boolean? {
        val body = source.substring(bodyStart, bodyEnd + 1)
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

    fun extractAtAnnotationBodies(source: String): List<String> {
        val bodies = mutableListOf<String>()
        var search = 0
        while (search < source.length) {
            val at = source.indexOf("@At", search)
            if (at < 0) break
            val paren = source.indexOf('(', at)
            if (paren < 0) break
            val close = findMatchingParen(source, paren) ?: break
            bodies += source.substring(paren + 1, close)
            search = close + 1
        }
        return bodies
    }

    private fun stripClassSuffix(value: String): String = value.removeSuffix(".class").trim()

    fun fqnToInternal(fqn: String): String = fqn.replace('.', '/')

    fun internalToFqn(internal: String): String = internal.replace('/', '.')
}
