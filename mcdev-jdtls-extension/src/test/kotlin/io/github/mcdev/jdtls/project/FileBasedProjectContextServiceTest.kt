package io.github.mcdev.jdtls.project

import io.github.mcdev.core.project.ModPlatform
import io.github.mcdev.core.project.ProjectIndexState
import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.jdtls.support.JdtlsFixtureSupport
import kotlin.io.path.createDirectories
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

    @Test
    fun discoversArchitecturyStyleSourceSetsInSortedOrder() {
        createArchitecturyStyleLayout(tempDir)
        val context = service.buildProjectContext(tempDir)
        assertEquals(
            listOf("common", "fabric", "forge", "neoforge", "testmod"),
            context.sourceSets.map { it.name },
        )
        context.sourceSets.forEach { sourceSet ->
            assertTrue(
                sourceSet.sourceDirectories.any {
                    it.endsWith(
                        "src${java.io.File.separator}${sourceSet.name}${java.io.File.separator}java",
                    )
                },
            )
            assertTrue(
                sourceSet.resourceDirectories.any {
                    it.endsWith(
                        "src${java.io.File.separator}${sourceSet.name}${java.io.File.separator}resources",
                    )
                },
            )
            assertEquals(
                tempDir.resolve("build/classes/java/${sourceSet.name}"),
                sourceSet.outputDirectory,
            )
        }
    }

    @Test
    fun preservesMappedSourcesOnMainSourceSetOnly() {
        createArchitecturyStyleLayout(tempDir)
        tempDir.resolve("mapped-sources").createDirectories()
        val context = service.buildProjectContext(tempDir)
        val common = context.sourceSets.single { it.name == "common" }
        assertTrue(common.sourceDirectories.none { it.endsWith("mapped-sources") })
        Files.createDirectories(tempDir.resolve("src/main/java"))
        val withMain = service.buildProjectContext(tempDir)
        val mainSourceSet = withMain.sourceSets.single { it.name == "main" }
        assertTrue(mainSourceSet.sourceDirectories.any { it.endsWith("mapped-sources") })
        assertTrue(withMain.sourceSets.none { it.name != "main" && it.sourceDirectories.any { dir -> dir.endsWith("mapped-sources") } })
    }

    @Test
    fun discoversGeneratedOutputsAndSourceSetProjectOutputs() {
        createArchitecturyStyleLayout(tempDir)
        val commonClasses = tempDir.resolve("build/classes/java/common").createDirectories()
        val fabricClasses = tempDir.resolve("build/classes/java/fabric").createDirectories()
        val generatedMainSources = tempDir.resolve("build/generated/sources/annotationProcessor/java/main")
            .createDirectories()
        val generatedCommonResources = tempDir.resolve("build/generated/resources/common")
            .createDirectories()
        Files.writeString(generatedMainSources.resolve("Example.java"), "class Example {}")
        Files.writeString(generatedCommonResources.resolve("pack.mcmeta"), "{}")

        val context = service.buildProjectContext(tempDir)
        assertEquals(
            listOf(commonClasses, fabricClasses).map { it.toString() }.sorted(),
            context.classpath.projectOutputs.map { it.toString() }.sorted(),
        )
        assertEquals(
            listOf(generatedMainSources, generatedCommonResources).map { it.toString() }.sorted(),
            context.classpath.generatedOutputs.map { it.toString() }.sorted(),
        )
        assertTrue(context.classpath.entryCount >= 4)
    }

    private fun createArchitecturyStyleLayout(root: Path) {
        listOf("common", "fabric", "forge", "neoforge", "testmod").forEach { name ->
            root.resolve("src/$name/java/com/example/$name").createDirectories()
            root.resolve("src/$name/resources").createDirectories()
            Files.writeString(
                root.resolve("src/$name/java/com/example/$name/${name.replaceFirstChar { it.uppercase() }}Mod.java"),
                "package com.example.$name; class ${name.replaceFirstChar { it.uppercase() }}Mod {}",
            )
        }
    }
}
