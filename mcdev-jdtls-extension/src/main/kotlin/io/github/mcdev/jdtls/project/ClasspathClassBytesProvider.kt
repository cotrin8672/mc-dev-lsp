package io.github.mcdev.jdtls.project

import io.github.mcdev.core.bytecode.ClassBytesProvider
import java.nio.file.Files
import java.nio.file.Path
import java.security.MessageDigest
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.locks.ReentrantLock
import java.util.jar.JarFile
import kotlin.concurrent.withLock
import kotlin.io.path.extension
import kotlin.io.path.isDirectory
import kotlin.io.path.isRegularFile

class ClasspathClassBytesProvider(
    val entries: List<Path>,
    private val entryTimestamps: Map<Path, Long> = emptyMap(),
    private val maxCachedOwners: Int = DEFAULT_MAX_CACHED_OWNERS,
    private val bytesReadCounter: AtomicInteger = AtomicInteger(0),
) : ClassBytesProvider {
    private val catalog: List<String> by lazy { discoverInternalNamesFromEntries(entries) }
    private val fqnToInternalName: Map<String, String> by lazy { buildFqnToInternalNameMap(catalog) }
    private val byteCache = LruByteCache(maxCachedOwners)
    private val loadStripes = Array(LOAD_STRIPE_COUNT) { ReentrantLock() }

    internal fun byteReadCount(): Int = bytesReadCounter.get()

    override fun getClassBytes(internalName: String): ByteArray? {
        byteCache.get(internalName)?.let { return it }
        val stripe = loadStripes[internalName.hashCode() and (LOAD_STRIPE_COUNT - 1)]
        return stripe.withLock {
            byteCache.get(internalName)?.let { return@withLock it }
            val bytes = readClassBytesFromEntries(internalName, countRead = true) ?: return@withLock null
            byteCache.put(internalName, bytes)
            bytes
        }
    }

    override fun classpathEntryIds(): Set<String> = entries.map { it.toString() }.toSet()

    override fun classpathEntryHash(entryId: String): Long? {
        val path = entries.firstOrNull { it.toString() == entryId } ?: return null
        return entryTimestamps[path] ?: runCatching {
            Files.getLastModifiedTime(path).toMillis()
        }.getOrNull()
    }

    override fun classpathHash(): String {
        val digest = MessageDigest.getInstance("SHA-256")
        catalog.forEach { name ->
            digest.update(name.toByteArray())
            readClassBytesFromEntries(name, countRead = false)?.let { digest.update(it) }
        }
        entries.sortedBy { it.toString() }.forEach { path ->
            digest.update(path.toString().toByteArray())
            digest.update((classpathEntryHash(path.toString()) ?: 0L).toString().toByteArray())
        }
        return digest.digest().joinToString("") { "%02x".format(it) }
    }

    fun classCount(): Int = catalog.size

    fun findInternalNames(prefix: String, limit: Int): List<String> {
        if (limit <= 0) {
            return emptyList()
        }
        val normalizedPrefix = prefix.lowercase()
        return catalog.asSequence()
            .filter { internalName -> internalNameMatchesPrefix(internalName, normalizedPrefix) }
            .take(limit)
            .toList()
    }

    fun containsInternalName(internalName: String): Boolean =
        catalog.binarySearch(internalName) >= 0

    fun findInternalNameByFqn(fqn: String): String? =
        fqnToInternalName[fqn.lowercase()]

    fun internalNames(): Set<String> = catalog.toSet()

    private fun readClassBytesFromEntries(internalName: String, countRead: Boolean): ByteArray? {
        val classEntryName = "$internalName.class"
        entries.sortedBy { it.toString() }.forEach { entry ->
            when {
                entry.isDirectory() -> {
                    val classFile = entry.resolve(classEntryName)
                    if (classFile.isRegularFile()) {
                        if (countRead) {
                            bytesReadCounter.incrementAndGet()
                        }
                        return Files.readAllBytes(classFile)
                    }
                }
                entry.isRegularFile() && entry.extension.equals("jar", ignoreCase = true) -> {
                    JarFile(entry.toFile()).use { jar ->
                        val jarEntry = jar.getJarEntry(classEntryName) ?: return@use
                        if (!jarEntry.isDirectory) {
                            if (countRead) {
                                bytesReadCounter.incrementAndGet()
                            }
                            return jar.getInputStream(jarEntry).use { it.readBytes() }
                        }
                    }
                }
            }
        }
        return null
    }

    private class LruByteCache(maxSize: Int) {
        private val lock = ReentrantLock()
        private val map = object : LinkedHashMap<String, ByteArray>(maxSize, 0.75f, true) {
            override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, ByteArray>?): Boolean =
                size > maxSize
        }

        fun get(key: String): ByteArray? = lock.withLock { map[key] }

        fun put(key: String, value: ByteArray) = lock.withLock { map[key] = value }
    }

    companion object {
        const val DEFAULT_MAX_CACHED_OWNERS = 256
        private const val LOAD_STRIPE_COUNT = 64

        fun discoverInternalNamesFromEntries(entries: List<Path>): List<String> {
            val discovered = linkedSetOf<String>()
            entries.sortedBy { it.toString() }.forEach { entry ->
                when {
                    entry.isDirectory() -> discoverDirectory(entry, discovered)
                    entry.isRegularFile() && entry.extension.equals("jar", ignoreCase = true) -> discoverJar(entry, discovered)
                }
            }
            return discovered.sorted()
        }

        internal fun buildFqnToInternalNameMap(catalog: List<String>): Map<String, String> {
            val map = HashMap<String, String>(catalog.size)
            catalog.forEach { internalName ->
                val key = internalName.replace('/', '.').lowercase()
                map.putIfAbsent(key, internalName)
            }
            return map
        }

        private fun internalNameMatchesPrefix(internalName: String, normalizedPrefix: String): Boolean {
            if (normalizedPrefix.isEmpty()) {
                return true
            }
            if (internalName.lowercase().startsWith(normalizedPrefix)) {
                return true
            }
            if (internalName.replace('/', '.').lowercase().startsWith(normalizedPrefix)) {
                return true
            }
            if (internalName.substringAfterLast('/').lowercase().startsWith(normalizedPrefix)) {
                return true
            }
            return false
        }

        private fun discoverDirectory(root: Path, discovered: MutableSet<String>) {
            Files.walk(root).use { stream ->
                stream
                    .filter { it.isRegularFile() && it.toString().endsWith(".class") }
                    .forEach { classFile ->
                        val relative = root.relativize(classFile).toString().replace('\\', '/').removeSuffix(".class")
                        if (relative !in discovered) {
                            discovered.add(relative)
                        }
                    }
            }
        }

        private fun discoverJar(jarPath: Path, discovered: MutableSet<String>) {
            JarFile(jarPath.toFile()).use { jar ->
                jar.entries().asSequence()
                    .filter { !it.isDirectory && it.name.endsWith(".class") }
                    .forEach { entry ->
                        val internalName = entry.name.removeSuffix(".class")
                        if (internalName !in discovered) {
                            discovered.add(internalName)
                        }
                    }
            }
        }
    }
}
