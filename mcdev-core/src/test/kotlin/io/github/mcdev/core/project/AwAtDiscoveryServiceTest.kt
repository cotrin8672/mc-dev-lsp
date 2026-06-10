package io.github.mcdev.core.project

import io.github.mcdev.core.model.MappingNamespace
import kotlin.io.path.createDirectories
import kotlin.io.path.writeText
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Path

class AwAtDiscoveryServiceTest {
    @TempDir
    lateinit var tempDir: Path

    private val accessWidenerContent = """
        accessWidener v2 named
        accessible class net/minecraft/client/MinecraftClient
        extendable method net/minecraft/client/MinecraftClient tick ()V
        mutable field net/minecraft/client/MinecraftClient currentScreen Lnet/minecraft/client/gui/screen/Screen;
    """.trimIndent()

    private val accessTransformerContent = """
        public net.minecraft.client.Minecraft
        public-f net.minecraft.client.Minecraft func_91152_a(Ljava/lang/String;)V
    """.trimIndent()

    @Test
    fun identifiesAccessWidenerExtensions() {
        assertTrue(AwAtDiscoveryService.isAccessWidenerFile(Path.of("mod.accesswidener")))
        assertTrue(AwAtDiscoveryService.isAccessWidenerFile(Path.of("mod.aw")))
    }

    @Test
    fun identifiesAccessTransformerFiles() {
        assertTrue(AwAtDiscoveryService.isAccessTransformerFile(Path.of("accesstransformer.cfg")))
        assertTrue(AwAtDiscoveryService.isAccessTransformerFile(Path.of("mod_at.cfg")))
    }

    @Test
    fun parsesValidAccessWidener() {
        val path = tempDir.resolve("mod.accesswidener").apply { writeText(accessWidenerContent) }
        val ref = AwAtDiscoveryService.parseAccessWidener(path)

        assertEquals(MappingNamespace.NAMED, ref.namespace)
        assertNotNull(ref.parsed)
        assertEquals(3, ref.parsed!!.entries.size)
    }

    @Test
    fun parsesInvalidAccessWidenerWithoutParsedModel() {
        val path = tempDir.resolve("bad.aw").apply { writeText("invalid header") }
        val ref = AwAtDiscoveryService.parseAccessWidener(path)

        assertNull(ref.namespace)
        assertNull(ref.parsed)
    }

    @Test
    fun parsesValidAccessTransformer() {
        val path = tempDir.resolve("accesstransformer.cfg").apply { writeText(accessTransformerContent) }
        val ref = AwAtDiscoveryService.parseAccessTransformer(path)

        assertNotNull(ref.parsed)
        assertEquals(2, ref.parsed!!.entries.size)
    }

    @Test
    fun parsesInvalidAccessTransformerWithoutParsedModel() {
        val path = tempDir.resolve("bad_at.cfg").apply { writeText("bad line") }
        val ref = AwAtDiscoveryService.parseAccessTransformer(path)

        assertNull(ref.parsed)
    }

    @Test
    fun discoversAccessWidenerFilesInTree() {
        val resources = tempDir.resolve("src/main/resources").createDirectories()
        resources.resolve("mod.accesswidener").writeText(accessWidenerContent)
        resources.resolve("extra.aw").writeText(accessWidenerContent)

        val refs = AwAtDiscoveryService.discoverAccessWideners(tempDir)
        assertEquals(2, refs.size)
    }

    @Test
    fun discoversAccessTransformerFilesInTree() {
        val resources = tempDir.resolve("src/main/resources/META-INF").createDirectories()
        resources.resolve("accesstransformer.cfg").writeText(accessTransformerContent)

        val refs = AwAtDiscoveryService.discoverAccessTransformers(tempDir)
        assertEquals(1, refs.size)
        assertNotNull(refs.first().parsed)
    }

    @Test
    fun excludesBuildDirectoryAwAtFiles() {
        val buildDir = tempDir.resolve("build/resources").createDirectories()
        buildDir.resolve("mod.aw").writeText(accessWidenerContent)
        val resources = tempDir.resolve("src/main/resources").createDirectories()
        resources.resolve("mod.aw").writeText(accessWidenerContent)

        val refs = AwAtDiscoveryService.discoverAccessWideners(tempDir)
        assertEquals(1, refs.size)
        assertTrue(refs.first().path.toString().contains("src"))
    }
}
