package io.github.mcdev.core.mixin.e2e

import io.github.mcdev.core.mixin.MixinDiagnosticCodes
import io.github.mcdev.fixtures.FixturePaths
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class MixinDiagnosticsE2ETest {
    private val fakeFacade = MixinE2ETestSupport.fakeFacade()
    private val simpleFacade = MixinE2ETestSupport.simpleTargetFacade()

    @Test
    fun reportsUnresolvedMixinTarget() {
        val source = """@Mixin(UnknownClass.class) class M {}"""
        val diagnostics = fakeFacade.diagnose(MixinE2ETestSupport.requestAt(source, "Unknown"))
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.UNRESOLVED_MIXIN_TARGET })
    }

    @Test
    fun noUnresolvedForValidMixinTarget() {
        val source = """@Mixin(MinecraftClient.class) class M {}"""
        val diagnostics = fakeFacade.diagnose(MixinE2ETestSupport.requestAt(source, "Minecraft"))
        assertTrue(diagnostics.none { it.code == MixinDiagnosticCodes.UNRESOLVED_MIXIN_TARGET })
    }

    @Test
    fun reportsMissingMixinConfigEntry() {
        val source = """@Mixin(MinecraftClient.class) class ExampleMixin {}"""
        val request = MixinE2ETestSupport.requestAt(source, "ExampleMixin").copy(
            mixinClassName = "ExampleMixin",
            mixinPackage = "example.mixin",
            mixinConfigContent = """{ "package": "example.mixin", "mixins": [] }""",
            mixinConfigPath = "mixins.json",
        )
        val diagnostics = fakeFacade.diagnose(request)
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
        val diagnostics = fakeFacade.diagnose(MixinE2ETestSupport.requestAt(source, "missing"))
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
        val diagnostics = fakeFacade.diagnose(MixinE2ETestSupport.requestAt(source, "render"))
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.AMBIGUOUS_INJECT_METHOD })
    }

    @Test
    fun reportsUnresolvedAtTargetFromBrokenFixture() {
        val source = MixinE2ETestSupport.loadFixtureText(FixturePaths.BROKEN_DIAGNOSTICS_MIXIN)
        val diagnostics = simpleFacade.diagnose(MixinE2ETestSupport.requestAt(source, "Missing"))
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.UNRESOLVED_AT_TARGET })
    }

    @Test
    fun reportsUnresolvedInjectMethodFromBrokenFixture() {
        val source = MixinE2ETestSupport.loadFixtureText(FixturePaths.BROKEN_DIAGNOSTICS_MIXIN)
        val diagnostics = simpleFacade.diagnose(MixinE2ETestSupport.requestAt(source, "missingMethod"))
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.UNRESOLVED_INJECT_METHOD })
    }

    @Test
    fun shadowFieldNotFoundDiagnostic() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Shadow private int missingField;
            }
        """.trimIndent()
        val diagnostics = fakeFacade.diagnose(MixinE2ETestSupport.requestAt(source, "missingField"))
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.SHADOW_TARGET_NOT_FOUND })
    }

    @Test
    fun shadowFieldDescriptorMismatch() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Shadow private int currentScreen;
            }
        """.trimIndent()
        val diagnostics = fakeFacade.diagnose(MixinE2ETestSupport.requestAt(source, "currentScreen"))
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.SHADOW_DESCRIPTOR_MISMATCH })
    }

    @Test
    fun shadowMethodValidationPasses() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Shadow private void tick();
            }
        """.trimIndent()
        val diagnostics = fakeFacade.diagnose(MixinE2ETestSupport.requestAt(source, "tick"))
        assertTrue(diagnostics.none { it.code == MixinDiagnosticCodes.SHADOW_TARGET_NOT_FOUND })
    }

    @Test
    fun accessorFieldNotFoundDiagnostic() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Accessor("missing")
                int getMissing();
            }
        """.trimIndent()
        val diagnostics = fakeFacade.diagnose(MixinE2ETestSupport.requestAt(source, "missing"))
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.ACCESSOR_FIELD_NOT_FOUND })
    }

    @Test
    fun accessorValidationPassesForValidField() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Accessor
                Screen getCurrentScreen();
            }
        """.trimIndent()
        val diagnostics = fakeFacade.diagnose(MixinE2ETestSupport.requestAt(source, "getCurrentScreen"))
        assertTrue(diagnostics.none { it.code == MixinDiagnosticCodes.ACCESSOR_FIELD_NOT_FOUND })
    }

    @Test
    fun invokerMethodNotFoundDiagnostic() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Invoker("missing")
                void invokeMissing();
            }
        """.trimIndent()
        val diagnostics = fakeFacade.diagnose(MixinE2ETestSupport.requestAt(source, "missing"))
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.INVOKER_METHOD_NOT_FOUND })
    }

    @Test
    fun invokerValidationPassesForValidMethod() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Invoker
                void invokeSetScreen(Screen screen);
            }
        """.trimIndent()
        val diagnostics = fakeFacade.diagnose(MixinE2ETestSupport.requestAt(source, "invokeSetScreen"))
        assertTrue(diagnostics.none { it.code == MixinDiagnosticCodes.INVOKER_METHOD_NOT_FOUND })
    }

    @Test
    fun simpleTargetMixinProducesNoUnresolvedTarget() {
        val source = """@Mixin(com.example.target.SimpleTarget.class) class M {}"""
        val diagnostics = simpleFacade.diagnose(MixinE2ETestSupport.requestAt(source, "SimpleTarget"))
        assertTrue(diagnostics.none { it.code == MixinDiagnosticCodes.UNRESOLVED_MIXIN_TARGET })
    }

    @Test
    fun reportsAmbiguousInjectMethodForSimpleTargetOverload() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            class M {
                @Inject(method = "draw", at = @At("HEAD"))
                void m() {}
            }
        """.trimIndent()
        val diagnostics = simpleFacade.diagnose(MixinE2ETestSupport.requestAt(source, "draw"))
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.AMBIGUOUS_INJECT_METHOD })
    }

    @Test
    fun reportsShadowStaticMismatchForSimpleTargetField() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            class M {
                @Shadow
                private static int counter;
            }
        """.trimIndent()
        val diagnostics = simpleFacade.diagnose(MixinE2ETestSupport.requestAt(source, "counter"))
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.SHADOW_STATIC_MISMATCH })
    }

    @Test
    fun fabricBasicExampleMixinHasNoUnresolvedInjectMethod() {
        val source = MixinE2ETestSupport.loadFixtureText(FixturePaths.FABRIC_BASIC_EXAMPLE_MIXIN)
        val diagnostics = simpleFacade.diagnose(MixinE2ETestSupport.requestAt(source, "draw"))
        assertTrue(diagnostics.none { it.code == MixinDiagnosticCodes.UNRESOLVED_INJECT_METHOD })
    }

    @Test
    fun brokenFixtureHasAtLeastTwoDistinctDiagnosticCodes() {
        val source = MixinE2ETestSupport.loadFixtureText(FixturePaths.BROKEN_DIAGNOSTICS_MIXIN)
        val diagnostics = simpleFacade.diagnose(MixinE2ETestSupport.requestAt(source, "BrokenMixin"))
        val codes = diagnostics.map { it.code }.toSet()
        assertTrue(codes.contains(MixinDiagnosticCodes.UNRESOLVED_INJECT_METHOD))
        assertTrue(codes.contains(MixinDiagnosticCodes.UNRESOLVED_AT_TARGET))
        assertTrue(codes.size >= 2)
    }
}
