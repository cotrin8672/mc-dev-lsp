package io.github.mcdev.core.mixin

import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.completion.McCompletionKind
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class SemanticCompletionContextExtractorTest {
    private val facade = MixinServiceFacade(FakeClassIndex(), FakeBytecodeIndex())

    @Test
    fun completesInjectMethodFromSemanticContextWithoutFallback() {
        val fixture = markedSource(
            """
            package com.example.mixin;

            @Mixin(net.minecraft.client.MinecraftClient.class)
            class TargetMixin {
                @Inject(method = "/*caret*/", at = @At("HEAD"))
                private void injected(CallbackInfo ci) {}
            }
            """.trimIndent(),
        )

        val result = facade.completeWithDebug(
            request = request(fixture),
            options = MixinCompletionOptions(),
        )

        assertTrue(result.items.any { it.insertText == "tick" })
        assertTrue(result.debug.semanticContextFound)
        assertFalse(result.debug.fallbackAnnotationContextUsed)
        assertEquals(null, result.debug.fallbackAnnotationContextReason)
        assertEquals(null, result.debug.zeroItemReason)
    }

    @Test
    fun injectMethodTextEditStartsAfterOpenQuote() {
        val fixture = markedSource(
            """
            package com.example.mixin;

            @Mixin(net.minecraft.world.item.Item.class)
            class ItemMixin {
                @Inject(method = "isF/*caret*/", at = @At("HEAD"))
                private void injected(CallbackInfoReturnable<Boolean> cir) {}
            }
            """.trimIndent(),
        )
        val model = MixinSemanticModelParser.parse(fixture.source, "file:///ItemMixin.java")
        val completionContext = SemanticCompletionContextExtractor.extract(
            source = fixture.source,
            line = fixture.line,
            character = fixture.character,
            model = model,
        )
        val annotationContext = SemanticCompletionContextExtractor.toAnnotationContext(
            source = fixture.source,
            line = fixture.line,
            character = fixture.character,
            model = model,
            context = completionContext!!,
        )!!

        val lineStart = fixture.source.lastIndexOf('\n', fixture.source.indexOf("isF")).let { if (it < 0) 0 else it + 1 }
        assertEquals(fixture.source.indexOf("isF"), annotationContext.valueStartOffset)
        assertEquals(fixture.source.indexOf("isF") + "isF".length, annotationContext.valueEndOffset)
        assertEquals("isF", annotationContext.partialValue)
        assertEquals("    @Inject(method = \"isF", fixture.source.substring(lineStart, annotationContext.valueEndOffset))
    }

    @Test
    fun shadowMemberContextUsesDeclarationNameAndReplacesTheWholeToken() {
        val fixture = markedSource(
            """
            @Mixin(net.minecraft.client.MinecraftClient.class)
            class ItemMixin {
                @Shadow private net.minecraft.client.gui.screen.Screen cu/*caret*/rrentScreen;
            }
            """.trimIndent(),
        )
        val parsedModel = MixinSemanticModelParser.parse(fixture.source)
        val nameStart = fixture.source.indexOf("currentScreen")
        val nameEnd = nameStart + "currentScreen".length
        val preciseNameStart = offsetToLineCharacter(fixture.source, nameStart)
        val preciseNameEnd = offsetToLineCharacter(fixture.source, nameEnd)
        val model = parsedModel.copy(
            members = parsedModel.members.map { member ->
                member.copy(
                    nameRange = McTextRange(
                        McTextPosition(preciseNameStart.first, preciseNameStart.second),
                        McTextPosition(preciseNameEnd.first, preciseNameEnd.second),
                    ),
                )
            },
        )
        val context = assertIs<MixinCompletionContext.MemberName>(
            SemanticCompletionContextExtractor.extract(
                source = fixture.source,
                line = fixture.line,
                character = fixture.character,
                model = model,
            ),
        )

        assertEquals("cu", context.partialValue)
        val annotationContext = assertNotNull(
            SemanticCompletionContextExtractor.toAnnotationContext(
                source = fixture.source,
                line = fixture.line,
                character = fixture.character,
                model = model,
                context = context,
            ),
        )
        assertEquals(nameStart, annotationContext.valueStartOffset)
        assertEquals(nameEnd, annotationContext.valueEndOffset)
        assertEquals(AnnotationSlot.SHADOW_MEMBER, annotationContext.slot)
    }

    @Test
    fun semanticMemberContextDoesNotStealShadowAttributes() {
        for (value in listOf("prefix = \"sha/*caret*/dow\"", "remap = tr/*caret*/ue")) {
            val fixture = markedSource(
                """
                @Mixin(net.minecraft.client.MinecraftClient.class)
                class ItemMixin {
                    @Shadow($value)
                    private net.minecraft.client.gui.screen.Screen currentScreen;
                }
                """.trimIndent(),
            )
            val model = MixinSemanticModelParser.parse(fixture.source)
            assertNull(
                SemanticCompletionContextExtractor.extract(
                    source = fixture.source,
                    line = fixture.line,
                    character = fixture.character,
                    model = model,
                ),
            )
            val result = facade.completeWithDebug(request(fixture))
            val expectedKind = if (value.startsWith("prefix")) "PREFIX" else "REMAP"
            assertEquals(expectedKind, result.debug.completionContextKind)
        }
    }

    @Test
    fun shadowAndOverwriteCompletionUseDeclaredMemberNames() {
        val shadowFixture = markedSource(
            """
            @Mixin(MinecraftClient.class)
            class ItemMixin {
                @Shadow private net.minecraft.client.gui.screen.Screen cu/*caret*/rrentScreen;
            }
            """.trimIndent(),
        )
        val shadowItems = facade.complete(request(shadowFixture))
        assertTrue(shadowItems.any { it.insertText == "currentScreen" })

        val overwriteFixture = markedSource(
            """
            @Mixin(MinecraftClient.class)
            class ItemMixin {
                @Overwrite public void ti/*caret*/ck() {}
            }
            """.trimIndent(),
        )
        val overwriteItems = facade.complete(request(overwriteFixture))
        assertTrue(overwriteItems.any { it.insertText == "tick" })
    }

    @Test
    fun overwriteCompletionExcludesConstructorsAndClassInitializersFromEmptyPrefix() {
        val owner = "net/minecraft/client/MinecraftClient"
        val methods = FakeClassIndex.defaultMethods().toMutableMap()
        methods[owner] = listOf(
            MethodIndexEntry("<init>", "(I)V", false, "<init>(int): void"),
            MethodIndexEntry("<clinit>", "()V", true, "<clinit>(): void"),
            MethodIndexEntry("tick", "()V", false, "tick(): void"),
        )
        val overwriteFacade = MixinServiceFacade(
            classIndex = FakeClassIndex(methods = methods),
            bytecodeIndex = FakeBytecodeIndex(),
        )
        val fixture = markedSource(
            """
            @Mixin(MinecraftClient.class)
            class ItemMixin {
                @Overwrite public void /*caret*/tick() {}
            }
            """.trimIndent(),
        )

        val items = overwriteFacade.complete(request(fixture))

        assertTrue(items.any { it.insertText == "tick" })
        assertTrue(items.none { it.insertText == "<init>" })
        assertTrue(items.none { it.insertText == "<clinit>" })
    }

    @Test
    fun shadowCompletionMatchesTheDeclaredMemberKind() {
        val fieldFixture = markedSource(
            """
            @Mixin(MinecraftClient.class)
            class ItemMixin {
                @Shadow private net.minecraft.client.gui.screen.Screen /*caret*/currentScreen;
            }
            """.trimIndent(),
        )
        val fieldItems = facade.complete(request(fieldFixture))
        assertTrue(fieldItems.any { it.kind == McCompletionKind.FIELD && it.insertText == "currentScreen" })
        assertFalse(fieldItems.any { it.kind == McCompletionKind.VALUE })

        val methodFixture = markedSource(
            """
            @Mixin(MinecraftClient.class)
            class ItemMixin {
                @Shadow public void /*caret*/tick() {}
            }
            """.trimIndent(),
        )
        val methodItems = facade.complete(request(methodFixture))
        assertTrue(methodItems.any { it.kind == McCompletionKind.VALUE && it.insertText == "tick" })
        assertFalse(methodItems.any { it.kind == McCompletionKind.FIELD })
    }

    @Test
    fun shadowCompletionKeepsMethodKindWhenDescriptorIsUnavailable() {
        val fixture = markedSource(
            """
            @Mixin(MinecraftClient.class)
            class ItemMixin {
                @Shadow public void /*caret*/tick(int value) {}
            }
            """.trimIndent(),
        )
        val parsed = MixinSemanticModelParser.parse(fixture.source)
        val model = parsed.copy(
            members = parsed.members.map { member ->
                member.copy(
                    returnDescriptor = null,
                    parameterDescriptors = emptyList(),
                    methodDescriptor = null,
                    isMethod = true,
                )
            },
        )

        val request = request(fixture).copy(semanticModel = model)
        val context = SemanticCompletionContextExtractor.extract(
            source = fixture.source,
            line = fixture.line,
            character = fixture.character,
            model = model,
        )
        val annotationContext = SemanticCompletionContextExtractor.toAnnotationContext(
            source = fixture.source,
            line = fixture.line,
            character = fixture.character,
            model = model,
            context = assertIs<MixinCompletionContext.MemberName>(context),
        )

        assertEquals(true, annotationContext?.shadowMemberIsMethod)
        val items = facade.complete(request)
        assertTrue(items.any { it.kind == McCompletionKind.VALUE && it.insertText == "tick" })
        assertFalse(items.any { it.kind == McCompletionKind.FIELD })
    }

    @Test
    fun shadowMemberSelectionDoesNotStealSameLineSiblingOrAnnotationValue() {
        val source = """
            @Mixin(MinecraftClient.class)
            class ItemMixin { @Shadow(prefix = "counter") private int counter; @Shadow private int tick; }
        """.trimIndent()
        val model = MixinSemanticModelParser.parse(source)
        val prefixOffset = source.indexOf("counter") + 2
        val (prefixLine, prefixCharacter) = offsetToLineCharacter(source, prefixOffset)
        assertNull(SemanticCompletionContextExtractor.extract(source, prefixLine, prefixCharacter, model))

        val siblingOffset = source.lastIndexOf("tick") + 2
        val (siblingLine, siblingCharacter) = offsetToLineCharacter(source, siblingOffset)
        val context = assertIs<MixinCompletionContext.MemberName>(
            SemanticCompletionContextExtractor.extract(source, siblingLine, siblingCharacter, model),
        )
        assertEquals("tick", context.member.javaName)
        assertEquals("tick", source.substring(context.valueStartOffset, context.valueEndOffset))
    }

    @Test
    fun memberCompletionStopsAtNextMemberAndMethodBody() {
        val source = """
            @Mixin(MinecraftClient.class)
            class ItemMixin {
                @Shadow private net.minecraft.client.gui.screen.Screen currentScreen;
                @Shadow public void tick() { int ordinary = 0; }
                String ordinaryField;
            }
        """.trimIndent()
        val model = MixinSemanticModelParser.parse(source)
        for (needle in listOf("ordinary =", "ordinaryField")) {
            val offset = source.indexOf(needle) + needle.length
            val (line, character) = offsetToLineCharacter(source, offset)
            assertNull(
                SemanticCompletionContextExtractor.extract(
                    source = source,
                    line = line,
                    character = character,
                    model = model,
                ),
                needle,
            )
            assertTrue(facade.complete(MixinFacadeRequest(source, line, character)).isEmpty(), needle)
        }
    }

    @Test
    fun completesAtTargetFromRecoveredSemanticContextWithoutFallback() {
        val fixture = markedSource(
            """
            package com.example.mixin;

            @Mixin(net.minecraft.client.MinecraftClient.class)
            class TargetMixin {
                @Inject(
                    method = {"tick", "render"},
                    at = {@At("HEAD"), @At(value = "INVOKE", target = "/*caret*/")}
                )
                private void injected(CallbackInfo ci) {}
            }
            """.trimIndent(),
        )

        val result = facade.completeWithDebug(
            request = request(fixture),
            options = MixinCompletionOptions(),
        )

        assertTrue(result.items.any { it.insertText.contains("draw") })
        assertTrue(result.debug.semanticContextFound)
        assertFalse(result.debug.fallbackAnnotationContextUsed)
        assertEquals(null, result.debug.fallbackAnnotationContextReason)
        assertEquals(null, result.debug.zeroItemReason)
    }

    private fun request(fixture: MarkedSource): MixinFacadeRequest =
        MixinFacadeRequest(
            bufferText = fixture.source,
            line = fixture.line,
            character = fixture.character,
            documentUri = "file:///TargetMixin.java",
            semanticModel = MixinSemanticModelParser.parse(fixture.source, "file:///TargetMixin.java"),
        )

    private fun offsetToLineCharacter(source: String, offset: Int): Pair<Int, Int> {
        var line = 0
        var character = 0
        for (index in 0 until offset) {
            if (source[index] == '\n') {
                line++
                character = 0
            } else {
                character++
            }
        }
        return line to character
    }

    private data class MarkedSource(
        val source: String,
        val line: Int,
        val character: Int,
    )

    private fun markedSource(sourceWithMarker: String): MarkedSource {
        val marker = "/*caret*/"
        val offset = sourceWithMarker.indexOf(marker)
        require(offset >= 0) { "missing caret marker" }
        val source = sourceWithMarker.replace(marker, "")
        val before = sourceWithMarker.substring(0, offset)
        val line = before.count { it == '\n' }
        val character = before.substringAfterLast('\n').length
        return MarkedSource(source, line, character)
    }
}
