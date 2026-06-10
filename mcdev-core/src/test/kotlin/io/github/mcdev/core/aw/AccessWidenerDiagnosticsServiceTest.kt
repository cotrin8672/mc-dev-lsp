package io.github.mcdev.core.aw

import io.github.mcdev.core.diagnostics.McSeverity
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class AccessWidenerDiagnosticsServiceTest {
    private val classIndex = AwTestFixtures.classIndex
    private val service = AccessWidenerDiagnosticsService(classIndex)
    private val mappingContext = AwTestFixtures.mappingContext()

    @Test
    fun validFileProducesNoErrors() {
        val diagnostics = analyze(AwTestFixtures.VALID_AW)
        assertTrue(diagnostics.none { it.severity == McSeverity.ERROR })
    }

    @Test
    fun detectsInvalidDirective() {
        val source = """
        accessWidener v2 named
        bad class net/minecraft/client/MinecraftClient
        """.trimIndent()
        assertTrue(analyze(source).any { it.code == AwDiagnosticCodes.INVALID_DIRECTIVE })
    }

    @Test
    fun detectsInvalidKind() {
        val source = """
        accessWidener v2 named
        accessible bad net/minecraft/client/MinecraftClient
        """.trimIndent()
        assertTrue(analyze(source).any { it.code == AwDiagnosticCodes.INVALID_KIND })
    }

    @Test
    fun detectsUnresolvedClass() {
        val source = """
        accessWidener v2 named
        accessible class missing/Class
        """.trimIndent()
        assertTrue(analyze(source).any { it.code == AwDiagnosticCodes.UNRESOLVED_CLASS })
    }

    @Test
    fun detectsUnresolvedMember() {
        val source = """
        accessWidener v2 named
        accessible method net/minecraft/client/MinecraftClient missingMethod ()V
        """.trimIndent()
        assertTrue(analyze(source).any { it.code == AwDiagnosticCodes.UNRESOLVED_MEMBER })
    }

    @Test
    fun detectsInvalidDescriptor() {
        val source = """
        accessWidener v2 named
        accessible method net/minecraft/client/MinecraftClient setScreen (Lbad
        """.trimIndent()
        assertTrue(analyze(source).any { it.code == AwDiagnosticCodes.INVALID_DESCRIPTOR })
    }

    @Test
    fun detectsDescriptorMismatch() {
        val source = """
        accessWidener v2 named
        accessible method net/minecraft/client/MinecraftClient setScreen ()V
        """.trimIndent()
        assertTrue(analyze(source).any { it.code == AwDiagnosticCodes.INVALID_DESCRIPTOR })
    }

    @Test
    fun detectsDuplicateEntry() {
        val source = """
        accessWidener v2 named
        accessible class net/minecraft/client/MinecraftClient
        accessible class net/minecraft/client/MinecraftClient
        """.trimIndent()
        assertTrue(analyze(source).any { it.code == AwDiagnosticCodes.DUPLICATE_ENTRY })
    }

    @Test
    fun detectsMutableOnNonField() {
        val source = """
        accessWidener v2 named
        mutable class net/minecraft/client/MinecraftClient
        """.trimIndent()
        assertTrue(analyze(source).any { it.code == AwDiagnosticCodes.MUTABLE_ON_NON_FIELD })
    }

    @Test
    fun detectsExtendableOnNonClass() {
        val source = """
        accessWidener v2 named
        extendable method net/minecraft/client/MinecraftClient setScreen (Lnet/minecraft/client/gui/screen/Screen;)V
        """.trimIndent()
        assertTrue(analyze(source).any { it.code == AwDiagnosticCodes.EXTENDABLE_INVALID_TARGET })
    }

    @Test
    fun detectsNamespaceMismatch() {
        val source = """
        accessWidener v2 named
        accessible class net/minecraft/client/class_310
        """.trimIndent()
        assertTrue(analyze(source, mappingContext).any { it.code == AwDiagnosticCodes.NAMESPACE_MISMATCH })
    }

    @Test
    fun duplicateDiagnosticsIncludeLineMetadata() {
        val source = """
        accessWidener v2 named
        accessible class net/minecraft/client/MinecraftClient
        accessible class net/minecraft/client/MinecraftClient
        """.trimIndent()
        val diagnostic = analyze(source).first { it.code == AwDiagnosticCodes.DUPLICATE_ENTRY }
        assertEquals("2", diagnostic.metadata["line"])
    }

    @Test
    fun parseFailureUsesInvalidDirectiveCodeForBadDirectiveLine() {
        val source = """
        accessWidener v2 named
        accessible method net/minecraft/client/MinecraftClient setScreen (Lbad
        """.trimIndent()
        val diagnostic = analyze(source).first { it.code == AwDiagnosticCodes.INVALID_DESCRIPTOR }
        assertEquals(McSeverity.ERROR, diagnostic.severity)
    }

    private fun analyze(source: String, mappingContext: io.github.mcdev.core.mapping.ProjectMappingContext? = null) =
        service.analyze(
            AccessWidenerDiagnosticRequest(
                source = source,
                documentUri = "file:///mod.accesswidener",
                mappingContext = mappingContext,
            ),
        )
}
