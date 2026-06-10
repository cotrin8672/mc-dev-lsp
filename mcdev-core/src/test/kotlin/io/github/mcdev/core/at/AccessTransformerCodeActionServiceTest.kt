package io.github.mcdev.core.at

import io.github.mcdev.core.codeaction.AddAccessTransformerEntryFix
import io.github.mcdev.core.codeaction.AddAtMethodDescriptorFix
import io.github.mcdev.core.codeaction.RemapAccessTransformerEntryFix
import io.github.mcdev.core.codeaction.RemoveDuplicateAtEntryFix
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

class AccessTransformerCodeActionServiceTest {
    private val diagnosticsService = AccessTransformerDiagnosticsService(AtTestFixtures.classIndex)

    @Test
    fun offersAddDescriptorFixForMissingDescriptor() {
        val source = "public net.minecraft.client.MinecraftClient setScreen"
        val service = AccessTransformerCodeActionService(AtTestFixtures.classIndex)
        val diagnostics = diagnosticsService.analyze(AtDiagnosticRequest(source = source))
        val fixes = service.fixesForDiagnostics(diagnostics, "file:///at.cfg", source)
        assertTrue(fixes.any { it is AddAtMethodDescriptorFix })
    }

    @Test
    fun appliesAddDescriptorFix() {
        val source = "public net.minecraft.client.MinecraftClient setScreen"
        val service = AccessTransformerCodeActionService(AtTestFixtures.classIndex)
        val fix = AddAtMethodDescriptorFix(
            title = "Add method descriptor",
            documentUri = "file:///at.cfg",
            line = 1,
            memberName = "setScreen",
            descriptor = "(Lnet/minecraft/client/gui/screen/Screen;)V",
        )
        val edit = service.applyMethodDescriptorFix(fix, source)
        assertNotNull(edit)
        assertTrue(edit!!.edits.single().newText.contains("setScreen(Lnet/minecraft/client/gui/screen/Screen;)V"))
    }

    @Test
    fun offersRemapFixForWrongNamespace() {
        val source = "public net.minecraft.client.MinecraftClient setScreen(Lnet/minecraft/client/gui/screen/Screen;)V"
        val service = AccessTransformerCodeActionService(AtTestFixtures.classIndex, AtTestFixtures.forgeMappingContext)
        val diagnostics = AccessTransformerDiagnosticsService(AtTestFixtures.classIndex, AtTestFixtures.forgeMappingContext)
            .analyze(AtDiagnosticRequest(source = source))
        val fixes = service.fixesForDiagnostics(diagnostics, "file:///at.cfg", source)
        assertTrue(fixes.any { it is RemapAccessTransformerEntryFix })
    }

    @Test
    fun appliesRemapFixToSrgNames() {
        val source = "public net.minecraft.client.MinecraftClient setScreen(Lnet/minecraft/client/gui/screen/Screen;)V"
        val service = AccessTransformerCodeActionService(AtTestFixtures.classIndex, AtTestFixtures.forgeMappingContext)
        val fix = RemapAccessTransformerEntryFix(
            title = "Remap AT entry namespace",
            documentUri = "file:///at.cfg",
            line = 1,
        )
        val edit = service.applyRemapFix(fix, source)
        assertNotNull(edit)
        assertTrue(edit!!.edits.single().newText.contains("m_91152_"))
    }

    @Test
    fun offersRemoveDuplicateFix() {
        val source = """
            public net.minecraft.client.MinecraftClient
            public net.minecraft.client.MinecraftClient
        """.trimIndent()
        val service = AccessTransformerCodeActionService(AtTestFixtures.classIndex)
        val diagnostics = diagnosticsService.analyze(AtDiagnosticRequest(source = source))
        val fixes = service.fixesForDiagnostics(diagnostics, "file:///at.cfg", source)
        assertTrue(fixes.any { it is RemoveDuplicateAtEntryFix })
    }

    @Test
    fun appliesRemoveDuplicateFix() {
        val source = """
            public net.minecraft.client.MinecraftClient
            public net.minecraft.client.MinecraftClient
        """.trimIndent()
        val service = AccessTransformerCodeActionService(AtTestFixtures.classIndex)
        val fix = RemoveDuplicateAtEntryFix(
            title = "Remove duplicate entry",
            documentUri = "file:///at.cfg",
            line = 2,
        )
        val edit = service.applyRemoveDuplicateFix(fix, source)
        assertNotNull(edit)
        assertEquals(1, edit!!.edits.single().newText.lines().count { it.isNotBlank() })
    }

    @Test
    fun generatesEntryFixFromContext() {
        val service = AccessTransformerCodeActionService(AtTestFixtures.classIndex)
        val context = AtContextExtractor.extract(
            "public net.minecraft.client.MinecraftClient",
            line = 0,
            character = 42,
        )
        assertNotNull(context)
        val fix = service.generateEntryFix(context, "file:///at.cfg")
        assertNotNull(fix)
        assertEquals("public", fix!!.modifier)
        assertEquals("net.minecraft.client.MinecraftClient", fix.owner)
    }

    @Test
    fun appliesGenerateEntryFix() {
        val service = AccessTransformerCodeActionService(AtTestFixtures.classIndex)
        val fix = AddAccessTransformerEntryFix(
            title = "Generate Access Transformer entry",
            documentUri = "file:///at.cfg",
            modifier = "public-f",
            owner = "net.minecraft.client.MinecraftClient",
            memberName = "currentScreen",
            memberDescriptor = null,
            insertLine = 1,
        )
        val edit = service.applyAddEntryFix(fix, "")
        assertNotNull(edit)
        assertEquals(
            "public-f net.minecraft.client.MinecraftClient currentScreen\n",
            edit!!.edits.single().newText,
        )
    }
}
