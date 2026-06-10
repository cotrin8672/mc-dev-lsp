package io.github.mcdev.core.mixin.e2e

import io.github.mcdev.core.codeaction.WorkspaceEditFix
import io.github.mcdev.core.mixin.InjectMethodDescriptorMode
import io.github.mcdev.core.mixin.MixinCompletionOptions
import io.github.mcdev.core.mixin.MixinServiceFacade
import io.github.mcdev.core.mixinextras.MixinExtrasDiagnosticCodes
import io.github.mcdev.core.mixinextras.MixinExtrasTestFixtures
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

class MixinExtrasE2ETest {
    private val facade = mixinExtrasFacade()

    @Test
    fun completesModifyExpressionValueMethodThroughFacade() {
        val source = wrapMixin("""
            @ModifyExpressionValue(method = "dra", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
            private float mcdevModifyX(float original) { return original; }
        """)
        val items = facade.complete(MixinE2ETestSupport.requestAt(source, "dra"))
        assertTrue(items.any { it.insertText.startsWith("draw") })
        assertEquals("mixinextras.injectMethod", items.first().metadata.source)
    }

    @Test
    fun completesModifyReturnValueMethodThroughFacade() {
        val source = wrapMixin("""
            @ModifyReturnValue(method = "draw", at = @At("RETURN"))
            private void mcdevModifyReturn() {}
        """)
        val items = facade.complete(MixinE2ETestSupport.requestAt(source, "draw"))
        assertTrue(items.any { it.insertText.startsWith("draw") })
    }

    @Test
    fun completesWrapOperationMethodThroughFacade() {
        val source = wrapMixin("""
            @WrapOperation(method = "dr", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrap(String instance, Operation<Integer> original) { return original.call(instance); }
        """)
        val items = facade.complete(MixinE2ETestSupport.requestAt(source, "dr"))
        assertTrue(items.any { it.insertText.startsWith("draw") })
    }

    @Test
    fun completesWrapWithConditionMethodThroughFacade() {
        val source = wrapMixin("""
            @WrapWithCondition(method = "draw", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private boolean mcdevWrap(String instance, Operation<Boolean> original) { return original.call(instance); }
        """)
        val items = facade.complete(MixinE2ETestSupport.requestAt(source, "draw"))
        assertTrue(items.isNotEmpty())
    }

    @Test
    fun completesWrapMethodTargetMethodThroughFacade() {
        val source = wrapMixin("""
            @WrapMethod(method = "dra")
            private void mcdevWrap(SimpleTarget instance, String arg0, float arg1, float arg2, Operation<Void> original) {
                original.call(instance, arg0, arg1, arg2);
            }
        """)
        val items = facade.complete(MixinE2ETestSupport.requestAt(source, "dra"))
        assertTrue(items.any { it.insertText.startsWith("draw") })
    }

    @Test
    fun completesModifyReceiverMethodThroughFacadeFallback() {
        val source = wrapMixin("""
            @ModifyReceiver(method = "dra", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private String mcdevModifyReceiver(String receiver) { return receiver; }
        """)
        val items = facade.complete(MixinE2ETestSupport.requestAt(source, "dra"))
        assertTrue(items.any { it.insertText.startsWith("draw") })
    }

    @Test
    fun completesWrapOperationAtInvokeTargetThroughFacade() {
        val source = wrapMixin("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = ""))
            private int mcdevWrap(String instance, Operation<Integer> original) { return original.call(instance); }
        """)
        val quote = source.indexOf("target = \"") + "target = \"".length
        val items = facade.complete(MixinE2ETestSupport.requestAtOffset(source, quote))
        assertEquals("Ljava/lang/String;length()I", items.first().insertText)
    }

    @Test
    fun completesWrapOperationAtTargetWithPartialFilter() {
        val source = wrapMixin("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "length"))
            private int mcdevWrap(String instance, Operation<Integer> original) { return original.call(instance); }
        """)
        val items = facade.complete(MixinE2ETestSupport.requestAt(source, "length"))
        assertTrue(items.isNotEmpty())
        assertTrue(items.any { it.insertText.contains("length") })
    }

    @Test
    fun completesWrapWithConditionAtInvokeTargetThroughFacade() {
        val source = wrapMixin("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = ""))
            private boolean mcdevWrap(String instance, Operation<Boolean> original) { return original.call(instance); }
        """)
        val quote = source.indexOf("target = \"") + "target = \"".length
        val items = facade.complete(MixinE2ETestSupport.requestAtOffset(source, quote))
        assertTrue(items.isNotEmpty())
    }

    @Test
    fun completesMixinExtrasExpressionAtValueThroughFacade() {
        val source = wrapMixin("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "MIXINEXTRAS:EXPR"))
            private float mcdevModifyX(float original) { return original; }
        """)
        val items = facade.complete(MixinE2ETestSupport.requestAt(source, "MIXINEXTRAS:EXPR"))
        assertTrue(items.any { it.insertText == "MIXINEXTRAS:EXPRESSION" })
        assertEquals("mixinextras.expressionAtValue", items.first { it.insertText == "MIXINEXTRAS:EXPRESSION" }.metadata.source)
    }

    @Test
    fun injectMethodDescriptorModeAppliesToMixinExtrasCompletion() {
        val source = wrapMixin("""
            @WrapOperation(method = "dra", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrap(String instance, Operation<Integer> original) { return original.call(instance); }
        """)
        val items = facade.complete(
            MixinE2ETestSupport.requestAt(source, "dra"),
            MixinCompletionOptions(injectMethodDescriptorMode = InjectMethodDescriptorMode.ALWAYS),
        )
        assertTrue(items.any { it.insertText.contains("(Ljava/lang/String;FF)V") })
    }

    @Test
    fun returnsEmptyForMixinExtrasOutsideAnnotationContext() {
        val source = "class Plain { int value; }"
        val items = facade.complete(MixinE2ETestSupport.requestAt(source, "value"))
        assertTrue(items.isEmpty())
    }

    @Test
    fun diagnoseValidWrapOperationProducesNoSignatureErrors() {
        val source = wrapMixin(MixinExtrasTestFixtures.WRAP_OPERATION_SOURCE.trimIndent())
        val diagnostics = facade.diagnose(MixinE2ETestSupport.requestAt(source, "mcdevWrapLength"))
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE })
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.MISSING_OPERATION_PARAMETER })
    }

    @Test
    fun diagnoseWrapOperationBadReturnThroughFacade() {
        val source = wrapMixin(MixinExtrasTestFixtures.WRAP_OPERATION_BAD_RETURN.trimIndent())
        val diagnostics = facade.diagnose(MixinE2ETestSupport.requestAt(source, "mcdevWrapLength"))
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE })
    }

    @Test
    fun diagnoseWrapOperationMissingOperationThroughFacade() {
        val source = wrapMixin(MixinExtrasTestFixtures.WRAP_OPERATION_MISSING_OP.trimIndent())
        val diagnostics = facade.diagnose(MixinE2ETestSupport.requestAt(source, "mcdevWrapLength"))
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.MISSING_OPERATION_PARAMETER })
    }

    @Test
    fun diagnoseWrapOperationOperationNotLastThroughFacade() {
        val source = wrapMixin(MixinExtrasTestFixtures.WRAP_OPERATION_OP_NOT_LAST.trimIndent())
        val diagnostics = facade.diagnose(MixinE2ETestSupport.requestAt(source, "mcdevWrapLength"))
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.MISSING_OPERATION_PARAMETER })
    }

    @Test
    fun diagnoseValidModifyExpressionValueThroughFacade() {
        val source = wrapMixin(MixinExtrasTestFixtures.MODIFY_EXPRESSION_SOURCE.trimIndent())
        val diagnostics = facade.diagnose(MixinE2ETestSupport.requestAt(source, "mcdevModifyX"))
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRONG_ORIGINAL_VALUE_TYPE })
    }

    @Test
    fun diagnoseValidModifyReturnValueThroughFacade() {
        val source = wrapMixin(MixinExtrasTestFixtures.MODIFY_RETURN_SOURCE.trimIndent())
        val diagnostics = facade.diagnose(MixinE2ETestSupport.requestAt(source, "mcdevModifyReturn"))
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE })
    }

    @Test
    fun diagnoseValidWrapWithConditionThroughFacade() {
        val source = wrapMixin(MixinExtrasTestFixtures.WRAP_WITH_CONDITION_SOURCE.trimIndent())
        val diagnostics = facade.diagnose(MixinE2ETestSupport.requestAt(source, "mcdevWrapCondition"))
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.MISSING_OPERATION_PARAMETER })
    }

    @Test
    fun diagnoseValidWrapMethodThroughFacade() {
        val source = wrapMixin(MixinExtrasTestFixtures.WRAP_METHOD_SOURCE.trimIndent())
        val diagnostics = facade.diagnose(MixinE2ETestSupport.requestAt(source, "mcdevWrapDraw"))
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun diagnoseExpressionContextWarningThroughFacade() {
        val source = wrapMixin("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
            private float mcdevModifyX(float original) { return original; }
        """)
        val diagnostics = facade.diagnose(MixinE2ETestSupport.requestAt(source, "mcdevModifyX"))
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.UNSUPPORTED_EXPRESSION_CONTEXT })
    }

    @Test
    fun codeActionIncludesMixinExtrasFixWhenFilteredByWrongReturnType() {
        val source = wrapMixin(MixinExtrasTestFixtures.WRAP_OPERATION_BAD_RETURN.trimIndent())
        val fixes = facade.codeActions(
            MixinE2ETestSupport.requestAt(source, "mcdevWrapLength"),
            MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE,
        )
        assertTrue(fixes.any { it.title == "Fix WrapOperation handler signature" })
    }

    @Test
    fun codeActionFixesWrapOperationBadReturnThroughFacade() {
        val source = wrapMixin(MixinExtrasTestFixtures.WRAP_OPERATION_BAD_RETURN.trimIndent())
        val fixes = facade.codeActions(
            MixinE2ETestSupport.requestAt(source, "mcdevWrapLength"),
            MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE,
        )
        val fix = fixes.filterIsInstance<WorkspaceEditFix>().firstOrNull { it.title == "Fix WrapOperation handler signature" }
        assertNotNull(fix)
        assertTrue(fix.edits.first().newText.contains("return original.call(instance);"))
    }

    @Test
    fun codeActionFixesMissingOperationThroughFacade() {
        val source = wrapMixin(MixinExtrasTestFixtures.WRAP_OPERATION_MISSING_OP.trimIndent())
        val fixes = facade.codeActions(
            MixinE2ETestSupport.requestAt(source, "mcdevWrapLength"),
            MixinExtrasDiagnosticCodes.MISSING_OPERATION_PARAMETER,
        )
        assertTrue(fixes.any { it.title == "Fix WrapOperation handler signature" })
    }

    @Test
    fun codeActionsWithoutFilterIncludesMixinExtrasFixesForBadReturn() {
        val source = wrapMixin(MixinExtrasTestFixtures.WRAP_OPERATION_BAD_RETURN.trimIndent())
        val fixes = facade.codeActions(MixinE2ETestSupport.requestAt(source, "mcdevWrapLength"))
        assertTrue(fixes.any { it.kind == "quickfix.mixinextras.fixHandlerSignature" })
    }

    @Test
    fun wrapOperationAtTargetUsesBytecodeIndexNotFakeDefaults() {
        val source = wrapMixin("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = ""))
            private int mcdevWrap(String instance, Operation<Integer> original) { return original.call(instance); }
        """)
        val quote = source.indexOf("target = \"") + "target = \"".length
        val items = facade.complete(MixinE2ETestSupport.requestAtOffset(source, quote))
        assertEquals(1, items.size)
        assertTrue(items.first().label.contains("length"))
    }

    private fun wrapMixin(body: String): String = """
        @Mixin(com.example.target.SimpleTarget.class)
        abstract class ExampleMixin {
            $body
        }
    """.trimIndent()

    private fun mixinExtrasFacade(): MixinServiceFacade =
        MixinServiceFacade(
            classIndex = MixinExtrasTestFixtures.classIndex,
            bytecodeIndex = MixinExtrasTestFixtures.bytecodeIndex,
        )
}
