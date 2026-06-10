package io.github.mcdev.core.aw

import io.github.mcdev.core.mapping.parseNamespace

object AwContextExtractor {
    fun extract(source: String, line: Int, character: Int): AwAnnotationContext? {
        val cursorOffset = toOffset(source, line, character) ?: return null
        return extractAtOffset(source, cursorOffset)
    }

    fun extractAtOffset(source: String, offset: Int): AwAnnotationContext? {
        val safeOffset = offset.coerceIn(0, source.length)
        val lineStart = source.lastIndexOf('\n', safeOffset - 1) + 1
        val lineEnd = source.indexOf('\n', safeOffset).let { if (it < 0) source.length else it }
        val lineNumber = source.substring(0, lineStart).count { it == '\n' } + 1
        val lineText = source.substring(lineStart, lineEnd)
        val column = safeOffset - lineStart

        val contentLine = lineText.substringBefore('#').trimEnd()
        if (contentLine.isBlank()) return null

        val fileNamespace = parseFileNamespace(source)
        if (lineNumber == 1) {
            return extractHeaderContext(source, safeOffset, lineStart, lineEnd, lineNumber, contentLine, fileNamespace)
        }

        val tokens = tokenizeAwLine(contentLine.trim(), lineStart + lineText.indexOf(contentLine.trim()))
        if (tokens.isEmpty()) return null

        val activeTokenIndex = tokens.indexOfFirst { safeOffset in it.startOffset..it.endOffset }
            .takeIf { it >= 0 }
            ?: tokens.indexOfLast { it.startOffset <= safeOffset }.takeIf { it >= 0 }
            ?: tokens.lastIndex

        val activeToken = tokens[activeTokenIndex]
        val directive = tokens.getOrNull(0)?.text?.let(::parseDirectiveToken)
        val kind = tokens.getOrNull(1)?.text?.let(::parseKindToken)
        val owner = tokens.getOrNull(2)?.text
        val name = if (kind == AccessWidenerKind.METHOD || kind == AccessWidenerKind.FIELD) {
            tokens.getOrNull(3)?.text
        } else {
            null
        }
        val descriptor = tokens.getOrNull(4)?.text

        val slot = when (activeTokenIndex) {
            0 -> AwSyntaxSlot.DIRECTIVE
            1 -> AwSyntaxSlot.KIND
            2 -> AwSyntaxSlot.OWNER
            3 -> when (kind) {
                AccessWidenerKind.METHOD, AccessWidenerKind.FIELD -> AwSyntaxSlot.NAME
                else -> AwSyntaxSlot.OWNER
            }
            else -> AwSyntaxSlot.DESCRIPTOR
        }

        val partialEnd = safeOffset.coerceIn(activeToken.startOffset, activeToken.endOffset)
        return AwAnnotationContext(
            slot = slot,
            partialValue = source.substring(activeToken.startOffset, partialEnd),
            valueStartOffset = activeToken.startOffset,
            valueEndOffset = partialEnd,
            lineStartOffset = lineStart,
            lineEndOffset = lineEnd,
            lineNumber = lineNumber,
            fileNamespace = fileNamespace,
            directive = directive,
            kind = kind,
            owner = owner,
            name = name,
            descriptor = descriptor,
            isHeaderLine = false,
        )
    }

    fun parseFileNamespace(source: String) = run {
        val header = source.lineSequence().firstOrNull()?.substringBefore('#')?.trim().orEmpty()
        val parts = header.split(Regex("\\s+"))
        if (parts.size < 3 || parts[0] != "accessWidener" || parts[1] != "v2") null
        else parseNamespace(parts[2])
    }

    fun toOffset(source: String, line: Int, character: Int): Int? {
        if (line < 0 || character < 0) return null
        val lines = source.lineSequence().toList()
        if (line >= lines.size) return null
        val lineText = lines[line]
        val safeCharacter = character.coerceIn(0, lineText.length)
        var offset = 0
        for (index in 0 until line) {
            offset += lines[index].length + 1
        }
        return offset + safeCharacter
    }

    fun offsetToPosition(source: String, offset: Int): Pair<Int, Int> {
        var line = 0
        var lineStart = 0
        for (index in source.indices) {
            if (index == offset) return line to (offset - lineStart)
            if (source[index] == '\n') {
                line++
                lineStart = index + 1
            }
        }
        return line to (offset - lineStart)
    }

    private fun extractHeaderContext(
        source: String,
        cursorOffset: Int,
        lineStart: Int,
        lineEnd: Int,
        lineNumber: Int,
        contentLine: String,
        fileNamespace: io.github.mcdev.core.model.MappingNamespace?,
    ): AwAnnotationContext? {
        val parts = contentLine.trim().split(Regex("\\s+"))
        if (parts.size < 3) return null
        val trimmed = contentLine.trim()
        val namespaceStart = lineStart + lineTextOffset(source.substring(lineStart, lineEnd), trimmed) +
            trimmed.indexOf(parts[2])
        val namespaceEnd = namespaceStart + parts[2].length
        if (cursorOffset !in namespaceStart..namespaceEnd) return null
        return AwAnnotationContext(
            slot = AwSyntaxSlot.HEADER_NAMESPACE,
            partialValue = source.substring(namespaceStart, cursorOffset.coerceAtMost(namespaceEnd)),
            valueStartOffset = namespaceStart,
            valueEndOffset = cursorOffset.coerceIn(namespaceStart, namespaceEnd),
            lineStartOffset = lineStart,
            lineEndOffset = lineEnd,
            lineNumber = lineNumber,
            fileNamespace = fileNamespace,
            directive = null,
            kind = null,
            owner = null,
            name = null,
            descriptor = null,
            isHeaderLine = true,
        )
    }

    private fun lineTextOffset(lineText: String, trimmed: String): Int =
        lineText.indexOf(trimmed).coerceAtLeast(0)

    private data class AwLineToken(
        val startOffset: Int,
        val endOffset: Int,
        val text: String,
    )

    private fun tokenizeAwLine(line: String, lineStartOffset: Int): List<AwLineToken> {
        val tokens = mutableListOf<AwLineToken>()
        var index = 0
        while (index < line.length) {
            while (index < line.length && line[index].isWhitespace()) index++
            if (index >= line.length) break
            val start = index
            if (line[index] == '(') {
                var depth = 0
                while (index < line.length) {
                    when (line[index]) {
                        '(' -> depth++
                        ')' -> {
                            depth--
                            if (depth == 0) {
                                index++
                                break
                            }
                        }
                    }
                    index++
                }
            } else {
                while (index < line.length && !line[index].isWhitespace()) index++
            }
            val text = line.substring(start, index)
            tokens += AwLineToken(
                startOffset = lineStartOffset + start,
                endOffset = lineStartOffset + index,
                text = text,
            )
        }
        return tokens
    }

    private fun parseDirectiveToken(value: String): AccessWidenerDirective? = when (value) {
        "accessible" -> AccessWidenerDirective.ACCESSIBLE
        "extendable" -> AccessWidenerDirective.EXTENDABLE
        "mutable" -> AccessWidenerDirective.MUTABLE
        "natural" -> AccessWidenerDirective.NATURAL
        else -> null
    }

    private fun parseKindToken(value: String): AccessWidenerKind? = when (value) {
        "class" -> AccessWidenerKind.CLASS
        "method" -> AccessWidenerKind.METHOD
        "field" -> AccessWidenerKind.FIELD
        else -> null
    }
}
