package io.github.mcdev.core.at

import kotlin.test.Test
import kotlin.test.assertTrue

class AccessTransformerDiagnosticsServiceTest {
    @Test
    fun reportsInvalidModifier() {
        val diagnostics = analyze("wider net.minecraft.client.MinecraftClient")
        assertTrue(diagnostics.any { it.code == AtDiagnosticCodes.INVALID_MODIFIER })
    }

    @Test
    fun reportsUnresolvedClass() {
        val diagnostics = analyze("public com.unknown.Missing")
        assertTrue(diagnostics.any { it.code == AtDiagnosticCodes.UNRESOLVED_CLASS })
    }

    @Test
    fun noUnresolvedForValidClassOnlyEntry() {
        val diagnostics = analyze("public net.minecraft.client.MinecraftClient")
        assertTrue(diagnostics.none { it.code == AtDiagnosticCodes.UNRESOLVED_CLASS })
    }

    @Test
    fun reportsUnresolvedMember() {
        val diagnostics = analyze("public net.minecraft.client.MinecraftClient missingField")
        assertTrue(diagnostics.any { it.code == AtDiagnosticCodes.UNRESOLVED_MEMBER })
    }

    @Test
    fun reportsMissingMethodDescriptor() {
        val diagnostics = analyze("public net.minecraft.client.MinecraftClient setScreen")
        assertTrue(diagnostics.any { it.code == AtDiagnosticCodes.MISSING_METHOD_DESCRIPTOR })
    }

    @Test
    fun noMissingDescriptorForFieldEntry() {
        val diagnostics = analyze("public-f net.minecraft.client.MinecraftClient currentScreen")
        assertTrue(diagnostics.none { it.code == AtDiagnosticCodes.MISSING_METHOD_DESCRIPTOR })
    }

    @Test
    fun reportsInvalidDescriptor() {
        val diagnostics = analyze("public net.minecraft.client.MinecraftClient setScreen(bad)")
        assertTrue(diagnostics.any { it.code == AtDiagnosticCodes.INVALID_DESCRIPTOR })
    }

    @Test
    fun reportsDuplicateEntry() {
        val source = """
            public net.minecraft.client.MinecraftClient
            public net.minecraft.client.MinecraftClient
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any { it.code == AtDiagnosticCodes.DUPLICATE_ENTRY })
    }

    @Test
    fun reportsWrongNamespaceForNamedMemberOnForge() {
        val diagnostics = analyze(
            "public net.minecraft.client.MinecraftClient setScreen(Lnet/minecraft/client/gui/screen/Screen;)V",
            AtTestFixtures.forgeMappingContext,
        )
        assertTrue(diagnostics.any { it.code == AtDiagnosticCodes.WRONG_NAMESPACE })
    }

    @Test
    fun acceptsSrgMemberOnForge() {
        val diagnostics = analyze(
            "public net.minecraft.client.MinecraftClient m_91152_(Lnet/minecraft/client/gui/screen/Screen;)V",
            AtTestFixtures.forgeMappingContext,
        )
        assertTrue(diagnostics.none { it.code == AtDiagnosticCodes.WRONG_NAMESPACE })
        assertTrue(diagnostics.none { it.code == AtDiagnosticCodes.UNRESOLVED_MEMBER })
    }

    @Test
    fun reportsSrgMappingNotFoundWhenMethodMappingMissing() {
        val diagnostics = analyze(
            "public net.minecraft.client.MinecraftClient setScreen(Lnet/minecraft/client/gui/screen/Screen;)V",
            AtTestFixtures.incompleteSrgMappingContext(),
        )
        assertTrue(diagnostics.any { it.code == AtDiagnosticCodes.SRG_MAPPING_NOT_FOUND })
    }

    @Test
    fun acceptsNamedMemberOnFabric() {
        val diagnostics = analyze(
            "public net.minecraft.client.MinecraftClient setScreen(Lnet/minecraft/client/gui/screen/Screen;)V",
            AtTestFixtures.fabricMappingContext,
        )
        assertTrue(diagnostics.none { it.code == AtDiagnosticCodes.WRONG_NAMESPACE })
        assertTrue(diagnostics.none { it.code == AtDiagnosticCodes.UNRESOLVED_MEMBER })
    }

    @Test
    fun reportsInvalidDescriptorMismatch() {
        val diagnostics = analyze(
            "public net.minecraft.client.MinecraftClient setScreen()V",
            AtTestFixtures.fabricMappingContext,
        )
        assertTrue(diagnostics.any { it.code == AtDiagnosticCodes.INVALID_DESCRIPTOR })
    }

    private fun analyze(
        source: String,
        mappingContext: io.github.mcdev.core.mapping.ProjectMappingContext = AtTestFixtures.fabricMappingContext,
    ) = AccessTransformerDiagnosticsService(AtTestFixtures.classIndex, mappingContext).analyze(
        AtDiagnosticRequest(source = source),
    )
}
