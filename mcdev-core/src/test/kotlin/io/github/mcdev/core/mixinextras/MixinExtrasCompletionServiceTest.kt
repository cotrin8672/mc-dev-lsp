package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.mixin.AnnotationContext
import io.github.mcdev.core.mixin.AnnotationSlot
import io.github.mcdev.core.mixin.MixinAnnotation
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class MixinExtrasCompletionServiceTest {
    private val classIndex = MixinExtrasTestFixtures.classIndex
    private val bytecodeIndex = MixinExtrasTestFixtures.bytecodeIndex
    private val service = MixinExtrasCompletionService(classIndex, bytecodeIndex)

    @Test
    fun completesModifyExpressionValueMethod() {
        val context = methodContext(MixinAnnotation.MODIFY_EXPRESSION_VALUE, "dra", listOf("com/example/target/SimpleTarget"))
        val items = service.complete(context)
        assertTrue(items.any { it.insertText.startsWith("draw") })
        assertEquals("mixinextras.injectMethod", items.first().metadata.source)
    }

    @Test
    fun completesModifyReturnValueMethod() {
        val context = methodContext(MixinAnnotation.MODIFY_RETURN_VALUE, "draw", listOf("com/example/target/SimpleTarget"))
        val items = service.complete(context)
        assertTrue(items.any { it.insertText.startsWith("draw") })
    }

    @Test
    fun completesWrapOperationMethod() {
        val context = methodContext(MixinAnnotation.WRAP_OPERATION, "draw", listOf("com/example/target/SimpleTarget"))
        val items = service.complete(context)
        assertTrue(items.isNotEmpty())
    }

    @Test
    fun completesWrapWithConditionMethod() {
        val context = methodContext(MixinAnnotation.WRAP_WITH_CONDITION, "draw", listOf("com/example/target/SimpleTarget"))
        val items = service.complete(context)
        assertEquals(1, items.size)
    }

    @Test
    fun completesWrapMethodTargetMethod() {
        val context = methodContext(MixinAnnotation.WRAP_METHOD, "draw", listOf("com/example/target/SimpleTarget"))
        val items = service.complete(context)
        assertTrue(items.isNotEmpty())
    }

    @Test
    fun completesWrapOperationAtInvokeTarget() {
        val context = AnnotationContext(
            annotation = MixinAnnotation.AT,
            slot = AnnotationSlot.TARGET,
            partialValue = "",
            valueStartOffset = 0,
            valueEndOffset = 0,
            annotationStartOffset = 0,
            annotationEndOffset = 0,
            mixinTargetInternalNames = listOf("com/example/target/SimpleTarget"),
            injectMethodName = "draw",
            atValue = "INVOKE",
        )
        val items = service.complete(context)
        assertEquals("Ljava/lang/String;length()I", items.first().insertText)
    }

    @Test
    fun wrapOperationAtTargetFiltersByPartial() {
        val context = AnnotationContext(
            annotation = MixinAnnotation.AT,
            slot = AnnotationSlot.TARGET,
            partialValue = "length",
            valueStartOffset = 0,
            valueEndOffset = 6,
            annotationStartOffset = 0,
            annotationEndOffset = 0,
            mixinTargetInternalNames = listOf("com/example/target/SimpleTarget"),
            injectMethodName = "draw",
            atValue = "INVOKE",
        )
        val items = service.complete(context)
        assertTrue(items.isNotEmpty())
    }

    @Test
    fun returnsEmptyForNonMethodSlot() {
        val context = AnnotationContext(
            annotation = MixinAnnotation.MODIFY_EXPRESSION_VALUE,
            slot = AnnotationSlot.VALUE,
            partialValue = "HEAD",
            valueStartOffset = 0,
            valueEndOffset = 4,
            annotationStartOffset = 0,
            annotationEndOffset = 0,
            mixinTargetInternalNames = listOf("com/example/target/SimpleTarget"),
        )
        assertTrue(service.complete(context).isEmpty())
    }

    @Test
    fun returnsEmptyAtTargetWithoutInjectMethodName() {
        val context = AnnotationContext(
            annotation = MixinAnnotation.AT,
            slot = AnnotationSlot.TARGET,
            partialValue = "",
            valueStartOffset = 0,
            valueEndOffset = 0,
            annotationStartOffset = 0,
            annotationEndOffset = 0,
            mixinTargetInternalNames = listOf("com/example/target/SimpleTarget"),
            atValue = "INVOKE",
        )
        assertTrue(service.complete(context).isEmpty())
    }

    @Test
    fun completeAtTargetForWrapOperationUsesBytecodeIndex() {
        val context = AnnotationContext(
            annotation = MixinAnnotation.AT,
            slot = AnnotationSlot.TARGET,
            partialValue = "",
            valueStartOffset = 0,
            valueEndOffset = 0,
            annotationStartOffset = 0,
            annotationEndOffset = 0,
        )
        val items = service.completeAtTargetForWrapOperation(
            listOf("com/example/target/SimpleTarget"),
            "draw",
            "(Ljava/lang/String;FF)V",
            context,
        )
        assertEquals(1, items.size)
    }

    private fun methodContext(
        annotation: MixinAnnotation,
        partial: String,
        targets: List<String>,
    ) = AnnotationContext(
        annotation = annotation,
        slot = AnnotationSlot.METHOD,
        partialValue = partial,
        valueStartOffset = 0,
        valueEndOffset = partial.length,
        annotationStartOffset = 0,
        annotationEndOffset = 0,
        mixinTargetInternalNames = targets,
    )
}
