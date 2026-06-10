package io.github.mcdev.fixtures

import java.io.InputStream

object FixtureResourceLoader {
    private val classLoader: ClassLoader
        get() = FixtureResourceLoader::class.java.classLoader

    fun exists(path: String): Boolean = classLoader.getResource(normalize(path)) != null

    fun openStream(path: String): InputStream {
        val normalized = normalize(path)
        return classLoader.getResourceAsStream(normalized)
            ?: error("Fixture resource not found: $normalized")
    }

    fun loadText(path: String): String = openStream(path).bufferedReader().use { it.readText() }

    fun loadBytes(path: String): ByteArray = openStream(path).use { it.readBytes() }

    fun loadLines(path: String): List<String> = loadText(path).lineSequence().toList()

    fun listResources(prefix: String): List<String> {
        val normalizedPrefix = normalize(prefix).trimEnd('/')
        val baseUrl = classLoader.getResource(normalizedPrefix)
            ?: classLoader.getResource("$normalizedPrefix/")
            ?: return emptyList()

        return when (baseUrl.protocol) {
            "file" -> listFileResources(java.io.File(baseUrl.toURI()), normalizedPrefix)
            "jar" -> listJarResources(baseUrl, normalizedPrefix)
            else -> emptyList()
        }
    }

    private fun normalize(path: String): String = path.trim().trimStart('/').replace('\\', '/')

    private fun listFileResources(root: java.io.File, prefix: String): List<String> {
        if (!root.exists()) return emptyList()
        return root.walkTopDown()
            .filter { it.isFile }
            .map { file ->
                val relative = file.relativeTo(root).path.replace('\\', '/')
                if (prefix.isEmpty()) relative else "$prefix/$relative"
            }
            .sorted()
            .toList()
    }

    private fun listJarResources(baseUrl: java.net.URL, prefix: String): List<String> {
        val jarPath = baseUrl.path.substringBefore("!").removePrefix("file:")
        val entryPrefix = baseUrl.path.substringAfter("!").removePrefix("/").trimEnd('/') + "/"
        return java.util.jar.JarFile(jarPath).use { jar ->
            jar.entries().asSequence()
                .filter { !it.isDirectory && it.name.startsWith(entryPrefix) }
                .map { entry ->
                    val relative = entry.name.removePrefix(entryPrefix)
                    if (prefix.isEmpty()) relative else "$prefix/$relative"
                }
                .sorted()
                .toList()
        }
    }
}
