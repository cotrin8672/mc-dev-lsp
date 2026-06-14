package io.github.mcdev.core.project

import io.github.mcdev.core.mapping.MappingParseResult
import io.github.mcdev.core.mapping.ProjectMappingContext
import io.github.mcdev.core.mapping.SrgParser
import io.github.mcdev.core.mapping.TinyV2Parser
import io.github.mcdev.core.mapping.asCompositeResolver
import io.github.mcdev.core.model.MappingNamespace
import java.nio.file.Files
import java.nio.file.Path
import kotlin.io.path.exists
import kotlin.io.path.extension
import kotlin.io.path.name
import kotlin.io.path.readText

object MappingDiscoveryService {
    private val MAPPING_EXTENSIONS = setOf("tiny", "srg", "tsrg")
    fun discoverMappingFiles(root: Path): List<Path> {
        if (!root.exists()) return emptyList()
        val normalizedRoot = root.toAbsolutePath().normalize()
        return Files.walk(normalizedRoot).use { stream ->
            stream
                .filter { Files.isRegularFile(it) }
                .filter { path ->
                    val extension = path.extension.lowercase()
                    extension in MAPPING_EXTENSIONS ||
                        (path.name.endsWith(".tiny") && extension.isEmpty())
                }
                .filter { path ->
                    val relativePath = normalizedRoot.relativize(path.toAbsolutePath().normalize())
                    !ProjectPathFilters.isExcludedFromMappingDiscovery(relativePath)
                }
                .sorted()
                .toList()
        }
    }

    fun discoverMappingContext(root: Path, platform: ModPlatform): ProjectMappingContext {
        val files = discoverMappingFiles(root)
        return buildMappingContext(files, platform)
    }

    fun buildMappingContext(files: List<Path>, platform: ModPlatform): ProjectMappingContext {
        val parsedSets = files.mapNotNull { path ->
            when (val result = parseMappingFile(path)) {
                is MappingParseResult.Success -> result.mappings
                is MappingParseResult.Failure -> null
            }
        }

        if (parsedSets.isEmpty()) {
            return emptyMappingContext()
        }

        val mergedNamespaces = parsedSets.flatMap { it.namespaces }.distinct()
        val availableNamespaces = mergedNamespaces.toSet()
        val resolver = parsedSets.asCompositeResolver()

        val sourceNamespace = defaultSourceNamespace(platform, availableNamespaces)
        val runtimeNamespace = defaultRuntimeNamespace(platform, availableNamespaces, sourceNamespace)
        val awNamespace = if (availableNamespaces.contains(MappingNamespace.NAMED)) {
            MappingNamespace.NAMED
        } else {
            availableNamespaces.firstOrNull()
        }
        val atNamespace = if (availableNamespaces.contains(MappingNamespace.SRG)) {
            MappingNamespace.SRG
        } else {
            availableNamespaces.firstOrNull()
        }

        return ProjectMappingContext(
            sourceNamespace = sourceNamespace,
            runtimeNamespace = runtimeNamespace,
            awNamespace = awNamespace,
            atNamespace = atNamespace,
            availableNamespaces = availableNamespaces,
            resolver = resolver,
        )
    }

    fun parseMappingFile(path: Path): MappingParseResult {
        val text = path.readText()
        val extension = path.extension.lowercase()
        val name = path.name.lowercase()
        return when {
            extension == "tiny" || name.endsWith(".tiny") -> TinyV2Parser.parse(text)
            extension == "srg" || extension == "tsrg" -> SrgParser.parse(text)
            text.trimStart().startsWith("tiny\t") -> TinyV2Parser.parse(text)
            text.contains("CL:") || text.contains("MD:") -> SrgParser.parse(text)
            else -> MappingParseResult.Failure(1, "unknown mapping format for ${path.name}")
        }
    }

    private fun emptyMappingContext(): ProjectMappingContext =
        ProjectMappingContext(
            sourceNamespace = MappingNamespace.NAMED,
            runtimeNamespace = MappingNamespace.INTERMEDIARY,
            awNamespace = null,
            atNamespace = null,
            availableNamespaces = emptySet(),
            resolver = EmptyMappingResolver,
        )

    private fun defaultSourceNamespace(
        platform: ModPlatform,
        available: Set<MappingNamespace>,
    ): MappingNamespace = when (platform) {
        ModPlatform.FABRIC -> preferredNamespace(available, MappingNamespace.NAMED, MappingNamespace.INTERMEDIARY)
        ModPlatform.FORGE, ModPlatform.NEOFORGE ->
            preferredNamespace(available, MappingNamespace.MCP, MappingNamespace.SRG, MappingNamespace.NAMED)
        ModPlatform.UNKNOWN -> available.firstOrNull() ?: MappingNamespace.NAMED
    }

    private fun defaultRuntimeNamespace(
        platform: ModPlatform,
        available: Set<MappingNamespace>,
        source: MappingNamespace,
    ): MappingNamespace = when (platform) {
        ModPlatform.FABRIC ->
            preferredNamespace(available, MappingNamespace.INTERMEDIARY, MappingNamespace.OFFICIAL)
                .takeIf { it != source } ?: source
        ModPlatform.FORGE, ModPlatform.NEOFORGE ->
            preferredNamespace(available, MappingNamespace.SRG, MappingNamespace.OFFICIAL)
                .takeIf { it != source } ?: source
        ModPlatform.UNKNOWN ->
            available.firstOrNull { it != source } ?: source
    }

    private fun preferredNamespace(
        available: Set<MappingNamespace>,
        vararg preferences: MappingNamespace,
    ): MappingNamespace {
        for (preference in preferences) {
            if (available.contains(preference)) return preference
        }
        return available.firstOrNull() ?: MappingNamespace.NAMED
    }
}
