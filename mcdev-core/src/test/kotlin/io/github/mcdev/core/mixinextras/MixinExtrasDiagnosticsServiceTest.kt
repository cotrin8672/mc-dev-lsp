package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.diagnostics.McSeverity
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class MixinExtrasDiagnosticsServiceTest {
    private val service = MixinExtrasDiagnosticsService(MixinExtrasTestFixtures.classIndex)

    @Test
    fun noDiagnosticsForValidWrapOperationHandler() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            ${MixinExtrasTestFixtures.WRAP_OPERATION_SOURCE.trimIndent()}
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.none { it.severity == McSeverity.ERROR })
    }

    @Test
    fun reportsWrongReturnTypeDiagnostic() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            ${MixinExtrasTestFixtures.WRAP_OPERATION_BAD_RETURN.trimIndent()}
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE })
    }

    @Test
    fun reportsMissingOperationParameterDiagnostic() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            ${MixinExtrasTestFixtures.WRAP_OPERATION_MISSING_OP.trimIndent()}
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.MISSING_OPERATION_PARAMETER })
    }

    @Test
    fun reportsOperationNotLastDiagnostic() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            ${MixinExtrasTestFixtures.WRAP_OPERATION_OP_NOT_LAST.trimIndent()}
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.MISSING_OPERATION_PARAMETER })
    }

    @Test
    fun reportsWrongOriginalValueTypeForModifyExpressionValue() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
                private float mcdevHandler(int original) { return original; }
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.WRONG_ORIGINAL_VALUE_TYPE })
    }

    @Test
    fun noDiagnosticsForValidModifyExpressionValue() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            ${MixinExtrasTestFixtures.MODIFY_EXPRESSION_SOURCE.trimIndent()}
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRONG_ORIGINAL_VALUE_TYPE })
    }

    @Test
    fun noDiagnosticsForValidModifyReturnValueVoid() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            ${MixinExtrasTestFixtures.MODIFY_RETURN_SOURCE.trimIndent()}
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.none { it.severity == McSeverity.ERROR })
    }

    @Test
    fun validatesWrapWithConditionHandler() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            ${MixinExtrasTestFixtures.WRAP_WITH_CONDITION_SOURCE.trimIndent()}
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.none { it.severity == McSeverity.ERROR })
    }

    @Test
    fun validatesWrapMethodHandler() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            ${MixinExtrasTestFixtures.WRAP_METHOD_SOURCE.trimIndent()}
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.none { it.severity == McSeverity.ERROR })
    }

    @Test
    fun expressionAtValueProducesLimitedValidationWarning() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
                private float mcdev${'$'}handler(float original) { return original; }
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.UNSUPPORTED_EXPRESSION_CONTEXT })
    }

    @Test
    fun diagnosticMetadataIncludesAnnotationName() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            ${MixinExtrasTestFixtures.WRAP_OPERATION_BAD_RETURN.trimIndent()}
        """.trimIndent()
        val diagnostic = analyze(source).first { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE }
        assertEquals("WrapOperation", diagnostic.metadata["annotation"])
    }

    private fun analyze(source: String) = service.analyze(
        MixinExtrasDiagnosticRequest(source = source, documentUri = "file:///Example.java"),
    )
}
