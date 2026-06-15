package io.github.mcdev.jdtls.mixin

import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FieldIndexEntry
import io.github.mcdev.core.mixin.JavaTypeDescriptorResolver
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.jdtls.project.McdevProjectSession
import io.github.mcdev.jdtls.project.UriPathSupport
import java.nio.file.Files
import java.nio.file.Path
import kotlin.io.path.extension

internal class SourceBackedClassIndex(
    private val delegate: ClassIndex,
    session: McdevProjectSession,
    currentDocumentUri: String,
    currentBufferText: String,
) : ClassIndex {
    private val sourceClasses = SourceClassScanner.scan(session, currentDocumentUri, currentBufferText)

    override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> {
        val sourceMatches = sourceClasses.values
            .asSequence()
            .filter { it.entry.simpleName.startsWith(prefix, ignoreCase = true) || it.entry.fqn.startsWith(prefix, ignoreCase = true) }
            .map { it.entry }
        return (sourceMatches + delegate.findClasses(prefix, limit).asSequence())
            .distinctBy { it.internalName }
            .take(limit)
            .toList()
    }

    override fun findClass(internalName: String): ClassIndexEntry? =
        sourceClasses[internalName]?.entry ?: delegate.findClass(internalName)

    override fun findClassByFqn(fqn: String): ClassIndexEntry? {
        val internal = fqn.replace('.', '/')
        return sourceClasses[internal]?.entry ?: sourceClasses.values.firstOrNull { it.entry.fqn == fqn }?.entry
            ?: delegate.findClassByFqn(fqn)
    }

    override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> {
        val sourceMethods = sourceClasses[ownerInternalName]?.methods.orEmpty()
        val bytecodeMethods = delegate.getMethods(ownerInternalName)
        return (sourceMethods + bytecodeMethods).distinctBy { it.name to it.descriptor }
    }

    override fun getFields(ownerInternalName: String): List<FieldIndexEntry> =
        delegate.getFields(ownerInternalName)
}

private data class SourceClass(
    val entry: ClassIndexEntry,
    val methods: List<MethodIndexEntry>,
)

private object SourceClassScanner {
    private val packagePattern = Regex("""(?m)^\s*package\s+([\w.]+)\s*;""")
    private val classPattern = Regex(
        """(?:public\s+|protected\s+|private\s+)?(?:abstract\s+|final\s+|static\s+)?(?:class|interface|enum|record)\s+(\w+)""",
    )
    private val methodPattern = Regex(
        """(?m)^\s*(?:@\w+(?:\([^)]*\))?\s*)*(public|protected|private)?\s*(static\s+)?(?:final\s+|abstract\s+|synchronized\s+|native\s+|strictfp\s+)*([\w.$<>\[\]?]+)\s+(\w+)\s*\(([^)]*)\)""",
    )

    fun scan(
        session: McdevProjectSession,
        currentDocumentUri: String,
        currentBufferText: String,
    ): Map<String, SourceClass> {
        val entries = linkedMapOf<String, String>()
        if (currentDocumentUri.endsWith(".java", ignoreCase = true)) {
            entries[currentDocumentUri] = currentBufferText
        }
        session.context.sourceSets.forEach { sourceSet ->
            sourceSet.sourceDirectories.forEach { sourceDir ->
                if (!Files.isDirectory(sourceDir)) return@forEach
                Files.walk(sourceDir).use { stream ->
                    stream
                        .filter { Files.isRegularFile(it) && it.extension.equals("java", ignoreCase = true) }
                        .forEach { file ->
                            val uri = UriPathSupport.pathToUri(file)
                            entries.putIfAbsent(uri, runCatching { Files.readString(file) }.getOrDefault(""))
                        }
                }
            }
        }
        return entries.values.mapNotNull(::scanClass).associateBy { it.entry.internalName }
    }

    private fun scanClass(source: String): SourceClass? {
        val imports = JavaTypeDescriptorResolver.importsFor(source)
        val packageName = packagePattern.find(source)?.groupValues?.get(1).orEmpty()
        val className = classPattern.find(source)?.groupValues?.get(1) ?: return null
        val fqn = if (packageName.isEmpty()) className else "$packageName.$className"
        val internalName = fqn.replace('.', '/')
        val methods = methodPattern.findAll(source)
            .filter { !isConstructor(it.groupValues[4], className) }
            .mapNotNull { match ->
                val returnType = match.groupValues[3]
                val name = match.groupValues[4]
                val params = JavaTypeDescriptorResolver.rawParameterTypes(match.groupValues[5])
                val descriptor = JavaTypeDescriptorResolver.methodDescriptorOrNull(returnType, params, imports)
                    ?: return@mapNotNull null
                MethodIndexEntry(
                    name = name,
                    descriptor = descriptor,
                    isStatic = match.groupValues[2].isNotBlank(),
                    readableSignature = "$name(${params.joinToString(", ") { readableType(it) }}): ${readableType(returnType)}",
                )
            }
            .toList()
        return SourceClass(
            entry = ClassIndexEntry(
                simpleName = className,
                packageName = packageName,
                internalName = internalName,
            ),
            methods = methods,
        )
    }

    private fun isConstructor(name: String, className: String): Boolean = name == className

    private fun normalize(rawType: String): Pair<String, Int> {
        var type = rawType
            .replace(Regex("""@\w+(?:\([^)]*\))?\s*"""), "")
            .replace(Regex("""\b(final|volatile|transient)\b"""), "")
            .trim()
        var arrayDepth = 0
        if (type.endsWith("...")) {
            arrayDepth++
            type = type.removeSuffix("...").trim()
        }
        while (type.endsWith("[]")) {
            arrayDepth++
            type = type.removeSuffix("[]").trim()
        }
        return type to arrayDepth
    }

    private fun readableType(rawType: String): String =
        normalize(rawType).let { (base, depth) -> base.substringBefore('<') + "[]".repeat(depth) }
}
