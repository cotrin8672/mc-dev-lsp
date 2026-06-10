package io.github.mcdev.core.fixtures

import java.io.InputStream

object CoreFixtureResourceLoader {
    private val classLoader: ClassLoader
        get() = CoreFixtureResourceLoader::class.java.classLoader

    fun exists(path: String): Boolean = classLoader.getResource(normalize(path)) != null

    fun openStream(path: String): InputStream {
        val normalized = normalize(path)
        return classLoader.getResourceAsStream(normalized)
            ?: error("Fixture resource not found: $normalized")
    }

    fun loadText(path: String): String = openStream(path).bufferedReader().use { it.readText() }

    fun loadBytes(path: String): ByteArray = openStream(path).use { it.readBytes() }

    private fun normalize(path: String): String = path.trim().trimStart('/').replace('\\', '/')
}
