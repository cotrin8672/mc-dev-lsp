package io.github.mcdev.core.mixin

import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mixinextras.ExpressionContext
import io.github.mcdev.core.mixinextras.MixinExtrasDefinitionIndex
import io.github.mcdev.core.mixinextras.MixinExtrasExpressionIndex
import io.github.mcdev.core.mixinextras.ResolvedMixinExtrasContext
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class MixinSemanticModelParserTest {
    @Test
    fun exposesFallbackSemanticModelWithMembers() {
        val source = """
            package com.example.mixin;
            @Mixin(SimpleTarget.class)
            class ExampleMixin {
                @Shadow private int counter;
                @Invoker("tick") abstract void invokeTick();
                @Inject(method = "draw(I)V", at = @At(value = "INVOKE", target = "Lcom/example/Target;tick()V"))
                private void draw() {}
            }
        """.trimIndent()

        val model = MixinSemanticModelParser.parse(source, "file:///ExampleMixin.java")

        assertEquals("file:///ExampleMixin.java", model.sourceUri)
        assertEquals("com.example.mixin", model.packageName)
        assertEquals("com.example.mixin.ExampleMixin", model.qualifiedName)
        assertEquals(ParseSource.HAND_WRITTEN_FALLBACK, model.parseSource)
        assertTrue(model.targets.any { it.internalName == "SimpleTarget" })
        assertTrue(model.members.any { it.annotationKind == MixinMemberAnnotationKind.SHADOW && it.javaName == "counter" })
        assertTrue(model.members.any { it.annotationKind == MixinMemberAnnotationKind.INVOKER && it.javaName == "invokeTick" })
        assertTrue(model.injectors.any { injector ->
            injector.methodSelectors.any { it.name == "draw" && it.descriptor == "(I)V" } &&
                injector.atSelectors.any { it.value == "INVOKE" && it.target == "Lcom/example/Target;tick()V" }
        })
        assertTrue(model.resolvedMixinExtrasContexts.isEmpty())
    }

    @Test
    fun retainsExplicitResolvedMixinExtrasContexts() {
        val handlerRange = McTextRange(McTextPosition(1, 4), McTextPosition(1, 20))
        val context = ExpressionContext(
            expressionIndex = MixinExtrasExpressionIndex(),
            definitionIndex = MixinExtrasDefinitionIndex(),
        )
        val resolved = ResolvedMixinExtrasContext(handlerRange = handlerRange, context = context)

        val model = MixinClassModel(
            targets = emptyList(),
            injectors = emptyList(),
            resolvedMixinExtrasContexts = listOf(resolved),
        )

        assertEquals(listOf(resolved), model.resolvedMixinExtrasContexts)
    }
}
