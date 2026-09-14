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
import java.nio.file.attribute.BasicFileAttributes
import java.nio.file.attribute.FileTime
import java.util.LinkedHashMap
import java.util.concurrent.atomic.AtomicInteger
import kotlin.io.path.extension

internal interface ProjectSourceQuery {
    fun findClasses(prefix: String, limit: Int): Sequence<ClassIndexEntry>

    fun findClass(internalName: String): ClassIndexEntry?

    fun findClassByFqn(fqn: String): ClassIndexEntry?

    fun getMethods(ownerInternalName: String): List<MethodIndexEntry>

    fun getFields(ownerInternalName: String): List<FieldIndexEntry> = emptyList()

    fun coversProjectDependencies(): Boolean = false
}

internal class BufferProjectSourceQuery private constructor(
    private val sourceClass: SourceClass?,
) : ProjectSourceQuery {
    companion object {
        fun fromBuffer(bufferText: String): BufferProjectSourceQuery =
            BufferProjectSourceQuery(SourceClassParser.parse(bufferText))
    }

    override fun findClasses(prefix: String, limit: Int): Sequence<ClassIndexEntry> =
        (sourceClass?.entry?.let { sequenceOf(it) } ?: emptySequence<ClassIndexEntry>())
            .filter {
                it.simpleName.startsWith(prefix, ignoreCase = true) ||
                    it.fqn.startsWith(prefix, ignoreCase = true)
            }
            .take(limit)

    override fun findClass(internalName: String): ClassIndexEntry? =
        sourceClass?.entry?.takeIf { it.internalName == internalName }

    override fun findClassByFqn(fqn: String): ClassIndexEntry? {
        val clazz = sourceClass ?: return null
        val internal = fqn.replace('.', '/')
        return clazz.entry.takeIf { it.internalName == internal || it.fqn == fqn }
    }

    override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> =
        sourceClass?.takeIf { it.entry.internalName == ownerInternalName }?.methods.orEmpty()
}

internal class ScannedProjectSourceSnapshot private constructor(
    private val sourceClasses: Map<String, SourceClass>,
) : ProjectSourceQuery {
    companion object {
        fun fromSession(
            session: McdevProjectSession,
            currentDocumentUri: String,
            currentBufferText: String,
        ): ScannedProjectSourceSnapshot =
            ScannedProjectSourceSnapshot(
                SourceClassScanner.scan(session, currentDocumentUri, currentBufferText),
            )
    }

    override fun findClasses(prefix: String, limit: Int): Sequence<ClassIndexEntry> =
        sourceClasses.values
            .asSequence()
            .filter {
                it.entry.simpleName.startsWith(prefix, ignoreCase = true) ||
                    it.entry.fqn.startsWith(prefix, ignoreCase = true)
            }
            .map { it.entry }
            .take(limit)

    override fun findClass(internalName: String): ClassIndexEntry? =
        sourceClasses[internalName]?.entry

    override fun findClassByFqn(fqn: String): ClassIndexEntry? {
        val internal = fqn.replace('.', '/')
        return sourceClasses[internal]?.entry
            ?: sourceClasses.values.firstOrNull { it.entry.fqn == fqn }?.entry
    }

    override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> =
        sourceClasses[ownerInternalName]?.methods.orEmpty()
}

internal class CompositeProjectSourceQuery(
    private val delegates: List<ProjectSourceQuery>,
) : ProjectSourceQuery {
    override fun findClasses(prefix: String, limit: Int): Sequence<ClassIndexEntry> =
        sequence {
            if (limit <= 0) {
                return@sequence
            }
            val seen = LinkedHashSet<String>()
            var count = 0
            for (delegate in delegates) {
                if (count >= limit) {
                    break
                }
                for (entry in delegate.findClasses(prefix, limit)) {
                    if (seen.add(entry.internalName)) {
                        yield(entry)
                        count++
                        if (count >= limit) {
                            break
                        }
                    }
                }
            }
        }

    override fun findClass(internalName: String): ClassIndexEntry? {
        for (delegate in delegates) {
            delegate.findClass(internalName)?.let { return it }
        }
        return null
    }

    override fun findClassByFqn(fqn: String): ClassIndexEntry? {
        for (delegate in delegates) {
            delegate.findClassByFqn(fqn)?.let { return it }
        }
        return null
    }

    override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> {
        val seen = LinkedHashSet<Pair<String, String>>()
        val results = mutableListOf<MethodIndexEntry>()
        for (delegate in delegates) {
            for (method in delegate.getMethods(ownerInternalName)) {
                if (seen.add(method.name to method.descriptor)) {
                    results.add(method)
                }
            }
        }
        return results
    }

    override fun getFields(ownerInternalName: String): List<FieldIndexEntry> {
        val seen = LinkedHashSet<Pair<String, String>>()
        val results = mutableListOf<FieldIndexEntry>()
        for (delegate in delegates) {
            for (field in delegate.getFields(ownerInternalName)) {
                if (seen.add(field.name to field.descriptor)) {
                    results.add(field)
                }
            }
        }
        return results
    }

    override fun coversProjectDependencies(): Boolean =
        delegates.any { it.coversProjectDependencies() }
}

internal class SourceBackedClassIndex(
    private val delegate: ClassIndex,
    private val sourceQuery: ProjectSourceQuery,
) : ClassIndex {
    private val authoritativeSource: Boolean
        get() = sourceQuery.coversProjectDependencies()

    override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> {
        val sourceEntries = sourceQuery.findClasses(prefix, limit).take(limit).toList()
        return if (authoritativeSource) {
            sourceEntries
        } else {
            (sourceEntries.asSequence() + delegate.findClasses(prefix, limit).asSequence())
                .distinctBy { it.internalName }
                .take(limit)
                .toList()
        }
    }

    override fun findClass(internalName: String): ClassIndexEntry? {
        val source = sourceQuery.findClass(internalName)
        return if (sourceQuery.coversProjectDependencies()) source else source ?: delegate.findClass(internalName)
    }

    override fun findClassByFqn(fqn: String): ClassIndexEntry? {
        val source = sourceQuery.findClassByFqn(fqn)
        return if (sourceQuery.coversProjectDependencies()) source else source ?: delegate.findClassByFqn(fqn)
    }

    override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> {
        val sourceMethods = sourceQuery.getMethods(ownerInternalName)
        if (sourceQuery.coversProjectDependencies()) {
            return sourceMethods
        }
        val bytecodeMethods = delegate.getMethods(ownerInternalName)
        return (sourceMethods + bytecodeMethods).distinctBy { it.name to it.descriptor }
    }

    override fun getFields(ownerInternalName: String): List<FieldIndexEntry> {
        val sourceFields = sourceQuery.getFields(ownerInternalName)
        if (sourceQuery.coversProjectDependencies()) {
            return sourceFields
        }
        val bytecodeFields = delegate.getFields(ownerInternalName)
        return (sourceFields + bytecodeFields).distinctBy { it.name to it.descriptor }
    }
}

internal data class SourceClass(
    val entry: ClassIndexEntry,
    val methods: List<MethodIndexEntry>,
)

private data class DiskFileFingerprint(
    val lastModifiedTime: FileTime,
    val size: Long,
)

private data class DiskFileCacheEntry(
    val fingerprint: DiskFileFingerprint,
    val sourceClass: SourceClass?,
)

private class ProjectSourceSnapshot {
    val entries = linkedMapOf<Path, DiskFileCacheEntry>()
}

internal object SourceClassParser {
    private val packagePattern = Regex("""(?m)^\s*package\s+([\w.]+)\s*;""")
    private val classPattern = Regex(
        """(?:public\s+|protected\s+|private\s+)?(?:abstract\s+|final\s+|static\s+)?(?:class|interface|enum|record)\s+(\w+)""",
    )
    private val methodPattern = Regex(
        """(?m)^\s*(?:@\w+(?:\([^)]*\))?\s*)*(public|protected|private)?\s*(static\s+)?(?:final\s+|abstract\s+|synchronized\s+|native\s+|strictfp\s+)*([\w.$<>\[\]?]+)\s+(\w+)\s*\(([^)]*)\)""",
    )

    fun parse(source: String): SourceClass? {
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

internal object SourceClassScanner {
    internal const val MaxDiskCacheEntries = 8

    private val projectCache = object : LinkedHashMap<Path, ProjectSourceSnapshot>(MaxDiskCacheEntries + 1, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<Path, ProjectSourceSnapshot>): Boolean =
            size > MaxDiskCacheEntries
    }

    private val diskReadCountValue = AtomicInteger(0)
    private val diskParseCountValue = AtomicInteger(0)
    private val overlayParseCountValue = AtomicInteger(0)

    internal fun diskReadCount(): Int = diskReadCountValue.get()

    internal fun diskParseCount(): Int = diskParseCountValue.get()

    internal fun overlayParseCount(): Int = overlayParseCountValue.get()

    internal fun diskCacheSize(): Int = synchronized(projectCache) { projectCache.size }

    internal fun resetMetrics() {
        diskReadCountValue.set(0)
        diskParseCountValue.set(0)
        overlayParseCountValue.set(0)
    }

    internal fun clearDiskCache() {
        synchronized(projectCache) {
            projectCache.clear()
        }
    }

    internal fun scan(
        session: McdevProjectSession,
        currentDocumentUri: String,
        currentBufferText: String,
    ): Map<String, SourceClass> {
        val currentPath = currentDocumentPath(currentDocumentUri)
        val seenPaths = linkedSetOf<Path>()
        val classes = linkedMapOf<String, SourceClass>()
        val projectRoot = session.context.root.toAbsolutePath().normalize()
        val snapshot = synchronized(projectCache) {
            projectCache.getOrPut(projectRoot) { ProjectSourceSnapshot() }
        }

        session.context.sourceSets.forEach { sourceSet ->
            sourceSet.sourceDirectories.forEach { sourceDir ->
                if (!Files.isDirectory(sourceDir)) return@forEach
                Files.walk(sourceDir).use { stream ->
                    stream
                        .filter { Files.isRegularFile(it) && it.extension.equals("java", ignoreCase = true) }
                        .forEach { file ->
                            val absolutePath = file.toAbsolutePath().normalize()
                            seenPaths.add(absolutePath)
                            if (absolutePath == currentPath) {
                                return@forEach
                            }
                            loadDiskFile(snapshot, absolutePath)?.let { sourceClass ->
                                classes[sourceClass.entry.internalName] = sourceClass
                            }
                        }
                }
            }
        }

        evictDeletedDiskFiles(snapshot, seenPaths)

        if (currentPath != null) {
            overlayParseCountValue.incrementAndGet()
            SourceClassParser.parse(currentBufferText)?.let { overlayClass ->
                classes[overlayClass.entry.internalName] = overlayClass
            }
        }

        return classes
    }

    private fun currentDocumentPath(currentDocumentUri: String): Path? {
        if (!currentDocumentUri.endsWith(".java", ignoreCase = true)) {
            return null
        }
        return runCatching { UriPathSupport.uriToPath(currentDocumentUri).toAbsolutePath().normalize() }.getOrNull()
    }

    private fun loadDiskFile(snapshot: ProjectSourceSnapshot, absolutePath: Path): SourceClass? {
        val attrs = runCatching { Files.readAttributes(absolutePath, BasicFileAttributes::class.java) }.getOrNull()
            ?: return null
        val fingerprint = DiskFileFingerprint(attrs.lastModifiedTime(), attrs.size())

        synchronized(projectCache) {
            snapshot.entries[absolutePath]?.takeIf { it.fingerprint == fingerprint }?.let { return it.sourceClass }
        }

        val content = runCatching { Files.readString(absolutePath) }.getOrDefault("")
        diskReadCountValue.incrementAndGet()
        val parsed = SourceClassParser.parse(content)
        diskParseCountValue.incrementAndGet()

        synchronized(projectCache) {
            snapshot.entries[absolutePath] = DiskFileCacheEntry(fingerprint, parsed)
        }
        return parsed
    }

    private fun evictDeletedDiskFiles(snapshot: ProjectSourceSnapshot, seenPaths: Set<Path>) {
        synchronized(projectCache) {
            snapshot.entries.keys.retainAll(seenPaths)
        }
    }
}
