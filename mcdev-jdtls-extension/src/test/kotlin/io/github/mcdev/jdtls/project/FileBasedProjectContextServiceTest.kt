package io.github.mcdev.jdtls.project

import io.github.mcdev.core.project.ModPlatform
import io.github.mcdev.core.project.ProjectIndexState
import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.jdtls.support.JdtlsFixtureSupport
import kotlin.io.path.createDirectories
import java.util.concurrent.Callable
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
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
    fun concurrentLoadCachedSessionReturnsSingleSessionIdentityAndVersion() {
        val buildCount = AtomicInteger(0)
        val baseService = FileBasedProjectContextService()
        val concurrentService = FileBasedProjectContextService { root ->
            buildCount.incrementAndGet()
            baseService.buildProjectContextInternal(root)
        }
        val uri = JdtlsFixtureSupport.workspaceUri(tempDir)
        val taskCount = 32
        val startGate = CountDownLatch(1)
        val executor = Executors.newFixedThreadPool(taskCount)
        try {
            val futures = (0 until taskCount).map {
                executor.submit(
                    Callable {
                        startGate.await(10, TimeUnit.SECONDS)
                        concurrentService.loadCachedSession(uri)
                    },
                )
            }
            startGate.countDown()
            val results = futures.map { it.get(30, TimeUnit.SECONDS) }
            val canonicalSession = results.first().session
            val canonicalVersion = results.first().version
            results.forEach { result ->
                assertTrue(result.session === canonicalSession)
                assertEquals(canonicalVersion, result.version)
            }
            assertEquals(1, results.count { !it.cacheHit })
            assertEquals(1, buildCount.get())
        } finally {
            executor.shutdownNow()
            executor.awaitTermination(5, TimeUnit.SECONDS)
        }
    }

    @Test
    fun warmCacheHitForOneWorkspaceDoesNotWaitOnColdBuildForAnother() {
        val workspaceA = tempDir.resolve("workspace-a")
        val workspaceB = tempDir.resolve("workspace-b")
        workspaceA.createDirectories()
        workspaceB.createDirectories()
        val uriA = JdtlsFixtureSupport.workspaceUri(workspaceA)
        val uriB = JdtlsFixtureSupport.workspaceUri(workspaceB)

        val bBuildEntered = CountDownLatch(1)
        val bBuildProceed = CountDownLatch(1)
        val aWarmHitCompleted = CountDownLatch(1)
        val bLoadCompleted = CountDownLatch(1)

        val baseService = FileBasedProjectContextService()
        val service = FileBasedProjectContextService { root ->
            if (root == workspaceB) {
                bBuildEntered.countDown()
                bBuildProceed.await(10, TimeUnit.SECONDS)
            }
            baseService.buildProjectContextInternal(root)
        }

        val prewarm = service.loadCachedSession(uriA)
        assertEquals(false, prewarm.cacheHit)

        val executor = Executors.newFixedThreadPool(2)
        try {
            val bFuture = executor.submit(
                Callable {
                    service.loadCachedSession(uriB)
                    bLoadCompleted.countDown()
                },
            )
            assertTrue(bBuildEntered.await(10, TimeUnit.SECONDS))

            executor.submit(
                Callable {
                    val warmHit = service.loadCachedSession(uriA)
                    assertTrue(warmHit.cacheHit)
                    aWarmHitCompleted.countDown()
                },
            ).get(10, TimeUnit.SECONDS)

            assertTrue(aWarmHitCompleted.await(10, TimeUnit.SECONDS))
            assertEquals(1L, bLoadCompleted.count)

            bBuildProceed.countDown()
            bFuture.get(10, TimeUnit.SECONDS)
            assertEquals(0L, bLoadCompleted.count)
        } finally {
            executor.shutdownNow()
            executor.awaitTermination(5, TimeUnit.SECONDS)
        }
    }

    @Test
    fun concurrentColdLoadAndReindexAlwaysReindexesAndBumpsVersion() {
        val buildCount = AtomicInteger(0)
        val coldBuildEntered = CountDownLatch(1)
        val coldBuildProceed = CountDownLatch(1)
        val reindexBuildEntered = CountDownLatch(1)
        val reindexBuildProceed = CountDownLatch(1)

        val baseService = FileBasedProjectContextService()
        val service = FileBasedProjectContextService { root ->
            when (buildCount.incrementAndGet()) {
                1 -> {
                    coldBuildEntered.countDown()
                    coldBuildProceed.await(10, TimeUnit.SECONDS)
                }
                else -> {
                    reindexBuildEntered.countDown()
                    reindexBuildProceed.await(10, TimeUnit.SECONDS)
                }
            }
            baseService.buildProjectContextInternal(root)
        }

        val uri = JdtlsFixtureSupport.workspaceUri(tempDir)
        val executor = Executors.newFixedThreadPool(2)
        try {
            val coldFuture = executor.submit(
                Callable {
                    service.loadCachedSession(uri)
                },
            )
            assertTrue(coldBuildEntered.await(10, TimeUnit.SECONDS))

            val reindexFuture = executor.submit(
                Callable {
                    service.reindex(uri)
                },
            )
            assertEquals(1, buildCount.get())

            coldBuildProceed.countDown()
            assertTrue(reindexBuildEntered.await(10, TimeUnit.SECONDS))
            assertEquals(2, buildCount.get())

            reindexBuildProceed.countDown()
            val coldResult = coldFuture.get(10, TimeUnit.SECONDS)
            val reindexed = reindexFuture.get(10, TimeUnit.SECONDS)
            val afterReindex = service.loadCachedSession(uri)

            assertEquals(false, coldResult.cacheHit)
            assertTrue(afterReindex.version > coldResult.version)
            assertTrue(afterReindex.cacheHit)
            assertTrue(afterReindex.session === reindexed)
        } finally {
            executor.shutdownNow()
            executor.awaitTermination(5, TimeUnit.SECONDS)
        }
    }

    @Test
    fun failedColdLoadPropagatesBuildFailureWithoutSecondarySessionMissing() {
        val buildCount = AtomicInteger(0)
        val baseService = FileBasedProjectContextService()
        val concurrentService = FileBasedProjectContextService { root ->
            buildCount.incrementAndGet()
            if (buildCount.get() == 1) {
                throw IllegalStateException("simulated build failure")
            }
            baseService.buildProjectContextInternal(root)
        }

        val uri = JdtlsFixtureSupport.workspaceUri(tempDir)
        val taskCount = 8
        val startGate = CountDownLatch(1)
        val executor = Executors.newFixedThreadPool(taskCount)
        try {
            val futures = (0 until taskCount).map {
                executor.submit(
                    Callable {
                        startGate.await(10, TimeUnit.SECONDS)
                        concurrentService.loadCachedSession(uri)
                    },
                )
            }
            startGate.countDown()

            val failures = futures.mapNotNull { future ->
                try {
                    future.get(30, TimeUnit.SECONDS)
                    null
                } catch (ex: Exception) {
                    ex
                }
            }
            val successes = futures.size - failures.size

            assertEquals(1, failures.size)
            assertTrue(
                failures.single().cause is IllegalStateException ||
                    failures.single() is IllegalStateException,
            )
            assertTrue(
                failures.none { it.message?.contains("Session missing") == true },
            )
            assertEquals(taskCount - 1, successes)
            assertEquals(2, buildCount.get())
            val cached = concurrentService.loadCachedSession(uri)
            assertTrue(cached.cacheHit)
        } finally {
            executor.shutdownNow()
            executor.awaitTermination(5, TimeUnit.SECONDS)
        }
    }

    @Test
    fun cachedSessionReportsHitAndReindexBumpsVersion() {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_BASIC, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        val uri = JdtlsFixtureSupport.workspaceUri(tempDir)
        val first = service.loadCachedSession(uri)
        val second = service.loadCachedSession(uri)
        service.reindex(uri)
        val afterReindex = service.loadCachedSession(uri)

        assertEquals(false, first.cacheHit)
        assertEquals(true, second.cacheHit)
        assertEquals(first.version, second.version)
        assertTrue(afterReindex.version > second.version)
    }

    @Test
    fun emptyWorkspaceHasNoClasspathEntries() {
        val context = service.buildProjectContext(tempDir)
        assertEquals(0, context.classpath.entryCount)
        assertEquals(ProjectIndexState.NOT_READY, context.indexState)
    }

    @Test
    fun discoverClasspathIncludesMinecraftJarsFromJarRoots() {
        val libsDir = tempDir.resolve("libs")
        Files.createDirectories(libsDir)
        val minecraftJar = libsDir.resolve("minecraft-client.jar")
        Files.writeString(minecraftJar, "fake")
        val context = service.buildProjectContext(tempDir)
        assertEquals(
            listOf(minecraftJar.toString()),
            context.classpath.minecraftJars.map { it.toString() },
        )
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
