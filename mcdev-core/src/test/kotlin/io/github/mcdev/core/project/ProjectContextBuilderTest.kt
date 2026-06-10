package io.github.mcdev.core.project

import io.github.mcdev.core.mapping.ProjectMappingContext
import io.github.mcdev.core.model.MappingNamespace
import kotlin.io.path.createDirectories
import kotlin.io.path.writeText
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Path

class ProjectContextBuilderTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun buildsEmptyContextWithExplicitUnknownPlatform() {
        val context = ProjectContextBuilder.empty("test-project", tempDir)

        assertEquals("test-project", context.projectId)
        assertEquals(tempDir, context.root)
        assertEquals(ModPlatform.UNKNOWN, context.platform)
        assertEquals(ProjectIndexState.NOT_READY, context.indexState)
        assertTrue(context.mixinConfigs.isEmpty())
        assertTrue(context.accessWideners.isEmpty())
        assertTrue(context.accessTransformers.isEmpty())
        assertTrue(context.mappings.availableNamespaces.isEmpty())
    }

    @Test
    fun buildsFabricContextFromGradleAndDiscoveredFiles() {
        tempDir.resolve("build.gradle.kts").writeText("plugins { id(\"fabric-loom\") }")

        val mappingsDir = tempDir.resolve("mappings").createDirectories()
        mappingsDir.resolve("minecraft.tiny").writeText(
            """
            tiny	2	0	named	intermediary
            c	net/minecraft/client/MinecraftClient	net/minecraft/class_310
            """.trimIndent(),
        )

        val resources = tempDir.resolve("src/main/resources").createDirectories()
        resources.resolve("mixins.json").writeText("""{"package":"com.example","mixins":["M"]}""")
        resources.resolve("mod.accesswidener").writeText(
            """
            accessWidener v2 named
            accessible class net/minecraft/client/MinecraftClient
            """.trimIndent(),
        )

        val minecraftJar = tempDir.resolve("libs/minecraft.jar").parent!!.createDirectories()
        minecraftJar.resolve("minecraft.jar").writeText("jar")

        val classpath = ClasspathSnapshot(
            dependencyJars = listOf(tempDir.resolve("libs/dep.jar")),
            minecraftJars = listOf(tempDir.resolve("libs/minecraft.jar")),
        )

        val context = ProjectContextBuilder.build(
            ProjectContextInput(
                projectId = "fabric-mod",
                root = tempDir,
                classpath = classpath,
                sourceSets = listOf(
                    SourceSetContext(
                        name = "main",
                        sourceDirectories = listOf(tempDir.resolve("src/main/java")),
                        resourceDirectories = listOf(resources),
                        outputDirectory = tempDir.resolve("build/classes"),
                    ),
                ),
                indexState = ProjectIndexState.READY,
            ),
        )

        assertEquals(ModPlatform.FABRIC, context.platform)
        assertEquals(MappingNamespace.NAMED, context.mappings.sourceNamespace)
        assertEquals(1, context.mixinConfigs.size)
        assertEquals(1, context.accessWideners.size)
        assertEquals(1, context.minecraftJars.size)
        assertEquals(ProjectIndexState.READY, context.indexState)
        assertEquals(1, context.sourceSets.size)
    }

    @Test
    fun usesExplicitPlatformOverGradleDetection() {
        tempDir.resolve("build.gradle.kts").writeText("id(\"fabric-loom\")")
        val context = ProjectContextBuilder.build(
            ProjectContextInput(
                projectId = "forced-forge",
                root = tempDir,
                platform = ModPlatform.FORGE,
            ),
        )
        assertEquals(ModPlatform.FORGE, context.platform)
    }

    @Test
    fun usesProvidedMappingsWithoutRediscovery() {
        val explicitMappings = ProjectMappingContext(
            sourceNamespace = MappingNamespace.MCP,
            runtimeNamespace = MappingNamespace.SRG,
            awNamespace = null,
            atNamespace = MappingNamespace.SRG,
            availableNamespaces = setOf(MappingNamespace.MCP, MappingNamespace.SRG),
            resolver = EmptyMappingResolver,
        )

        val context = ProjectContextBuilder.build(
            ProjectContextInput(
                projectId = "custom",
                root = tempDir,
                platform = ModPlatform.FORGE,
                mappings = explicitMappings,
            ),
        )

        assertEquals(MappingNamespace.MCP, context.mappings.sourceNamespace)
        assertEquals(MappingNamespace.SRG, context.mappings.runtimeNamespace)
    }

    @Test
    fun detectsPlatformFromGradleContentsWhenRootFilesMissing() {
        val context = ProjectContextBuilder.build(
            ProjectContextInput(
                projectId = "neoforge-mod",
                root = tempDir,
                gradleContents = listOf("plugins { id 'org.neoforged.moddev' }"),
            ),
        )
        assertEquals(ModPlatform.NEOFORGE, context.platform)
    }

    @Test
    fun preservesClasspathSnapshotInBuiltContext() {
        val classpath = ClasspathSnapshot(
            projectOutputs = listOf(tempDir.resolve("out")),
            dependencyJars = listOf(tempDir.resolve("lib/a.jar"), tempDir.resolve("lib/b.jar")),
            entryTimestamps = mapOf(tempDir.resolve("lib/a.jar") to 100L),
        )

        val context = ProjectContextBuilder.build(
            ProjectContextInput(
                projectId = "classpath-test",
                root = tempDir,
                classpath = classpath,
            ),
        )

        assertEquals(2, context.classpath.dependencyJars.size)
        assertEquals(100L, context.classpath.entryTimestamps[tempDir.resolve("lib/a.jar")])
        assertEquals(3, context.classpath.entryCount)
    }
}
