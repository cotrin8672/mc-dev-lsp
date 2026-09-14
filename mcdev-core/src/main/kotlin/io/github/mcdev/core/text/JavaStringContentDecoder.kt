package io.github.mcdev.core.text

object JavaStringContentDecoder {
    data class DecodedEscape(val char: Char, val nextIndex: Int)

    /** Decode one phase-1 Java Unicode escape from raw input. */
    fun decodeUnicodeEscape(source: String, index: Int, limit: Int): DecodedEscape? {
        if (source.getOrNull(index) != '\\' || index + 1 >= limit || source[index + 1] != 'u') return null
        var hexStart = index + 1
        while (hexStart < limit && source[hexStart] == 'u') hexStart++
        if (hexStart + 4 > limit) return null
        var value = 0
        repeat(4) { offset ->
            val digit = when (val ch = source[hexStart + offset]) {
                in '0'..'9' -> ch - '0'
                in 'a'..'f' -> ch - 'a' + 10
                in 'A'..'F' -> ch - 'A' + 10
                else -> return null
            }
            value = (value shl 4) or digit
        }
        return DecodedEscape(value.toChar(), hexStart + 4)
    }

    fun decodeEscape(source: String, index: Int, limit: Int): DecodedEscape? {
        if (source.getOrNull(index) != '\\' || index + 1 >= limit) return null
        return when (val escape = source[index + 1]) {
            'b' -> DecodedEscape('\b', index + 2)
            't' -> DecodedEscape('\t', index + 2)
            'n' -> DecodedEscape('\n', index + 2)
            'f' -> DecodedEscape('\u000C', index + 2)
            'r' -> DecodedEscape('\r', index + 2)
            '"' -> DecodedEscape('"', index + 2)
            '\'' -> DecodedEscape('\'', index + 2)
            '\\' -> DecodedEscape('\\', index + 2)
            's' -> DecodedEscape(' ', index + 2)
            in '0'..'7' -> decodeOctalEscape(source, index, limit)
            else -> null
        }
    }

    fun appendDecodedChar(
        source: String,
        fileIndex: Int,
        fileLimit: Int,
        builder: StringBuilder,
    ): Int? {
        if (fileIndex >= fileLimit) return null
        return when (source[fileIndex]) {
            '\\' -> {
                val decoded = decodeEscape(source, fileIndex, fileLimit) ?: return null
                builder.append(decoded.char)
                decoded.nextIndex
            }
            else -> {
                builder.append(source[fileIndex])
                fileIndex + 1
            }
        }
    }

    fun decodeContent(source: String, contentStart: Int, contentEnd: Int): String? {
        if (contentStart < 0 || contentStart > contentEnd || contentEnd > source.length) return null
        val builder = StringBuilder()
        var index = contentStart
        while (index < contentEnd) {
            index = appendDecodedChar(source, index, contentEnd, builder) ?: return null
        }
        return builder.toString()
    }

    private fun decodeOctalEscape(source: String, index: Int, limit: Int): DecodedEscape? {
        var value = source[index + 1] - '0'
        var next = index + 2
        if (next < limit && source[next] in '0'..'7') {
            value = value * 8 + (source[next] - '0')
            next++
            if (source[index + 1] <= '3' && next < limit && source[next] in '0'..'7') {
                value = value * 8 + (source[next] - '0')
                next++
            }
        }
        if (value > 0xFFFF) return null
        return DecodedEscape(value.toChar(), next)
    }
}
