package io.github.mcdev.core.mixin

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
    }
}
