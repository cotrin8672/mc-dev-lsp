package io.github.mcdev.core.project

import kotlin.io.path.createDirectories
import kotlin.io.path.writeText
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Files
import java.nio.file.Path

class ProjectTreeWalkerTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun returnsEmptyForMissingRoot() {
        assertEquals(
            emptyList(),
            ProjectTreeWalker.walkRegularFiles(tempDir.resolve("missing-root")),
        )
    }

    @Test
    fun skipsExcludedSubtreesWithoutVisitingThem() {
        tempDir.resolve("src").createDirectories().resolve("keep.txt").writeText("keep")
        val excludedRoot = tempDir.resolve("build/deep").createDirectories()
        excludedRoot.resolve("sentinel.txt").writeText("sentinel")
        Files.setAttribute(excludedRoot.resolve("sentinel.txt"), "dos:readonly", true)

        val visited = ProjectTreeWalker.walkRegularFiles(tempDir)

        assertEquals(listOf(tempDir.resolve("src/keep.txt")), visited)
        assertTrue(visited.none { it.toString().contains("build") })
    }

    @Test
    fun skipsNestedExcludedDirectorySegments() {
        tempDir.resolve("included.txt").writeText("included")
        tempDir.resolve(".gradle/build/nested").createDirectories().resolve("excluded.txt").writeText("excluded")
        tempDir.resolve("node_modules/pkg").createDirectories().resolve("index.js").writeText("module")
        tempDir.resolve("BUILD").createDirectories().resolve("upper.txt").writeText("upper")

        val visited = ProjectTreeWalker.walkRegularFiles(tempDir)

        assertEquals(listOf(tempDir.resolve("included.txt")), visited)
    }

    @Test
    fun includesSimilarlyNamedDirectories() {
        tempDir.resolve("my-build").createDirectories().resolve("keep.txt").writeText("keep")
        tempDir.resolve(".gradle-backup").createDirectories().resolve("keep.txt").writeText("backup")
        tempDir.resolve("node_modules-extra").createDirectories().resolve("keep.txt").writeText("extra")

        val visited = ProjectTreeWalker.walkRegularFiles(tempDir)

        assertEquals(
            listOf(
                tempDir.resolve(".gradle-backup/keep.txt"),
                tempDir.resolve("my-build/keep.txt"),
                tempDir.resolve("node_modules-extra/keep.txt"),
            ),
            visited,
        )
    }

    @Test
    fun returnsDeterministicSortedOrder() {
        tempDir.resolve("z-dir").createDirectories().resolve("z.txt").writeText("z")
        tempDir.resolve("a-dir").createDirectories().resolve("a.txt").writeText("a")
        tempDir.resolve("m.txt").writeText("m")

        val first = ProjectTreeWalker.walkRegularFiles(tempDir)
        val second = ProjectTreeWalker.walkRegularFiles(tempDir)

        assertEquals(
            listOf(
                tempDir.resolve("a-dir/a.txt"),
                tempDir.resolve("m.txt"),
                tempDir.resolve("z-dir/z.txt"),
            ),
            first,
        )
        assertEquals(first, second)
    }

    @Test
    fun mappingWalkReturnsEmptyForMissingRoot() {
        assertEquals(
            emptyList(),
            ProjectTreeWalker.walkMappingDiscoveryRegularFiles(tempDir.resolve("missing-root")),
        )
    }

    @Test
    fun mappingWalkPrunesIrrelevantBuildAndGradleSubtrees() {
        tempDir.resolve("src").createDirectories().resolve("keep.txt").writeText("keep")
        tempDir.resolve("build/classes/deep").createDirectories().resolve("sentinel.txt").writeText("sentinel")
        tempDir.resolve(".gradle/caches/other/deep").createDirectories().resolve("sentinel.txt").writeText("sentinel")
        tempDir.resolve("node_modules/pkg").createDirectories().resolve("sentinel.txt").writeText("sentinel")

        val visited = ProjectTreeWalker.walkMappingDiscoveryRegularFiles(tempDir)

        assertEquals(listOf(tempDir.resolve("src/keep.txt")), visited)
        assertTrue(visited.none { it.toString().contains("sentinel") })
    }

    @Test
    fun mappingWalkDescendsThroughNestedWhitelistPaths() {
        tempDir.resolve("src/build/loom-cache").createDirectories().resolve("nested.tiny").writeText("nested")
        tempDir
            .resolve("nested/.gradle/caches/fabric-loom/mappings")
            .createDirectories()
            .resolve("intermediary.tiny")
            .writeText("intermediary")
        tempDir.resolve("build/classes").createDirectories().resolve("ignored.tiny").writeText("ignored")

        val visited = ProjectTreeWalker.walkMappingDiscoveryRegularFiles(tempDir)
        val normalized = visited.map { it.toString().replace('\\', '/') }

        assertEquals(2, visited.size)
        assertTrue(normalized.any { it.contains("src/build/loom-cache/nested.tiny") })
        assertTrue(normalized.any { it.contains("nested/.gradle/caches/fabric-loom/mappings/intermediary.tiny") })
        assertTrue(normalized.none { it.contains("build/classes") })
    }

    @Test
    fun mappingWalkCoversAllMappingWhitelistPrefixes() {
        tempDir.resolve(".gradle/loom-cache/remapped").createDirectories().resolve("loom.tiny").writeText("loom")
        tempDir
            .resolve(".gradle/caches/fabric-loom/mappings")
            .createDirectories()
            .resolve("fabric.tiny")
            .writeText("fabric")
        tempDir
            .resolve(".gradle/caches/modules-2/files-2.1/net.fabricmc/yarn/1.21.1")
            .createDirectories()
            .resolve("yarn.tiny")
            .writeText("yarn")
        tempDir
            .resolve(".gradle/caches/modules-2/files-2.1/net.minecraft/mappings/1.21.1")
            .createDirectories()
            .resolve("client.tiny")
            .writeText("client")
        tempDir
            .resolve(".gradle/caches/modules-2/files-2.1/net.minecraftforge/forge/1.21.1")
            .createDirectories()
            .resolve("forge.srg")
            .writeText("forge")
        tempDir
            .resolve(".gradle/caches/modules-2/files-2.1/net.neoforged/neoforge/1.21.1")
            .createDirectories()
            .resolve("neoforge.srg")
            .writeText("neoforge")
        tempDir.resolve("build/loom-cache/remapped").createDirectories().resolve("build-loom.tiny").writeText("build-loom")
        tempDir.resolve("build/createSrgToMcp/output").createDirectories().resolve("joined.srg").writeText("joined")
        tempDir.resolve("build/tmp/compileJava").createDirectories().resolve("generated.srg").writeText("generated")
        tempDir.resolve(".gradle/caches/unrelated").createDirectories().resolve("ignored.tiny").writeText("ignored")
        tempDir.resolve("build/classes").createDirectories().resolve("ignored.tiny").writeText("ignored")

        val visited = ProjectTreeWalker.walkMappingDiscoveryRegularFiles(tempDir)
        val normalized = visited.map { it.toString().replace('\\', '/') }.sorted()

        assertEquals(9, visited.size)
        assertTrue(normalized.any { it.contains(".gradle/loom-cache/") })
        assertTrue(normalized.any { it.contains(".gradle/caches/fabric-loom/") })
        assertTrue(normalized.any { it.contains("net.fabricmc/yarn/") })
        assertTrue(normalized.any { it.contains("net.minecraft/") })
        assertTrue(normalized.any { it.contains("net.minecraftforge/") })
        assertTrue(normalized.any { it.contains("net.neoforged/") })
        assertTrue(normalized.any { it.contains("build/loom-cache/") })
        assertTrue(normalized.any { it.contains("build/createSrgToMcp/") })
        assertTrue(normalized.any { it.contains("build/tmp/") })
        assertTrue(normalized.none { it.contains("caches/unrelated") })
        assertTrue(normalized.none { it.contains("build/classes") })
    }

    @Test
    fun mappingWalkReturnsDeterministicSortedOrder() {
        tempDir.resolve("z-dir").createDirectories().resolve("z.txt").writeText("z")
        tempDir.resolve("a-dir").createDirectories().resolve("a.txt").writeText("a")
        tempDir.resolve("m.txt").writeText("m")

        val first = ProjectTreeWalker.walkMappingDiscoveryRegularFiles(tempDir)
        val second = ProjectTreeWalker.walkMappingDiscoveryRegularFiles(tempDir)

        assertEquals(
            listOf(
                tempDir.resolve("a-dir/a.txt"),
                tempDir.resolve("m.txt"),
                tempDir.resolve("z-dir/z.txt"),
            ),
            first,
        )
        assertEquals(first, second)
    }

    @Test
    fun shouldPruneMappingDiscoveryDirectoryMatchesWhitelistSemantics() {
        val fs = tempDir.fileSystem
        fun shouldPrune(vararg segments: String): Boolean =
            ProjectPathFilters.shouldPruneMappingDiscoveryDirectory(
                fs.getPath(segments.first(), *segments.drop(1).toTypedArray()),
            )

        // Excluded roots themselves must not prune: empty suffix is detected by index, not Path representation.
        assertEquals(false, shouldPrune("build"))
        assertEquals(false, shouldPrune(".gradle"))

        // Partial whitelist prefixes stay traversable.
        assertEquals(false, shouldPrune("build", "loom-cache"))
        assertEquals(false, shouldPrune("build", "createSrgToMcp"))
        assertEquals(false, shouldPrune("build", "tmp"))
        assertEquals(false, shouldPrune(".gradle", "loom-cache"))
        assertEquals(false, shouldPrune(".gradle", "caches"))
        assertEquals(false, shouldPrune(".gradle", "caches", "fabric-loom"))
        assertEquals(false, shouldPrune(".gradle", "caches", "modules-2", "files-2.1", "net.fabricmc"))

        // Rejected siblings under excluded roots prune immediately.
        assertEquals(true, shouldPrune("build", "classes"))
        assertEquals(true, shouldPrune("build", "generated"))
        assertEquals(true, shouldPrune(".gradle", "caches", "other"))
        assertEquals(true, shouldPrune(".gradle", "wrapper"))
        assertEquals(true, shouldPrune("node_modules", "pkg"))
        assertEquals(true, shouldPrune(".gradle", "build", "nested"))

        // Full and deeper whitelist paths remain open, including nested excluded segments.
        assertEquals(false, shouldPrune("build", "loom-cache", "remapped"))
        assertEquals(false, shouldPrune("build", "createSrgToMcp", "output"))
        assertEquals(false, shouldPrune("build", "tmp", "compileJava"))
        assertEquals(false, shouldPrune(".gradle", "caches", "fabric-loom", "mappings"))
        assertEquals(
            false,
            shouldPrune(".gradle", "caches", "modules-2", "files-2.1", "net.fabricmc", "yarn", "1.21.1"),
        )
        assertEquals(
            false,
            shouldPrune("src", "build", "loom-cache", "remapped"),
        )
    }
}
