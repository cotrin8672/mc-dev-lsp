package io.github.mcdev.core.project

import io.github.mcdev.core.mapping.MappingParseResult
import io.github.mcdev.core.model.MappingNamespace
import kotlin.io.path.createDirectories
import kotlin.io.path.name
import kotlin.io.path.writeText
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Path

class MappingDiscoveryServiceTest {
    @TempDir
    lateinit var tempDir: Path

    private val tinyMapping = """
        tiny	2	0	named	intermediary
        c	net/minecraft/client/MinecraftClient	net/minecraft/class_310
        	m	()V	tick	method_1574
        """.trimIndent()

    private val srgMapping = """
        CL: net/minecraft/client/Minecraft net/minecraft/client/Minecraft
        MD: net/minecraft/client/Minecraft/func_91152_a (Ljava/lang/String;)V net/minecraft/client/Minecraft/m_91152_ (Ljava/lang/String;)V
        """.trimIndent()

    @Test
    fun discoversTinyMappingFiles() {
        val mappingsDir = tempDir.resolve("mappings").createDirectories()
        mappingsDir.resolve("minecraft.tiny").writeText(tinyMapping)

        val files = MappingDiscoveryService.discoverMappingFiles(tempDir)
        assertEquals(1, files.size)
        assertTrue(files.first().name.endsWith("minecraft.tiny"))
    }

    @Test
    fun discoversSrgMappingFiles() {
        tempDir.resolve("joined.srg").writeText(srgMapping)
        val files = MappingDiscoveryService.discoverMappingFiles(tempDir)
        assertEquals(1, files.size)
        assertEquals("joined.srg", files.first().name)
    }

    @Test
    fun excludesBuildDirectoryMappings() {
        val buildMappings = tempDir.resolve("build/mappings").createDirectories()
        buildMappings.resolve("ignored.tiny").writeText(tinyMapping)
        tempDir.resolve("valid.tiny").writeText(tinyMapping)

        val files = MappingDiscoveryService.discoverMappingFiles(tempDir)
        assertEquals(1, files.size)
        assertEquals("valid.tiny", files.first().name)
    }

    @Test
    fun parsesTinyMappingFile() {
        val path = tempDir.resolve("test.tiny").apply { writeText(tinyMapping) }
        val result = assertIs<MappingParseResult.Success>(MappingDiscoveryService.parseMappingFile(path))
        assertEquals(listOf(MappingNamespace.NAMED, MappingNamespace.INTERMEDIARY), result.mappings.namespaces)
    }

    @Test
    fun parsesSrgMappingFile() {
        val path = tempDir.resolve("test.srg").apply { writeText(srgMapping) }
        val result = assertIs<MappingParseResult.Success>(MappingDiscoveryService.parseMappingFile(path))
        assertTrue(result.mappings.namespaces.contains(MappingNamespace.SRG))
    }

    @Test
    fun buildsFabricMappingContextFromTinyFile() {
        tempDir.resolve("mappings.tiny").writeText(tinyMapping)
        val context = MappingDiscoveryService.discoverMappingContext(tempDir, ModPlatform.FABRIC)

        assertEquals(MappingNamespace.NAMED, context.sourceNamespace)
        assertEquals(MappingNamespace.INTERMEDIARY, context.runtimeNamespace)
        assertEquals(MappingNamespace.NAMED, context.awNamespace)
        assertEquals(setOf(MappingNamespace.NAMED, MappingNamespace.INTERMEDIARY), context.availableNamespaces)
        assertTrue(context.resolver.namespaces.isNotEmpty())
    }

    @Test
    fun buildsForgeMappingContextFromSrgFile() {
        tempDir.resolve("joined.srg").writeText(srgMapping)
        val context = MappingDiscoveryService.discoverMappingContext(tempDir, ModPlatform.FORGE)

        assertEquals(MappingNamespace.SRG, context.atNamespace)
        assertTrue(context.availableNamespaces.contains(MappingNamespace.SRG))
    }

    @Test
    fun emptyMappingContextWhenNoFilesFound() {
        val context = MappingDiscoveryService.discoverMappingContext(tempDir, ModPlatform.UNKNOWN)
        assertTrue(context.availableNamespaces.isEmpty())
        assertEquals(EmptyMappingResolver, context.resolver)
        assertEquals(null, context.awNamespace)
        assertEquals(null, context.atNamespace)
    }

    @Test
    fun buildMappingContextFromExplicitFileList() {
        val tinyPath = tempDir.resolve("tiny.tiny").apply { writeText(tinyMapping) }
        val context = MappingDiscoveryService.buildMappingContext(listOf(tinyPath), ModPlatform.FABRIC)
        assertEquals(MappingNamespace.NAMED, context.sourceNamespace)
        assertEquals(MappingNamespace.INTERMEDIARY, context.runtimeNamespace)
    }

    @Test
    fun returnsEmptyListForNonexistentRoot() {
        val missing = tempDir.resolve("does-not-exist")
        assertEquals(emptyList(), MappingDiscoveryService.discoverMappingFiles(missing))
    }
}
