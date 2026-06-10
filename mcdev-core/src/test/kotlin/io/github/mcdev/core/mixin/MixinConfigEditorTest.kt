package io.github.mcdev.core.mixin

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class MixinConfigEditorTest {
    private val editor = MixinConfigEditor()

    @Test
    fun parsesMixinConfigObject() {
        val content = """
            {
              "package": "example.mixin",
              "mixins": ["AlphaMixin"],
              "client": ["ClientMixin"]
            }
        """.trimIndent()
        val config = editor.parse(content, "mixins.json")
        assertEquals("example.mixin", config.packageName)
        assertEquals(listOf("AlphaMixin"), config.mixins)
        assertEquals(listOf("ClientMixin"), config.client)
    }

    @Test
    fun parsesRootArrayConfig() {
        val content = """["AlphaMixin", "BetaMixin"]"""
        val config = editor.parse(content)
        assertEquals(listOf("AlphaMixin", "BetaMixin"), config.mixins)
    }

    @Test
    fun addEntrySortsDeterministically() {
        val content = """{ "mixins": ["ZuluMixin", "AlphaMixin"] }"""
        val result = editor.addEntry(content, "MikeMixin")
        assertTrue(result.added)
        assertEquals(listOf("AlphaMixin", "MikeMixin", "ZuluMixin"), editor.parse(result.content).mixins)
    }

    @Test
    fun addEntryDoesNotDuplicate() {
        val content = """{ "mixins": ["ExampleMixin"] }"""
        val result = editor.addEntry(content, "ExampleMixin")
        assertFalse(result.added)
        assertEquals(content, result.content)
    }

    @Test
    fun containsEntryChecksAllArrays() {
        val content = """{ "mixins": [], "client": ["ClientMixin"] }"""
        assertTrue(editor.containsEntry(content, "ClientMixin"))
        assertFalse(editor.containsEntry(content, "MissingMixin"))
    }

    @Test
    fun listMixinClassesReturnsSortedDistinctEntries() {
        val content = """{ "mixins": ["B", "A"], "client": ["A", "C"] }"""
        assertEquals(listOf("A", "B", "C"), editor.listMixinClasses(content))
    }
}
