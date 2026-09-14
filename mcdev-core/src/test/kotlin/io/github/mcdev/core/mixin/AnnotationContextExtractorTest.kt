package io.github.mcdev.core.mixin

import io.github.mcdev.core.mixinextras.ExpressionCompletionPosition
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull

class AnnotationContextExtractorTest {
    @Test
    fun defersUppercaseMixinPartialToClassSlot() {
        val source = "@Mixin(T\nclass ExampleMixin {}"
        val offset = source.indexOf('T') + 1
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, offset))
        assertEquals(MixinAnnotation.MIXIN, context.annotation)
        assertEquals(AnnotationSlot.CLASS, context.slot)
        assertEquals("T", context.partialValue)
    }

    @Test
    fun defersAmbiguousUppercaseMixinPrefixToClassSlot() {
        val source = "@Mixin(Tar\nclass ExampleMixin {}"
        val offset = source.indexOf("Tar") + "Tar".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, offset))
        assertEquals(MixinAnnotation.MIXIN, context.annotation)
        assertEquals(AnnotationSlot.CLASS, context.slot)
        assertEquals("Tar", context.partialValue)
    }

    @Test
    fun extractsLowercaseMixinAttributePrefix() {
        val source = "@Mixin(pri"
        val offset = source.indexOf("pri") + "pri".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, offset))
        assertEquals(MixinAnnotation.MIXIN, context.annotation)
        assertEquals(AnnotationSlot.ATTRIBUTE, context.slot)
        assertEquals("pri", context.partialValue)
    }

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
    fun annotationIdentityAcceptsOfficialTokensAndRejectsUnrelatedSameSimpleName() {
        data class Case(val source: String, val marker: String, val expected: MixinAnnotation?)

        val injectFqn = "org.spongepowered.asm.mixin.injection.Inject"
        val localFqn = "com.llamalad7.mixinextras.sugar.Local"
        val constantFqn = "org.spongepowered.asm.mixin.injection.Constant"

        val cases = listOf(
            Case("""@$injectFqn(method = "^ti")""", "^ti", MixinAnnotation.INJECT),
            Case("import $injectFqn;\n@Inject(method = \"^ti\")", "^ti", MixinAnnotation.INJECT),
            Case("""@Inject(method = "ti")""", "ti", MixinAnnotation.INJECT),
            Case("""@com.example.Inject(method = "ti")""", "ti", null),
            Case("import com.example.Inject;\n@Inject(method = \"ti\")", "ti", null),
            Case("""@$localFqn(ord""", "ord", MixinAnnotation.LOCAL),
            Case("import $localFqn;\n@Local(ord", "ord", MixinAnnotation.LOCAL),
            Case("""@Local(ord""", "ord", MixinAnnotation.LOCAL),
            Case("""@com.example.Local(ord""", "ord", null),
            Case("import com.example.Local;\n@Local(ord", "ord", null),
            Case(
                """@ModifyConstant(method = "tick", constant = @$constantFqn(nul))""",
                "nul",
                MixinAnnotation.CONSTANT,
            ),
            Case(
                "import $constantFqn;\n@ModifyConstant(method = \"tick\", constant = @Constant(nul))",
                "nul",
                MixinAnnotation.CONSTANT,
            ),
            Case("""@ModifyConstant(method = "tick", constant = @Constant(nul))""", "nul", MixinAnnotation.CONSTANT),
            Case(
                """@ModifyConstant(method = "tick", constant = @com.example.Constant(nul))""",
                "nul",
                null,
            ),
            Case(
                "import com.example.Constant;\n@ModifyConstant(method = \"tick\", constant = @Constant(nul))",
                "nul",
                null,
            ),
        )

        cases.forEach { (source, marker, expected) ->
            val offset = source.indexOf(marker) + marker.length
            val context = AnnotationContextExtractor.extractAtOffset(source, offset)
            if (expected == null) {
                assertNull(context, "Expected no context for source:\n$source")
            } else {
                assertNotNull(context, "Expected context for source:\n$source")
                assertEquals(expected, context.annotation)
            }
        }
    }

    @Test
    fun resolvesWrapWithConditionAnnotationFqnFromQualifiedTokenOrExplicitImport() {
        data class Case(val source: String, val expectedFqn: String?)

        val v1Fqn = "com.llamalad7.mixinextras.injector.WrapWithCondition"
        val v2Fqn = "com.llamalad7.mixinextras.injector.v2.WrapWithCondition"
        val cases = listOf(
            Case("@$v1Fqn(ord", v1Fqn),
            Case("import $v1Fqn;\n@WrapWithCondition(ord", v1Fqn),
            Case("@$v2Fqn(ord", v2Fqn),
            Case("import $v2Fqn;\n@WrapWithCondition(ord", v2Fqn),
            Case("@WrapWithCondition(ord", null),
        )

        cases.forEach { (source, expectedFqn) ->
            val cursor = source.indexOf("ord") + "ord".length
            val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
            assertEquals(MixinAnnotation.WRAP_WITH_CONDITION, context.annotation)
            assertEquals(AnnotationSlot.ATTRIBUTE, context.slot)
            assertEquals(expectedFqn, context.resolvedAnnotationFqn)
        }
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
    fun quotedValueReplacementKeepsQuotesAndReplacesTheWholeValue() {
        val source = """@Inject(method = "draw")"""
        val cursor = source.indexOf("draw") + 2
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(source.indexOf("draw"), context.valueStartOffset)
        assertEquals(source.indexOf("draw") + "draw".length, context.valueEndOffset)
        assertEquals("dr", context.partialValue)
    }

    @Test
    fun extractsCurrentStringFromMethodArrayWithoutReplacingOtherElements() {
        val source = """@Inject(method = { "tick", "render" }, at = @At("HEAD"))"""
        val cursor = source.indexOf("render") + 3
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(AnnotationSlot.METHOD, context.slot)
        assertEquals("ren", context.partialValue)
        assertEquals(source.indexOf("render"), context.valueStartOffset)
        assertEquals(source.indexOf("render") + "render".length, context.valueEndOffset)
    }

    @Test
    fun extractsIncompleteInjectorAttributeName() {
        val source = """@Inject(meth)"""
        val cursor = source.indexOf("meth") + "meth".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(AnnotationSlot.ATTRIBUTE, context.slot)
        assertEquals("meth", context.partialValue)
        assertEquals(source.indexOf("meth"), context.valueStartOffset)
    }

    @Test
    fun nestedAtAttributesAreNotTreatedAsOuterInjectorAttributes() {
        val source = """@Inject(method = "tick", at = @At(value = "INVOKE", remap = false), re)"""
        val cursor = source.lastIndexOf("re") + 2
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(AnnotationSlot.ATTRIBUTE, context.slot)
        assertEquals(setOf("method", "at"), context.existingAttributes)
    }

    @Test
    fun atInsideSliceIsTrueWhenAtIsNestedInSliceWithinInjector() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(
                    method = "tick",
                    slice = @Slice(from = @At(value = "IN"), to = @At("RETURN"))
                )
                void m() {}
            }
        """.trimIndent()
        val offset = source.indexOf("IN") + "IN".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, offset))
        assertEquals(MixinAnnotation.AT, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals(true, context.atInsideSlice)
        assertEquals(MixinAnnotation.INJECT, context.parentInjectorAnnotation)
        assertEquals("tick", context.injectMethodName)
    }

    @Test
    fun atInsideSliceIsFalseForDirectInjectorAt() {
        val source = """@Inject(method = "tick", at = @At(value = "HE"))"""
        val offset = source.indexOf("HE") + "HE".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, offset))
        assertEquals(MixinAnnotation.AT, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals(false, context.atInsideSlice)
        assertEquals(MixinAnnotation.INJECT, context.parentInjectorAnnotation)
        assertEquals("tick", context.injectMethodName)
    }

    @Test
    fun atInsideUnrelatedSameSimpleNameSliceIsFalse() {
        val source = """@Inject(method = "tick", slice = @com.example.Slice(from = @At(value = "IN")))"""
        val offset = source.indexOf("IN") + "IN".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, offset))
        assertEquals(MixinAnnotation.AT, context.annotation)
        assertEquals(false, context.atInsideSlice)
    }

    @Test
    fun atInsideSliceIsFalseWhenFakeSliceAppearsInString() {
        val source = """@Inject(method = "tick", note = "@Slice(from = @At(\"X\"))", at = @At(value = "HE"))"""
        val offset = source.indexOf("HE") + "HE".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, offset))
        assertEquals(MixinAnnotation.AT, context.annotation)
        assertEquals(false, context.atInsideSlice)
        assertEquals(MixinAnnotation.INJECT, context.parentInjectorAnnotation)
        assertEquals("tick", context.injectMethodName)
    }

    @Test
    fun atInsideSliceIsFalseWhenFakeSliceAppearsInLineComment() {
        val source = """
            @Inject(
                method = "tick", // @Slice(from = @At("X"))
                at = @At(value = "HE")
            )
        """.trimIndent()
        val offset = source.indexOf("HE") + "HE".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, offset))
        assertEquals(MixinAnnotation.AT, context.annotation)
        assertEquals(false, context.atInsideSlice)
        assertEquals(MixinAnnotation.INJECT, context.parentInjectorAnnotation)
        assertEquals("tick", context.injectMethodName)
    }

    @Test
    fun atInsideSliceIsFalseWhenFakeSliceAppearsInBlockComment() {
        val source = """
            @Inject(
                method = "tick", /* @Slice(from = @At("X")) */
                at = @At(value = "HE")
            )
        """.trimIndent()
        val offset = source.indexOf("HE") + "HE".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, offset))
        assertEquals(MixinAnnotation.AT, context.annotation)
        assertEquals(false, context.atInsideSlice)
        assertEquals(MixinAnnotation.INJECT, context.parentInjectorAnnotation)
        assertEquals("tick", context.injectMethodName)
    }

    @Test
    fun atInsideSliceIsFalseWhenFakeSliceAppearsInCharLiteral() {
        val source = """@Inject(method = "tick", note = '@Slice(from = @At("X"))', at = @At(value = "HE"))"""
        val offset = source.indexOf("HE") + "HE".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, offset))
        assertEquals(MixinAnnotation.AT, context.annotation)
        assertEquals(false, context.atInsideSlice)
        assertEquals(MixinAnnotation.INJECT, context.parentInjectorAnnotation)
        assertEquals("tick", context.injectMethodName)
    }

    @Test
    fun atInsideSliceIsTrueWhenSliceBodyContainsParenInEscapedStringBeforeAt() {
        val source = """
            @Inject(
                method = "tick",
                slice = @Slice(note = "decoy \") still", from = @At(value = "IN"), to = @At("RETURN"))
            )
        """.trimIndent()
        val offset = source.indexOf("IN") + "IN".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, offset))
        assertEquals(MixinAnnotation.AT, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals(true, context.atInsideSlice)
        assertEquals(MixinAnnotation.INJECT, context.parentInjectorAnnotation)
        assertEquals("tick", context.injectMethodName)
    }

    @Test
    fun atInsideSliceIsTrueWhenSliceBodyContainsParenInLineCommentBeforeAt() {
        val source = """
            @Inject(
                method = "tick",
                slice = @Slice(
                    // decoy )
                    from = @At(value = "IN"),
                    to = @At("RETURN")
                )
            )
        """.trimIndent()
        val offset = source.indexOf("IN") + "IN".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, offset))
        assertEquals(MixinAnnotation.AT, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals(true, context.atInsideSlice)
        assertEquals(MixinAnnotation.INJECT, context.parentInjectorAnnotation)
        assertEquals("tick", context.injectMethodName)
    }

    @Test
    fun atInsideSliceIsTrueWhenSliceBodyContainsParenInBlockCommentBeforeAt() {
        val source = """
            @Inject(
                method = "tick",
                slice = @Slice(/* decoy ) */ from = @At(value = "IN"), to = @At("RETURN"))
            )
        """.trimIndent()
        val offset = source.indexOf("IN") + "IN".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, offset))
        assertEquals(MixinAnnotation.AT, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals(true, context.atInsideSlice)
        assertEquals(MixinAnnotation.INJECT, context.parentInjectorAnnotation)
        assertEquals("tick", context.injectMethodName)
    }

    @Test
    fun atInsideSliceIsTrueWhenSliceBodyContainsParenInCharLiteralBeforeAt() {
        val source = """
            @Inject(
                method = "tick",
                slice = @Slice(marker = ')', from = @At(value = "IN"), to = @At("RETURN"))
            )
        """.trimIndent()
        val offset = source.indexOf("IN") + "IN".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, offset))
        assertEquals(MixinAnnotation.AT, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals(true, context.atInsideSlice)
        assertEquals(MixinAnnotation.INJECT, context.parentInjectorAnnotation)
        assertEquals("tick", context.injectMethodName)
    }

    @Test
    fun atInsideSliceIgnoresManyFakeAtSignsBeforeRealSlice() {
        val fakeAtNoise = buildString {
            repeat(300) { index ->
                append("""note$index = "@At @Slice @At", """)
                append("// @Slice(from = @At(\"X\"))\n")
                append("""/* @Slice(from = @At("X")) */ """)
                append("""marker$index = '@@', """)
            }
        }
        val source = """
            @Inject(
                method = "tick",
                $fakeAtNoise
                slice = @Slice(from = @At(value = "IN"), to = @At("RETURN"))
            )
        """.trimIndent()
        val offset = source.indexOf("IN") + "IN".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, offset))
        assertEquals(MixinAnnotation.AT, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals(true, context.atInsideSlice)
        assertEquals(MixinAnnotation.INJECT, context.parentInjectorAnnotation)
        assertEquals("tick", context.injectMethodName)
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
    fun extractsShadowMemberPrefixAfterBareAnnotation() {
        val source = """
            @Mixin(Item.class)
            class ItemMixin {
                @Shadow public boolean is
            }
        """.trimIndent()
        val start = source.indexOf("is")
        val context = AnnotationContextExtractor.extractAtOffset(source, start + "is".length)
        assertNotNull(context)
        assertEquals(MixinAnnotation.SHADOW, context.annotation)
        assertEquals(AnnotationSlot.SHADOW_MEMBER, context.slot)
        assertEquals("is", context.partialValue)
        assertEquals(start, context.valueStartOffset)
        assertEquals(start + "is".length, context.valueEndOffset)
    }

    @Test
    fun extractsShadowMemberPrefixAfterConfiguredAnnotation() {
        val source = """
            @Mixin(Item.class)
            class ItemMixin {
                @Shadow(remap = true) public boolean is
            }
        """.trimIndent()
        val start = source.indexOf("is")
        val context = AnnotationContextExtractor.extractAtOffset(source, start + "is".length)
        assertNotNull(context)
        assertEquals(MixinAnnotation.SHADOW, context.annotation)
        assertEquals(AnnotationSlot.SHADOW_MEMBER, context.slot)
        assertEquals("is", context.partialValue)
    }

    @Test
    fun overwriteReplacementCoversTheWholeIdentifier() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Overwrite public void ticker() {}
            }
        """.trimIndent()
        val cursor = source.indexOf("ticker") + 2
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(AnnotationSlot.OVERWRITE_METHOD, context.slot)
        assertEquals("ti", context.partialValue)
        assertEquals(source.indexOf("ticker") + "ticker".length, context.valueEndOffset)
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
    fun resolvesMixinTargetsFromUnterminatedClass() {
        val source = "@Mixin(MinecraftClient.class)\nclass ExampleMixin {"
        val targets = AnnotationContextExtractor.resolveRawMixinTargets(source, source.length)
        assertEquals(listOf("MinecraftClient"), targets)
    }

    @Test
    fun resolvesOuterMixinTargetsAfterClosedNestedMixinClass() {
        val source = """
            @Mixin(OuterTarget.class)
            class OuterMixin {
                @Mixin(NestedTarget.class)
                class NestedMixin {}
                void outerHandler() {}
            }
        """.trimIndent()
        val targets = AnnotationContextExtractor.resolveRawMixinTargets(source, source.indexOf("outerHandler"))
        assertEquals(listOf("OuterTarget"), targets)
    }

    @Test
    fun doesNotResolveOuterMixinTargetsInsideNonMixinNestedClass() {
        val source = """
            @Mixin(OuterTarget.class)
            class OuterMixin {
                class Helper {
                    @Inject(method = "draw")
                    void helper() {}
                }
            }
        """.trimIndent()
        val targets = AnnotationContextExtractor.resolveRawMixinTargets(source, source.indexOf("helper()"))
        assertEquals(emptyList(), targets)
    }

    @Test
    fun ignoresTypeDeclarationsInsideTextBlocksWhenResolvingMixinTargets() {
        val textBlock = "\"\"\""
        val source = """
            @Mixin(OuterTarget.class)
            class OuterMixin {
                String text = $textBlock
                    @Mixin(FakeTarget.class)
                    class FakeMixin {
                $textBlock;
                void outerHandler() {}
            }
        """.trimIndent()
        val targets = AnnotationContextExtractor.resolveRawMixinTargets(source, source.indexOf("outerHandler"))
        assertEquals(listOf("OuterTarget"), targets)
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
    fun annotationOffsetDiscoveryIgnoresAtSignsInNonCode() {
        val source = """
            // @Inject @At
            String text = "@Inject @At";
            char marker = '@';
            @Inject(method = "tick", at = @At("HEAD"))
        """.trimIndent()
        val injectOffset = source.lastIndexOf("@Inject")
        val atOffset = source.lastIndexOf("@At")
        assertEquals(
            listOf(atOffset),
            AnnotationContextExtractor.findAnnotationOffsets(source, MixinAnnotation.AT),
        )
        assertEquals(
            listOf(injectOffset),
            AnnotationContextExtractor.findInjectorAnnotationOffsets(source),
        )
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

    @Test
    fun extractsLocalAttributePrefixFromSimpleName() {
        val source = """@Local(ord"""
        val cursor = source.indexOf("ord") + "ord".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.LOCAL, context.annotation)
        assertEquals(AnnotationSlot.ATTRIBUTE, context.slot)
        assertEquals("ord", context.partialValue)
    }

    @Test
    fun extractsLocalAttributePrefixFromFullyQualifiedName() {
        val source = """@com.llamalad7.mixinextras.sugar.Local(ind"""
        val cursor = source.indexOf("ind") + "ind".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.LOCAL, context.annotation)
        assertEquals(AnnotationSlot.ATTRIBUTE, context.slot)
        assertEquals("ind", context.partialValue)
    }

    @Test
    fun extractsShareAttributePrefixFromSimpleName() {
        val source = """@Share(name"""
        val cursor = source.indexOf("name") + "name".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.SHARE, context.annotation)
        assertEquals(AnnotationSlot.ATTRIBUTE, context.slot)
        assertEquals("name", context.partialValue)
    }

    @Test
    fun extractsShareAttributePrefixFromFullyQualifiedName() {
        val source = """@com.llamalad7.mixinextras.sugar.Share(val"""
        val cursor = source.indexOf("val") + "val".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.SHARE, context.annotation)
        assertEquals(AnnotationSlot.ATTRIBUTE, context.slot)
        assertEquals("val", context.partialValue)
    }

    @Test
    fun localAttributeContextTracksExistingAttributes() {
        val source = """@Local(ordinal = 0, ind"""
        val cursor = source.indexOf("ind") + "ind".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.LOCAL, context.annotation)
        assertEquals(AnnotationSlot.ATTRIBUTE, context.slot)
        assertEquals(setOf("ordinal"), context.existingAttributes)
    }

    @Test
    fun extractsLocalEmptyOrdinalValueAtClosingParen() {
        val source = """@Local(ordinal = )"""
        val cursor = source.indexOf(')')
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.LOCAL, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals("ordinal", context.attributeName)
        assertEquals("", context.partialValue)
        assertEquals(cursor, context.valueStartOffset)
        assertEquals(cursor, context.valueEndOffset)
    }

    @Test
    fun extractsLocalEmptyIndexValueAtClosingParen() {
        val source = """@Local(index = )"""
        val cursor = source.indexOf(')')
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.LOCAL, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals("index", context.attributeName)
        assertEquals("", context.partialValue)
        assertEquals(cursor, context.valueStartOffset)
        assertEquals(cursor, context.valueEndOffset)
    }

    @Test
    fun rejectsLocalAttributePrefixWithNoValidCandidates() {
        val source = """@Local(xyz"""
        val cursor = source.indexOf("xyz") + "xyz".length
        assertNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
    }

    @Test
    fun rejectsShareAttributePrefixWithNoValidCandidates() {
        val source = """@Share(xyz"""
        val cursor = source.indexOf("xyz") + "xyz".length
        assertNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
    }

    @Test
    fun extractsDefinitionAttributePrefixFromSimpleName() {
        val source = """@Definition(met"""
        val cursor = source.indexOf("met") + "met".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.DEFINITION, context.annotation)
        assertEquals(AnnotationSlot.ATTRIBUTE, context.slot)
        assertEquals("met", context.partialValue)
    }

    @Test
    fun extractsDefinitionAttributePrefixFromFullyQualifiedName() {
        val source = """@com.llamalad7.mixinextras.expression.Definition(rem"""
        val cursor = source.indexOf("rem") + "rem".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.DEFINITION, context.annotation)
        assertEquals(AnnotationSlot.ATTRIBUTE, context.slot)
        assertEquals("rem", context.partialValue)
    }

    @Test
    fun extractsExpressionAttributePrefixFromSimpleName() {
        val source = """@Expression(val"""
        val cursor = source.indexOf("val") + "val".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.EXPRESSION, context.annotation)
        assertEquals(AnnotationSlot.ATTRIBUTE, context.slot)
        assertEquals("val", context.partialValue)
    }

    @Test
    fun extractsExpressionAttributePrefixFromFullyQualifiedName() {
        val source = """@com.llamalad7.mixinextras.expression.Expression(id"""
        val cursor = source.indexOf("id") + "id".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.EXPRESSION, context.annotation)
        assertEquals(AnnotationSlot.ATTRIBUTE, context.slot)
        assertEquals("id", context.partialValue)
    }

    @Test
    fun extractsDefinitionsAttributePrefixFromSimpleName() {
        val source = """@Definitions(val"""
        val cursor = source.indexOf("val") + "val".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.DEFINITIONS, context.annotation)
        assertEquals(AnnotationSlot.ATTRIBUTE, context.slot)
        assertEquals("val", context.partialValue)
    }

    @Test
    fun extractsExpressionsAttributePrefixFromFullyQualifiedName() {
        val source = """@com.llamalad7.mixinextras.expression.Expressions(val"""
        val cursor = source.indexOf("val") + "val".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.EXPRESSIONS, context.annotation)
        assertEquals(AnnotationSlot.ATTRIBUTE, context.slot)
        assertEquals("val", context.partialValue)
    }

    @Test
    fun definitionAttributeContextTracksExistingAttributes() {
        val source = """@Definition(id = "main", met"""
        val cursor = source.indexOf("met") + "met".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.DEFINITION, context.annotation)
        assertEquals(AnnotationSlot.ATTRIBUTE, context.slot)
        assertEquals(setOf("id"), context.existingAttributes)
        assertNull(context.attributeName)
    }

    @Test
    fun extractsDefinitionMethodValueWithAttributeName() {
        val source = """
            @Definition(method = "Lcom/example/Foo;run()V")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
            private void onDraw() {}
        """.trimIndent()
        val marker = "Lcom/example/Foo;run"
        val cursor = source.indexOf(marker) + "run".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.DEFINITION, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals("method", context.attributeName)
    }

    @Test
    fun extractsDefinitionFieldValueWithAttributeName() {
        val source = """
            @Definition(field = "Lcom/example/Foo;count:I")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
            private void onDraw() {}
        """.trimIndent()
        val marker = "Lcom/example/Foo;count"
        val cursor = source.indexOf(marker) + "count".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.DEFINITION, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals("field", context.attributeName)
    }

    @Test
    fun extractsNestedDefinitionFieldValueInDefinitionsArrayWithAttributeName() {
        val source = """
            @Definitions({
                @Definition(method = "Lcom/example/Foo;first()V"),
                @Definition(field = "Lcom/example/Foo;count:I")
            })
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
            private void onDraw() {}
        """.trimIndent()
        val marker = "Lcom/example/Foo;count"
        val cursor = source.indexOf(marker) + "count".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.DEFINITION, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals("field", context.attributeName)
    }

    @Test
    fun extractsFullyQualifiedDefinitionMethodValueWithAttributeName() {
        val source = """
            @com.llamalad7.mixinextras.expression.Definition(method = "Ljava/lang/String;trim()Ljava/lang/String;")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
            private void onDraw() {}
        """.trimIndent()
        val marker = "Ljava/lang/String;trim"
        val cursor = source.indexOf(marker) + "trim".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.DEFINITION, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals("method", context.attributeName)
    }

    @Test
    fun definitionShorthandValueKeepsAttributeNameNull() {
        val source = """@Definition("Lcom/example/Foo;run()V")"""
        val cursor = source.indexOf("run") + "run".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.DEFINITION, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertNull(context.attributeName)
    }

    @Test
    fun rejectsDefinitionAttributePrefixWithNoValidCandidates() {
        val source = """@Definition(xyz"""
        val cursor = source.indexOf("xyz") + "xyz".length
        assertNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
    }

    @Test
    fun ignoresFakeDefinitionInLineComment() {
        val source = """
            // @Definition(method = "fake")
            @Definition(met
        """.trimIndent()
        val fakeCursor = source.indexOf("fake") + 2
        assertNull(AnnotationContextExtractor.extractAtOffset(source, fakeCursor))
    }

    @Test
    fun ignoresFakeDefinitionInBlockComment() {
        val source = """
            /* @Definition(method = "fake") */
            @Definition(met
        """.trimIndent()
        val fakeCursor = source.indexOf("fake") + 2
        assertNull(AnnotationContextExtractor.extractAtOffset(source, fakeCursor))
    }

    @Test
    fun ignoresFakeDefinitionInStringLiteral() {
        val source = """
            note = "@Definition(method = \"fake\")",
            @Definition(met
        """.trimIndent()
        val fakeCursor = source.indexOf("fake") + 2
        assertNull(AnnotationContextExtractor.extractAtOffset(source, fakeCursor))
    }

    @Test
    fun extractsNearbyRealDefinitionAfterCommentAndStringFakes() {
        val source = """
            // @Definition(method = "decoy")
            note = "@Definition(method = \"decoy\")",
            @Definition(met
        """.trimIndent()
        val cursor = source.lastIndexOf("met") + "met".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.DEFINITION, context.annotation)
        assertEquals(AnnotationSlot.ATTRIBUTE, context.slot)
        assertEquals("met", context.partialValue)
    }

    @Test
    fun rejectsExpressionAttributePrefixWithNoValidCandidates() {
        val source = """@Expression(xyz"""
        val cursor = source.indexOf("xyz") + "xyz".length
        assertNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
    }

    @Test
    fun extractsNestedConstantAttributePrefixFromModifyConstant() {
        val source = """@ModifyConstant(method = "tick", constant = @Constant(nul))"""
        val cursor = source.indexOf("nul") + "nul".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.CONSTANT, context.annotation)
        assertEquals(AnnotationSlot.ATTRIBUTE, context.slot)
        assertEquals("nul", context.partialValue)
    }

    @Test
    fun extractsNestedSliceAttributePrefixFromInject() {
        val source = """@Inject(method = "tick", slice = @Slice(fr))"""
        val cursor = source.indexOf("fr") + "fr".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.SLICE, context.annotation)
        assertEquals(AnnotationSlot.ATTRIBUTE, context.slot)
        assertEquals("fr", context.partialValue)
    }

    @Test
    fun expressionShorthandUsesCurrentTokenReplacementRange() {
        val source = """@Expression("th")"""
        val tokenStart = source.indexOf("th")
        val cursor = tokenStart + "th".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.EXPRESSION, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals("th", context.partialValue)
        assertEquals(tokenStart, context.valueStartOffset)
        assertEquals(tokenStart + "th".length, context.valueEndOffset)
    }

    @Test
    fun expressionNamedValueUsesCurrentTokenReplacementRange() {
        val source = """@Expression(value = "a + th")"""
        val tokenStart = source.indexOf("th")
        val cursor = tokenStart + "th".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.EXPRESSION, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals("th", context.partialValue)
        assertEquals(tokenStart, context.valueStartOffset)
        assertEquals(tokenStart + "th".length, context.valueEndOffset)
    }

    @Test
    fun expressionsArrayElementUsesCurrentTokenReplacementRange() {
        val source = """@Expressions(value = { "a + th", "other" })"""
        val tokenStart = source.indexOf("th")
        val cursor = tokenStart + "th".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.EXPRESSIONS, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals("th", context.partialValue)
        assertEquals(tokenStart, context.valueStartOffset)
        assertEquals(tokenStart + "th".length, context.valueEndOffset)
    }

    @Test
    fun expressionCursorInMiddleOfClosedTokenReplacesFullTokenNotWholeLiteral() {
        val source = """@Expression(value = "return x")"""
        val tokenStart = source.indexOf("return")
        val cursor = tokenStart + 3
        val delimiterOffset = source.indexOf(' ', tokenStart)
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.EXPRESSION, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals("ret", context.partialValue)
        assertEquals(tokenStart, context.valueStartOffset)
        assertEquals(tokenStart + "return".length, context.valueEndOffset)
        assertEquals(delimiterOffset, context.valueEndOffset)
    }

    @Test
    fun expressionClosedTokenEndsBeforePunctuationDelimiter() {
        val source = """@Expression(value = "return;")"""
        val tokenStart = source.indexOf("return")
        val cursor = tokenStart + 3
        val delimiterOffset = source.indexOf(';')
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.EXPRESSION, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals("ret", context.partialValue)
        assertEquals(tokenStart, context.valueStartOffset)
        assertEquals(tokenStart + "return".length, context.valueEndOffset)
        assertEquals(delimiterOffset, context.valueEndOffset)
    }

    @Test
    fun expressionCursorBeforeEscapeAfterCursorCompletesTokenEnd() {
        val source = """@Expression(value = "ret\165rn")"""
        val tokenStart = source.indexOf("ret")
        val cursor = tokenStart + 3
        val escapeStart = source.indexOf("\\165", tokenStart)
        val tokenEndFile = escapeStart + "\\165".length + "rn".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.EXPRESSION, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals("ret", context.partialValue)
        assertEquals("return", context.decodedPartialValue)
        assertEquals(tokenStart, context.valueStartOffset)
        assertEquals(tokenEndFile, context.valueEndOffset)
    }

    @Test
    fun expressionOctalEscapeExposesDecodedIdentifierWhilePreservingRawOffsets() {
        val source = """@Expression("\144rawCall")"""
        val escapeStart = source.indexOf("\\144")
        val cursor = escapeStart + 2
        val tokenEndFile = source.indexOf("rawCall") + "rawCall".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.EXPRESSION, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals("drawCall", context.decodedPartialValue)
        assertEquals(escapeStart, context.valueStartOffset)
        assertEquals(tokenEndFile, context.valueEndOffset)
        assertEquals(source.substring(escapeStart, cursor), context.partialValue)
    }

    @Test
    fun expressionUnicodeEscapeExposesDecodedIdentifierWhilePreservingRawOffsets() {
        val source = """@Expression("\u0064rawCall")"""
        val escapeStart = source.indexOf("\\u0064")
        val cursor = escapeStart + 2
        val tokenEndFile = source.indexOf("rawCall") + "rawCall".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.EXPRESSION, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals("drawCall", context.decodedPartialValue)
        assertEquals(DecodedExpressionPrefix(prefix = "d", cursor = 1), context.decodedExpressionPrefix)
        assertEquals(escapeStart, context.valueStartOffset)
        assertEquals(tokenEndFile, context.valueEndOffset)
        assertEquals(source.substring(escapeStart, cursor), context.partialValue)
    }

    @Test
    fun expressionOctalEscapeCursorOnSlashMapsToDecodedTokenStart() {
        val source = """@Expression("\144rawCall")"""
        val escapeStart = source.indexOf("\\144")
        val tokenEndFile = source.indexOf("rawCall") + "rawCall".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, escapeStart))
        assertEquals("", context.partialValue)
        assertEquals("drawCall", context.decodedPartialValue)
        assertEquals(escapeStart, context.valueStartOffset)
        assertEquals(tokenEndFile, context.valueEndOffset)
    }

    @Test
    fun expressionOctalEscapeCursorOnFirstOctalDigitMapsToDecodedTokenStart() {
        val source = """@Expression("\144rawCall")"""
        val escapeStart = source.indexOf("\\144")
        val tokenEndFile = source.indexOf("rawCall") + "rawCall".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, escapeStart + 1))
        assertEquals("\\", context.partialValue)
        assertEquals("drawCall", context.decodedPartialValue)
        assertEquals(escapeStart, context.valueStartOffset)
        assertEquals(tokenEndFile, context.valueEndOffset)
    }

    @Test
    fun expressionOctalEscapeCursorImmediatelyAfterEscapeUsesDecodedSuffix() {
        val source = """@Expression("\144rawCall")"""
        val escapeStart = source.indexOf("\\144")
        val cursor = escapeStart + "\\144".length
        val tokenEndFile = source.indexOf("rawCall") + "rawCall".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals("\\144", context.partialValue)
        assertEquals("drawCall", context.decodedPartialValue)
        assertEquals(escapeStart, context.valueStartOffset)
        assertEquals(tokenEndFile, context.valueEndOffset)
    }

    @Test
    fun expressionIncompleteUnclosedStringUsesCurrentTokenReplacementRange() {
        val source = """@Expression("ret"""
        val tokenStart = source.indexOf("ret")
        val cursor = tokenStart + "ret".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.EXPRESSION, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals("ret", context.partialValue)
        assertEquals(tokenStart, context.valueStartOffset)
        assertEquals(tokenStart + "ret".length, context.valueEndOffset)
    }

    @Test
    fun expressionAfterDotUsesEmptyTokenAtCursor() {
        val source = """@Expression("this.")"""
        val cursor = source.indexOf('.') + 1
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.EXPRESSION, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals("", context.partialValue)
        assertEquals(cursor, context.valueStartOffset)
        assertEquals(cursor, context.valueEndOffset)
        assertEquals(ExpressionCompletionPosition.AFTER_DOT, context.expressionCompletionPosition)
        assertEquals(
            DecodedExpressionPrefix(prefix = "this.", cursor = 5),
            context.decodedExpressionPrefix,
        )
    }

    @Test
    fun expressionAfterDotDecodedPrefixUsesDecodedReceiverSpelling() {
        val source = """@Expression("\164his.")"""
        val cursor = source.indexOf('.') + 1
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.EXPRESSION, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals("", context.partialValue)
        assertEquals(cursor, context.valueStartOffset)
        assertEquals(cursor, context.valueEndOffset)
        assertEquals(
            DecodedExpressionPrefix(prefix = "this.", cursor = 5),
            context.decodedExpressionPrefix,
        )
    }

    @Test
    fun expressionsArrayElementProvidesDecodedExpressionPrefix() {
        val source = """@Expressions({ "this.foo.", "other" })"""
        val cursor = source.indexOf("this.foo.") + "this.foo.".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.EXPRESSIONS, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals("", context.partialValue)
        assertEquals(cursor, context.valueStartOffset)
        assertEquals(cursor, context.valueEndOffset)
        assertEquals(
            DecodedExpressionPrefix(prefix = "this.foo.", cursor = 9),
            context.decodedExpressionPrefix,
        )
    }

    @Test
    fun expressionNamedValueProvidesDecodedExpressionPrefix() {
        val source = """@Expression(value = "this.foo.")"""
        val cursor = source.indexOf('.') + 1
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.EXPRESSION, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals("value", context.attributeName)
        assertEquals("", context.partialValue)
        assertEquals(cursor, context.valueStartOffset)
        assertEquals(cursor, context.valueEndOffset)
        assertEquals(
            DecodedExpressionPrefix(prefix = "this.", cursor = 5),
            context.decodedExpressionPrefix,
        )
    }

    @Test
    fun expressionIncompleteStringProvidesDecodedExpressionPrefix() {
        val source = """@Expression("this.fo"""
        val tokenStart = source.indexOf("this.fo")
        val cursor = tokenStart + "this.fo".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.EXPRESSION, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals("fo", context.partialValue)
        assertEquals(tokenStart + "this.".length, context.valueStartOffset)
        assertEquals(tokenStart + "this.fo".length, context.valueEndOffset)
        assertEquals(
            DecodedExpressionPrefix(prefix = "this.fo", cursor = 7),
            context.decodedExpressionPrefix,
        )
    }

    @Test
    fun expressionIdAttributeDoesNotProvideDecodedExpressionPrefix() {
        val source = """@Expression(id = "main")"""
        val contentStart = source.indexOf("main")
        val cursor = contentStart + 2
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.EXPRESSION, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals("ma", context.partialValue)
        assertEquals(contentStart, context.valueStartOffset)
        assertEquals(contentStart + "main".length, context.valueEndOffset)
        assertNull(context.decodedExpressionPrefix)
    }

    @Test
    fun expressionAfterMethodReferenceUsesEmptyTokenAtCursor() {
        val source = """@Expression("foo::")"""
        val cursor = source.indexOf("::") + 2
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.EXPRESSION, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals("", context.partialValue)
        assertEquals(cursor, context.valueStartOffset)
        assertEquals(cursor, context.valueEndOffset)
        assertEquals(ExpressionCompletionPosition.AFTER_METHOD_REFERENCE, context.expressionCompletionPosition)
    }

    @Test
    fun expressionValuePrefixSetsCompletionPosition() {
        val source = """@Expression("a + th")"""
        val tokenStart = source.indexOf("th")
        val cursor = tokenStart + "th".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(ExpressionCompletionPosition.VALUE_START, context.expressionCompletionPosition)
    }

    @Test
    fun fullyQualifiedExpressionValuePrefixSetsCompletionPosition() {
        val source = """@com.llamalad7.mixinextras.expression.Expression("lengthCall")"""
        val tokenStart = source.indexOf("lengthCall")
        val cursor = tokenStart + 3
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.EXPRESSION, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals("lengthCall", context.decodedPartialValue)
        assertEquals(ExpressionCompletionPosition.VALUE_START, context.expressionCompletionPosition)
    }

    @Test
    fun expressionBlockBodyPrefixSetsStatementStartPosition() {
        val source = """@Expression("{ ret")"""
        val tokenStart = source.indexOf("ret")
        val cursor = tokenStart + "ret".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(ExpressionCompletionPosition.STATEMENT_START, context.expressionCompletionPosition)
    }

    @Test
    fun expressionIdAttributeRemainsWholeStringReplacement() {
        val source = """@Expression(id = "main")"""
        val contentStart = source.indexOf("main")
        val cursor = contentStart + 2
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals(MixinAnnotation.EXPRESSION, context.annotation)
        assertEquals(AnnotationSlot.VALUE, context.slot)
        assertEquals("ma", context.partialValue)
        assertEquals(contentStart, context.valueStartOffset)
        assertEquals(contentStart + "main".length, context.valueEndOffset)
        assertEquals(null, context.expressionCompletionPosition)
    }

    @Test
    fun expressionEscapeBoundaryDoesNotCreateFalseTokenStart() {
        val source = """@Expression("\\\\th")"""
        val tokenStart = source.indexOf("th")
        val cursor = tokenStart + "th".length
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, cursor))
        assertEquals("th", context.partialValue)
        assertEquals(tokenStart, context.valueStartOffset)
        assertEquals(tokenStart + "th".length, context.valueEndOffset)
    }

    @Test
    fun atTargetReplacementRegression() {
        val source = """@Inject(method = "tick", at = @At(value = "INVOKE", target = "Lnet/minecraft/"))"""
        val partial = "Lnet/minecraft/"
        val offset = source.indexOf(partial) + partial.length
        val context = AnnotationContextExtractor.extractAtOffset(source, offset)
        assertNotNull(context)
        assertEquals(MixinAnnotation.AT, context.annotation)
        assertEquals(AnnotationSlot.TARGET, context.slot)
        assertEquals(partial, context.partialValue)
        assertEquals(source.indexOf(partial), context.valueStartOffset)
        assertEquals(source.indexOf(partial) + partial.length, context.valueEndOffset)
    }

    private fun extract(source: String, line: Int, character: Int): AnnotationContext? =
        AnnotationContextExtractor.extract(source, line, character)
}
