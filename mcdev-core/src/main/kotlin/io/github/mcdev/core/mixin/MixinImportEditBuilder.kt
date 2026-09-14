package io.github.mcdev.core.mixin

import io.github.mcdev.core.codeaction.McTextEdit

object MixinImportEditBuilder {
    private val importPattern = Regex("""(?m)^\s*import\s+[\w.${'$'}]+(?:\.\*)?\s*;""")
    private val packagePattern = Regex("""(?m)^\s*package\s+([\w.]+)\s*;""")

    data class TypeReference(
        val text: String,
        val importFqn: String? = null,
    )

    /**
     * Select a source-level name for a descriptor-derived JVM internal name.
     * The slash and binary '$' delimiters provide the package/class boundary;
     * this deliberately does not infer it from class-name capitalization.
     */
    fun referenceForInternalName(
        source: String,
        internalName: String,
        additionalInternalNames: Set<String> = emptySet(),
    ): TypeReference {
        val className = internalName.substringAfterLast('/')
        if (className.isEmpty()) return TypeReference(internalName)
        val packageName = internalName.substringBeforeLast('/', "").replace('/', '.')
        val sourceClassName = className.replace('$', '.')
        val sourceFqn = if (packageName.isEmpty()) sourceClassName else "$packageName.$sourceClassName"
        // A nested-class import binds its innermost name, not Outer.Inner.
        if ('$' in className) return TypeReference(sourceFqn)
        val bindingName = sourceClassName.substringBefore('.')
        val parsedImports = JavaTypeDescriptorResolver.importsFor(source)
        val sourcePackage = parsedImports.packageName
        val imports = parsedImports.explicit.values.toList()
        val hasExplicitConflict = imports.any { imported ->
            !imported.endsWith(".*") &&
                imported.substringAfterLast('.') == bindingName &&
                imported != sourceFqn
        }
        val generatedConflict = additionalInternalNames.any { candidate ->
            candidate != internalName &&
                candidate.substringAfterLast('/').replace('$', '.').substringBefore('.') == bindingName
        }
        val hasWildcardConflict = parsedImports.wildcardPackages.any { it != packageName }
        if (hasExplicitConflict || generatedConflict || hasWildcardConflict) {
            return TypeReference(sourceFqn)
        }
        if (imports.any { imported -> imported == sourceFqn }) {
            return TypeReference(sourceClassName)
        }
        if (sourcePackage == packageName || packageName == "java.lang") {
            return TypeReference(sourceClassName)
        }
        return TypeReference(sourceClassName, sourceFqn)
    }

    fun buildImportEdit(source: String, fqn: String): McTextEdit? {
        if (fqn.isBlank() || !fqn.contains('.')) return null
        val code = AnnotationContextExtractor.maskNonCode(source)
        if (code.contains("import $fqn;")) return null

        val targetPackage = fqn.substringBeforeLast('.')
        packagePattern.find(code)?.let { match ->
            if (match.groupValues[1] == targetPackage) return null
        }

        val insertOffset = findImportInsertOffset(source, code)
        val needsLeadingNewline = insertOffset > 0 && source.getOrNull(insertOffset - 1) != '\n'
        val prefix = if (needsLeadingNewline) "\n" else ""
        return McTextEdit(
            startOffset = insertOffset,
            endOffset = insertOffset,
            newText = "${prefix}import $fqn;\n",
        )
    }

    fun needsImport(source: String, fqn: String): Boolean = buildImportEdit(source, fqn) != null

    private fun findImportInsertOffset(source: String, code: String = AnnotationContextExtractor.maskNonCode(source)): Int {
        val lastImport = importPattern.findAll(code).lastOrNull()
        if (lastImport != null) {
            return lastImport.range.last + 1
        }
        val packageDecl = packagePattern.find(code)
        if (packageDecl != null) {
            return packageDecl.range.last + 1
        }
        return 0
    }

}
