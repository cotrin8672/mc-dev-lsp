package io.github.mcdev.jdtls.java

import io.github.mcdev.core.project.ClasspathSnapshot
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Files
import java.nio.file.Path

class JdtClasspathBridgeTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun discoversLoomRemappedJars() {
        val loomDir = tempDir.resolve(".gradle/loom-cache/remapped_working")
        Files.createDirectories(loomDir)
        val jar = loomDir.resolve("minecraft-client-mapped.jar")
        Files.writeString(jar, "fake")
        val discovered = JdtClasspathBridge.discoverLoomRemappedJars(tempDir)
        assertEquals(1, discovered.size)
        assertTrue(discovered.first().fileName.toString().contains("minecraft"))
    }

    @Test
    fun mergeClasspathAddsMinecraftJarsFromLoomNames() {
        val jar = tempDir.resolve("minecraft-client-mapped.jar")
        Files.writeString(jar, "fake")
        val merged = JdtClasspathBridge.mergeClasspath(
            ClasspathSnapshot.EMPTY,
            listOf(jar),
        )
        assertEquals(1, merged.minecraftJars.size)
    }

    @Test
    fun mergeWithJdtClasspathReturnsBaseWhenJdtUnavailable() {
        val base = ClasspathSnapshot(
            projectOutputs = listOf(tempDir.resolve("classes")),
        )
        val merged = JdtClasspathBridge.mergeWithJdtClasspath(
            base,
            "file://${tempDir.toUri().rawPath}",
        )
        assertEquals(base, merged)
    }
}
