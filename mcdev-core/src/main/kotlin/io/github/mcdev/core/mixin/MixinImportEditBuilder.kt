package io.github.mcdev.core.mixin

import io.github.mcdev.core.codeaction.McTextEdit

object MixinImportEditBuilder {
    private val importPattern = Regex("""import\s+([\w.]+)\s*;""")
    private val packagePattern = Regex("""package\s+([\w.]+)\s*;""")

    fun buildImportEdit(source: String, fqn: String): McTextEdit? {
        if (fqn.isBlank() || !fqn.contains('.')) return null
        if (source.contains("import $fqn;")) return null

        val targetPackage = fqn.substringBeforeLast('.')
        packagePattern.find(source)?.let { match ->
            if (match.groupValues[1] == targetPackage) return null
        }

        val insertOffset = findImportInsertOffset(source)
        val needsLeadingNewline = insertOffset > 0 && source.getOrNull(insertOffset - 1) != '\n'
        val prefix = if (needsLeadingNewline) "\n" else ""
        return McTextEdit(
            startOffset = insertOffset,
            endOffset = insertOffset,
            newText = "${prefix}import $fqn;\n",
        )
    }

    fun needsImport(source: String, fqn: String): Boolean = buildImportEdit(source, fqn) != null

    private fun findImportInsertOffset(source: String): Int {
        val lastImport = importPattern.findAll(source).lastOrNull()
        if (lastImport != null) {
            return lastImport.range.last + 1
        }
        val packageDecl = packagePattern.find(source)
        if (packageDecl != null) {
            return packageDecl.range.last + 1
        }
        return 0
    }
}
