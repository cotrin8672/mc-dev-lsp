package io.github.mcdev.jdtls.convert

import com.google.gson.Gson
import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata
import io.github.mcdev.core.mixin.CLASS_COMPLETION_LIMIT
import io.github.mcdev.protocol.McdevCompletionResponse
import kotlin.test.Test
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class CompletionTruncationTest {
    @Test
    fun onlyCappedClassSourcesAreIncomplete() {
        assertTrue(CompletionItemConverter.isIncomplete(items(CLASS_COMPLETION_LIMIT, "mixin.target")))
        assertTrue(CompletionItemConverter.isIncomplete(items(CLASS_COMPLETION_LIMIT, "mixin.targets")))
        assertTrue(CompletionItemConverter.isIncomplete(items(CLASS_COMPLETION_LIMIT, "aw.class")))
        assertTrue(CompletionItemConverter.isIncomplete(items(CLASS_COMPLETION_LIMIT, "at.class")))
        assertFalse(CompletionItemConverter.isIncomplete(items(CLASS_COMPLETION_LIMIT - 1, "mixin.target")))
        assertFalse(CompletionItemConverter.isIncomplete(items(CLASS_COMPLETION_LIMIT, "mixin.atTarget")))
    }

    @Test
    fun wireResponseIncludesIsIncomplete() {
        val response = McdevCompletionResponse(
            items = emptyList(),
            isIncomplete = true,
        )

        assertTrue(Gson().toJson(response).contains("\"isIncomplete\":true"))
        assertTrue(Gson().toJson(response.copy(isIncomplete = false)).contains("\"isIncomplete\":false"))
    }

    private fun items(count: Int, source: String): List<McCompletionItem> =
        List(count) { index ->
            McCompletionItem(
                label = "Class$index",
                detail = null,
                documentation = null,
                filterText = "Class$index",
                insertText = "Class$index.class",
                kind = McCompletionKind.CLASS,
                sortKey = "0100_Class$index",
                metadata = McCompletionMetadata(source = source),
            )
        }
}
