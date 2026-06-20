package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.mixin.AnnotationContextExtractor
import io.github.mcdev.core.mixin.AnnotationSlot
import io.github.mcdev.core.mixin.MixinAnnotation
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull

class MixinExtrasAnnotationContextTest {
    @Test
    fun extractsModifyExpressionValueMethodSlot() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            class ExampleMixin {
                @ModifyExpressionValue(method = "dra", at = @At("HEAD"))
                private void handler() {}
            }
        """.trimIndent()
        val offset = source.indexOf("dra") + 3
        val context = AnnotationContextExtractor.extractAtOffset(source, offset)
        assertNotNull(context)
        assertEquals(MixinAnnotation.MODIFY_EXPRESSION_VALUE, context.annotation)
        assertEquals(AnnotationSlot.METHOD, context.slot)
    }

    @Test
    fun extractsModifyReturnValueMethodSlot() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            class ExampleMixin {
                @ModifyReturnValue(method = "draw", at = @At("RETURN"))
                private void handler() {}
            }
        """.trimIndent()
        val offset = source.indexOf("draw") + 4
        val context = AnnotationContextExtractor.extractAtOffset(source, offset)
        assertNotNull(context)
        assertEquals(MixinAnnotation.MODIFY_RETURN_VALUE, context.annotation)
    }

    @Test
    fun extractsWrapOperationMethodSlot() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            class ExampleMixin {
                @WrapOperation(method = "draw", at = @At(value = "INVOKE", target = ""))
                private void handler() {}
            }
        """.trimIndent()
        val offset = source.indexOf("draw") + 4
        val context = AnnotationContextExtractor.extractAtOffset(source, offset)
        assertNotNull(context)
        assertEquals(MixinAnnotation.WRAP_OPERATION, context.annotation)
    }

    @Test
    fun extractsAtTargetContextWithParentMixinExtrasMethod() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            class ExampleMixin {
                @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "length"))
                private void handler() {}
            }
        """.trimIndent()
        val offset = source.indexOf("length") + 6
        val context = AnnotationContextExtractor.extractAtOffset(source, offset)
        assertNotNull(context)
        assertEquals(MixinAnnotation.AT, context.annotation)
        assertEquals(AnnotationSlot.TARGET, context.slot)
        assertEquals("draw(Ljava/lang/String;FF)V", context.injectMethodName)
        assertEquals("INVOKE", context.atValue)
    }

    @Test
    fun extractsWrapWithConditionMethodSlot() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            class ExampleMixin {
                @WrapWithCondition(method = "draw", at = @At("HEAD"))
                private void handler() {}
            }
        """.trimIndent()
        val offset = source.indexOf("draw") + 4
        val context = AnnotationContextExtractor.extractAtOffset(source, offset)
        assertNotNull(context)
        assertEquals(MixinAnnotation.WRAP_WITH_CONDITION, context.annotation)
    }

    @Test
    fun extractsWrapMethodMethodSlot() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            class ExampleMixin {
                @WrapMethod(method = "draw")
                private void handler() {}
            }
        """.trimIndent()
        val offset = source.indexOf("draw") + 4
        val context = AnnotationContextExtractor.extractAtOffset(source, offset)
        assertNotNull(context)
        assertEquals(MixinAnnotation.WRAP_METHOD, context.annotation)
    }
}
