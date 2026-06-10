package io.github.mcdev.core.project

import io.github.mcdev.core.model.MappingNamespace

object InfoService {
    fun formatLines(
        context: ProjectContext,
        protocolVersion: Int = 1,
        extensionVersion: String = "0.1.0",
    ): List<String> {
        val lines = mutableListOf<String>()

        lines += "Project: ${context.projectId}"
        lines += "Root: ${context.root}"
        lines += "Platform: ${formatPlatform(context.platform)}"
        lines += formatMappingsLine(context)
        lines += "Source namespace: ${formatNamespace(context.mappings.sourceNamespace)}"
        lines += "Runtime namespace: ${formatNamespace(context.mappings.runtimeNamespace)}"
        lines += formatMinecraftJarLine(context)
        lines += formatMixinConfigLine(context)
        lines += formatAccessWidenerLine(context)
        lines += formatAccessTransformerLine(context)
        lines += "Classpath entries: ${context.classpath.entryCount}"
        lines += "Class index: ${formatIndexState(context.indexState)}"
        lines += "Bytecode index: ${formatIndexState(context.indexState)}"
        lines += "Protocol: $protocolVersion"
        lines += "Extension: $extensionVersion"

        return lines
    }

    fun format(context: ProjectContext, protocolVersion: Int = 1, extensionVersion: String = "0.1.0"): String =
        formatLines(context, protocolVersion, extensionVersion).joinToString("\n")

    private fun formatPlatform(platform: ModPlatform): String = when (platform) {
        ModPlatform.FABRIC -> "Fabric"
        ModPlatform.FORGE -> "Forge"
        ModPlatform.NEOFORGE -> "NeoForge"
        ModPlatform.UNKNOWN -> "Unknown"
    }

    private fun formatNamespace(namespace: MappingNamespace): String = when (namespace) {
        MappingNamespace.NAMED -> "named"
        MappingNamespace.INTERMEDIARY -> "intermediary"
        MappingNamespace.OFFICIAL -> "official"
        MappingNamespace.SRG -> "srg"
        MappingNamespace.MCP -> "mcp"
    }

    private fun formatMappingsLine(context: ProjectContext): String {
        val available = context.mappings.availableNamespaces
        if (available.isEmpty()) return "Mappings: none"
        val source = formatNamespace(context.mappings.sourceNamespace)
        val runtime = formatNamespace(context.mappings.runtimeNamespace)
        val status = if (available.size >= 2) "loaded" else "partial"
        return "Mappings: $source <-> $runtime $status"
    }

    private fun formatMinecraftJarLine(context: ProjectContext): String {
        val jars = context.minecraftJars.ifEmpty { context.classpath.minecraftJars }
        return if (jars.isNotEmpty()) "Minecraft jar: found" else "Minecraft jar: none"
    }

    private fun formatMixinConfigLine(context: ProjectContext): String {
        val count = context.mixinConfigs.size
        return when (count) {
            0 -> "Mixin config: none"
            1 -> "Mixin config: 1 file"
            else -> "Mixin config: $count files"
        }
    }

    private fun formatAccessWidenerLine(context: ProjectContext): String {
        val count = context.accessWideners.size
        return when (count) {
            0 -> "Access Widener: none"
            1 -> "Access Widener: 1 file"
            else -> "Access Widener: $count files"
        }
    }

    private fun formatAccessTransformerLine(context: ProjectContext): String {
        val count = context.accessTransformers.size
        return when (count) {
            0 -> "Access Transformer: none"
            1 -> "Access Transformer: 1 file"
            else -> "Access Transformer: $count files"
        }
    }

    private fun formatIndexState(state: ProjectIndexState): String = when (state) {
        ProjectIndexState.NOT_READY -> "not ready"
        ProjectIndexState.BUILDING -> "building"
        ProjectIndexState.READY -> "ready"
        ProjectIndexState.STALE -> "stale"
        ProjectIndexState.FAILED -> "failed"
    }
}
