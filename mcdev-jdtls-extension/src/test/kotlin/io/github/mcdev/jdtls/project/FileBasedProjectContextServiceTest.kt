package io.github.mcdev.jdtls.project

import io.github.mcdev.core.project.ModPlatform
import io.github.mcdev.core.project.ProjectIndexState
import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.jdtls.support.JdtlsFixtureSupport
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Files
import java.nio.file.Path

class FileBasedProjectContextServiceTest {
    @TempDir
    lateinit var tempDir: Path

    private val service = FileBasedProjectContextService()

    @Test
    fun discoversFabricPlatformFromFixtureGradle() {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_BASIC, tempDir)
        val context = service.buildProjectContext(tempDir)
        assertEquals(ModPlatform.FABRIC, context.platform)
    }

    @Test
    fun discoversMixinConfigFromFixture() {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_BASIC, tempDir)
        val context = service.buildProjectContext(tempDir)
        assertEquals(1, context.mixinConfigs.size)
    }

    @Test
    fun discoversMappingsFromFixture() {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_BASIC, tempDir)
        val context = service.buildProjectContext(tempDir)
        assertTrue(context.mappings.availableNamespaces.isNotEmpty())
    }

    @Test
    fun discoversClasspathDirectory() {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_BASIC, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        val context = service.buildProjectContext(tempDir)
        assertTrue(context.classpath.entryCount >= 1)
    }

    @Test
    fun sessionIndexesClasspathClasses() {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_BASIC, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        val session = service.loadSession(JdtlsFixtureSupport.workspaceUri(tempDir))
        assertTrue(session.classBytesProvider.classCount() >= 1)
        assertEquals(ProjectIndexState.READY, session.context.indexState)
    }

    @Test
    fun reindexRefreshesSession() {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_BASIC, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        val uri = JdtlsFixtureSupport.workspaceUri(tempDir)
        service.loadSession(uri)
        val reindexed = service.reindex(uri)
        assertEquals(ProjectIndexState.READY, reindexed.context.indexState)
    }

    @Test
    fun emptyWorkspaceHasNoClasspathEntries() {
        val context = service.buildProjectContext(tempDir)
        assertEquals(0, context.classpath.entryCount)
        assertEquals(ProjectIndexState.NOT_READY, context.indexState)
    }

    @Test
    fun discoversLoomRemappedJarsInEnhancedClasspath() {
        val loomDir = tempDir.resolve(".gradle/loom-cache/remapped_working")
        Files.createDirectories(loomDir)
        Files.writeString(loomDir.resolve("minecraft-client-mapped.jar"), "fake")
        val context = service.buildProjectContext(tempDir)
        assertTrue(context.classpath.minecraftJars.isNotEmpty())
        assertTrue(
            context.classpath.minecraftJars.any {
                it.fileName.toString().contains("minecraft")
            },
        )
    }

    @Test
    fun discoversMappedSourcesForLoomFixture() {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_LOOM_E2E, tempDir)
        val context = service.buildProjectContext(tempDir)
        val main = context.sourceSets.single { it.name == "main" }
        assertTrue(
            main.sourceDirectories.any {
                it.endsWith("mapped-sources")
            },
        )
        assertTrue(
            main.sourceDirectories.none {
                it.endsWith(
                    "mapped-sources${java.io.File.separator}com${java.io.File.separator}example" +
                        "${java.io.File.separator}target",
                )
            },
        )
    }

    @Test
    fun discoversMainAndClientSourceSetsFromFixture() {
        JdtlsFixtureSupport.copyFixture(FixturePaths.MULTI_SOURCE_SET, tempDir)
        val context = service.buildProjectContext(tempDir)
        assertEquals(2, context.sourceSets.size)
        assertEquals(setOf("main", "client"), context.sourceSets.map { it.name }.toSet())
        assertTrue(
            context.sourceSets.single { it.name == "main" }.sourceDirectories.any {
                it.endsWith("src${java.io.File.separator}main${java.io.File.separator}java")
            },
        )
        assertTrue(
            context.sourceSets.single { it.name == "client" }.sourceDirectories.any {
                it.endsWith("src${java.io.File.separator}client${java.io.File.separator}java")
            },
        )
    }
}
