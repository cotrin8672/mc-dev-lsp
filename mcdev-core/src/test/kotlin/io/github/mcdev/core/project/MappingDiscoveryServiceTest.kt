package io.github.mcdev.core.project

import io.github.mcdev.core.mapping.ClassRef
import io.github.mcdev.core.mapping.MappingLookupResult
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

    private val secondaryTinyMapping = """
        tiny	2	0	named	intermediary
        c	net/minecraft/client/gui/screen/Screen	net/minecraft/class_437
        	m	()V	tick	method_25426
        """.trimIndent()

    private val srgOnlyClassMapping = """
        CL: net/minecraft/world/level/Level net/minecraft/world/level/Level
        MD: net/minecraft/world/level/Level/m_5776_ ()V net/minecraft/world/level/Level/func_72912_H ()V
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
    fun includesGradleLoomCacheMappings() {
        val loomCache = tempDir.resolve(".gradle/loom-cache/remapped_working").createDirectories()
        loomCache.resolve("mappings.tiny").writeText(tinyMapping)
        tempDir.resolve(".gradle/caches/unrelated").createDirectories().resolve("ignored.tiny").writeText(tinyMapping)

        val files = MappingDiscoveryService.discoverMappingFiles(tempDir)
        assertEquals(1, files.size)
        assertTrue(files.first().toString().replace('\\', '/').contains(".gradle/loom-cache/"))
    }

    @Test
    fun includesGradleFabricLoomCacheMappings() {
        val fabricLoomCache = tempDir.resolve(".gradle/caches/fabric-loom/mappings").createDirectories()
        fabricLoomCache.resolve("intermediary.tiny").writeText(tinyMapping)

        val files = MappingDiscoveryService.discoverMappingFiles(tempDir)
        assertEquals(1, files.size)
        assertTrue(files.first().toString().replace('\\', '/').contains(".gradle/caches/fabric-loom/"))
    }

    @Test
    fun includesGradleModuleCacheMappings() {
        val yarnCache = tempDir
            .resolve(".gradle/caches/modules-2/files-2.1/net.fabricmc/yarn/1.21.1")
            .createDirectories()
        yarnCache.resolve("mappings.tiny").writeText(tinyMapping)

        val minecraftCache = tempDir
            .resolve(".gradle/caches/modules-2/files-2.1/net.minecraft/mappings/1.21.1")
            .createDirectories()
        minecraftCache.resolve("client.tiny").writeText(tinyMapping)

        val forgeCache = tempDir
            .resolve(".gradle/caches/modules-2/files-2.1/net.minecraftforge/forge/1.21.1")
            .createDirectories()
        forgeCache.resolve("joined.srg").writeText(srgMapping)

        val neoforgeCache = tempDir
            .resolve(".gradle/caches/modules-2/files-2.1/net.neoforged/neoforge/1.21.1")
            .createDirectories()
        neoforgeCache.resolve("joined.srg").writeText(srgMapping)

        val files = MappingDiscoveryService.discoverMappingFiles(tempDir)
        assertEquals(4, files.size)
        val normalized = files.map { it.toString().replace('\\', '/') }
        assertTrue(normalized.any { it.contains("net.fabricmc/yarn/") })
        assertTrue(normalized.any { it.contains("net.minecraft/") })
        assertTrue(normalized.any { it.contains("net.minecraftforge/") })
        assertTrue(normalized.any { it.contains("net.neoforged/") })
    }

    @Test
    fun includesBuildLoomCacheAndForgeTmpMappings() {
        val loomCache = tempDir.resolve("build/loom-cache/remapped_working").createDirectories()
        loomCache.resolve("loom.tiny").writeText(tinyMapping)

        val forgeTmp = tempDir.resolve("build/createSrgToMcp/output").createDirectories()
        forgeTmp.resolve("joined.srg").writeText(srgMapping)

        val forgeTmpDir = tempDir.resolve("build/tmp/compileJava").createDirectories()
        forgeTmpDir.resolve("generated.srg").writeText(srgMapping)

        tempDir.resolve("build/classes").createDirectories().resolve("ignored.tiny").writeText(tinyMapping)

        val files = MappingDiscoveryService.discoverMappingFiles(tempDir)
        assertEquals(3, files.size)
        val normalized = files.map { it.toString().replace('\\', '/') }
        assertTrue(normalized.any { it.contains("build/loom-cache/") })
        assertTrue(normalized.any { it.contains("build/createSrgToMcp/") })
        assertTrue(normalized.any { it.contains("build/tmp/") })
    }

    @Test
    fun excludesUnrelatedGradleBuildAndNodeModulesMappings() {
        tempDir.resolve(".gradle/caches/other").createDirectories().resolve("ignored.tiny").writeText(tinyMapping)
        tempDir.resolve("build/classes").createDirectories().resolve("ignored.tiny").writeText(tinyMapping)
        tempDir.resolve("node_modules/pkg").createDirectories().resolve("ignored.tiny").writeText(tinyMapping)
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
    fun mergesMultipleTinyMappingFilesForLookup() {
        val firstPath = tempDir.resolve("first.tiny").apply { writeText(tinyMapping) }
        val secondPath = tempDir.resolve("second.tiny").apply { writeText(secondaryTinyMapping) }

        val context = MappingDiscoveryService.buildMappingContext(listOf(firstPath, secondPath), ModPlatform.FABRIC)

        assertEquals(
            setOf(MappingNamespace.NAMED, MappingNamespace.INTERMEDIARY),
            context.availableNamespaces,
        )

        val firstClass = assertIs<MappingLookupResult.Found<ClassRef>>(
            context.resolver.remapClass(
                ClassRef("net/minecraft/client/MinecraftClient", MappingNamespace.NAMED),
                MappingNamespace.INTERMEDIARY,
            ),
        )
        assertEquals("net/minecraft/class_310", firstClass.value.internalName)

        val secondClass = assertIs<MappingLookupResult.Found<ClassRef>>(
            context.resolver.remapClass(
                ClassRef("net/minecraft/client/gui/screen/Screen", MappingNamespace.NAMED),
                MappingNamespace.INTERMEDIARY,
            ),
        )
        assertEquals("net/minecraft/class_437", secondClass.value.internalName)
    }

    @Test
    fun mergesTinyAndSrgMappingFilesForLookup() {
        val tinyPath = tempDir.resolve("client.tiny").apply { writeText(tinyMapping) }
        val srgPath = tempDir.resolve("joined.srg").apply { writeText(srgOnlyClassMapping) }

        val context = MappingDiscoveryService.buildMappingContext(listOf(tinyPath, srgPath), ModPlatform.FORGE)

        assertEquals(
            setOf(
                MappingNamespace.NAMED,
                MappingNamespace.INTERMEDIARY,
                MappingNamespace.OFFICIAL,
                MappingNamespace.SRG,
            ),
            context.availableNamespaces,
        )

        val tinyClass = assertIs<MappingLookupResult.Found<ClassRef>>(
            context.resolver.remapClass(
                ClassRef("net/minecraft/client/MinecraftClient", MappingNamespace.NAMED),
                MappingNamespace.INTERMEDIARY,
            ),
        )
        assertEquals("net/minecraft/class_310", tinyClass.value.internalName)

        val srgClass = assertIs<MappingLookupResult.Found<ClassRef>>(
            context.resolver.remapClass(
                ClassRef("net/minecraft/world/level/Level", MappingNamespace.OFFICIAL),
                MappingNamespace.SRG,
            ),
        )
        assertEquals("net/minecraft/world/level/Level", srgClass.value.internalName)
    }

    @Test
    fun returnsEmptyListForNonexistentRoot() {
        val missing = tempDir.resolve("does-not-exist")
        assertEquals(emptyList(), MappingDiscoveryService.discoverMappingFiles(missing))
    }
}
