package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class ResolvedMixinExtrasContextTest {
    @Test
    fun selectorPrefersUniqueAnnotationContainment() {
        val annotationRange = McTextRange(McTextPosition(2, 4), McTextPosition(2, 20))
        val handlerRange = McTextRange(McTextPosition(3, 4), McTextPosition(3, 40))
        val site = testSite(annotationRange = annotationRange, handlerRange = handlerRange)
        val byAnnotation = resolvedContext(
            handlerRange = McTextRange(McTextPosition(2, 0), McTextPosition(4, 0)),
        )
        val byHandlerOnly = resolvedContext(
            handlerRange = McTextRange(McTextPosition(3, 0), McTextPosition(4, 0)),
        )

        assertEquals(byAnnotation, selectResolvedMixinExtrasContext(site, listOf(byAnnotation, byHandlerOnly)))
    }

    @Test
    fun selectorFallsBackToUniqueHandlerOverlap() {
        val annotationRange = McTextRange(McTextPosition(1, 0), McTextPosition(1, 10))
        val handlerRange = McTextRange(McTextPosition(3, 4), McTextPosition(3, 40))
        val site = testSite(annotationRange = annotationRange, handlerRange = handlerRange)
        val match = resolvedContext(
            handlerRange = McTextRange(McTextPosition(3, 0), McTextPosition(3, 50)),
        )

        assertEquals(match, selectResolvedMixinExtrasContext(site, listOf(match)))
    }

    @Test
    fun selectorReturnsNullWhenAnnotationMatchesAreAmbiguous() {
        val annotationRange = McTextRange(McTextPosition(2, 4), McTextPosition(2, 20))
        val site = testSite(annotationRange = annotationRange)
        val first = resolvedContext(
            handlerRange = McTextRange(McTextPosition(2, 0), McTextPosition(4, 0)),
        )
        val second = resolvedContext(
            handlerRange = McTextRange(McTextPosition(2, 2), McTextPosition(4, 2)),
        )

        assertNull(selectResolvedMixinExtrasContext(site, listOf(first, second)))
    }

    @Test
    fun selectorReturnsNullWhenHandlerOverlapIsAmbiguous() {
        val handlerRange = McTextRange(McTextPosition(3, 4), McTextPosition(3, 40))
        val site = testSite(
            annotationRange = McTextRange(McTextPosition(0, 0), McTextPosition(0, 1)),
            handlerRange = handlerRange,
        )
        val first = resolvedContext(
            handlerRange = McTextRange(McTextPosition(3, 0), McTextPosition(3, 20)),
        )
        val second = resolvedContext(
            handlerRange = McTextRange(McTextPosition(3, 10), McTextPosition(3, 50)),
        )

        assertNull(selectResolvedMixinExtrasContext(site, listOf(first, second)))
    }

    private fun resolvedContext(
        handlerRange: McTextRange,
        context: ExpressionContext = emptyExpressionContext(),
    ): ResolvedMixinExtrasContext = ResolvedMixinExtrasContext(handlerRange = handlerRange, context = context)

    private fun emptyExpressionContext() = ExpressionContext(
        expressionIndex = MixinExtrasExpressionIndex(),
        definitionIndex = MixinExtrasDefinitionIndex(),
    )

    private fun testSite(
        annotationRange: McTextRange,
        handlerRange: McTextRange? = null,
    ): MixinExtrasAnnotationSite = MixinExtrasAnnotationSite(
        annotation = MixinExtrasAnnotation.MODIFY_EXPRESSION_VALUE,
        methodAttribute = "draw(Ljava/lang/String;FF)V",
        atValue = "MIXINEXTRAS:EXPRESSION",
        atTarget = null,
        annotationRange = annotationRange,
        handlerMethod = handlerRange?.let {
            HandlerMethodDeclaration(
                methodName = "mcdevHandler",
                returnTypeName = "int",
                returnTypeDescriptor = "I",
                parameters = emptyList(),
                range = it,
            )
        },
    )
}
