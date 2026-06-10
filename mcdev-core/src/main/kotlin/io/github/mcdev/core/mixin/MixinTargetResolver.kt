package io.github.mcdev.core.mixin

object MixinTargetResolver {
    fun resolveTargets(rawTargets: List<String>, classIndex: ClassIndex): List<String> =
        rawTargets.mapNotNull { resolveTarget(it, classIndex) }.distinct()

    fun resolveTarget(raw: String, classIndex: ClassIndex): String? {
        val trimmed = raw.trim()
        if (trimmed.isEmpty()) return null
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
        return resolveTargets(raw, classIndex)
    }
}
