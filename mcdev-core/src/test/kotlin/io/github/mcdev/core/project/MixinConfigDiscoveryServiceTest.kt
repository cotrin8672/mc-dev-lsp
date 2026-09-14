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
    fun identifiesMixinsJson5File() {
        assertTrue(MixinConfigDiscoveryService.isMixinConfigFile(Path.of("mixins.json5")))
    }

    @Test
    fun identifiesMixinNamedJson5File() {
        assertTrue(MixinConfigDiscoveryService.isMixinConfigFile(Path.of("client-mixin-config.json5")))
    }

    @Test
    fun parsesJson5ConfigWithCommentsAndTrailingCommas() {
        val json5 = """
            {
              // package for mixins
              "package": "com.example.mixin",
              "mixins": [
                "ExampleMixin",
              ],
            }
        """.trimIndent()
        val path = tempDir.resolve("mixins.json5")
        val ref = assertNotNull(MixinConfigDiscoveryService.parseContent(path, json5))

        assertEquals("com.example.mixin", ref.packageName)
        assertEquals(listOf("ExampleMixin"), ref.mixins)
    }

    @Test
    fun selectsConfigListingMixinClassOverEarlierConfig() {
        val wrong = MixinConfigRef(
            path = tempDir.resolve("aaa-wrong.mixins.json"),
            packageName = "com.other.mixin",
            mixins = listOf("OtherMixin"),
        )
        val matching = MixinConfigRef(
            path = tempDir.resolve("zzz-target.mixins.json"),
            packageName = "com.example.mixin",
            mixins = listOf("ListedMixin"),
        )

        val selected = MixinConfigDiscoveryService.selectForMixin(
            configs = listOf(wrong, matching),
            mixinClassName = "ListedMixin",
            mixinPackage = "com.example.mixin",
        )

        assertEquals("zzz-target.mixins.json", selected?.path?.name)
    }

    @Test
    fun selectsPackageMatchingConfigWhenClassIsNotListed() {
        val wrong = MixinConfigRef(
            path = tempDir.resolve("aaa-wrong.mixins.json"),
            packageName = "com.other.mixin",
            mixins = emptyList(),
        )
        val matching = MixinConfigRef(
            path = tempDir.resolve("mixins.json"),
            packageName = "com.example.mixin",
            mixins = listOf("ExampleMixin"),
        )

        val selected = MixinConfigDiscoveryService.selectForMixin(
            configs = listOf(wrong, matching),
            mixinClassName = "UnlistedMixin",
            mixinPackage = "com.example.mixin",
        )

        assertEquals("mixins.json", selected?.path?.name)
    }

    @Test
    fun returnsNullWhenNoSelectionEvidence() {
        val first = MixinConfigRef(path = tempDir.resolve("aaa.mixins.json"), packageName = "com.a")
        val second = MixinConfigRef(path = tempDir.resolve("zzz.mixins.json"), packageName = "com.z")

        val selected = MixinConfigDiscoveryService.selectForMixin(
            configs = listOf(second, first),
            mixinClassName = "UnknownMixin",
            mixinPackage = "com.unknown",
        )

        assertNull(selected)
    }

    @Test
    fun returnsNullWhenMultipleConfigsListMixinClass() {
        val first = MixinConfigRef(
            path = tempDir.resolve("aaa.mixins.json"),
            packageName = "com.example.mixin",
            mixins = listOf("SharedMixin"),
        )
        val second = MixinConfigRef(
            path = tempDir.resolve("zzz.mixins.json"),
            packageName = "com.example.mixin",
            mixins = listOf("SharedMixin"),
        )

        val selected = MixinConfigDiscoveryService.selectForMixin(
            configs = listOf(first, second),
            mixinClassName = "SharedMixin",
            mixinPackage = "com.example.mixin",
        )

        assertNull(selected)
    }

    @Test
    fun returnsNullWhenMultipleConfigsMatchPackage() {
        val first = MixinConfigRef(
            path = tempDir.resolve("aaa.mixins.json"),
            packageName = "com.example.mixin",
        )
        val second = MixinConfigRef(
            path = tempDir.resolve("zzz.mixins.json"),
            packageName = "com.example.mixin",
        )

        val selected = MixinConfigDiscoveryService.selectForMixin(
            configs = listOf(first, second),
            mixinClassName = "UnlistedMixin",
            mixinPackage = "com.example.mixin",
        )

        assertNull(selected)
    }

    @Test
    fun selectsParentConfigPackageForSubpackageMixin() {
        val parent = MixinConfigRef(
            path = tempDir.resolve("mixins.json"),
            packageName = "com.example.mixin",
        )
        val unrelated = MixinConfigRef(
            path = tempDir.resolve("other.mixins.json"),
            packageName = "com.other.mixin",
        )

        val selected = MixinConfigDiscoveryService.selectForMixin(
            configs = listOf(unrelated, parent),
            mixinClassName = "SubMixin",
            mixinPackage = "com.example.mixin.client",
        )

        assertEquals("mixins.json", selected?.path?.name)
    }

    @Test
    fun selectsMoreSpecificPackagePrefixOverBroaderOne() {
        val broad = MixinConfigRef(
            path = tempDir.resolve("aaa-broad.mixins.json"),
            packageName = "com.example",
        )
        val specific = MixinConfigRef(
            path = tempDir.resolve("zzz-specific.mixins.json"),
            packageName = "com.example.mixin",
        )

        val selected = MixinConfigDiscoveryService.selectForMixin(
            configs = listOf(broad, specific),
            mixinClassName = "ExampleMixin",
            mixinPackage = "com.example.mixin",
        )

        assertEquals("zzz-specific.mixins.json", selected?.path?.name)
    }

    @Test
    fun selectsConfigListingRelativeSubpackageEntry() {
        val matching = MixinConfigRef(
            path = tempDir.resolve("mixins.json"),
            packageName = "com.example.mixin",
            client = listOf("client.ClientMixin"),
        )
        val wrong = MixinConfigRef(
            path = tempDir.resolve("other.mixins.json"),
            packageName = "com.other.mixin",
            client = listOf("client.ClientMixin"),
        )

        val selected = MixinConfigDiscoveryService.selectForMixin(
            configs = listOf(wrong, matching),
            mixinClassName = "ClientMixin",
            mixinPackage = "com.example.mixin.client",
        )

        assertEquals("mixins.json", selected?.path?.name)
    }

    @Test
    fun returnsNullWhenEqualLongestPackagePrefixIsAmbiguous() {
        val first = MixinConfigRef(
            path = tempDir.resolve("aaa.mixins.json"),
            packageName = "com.example.mixin",
        )
        val second = MixinConfigRef(
            path = tempDir.resolve("zzz.mixins.json"),
            packageName = "com.example.mixin",
        )

        val selected = MixinConfigDiscoveryService.selectForMixin(
            configs = listOf(first, second),
            mixinClassName = "SubMixin",
            mixinPackage = "com.example.mixin.client",
        )

        assertNull(selected)
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

    @Test
    fun excludesGradleDirectoryMixinConfigs() {
        val gradleResources = tempDir.resolve(".gradle/resources/mixins.json").parent!!.createDirectories()
        gradleResources.resolve("mixins.json").writeText(mixinJson)
        val resources = tempDir.resolve("src/main/resources").createDirectories()
        resources.resolve("mixins.json").writeText(mixinJson)

        val configs = MixinConfigDiscoveryService.discover(tempDir)
        assertEquals(1, configs.size)
        assertTrue(configs.first().path.toString().contains("src"))
    }

    @Test
    fun excludesNodeModulesDirectoryMixinConfigs() {
        val nodeModulesResources = tempDir.resolve("node_modules/pkg/mixins.json").parent!!.createDirectories()
        nodeModulesResources.resolve("mixins.json").writeText(mixinJson)
        val resources = tempDir.resolve("src/main/resources").createDirectories()
        resources.resolve("mixins.json").writeText(mixinJson)

        val configs = MixinConfigDiscoveryService.discover(tempDir)
        assertEquals(1, configs.size)
        assertTrue(configs.first().path.toString().contains("src"))
    }

    private fun architecturySourceSets(root: Path): List<SourceSetContext> =
        listOf("fabric", "forge").map { name ->
            SourceSetContext(
                name = name,
                sourceDirectories = listOf(root.resolve("src/$name/java")),
                resourceDirectories = listOf(root.resolve("src/$name/resources")),
            )
        }

    private fun configRef(root: Path, relativePath: String): MixinConfigRef =
        MixinConfigRef(path = root.resolve(relativePath))

    @Test
    fun configsForSourceSetReturnsSelectedFabricOrForgeResources() {
        val sourceSets = architecturySourceSets(tempDir)
        val fabric = sourceSets.single { it.name == "fabric" }
        val forge = sourceSets.single { it.name == "forge" }
        val configs = listOf(
            configRef(tempDir, "src/fabric/resources/fabric.mixins.json"),
            configRef(tempDir, "src/forge/resources/forge.mixins.json"),
        )

        val fabricConfigs = MixinConfigDiscoveryService.configsForSourceSet(
            configs = configs,
            sourceSet = fabric,
            allSourceSets = sourceSets,
            projectRoot = tempDir,
        )
        val forgeConfigs = MixinConfigDiscoveryService.configsForSourceSet(
            configs = configs,
            sourceSet = forge,
            allSourceSets = sourceSets,
            projectRoot = tempDir,
        )

        assertEquals(listOf("fabric.mixins.json"), fabricConfigs.map { it.path.name })
        assertEquals(listOf("forge.mixins.json"), forgeConfigs.map { it.path.name })
    }

    @Test
    fun configsForSourceSetPrefersLocalResourcesOverRootShared() {
        val sourceSets = architecturySourceSets(tempDir)
        val fabric = sourceSets.single { it.name == "fabric" }
        val configs = listOf(
            configRef(tempDir, "src/fabric/resources/fabric.mixins.json"),
            configRef(tempDir, "mixins.json"),
        )

        val scoped = MixinConfigDiscoveryService.configsForSourceSet(
            configs = configs,
            sourceSet = fabric,
            allSourceSets = sourceSets,
            projectRoot = tempDir,
        )

        assertEquals(listOf("fabric.mixins.json"), scoped.map { it.path.name })
    }

    @Test
    fun configsForSourceSetFallsBackToRootSharedWhenSelectedSourceSetHasNoConfig() {
        val sourceSets = architecturySourceSets(tempDir)
        val fabric = sourceSets.single { it.name == "fabric" }
        val configs = listOf(
            configRef(tempDir, "mixins.json"),
            configRef(tempDir, "src/forge/resources/forge.mixins.json"),
        )

        val scoped = MixinConfigDiscoveryService.configsForSourceSet(
            configs = configs,
            sourceSet = fabric,
            allSourceSets = sourceSets,
            projectRoot = tempDir,
        )

        assertEquals(listOf("mixins.json"), scoped.map { it.path.name })
    }

    @Test
    fun configsForSourceSetReturnsEmptyWhenOnlySiblingSourceSetHasConfigs() {
        val sourceSets = architecturySourceSets(tempDir)
        val fabric = sourceSets.single { it.name == "fabric" }
        val configs = listOf(configRef(tempDir, "src/forge/resources/forge.mixins.json"))

        val scoped = MixinConfigDiscoveryService.configsForSourceSet(
            configs = configs,
            sourceSet = fabric,
            allSourceSets = sourceSets,
            projectRoot = tempDir,
        )

        assertTrue(scoped.isEmpty())
    }

    @Test
    fun sharedConfigsReturnsRootUnownedConfigsOnly() {
        val sourceSets = architecturySourceSets(tempDir)
        val outsideRoot = tempDir.parent.resolve("outside.mixins.json")
        val configs = listOf(
            configRef(tempDir, "mixins.json"),
            configRef(tempDir, "shared/common.mixins.json"),
            configRef(tempDir, "src/fabric/resources/fabric.mixins.json"),
            configRef(tempDir, "src/forge/resources/forge.mixins.json"),
            MixinConfigRef(path = outsideRoot),
        )

        val shared = MixinConfigDiscoveryService.sharedConfigs(
            configs = configs,
            allSourceSets = sourceSets,
            projectRoot = tempDir,
        )

        assertEquals(
            listOf("mixins.json", "common.mixins.json"),
            shared.map { it.path.name },
        )
    }

    @Test
    fun configsForSourceSetExcludesPathsOutsideProjectRoot() {
        val sourceSets = architecturySourceSets(tempDir)
        val fabric = sourceSets.single { it.name == "fabric" }
        val outsideRoot = tempDir.parent.resolve("outside.mixins.json")
        val configs = listOf(
            configRef(tempDir, "mixins.json"),
            MixinConfigRef(path = outsideRoot),
        )

        val scoped = MixinConfigDiscoveryService.configsForSourceSet(
            configs = configs,
            sourceSet = fabric,
            allSourceSets = sourceSets,
            projectRoot = tempDir,
        )

        assertEquals(listOf("mixins.json"), scoped.map { it.path.name })
    }
}
