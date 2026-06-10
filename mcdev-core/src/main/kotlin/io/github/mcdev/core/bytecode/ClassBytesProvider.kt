package io.github.mcdev.core.bytecode

/**
 * Supplies raw class bytes from classpath entries without scanning jars on demand during completion.
 */
interface ClassBytesProvider {
    fun getClassBytes(internalName: String): ByteArray?

    fun classpathEntryIds(): Set<String>

    fun classpathEntryHash(entryId: String): Long?

    fun classpathHash(): String
}
