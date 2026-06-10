package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.codeaction.WorkspaceEditFix
import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.core.diagnostics.McSeverity
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

class MixinExtrasCodeActionServiceTest {
    private val signatureService = HandlerSignatureService(MixinExtrasTestFixtures.classIndex)
    private val service = MixinExtrasCodeActionService(MixinExtrasTestFixtures.classIndex, signatureService)
    private val documentUri = "file:///ExampleMixin.java"

    @Test
    fun generateWrapOperationHandlerCodeAction() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                ${MixinExtrasTestFixtures.WRAP_OPERATION_NO_HANDLER.trimIndent()}
            }
        """.trimIndent()
        val site = HandlerSignatureService.findAnnotationSites(source).first()
        val fix = service.generateHandlerFixes(documentUri, source, site, listOf("com/example/target/SimpleTarget")).first()
        assertEquals("Generate WrapOperation handler", fix.title)
        assertTrue(fix is WorkspaceEditFix)
        assertTrue((fix as WorkspaceEditFix).edits.first().newText.contains("Operation<int>"))
    }

    @Test
    fun generateModifyExpressionValueHandlerCodeAction() {
        val source = """
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
        """.trimIndent()
        val site = HandlerSignatureService.findAnnotationSites(source).first()
        val fix = service.generateHandlerFixes(documentUri, source, site, listOf("com/example/target/SimpleTarget")).first()
        assertEquals("Generate ModifyExpressionValue handler", fix.title)
    }

    @Test
    fun generateModifyReturnValueHandlerCodeAction() {
        val source = """
            @ModifyReturnValue(method = "draw(Ljava/lang/String;FF)V", at = @At("RETURN"))
        """.trimIndent()
        val site = HandlerSignatureService.findAnnotationSites(source).first()
        val fix = service.generateHandlerFixes(documentUri, source, site, listOf("com/example/target/SimpleTarget")).first()
        assertEquals("Generate ModifyReturnValue handler", fix.title)
    }

    @Test
    fun fixWrapOperationHandlerSignatureCodeAction() {
        val source = MixinExtrasTestFixtures.WRAP_OPERATION_BAD_RETURN.trimIndent()
        val diagnostic = McDiagnostic(
            code = MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE,
            severity = McSeverity.ERROR,
            message = "wrong return",
            range = McTextRange(McTextPosition(0, 0), McTextPosition(0, 0)),
            metadata = mapOf("annotation" to "WrapOperation"),
        )
        val fixes = service.fixesForDiagnostics(listOf(diagnostic), documentUri, source)
        assertTrue(fixes.any { it.title == "Fix WrapOperation handler signature" })
    }

    @Test
    fun fixModifyExpressionValueHandlerSignatureCodeAction() {
        val source = """
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
                private float mcdevHandler(int original) { return original; }
        """.trimIndent()
        val diagnostic = McDiagnostic(
            code = MixinExtrasDiagnosticCodes.WRONG_ORIGINAL_VALUE_TYPE,
            severity = McSeverity.ERROR,
            message = "wrong original",
            range = HandlerSignatureService.findAnnotationSites(source).first().handlerMethod!!.range,
            metadata = mapOf("annotation" to "ModifyExpressionValue"),
        )
        val fixes = service.fixesForDiagnostics(listOf(diagnostic), documentUri, source)
        assertTrue(fixes.any { it.title == "Fix ModifyExpressionValue handler signature" })
    }

    @Test
    fun fixHandlerSignatureReplacesMethodBody() {
        val source = MixinExtrasTestFixtures.WRAP_OPERATION_BAD_RETURN.trimIndent()
        val diagnostic = McDiagnostic(
            code = MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE,
            severity = McSeverity.ERROR,
            message = "wrong return",
            range = McTextRange(McTextPosition(0, 0), McTextPosition(0, 0)),
            metadata = mapOf("annotation" to "WrapOperation"),
        )
        val fix = service.fixesForDiagnostics(listOf(diagnostic), documentUri, source)
            .filterIsInstance<WorkspaceEditFix>()
            .first { it.title == "Fix WrapOperation handler signature" }
        val applied = service.applyHandlerFix(fix, source)
        assertTrue(applied.edits.first().newText.contains("return original.call(instance);"))
    }

    @Test
    fun generateHandlerFixUsesQuickfixKind() {
        val source = MixinExtrasTestFixtures.WRAP_OPERATION_NO_HANDLER.trimIndent()
        val site = HandlerSignatureService.findAnnotationSites(source).first()
        val fix = service.generateHandlerFixes(documentUri, source, site, listOf("com/example/target/SimpleTarget")).first()
        assertEquals("quickfix.mixinextras.generateHandler", fix.kind)
    }

    @Test
    fun fixesForMissingOperationIncludeGenerateAndFixTitles() {
        val source = MixinExtrasTestFixtures.WRAP_OPERATION_MISSING_OP.trimIndent()
        val diagnostic = McDiagnostic(
            code = MixinExtrasDiagnosticCodes.MISSING_OPERATION_PARAMETER,
            severity = McSeverity.ERROR,
            message = "missing op",
            range = HandlerSignatureService.findAnnotationSites(source).first().handlerMethod!!.range,
            metadata = mapOf("annotation" to "WrapOperation"),
        )
        val fixes = service.fixesForDiagnostics(listOf(diagnostic), documentUri, source)
        assertNotNull(fixes.firstOrNull { it.title == "Fix WrapOperation handler signature" })
    }
}
