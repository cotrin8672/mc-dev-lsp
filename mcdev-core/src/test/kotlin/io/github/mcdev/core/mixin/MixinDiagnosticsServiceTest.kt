package io.github.mcdev.core.mixin

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class MixinDiagnosticsServiceTest {
    private val classIndex = FakeClassIndex()
    private val bytecodeIndex = FakeBytecodeIndex()
    private val service = MixinDiagnosticsService(classIndex, bytecodeIndex)

    @Test
    fun reportsUnresolvedMixinClassTarget() {
        val source = """@Mixin(UnknownClass.class) class M {}"""
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.UNRESOLVED_MIXIN_TARGET })
    }

    @Test
    fun reportsUnresolvedMixinStringTarget() {
        val source = """@Mixin(targets = "com.unknown.Missing") class M {}"""
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.UNRESOLVED_MIXIN_TARGET })
    }

    @Test
    fun noUnresolvedForValidClassTarget() {
        val source = """@Mixin(MinecraftClient.class) class M {}"""
        val diagnostics = analyze(source)
        assertTrue(diagnostics.none { it.code == MixinDiagnosticCodes.UNRESOLVED_MIXIN_TARGET })
    }

    @Test
    fun reportsMissingMixinConfigEntry() {
        val source = """@Mixin(MinecraftClient.class) class ExampleMixin {}"""
        val config = """{ "package": "example.mixin", "mixins": [] }"""
        val diagnostics = service.analyze(
            MixinDiagnosticRequest(
                source = source,
                documentUri = "file:///Mixin.java",
                mixinClassName = "ExampleMixin",
                mixinPackage = "example.mixin",
                mixinConfigContent = config,
                mixinConfigPath = "mixins.json",
            ),
        )
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.MIXIN_CLASS_NOT_LISTED_IN_CONFIG })
    }

    @Test
    fun reportsUnresolvedInjectMethod() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(method = "missing", at = @At("HEAD"))
                void m() {}
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.UNRESOLVED_INJECT_METHOD })
    }

    @Test
    fun reportsAmbiguousInjectMethod() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(method = "render", at = @At("HEAD"))
                void m() {}
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.AMBIGUOUS_INJECT_METHOD })
    }

    @Test
    fun reportsDescriptorMismatchForInjectMethod() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(method = "tick(I)V", at = @At("HEAD"))
                void m() {}
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.DESCRIPTOR_MISMATCH })
    }

    @Test
    fun reportsUnresolvedAtTarget() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(method = "tick", at = @At(value = "INVOKE", target = "Lmissing/Class;draw()V"))
                void m() {}
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.UNRESOLVED_AT_TARGET })
    }

    @Test
    fun reportsInvalidAtTargetDescriptor() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(method = "tick", at = @At(value = "INVOKE", target = "invalid"))
                void m() {}
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.INVALID_AT_TARGET_DESCRIPTOR })
    }

    @Test
    fun reportsOrdinalOutOfRange() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(method = "tick", at = @At(value = "RETURN", ordinal = 5))
                void m() {}
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.ORDINAL_OUT_OF_RANGE })
    }

    @Test
    fun noDiagnosticForValidAtTarget() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(method = "tick", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/font/TextRenderer;draw(Ljava/lang/String;FFI)I"))
                void m() {}
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.none { it.code == MixinDiagnosticCodes.UNRESOLVED_AT_TARGET })
    }

    private fun analyze(source: String) = service.analyze(
        MixinDiagnosticRequest(
            source = source,
            documentUri = "file:///Mixin.java",
            mixinClassName = null,
            mixinPackage = null,
            mixinConfigContent = null,
            mixinConfigPath = null,
        ),
    )
}
