package io.github.mcdev.core.project

import java.nio.file.Path

internal object ProjectPathFilters {
    private val EXCLUDED_DIRECTORY_NAMES = setOf("build", ".gradle", "node_modules")

    private val GRADLE_MAPPING_WHITELIST_PREFIXES = listOf(
        listOf("loom-cache"),
        listOf("caches", "fabric-loom"),
        listOf("caches", "modules-2", "files-2.1", "net.fabricmc", "yarn"),
        listOf("caches", "modules-2", "files-2.1", "net.minecraft"),
        listOf("caches", "modules-2", "files-2.1", "net.minecraftforge"),
        listOf("caches", "modules-2", "files-2.1", "net.neoforged"),
    )

    private val BUILD_MAPPING_WHITELIST_PREFIXES = listOf(
        listOf("loom-cache"),
        listOf("createSrgToMcp"),
        listOf("tmp"),
    )

    fun isUnderExcludedDirectory(path: Path): Boolean {
        for (index in 0 until path.nameCount) {
            if (path.getName(index).toString().lowercase() in EXCLUDED_DIRECTORY_NAMES) {
                return true
            }
        }
        return false
    }

    fun isExcludedFromMappingDiscovery(relativePath: Path): Boolean {
        val path = relativePath.normalize()
        for (index in 0 until path.nameCount) {
            when (path.getName(index).toString().lowercase()) {
                ".gradle" -> {
                    if (!isUnderWhitelistedPrefix(path, index + 1, GRADLE_MAPPING_WHITELIST_PREFIXES)) {
                        return true
                    }
                }
                "build" -> {
                    if (!isUnderWhitelistedPrefix(path, index + 1, BUILD_MAPPING_WHITELIST_PREFIXES)) {
                        return true
                    }
                }
                "node_modules" -> return true
            }
        }
        return false
    }

    private fun isUnderWhitelistedPrefix(path: Path, startIndex: Int, prefixes: List<List<String>>): Boolean =
        prefixes.any { prefix -> matchesPrefix(path, startIndex, prefix) }

    private fun matchesPrefix(path: Path, startIndex: Int, prefix: List<String>): Boolean {
        for (index in prefix.indices) {
            val pathIndex = startIndex + index
            if (pathIndex >= path.nameCount) {
                return false
            }
            if (path.getName(pathIndex).toString().lowercase() != prefix[index].lowercase()) {
                return false
            }
        }
        return true
    }
}
