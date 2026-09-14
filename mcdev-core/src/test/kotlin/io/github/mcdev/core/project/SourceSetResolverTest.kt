package io.github.mcdev.core.project

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Path

class SourceSetResolverTest {
    @TempDir
    lateinit var tempDir: Path

    private fun mainAndClientSourceSets(root: Path): List<SourceSetContext> =
        listOf(
            SourceSetContext(
                name = "main",
                sourceDirectories = listOf(
                    root.resolve("src/main/java"),
                    root.resolve("mapped-sources"),
                ),
            ),
            SourceSetContext(
                name = "client",
                sourceDirectories = listOf(root.resolve("src/client/java")),
            ),
        )

    @Test
    fun resolvesMainAndClientSourceSets() {
        val sourceSets = mainAndClientSourceSets(tempDir)

        assertEquals(
            "main",
            SourceSetResolver.containingSourceSet(
                sourceSets,
                tempDir.resolve("src/main/java/com/example/Main.java"),
            )?.name,
        )
        assertEquals(
            "client",
            SourceSetResolver.containingSourceSet(
                sourceSets,
                tempDir.resolve("src/client/java/com/example/ClientEntry.java"),
            )?.name,
        )
    }

    @Test
    fun prefersNestedMostSpecificSourceRoot() {
        val sourceSets = listOf(
            SourceSetContext(
                name = "main",
                sourceDirectories = listOf(tempDir.resolve("src/main/java")),
            ),
            SourceSetContext(
                name = "overlay",
                sourceDirectories = listOf(tempDir.resolve("src/main/java/nested")),
            ),
        )

        assertEquals(
            "overlay",
            SourceSetResolver.containingSourceSet(
                sourceSets,
                tempDir.resolve("src/main/java/nested/com/example/Nested.java"),
            )?.name,
        )
    }

    @Test
    fun returnsNullWhenEqualSpecificityTie() {
        val sharedRoot = tempDir.resolve("src/shared")
        val sourceSets = listOf(
            SourceSetContext(name = "main", sourceDirectories = listOf(sharedRoot)),
            SourceSetContext(name = "client", sourceDirectories = listOf(sharedRoot)),
        )

        assertNull(
            SourceSetResolver.containingSourceSet(
                sourceSets,
                tempDir.resolve("src/shared/com/example/Shared.java"),
            ),
        )
    }

    @Test
    fun returnsNullForUnknownDocumentPath() {
        val sourceSets = mainAndClientSourceSets(tempDir)

        assertNull(
            SourceSetResolver.containingSourceSet(
                sourceSets,
                tempDir.resolve("build/generated/com/example/Generated.java"),
            ),
        )
    }

    @Test
    fun resolvesMappedSourcesToMainSourceSet() {
        val sourceSets = mainAndClientSourceSets(tempDir)

        assertEquals(
            "main",
            SourceSetResolver.containingSourceSet(
                sourceSets,
                tempDir.resolve("mapped-sources/com/example/target/SimpleTarget.java"),
            )?.name,
        )
    }
}
