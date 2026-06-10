package io.github.mcdev.core.at

import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange

internal object AtTextPositions {
    fun toOffset(source: String, line: Int, character: Int): Int? {
        if (line < 0 || character < 0) return null
        var currentLine = 0
        var index = 0
        while (index <= source.length) {
            if (currentLine == line) {
                val lineEnd = source.indexOf('\n', index).let { if (it < 0) source.length else it }
                return (index + character).coerceAtMost(lineEnd)
            }
            val nextNewline = source.indexOf('\n', index)
            if (nextNewline < 0) return null
            index = nextNewline + 1
            currentLine++
        }
        return null
    }

    fun offsetToPosition(source: String, offset: Int): McTextPosition {
        val safeOffset = offset.coerceIn(0, source.length)
        var line = 0
        var lineStart = 0
        for (index in source.indices) {
            if (index >= safeOffset) break
            if (source[index] == '\n') {
                line++
                lineStart = index + 1
            }
        }
        return McTextPosition(line, safeOffset - lineStart)
    }

    fun rangeForOffsets(source: String, start: Int, end: Int): McTextRange =
        McTextRange(offsetToPosition(source, start), offsetToPosition(source, end))

    fun lineRange(source: String, lineNumber: Int): McTextRange? {
        var currentLine = 1
        var index = 0
        while (index <= source.length) {
            if (currentLine == lineNumber) {
                val end = source.indexOf('\n', index).let { if (it < 0) source.length else it }
                return rangeForOffsets(source, index, end)
            }
            val nextNewline = source.indexOf('\n', index)
            if (nextNewline < 0) return null
            index = nextNewline + 1
            currentLine++
        }
        return null
    }
}
