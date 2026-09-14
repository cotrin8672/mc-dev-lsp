package io.github.mcdev.core.mixinextras

import kotlin.test.Test
import kotlin.test.assertEquals

class MixinExtrasDefinitionIndexTest {
    @Test
    fun emptyIndexHasNoDefinitions() {
        val index = MixinExtrasDefinitionIndex()
        assertEquals(emptyList(), index.definitions)
        assertEquals(emptyList(), index.definitionsWithId("main"))
    }

    @Test
    fun preservesDocumentOrder() {
        val first = MixinExtrasDefinition(id = "a")
        val second = MixinExtrasDefinition(
            id = "b",
            rawMethodReferences = listOf("Lcom/example/Foo;bar()V"),
        )
        val third = MixinExtrasDefinition(
            id = "c",
            classLiteralTypeNames = listOf("java.lang.String"),
            localSpecs = listOf(HandlerParameterSugarSpec.Local(ordinal = 0)),
        )
        val index = MixinExtrasDefinitionIndex(listOf(first, second, third))
        assertEquals(listOf(first, second, third), index.definitions)
    }

    @Test
    fun definitionsWithIdReturnsAllMatchesInDocumentOrder() {
        val first = MixinExtrasDefinition(id = "main", rawFieldReferences = listOf("Lcom/example/Foo;field:I"))
        val other = MixinExtrasDefinition(id = "other")
        val second = MixinExtrasDefinition(id = "main", rawMethodReferences = listOf("Lcom/example/Foo;run()V"))
        val index = MixinExtrasDefinitionIndex(listOf(first, other, second))
        assertEquals(listOf(first, second), index.definitionsWithId("main"))
        assertEquals(listOf(other), index.definitionsWithId("other"))
        assertEquals(emptyList(), index.definitionsWithId("missing"))
    }
}
