package io.github.mcdev.core.project

import io.github.mcdev.core.at.AccessTransformerParser
import io.github.mcdev.core.at.AccessTransformerParseResult
import io.github.mcdev.core.aw.AccessWidenerParser
import io.github.mcdev.core.aw.AccessWidenerParseResult
import java.nio.file.Files
import java.nio.file.Path
import kotlin.io.path.exists
import kotlin.io.path.extension
import kotlin.io.path.name
import kotlin.io.path.readText

object AwAtDiscoveryService {

    fun discoverAccessWideners(root: Path): List<AccessWidenerRef> {
        if (!root.exists()) return emptyList()
        val paths = Files.walk(root).use { stream ->
            stream
                .filter { Files.isRegularFile(it) }
                .filter { isAccessWidenerFile(it) }
                .filter { path -> !ProjectPathFilters.isUnderExcludedDirectory(path) }
                .sorted()
                .toList()
        }
        return paths.map { path -> parseAccessWidener(path) }.sortedBy { it.path.toString() }
    }

    fun discoverAccessTransformers(root: Path): List<AccessTransformerRef> {
        if (!root.exists()) return emptyList()
        val paths = Files.walk(root).use { stream ->
            stream
                .filter { Files.isRegularFile(it) }
                .filter { isAccessTransformerFile(it) }
                .filter { path -> !ProjectPathFilters.isUnderExcludedDirectory(path) }
                .sorted()
                .toList()
        }
        return paths.map { path -> parseAccessTransformer(path) }.sortedBy { it.path.toString() }
    }

    fun isAccessWidenerFile(path: Path): Boolean {
        val name = path.name.lowercase()
        val extension = path.extension.lowercase()
        return extension == "accesswidener" ||
            extension == "aw" ||
            name.endsWith(".accesswidener")
    }

    fun isAccessTransformerFile(path: Path): Boolean {
        val name = path.name.lowercase()
        return name == "accesstransformer.cfg" ||
            name.endsWith("_at.cfg") ||
            name.endsWith(".at") ||
            (path.extension.lowercase() == "cfg" && name.contains("access") && name.contains("transform"))
    }

    fun parseAccessWidener(path: Path): AccessWidenerRef {
        val content = path.readText()
        val result = AccessWidenerParser.parse(content)
        return when (result) {
            is AccessWidenerParseResult.Success -> AccessWidenerRef(
                path = path,
                namespace = result.file.namespace,
                parsed = result.file,
            )
            is AccessWidenerParseResult.Failure -> AccessWidenerRef(path = path, namespace = null, parsed = null)
        }
    }

    fun parseAccessTransformer(path: Path): AccessTransformerRef {
        val content = path.readText()
        val result = AccessTransformerParser.parse(content)
        return when (result) {
            is AccessTransformerParseResult.Success -> AccessTransformerRef(path = path, parsed = result.file)
            is AccessTransformerParseResult.Failure -> AccessTransformerRef(path = path, parsed = null)
        }
    }

}
