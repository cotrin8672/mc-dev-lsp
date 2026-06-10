package io.github.mcdev.core.at

data class AccessTransformerEditResult(
    val content: String,
    val changed: Boolean,
)

class AccessTransformerEditor {
    fun formatEntry(entry: AccessTransformerEntry): String {
        val member = when {
            entry.name == null -> null
            entry.descriptor != null -> "${entry.name}${entry.descriptor}"
            else -> entry.name
        }
        return if (member != null) {
            "${entry.modifier.token} ${entry.owner} $member"
        } else {
            "${entry.modifier.token} ${entry.owner}"
        }
    }

    fun appendEntry(content: String, entry: AccessTransformerEntry): AccessTransformerEditResult {
        val line = formatEntry(entry)
        val suffix = if (content.isEmpty() || content.endsWith("\n")) "" else "\n"
        val updated = content + suffix + line + "\n"
        return AccessTransformerEditResult(updated, changed = true)
    }

    fun replaceLine(content: String, lineNumber: Int, newLine: String): AccessTransformerEditResult {
        val lines = content.split("\n").toMutableList()
        if (lineNumber < 1 || lineNumber > lines.size) {
            return AccessTransformerEditResult(content, changed = false)
        }
        val previous = lines[lineNumber - 1]
        if (previous == newLine) {
            return AccessTransformerEditResult(content, changed = false)
        }
        lines[lineNumber - 1] = newLine
        val trailingNewline = if (content.endsWith("\n")) "\n" else ""
        return AccessTransformerEditResult(lines.joinToString("\n") + trailingNewline, changed = true)
    }

    fun removeLine(content: String, lineNumber: Int): AccessTransformerEditResult {
        val lines = content.split("\n").toMutableList()
        if (lineNumber < 1 || lineNumber > lines.size) {
            return AccessTransformerEditResult(content, changed = false)
        }
        lines.removeAt(lineNumber - 1)
        val trailingNewline = if (content.endsWith("\n") && lines.isNotEmpty()) "\n" else ""
        val updated = lines.joinToString("\n") + trailingNewline
        return AccessTransformerEditResult(updated, changed = updated != content)
    }

    fun entryKey(entry: AccessTransformerEntry): String =
        listOf(
            entry.modifier.token,
            entry.owner,
            entry.name.orEmpty(),
            entry.descriptor.orEmpty(),
        ).joinToString("|")
}
