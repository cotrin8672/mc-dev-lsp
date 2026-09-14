package io.github.mcdev.core.mixin

import io.github.mcdev.core.mixinextras.ClassLiteralTypeNameResolver
import org.objectweb.asm.Type

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
        resolveClassLiteralTarget(trimmed, classIndex, imports)?.let { return it }
        val sourceResolutionContext = hasSourceResolutionContext(imports)
        val internal = when {
            trimmed.contains('/') -> trimmed
            trimmed.contains('.') -> AnnotationContextExtractor.fqnToInternal(trimmed)
            else -> trimmed
        }
        if (!sourceResolutionContext || trimmed.contains('/') || trimmed.contains('.')) {
            classIndex.findClass(internal)?.internalName?.let { return it }
            classIndex.findClassByFqn(trimmed)?.internalName?.let { return it }
        }
        if (!sourceResolutionContext) {
            classIndex.findClasses(trimmed, limit = 5).singleOrNull { it.simpleName == trimmed }
                ?.internalName
                ?.let { return it }
        }
        return null
    }

    fun resolveTargetsFromSource(source: String, classIndex: ClassIndex): List<String> {
        val mixinAt = Regex("""@Mixin\s*\(""").find(source)?.range?.first ?: return emptyList()
        val raw = AnnotationContextExtractor.parseMixinTargetValues(source, mixinAt)
        return resolveTargets(raw, classIndex, JavaTypeDescriptorResolver.importsFor(source))
    }

    private fun resolveClassLiteralTarget(
        target: String,
        classIndex: ClassIndex,
        imports: JavaSourceImports?,
    ): String? {
        if (imports == null) return null
        val resolver = ClassLiteralTypeNameResolver.forImports(imports, classIndex)
        val candidates = sequenceOf(
            target,
            target.replace('/', '.').takeIf { '/' in target },
        ).filterNotNull().distinct()
        return candidates
            .mapNotNull { candidate ->
                resolver.resolve(candidate)
                    ?.takeIf { it.sort == Type.OBJECT }
                    ?.internalName
            }
            .mapNotNull { internalName -> classIndex.findClass(internalName)?.internalName }
            .firstOrNull()
    }

    private fun hasSourceResolutionContext(imports: JavaSourceImports?): Boolean =
        imports != null && (
            imports.explicit.isNotEmpty() ||
                imports.wildcardPackages.isNotEmpty() ||
                imports.ambiguousExplicit.isNotEmpty()
            )
}
