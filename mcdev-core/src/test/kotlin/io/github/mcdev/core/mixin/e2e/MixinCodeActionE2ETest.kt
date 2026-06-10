package io.github.mcdev.core.mixin.e2e

import io.github.mcdev.core.codeaction.AddMethodDescriptorFix
import io.github.mcdev.core.codeaction.AddMixinConfigEntryFix
import io.github.mcdev.core.codeaction.GenerateAccessorMethodFix
import io.github.mcdev.core.codeaction.GenerateInvokerMethodFix
import io.github.mcdev.core.mixin.MixinCodeActionService
import io.github.mcdev.core.mixin.MixinDiagnosticCodes
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

class MixinCodeActionE2ETest {
    private val facade = MixinE2ETestSupport.fakeFacade()
    private val codeActionService = MixinCodeActionService()

    @Test
    fun producesAddMixinConfigEntryFix() {
        val source = """@Mixin(MinecraftClient.class) class ExampleMixin {}"""
        val request = MixinE2ETestSupport.requestAt(source, "ExampleMixin").copy(
            mixinClassName = "ExampleMixin",
            mixinPackage = "example.mixin",
            mixinConfigContent = """{ "mixins": [] }""",
            mixinConfigPath = "mixins.json",
        )
        val fixes = facade.codeActions(request, MixinDiagnosticCodes.MIXIN_CLASS_NOT_LISTED_IN_CONFIG)
        assertEquals(1, fixes.size)
        val fix = fixes.first() as AddMixinConfigEntryFix
        assertEquals("ExampleMixin", fix.mixinClassName)
        assertEquals("mixins.json", fix.configPath)
    }

    @Test
    fun applyMixinConfigFixAddsEntry() {
        val source = """@Mixin(MinecraftClient.class) class ExampleMixin {}"""
        val request = MixinE2ETestSupport.requestAt(source, "ExampleMixin").copy(
            mixinClassName = "ExampleMixin",
            mixinConfigContent = """{ "mixins": ["AlphaMixin"] }""",
            mixinConfigPath = "mixins.json",
        )
        val fix = facade.codeActions(request, MixinDiagnosticCodes.MIXIN_CLASS_NOT_LISTED_IN_CONFIG)
            .first() as AddMixinConfigEntryFix
        val edit = codeActionService.applyMixinConfigFix(fix, request.mixinConfigContent!!)
        assertNotNull(edit)
        assertTrue(edit.edits.first().newText.contains("ExampleMixin"))
    }

    @Test
    fun producesAddDescriptorFixForAmbiguousInjectMethod() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(method = "render", at = @At("HEAD"))
                void m() {}
            }
        """.trimIndent()
        val fixes = facade.codeActions(
            MixinE2ETestSupport.requestAt(source, "render"),
            MixinDiagnosticCodes.AMBIGUOUS_INJECT_METHOD,
        )
        assertEquals(1, fixes.size)
        val fix = fixes.first() as AddMethodDescriptorFix
        assertEquals("render", fix.methodName)
        assertTrue(fix.descriptor.startsWith("("))
    }

    @Test
    fun applyMethodDescriptorFixUpdatesSource() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(method = "render", at = @At("HEAD"))
                void m() {}
            }
        """.trimIndent()
        val fixes = facade.codeActions(
            MixinE2ETestSupport.requestAt(source, "render"),
            MixinDiagnosticCodes.AMBIGUOUS_INJECT_METHOD,
        )
        val fix = fixes.first() as AddMethodDescriptorFix
        val edit = codeActionService.applyMethodDescriptorFix(fix, source)
        assertTrue(edit.edits.first().newText.contains("render("))
    }

    @Test
    fun producesGenerateAccessorFix() {
        val source = """@Mixin(MinecraftClient.class) class M { }"""
        val diagnostics = facade.diagnose(MixinE2ETestSupport.requestAt(source, "M"))
        val diagnostic = diagnostics.firstOrNull { it.code == MixinDiagnosticCodes.ACCESSOR_FIELD_NOT_FOUND }
            ?: io.github.mcdev.core.diagnostics.McDiagnostic(
                code = MixinDiagnosticCodes.ACCESSOR_FIELD_NOT_FOUND,
                severity = io.github.mcdev.core.diagnostics.McSeverity.ERROR,
                message = "missing",
                range = io.github.mcdev.core.diagnostics.McTextRange(
                    io.github.mcdev.core.diagnostics.McTextPosition(0, 0),
                    io.github.mcdev.core.diagnostics.McTextPosition(0, 0),
                ),
                metadata = mapOf("field" to "currentScreen"),
            )
        val fixes = io.github.mcdev.core.mixin.MixinCodeActionService(
            accessorService = io.github.mcdev.core.mixin.AccessorService(io.github.mcdev.core.mixin.FakeClassIndex()),
            invokerService = io.github.mcdev.core.mixin.InvokerService(io.github.mcdev.core.mixin.FakeClassIndex()),
        ).fixesForDiagnostics(
            listOf(diagnostic),
            documentUri = "file:///Mixin.java",
            source = source,
            mixinConfigContent = null,
            mixinConfigPath = null,
            mixinPackage = null,
            classIndex = io.github.mcdev.core.mixin.FakeClassIndex(),
        )
        assertTrue(fixes.any { it is GenerateAccessorMethodFix })
    }

    @Test
    fun producesGenerateInvokerFix() {
        val source = """@Mixin(MinecraftClient.class) class M { }"""
        val diagnostic = io.github.mcdev.core.diagnostics.McDiagnostic(
            code = MixinDiagnosticCodes.INVOKER_METHOD_NOT_FOUND,
            severity = io.github.mcdev.core.diagnostics.McSeverity.ERROR,
            message = "missing",
            range = io.github.mcdev.core.diagnostics.McTextRange(
                io.github.mcdev.core.diagnostics.McTextPosition(0, 0),
                io.github.mcdev.core.diagnostics.McTextPosition(0, 0),
            ),
            metadata = mapOf("name" to "setScreen"),
        )
        val fixes = io.github.mcdev.core.mixin.MixinCodeActionService(
            accessorService = io.github.mcdev.core.mixin.AccessorService(io.github.mcdev.core.mixin.FakeClassIndex()),
            invokerService = io.github.mcdev.core.mixin.InvokerService(io.github.mcdev.core.mixin.FakeClassIndex()),
        ).fixesForDiagnostics(
            listOf(diagnostic),
            documentUri = "file:///Mixin.java",
            source = source,
            mixinConfigContent = null,
            mixinConfigPath = null,
            mixinPackage = null,
            classIndex = io.github.mcdev.core.mixin.FakeClassIndex(),
        )
        assertTrue(fixes.any { it is GenerateInvokerMethodFix })
    }

    @Test
    fun generateAccessorFixContainsFieldName() {
        val source = """@Mixin(MinecraftClient.class) class M { }"""
        val diagnostic = io.github.mcdev.core.diagnostics.McDiagnostic(
            code = MixinDiagnosticCodes.ACCESSOR_FIELD_NOT_FOUND,
            severity = io.github.mcdev.core.diagnostics.McSeverity.ERROR,
            message = "missing",
            range = io.github.mcdev.core.diagnostics.McTextRange(
                io.github.mcdev.core.diagnostics.McTextPosition(0, 0),
                io.github.mcdev.core.diagnostics.McTextPosition(0, 0),
            ),
            metadata = mapOf("field" to "currentScreen"),
        )
        val fixes = io.github.mcdev.core.mixin.MixinCodeActionService(
            accessorService = io.github.mcdev.core.mixin.AccessorService(io.github.mcdev.core.mixin.FakeClassIndex()),
        ).fixesForDiagnostics(
            listOf(diagnostic),
            documentUri = "file:///Mixin.java",
            source = source,
            mixinConfigContent = null,
            mixinConfigPath = null,
            mixinPackage = null,
            classIndex = io.github.mcdev.core.mixin.FakeClassIndex(),
        )
        val fix = fixes.filterIsInstance<GenerateAccessorMethodFix>().first()
        assertTrue(fix.methodSource.contains("currentScreen"))
    }

    @Test
    fun codeActionsWithoutFilterReturnsAllApplicableFixes() {
        val source = """@Mixin(MinecraftClient.class) class ExampleMixin {}"""
        val request = MixinE2ETestSupport.requestAt(source, "ExampleMixin").copy(
            mixinClassName = "ExampleMixin",
            mixinConfigContent = """{ "mixins": [] }""",
            mixinConfigPath = "mixins.json",
        )
        val fixes = facade.codeActions(request)
        assertTrue(fixes.isNotEmpty())
    }

    @Test
    fun filteredCodeActionExcludesOtherDiagnosticFixes() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(method = "render", at = @At("HEAD"))
                void m() {}
            }
        """.trimIndent()
        val fixes = facade.codeActions(
            MixinE2ETestSupport.requestAt(source, "render"),
            MixinDiagnosticCodes.MIXIN_CLASS_NOT_LISTED_IN_CONFIG,
        )
        assertTrue(fixes.isEmpty())
    }

}
