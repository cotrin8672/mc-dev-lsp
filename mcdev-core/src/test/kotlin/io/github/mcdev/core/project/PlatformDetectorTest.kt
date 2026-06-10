package io.github.mcdev.core.project

import kotlin.io.path.createDirectories
import kotlin.io.path.writeText
import kotlin.test.Test
import kotlin.test.assertEquals
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Path

class PlatformDetectorTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun detectsFabricFromFabricLoomPlugin() {
        val content = """
            plugins {
                id 'fabric-loom' version '1.7-SNAPSHOT'
            }
        """.trimIndent()
        assertEquals(ModPlatform.FABRIC, PlatformDetector.detect(listOf(content)))
    }

    @Test
    fun detectsFabricFromFabricLoomCoordinate() {
        val content = "id(\"net.fabricmc.fabric-loom\") version \"1.6.12\""
        assertEquals(ModPlatform.FABRIC, PlatformDetector.detect(listOf(content)))
    }

    @Test
    fun detectsNeoForgeFromModdevGradle() {
        val content = """
            plugins {
                id 'org.neoforged.moddev' version '2.0.78'
            }
            dependencies {
                implementation "net.neoforged:neoforge:21.1.0"
            }
        """.trimIndent()
        assertEquals(ModPlatform.NEOFORGE, PlatformDetector.detect(listOf(content)))
    }

    @Test
    fun detectsNeoForgeFromNeoforgedGradleCoordinate() {
        val content = "id(\"net.neoforged.gradle\") version \"7.0.80\""
        assertEquals(ModPlatform.NEOFORGE, PlatformDetector.detect(listOf(content)))
    }

    @Test
    fun detectsForgeFromForgeGradle() {
        val content = """
            plugins {
                id 'net.minecraftforge.gradle' version '6.0.24'
            }
        """.trimIndent()
        assertEquals(ModPlatform.FORGE, PlatformDetector.detect(listOf(content)))
    }

    @Test
    fun detectsForgeFromForgeGradleLegacyMarker() {
        val content = "apply plugin: 'forgegradle'"
        assertEquals(ModPlatform.FORGE, PlatformDetector.detect(listOf(content)))
    }

    @Test
    fun returnsUnknownForPlainJavaProject() {
        val content = """
            plugins {
                id 'java'
            }
        """.trimIndent()
        assertEquals(ModPlatform.UNKNOWN, PlatformDetector.detect(listOf(content)))
    }

    @Test
    fun returnsUnknownForEmptyGradleContents() {
        assertEquals(ModPlatform.UNKNOWN, PlatformDetector.detect(emptyList()))
    }

    @Test
    fun neoforgeTakesPriorityOverForgeWhenBothPresent() {
        val content = """
            id 'org.neoforged.moddev'
            net.minecraftforge.gradle
        """.trimIndent()
        assertEquals(ModPlatform.NEOFORGE, PlatformDetector.detect(listOf(content)))
    }

    @Test
    fun detectsFromRootGradleFiles() {
        tempDir.resolve("build.gradle.kts").writeText(
            """
            plugins {
                id("fabric-loom") version "1.7.4"
            }
            """.trimIndent(),
        )
        assertEquals(ModPlatform.FABRIC, PlatformDetector.detectFromRoot(tempDir))
    }

    @Test
    fun combinesMultipleGradleFilesForDetection() {
        tempDir.resolve("settings.gradle.kts").writeText("// settings")
        tempDir.resolve("build.gradle.kts").writeText("id(\"net.minecraftforge.gradle\")")
        assertEquals(ModPlatform.FORGE, PlatformDetector.detectFromRoot(tempDir))
    }
}
