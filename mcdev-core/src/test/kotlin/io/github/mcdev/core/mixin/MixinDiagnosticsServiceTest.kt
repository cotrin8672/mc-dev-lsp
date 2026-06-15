package io.github.mcdev.core.mixin

import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
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
    fun duplicateMixinConfigEntryRangesConfigEntry() {
        val source = """@Mixin(MinecraftClient.class) class ExampleMixin {}"""
        val config = """
            {
              "package": "example.mixin",
              "mixins": ["ExampleMixin", "ExampleMixin"]
            }
        """.trimIndent()
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
        val diagnostic = diagnostics.single { it.code == MixinDiagnosticCodes.DUPLICATE_MIXIN_CONFIG_ENTRY }
        assertRangeCovers(config, diagnostic.range, "ExampleMixin")
        assertTrue(diagnostic.range.start != McTextPosition(0, 0))
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
    fun reportsUnresolvedJavaTypeInInvokerDeclaration() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Invoker("setScreen")
                abstract void invokeSetScreen(UnknownScreen screen);
            }
        """.trimIndent()

        val diagnostics = analyze(source)

        assertTrue(diagnostics.any {
            it.code == MixinDiagnosticCodes.UNRESOLVED_JAVA_TYPE &&
                it.metadata["normalizedType"] == "UnknownScreen"
        })
    }

    @Test
    fun analyzesInjectMethodArray() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(
                    method = {
                        "tick",
                        "missing"
                    },
                    at = @At("HEAD")
                )
                void m() {}
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any {
            it.code == MixinDiagnosticCodes.UNRESOLVED_INJECT_METHOD &&
                it.metadata["method"] == "missing"
        })
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
    fun analyzesAtArraySeparately() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(
                    method = "tick",
                    at = {
                        @At("HEAD"),
                        @At(value = "INVOKE", target = "Lmissing/Class;draw()V")
                    }
                )
                void m() {}
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any {
            it.code == MixinDiagnosticCodes.UNRESOLVED_AT_TARGET &&
                it.metadata["target"] == "Lmissing/Class;draw()V"
        })
    }

    @Test
    fun reportsUnresolvedAtTargetForDescriptorBearingInjectMethod() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(method = "tick()V", at = @At(value = "INVOKE", target = "Lmissing/Class;draw()V"))
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

    @Test
    fun injectMethodDiagnosticUsesAbsoluteSourceOffset() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(
                    method = "missing",
                    at = @At("HEAD")
                )
                void m() {}
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        val diagnostic = diagnostics.single { it.code == MixinDiagnosticCodes.UNRESOLVED_INJECT_METHOD }
        assertRangeCovers(source, diagnostic.range, "method = \"missing\"")
    }

    @Test
    fun injectMethodDiagnosticUsesAbsoluteSourceOffsetWithCrlf() {
        val source = "@Mixin(MinecraftClient.class)\r\nclass M {\r\n    @Inject(method = \"missing\", at = @At(\"HEAD\"))\r\n    void m() {}\r\n}"
        val diagnostics = analyze(source)
        val diagnostic = diagnostics.single { it.code == MixinDiagnosticCodes.UNRESOLVED_INJECT_METHOD }
        assertRangeCovers(source, diagnostic.range, "method = \"missing\"")
    }

    @Test
    fun atTargetDiagnosticRangesTargetValueInSource() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(method = "tick", at = @At(value = "INVOKE", target = "Lmissing/Class;draw()V"))
                void m() {}
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        val diagnostic = diagnostics.single { it.code == MixinDiagnosticCodes.UNRESOLVED_AT_TARGET }
        assertRangeCovers(source, diagnostic.range, "Lmissing/Class;draw()V")
        assertTrue(diagnostic.range.start != McTextPosition(0, 0))
    }

    @Test
    fun returnOrdinalValidationUsesContainingInjectorMethod() {
        val classIndex = FakeClassIndex(
            methods = FakeClassIndex.defaultMethods() + mapOf(
                "net/minecraft/client/MinecraftClient" to listOf(
                    MethodIndexEntry("foo", "()V", false, "foo(): void"),
                    MethodIndexEntry("bar", "()V", false, "bar(): void"),
                ),
            ),
        )
        val bytecodeIndex = FakeBytecodeIndex(
            returnCounts = mapOf(
                "net/minecraft/client/MinecraftClient#foo" to 10,
                "net/minecraft/client/MinecraftClient#bar" to 2,
            ),
        )
        val service = MixinDiagnosticsService(classIndex, bytecodeIndex)
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(method = "foo", at = @At(value = "RETURN", ordinal = 5))
                void onFoo() {}

                @Inject(method = "bar", at = @At(value = "RETURN", ordinal = 5))
                void onBar() {}
            }
        """.trimIndent()
        val diagnostics = service.analyze(
            MixinDiagnosticRequest(
                source = source,
                documentUri = "file:///Mixin.java",
                mixinClassName = null,
                mixinPackage = null,
                mixinConfigContent = null,
                mixinConfigPath = null,
            ),
        )
        val ordinalDiagnostics = diagnostics.filter { it.code == MixinDiagnosticCodes.ORDINAL_OUT_OF_RANGE }
        assertEquals(1, ordinalDiagnostics.size)
        assertRangeCovers(
            source,
            ordinalDiagnostics.single().range,
            "ordinal = 5",
            fromIndex = source.indexOf("method = \"bar\""),
        )
    }

    private fun assertRangeCovers(
        source: String,
        range: McTextRange,
        substring: String,
        fromIndex: Int = 0,
    ) {
        val start = source.indexOf(substring, fromIndex)
        assertTrue(start >= 0, "substring not found: $substring")
        val end = start + substring.length
        assertEquals(offsetToPosition(source, start), range.start)
        assertEquals(offsetToPosition(source, end), range.end)
    }

    private fun offsetToPosition(source: String, offset: Int): McTextPosition {
        var line = 0
        var character = 0
        var i = 0
        while (i < offset && i < source.length) {
            if (source[i] == '\n') {
                line++
                character = 0
            } else {
                character++
            }
            i++
        }
        return McTextPosition(line, character)
    }

    @Test
    fun reportsUnresolvedFullyQualifiedMixinClassTarget() {
        val source = """@org.spongepowered.asm.mixin.Mixin(com.example.missing.Unknown.class) class M {}"""
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any {
            it.code == MixinDiagnosticCodes.UNRESOLVED_MIXIN_TARGET &&
                it.message.contains("com/example/missing/Unknown")
        })
    }

    @Test
    fun reportsUnresolvedFullyQualifiedMixinArrayTarget() {
        val source = """@org.spongepowered.asm.mixin.Mixin({ com.example.missing.A.class, MinecraftClient.class }) class M {}"""
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any {
            it.code == MixinDiagnosticCodes.UNRESOLVED_MIXIN_TARGET &&
                it.message.contains("com/example/missing/A")
        })
        assertTrue(diagnostics.none {
            it.code == MixinDiagnosticCodes.UNRESOLVED_MIXIN_TARGET &&
                it.message.contains("MinecraftClient")
        })
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
