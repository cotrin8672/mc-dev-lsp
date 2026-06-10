package io.github.mcdev.jdtls.project

import io.github.mcdev.core.project.ModPlatform
import io.github.mcdev.core.project.ProjectIndexState
import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.jdtls.support.JdtlsFixtureSupport
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
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
}
