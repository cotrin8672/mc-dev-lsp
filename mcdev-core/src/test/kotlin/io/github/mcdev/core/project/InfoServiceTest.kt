package io.github.mcdev.core.project

import io.github.mcdev.core.mapping.ProjectMappingContext
import io.github.mcdev.core.model.MappingNamespace
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Path

class InfoServiceTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun formatsCompleteFabricProjectInfo() {
        val context = ProjectContext(
            projectId = "fabric",
            root = tempDir,
            platform = ModPlatform.FABRIC,
            classpath = ClasspathSnapshot(
                dependencyJars = (1..142).map { tempDir.resolve("lib/$it.jar") },
            ),
            mappings = ProjectMappingContext(
                sourceNamespace = MappingNamespace.NAMED,
                runtimeNamespace = MappingNamespace.INTERMEDIARY,
                awNamespace = MappingNamespace.NAMED,
                atNamespace = null,
                availableNamespaces = setOf(MappingNamespace.NAMED, MappingNamespace.INTERMEDIARY),
                resolver = EmptyMappingResolver,
            ),
            mixinConfigs = listOf(
                MixinConfigRef(tempDir.resolve("mixins.json")),
                MixinConfigRef(tempDir.resolve("client.mixins.json")),
            ),
            accessWideners = listOf(AccessWidenerRef(tempDir.resolve("mod.accesswidener"))),
            accessTransformers = emptyList(),
            minecraftJars = listOf(tempDir.resolve("minecraft.jar")),
            sourceSets = emptyList(),
            indexState = ProjectIndexState.READY,
        )

        val lines = InfoService.formatLines(context, protocolVersion = 1, extensionVersion = "0.1.0")

        assertEquals("Project: fabric", lines[0])
        assertEquals("Root: $tempDir", lines[1])
        assertEquals("Platform: Fabric", lines[2])
        assertEquals("Mappings: named <-> intermediary loaded", lines[3])
        assertEquals("Mapping source: provided", lines[4])
        assertEquals("Source namespace: named", lines[5])
        assertEquals("Runtime namespace: intermediary", lines[6])
        assertEquals("Minecraft jar: found", lines[7])
        assertEquals("Mixin config: 2 files", lines[8])
        assertTrue(lines[9].startsWith("Mixin config search: "))
        assertEquals("Access Widener: 1 file", lines[10])
        assertEquals("Access Transformer: none", lines[11])
        assertEquals("Classpath entries: 142", lines[12])
        assertEquals("Classpath source: project outputs=0, dependencies=142, minecraft=0", lines[13])
        assertEquals("Source sets: 0", lines[14])
        assertEquals("Class index: ready", lines[15])
        assertEquals("Bytecode index: ready", lines[16])
        assertEquals("Mapping namespaces: named, intermediary", lines[17])
        assertEquals("Protocol: 1", lines[18])
        assertEquals("Extension: 0.1.0", lines[19])
    }

    @Test
    fun formatsPartialContextWithNoMappings() {
        val context = ProjectContextBuilder.empty("partial", tempDir)
        val lines = InfoService.formatLines(context)

        assertTrue(lines.any { it == "Mappings: none" })
        assertTrue(lines.any { it == "Mapping source: none (no supported mapping files discovered)" })
        assertTrue(lines.any { it == "Mixin config: none" })
        assertTrue(lines.any { it == "Mixin config search: none found under $tempDir" })
        assertTrue(lines.any { it == "Access Widener: none" })
        assertTrue(lines.any { it == "Access Transformer: none" })
        assertTrue(lines.any { it == "Classpath source: empty (no project outputs, dependency jars, or minecraft jars discovered)" })
        assertTrue(lines.any { it == "Mapping namespaces: none (searched project mapping files and known Gradle mapping caches)" })
        assertTrue(lines.any { it == "Minecraft jar: none" })
        assertTrue(lines.any { it == "Class index: not ready" })
    }

    @Test
    fun formatsUnknownPlatform() {
        val context = ProjectContextBuilder.empty("unknown-mod", tempDir)
        val lines = InfoService.formatLines(context)
        assertTrue(lines.any { it == "Platform: Unknown" })
    }

    @Test
    fun formatsPartialMappingsAsPartialStatus() {
        val context = ProjectContext(
            projectId = "single-ns",
            root = tempDir,
            platform = ModPlatform.UNKNOWN,
            classpath = ClasspathSnapshot.EMPTY,
            mappings = ProjectMappingContext(
                sourceNamespace = MappingNamespace.NAMED,
                runtimeNamespace = MappingNamespace.NAMED,
                awNamespace = MappingNamespace.NAMED,
                atNamespace = null,
                availableNamespaces = setOf(MappingNamespace.NAMED),
                resolver = EmptyMappingResolver,
            ),
            mixinConfigs = emptyList(),
            accessWideners = emptyList(),
            accessTransformers = emptyList(),
            minecraftJars = emptyList(),
            sourceSets = emptyList(),
            indexState = ProjectIndexState.NOT_READY,
        )

        val lines = InfoService.formatLines(context)
        assertTrue(lines.any { it == "Mappings: named <-> named partial" })
    }

    @Test
    fun formatJoinsLinesWithNewlines() {
        val context = ProjectContextBuilder.empty("join-test", tempDir)
        val text = InfoService.format(context)
        assertTrue(text.contains("\n"))
        assertEquals(InfoService.formatLines(context).joinToString("\n"), text)
    }

    @Test
    fun formatsIndexStates() {
        val states = mapOf(
            ProjectIndexState.BUILDING to "building",
            ProjectIndexState.STALE to "stale",
            ProjectIndexState.FAILED to "failed",
        )

        for ((state, label) in states) {
            val context = ProjectContextBuilder.empty("state", tempDir).copy(indexState = state)
            val lines = InfoService.formatLines(context)
            assertTrue(lines.any { it == "Class index: $label" })
        }
    }

    @Test
    fun usesClasspathMinecraftJarsWhenContextListEmpty() {
        val jar = tempDir.resolve("mc.jar")
        val context = ProjectContext(
            projectId = "jar-fallback",
            root = tempDir,
            platform = ModPlatform.FABRIC,
            classpath = ClasspathSnapshot(minecraftJars = listOf(jar)),
            mappings = ProjectMappingContext(
                sourceNamespace = MappingNamespace.NAMED,
                runtimeNamespace = MappingNamespace.INTERMEDIARY,
                awNamespace = null,
                atNamespace = null,
                availableNamespaces = emptySet(),
                resolver = EmptyMappingResolver,
            ),
            mixinConfigs = emptyList(),
            accessWideners = emptyList(),
            accessTransformers = emptyList(),
            minecraftJars = emptyList(),
            sourceSets = emptyList(),
            indexState = ProjectIndexState.NOT_READY,
        )

        val lines = InfoService.formatLines(context)
        assertTrue(lines.any { it == "Minecraft jar: found" })
    }

    @Test
    fun formatsDiscoveredMappingSourceAndSourceSetCount() {
        val mappingFile = tempDir.resolve("mappings.tiny")
        val context = ProjectContextBuilder.empty("mapped", tempDir).copy(
            mappings = ProjectMappingContext(
                sourceNamespace = MappingNamespace.NAMED,
                runtimeNamespace = MappingNamespace.INTERMEDIARY,
                awNamespace = MappingNamespace.NAMED,
                atNamespace = null,
                availableNamespaces = setOf(MappingNamespace.NAMED, MappingNamespace.INTERMEDIARY),
                resolver = EmptyMappingResolver,
            ),
            mappingFiles = listOf(mappingFile),
            sourceSets = listOf(
                SourceSetContext(
                    name = "main",
                    sourceDirectories = listOf(tempDir.resolve("src/main/java")),
                    resourceDirectories = emptyList(),
                    outputDirectory = null,
                ),
            ),
        )

        val lines = InfoService.formatLines(context)
        assertTrue(lines.any { it == "Mapping source: 1 file(s)" })
        assertTrue(lines.any { it == "Source sets: 1" })
    }
}
