package io.github.mcdev.core.at

import io.github.mcdev.core.codeaction.AddAccessTransformerEntryFix
import io.github.mcdev.core.codeaction.AddAtMethodDescriptorFix
import io.github.mcdev.core.codeaction.RemapAccessTransformerEntryFix
import io.github.mcdev.core.codeaction.RemoveDuplicateAtEntryFix
import io.github.mcdev.core.mapping.ClassMapping
import io.github.mcdev.core.mapping.MappingSet
import io.github.mcdev.core.mapping.MethodMapping
import io.github.mcdev.core.mapping.ProjectMappingContext
import io.github.mcdev.core.mapping.asResolver
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FakeClassIndex
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.model.MappingNamespace
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
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
    fun remapActionIsNotOfferedForOverloadedMethodWithoutDescriptor() {
        val source = "public com.example.target.OverloadedTarget setValue"
        val classIndex = overloadedClassIndex()
        val service = AccessTransformerCodeActionService(classIndex, overloadedMappingContext())
        val diagnostics = AccessTransformerDiagnosticsService(classIndex, overloadedMappingContext())
            .analyze(AtDiagnosticRequest(source = source))

        val fixes = service.fixesForDiagnostics(diagnostics, "file:///at.cfg", source)

        assertFalse(fixes.any { it is RemapAccessTransformerEntryFix })
    }

    @Test
    fun remapActionDoesNotUseFirstOverloadWhenDescriptorMismatches() {
        val source = "public com.example.target.OverloadedTarget setValue(Z)V"
        val service = AccessTransformerCodeActionService(overloadedClassIndex(), overloadedMappingContext())
        val fix = RemapAccessTransformerEntryFix(
            title = "Remap AT entry namespace",
            documentUri = "file:///at.cfg",
            line = 1,
        )

        val edit = service.applyRemapFix(fix, source)

        assertEquals(null, edit)
    }

    @Test
    fun remapActionUsesExactDescriptorWhenDescriptorIsPresent() {
        val source = "public com.example.target.OverloadedTarget setValue(Ljava/lang/String;)V"
        val service = AccessTransformerCodeActionService(overloadedClassIndex(), overloadedMappingContext())
        val fix = RemapAccessTransformerEntryFix(
            title = "Remap AT entry namespace",
            documentUri = "file:///at.cfg",
            line = 1,
        )

        val edit = service.applyRemapFix(fix, source)

        assertNotNull(edit)
        assertTrue(edit!!.edits.single().newText.contains("m_string_(Ljava/lang/String;)V"))
    }

    @Test
    fun remapActionUsesSingleCandidateOnlyWhenDescriptorIsOmitted() {
        val source = "public com.example.target.SingleTarget tick"
        val service = AccessTransformerCodeActionService(singleMethodClassIndex(), singleMethodMappingContext())
        val fix = RemapAccessTransformerEntryFix(
            title = "Remap AT entry namespace",
            documentUri = "file:///at.cfg",
            line = 1,
        )

        val edit = service.applyRemapFix(fix, source)

        assertNotNull(edit)
        assertTrue(edit!!.edits.single().newText.contains("m_tick_()V"))
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

    private fun overloadedClassIndex(): FakeClassIndex {
        val owner = ClassIndexEntry(
            simpleName = "OverloadedTarget",
            packageName = "com.example.target",
            internalName = "com/example/target/OverloadedTarget",
        )
        return FakeClassIndex(
            classes = listOf(owner),
            methods = mapOf(
                owner.internalName to listOf(
                    MethodIndexEntry("setValue", "(I)V", false, "setValue(int): void"),
                    MethodIndexEntry("setValue", "(Ljava/lang/String;)V", false, "setValue(String): void"),
                ),
            ),
        )
    }

    private fun overloadedMappingContext(): ProjectMappingContext {
        val owner = ClassMapping(
            mapOf(
                MappingNamespace.NAMED to "com/example/target/OverloadedTarget",
                MappingNamespace.SRG to "com/example/target/OverloadedTarget",
            ),
        )
        return ProjectMappingContext(
            sourceNamespace = MappingNamespace.NAMED,
            runtimeNamespace = MappingNamespace.SRG,
            awNamespace = MappingNamespace.NAMED,
            atNamespace = MappingNamespace.SRG,
            availableNamespaces = setOf(MappingNamespace.NAMED, MappingNamespace.SRG),
            resolver = MappingSet(
                namespaces = listOf(MappingNamespace.NAMED, MappingNamespace.SRG),
                classes = listOf(owner),
                methods = listOf(
                    MethodMapping(
                        owner = owner,
                        descriptor = "(I)V",
                        names = mapOf(
                            MappingNamespace.NAMED to "setValue",
                            MappingNamespace.SRG to "m_int_",
                        ),
                    ),
                    MethodMapping(
                        owner = owner,
                        descriptor = "(Ljava/lang/String;)V",
                        names = mapOf(
                            MappingNamespace.NAMED to "setValue",
                            MappingNamespace.SRG to "m_string_",
                        ),
                    ),
                ),
                fields = emptyList(),
            ).asResolver(),
        )
    }

    private fun singleMethodClassIndex(): FakeClassIndex {
        val owner = ClassIndexEntry(
            simpleName = "SingleTarget",
            packageName = "com.example.target",
            internalName = "com/example/target/SingleTarget",
        )
        return FakeClassIndex(
            classes = listOf(owner),
            methods = mapOf(
                owner.internalName to listOf(
                    MethodIndexEntry("tick", "()V", false, "tick(): void"),
                ),
            ),
        )
    }

    private fun singleMethodMappingContext(): ProjectMappingContext {
        val owner = ClassMapping(
            mapOf(
                MappingNamespace.NAMED to "com/example/target/SingleTarget",
                MappingNamespace.SRG to "com/example/target/SingleTarget",
            ),
        )
        return ProjectMappingContext(
            sourceNamespace = MappingNamespace.NAMED,
            runtimeNamespace = MappingNamespace.SRG,
            awNamespace = MappingNamespace.NAMED,
            atNamespace = MappingNamespace.SRG,
            availableNamespaces = setOf(MappingNamespace.NAMED, MappingNamespace.SRG),
            resolver = MappingSet(
                namespaces = listOf(MappingNamespace.NAMED, MappingNamespace.SRG),
                classes = listOf(owner),
                methods = listOf(
                    MethodMapping(
                        owner = owner,
                        descriptor = "()V",
                        names = mapOf(
                            MappingNamespace.NAMED to "tick",
                            MappingNamespace.SRG to "m_tick_",
                        ),
                    ),
                ),
                fields = emptyList(),
            ).asResolver(),
        )
    }
}
