package io.github.mcdev.core.project

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Path

class ProjectModelTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun classpathSnapshotAggregatesAllEntryTypes() {
        val outputs = listOf(tempDir.resolve("out"))
        val deps = listOf(tempDir.resolve("lib/a.jar"))
        val mc = listOf(tempDir.resolve("minecraft.jar"))
        val generated = listOf(tempDir.resolve("gen"))

        val snapshot = ClasspathSnapshot(
            projectOutputs = outputs,
            dependencyJars = deps,
            minecraftJars = mc,
            generatedOutputs = generated,
        )

        assertEquals(4, snapshot.entryCount)
        assertEquals(outputs + deps + mc + generated, snapshot.allEntries)
    }

    @Test
    fun classpathSnapshotEmptyDefaults() {
        val snapshot = ClasspathSnapshot.EMPTY
        assertTrue(snapshot.projectOutputs.isEmpty())
        assertTrue(snapshot.dependencyJars.isEmpty())
        assertEquals(0, snapshot.entryCount)
    }

    @Test
    fun sourceSetContextStoresDirectories() {
        val source = tempDir.resolve("src/main/java")
        val resources = tempDir.resolve("src/main/resources")
        val output = tempDir.resolve("build/classes")

        val sourceSet = SourceSetContext(
            name = "main",
            sourceDirectories = listOf(source),
            resourceDirectories = listOf(resources),
            outputDirectory = output,
        )

        assertEquals("main", sourceSet.name)
        assertEquals(listOf(source), sourceSet.sourceDirectories)
        assertEquals(output, sourceSet.outputDirectory)
    }

    @Test
    fun mixinConfigRefDefaultsToEmptyLists() {
        val ref = MixinConfigRef(path = tempDir.resolve("mixins.json"))
        assertTrue(ref.mixins.isEmpty())
        assertTrue(ref.client.isEmpty())
        assertTrue(ref.server.isEmpty())
        assertTrue(ref.common.isEmpty())
    }

    @Test
    fun projectIndexStateHasAllLifecycleValues() {
        val expected = setOf(
            ProjectIndexState.NOT_READY,
            ProjectIndexState.BUILDING,
            ProjectIndexState.READY,
            ProjectIndexState.STALE,
            ProjectIndexState.FAILED,
        )
        assertEquals(expected, ProjectIndexState.entries.toSet())
    }

    @Test
    fun modPlatformHasSupportedValues() {
        val expected = setOf(
            ModPlatform.FABRIC,
            ModPlatform.FORGE,
            ModPlatform.NEOFORGE,
            ModPlatform.UNKNOWN,
        )
        assertEquals(expected, ModPlatform.entries.toSet())
    }
}
