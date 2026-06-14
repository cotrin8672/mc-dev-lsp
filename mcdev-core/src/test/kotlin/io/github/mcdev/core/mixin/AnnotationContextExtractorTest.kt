package io.github.mcdev.core.mixin

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull

class AnnotationContextExtractorTest {
    @Test
    fun extractsMixinClassSlot() {
        val source = "@Mixin(MinecraftClient.class)\npublic class ExampleMixin {}"
        val offset = source.indexOf("MinecraftClient") + "MinecraftClient".length
        val context = AnnotationContextExtractor.extractAtOffset(source, offset)
        assertNotNull(context)
        assertEquals(MixinAnnotation.MIXIN, context.annotation)
        assertEquals(AnnotationSlot.CLASS, context.slot)
        assertEquals("MinecraftClient", context.partialValue)
    }

    @Test
    fun extractsMixinValueClassSlot() {
        val source = "@Mixin(value = GameRenderer.class)\nclass M {}"
        val offset = source.indexOf("GameRenderer") + "GameRenderer".length
        val context = AnnotationContextExtractor.extractAtOffset(source, offset)
        assertNotNull(context)
        assertEquals(AnnotationSlot.CLASS, context.slot)
        assertEquals("GameRenderer", context.partialValue)
    }

    @Test
    fun extractsMixinTargetsStringSlot() {
        val source = """@Mixin(targets = "net.minecraft.client.Mine")"""
        val partial = "net.minecraft.client.Mine"
        val offset = source.indexOf(partial) + partial.length
        val context = AnnotationContextExtractor.extractAtOffset(source, offset)
        assertNotNull(context)
        assertEquals(AnnotationSlot.TARGETS, context.slot)
        assertEquals(partial, context.partialValue)
    }

    @Test
    fun extractsMixinTargetsArraySlot() {
        val source = """@Mixin(targets = { "net.minecraft.client.MinecraftClient", "a.b.C" })"""
        val cursor = source.indexOf("Mine") + 4
        val offset = AnnotationContextExtractor.toOffset(source, 0, cursor)!!
        val context = AnnotationContextExtractor.extractAtOffset(source, offset)
        assertNotNull(context)
        assertEquals(AnnotationSlot.TARGETS, context.slot)
    }

    @Test
    fun extractsInjectMethodSlot() {
        val source = """
            @Mixin(MinecraftClient.class)
            class ExampleMixin {
                @Inject(method = "ti", at = @At("HEAD"))
                private void onTick() {}
            }
        """.trimIndent()
        val partial = "ti"
        val offset = source.indexOf(partial) + partial.length
        val context = AnnotationContextExtractor.extractAtOffset(source, offset)
        assertNotNull(context)
        assertEquals(MixinAnnotation.INJECT, context.annotation)
        assertEquals(AnnotationSlot.METHOD, context.slot)
        assertEquals(partial, context.partialValue)
        assertEquals(listOf("MinecraftClient"), context.mixinTargetInternalNames)
    }

    @Test
    fun extractsNestedAtValueSlot() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(method = "tick", at = @At(value = "IN", ordinal = 0))
                void m() {}
            }
        """.trimIndent()
        val partial = "IN"
        val offset = source.indexOf(partial) + partial.length
        val context = AnnotationContextExtractor.extractAtOffset(source, offset)
        assertNotNull(context)
        assertEquals(MixinAnnotation.AT, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals(partial, context.partialValue)
    }

    @Test
    fun extractsAtTargetSlot() {
        val source = """@Inject(method = "tick", at = @At(value = "INVOKE", target = "Lnet/minecraft/"))"""
        val partial = "Lnet/minecraft/"
        val offset = source.indexOf(partial) + partial.length
        val context = AnnotationContextExtractor.extractAtOffset(source, offset)
        assertNotNull(context)
        assertEquals(MixinAnnotation.AT, context.annotation)
        assertEquals(AnnotationSlot.TARGET, context.slot)
        assertEquals(partial, context.partialValue)
    }

    @Test
    fun extractsAccessorValueSlot() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Accessor("current")
                Screen getCurrentScreen();
            }
        """.trimIndent()
        val idx = source.indexOf("current") + 3
        val context = AnnotationContextExtractor.extractAtOffset(source, idx)
        assertNotNull(context)
        assertEquals(MixinAnnotation.ACCESSOR, context.annotation)
        assertEquals(AnnotationSlot.ACCESSOR_VALUE, context.slot)
    }

    @Test
    fun extractsInvokerValueSlot() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Invoker("setSc")
                void invokeSetScreen(Screen s);
            }
        """.trimIndent()
        val idx = source.indexOf("setSc") + 4
        val context = AnnotationContextExtractor.extractAtOffset(source, idx)
        assertNotNull(context)
        assertEquals(MixinAnnotation.INVOKER, context.annotation)
        assertEquals(AnnotationSlot.INVOKER_VALUE, context.slot)
    }

    @Test
    fun extractsShadowPrefixSlot() {
        val source = """@Shadow(prefix = "shadow$")"""
        val idx = source.indexOf("shadow$") + 3
        val context = AnnotationContextExtractor.extractAtOffset(source, idx)
        assertNotNull(context)
        assertEquals(MixinAnnotation.SHADOW, context.annotation)
        assertEquals(AnnotationSlot.PREFIX, context.slot)
    }

    @Test
    fun extractsRedirectMethodSlot() {
        val source = """@Redirect(method = "tick", at = @At("HEAD"))"""
        val idx = source.indexOf("tick")
        val context = AnnotationContextExtractor.extractAtOffset(source, idx + 2)
        assertNotNull(context)
        assertEquals(MixinAnnotation.REDIRECT, context.annotation)
        assertEquals(AnnotationSlot.METHOD, context.slot)
    }

    @Test
    fun extractsModifyArgMethodSlot() {
        val source = """@ModifyArg(method = "render", at = @At("HEAD"))"""
        val idx = source.indexOf("render") + 3
        val context = AnnotationContextExtractor.extractAtOffset(source, idx)
        assertNotNull(context)
        assertEquals(MixinAnnotation.MODIFY_ARG, context.annotation)
    }

    @Test
    fun extractsModifyArgsMethodSlot() {
        val source = """@ModifyArgs(method = "render", at = @At("HEAD"))"""
        val context = AnnotationContextExtractor.extractAtOffset(source, source.indexOf("render") + 2)
        assertNotNull(context)
        assertEquals(MixinAnnotation.MODIFY_ARGS, context.annotation)
    }

    @Test
    fun extractsModifyVariableMethodSlot() {
        val source = """@ModifyVariable(method = "tick", at = @At("HEAD"))"""
        val context = AnnotationContextExtractor.extractAtOffset(source, source.indexOf("tick") + 1)
        assertNotNull(context)
        assertEquals(MixinAnnotation.MODIFY_VARIABLE, context.annotation)
    }

    @Test
    fun extractsModifyConstantMethodSlot() {
        val source = """@ModifyConstant(method = "tick", at = @At("HEAD"))"""
        val context = AnnotationContextExtractor.extractAtOffset(source, source.indexOf("tick") + 1)
        assertNotNull(context)
        assertEquals(MixinAnnotation.MODIFY_CONSTANT, context.annotation)
    }

    @Test
    fun returnsNullOutsideAnnotation() {
        val source = "class Example { int value; }"
        assertNull(extract(source, 0, 10))
    }

    @Test
    fun resolvesMixinTargetsFromClassAnnotation() {
        val source = "@Mixin(MinecraftClient.class)\nclass ExampleMixin {}"
        val targets = AnnotationContextExtractor.resolveMixinTargets(source, source.length)
        assertEquals(listOf("MinecraftClient"), targets)
    }

    @Test
    fun resolvesMixinTargetsFromStringTargets() {
        val source = """@Mixin(targets = "net.minecraft.client.MinecraftClient") class M {}"""
        val targets = AnnotationContextExtractor.resolveMixinTargets(source, source.length)
        assertEquals(listOf("net.minecraft.client.MinecraftClient"), targets)
    }

    @Test
    fun parsesMultipleMixinClassTargets() {
        val source = """@Mixin({ MinecraftClient.class, GameRenderer.class })"""
        val targets = AnnotationContextExtractor.parseMixinTargetValues(source, 0)
        assertEquals(listOf("MinecraftClient", "GameRenderer"), targets)
    }

    @Test
    fun parsesMixinValueClassTarget() {
        val source = """@Mixin(value = SimpleTarget.class) class M {}"""
        val targets = AnnotationContextExtractor.parseMixinTargetValues(source, 0)
        assertEquals(listOf("SimpleTarget"), targets)
    }

    @Test
    fun extractsFullyQualifiedMixinClassSlot() {
        val source = "@org.spongepowered.asm.mixin.Mixin(net.minecraft.client.MinecraftClient.class)\nclass M {}"
        val offset = source.indexOf("MinecraftClient") + "MinecraftClient".length
        val context = AnnotationContextExtractor.extractAtOffset(source, offset)
        assertNotNull(context)
        assertEquals(MixinAnnotation.MIXIN, context.annotation)
        assertEquals(AnnotationSlot.CLASS, context.slot)
        assertEquals("MinecraftClient", context.partialValue)
    }

    @Test
    fun extractsFullyQualifiedMixinArrayClassSlot() {
        val source = "@org.spongepowered.asm.mixin.Mixin({ net.minecraft.client.GameRenderer.class }) class M {}"
        val offset = source.indexOf("GameRenderer") + "GameRenderer".length
        val context = AnnotationContextExtractor.extractAtOffset(source, offset)
        assertNotNull(context)
        assertEquals(MixinAnnotation.MIXIN, context.annotation)
        assertEquals(AnnotationSlot.CLASS, context.slot)
    }

    @Test
    fun parsesFullyQualifiedMixinClassTarget() {
        val source = "@org.spongepowered.asm.mixin.Mixin(com.example.client.MinecraftClient.class) class M {}"
        val targets = AnnotationContextExtractor.parseMixinTargetValues(source, 0)
        assertEquals(listOf("com/example/client/MinecraftClient"), targets)
    }

    @Test
    fun parsesFullyQualifiedMixinArrayTargets() {
        val source = """@org.spongepowered.asm.mixin.Mixin({ com.example.A.class, com.example.B.class }) class M {}"""
        val targets = AnnotationContextExtractor.parseMixinTargetValues(source, 0)
        assertEquals(listOf("com/example/A", "com/example/B"), targets)
    }

    @Test
    fun resolvesMixinTargetsFromFullyQualifiedAnnotationOnInterface() {
        val padding = "// comment\n".repeat(80)
        val source = "$padding@org.spongepowered.asm.mixin.Mixin(MinecraftClient.class)\ninterface ExampleMixin {}"
        val targets = AnnotationContextExtractor.resolveMixinTargets(source, source.length)
        assertEquals(listOf("MinecraftClient"), targets)
    }

    @Test
    fun extractsFullyQualifiedInjectMethodSlot() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @org.spongepowered.asm.mixin.injection.Inject(method = "ti", at = @org.spongepowered.asm.mixin.injection.At("HEAD"))
                private void onTick() {}
            }
        """.trimIndent()
        val partial = "ti"
        val offset = source.indexOf("method = \"$partial\"") + "method = \"".length + partial.length
        val context = AnnotationContextExtractor.extractAtOffset(source, offset)
        assertNotNull(context)
        assertEquals(MixinAnnotation.INJECT, context.annotation)
        assertEquals(AnnotationSlot.METHOD, context.slot)
        assertEquals(partial, context.partialValue)
    }

    @Test
    fun extractsFullyQualifiedAtValueSlot() {
        val source = """@Inject(method = "tick", at = @org.spongepowered.asm.mixin.injection.At(value = "INVOKE", target = "Lnet/minecraft/"))"""
        val offset = source.indexOf("INVOKE") + "IN".length
        val context = AnnotationContextExtractor.extractAtOffset(source, offset)
        assertNotNull(context)
        assertEquals(MixinAnnotation.AT, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
    }

    @Test
    fun fqnToInternalConversion() {
        assertEquals("net/minecraft/client/MinecraftClient", AnnotationContextExtractor.fqnToInternal("net.minecraft.client.MinecraftClient"))
    }

    @Test
    fun toOffsetComputesCorrectPosition() {
        val source = "line1\nline2\n"
        assertEquals(8, AnnotationContextExtractor.toOffset(source, 1, 2))
    }

    private fun extract(source: String, line: Int, character: Int): AnnotationContext? =
        AnnotationContextExtractor.extract(source, line, character)
}
