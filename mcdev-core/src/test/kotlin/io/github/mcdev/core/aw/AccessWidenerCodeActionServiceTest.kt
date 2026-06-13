package io.github.mcdev.core.aw

import io.github.mcdev.core.codeaction.FixAccessWidenerDescriptorFix
import io.github.mcdev.core.codeaction.GenerateAccessWidenerEntryFix
import io.github.mcdev.core.codeaction.RemoveDuplicateAccessWidenerEntryFix
import io.github.mcdev.core.codeaction.RemapAccessWidenerEntryFix
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

class AccessWidenerCodeActionServiceTest {
    private val classIndex = AwTestFixtures.classIndex
    private val diagnosticsService = AccessWidenerDiagnosticsService(classIndex)
    private val service = AccessWidenerCodeActionService()

    @Test
    fun offersRemoveDuplicateFix() {
        val source = """
        accessWidener v2 named
        accessible class net/minecraft/client/MinecraftClient
        accessible class net/minecraft/client/MinecraftClient
        """.trimIndent()
        val diagnostics = diagnostics(source)
        val fixes = service.fixesForDiagnostics(diagnostics, "file:///mod.accesswidener", source)
        assertIs<RemoveDuplicateAccessWidenerEntryFix>(fixes.first())
    }

    @Test
    fun offersRemapNamespaceFix() {
        val source = """
        accessWidener v2 named
        accessible class net/minecraft/client/class_310
        """.trimIndent()
        val diagnostics = diagnostics(source, AwTestFixtures.mappingContext())
        val fixes = service.fixesForDiagnostics(
            diagnostics,
            "file:///mod.accesswidener",
            source,
            classIndex,
            AwTestFixtures.mappingContext(),
        )
        assertIs<RemapAccessWidenerEntryFix>(fixes.first())
    }

    @Test
    fun applyRemoveDuplicateFixRemovesLine() {
        val source = """
        accessWidener v2 named
        accessible class net/minecraft/client/MinecraftClient
        accessible class net/minecraft/client/MinecraftClient
        """.trimIndent()
        val fix = RemoveDuplicateAccessWidenerEntryFix(
            title = "Remove duplicate",
            documentUri = "file:///mod.accesswidener",
            lineNumber = 3,
        )
        val edit = service.applyRemoveDuplicateFix(fix, source)
        assertEquals(1, edit.edits.single().newText.lineSequence().count { it.contains("accessible class") })
    }

    @Test
    fun generateEntryFixUsesEditorFormatting() {
        val entry = AccessWidenerEntry(
            AccessWidenerDirective.ACCESSIBLE,
            AccessWidenerKind.CLASS,
            "net/minecraft/client/MinecraftClient",
            line = 2,
        )
        val fix = service.generateEntryFix("file:///mod.accesswidener", entry, insertLine = 2)
        assertEquals("accessible class net/minecraft/client/MinecraftClient", fix.entry)
    }

    @Test
    fun applyGenerateEntryFixInsertsLine() {
        val fix = GenerateAccessWidenerEntryFix(
            title = "Generate entry",
            documentUri = "file:///mod.accesswidener",
            insertLine = 1,
            entry = "accessible class net/minecraft/client/MinecraftClient",
        )
        val edit = service.applyGenerateEntryFix(fix, "accessWidener v2 named\n")
        assertTrue(edit.edits.single().newText.contains("accessible class net/minecraft/client/MinecraftClient"))
    }

    @Test
    fun offersFixDescriptorForMismatch() {
        val source = """
        accessWidener v2 named
        accessible method net/minecraft/client/MinecraftClient setScreen ()V
        """.trimIndent()
        val diagnostics = diagnostics(source)
        val fixes = service.fixesForDiagnostics(diagnostics, "file:///mod.accesswidener", source, classIndex)
        assertIs<FixAccessWidenerDescriptorFix>(fixes.first())
    }

    @Test
    fun applyDescriptorFixReplacesWrongMethodDescriptor() {
        val source = """
        accessWidener v2 named
        accessible method net/minecraft/client/MinecraftClient setScreen ()V
        """.trimIndent()
        val diagnostics = diagnostics(source)
        val fix = service.fixesForDiagnostics(diagnostics, "file:///mod.accesswidener", source, classIndex)
            .filterIsInstance<FixAccessWidenerDescriptorFix>()
            .first()
        val edit = assertNotNull(service.applyDescriptorFix(fix, source))
        assertTrue(edit.edits.single().newText.contains("(Lnet/minecraft/client/gui/screen/Screen;)V"))
    }

    private fun diagnostics(
        source: String,
        mappingContext: io.github.mcdev.core.mapping.ProjectMappingContext? = null,
    ) = diagnosticsService.analyze(
        AccessWidenerDiagnosticRequest(
            source = source,
            documentUri = "file:///mod.accesswidener",
            mappingContext = mappingContext,
        ),
    )
}
