package io.github.mcdev.jdtls.project

import io.github.mcdev.core.bytecode.ClassBytesProvider
import java.nio.file.Files
import java.nio.file.Path
import java.security.MessageDigest
import java.util.jar.JarFile
import kotlin.io.path.extension
import kotlin.io.path.isDirectory
import kotlin.io.path.isRegularFile

class ClasspathClassBytesProvider(
    val entries: List<Path>,
    private val entryTimestamps: Map<Path, Long> = emptyMap(),
) : ClassBytesProvider {
    private val classes: Map<String, ByteArray> by lazy { loadClassesFromEntries(entries) }

    override fun getClassBytes(internalName: String): ByteArray? = classes[internalName]

    override fun classpathEntryIds(): Set<String> = entries.map { it.toString() }.toSet()

    override fun classpathEntryHash(entryId: String): Long? {
        val path = entries.firstOrNull { it.toString() == entryId } ?: return null
        return entryTimestamps[path] ?: runCatching {
            Files.getLastModifiedTime(path).toMillis()
        }.getOrNull()
    }

    override fun classpathHash(): String {
        val digest = MessageDigest.getInstance("SHA-256")
        classes.entries
            .sortedBy { it.key }
            .forEach { (name, bytes) ->
                digest.update(name.toByteArray())
                digest.update(bytes)
            }
        entries.sortedBy { it.toString() }.forEach { path ->
            digest.update(path.toString().toByteArray())
            digest.update((classpathEntryHash(path.toString()) ?: 0L).toString().toByteArray())
        }
        return digest.digest().joinToString("") { "%02x".format(it) }
    }

    fun classCount(): Int = classes.size

    fun internalNames(): Set<String> = classes.keys

    companion object {
        fun loadClassesFromEntries(entries: List<Path>): Map<String, ByteArray> {
            val loaded = linkedMapOf<String, ByteArray>()
            entries.sortedBy { it.toString() }.forEach { entry ->
                when {
                    entry.isDirectory() -> loadDirectory(entry, loaded)
                    entry.isRegularFile() && entry.extension.equals("jar", ignoreCase = true) -> loadJar(entry, loaded)
                }
            }
            return loaded
        }

        private fun loadDirectory(root: Path, loaded: MutableMap<String, ByteArray>) {
            Files.walk(root).use { stream ->
                stream
                    .filter { it.isRegularFile() && it.toString().endsWith(".class") }
                    .forEach { classFile ->
                        val relative = root.relativize(classFile).toString().replace('\\', '/').removeSuffix(".class")
                        if (relative !in loaded) {
                            loaded[relative] = Files.readAllBytes(classFile)
                        }
                    }
            }
        }

        private fun loadJar(jarPath: Path, loaded: MutableMap<String, ByteArray>) {
            JarFile(jarPath.toFile()).use { jar ->
                jar.entries().asSequence()
                    .filter { !it.isDirectory && it.name.endsWith(".class") }
                    .forEach { entry ->
                        val internalName = entry.name.removeSuffix(".class")
                        if (internalName !in loaded) {
                            jar.getInputStream(entry).use { stream ->
                                loaded[internalName] = stream.readBytes()
                            }
                        }
                    }
            }
        }
    }
}
