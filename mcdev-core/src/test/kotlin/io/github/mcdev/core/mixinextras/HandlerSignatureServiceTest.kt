package io.github.mcdev.core.mixinextras

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

class HandlerSignatureServiceTest {
    private val classIndex = MixinExtrasTestFixtures.classIndex
    private val service = HandlerSignatureService(classIndex)

    @Test
    fun modifyExpressionValueExpectsOriginalFloatParameter() {
        val site = sites(MixinExtrasTestFixtures.MODIFY_EXPRESSION_SOURCE).first()
        val spec = service.expectedSignature(site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals("F", spec.returnTypeDescriptor)
        assertEquals(1, spec.parameters.size)
        assertEquals("F", spec.parameters.first().typeDescriptor)
        assertEquals("original", spec.parameters.first().name)
    }

    @Test
    fun modifyReturnValueVoidHasNoOriginalParameter() {
        val site = sites(MixinExtrasTestFixtures.MODIFY_RETURN_SOURCE).first()
        val spec = service.expectedSignature(site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals("V", spec.returnTypeDescriptor)
        assertTrue(spec.parameters.isEmpty())
    }

    @Test
    fun wrapOperationExpectsReceiverOperationLast() {
        val site = sites(MixinExtrasTestFixtures.WRAP_OPERATION_SOURCE).first()
        val spec = service.expectedSignature(site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals(2, spec.parameters.size)
        assertEquals("String", spec.parameters.first().readableType)
        assertTrue(spec.parameters.last().isOperation)
        assertEquals("I", spec.parameters.last().operationGenericDescriptor)
        assertEquals("original", spec.parameters.last().name)
    }

    @Test
    fun wrapOperationExpectsIntReturnType() {
        val site = sites(MixinExtrasTestFixtures.WRAP_OPERATION_SOURCE).first()
        val spec = service.expectedSignature(site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals("I", spec.returnTypeDescriptor)
    }

    @Test
    fun wrapWithConditionExpectsBooleanReturnAndOperation() {
        val site = sites(MixinExtrasTestFixtures.WRAP_WITH_CONDITION_SOURCE).first()
        val spec = service.expectedSignature(site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals("Z", spec.returnTypeDescriptor)
        assertTrue(spec.parameters.last().isOperation)
        assertEquals("Z", spec.parameters.last().operationGenericDescriptor)
    }

    @Test
    fun wrapMethodIncludesTargetParametersAndOperation() {
        val site = sites(MixinExtrasTestFixtures.WRAP_METHOD_SOURCE).first()
        val spec = service.expectedSignature(site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals(5, spec.parameters.size)
        assertTrue(spec.parameters.last().isOperation)
        assertEquals("V", spec.returnTypeDescriptor)
    }

    @Test
    fun wrapMethodInstanceReceiverUsesMixinTarget() {
        val site = sites(MixinExtrasTestFixtures.WRAP_METHOD_SOURCE).first()
        val spec = service.expectedSignature(site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals("Lcom/example/target/SimpleTarget;", spec.parameters.first().typeDescriptor)
    }

    @Test
    fun textRendererWrapOperationIncludesAllInvokeArgs() {
        val source = """
            @WrapOperation(method = "tick()V", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/font/TextRenderer;draw(Ljava/lang/String;FFI)I"))
            private int handler(TextRenderer instance, String arg0, float arg1, float arg2, int arg3, Operation<Integer> original) {
                return original.call(instance, arg0, arg1, arg2, arg3);
            }
        """
        val site = sites(source).first()
        val spec = service.expectedSignature(site, listOf("net/minecraft/client/MinecraftClient"))
        assertNotNull(spec)
        assertEquals(6, spec.parameters.size)
        assertEquals(listOf("textRenderer", "arg0", "arg1", "arg2", "arg3"), spec.operationCallArgs)
    }

    @Test
    fun validatesCorrectWrapOperationHandler() {
        val site = sites(MixinExtrasTestFixtures.WRAP_OPERATION_SOURCE).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun detectsWrongReturnType() {
        val site = sites(MixinExtrasTestFixtures.WRAP_OPERATION_BAD_RETURN).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE })
    }

    @Test
    fun detectsMissingOperationParameter() {
        val site = sites(MixinExtrasTestFixtures.WRAP_OPERATION_MISSING_OP).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.MISSING_OPERATION_PARAMETER })
    }

    @Test
    fun detectsOperationNotLastParameter() {
        val site = sites(MixinExtrasTestFixtures.WRAP_OPERATION_OP_NOT_LAST).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.MISSING_OPERATION_PARAMETER })
    }

    @Test
    fun detectsWrongOriginalValueTypeForModifyExpressionValue() {
        val source = """
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
            private float mcdevHandler(int original) { return original; }
        """
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.WRONG_ORIGINAL_VALUE_TYPE })
    }

    @Test
    fun generatesWrapOperationHandlerStub() {
        val site = sites(MixinExtrasTestFixtures.WRAP_OPERATION_NO_HANDLER.trimIndent()).first()
        val stub = service.generateHandlerStub(site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(stub)
        assertTrue(stub.contains("Operation<int>"))
        assertTrue(stub.contains("original.call(instance)"))
    }

    @Test
    fun generatesModifyExpressionValueHandlerStub() {
        val site = sites(MixinExtrasTestFixtures.WRAP_OPERATION_NO_HANDLER.replace("WrapOperation", "ModifyExpressionValue")
            .replace("""at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I")""",
                """at = @At(value = "CONSTANT", args = "floatValue=0.0")""")).first()
        val stub = service.generateHandlerStub(site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(stub)
        assertTrue(stub.contains("float original"))
        assertTrue(stub.contains("return original"))
    }

    @Test
    fun generatesModifyReturnValueHandlerStub() {
        val site = sites("""
            @ModifyReturnValue(method = "draw(Ljava/lang/String;FF)V", at = @At("RETURN"))
        """).first()
        val stub = service.generateHandlerStub(site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(stub)
        assertTrue(stub.contains("void mcdevHandler()"))
    }

    @Test
    fun generatesWrapWithConditionHandlerStub() {
        val site = sites("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
        """).first()
        val stub = service.generateHandlerStub(site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(stub)
        assertTrue(stub.contains("Operation<boolean>"))
    }

    @Test
    fun generatesWrapMethodHandlerStub() {
        val site = sites("""
            @WrapMethod(method = "draw(Ljava/lang/String;FF)V")
        """).first()
        val stub = service.generateHandlerStub(site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(stub)
        assertTrue(stub.contains("Operation<void>"))
        assertTrue(stub.contains("simpleTarget"))
    }

    @Test
    fun resolveTargetMethodUsesDescriptorSuffix() {
        val method = service.resolveTargetMethod(
            listOf("com/example/target/SimpleTarget"),
            "draw(Ljava/lang/String;FF)V",
        )
        assertNotNull(method)
        assertEquals("draw", method.name)
    }

    @Test
    fun resolveTargetMethodReturnsNullForMissingMethod() {
        val method = service.resolveTargetMethod(listOf("com/example/target/SimpleTarget"), "missing")
        assertEquals(null, method)
    }

    @Test
    fun findAnnotationSitesParsesMultipleAnnotations() {
        val source = MixinExtrasTestFixtures.MODIFY_EXPRESSION_SOURCE + MixinExtrasTestFixtures.MODIFY_RETURN_SOURCE
        val sites = HandlerSignatureService.findAnnotationSites(source)
        assertEquals(2, sites.size)
    }

    @Test
    fun operationSignatureRendererFormatsGeneric() {
        assertEquals("Operation<int>", OperationSignatureRenderer.renderOperationType("I"))
        assertEquals("float", OperationSignatureRenderer.readableType("F"))
    }

    @Test
    fun mixinExtrasAnnotationMapsFromSimpleName() {
        assertEquals(MixinExtrasAnnotation.WRAP_OPERATION, MixinExtrasAnnotation.fromSimpleName("WrapOperation"))
        assertEquals(MixinExtrasAnnotation.EXPRESSION, MixinExtrasAnnotation.fromSimpleName("Expression"))
    }

    private fun sites(source: String) = HandlerSignatureService.findAnnotationSites(source.trimIndent())

    private fun enrich(handler: HandlerMethodDeclaration) =
        HandlerSignatureService.enrichHandlerTypes(handler, classIndex)
}
