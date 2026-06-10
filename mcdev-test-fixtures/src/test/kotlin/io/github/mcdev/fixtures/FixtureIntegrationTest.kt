package io.github.mcdev.fixtures

import io.github.mcdev.core.mapping.MappingParseResult
import io.github.mcdev.core.mapping.TinyV2Parser
import io.github.mcdev.core.model.MappingNamespace
import io.github.mcdev.core.project.PlatformDetector
import io.github.mcdev.core.project.ModPlatform
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue

class FixtureResourceLoaderTest {
    @Test
    fun loadsAllRequiredFixtureResources() {
        FixturePaths.REQUIRED_RESOURCES.forEach { path ->
            assertTrue(FixtureResourceLoader.exists(path), "missing fixture resource: $path")
        }
    }

    @Test
    fun loadsFabricBasicBuildSnippet() {
        val buildGradle = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_BUILD_GRADLE)
        assertTrue(buildGradle.contains("fabric-loom"))
        assertTrue(buildGradle.contains("mixin:0.8.7"))
    }

    @Test
    fun loadsCompiledSimpleTargetClassBytes() {
        val bytes = FixtureResourceLoader.loadBytes(FixturePaths.SIMPLE_TARGET_CLASS)
        assertTrue(bytes.size > 4)
        assertEquals(0xCA, bytes[0].toInt() and 0xFF)
        assertEquals(0xFE, bytes[1].toInt() and 0xFF)
        assertEquals(0xBA, bytes[2].toInt() and 0xFF)
        assertEquals(0xBE, bytes[3].toInt() and 0xFF)
    }

    @Test
    fun listsResourcesUnderFixtureRoot() {
        val resources = FixtureResourceLoader.listResources(FixturePaths.FABRIC_BASIC)
        assertTrue(resources.contains(FixturePaths.FABRIC_BASIC_MIXINS_JSON))
        assertTrue(resources.contains(FixturePaths.FABRIC_BASIC_EXAMPLE_MIXIN))
    }
}

class FixtureMappingTest {
    @Test
    fun parsesFabricBasicTinyMappings() {
        val mappingsText = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_MAPPINGS)
        val result = assertIs<MappingParseResult.Success>(TinyV2Parser.parse(mappingsText))

        assertEquals(
            "com/example/target/class_1",
            result.mappings.className(
                "com/example/target/SimpleTarget",
                MappingNamespace.NAMED,
                MappingNamespace.INTERMEDIARY,
            ),
        )
        assertEquals(
            "method_1",
            result.mappings.methodName(
                "com/example/target/SimpleTarget",
                "draw",
                "(Ljava/lang/String;FF)V",
                MappingNamespace.NAMED,
                MappingNamespace.INTERMEDIARY,
            ),
        )
        assertEquals(
            "field_1",
            result.mappings.fieldName(
                "com/example/target/SimpleTarget",
                "counter",
                "I",
                MappingNamespace.NAMED,
                MappingNamespace.INTERMEDIARY,
            ),
        )
    }
}

class FixtureMixinConfigTest {
    @Test
    fun validatesFabricBasicMixinConfigStructure() {
        val mixinsJson = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_MIXINS_JSON)
        assertEquals(emptyList(), MixinConfigValidator.validateStructure(mixinsJson))

        val config = MixinConfigValidator.parse(mixinsJson)
        assertEquals("com.example.mixin", config.packageName)
        assertEquals(listOf("ExampleMixin"), config.mixins)
    }

    @Test
    fun validatesFabricMixinExtrasConfigStructure() {
        val mixinsJson = FixtureResourceLoader.loadText(FixturePaths.FABRIC_MIXINEXTRAS_MIXINS_JSON)
        assertEquals(emptyList(), MixinConfigValidator.validateStructure(mixinsJson))

        val config = MixinConfigValidator.parse(mixinsJson)
        assertEquals(listOf("MixinExtrasExample"), config.mixins)
    }

    @Test
    fun validatesBrokenDiagnosticsMixinConfigStructure() {
        val mixinsJson = FixtureResourceLoader.loadText(FixturePaths.BROKEN_DIAGNOSTICS_MIXINS_JSON)
        assertEquals(emptyList(), MixinConfigValidator.validateStructure(mixinsJson))

        val config = MixinConfigValidator.parse(mixinsJson)
        assertTrue(config.mixins.isEmpty())
    }

    @Test
    fun brokenDiagnosticsMixinSourceContainsInvalidTargets() {
        val mixinSource = FixtureResourceLoader.loadText(FixturePaths.BROKEN_DIAGNOSTICS_MIXIN)
        assertTrue(mixinSource.contains("missingMethod"))
        assertTrue(mixinSource.contains("com/example/missing/Missing"))
    }

    @Test
    fun fabricMixinExtrasSourceContainsMixinExtrasAnnotations() {
        val mixinSource = FixtureResourceLoader.loadText(FixturePaths.FABRIC_MIXINEXTRAS_MIXIN)
        assertTrue(mixinSource.contains("@ModifyExpressionValue"))
        assertTrue(mixinSource.contains("@ModifyReturnValue"))
        assertTrue(mixinSource.contains("@WrapOperation"))
    }

    @Test
    fun forgeBasicFixtureContainsForgeGradleMarker() {
        val buildGradle = FixtureResourceLoader.loadText(FixturePaths.FORGE_BASIC_BUILD_GRADLE)
        assertEquals(ModPlatform.FORGE, PlatformDetector.detect(listOf(buildGradle)))
        assertTrue(buildGradle.contains("mixin:0.8.7"))
    }

    @Test
    fun neoforgeBasicFixtureContainsNeoForgeMarker() {
        val buildGradle = FixtureResourceLoader.loadText(FixturePaths.NEOFORGE_BASIC_BUILD_GRADLE)
        assertEquals(ModPlatform.NEOFORGE, PlatformDetector.detect(listOf(buildGradle)))
    }

    @Test
    fun validatesForgeBasicMixinConfigStructure() {
        val mixinsJson = FixtureResourceLoader.loadText(FixturePaths.FORGE_BASIC_MIXINS_JSON)
        assertEquals(emptyList(), MixinConfigValidator.validateStructure(mixinsJson))
        assertEquals(listOf("ForgeExampleMixin"), MixinConfigValidator.parse(mixinsJson).mixins)
    }

    @Test
    fun validatesNeoForgeBasicMixinConfigStructure() {
        val mixinsJson = FixtureResourceLoader.loadText(FixturePaths.NEOFORGE_BASIC_MIXINS_JSON)
        assertEquals(emptyList(), MixinConfigValidator.validateStructure(mixinsJson))
        assertEquals(listOf("NeoForgeExampleMixin"), MixinConfigValidator.parse(mixinsJson).mixins)
    }
}
