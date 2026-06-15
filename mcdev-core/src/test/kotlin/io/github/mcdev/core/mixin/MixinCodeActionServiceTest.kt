package io.github.mcdev.core.mixin

import io.github.mcdev.core.codeaction.AddMixinConfigEntryFix
import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.core.diagnostics.McSeverity
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class MixinCodeActionServiceTest {
    private val configEditor = MixinConfigEditor()
    private val service = MixinCodeActionService(
        configEditor = configEditor,
        accessorService = AccessorService(FakeClassIndex()),
        invokerService = InvokerService(FakeClassIndex()),
    )

    @Test
    fun producesAddMixinConfigFix() {
        val diagnostic = McDiagnostic(
            code = MixinDiagnosticCodes.MIXIN_CLASS_NOT_LISTED_IN_CONFIG,
            severity = McSeverity.WARNING,
            message = "missing",
            range = McTextRange(McTextPosition(0, 0), McTextPosition(0, 10)),
            metadata = mapOf("mixinClass" to "ExampleMixin"),
        )
        val fixes = service.fixesForDiagnostics(
            listOf(diagnostic),
            documentUri = "file:///Mixin.java",
            source = "@Mixin(MinecraftClient.class) class ExampleMixin {}",
            mixinConfigContent = """{ "mixins": [] }""",
            mixinConfigPath = "mixins.json",
            mixinPackage = "example.mixin",
        )
        assertEquals(1, fixes.size)
        val fix = fixes.first() as AddMixinConfigEntryFix
        assertEquals("ExampleMixin", fix.mixinClassName)
        assertEquals("mixins.json", fix.configPath)
    }

    @Test
    fun applyMixinConfigFixAddsEntryDeterministically() {
        val fix = AddMixinConfigEntryFix(
            title = "Add mixin",
            configPath = "mixins.json",
            mixinClassName = "ExampleMixin",
            mixinPackage = "example.mixin",
        )
        val content = """{ "mixins": ["AlphaMixin"] }"""
        val edit = service.applyMixinConfigFix(fix, content)
        assertNotNull(edit)
        assertTrue(edit.edits.first().newText.contains("ExampleMixin"))
        assertTrue(edit.edits.first().newText.indexOf("AlphaMixin") < edit.edits.first().newText.indexOf("ExampleMixin"))
    }

    @Test
    fun applyMixinConfigFixSkipsDuplicate() {
        val fix = AddMixinConfigEntryFix(
            title = "Add mixin",
            configPath = "mixins.json",
            mixinClassName = "ExampleMixin",
            mixinPackage = null,
        )
        val content = """{ "mixins": ["ExampleMixin"] }"""
        assertNull(service.applyMixinConfigFix(fix, content))
    }

    @Test
    fun doesNotGuessDescriptorForAmbiguousMethod() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(method = "render", at = @At("HEAD"))
                void m() {}
            }
        """.trimIndent()
        val diagnostic = McDiagnostic(
            code = MixinDiagnosticCodes.AMBIGUOUS_INJECT_METHOD,
            severity = McSeverity.WARNING,
            message = "ambiguous",
            range = McTextRange(McTextPosition(2, 20), McTextPosition(2, 28)),
            metadata = mapOf("method" to "render"),
        )
        val fixes = service.fixesForDiagnostics(
            listOf(diagnostic),
            documentUri = "file:///Mixin.java",
            source = source,
            mixinConfigContent = null,
            mixinConfigPath = null,
            mixinPackage = null,
            classIndex = FakeClassIndex(),
        )
        assertTrue(fixes.isEmpty())
    }

    @Test
    fun producesGenerateAccessorFix() {
        val diagnostic = McDiagnostic(
            code = MixinDiagnosticCodes.ACCESSOR_FIELD_NOT_FOUND,
            severity = McSeverity.ERROR,
            message = "missing",
            range = McTextRange(McTextPosition(0, 0), McTextPosition(0, 0)),
            metadata = mapOf("field" to "currentScreen"),
        )
        val source = "@Mixin(MinecraftClient.class) class M { }"
        val fixes = service.fixesForDiagnostics(
            listOf(diagnostic),
            documentUri = "file:///Mixin.java",
            source = source,
            mixinConfigContent = null,
            mixinConfigPath = null,
            mixinPackage = null,
            classIndex = FakeClassIndex(),
        )
        assertTrue(fixes.any { it.title.contains("Accessor") })
    }

    @Test
    fun producesGenerateInvokerFix() {
        val diagnostic = McDiagnostic(
            code = MixinDiagnosticCodes.INVOKER_METHOD_NOT_FOUND,
            severity = McSeverity.ERROR,
            message = "missing",
            range = McTextRange(McTextPosition(0, 0), McTextPosition(0, 0)),
            metadata = mapOf("name" to "setScreen"),
        )
        val source = "@Mixin(MinecraftClient.class) class M { }"
        val fixes = service.fixesForDiagnostics(
            listOf(diagnostic),
            documentUri = "file:///Mixin.java",
            source = source,
            mixinConfigContent = null,
            mixinConfigPath = null,
            mixinPackage = null,
            classIndex = FakeClassIndex(),
        )
        assertTrue(fixes.any { it.title.contains("Invoker") })
    }

    @Test
    fun doesNotOfferQuickfixForUnresolvedHandlerDescriptor() {
        val diagnostic = McDiagnostic(
            code = MixinDiagnosticCodes.UNRESOLVED_HANDLER_DESCRIPTOR,
            severity = McSeverity.ERROR,
            message = "ambiguous type",
            range = McTextRange(McTextPosition(0, 0), McTextPosition(0, 0)),
            metadata = mapOf("type" to "Widget"),
        )

        val fixes = service.fixesForDiagnostics(
            listOf(diagnostic),
            documentUri = "file:///Mixin.java",
            source = "@Mixin(MinecraftClient.class) class M { }",
            mixinConfigContent = null,
            mixinConfigPath = null,
            mixinPackage = null,
            classIndex = FakeClassIndex(),
        )

        assertTrue(fixes.isEmpty())
    }
}
