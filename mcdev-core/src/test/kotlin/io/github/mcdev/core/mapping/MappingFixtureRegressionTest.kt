package io.github.mcdev.core.mapping

import io.github.mcdev.core.model.MappingNamespace
import io.github.mcdev.core.mapping.asResolver
import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.fixtures.FixtureResourceLoader
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs

class MappingFixtureRegressionTest {
    private val resolver: MappingResolver by lazy {
        val mappingsText = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_MAPPINGS)
        assertIs<MappingParseResult.Success>(TinyV2Parser.parse(mappingsText)).mappings.asResolver()
    }

    @Test
    fun remapsSimpleTargetClassNamedToIntermediary() {
        val result = assertIs<MappingLookupResult.Found<ClassRef>>(
            resolver.remapClass(
                ClassRef("com/example/target/SimpleTarget", MappingNamespace.NAMED),
                MappingNamespace.INTERMEDIARY,
            ),
        )
        assertEquals("com/example/target/class_1", result.value.internalName)
    }

    @Test
    fun remapsSimpleTargetMethodNamedToIntermediary() {
        val result = assertIs<MappingLookupResult.Found<MethodRef>>(
            resolver.remapMethod(
                MethodRef(
                    owner = "com/example/target/SimpleTarget",
                    name = "draw",
                    descriptor = "(Ljava/lang/String;FF)V",
                    namespace = MappingNamespace.NAMED,
                ),
                MappingNamespace.INTERMEDIARY,
            ),
        )
        assertEquals("com/example/target/class_1", result.value.owner)
        assertEquals("method_1", result.value.name)
        assertEquals("(Ljava/lang/String;FF)V", result.value.descriptor)
    }

    @Test
    fun remapsSimpleTargetFieldNamedToIntermediary() {
        val result = assertIs<MappingLookupResult.Found<FieldRef>>(
            resolver.remapField(
                FieldRef(
                    owner = "com/example/target/SimpleTarget",
                    name = "counter",
                    descriptor = "I",
                    namespace = MappingNamespace.NAMED,
                ),
                MappingNamespace.INTERMEDIARY,
            ),
        )
        assertEquals("field_1", result.value.name)
    }

    @Test
    fun remapsMethodDescriptorTypesNamedToIntermediary() {
        val result = assertIs<MappingLookupResult.Found<String>>(
            resolver.remapDescriptor(
                "(Lcom/example/target/SimpleTarget;)V",
                MappingNamespace.NAMED,
                MappingNamespace.INTERMEDIARY,
            ),
        )
        assertEquals("(Lcom/example/target/class_1;)V", result.value)
    }

    @Test
    fun reportsMissingIntermediaryMappingForUnknownClass() {
        val missing = assertIs<MappingLookupResult.Missing>(
            resolver.remapClass(
                ClassRef("com/example/missing/Unknown", MappingNamespace.NAMED),
                MappingNamespace.INTERMEDIARY,
            ),
        )
        assertEquals(MappingSubject.CLASS, missing.subject)
        assertEquals(MappingNamespace.NAMED, missing.from)
        assertEquals(MappingNamespace.INTERMEDIARY, missing.to)
    }
}
