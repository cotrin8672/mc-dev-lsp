package io.github.mcdev.core.mixin

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertTrue

class MixinConfigEditorTest {
    private val editor = MixinConfigEditor()

    private fun applyDelta(original: String, delta: MixinConfigEditDelta): String =
        original.substring(0, delta.startOffset) + delta.newText + original.substring(delta.endOffset)

    private fun assertLosslessEditDelta(original: String, result: MixinConfigEditResult) {
        val delta = result.delta ?: error("expected lossless edit delta")
        assertEquals(delta.startOffset, delta.endOffset)
        assertEquals(result.content, applyDelta(original, delta))
        assertEquals(original.substring(0, delta.startOffset), result.content.substring(0, delta.startOffset))
        assertEquals(
            original.substring(delta.endOffset),
            result.content.substring(delta.startOffset + delta.newText.length),
        )
    }

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
    fun addEntryPreservesExistingOrder() {
        val content = """{ "mixins": ["ZuluMixin", "AlphaMixin"] }"""
        val result = editor.addEntry(content, "MikeMixin")
        assertTrue(result.added)
        assertEquals("""{ "mixins": ["ZuluMixin", "AlphaMixin", "MikeMixin"] }""", result.content)
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
        val content = """{ "mixins": [], "client": ["ClientMixin"], "common": ["CommonMixin"] }"""
        assertTrue(editor.containsEntry(content, "ClientMixin"))
        assertTrue(editor.containsEntry(content, "CommonMixin"))
        assertFalse(editor.containsEntry(content, "MissingMixin"))
    }

    @Test
    fun listMixinClassesReturnsSortedDistinctEntries() {
        val content = """{ "mixins": ["B", "A"], "client": ["A", "C"], "common": ["D"] }"""
        assertEquals(listOf("A", "B", "C", "D"), editor.listMixinClasses(content))
    }

    @Test
    fun parsesJson5CommentsAndTrailingCommas() {
        val content = """
            {
              // json5-style comment
              "package": "example.mixin",
              "mixins": ["AlphaMixin",],
              "common": ["CommonMixin",],
            }
        """.trimIndent()
        val config = editor.parse(content, "mod.mixins.json5")
        assertEquals("example.mixin", config.packageName)
        assertEquals(listOf("AlphaMixin"), config.mixins)
        assertEquals(listOf("CommonMixin"), config.common)
    }

    @Test
    fun addEntryLosslessIgnoresCommentText() {
        val content = """
            {
              // "client": ["CommentMixin"],
              "mixins": ["AlphaMixin"]
            }
        """.trimIndent()
        val result = editor.addEntry(content, "BetaMixin")
        assertTrue(result.added)
        assertEquals(
            """
            {
              // "client": ["CommentMixin"],
              "mixins": ["AlphaMixin", "BetaMixin"]
            }
            """.trimIndent(),
            result.content,
        )
    }

    @Test
    fun addEntryLosslessIgnoresNestedArrayKey() {
        val content = """
            {
              "nested": { "mixins": ["NestedMixin"] },
              "mixins": ["AlphaMixin"]
            }
        """.trimIndent()
        val result = editor.addEntry(content, "BetaMixin")
        assertTrue(result.added)
        assertEquals(
            """
            {
              "nested": { "mixins": ["NestedMixin"] },
              "mixins": ["AlphaMixin", "BetaMixin"]
            }
            """.trimIndent(),
            result.content,
        )
    }

    @Test
    fun addEntryLosslessIgnoresStringLiteralKeyText() {
        val content = """{ "note": "mixins: [\"FakeMixin\"]", "mixins": ["AlphaMixin"] }"""
        val result = editor.addEntry(content, "BetaMixin")
        assertTrue(result.added)
        assertEquals(
            """{ "note": "mixins: [\"FakeMixin\"]", "mixins": ["AlphaMixin", "BetaMixin"] }""",
            result.content,
        )
    }

    @Test
    fun addEntryLosslessAppendsToEmptyArray() {
        val content = """{ "mixins": [] }"""
        val result = editor.addEntry(content, "AlphaMixin")
        assertTrue(result.added)
        assertEquals("""{ "mixins": ["AlphaMixin"] }""", result.content)
    }

    @Test
    fun addEntryLosslessPreservesTrailingComma() {
        val content = """
            {
              "mixins": ["AlphaMixin",],
            }
        """.trimIndent()
        val result = editor.addEntry(content, "BetaMixin")
        assertTrue(result.added)
        assertEquals(
            """
            {
              "mixins": ["AlphaMixin","BetaMixin",],
            }
            """.trimIndent(),
            result.content,
        )
    }

    @Test
    fun addEntryLosslessPreservesCrlf() {
        val content = "{\r\n  \"mixins\": [\"AlphaMixin\"]\r\n}"
        val result = editor.addEntry(content, "BetaMixin")
        assertTrue(result.added)
        assertEquals("{\r\n  \"mixins\": [\"AlphaMixin\", \"BetaMixin\"]\r\n}", result.content)
    }

    @Test
    fun addEntryLosslessAppendsToClientArray() {
        val content = """{ "client": ["ClientMixin"] }"""
        val result = editor.addEntry(content, "NewClientMixin", arrayName = "client")
        assertTrue(result.added)
        assertEquals("""{ "client": ["ClientMixin", "NewClientMixin"] }""", result.content)
    }

    @Test
    fun addEntryLosslessAppendsToServerArray() {
        val content = """{ "server": ["ServerMixin"] }"""
        val result = editor.addEntry(content, "NewServerMixin", arrayName = "server")
        assertTrue(result.added)
        assertEquals("""{ "server": ["ServerMixin", "NewServerMixin"] }""", result.content)
    }

    @Test
    fun addEntryLosslessEscapesSpecialCharacters() {
        val content = """{ "mixins": [] }"""
        val result = editor.addEntry(content, "Mixin\"Quote\\Slash")
        assertTrue(result.added)
        assertEquals("""{ "mixins": ["Mixin\"Quote\\Slash"] }""", result.content)
    }

    @Test
    fun addEntryLosslessDuplicateIsIdempotentWithEscapedEntry() {
        val content = """{ "mixins": ["Quoted\"Mixin"] }"""
        val result = editor.addEntry(content, "Quoted\"Mixin")
        assertFalse(result.added)
        assertEquals(content, result.content)
    }

    @Test
    fun addEntryLosslessAppendsToCommentOnlyEmptyArray() {
        val content = """
            {
              "mixins": [
                // no entries yet
              ]
            }
        """.trimIndent()
        val result = editor.addEntry(content, "AlphaMixin")
        assertTrue(result.added)
        assertEquals(
            """
            {
              "mixins": ["AlphaMixin"
                // no entries yet
              ]
            }
            """.trimIndent(),
            result.content,
        )
    }

    @Test
    fun addEntryLosslessTrailingCommaBeforeCommentsDoesNotDoubleComma() {
        val content = """
            {
              "mixins": ["AlphaMixin", /* keep trailing comma */ ,]
            }
        """.trimIndent()
        val result = editor.addEntry(content, "BetaMixin")
        assertTrue(result.added)
        assertEquals(
            """
            {
              "mixins": ["AlphaMixin", /* keep trailing comma */ ,"BetaMixin",]
            }
            """.trimIndent(),
            result.content,
        )
    }

    @Test
    fun addEntryLosslessInsertsMixinsArrayWhenMissing() {
        val content = """
            {
              // package config
              "package": "example.mixin",
            }
        """.trimIndent()
        val result = editor.addEntry(content, "NewMixin")
        assertTrue(result.added)
        assertEquals(
            """
            {
              // package config
              "package": "example.mixin",
              "mixins": [
                "NewMixin"
              ]
            }
            """.trimIndent(),
            result.content,
        )
        assertLosslessEditDelta(content, result)
    }

    @Test
    fun addEntryLosslessEditDelta() {
        val emptyArray = """{ "mixins": [] }"""
        val emptyResult = editor.addEntry(emptyArray, "AlphaMixin")
        assertTrue(emptyResult.added)
        assertLosslessEditDelta(emptyArray, emptyResult)
        assertEquals("""{ "mixins": [""", emptyArray.substring(0, emptyResult.delta!!.startOffset))
        assertEquals("\"AlphaMixin\"", emptyResult.delta!!.newText)

        val nonemptyArray = """{ "mixins": ["AlphaMixin"] }"""
        val nonemptyResult = editor.addEntry(nonemptyArray, "BetaMixin")
        assertTrue(nonemptyResult.added)
        assertLosslessEditDelta(nonemptyArray, nonemptyResult)
        assertEquals(", \"BetaMixin\"", nonemptyResult.delta!!.newText)

        val trailingCommaComment = """
            {
              "mixins": ["AlphaMixin", /* keep trailing comma */ ,]
            }
        """.trimIndent()
        val trailingCommaResult = editor.addEntry(trailingCommaComment, "BetaMixin")
        assertTrue(trailingCommaResult.added)
        assertLosslessEditDelta(trailingCommaComment, trailingCommaResult)
        assertEquals("\"BetaMixin\",", trailingCommaResult.delta!!.newText)

        val crlfContent = "{\r\n  \"mixins\": [\"AlphaMixin\"]\r\n}"
        val crlfResult = editor.addEntry(crlfContent, "BetaMixin")
        assertTrue(crlfResult.added)
        assertLosslessEditDelta(crlfContent, crlfResult)

        val duplicate = """{ "mixins": ["ExampleMixin"] }"""
        val duplicateResult = editor.addEntry(duplicate, "ExampleMixin")
        assertFalse(duplicateResult.added)
        assertNull(duplicateResult.delta)
    }

    @Test
    fun addEntryLosslessInsertsMixinsArrayWithLfLineEndingBeforeClosingBrace() {
        val content = "{}"
        val result = editor.addEntry(content, "AlphaMixin")
        assertTrue(result.added)
        assertEquals(
            "{\n  \"mixins\": [\n    \"AlphaMixin\"\n  ]\n}",
            result.content,
        )
        assertLosslessEditDelta(content, result)
    }

    @Test
    fun addEntryLosslessAbortsWhenClosingBracketMissing() {
        val content = """{ "mixins": ["AlphaMixin" """
        assertFailsWith<Exception> {
            editor.addEntry(content, "BetaMixin")
        }
    }

    @Test
    fun addEntryLosslessAbortsOnIncompleteUnicodeEscape() {
        val content = """{ "mixins": [""" + "\"\\" + "u12\"" + """"] }"""
        assertFailsWith<Exception> {
            editor.addEntry(content, "BetaMixin")
        }
    }

    @Test
    fun addEntryLosslessAbortsOnInvalidUnicodeEscape() {
        val content = """{ "mixins": [""" + "\"\\" + "uGGGG\"" + """"] }"""
        assertFailsWith<Exception> {
            editor.addEntry(content, "BetaMixin")
        }
    }
}
