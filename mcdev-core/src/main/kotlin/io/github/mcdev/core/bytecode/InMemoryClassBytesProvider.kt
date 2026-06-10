package io.github.mcdev.core.bytecode

import java.security.MessageDigest

class InMemoryClassBytesProvider(
    private val classes: Map<String, ByteArray>,
    private val entryHashes: Map<String, Long> = emptyMap(),
    private val defaultEntryId: String = "in-memory",
) : ClassBytesProvider {
    init {
        require(classes.keys.none { it.contains('.') }) {
            "class keys must use internal names with slashes, not dots"
        }
    }

    override fun getClassBytes(internalName: String): ByteArray? = classes[internalName]

    override fun classpathEntryIds(): Set<String> =
        if (entryHashes.isEmpty()) setOf(defaultEntryId) else entryHashes.keys

    override fun classpathEntryHash(entryId: String): Long? =
        entryHashes[entryId] ?: if (entryId == defaultEntryId) classes.hashCode().toLong() else null

    fun internalNames(): Set<String> = classes.keys

    fun allClasses(): Map<String, ByteArray> = classes

    override fun classpathHash(): String {
        val digest = MessageDigest.getInstance("SHA-256")
        classes.entries
            .sortedBy { it.key }
            .forEach { (name, bytes) ->
                digest.update(name.toByteArray())
                digest.update(bytes)
            }
        entryHashes.entries
            .sortedBy { it.key }
            .forEach { (id, hash) ->
                digest.update(id.toByteArray())
                digest.update(hash.toString().toByteArray())
            }
        return digest.digest().joinToString("") { "%02x".format(it) }
    }

    companion object {
        fun fromResources(
            resourcePrefix: String,
            classLoader: ClassLoader = InMemoryClassBytesProvider::class.java.classLoader,
        ): InMemoryClassBytesProvider {
            val classes = mutableMapOf<String, ByteArray>()
            val urls = classLoader.getResources(resourcePrefix)
            while (urls.hasMoreElements()) {
                val url = urls.nextElement()
                val path = url.path
                if (path.endsWith(".class")) {
                    val internalName = path
                        .substringAfter(resourcePrefix.trimEnd('/'))
                        .removePrefix("/")
                        .removeSuffix(".class")
                    classLoader.getResourceAsStream("$resourcePrefix/$internalName.class")?.use { stream ->
                        classes[internalName] = stream.readBytes()
                    }
                }
            }
            return InMemoryClassBytesProvider(classes)
        }
    }
}
