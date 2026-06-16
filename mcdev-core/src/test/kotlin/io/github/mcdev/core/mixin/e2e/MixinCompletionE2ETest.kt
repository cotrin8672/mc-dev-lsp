package io.github.mcdev.core.mixin.e2e

import io.github.mcdev.core.mixin.InjectMethodDescriptorMode
import io.github.mcdev.core.mixin.MixinClassInsertMode
import io.github.mcdev.core.mixin.MixinCompletionOptions
import io.github.mcdev.fixtures.FixturePaths
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue

class MixinCompletionE2ETest {
    private val fakeFacade = MixinE2ETestSupport.fakeFacade()
    private val simpleFacade = MixinE2ETestSupport.simpleTargetFacade()

    @Test
    fun completesMixinClassTargetWithFakeIndex() {
        val source = """@Mixin(Mine"""
        val items = fakeFacade.complete(MixinE2ETestSupport.requestAt(source, "Mine"))
        assertTrue(items.any { it.label == "MinecraftClient" })
        assertEquals("MinecraftClient.class", items.first { it.label == "MinecraftClient" }.insertText)
    }

    @Test
    fun mixinClassCompletionSeparatesLabelAndInsertText() {
        val source = """@Mixin(Mine"""
        val item = fakeFacade.complete(MixinE2ETestSupport.requestAt(source, "Mine"))
            .first { it.label == "MinecraftClient" }
        assertEquals("MinecraftClient", item.label)
        assertNotEquals(item.label, item.insertText)
        assertTrue(item.insertText.endsWith(".class"))
    }

    @Test
    fun completesMixinClassWithFqnMode() {
        val source = """@Mixin(Mine"""
        val items = fakeFacade.complete(
            MixinE2ETestSupport.requestAt(source, "Mine"),
            MixinCompletionOptions(classInsertMode = MixinClassInsertMode.FQN),
        )
        assertTrue(items.any { it.insertText == "net.minecraft.client.MinecraftClient.class" })
    }

    @Test
    fun completesMixinTargetsString() {
        val source = """@Mixin(targets = "net.minecraft.client.Mine")"""
        val items = fakeFacade.complete(MixinE2ETestSupport.requestAt(source, "Mine"))
        assertTrue(items.any { it.insertText == "net.minecraft.client.MinecraftClient" })
    }

    @Test
    fun completesInjectMethodFromResolvedMixinTarget() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(method = "ti", at = @At("HEAD"))
                void m() {}
            }
        """.trimIndent()
        val items = fakeFacade.complete(MixinE2ETestSupport.requestAt(source, "ti"))
        assertTrue(items.any { it.insertText == "tick" })
    }

    @Test
    fun completesInjectMethodAtEmptyOpenQuote() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(method = "
            }
        """.trimIndent()
        val quote = source.indexOf("method = \"") + "method = \"".length
        val items = fakeFacade.complete(MixinE2ETestSupport.requestAtOffset(source, quote))
        assertTrue(items.any { it.insertText == "tick" })
    }

    @Test
    fun completesImportedItemMixinInjectMethod() {
        val classIndex = io.github.mcdev.core.mixin.FakeClassIndex(
            classes = io.github.mcdev.core.mixin.FakeClassIndex.defaultClasses() + listOf(
                io.github.mcdev.core.mixin.ClassIndexEntry("Item", "net.minecraft.world.item", "net/minecraft/world/item/Item"),
                io.github.mcdev.core.mixin.ClassIndexEntry("Item", "com.example.other", "com/example/other/Item"),
            ),
            methods = io.github.mcdev.core.mixin.FakeClassIndex.defaultMethods() + mapOf(
                "net/minecraft/world/item/Item" to listOf(
                    io.github.mcdev.core.mixin.MethodIndexEntry("isFoil", "()Z", false, "isFoil(): boolean"),
                ),
            ),
        )
        val facade = io.github.mcdev.core.mixin.MixinServiceFacade(classIndex, io.github.mcdev.core.mixin.FakeBytecodeIndex())
        val source = """
            import net.minecraft.world.item.Item;

            @Mixin(Item.class)
            class ItemMixin {
                @Inject(method = "
            }
        """.trimIndent()
        val quote = source.indexOf("method = \"") + "method = \"".length

        val items = facade.complete(MixinE2ETestSupport.requestAtOffset(source, quote))

        assertTrue(items.any { it.insertText == "isFoil" })
    }

    @Test
    fun completesImportedItemShadowMethodAtDeclarationName() {
        val classIndex = io.github.mcdev.core.mixin.FakeClassIndex(
            classes = io.github.mcdev.core.mixin.FakeClassIndex.defaultClasses() + listOf(
                io.github.mcdev.core.mixin.ClassIndexEntry("Item", "net.minecraft.world.item", "net/minecraft/world/item/Item"),
                io.github.mcdev.core.mixin.ClassIndexEntry("Item", "com.example.other", "com/example/other/Item"),
            ),
            methods = io.github.mcdev.core.mixin.FakeClassIndex.defaultMethods() + mapOf(
                "net/minecraft/world/item/Item" to listOf(
                    io.github.mcdev.core.mixin.MethodIndexEntry("isFoil", "()Z", false, "isFoil(): boolean"),
                    io.github.mcdev.core.mixin.MethodIndexEntry("getCount", "()I", false, "getCount(): int"),
                ),
                "com/example/other/Item" to listOf(
                    io.github.mcdev.core.mixin.MethodIndexEntry("otherOnly", "()V", false, "otherOnly(): void"),
                ),
            ),
        )
        val facade = io.github.mcdev.core.mixin.MixinServiceFacade(classIndex, io.github.mcdev.core.mixin.FakeBytecodeIndex())
        val source = """
            import net.minecraft.world.item.Item;

            @Mixin(Item.class)
            class ItemMixin {
                @Shadow public boolean is
            }
        """.trimIndent()

        val items = facade.complete(MixinE2ETestSupport.requestAt(source, "is"))

        assertTrue(items.any { it.insertText == "isFoil" })
        assertTrue(items.none { it.insertText == "getCount" })
        assertTrue(items.none { it.insertText == "otherOnly" })
    }

    @Test
    fun injectMethodCompletionLabelUsesReadableSignature() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(method = "ti", at = @At("HEAD"))
                void m() {}
            }
        """.trimIndent()
        val item = fakeFacade.complete(MixinE2ETestSupport.requestAt(source, "ti"))
            .first { it.insertText == "tick" }
        assertTrue(item.label.contains("tick"))
        assertNotEquals(item.label, item.insertText)
    }

    @Test
    fun injectMethodCompletionWithDescriptorModeAlways() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(method = "ti", at = @At("HEAD"))
                void m() {}
            }
        """.trimIndent()
        val items = fakeFacade.complete(
            MixinE2ETestSupport.requestAt(source, "ti"),
            MixinCompletionOptions(injectMethodDescriptorMode = InjectMethodDescriptorMode.ALWAYS),
        )
        assertTrue(items.any { it.insertText.contains("()V") })
    }

    @Test
    fun completesAtValueHead() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(method = "tick", at = @At("HE"))
                void m() {}
            }
        """.trimIndent()
        val items = fakeFacade.complete(MixinE2ETestSupport.requestAt(source, "HE"))
        assertTrue(items.any { it.insertText == "HEAD" })
    }

    @Test
    fun completesAtValueInvoke() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(method = "tick", at = @At("INV"))
                void m() {}
            }
        """.trimIndent()
        val items = fakeFacade.complete(MixinE2ETestSupport.requestAt(source, "INV"))
        assertTrue(items.any { it.insertText == "INVOKE" })
    }

    @Test
    fun completesAtTargetWithFakeBytecodeIndex() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(method = "tick", at = @At(value = "INVOKE", target = ""))
                void m() {}
            }
        """.trimIndent()
        val quote = source.indexOf("target = \"") + "target = \"".length
        val items = fakeFacade.complete(MixinE2ETestSupport.requestAtOffset(source, quote))
        assertTrue(items.isNotEmpty())
        assertTrue(items.first().insertText.startsWith("L"))
        assertNotEquals(items.first().label, items.first().insertText)
    }

    @Test
    fun completesAtReturnTargetFromSimpleTargetBytecode() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            class M {
                @Inject(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "RETURN", target = ""))
                void m() {}
            }
        """.trimIndent()
        val quote = source.indexOf("target = \"") + "target = \"".length
        val items = simpleFacade.complete(MixinE2ETestSupport.requestAtOffset(source, quote))
        assertTrue(items.isNotEmpty())
        assertEquals("RETURN", items.first().label)
    }

    @Test
    fun atTargetCompletionUsesDescriptorQualifiedInjectMethod() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            class M {
                @Inject(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = ""))
                void m() {}
            }
        """.trimIndent()
        val quote = source.indexOf("target = \"") + "target = \"".length
        val items = simpleFacade.complete(MixinE2ETestSupport.requestAtOffset(source, quote))
        assertTrue(items.any { it.insertText == "Ljava/lang/String;length()I" })
    }

    @Test
    fun atTargetCompletionDoesNotUseWrongOverloadWhenDescriptorDiffers() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            class M {
                @Inject(method = "draw(I)V", at = @At(value = "INVOKE", target = ""))
                void m() {}
            }
        """.trimIndent()
        val quote = source.indexOf("target = \"") + "target = \"".length
        val items = simpleFacade.complete(MixinE2ETestSupport.requestAtOffset(source, quote))
        assertTrue(items.isEmpty())
    }

    @Test
    fun completesShadowPrefixAttribute() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Shadow(prefix = "shadow")
                private Screen currentScreen;
            }
        """.trimIndent()
        val items = fakeFacade.complete(MixinE2ETestSupport.requestAt(source, "shadow"))
        assertTrue(items.isEmpty())
    }

    @Test
    fun completesSimpleTargetMixinClassFromBytecodeIndex() {
        val source = """@Mixin(Simple"""
        val items = simpleFacade.complete(MixinE2ETestSupport.requestAt(source, "Simple"))
        assertTrue(items.any { it.label == "SimpleTarget" })
    }

    @Test
    fun completesSimpleTargetInjectMethod() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            class M {
                @Inject(method = "dr", at = @At("HEAD"))
                void m() {}
            }
        """.trimIndent()
        val items = simpleFacade.complete(MixinE2ETestSupport.requestAt(source, "dr"))
        assertTrue(items.any { it.insertText.startsWith("draw") })
    }

    @Test
    fun fabricBasicExampleMixinLoadsFromFixture() {
        val source = MixinE2ETestSupport.loadFixtureText(FixturePaths.FABRIC_BASIC_EXAMPLE_MIXIN)
        val methodNeedle = "method = \"draw"
        val items = simpleFacade.complete(MixinE2ETestSupport.requestAt(source, methodNeedle))
        assertTrue(items.any { it.insertText.startsWith("draw") })
    }

    @Test
    fun returnsEmptyOutsideAnnotationContext() {
        val source = "class Plain { int value; }"
        val items = fakeFacade.complete(MixinE2ETestSupport.requestAt(source, "value"))
        assertTrue(items.isEmpty())
    }

    @Test
    fun atTargetCompletionPreservesDisplayLabel() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(method = "tick", at = @At(value = "INVOKE", target = "Text"))
                void m() {}
            }
        """.trimIndent()
        val item = fakeFacade.complete(MixinE2ETestSupport.requestAt(source, "Text"))
            .firstOrNull()
        assertTrue(item != null)
        assertTrue(item.label.contains("draw"))
        assertTrue(item.insertText.startsWith("L"))
    }
}
