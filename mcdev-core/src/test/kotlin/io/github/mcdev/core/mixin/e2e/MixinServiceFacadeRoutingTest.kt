package io.github.mcdev.core.mixin.e2e

import io.github.mcdev.core.mixin.AnnotationContext
import io.github.mcdev.core.mixin.AnnotationSlot
import io.github.mcdev.core.mixin.MixinAnnotation
import io.github.mcdev.core.mixin.MixinCompletionOptions
import kotlin.test.Test
import kotlin.test.assertTrue

class MixinServiceFacadeRoutingTest {
    private val facade = MixinE2ETestSupport.fakeFacade()

    @Test
    fun routesShadowMemberCompletion() {
        val context = AnnotationContext(
            annotation = MixinAnnotation.SHADOW,
            slot = AnnotationSlot.SHADOW_MEMBER,
            partialValue = "cur",
            valueStartOffset = 0,
            valueEndOffset = 3,
            annotationStartOffset = 0,
            annotationEndOffset = 0,
            mixinTargetInternalNames = listOf("net/minecraft/client/MinecraftClient"),
        )
        val items = facade.complete(
            MixinE2ETestSupport.requestAt("@Shadow", "Shadow"),
            MixinCompletionOptions(),
        )
        val shadowFields = io.github.mcdev.core.mixin.ShadowValidationService(
            io.github.mcdev.core.mixin.FakeClassIndex(),
        ).completeFields(context.mixinTargetInternalNames, context.partialValue)
        assertTrue(shadowFields.any { it.name == "currentScreen" })
        assertTrue(items.isEmpty())
    }

    @Test
    fun routesAccessorFieldCompletionEndToEnd() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Accessor("cur")
                Screen getCurrentScreen();
            }
        """.trimIndent()
        val items = facade.complete(
            MixinE2ETestSupport.requestInAnnotationValue(source, "@Accessor", "cur"),
        )
        assertTrue(items.any { it.insertText == "currentScreen" })
    }

    @Test
    fun routesInvokerMethodCompletionEndToEnd() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Invoker("set")
                void invokeSetScreen(Screen screen);
            }
        """.trimIndent()
        val items = facade.complete(
            MixinE2ETestSupport.requestInAnnotationValue(source, "@Invoker", "set"),
        )
        assertTrue(items.any { it.insertText == "setScreen" })
    }
}
