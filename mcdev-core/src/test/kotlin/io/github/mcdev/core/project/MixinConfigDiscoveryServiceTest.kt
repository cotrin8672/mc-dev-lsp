package io.github.mcdev.core.project

import kotlin.io.path.createDirectories
import kotlin.io.path.name
import kotlin.io.path.writeText
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Path

class MixinConfigDiscoveryServiceTest {
    @TempDir
    lateinit var tempDir: Path

    private val mixinJson = """
        {
          "required": true,
          "package": "com.example.mixin",
          "mixins": ["ExampleMixin"],
          "client": ["ClientMixin"],
          "server": ["ServerMixin"],
          "common": ["CommonMixin"]
        }
    """.trimIndent()

    @Test
    fun identifiesMixinsJsonFile() {
        assertTrue(MixinConfigDiscoveryService.isMixinConfigFile(Path.of("mixins.json")))
    }

    @Test
    fun identifiesModMixinsJsonFile() {
        assertTrue(MixinConfigDiscoveryService.isMixinConfigFile(Path.of("example.mixins.json")))
    }

    @Test
    fun identifiesMixinNamedJsonFile() {
        assertTrue(MixinConfigDiscoveryService.isMixinConfigFile(Path.of("client-mixin-config.json")))
    }

    @Test
    fun rejectsNonMixinJsonFile() {
        assertTrue(!MixinConfigDiscoveryService.isMixinConfigFile(Path.of("fabric.mod.json")))
    }

    @Test
    fun parsesMixinConfigContent() {
        val path = tempDir.resolve("mixins.json")
        val ref = assertNotNull(MixinConfigDiscoveryService.parseContent(path, mixinJson))

        assertEquals("com.example.mixin", ref.packageName)
        assertEquals(listOf("ExampleMixin"), ref.mixins)
        assertEquals(listOf("ClientMixin"), ref.client)
        assertEquals(listOf("ServerMixin"), ref.server)
        assertEquals(listOf("CommonMixin"), ref.common)
    }

    @Test
    fun parsesMixinConfigWithMissingOptionalFields() {
        val json = """{"mixins": ["OnlyMixin"]}"""
        val path = tempDir.resolve("mixins.json")
        val ref = assertNotNull(MixinConfigDiscoveryService.parseContent(path, json))

        assertNull(ref.packageName)
        assertEquals(listOf("OnlyMixin"), ref.mixins)
        assertTrue(ref.client.isEmpty())
        assertTrue(ref.server.isEmpty())
    }

    @Test
    fun returnsNullForInvalidJson() {
        val path = tempDir.resolve("mixins.json")
        assertNull(MixinConfigDiscoveryService.parseContent(path, "not json"))
    }

    @Test
    fun returnsNullForJsonArrayRoot() {
        val path = tempDir.resolve("mixins.json")
        assertNull(MixinConfigDiscoveryService.parseContent(path, "[]"))
    }

    @Test
    fun discoversMixinConfigsInProjectTree() {
        val resources = tempDir.resolve("src/main/resources").createDirectories()
        resources.resolve("mixins.json").writeText(mixinJson)
        resources.resolve("client.mixins.json").writeText("""{"client": ["A"]}""")

        val configs = MixinConfigDiscoveryService.discover(tempDir)
        assertEquals(2, configs.size)
        assertEquals("com.example.mixin", configs.first { it.path.name == "mixins.json" }.packageName)
    }

    @Test
    fun excludesBuildDirectoryMixinConfigs() {
        val buildResources = tempDir.resolve("build/resources/mixins.json").parent!!.createDirectories()
        buildResources.resolve("mixins.json").writeText(mixinJson)
        val resources = tempDir.resolve("src/main/resources").createDirectories()
        resources.resolve("mixins.json").writeText(mixinJson)

        val configs = MixinConfigDiscoveryService.discover(tempDir)
        assertEquals(1, configs.size)
        assertTrue(configs.first().path.toString().contains("src"))
    }
}
