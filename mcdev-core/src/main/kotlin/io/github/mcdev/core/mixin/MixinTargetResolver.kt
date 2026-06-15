package io.github.mcdev.core.mixin

object MixinTargetResolver {
    fun resolveTargets(
        rawTargets: List<String>,
        classIndex: ClassIndex,
        imports: JavaSourceImports? = null,
    ): List<String> =
        rawTargets.mapNotNull { resolveTarget(it, classIndex, imports) }.distinct()

    fun resolveTarget(
        raw: String,
        classIndex: ClassIndex,
        imports: JavaSourceImports? = null,
    ): String? {
        val trimmed = raw.trim()
        if (trimmed.isEmpty()) return null
        resolveImportedTarget(trimmed, classIndex, imports)?.let { return it }
        val internal = when {
            trimmed.contains('/') -> trimmed
            trimmed.contains('.') -> AnnotationContextExtractor.fqnToInternal(trimmed)
            else -> trimmed
        }
        classIndex.findClass(internal)?.internalName?.let { return it }
        classIndex.findClassByFqn(trimmed)?.internalName?.let { return it }
        classIndex.findClasses(trimmed, limit = 5).singleOrNull { it.simpleName == trimmed }?.internalName?.let { return it }
        return null
    }

    fun resolveTargetsFromSource(source: String, classIndex: ClassIndex): List<String> {
        val mixinAt = Regex("""@Mixin\s*\(""").find(source)?.range?.first ?: return emptyList()
        val raw = AnnotationContextExtractor.parseMixinTargetValues(source, mixinAt)
        return resolveTargets(raw, classIndex, JavaTypeDescriptorResolver.importsFor(source))
    }

    private fun resolveImportedTarget(
        target: String,
        classIndex: ClassIndex,
        imports: JavaSourceImports?,
    ): String? {
        if (imports == null || target.contains('/') || target.contains('.')) return null
        val importedFqn = imports.explicit[target] ?: return null
        return classIndex.findClassByFqn(importedFqn)?.internalName
            ?: classIndex.findClass(AnnotationContextExtractor.fqnToInternal(importedFqn))?.internalName
    }
}
