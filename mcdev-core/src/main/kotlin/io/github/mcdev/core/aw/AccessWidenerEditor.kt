package io.github.mcdev.core.aw

import io.github.mcdev.core.aw.AccessWidenerDirective.ACCESSIBLE
import io.github.mcdev.core.aw.AccessWidenerDirective.EXTENDABLE
import io.github.mcdev.core.aw.AccessWidenerDirective.MUTABLE
import io.github.mcdev.core.aw.AccessWidenerDirective.NATURAL
import io.github.mcdev.core.aw.AccessWidenerKind.CLASS
import io.github.mcdev.core.aw.AccessWidenerKind.FIELD
import io.github.mcdev.core.aw.AccessWidenerKind.METHOD

object AccessWidenerEditor {
    fun formatEntry(entry: AccessWidenerEntry): String = when (entry.kind) {
        CLASS -> formatClassEntry(entry.directive, entry.owner)
        METHOD -> formatMethodEntry(entry.directive, entry.owner, entry.name.orEmpty(), entry.descriptor.orEmpty())
        FIELD -> formatFieldEntry(entry.directive, entry.owner, entry.name.orEmpty(), entry.descriptor.orEmpty())
    }

    fun formatClassEntry(directive: AccessWidenerDirective, owner: String): String =
        "${directive.toAwToken()} class $owner"

    fun formatMethodEntry(
        directive: AccessWidenerDirective,
        owner: String,
        name: String,
        descriptor: String,
    ): String = "${directive.toAwToken()} method $owner $name $descriptor"

    fun formatFieldEntry(
        directive: AccessWidenerDirective,
        owner: String,
        name: String,
        descriptor: String,
    ): String = "${directive.toAwToken()} field $owner $name $descriptor"

    fun insertEntry(content: String, entry: AccessWidenerEntry, afterLine: Int? = null): String {
        val lines = content.lineSequence().toMutableList()
        val insertAt = afterLine?.coerceIn(1, lines.size) ?: lines.size
        lines.add(insertAt, formatEntry(entry))
        return lines.joinToString("\n").let { formatted ->
            if (content.endsWith("\n") && !formatted.endsWith("\n")) "$formatted\n" else formatted
        }
    }

    fun removeLine(content: String, lineNumber: Int): String {
        val lines = content.lineSequence().toMutableList()
        if (lineNumber !in 1..lines.size) return content
        lines.removeAt(lineNumber - 1)
        return lines.joinToString("\n").let { formatted ->
            when {
                content.endsWith("\n") && lines.isNotEmpty() && !formatted.endsWith("\n") -> "$formatted\n"
                content.endsWith("\n") && lines.isEmpty() -> ""
                else -> formatted
            }
        }
    }

    fun replaceLine(content: String, lineNumber: Int, newLine: String): String {
        val lines = content.lineSequence().toMutableList()
        if (lineNumber !in 1..lines.size) return content
        lines[lineNumber - 1] = newLine
        return lines.joinToString("\n").let { formatted ->
            if (content.endsWith("\n") && !formatted.endsWith("\n")) "$formatted\n" else formatted
        }
    }

    fun entryKey(entry: AccessWidenerEntry): String = buildString {
        append(entry.directive.name)
        append('|')
        append(entry.kind.name)
        append('|')
        append(entry.owner)
        if (entry.kind != CLASS) {
            append('|')
            append(entry.name.orEmpty())
            append('|')
            append(entry.descriptor.orEmpty())
        }
    }

    private fun AccessWidenerDirective.toAwToken(): String = when (this) {
        ACCESSIBLE -> "accessible"
        EXTENDABLE -> "extendable"
        MUTABLE -> "mutable"
        NATURAL -> "natural"
    }
}
