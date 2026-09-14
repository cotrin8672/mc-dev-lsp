package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.bytecode.BytecodeFixtureCompiler
import io.github.mcdev.core.codeaction.WorkspaceEditFix
import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.core.diagnostics.McSeverity
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mixin.AnnotationContextExtractor
import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FakeBytecodeIndex
import io.github.mcdev.core.mixin.FakeClassIndex
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.mixin.MixinClassModel
import io.github.mcdev.core.mixin.MixinFacadeRequest
import io.github.mcdev.core.mixin.MixinServiceFacade
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.objectweb.asm.Type

class MixinExtrasCodeActionServiceTest {
    private val signatureService = HandlerSignatureService(MixinExtrasTestFixtures.classIndex)
    private val service = MixinExtrasCodeActionService(MixinExtrasTestFixtures.classIndex, signatureService)
    private val expressionSignatureService = HandlerSignatureService(
        MixinExtrasTestFixtures.classIndex,
        MixinExtrasTestFixtures.bytecodeIndex,
    )
    private val expressionService = MixinExtrasCodeActionService(
        MixinExtrasTestFixtures.classIndex,
        expressionSignatureService,
    )
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
        assertTrue((fix as WorkspaceEditFix).edits.first().newText.contains("Operation<Integer>"))
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
            @ModifyReturnValue(method = "compute()I", at = @At("RETURN"))
        """.trimIndent()
        val site = HandlerSignatureService.findAnnotationSites(source).first()
        val fix = service.generateHandlerFixes(documentUri, source, site, listOf("com/example/target/SimpleTarget")).first()
        assertEquals("Generate ModifyReturnValue handler", fix.title)
    }

    @Test
    fun generateModifyReceiverHandlerCodeAction() {
        val source = """
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
        """.trimIndent()
        val site = HandlerSignatureService.findAnnotationSites(source).first()
        val fix = expressionService.generateHandlerFixes(documentUri, source, site, listOf("com/example/target/SimpleTarget"))
            .filterIsInstance<WorkspaceEditFix>()
            .single()
        assertEquals("Generate ModifyReceiver handler", fix.title)
        assertTrue(fix.edits.first().newText.contains("String mcdevHandler(String instance)"))
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
    fun wrapMethodStaticMismatchDoesNotOfferSignatureFix() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @WrapMethod(method = "noop()V")
                private void mcdevWrapNoop(Operation<Void> original) {
                    original.call();
                }
            }
        """.trimIndent()
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val handler = site.handlerMethod
        assertNotNull(handler)
        assertFalse(handler.isStatic)
        val diagnostic = McDiagnostic(
            code = MixinExtrasDiagnosticCodes.WRAP_METHOD_STATIC_MISMATCH,
            severity = McSeverity.ERROR,
            message = "WrapMethod handler must be static to match the target method",
            range = handler.range,
            metadata = mapOf("annotation" to "WrapMethod"),
        )

        assertTrue(service.fixesForDiagnostics(listOf(diagnostic), documentUri, source).isEmpty())
    }

    @Test
    fun fixCancellableParameterTypeSupportsBothReturnKindsAndInject() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @ModifyReturnValue(method = "compute()I", at = @At("RETURN"))
                private int nonVoid(int original, @Cancellable /* CallbackInfo */ final CallbackInfo /* CallbackInfo */ ci /* trailing */) {
                    return original;
                }

                @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
                private float voidTarget(float original, @Cancellable CallbackInfoReturnable ci) {
                    return original;
                }

                @Inject(method = "compute()I", at = @At("RETURN"))
                private void inject(@Cancellable CallbackInfo ci) {
                }
            }
        """.trimIndent()
        val facade = MixinServiceFacade(MixinExtrasTestFixtures.classIndex, MixinExtrasTestFixtures.bytecodeIndex)
        val request = MixinFacadeRequest(source, line = 0, character = 0, documentUri = documentUri)
        val diagnostics = facade.diagnose(request)
        val cancellableDiagnostics = diagnostics.filter {
            it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH &&
                it.message.startsWith("Cancellable parameter type should be ")
        }
        assertEquals(3, cancellableDiagnostics.size)

        val fixes = facade.codeActions(request, MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH)
            .filterIsInstance<WorkspaceEditFix>()
            .sortedBy { it.edits.single().startOffset }
        assertEquals(3, fixes.size)
        assertEquals(
            listOf(
                "org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable",
                "org.spongepowered.asm.mixin.injection.callback.CallbackInfo",
                "org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable",
            ),
            fixes.map { it.edits.single().newText },
        )
        assertEquals(
            listOf("CallbackInfo", "CallbackInfoReturnable", "CallbackInfo"),
            fixes.map { fix ->
                val edit = fix.edits.single()
                source.substring(edit.startOffset, edit.endOffset)
            },
        )
        assertTrue(fixes.all { it.kind == "quickfix.mixinextras.fixCancellableParameterType" })
        val applied = applyFix(source, fixes.first())
        assertTrue(applied.contains("@Cancellable /* CallbackInfo */ final org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable /* CallbackInfo */ ci /* trailing */"))
        assertTrue(applied.contains("return original;"))
    }

    @Test
    fun fixCancellableParameterTypeUsesEachMixinScope() {
        val classIndex = cancellableTargetClassIndex()
        val source = """
            @Mixin(com.example.target.VoidTarget.class)
            abstract class VoidMixin {
                @Inject(method = "run()V", at = @At("HEAD"))
                private void voidHandler(@Cancellable CallbackInfoReturnable ci) {
                }
            }

            @Mixin(com.example.target.IntTarget.class)
            abstract class IntMixin {
                @Inject(method = "run()I", at = @At("HEAD"))
                private void intHandler(@Cancellable CallbackInfo ci) {
                }
            }
        """.trimIndent()
        val facade = MixinServiceFacade(classIndex, FakeBytecodeIndex())
        val request = MixinFacadeRequest(source, line = 0, character = 0, documentUri = documentUri)
        val diagnostics = facade.diagnose(request)
        val cancellableDiagnostics = diagnostics.filter {
            it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH &&
                it.message.startsWith("Cancellable parameter type should be ")
        }
        assertEquals(2, cancellableDiagnostics.size)
        val fixes = facade.codeActions(request, MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH)
            .filterIsInstance<WorkspaceEditFix>()
            .sortedBy { it.edits.single().startOffset }
        assertEquals(2, fixes.size)
        assertEquals(
            listOf("CallbackInfo", "CallbackInfoReturnable"),
            fixes.map { it.edits.single().newText.substringAfterLast('.') },
        )
        assertEquals(
            listOf("CallbackInfoReturnable", "CallbackInfo"),
            fixes.map { fix ->
                val edit = fix.edits.single()
                source.substring(edit.startOffset, edit.endOffset)
            },
        )
    }

    @Test
    fun noCancellableFixWhenMethodSelectorsRequireDifferentTypes() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ArrayMixin {
                @Inject(method = { "compute()I", "draw(Ljava/lang/String;FF)V" }, at = @At("RETURN"))
                private void handler(@Cancellable CallbackInfo ci) {
                }
            }
        """.trimIndent()
        val facade = MixinServiceFacade(MixinExtrasTestFixtures.classIndex, MixinExtrasTestFixtures.bytecodeIndex)
        val request = MixinFacadeRequest(source, line = 0, character = 0, documentUri = documentUri)
        val diagnostics = facade.diagnose(request).filter {
            it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH &&
                it.message.startsWith("Cancellable parameter type should be ")
        }
        assertEquals(1, diagnostics.size)
        assertTrue(
            facade.codeActions(request, MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH)
                .none { it.kind == "quickfix.mixinextras.fixCancellableParameterType" },
        )
    }

    @Test
    fun coerceUsesHandlerToContractDirectionAndPreservesValidParameterInFix() {
        val classIndex = stringApplyClassIndex()
        val signatureService = HandlerSignatureService(classIndex)
        val service = MixinExtrasCodeActionService(classIndex, signatureService)
        val source = """
            import org.spongepowered.asm.mixin.injection.Coerce;

            @Mixin(com.example.target.StringTarget.class)
            abstract class ExampleMixin {
                @WrapMethod(method = "apply(Ljava/lang/String;)Ljava/lang/String;")
                @Coerce
                private Object mcdevHandler(@Coerce Object value, Operation<Object> original) {
                    return original.call(value);
                }
            }
        """.trimIndent()

        val diagnostics = MixinExtrasDiagnosticsService(classIndex, FakeBytecodeIndex()).analyze(
            MixinExtrasDiagnosticRequest(source, documentUri),
        )
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE })
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRONG_ORIGINAL_VALUE_TYPE })
        assertTrue(
            diagnostics.any { it.code == MixinExtrasDiagnosticCodes.WRONG_OPERATION_GENERIC },
            diagnostics.joinToString(),
        )

        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val staleDiagnostic = McDiagnostic(
            code = MixinExtrasDiagnosticCodes.WRONG_OPERATION_GENERIC,
            severity = McSeverity.ERROR,
            message = "stale",
            range = site.handlerMethod!!.range,
            metadata = mapOf("annotation" to "WrapMethod"),
        )
        val fix = service.fixesForDiagnostics(listOf(staleDiagnostic), documentUri, source)
            .filterIsInstance<WorkspaceEditFix>()
            .single { it.title == "Fix WrapMethod handler signature" }
        assertEquals(
            "Object mcdevHandler(@Coerce Object value, Operation<String> original)",
            fix.edits.first().newText,
        )

        val orderedAnnotations = listOf(
            "@Coerce\n@WrapMethod(method = \"apply(Ljava/lang/String;)Ljava/lang/String;\")",
            "@WrapMethod(method = \"apply(Ljava/lang/String;)Ljava/lang/String;\")\n@Coerce",
            "@Coerce\n@WrapOperation(method = \"apply(Ljava/lang/String;)Ljava/lang/String;\", at = @At(value = \"INVOKE\", target = \"Lcom/example/target/StringTarget;apply(Ljava/lang/String;)Ljava/lang/String;\"))",
            "@WrapOperation(method = \"apply(Ljava/lang/String;)Ljava/lang/String;\", at = @At(value = \"INVOKE\", target = \"Lcom/example/target/StringTarget;apply(Ljava/lang/String;)Ljava/lang/String;\"))\n@Coerce",
        )
        for (annotations in orderedAnnotations) {
            val orderedSource = """
                import org.spongepowered.asm.mixin.injection.Coerce;

                @Mixin(com.example.target.StringTarget.class)
                abstract class ExampleMixin {
                    $annotations
                    private Object mcdevHandler(@Coerce Object value, Operation<Object> original) {
                        return value;
                    }
                }
            """.trimIndent()
            assertTrue(HandlerSignatureService.findAnnotationSites(orderedSource).single().handlerMethod!!.hasMethodLevelCoerce)
        }
    }

    @Test
    fun coerceRejectsUnresolvedParameterType() {
        val classIndex = stringApplyClassIndex()
        val signatureService = HandlerSignatureService(classIndex)
        val source = """
            import org.spongepowered.asm.mixin.injection.Coerce;

            @Mixin(com.example.target.StringTarget.class)
            abstract class ExampleMixin {
                @WrapMethod(method = "apply(Ljava/lang/String;)Ljava/lang/String;")
                @Coerce
                private Object mcdevHandler(@Coerce Missing value, Operation<String> original) {
                    return original.call(value);
                }
            }
        """.trimIndent()
        val diagnostics = MixinExtrasDiagnosticsService(classIndex, FakeBytecodeIndex(), signatureService).analyze(
            MixinExtrasDiagnosticRequest(source, documentUri),
        )
        assertTrue(
            diagnostics.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH },
            diagnostics.joinToString(),
        )
    }

    @Test
    fun coerceRejectsExpectedSupertypeForHandlerSubtype() {
        val classIndex = stringApplyObjectReturnClassIndex()
        val signatureService = HandlerSignatureService(classIndex)
        val source = """
            import org.spongepowered.asm.mixin.injection.Coerce;

            @Mixin(com.example.target.StringTarget.class)
            abstract class ExampleMixin {
                @WrapMethod(method = "apply(Ljava/lang/String;)Ljava/lang/Object;")
                @Coerce
                private String mcdevHandler(String value, Operation<Object> original) {
                    return value;
                }
            }
        """.trimIndent()
        val diagnostics = MixinExtrasDiagnosticsService(classIndex, FakeBytecodeIndex(), signatureService).analyze(
            MixinExtrasDiagnosticRequest(source, documentUri),
        )
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE })
    }

    @Test
    fun rawOperationGenericMustBeExplicitAndFixCanonicalizesIt() {
        val classIndex = stringApplyClassIndex()
        val signatureService = HandlerSignatureService(classIndex)
        val service = MixinExtrasCodeActionService(classIndex, signatureService)
        val source = """
            @Mixin(com.example.target.StringTarget.class)
            abstract class ExampleMixin {
                @WrapMethod(method = "apply(Ljava/lang/String;)Ljava/lang/String;")
                private String mcdevHandler(String value, Operation original) {
                    return original.call(value);
                }
            }
        """.trimIndent()

        val diagnostics = MixinExtrasDiagnosticsService(classIndex, FakeBytecodeIndex(), signatureService).analyze(
            MixinExtrasDiagnosticRequest(source, documentUri),
        )
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.WRONG_OPERATION_GENERIC })

        val fix = service.fixesForDiagnostics(diagnostics, documentUri, source)
            .filterIsInstance<WorkspaceEditFix>()
            .single { it.title == "Fix WrapMethod handler signature" }
        assertEquals(
            "String mcdevHandler(String value, Operation<String> original)",
            fix.edits.first().newText,
        )
    }

    @Test
    fun fixHandlerSignatureAtArrayCompatibleSitesProduceOneAction() {
        val source = wrapOperationAtArrayHandlerSource(
            atArray = """
                @At(value = "INVOKE", target = "Ljava/lang/String;length()I"),
                @At(value = "INVOKE", target = "Ljava/lang/String;length()I"),
            """.trimIndent(),
            handler = """
                private void mcdevWrapLength(String instance, Operation<Integer> original) {
                    original.call(instance);
                }
            """.trimIndent(),
        )
        assertEquals(2, HandlerSignatureService.findAnnotationSites(source).size)
        val diagnostic = wrapOperationSignatureDiagnostic()
        val fixes = service.fixesForDiagnostics(listOf(diagnostic), documentUri, source)
        assertEquals(1, fixes.count { it.title == "Fix WrapOperation handler signature" })
    }

    @Test
    fun fixHandlerSignatureAtArrayIncompatibleSitesProduceNoAction() {
        val source = wrapOperationAtArrayHandlerSource(
            atArray = """
                @At(value = "INVOKE", target = "Ljava/lang/String;length()I"),
                @At(value = "INVOKE", target = "Ljava/lang/String;charAt(I)C"),
            """.trimIndent(),
            handler = """
                private void mcdevWrapLength(String instance, Operation<Integer> original) {
                    original.call(instance);
                }
            """.trimIndent(),
        )
        val fixes = service.fixesForDiagnostics(listOf(wrapOperationSignatureDiagnostic()), documentUri, source)
        assertTrue(fixes.none { it.title == "Fix WrapOperation handler signature" })
    }

    @Test
    fun fixHandlerSignatureMethodArrayCapturedTailRequiresAgreementAcrossSites() {
        val classIndex = methodArrayCapturedTailClassIndex()
        val methodArrayService = MixinExtrasCodeActionService(
            classIndex,
            HandlerSignatureService(classIndex),
        )
        val diagnostic = wrapOperationSignatureDiagnostic()
        val incompatibleSource = wrapOperationMethodArraySource(
            methods = """"foo(I)I", "bar(J)I"""",
            handler = """
                private void mcdevHandler(String instance, Operation<Integer> original, int captured) {
                    original.call(instance);
                }
            """.trimIndent(),
        )
        val compatibleSource = wrapOperationMethodArraySource(
            methods = """"foo(I)I", "bar(J)I"""",
            handler = """
                private void mcdevHandler(String instance, Operation<Integer> original) {
                    original.call(instance);
                }
            """.trimIndent(),
        )
        assertEquals(2, HandlerSignatureService.findAnnotationSites(incompatibleSource).size)

        val incompatibleFixes = methodArrayService.fixesForDiagnostics(
            listOf(diagnostic),
            documentUri,
            incompatibleSource,
        )
        assertTrue(incompatibleFixes.none { it.title == "Fix WrapOperation handler signature" })

        val compatibleFixes = methodArrayService.fixesForDiagnostics(
            listOf(diagnostic),
            documentUri,
            compatibleSource,
        )
        assertEquals(1, compatibleFixes.count { it.title == "Fix WrapOperation handler signature" })
        val fix = compatibleFixes.filterIsInstance<WorkspaceEditFix>()
            .single { it.title == "Fix WrapOperation handler signature" }
        assertEquals(
            "int mcdevHandler(String instance, Operation<Integer> original)",
            fix.edits.first().newText,
        )
    }

    @Test
    fun fixHandlerSignatureAtArrayUnresolvedSiblingProducesNoAction() {
        val source = wrapOperationAtArrayHandlerSource(
            atArray = """
                @At(value = "INVOKE", target = "Ljava/lang/String;length()I"),
                @At(value = "INVOKE", target = "not-a-member-target"),
            """.trimIndent(),
            handler = """
                private void mcdevWrapLength(String instance, Operation<Integer> original) {
                    original.call(instance);
                }
            """.trimIndent(),
        )
        val fixes = service.fixesForDiagnostics(listOf(wrapOperationSignatureDiagnostic()), documentUri, source)
        assertTrue(fixes.none { it.title == "Fix WrapOperation handler signature" })
    }

    @Test
    fun fixHandlerSignatureDistinctHandlersRemainIndependent() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
                private void mcdevWrapLength(String instance, Operation<Integer> original) {
                    original.call(instance);
                }

                @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;charAt(I)C"))
                private void mcdevWrapChar(String receiver, int value, Operation<Character> original) {
                    return original.call(receiver, value);
                }
            }
        """.trimIndent()
        val sites = HandlerSignatureService.findAnnotationSites(source)
        assertEquals(2, sites.size)
        val diagnostics = sites.map { site ->
            McDiagnostic(
                code = MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE,
                severity = McSeverity.ERROR,
                message = "wrong return",
                range = site.handlerMethod!!.range,
                metadata = mapOf(
                    "annotation" to "WrapOperation",
                    "method" to site.methodAttribute,
                ),
            )
        }
        val fixes = service.fixesForDiagnostics(diagnostics, documentUri, source)
            .filterIsInstance<WorkspaceEditFix>()
            .filter { it.title == "Fix WrapOperation handler signature" }
        assertEquals(2, fixes.size)
        val fixByHandler = sites.associate { site ->
            val handler = site.handlerMethod!!
            val handlerStart = rangeStart(source, handler.range)
            val expectedStart = source.indexOf(handler.returnTypeName, handlerStart)
            val expectedEnd = rangeEnd(source, handler.range)
            handler.methodName to fixes.single { fix ->
                fix.edits.any { candidate ->
                    candidate.startOffset == expectedStart && candidate.endOffset == expectedEnd
                }
            }
        }
        assertEquals(
            "int mcdevWrapLength(String instance, Operation<Integer> original)",
            fixByHandler["mcdevWrapLength"]!!.edits.first().newText,
        )
        assertEquals(
            "char mcdevWrapChar(String receiver, int value, Operation<Character> original)",
            fixByHandler["mcdevWrapChar"]!!.edits.first().newText,
        )
    }

    @Test
    fun metadataOnlyDiagnosticAcrossDistinctAnnotationGroupsOffersNoAction() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
                @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            }
        """.trimIndent()
        val diagnostic = McDiagnostic(
            code = MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE,
            severity = McSeverity.ERROR,
            message = "wrong return",
            range = McTextRange(McTextPosition(0, 0), McTextPosition(0, 0)),
            metadata = mapOf(
                "annotation" to "WrapOperation",
                "method" to "draw(Ljava/lang/String;FF)V",
            ),
        )
        assertTrue(service.fixesForDiagnostics(listOf(diagnostic), documentUri, source).isEmpty())
    }

    @Test
    fun generateHandlerAtArrayCompatibleSitesProduceOneAction() {
        val source = wrapOperationAtArrayHandlerSource(
            atArray = """
                @At(value = "INVOKE", target = "Ljava/lang/String;length()I"),
                @At(value = "INVOKE", target = "Ljava/lang/String;length()I"),
            """.trimIndent(),
            handler = "",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).first()
        assertNull(site.handlerMethod)
        val fixes = service.generateHandlerFixes(
            documentUri,
            source,
            site,
            listOf("com/example/target/SimpleTarget"),
        )
        assertEquals(1, fixes.size)
        assertEquals("Generate WrapOperation handler", fixes.single().title)
    }

    @Test
    fun fixOperationCallArgumentsAtArrayDuplicateSitesStillOfferFix() {
        val source = wrapOperationAtArrayHandlerSource(
            atArray = """
                @At(value = "INVOKE", target = "Ljava/lang/String;length()I"),
                @At(value = "INVOKE", target = "Ljava/lang/String;length()I"),
            """.trimIndent(),
            handler = """
                private int mcdevWrapLength(String instance, Operation<Integer> original) {
                    return original.call();
                }
            """.trimIndent(),
        )
        assertEquals(2, HandlerSignatureService.findAnnotationSites(source).size)
        val diagnostic = operationCallDiagnostic(source)
        val fix = operationCallFix(source, diagnostic)
        assertEquals("instance", fix.edits.single().newText)
    }

    @Test
    fun fixOperationCallArgumentsAtArrayUnresolvedSiblingOffersNoAction() {
        val source = wrapOperationAtArrayHandlerSource(
            atArray = """
                @At(value = "INVOKE", target = "Ljava/lang/String;length()I"),
                @At(value = "INVOKE", target = "not-a-member-target"),
            """.trimIndent(),
            handler = """
                private int mcdevWrapLength(String instance, Operation<Integer> original) {
                    return original.call();
                }
            """.trimIndent(),
        )
        val diagnostic = operationCallDiagnostic(source)
        val fixes = service.fixesForDiagnostics(listOf(diagnostic), documentUri, source)
        assertTrue(fixes.none { it.title == "Fix Operation.call arguments" })
    }

    @Test
    fun fixModifyExpressionValueHandlerSignatureCodeAction() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
                private float mcdevHandler(int original) { return original; }
            }
        """.trimIndent()
        val diagnostic = McDiagnostic(
            code = MixinExtrasDiagnosticCodes.WRONG_ORIGINAL_VALUE_TYPE,
            severity = McSeverity.ERROR,
            message = "wrong original",
            range = HandlerSignatureService.findAnnotationSites(source).first().handlerMethod!!.range,
            metadata = mapOf("annotation" to "ModifyExpressionValue"),
        )
        val fix = service.fixesForDiagnostics(listOf(diagnostic), documentUri, source)
            .filterIsInstance<WorkspaceEditFix>()
            .first { it.title == "Fix ModifyExpressionValue handler signature" }
        assertTrue(fix.edits.first().newText.contains("float mcdevHandler(float original)"))
    }

    @Test
    fun fixHandlerSignaturePreservesBody() {
        val distinctiveBody = """
            // keep-this-comment
                original.call(instance);
            /* block-comment-stays */
                throw new RuntimeException("stay-put");
        """.trimIndent()
        val source = wrapOperationHandlerSource(
            handler = """
                private void mcdevWrapLength(String instance, Operation<Integer> original) {
                    $distinctiveBody
                }
            """.trimIndent(),
        )
        val handlerStart = source.indexOf("private void mcdevWrapLength")
        val bodyStart = source.indexOf('{', handlerStart) + 1
        val bodyEnd = source.lastIndexOf('}')
        val originalBody = source.substring(bodyStart, bodyEnd)
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
        val edit = fix.edits.first()
        assertEquals("int mcdevWrapLength(String instance, Operation<Integer> original)", edit.newText)
        assertFalse(edit.newText.contains("private"))
        assertFalse(edit.newText.contains("{"))
        assertFalse(edit.newText.contains("return original.call(instance);"))
        val applied = applyFix(source, fix)
        assertTrue(applied.contains("private int mcdevWrapLength("))
        val appliedBodyStart = applied.indexOf('{', applied.indexOf("mcdevWrapLength")) + 1
        val appliedBodyEnd = applied.lastIndexOf('}')
        assertEquals(originalBody, applied.substring(appliedBodyStart, appliedBodyEnd))
    }

    @Test
    fun fixHandlerSignaturePreservesStaticAndThrows() {
        val source = wrapOperationHandlerSource(
            handler = """
                private static void mcdevWrapLength(String instance, Operation<Integer> original) throws Exception {
                    return original.call(instance);
                }
            """.trimIndent(),
        )
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
        val applied = applyFix(source, fix)
        assertTrue(applied.contains("private static int mcdevWrapLength("))
        assertTrue(applied.contains(") throws Exception {"))
        assertTrue(applied.contains("return original.call(instance);"))
    }

    @Test
    fun fixHandlerSignaturePreservesParameterNamesAndTrailingSugar() {
        for (lineEnding in listOf("\n", "\r\n")) {
            val source = wrapOperationHandlerSource(
                handler = """
                    private long mcdevWrapLength(String receiver, Operation<Integer> op, @Local int counter, @Share LocalRef shared) {
                        return op.call(receiver);
                    }
                """.trimIndent(),
            ).replace("\n", lineEnding)
            val diagnostic = McDiagnostic(
                code = MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE,
                severity = McSeverity.ERROR,
                message = "wrong return",
                range = McTextRange(McTextPosition(0, 0), McTextPosition(0, 0)),
                metadata = mapOf("annotation" to "WrapOperation"),
            )
            val fix = service.fixesForDiagnostics(listOf(diagnostic), documentUri, source)
                .filterIsInstance<WorkspaceEditFix>()
                .single { it.title == "Fix WrapOperation handler signature" }
            assertEquals(
                "int mcdevWrapLength(String receiver, Operation<Integer> op, @Local int counter, @Share LocalRef shared)",
            fix.edits.first().newText,
            )
            val applied = applyFix(source, fix)
            assertTrue(applied.contains("private int mcdevWrapLength(String receiver, Operation<Integer> op, @Local int counter, @Share LocalRef shared) {"))
            assertTrue(applied.contains("return op.call(receiver);"))
        }
    }

    @Test
    fun generateHandlerFixUsesQuickfixKind() {
        val source = MixinExtrasTestFixtures.WRAP_OPERATION_NO_HANDLER.trimIndent()
        val site = HandlerSignatureService.findAnnotationSites(source).first()
        val fix = service.generateHandlerFixes(documentUri, source, site, listOf("com/example/target/SimpleTarget")).first()
        assertEquals("quickfix.mixinextras.generateHandler", fix.kind)
    }

    @Test
    fun facadeOffersGenerationForMissingHandlerAtRequestedPosition() {
        val source = MixinExtrasTestFixtures.WRAP_OPERATION_NO_HANDLER.trimIndent()
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val facade = MixinServiceFacade(
            classIndex = MixinExtrasTestFixtures.classIndex,
            bytecodeIndex = MixinExtrasTestFixtures.bytecodeIndex,
        )
        val fixes = facade.codeActions(
            MixinFacadeRequest(
                bufferText = source,
                line = site.annotationRange.start.line,
                character = site.annotationRange.start.character,
                documentUri = documentUri,
            ),
        )
        assertTrue(fixes.any { it.kind == "quickfix.mixinextras.generateHandler" })
    }

    @Test
    fun facadeOffersGenerationWithDiagnosticFilterAtRequestedPosition() {
        val source = MixinExtrasTestFixtures.WRAP_OPERATION_NO_HANDLER.trimIndent()
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val facade = MixinServiceFacade(
            classIndex = MixinExtrasTestFixtures.classIndex,
            bytecodeIndex = MixinExtrasTestFixtures.bytecodeIndex,
        )
        val fixes = facade.codeActions(
            MixinFacadeRequest(
                bufferText = source,
                line = site.annotationRange.start.line,
                character = site.annotationRange.start.character,
                documentUri = documentUri,
            ),
            diagnosticCode = MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE,
        )
        assertTrue(fixes.any { it.kind == "quickfix.mixinextras.generateHandler" })
    }

    @Test
    fun facadeOffersModifyReceiverGenerationWithDiagnosticFilterAtRequestedPosition() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            }
        """.trimIndent()
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val facade = MixinServiceFacade(
            classIndex = MixinExtrasTestFixtures.classIndex,
            bytecodeIndex = MixinExtrasTestFixtures.bytecodeIndex,
        )
        val fixes = facade.codeActions(
            MixinFacadeRequest(
                bufferText = source,
                line = site.annotationRange.start.line,
                character = site.annotationRange.start.character,
                documentUri = documentUri,
            ),
            diagnosticCode = "JAVA_UNRELATED_DIAGNOSTIC",
        )
        assertTrue(fixes.any { it.title == "Generate ModifyReceiver handler" })
    }

    @Test
    fun facadeDoesNotGenerateOverCommentSeparatedExistingHandler() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @WrapMethod(method = "draw(Ljava/lang/String;FF)V")
                // keep this comment
                private /* between modifiers */ final synchronized void mcdevWrapDraw(
                    String arg0, float arg1, float arg2, Operation<Void> original
                ) {
                    original.call(arg0, arg1, arg2);
                }
            }
        """.trimIndent()
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        assertNotNull(site.handlerMethod)
        val facade = MixinServiceFacade(
            classIndex = MixinExtrasTestFixtures.classIndex,
            bytecodeIndex = MixinExtrasTestFixtures.bytecodeIndex,
        )
        val fixes = facade.codeActions(
            MixinFacadeRequest(
                bufferText = source,
                line = site.annotationRange.start.line,
                character = site.annotationRange.start.character,
                documentUri = documentUri,
            ),
        )
        assertTrue(fixes.none { it.kind == "quickfix.mixinextras.generateHandler" })
    }

    @Test
    fun generatedAndSignatureFixesAddMissingOperationImport() {
        val noHandlerSource = MixinExtrasTestFixtures.WRAP_OPERATION_NO_HANDLER.trimIndent()
        val noHandlerSite = HandlerSignatureService.findAnnotationSites(noHandlerSource).single()
        val generated = service.generateHandlerFixes(
            documentUri = documentUri,
            source = noHandlerSource,
            site = noHandlerSite,
            mixinTargets = listOf("com/example/target/SimpleTarget"),
        ).filterIsInstance<WorkspaceEditFix>().single()
        assertTrue(generated.edits.any { it.newText.contains("import com.llamalad7.mixinextras.injector.wrapoperation.Operation;") })
        assertTrue(applyFix(noHandlerSource, generated).contains("Operation<Integer>"))

        val badReturnSource = MixinExtrasTestFixtures.WRAP_OPERATION_BAD_RETURN.trimIndent()
        val diagnostic = McDiagnostic(
            code = MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE,
            severity = McSeverity.ERROR,
            message = "wrong return",
            range = McTextRange(McTextPosition(0, 0), McTextPosition(0, 0)),
            metadata = mapOf("annotation" to "WrapOperation"),
        )
        val signature = service.fixesForDiagnostics(listOf(diagnostic), documentUri, badReturnSource)
            .filterIsInstance<WorkspaceEditFix>()
            .single { it.title == "Fix WrapOperation handler signature" }
        assertTrue(signature.edits.any { it.newText.contains("import com.llamalad7.mixinextras.injector.wrapoperation.Operation;") })
    }

    @Test
    fun generatedHandlerUsesCanonicalTypeWhenSimpleNameConflicts() {
        val targetOwner = "com/example/target/CustomTarget"
        val customType = "com/example/target/Custom"
        val classIndex = FakeClassIndex(
            classes = listOf(
                ClassIndexEntry("CustomTarget", "com.example.target", targetOwner),
                ClassIndexEntry("Custom", "com.example.target", customType),
            ),
            methods = mapOf(
                targetOwner to listOf(
                    MethodIndexEntry("compute", "()L$customType;", false, "compute(): Custom"),
                ),
            ),
        )
        val customService = MixinExtrasCodeActionService(classIndex)
        val source = """
            package com.example.mixin;
            import com.example.other.Custom;
            import com.example.target.CustomTarget;
            @Mixin(CustomTarget.class)
            abstract class ExampleMixin {
                @WrapMethod(method = "compute()L$customType;")
            }
        """.trimIndent()
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val fix = customService.generateHandlerFixes(
            documentUri = documentUri,
            source = source,
            site = site,
            mixinTargets = listOf(targetOwner),
        ).filterIsInstance<WorkspaceEditFix>().single()
        val stubEdit = fix.edits.first()
        assertTrue(stubEdit.newText.contains("com.example.target.Custom mcdevHandler(Operation<com.example.target.Custom> original)"))
        assertTrue(fix.edits.none { it.newText.contains("import com.example.target.Custom;") })
    }

    @Test
    fun generatedHandlerRendersObjectArraysWithSuffixNotation() {
        val targetOwner = "com/example/target/ArrayTarget"
        val valueType = "com/example/target/Value"
        val classIndex = FakeClassIndex(
            classes = listOf(
                ClassIndexEntry("ArrayTarget", "com.example.target", targetOwner),
                ClassIndexEntry("Value", "com.example.target", valueType),
            ),
            methods = mapOf(
                targetOwner to listOf(
                    MethodIndexEntry("read", "()[L$valueType;", false, "read(): Value[]"),
                ),
            ),
        )
        val source = """
            package com.example.mixin;
            import com.example.target.ArrayTarget;
            @Mixin(ArrayTarget.class)
            abstract class ExampleMixin {
                @WrapMethod(method = "read()[L$valueType;")
            }
        """.trimIndent()
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val fix = MixinExtrasCodeActionService(classIndex).generateHandlerFixes(
            documentUri = documentUri,
            source = source,
            site = site,
            mixinTargets = listOf(targetOwner),
        ).filterIsInstance<WorkspaceEditFix>().single()
        val stub = fix.edits.first().newText
        assertTrue(stub.contains("Value[] mcdevHandler(Operation<Value[]> original)"))
        assertFalse(stub.contains("[]Value"))
        val nestedSource = source.replace(valueType, "com/example/target/Outer\$Value")
        val nestedIndex = FakeClassIndex(
            classes = classIndex.findClasses("", 100),
            methods = mapOf(targetOwner to listOf(
                MethodIndexEntry("read", "()[Lcom/example/target/Outer\$Value;", false, "read(): Outer.Value[]"),
            )),
        )
        val nestedFix = MixinExtrasCodeActionService(nestedIndex).generateHandlerFixes(
            documentUri, nestedSource, HandlerSignatureService.findAnnotationSites(nestedSource).single(),
            listOf(targetOwner),
        ).filterIsInstance<WorkspaceEditFix>().single()
        assertTrue(nestedFix.edits.first().newText.contains("com.example.target.Outer.Value[]"))
        assertFalse(nestedFix.edits.any { it.newText.contains("import com.example.target.Outer.Value;") })
    }

    @Test
    fun generatedHandlerQualifiesDistinctPackagesSharingSimpleName() {
        val targetOwner = "com/example/target/TwoFoos"
        val firstType = "a/Foo"
        val secondType = "b/Foo"
        val classIndex = FakeClassIndex(
            classes = listOf(
                ClassIndexEntry("TwoFoos", "com.example.target", targetOwner),
                ClassIndexEntry("Foo", "a", firstType),
                ClassIndexEntry("Foo", "b", secondType),
            ),
            methods = mapOf(
                targetOwner to listOf(
                    MethodIndexEntry(
                        "join",
                        "(L$firstType;L$secondType;)L$firstType;",
                        false,
                        "join(Foo, Foo): Foo",
                    ),
                ),
            ),
        )
        val source = """
            package com.example.mixin;
            import com.example.target.TwoFoos;
            @Mixin(TwoFoos.class)
            abstract class ExampleMixin {
                @WrapMethod(method = "join(L$firstType;L$secondType;)L$firstType;")
            }
        """.trimIndent()
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val fix = MixinExtrasCodeActionService(classIndex).generateHandlerFixes(
            documentUri = documentUri,
            source = source,
            site = site,
            mixinTargets = listOf(targetOwner),
        ).filterIsInstance<WorkspaceEditFix>().single()
        val stub = fix.edits.first().newText
        assertTrue(stub.contains("a.Foo mcdevHandler(a.Foo arg0, b.Foo arg1, Operation<a.Foo> original)"))
        assertTrue(fix.edits.none { it.newText.contains("import a.Foo;") || it.newText.contains("import b.Foo;") })
    }

    @Test
    fun generatedHandlerKeepsMethodNameWhenItMatchesTypeName() {
        val targetOwner = "com/example/target/FooTarget"
        val targetType = "com/example/target/Foo"
        val classIndex = FakeClassIndex(
            classes = listOf(
                ClassIndexEntry("FooTarget", "com.example.target", targetOwner),
                ClassIndexEntry("Foo", "com.example.target", targetType),
            ),
            methods = mapOf(
                targetOwner to listOf(
                    MethodIndexEntry("compute", "()L$targetType;", false, "compute(): Foo"),
                ),
            ),
        )
        val source = """
            package com.example.mixin;
            import com.example.other.Foo;
            import com.example.target.FooTarget;
            @Mixin(FooTarget.class)
            abstract class ExampleMixin {
                @WrapMethod(method = "compute()L$targetType;")
                private int Foo(Operation<Foo> original) { return 0; }
            }
        """.trimIndent()
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val fix = MixinExtrasCodeActionService(classIndex).generateHandlerFixes(
            documentUri = documentUri,
            source = source,
            site = site,
            mixinTargets = listOf(targetOwner),
        ).filterIsInstance<WorkspaceEditFix>().single()
        val stub = fix.edits.first().newText
        assertTrue(stub.contains("com.example.target.Foo Foo(Operation<com.example.target.Foo> original)"))
        assertFalse(stub.contains("com.example.target.Foo com.example.target.Foo("))
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

    @Test
    fun fixMissingOperationPreservesExistingParameterName() {
        val source = wrapOperationHandlerSource(
            handler = """
                private int mcdevWrapLength(String receiver) {
                    return 0;
                }
            """.trimIndent(),
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val diagnostic = McDiagnostic(
            code = MixinExtrasDiagnosticCodes.MISSING_OPERATION_PARAMETER,
            severity = McSeverity.ERROR,
            message = "missing operation",
            range = site.handlerMethod!!.range,
            metadata = mapOf("annotation" to "WrapOperation"),
        )
        val fix = service.fixesForDiagnostics(listOf(diagnostic), documentUri, source)
            .filterIsInstance<WorkspaceEditFix>()
            .single { it.title == "Fix WrapOperation handler signature" }
        assertEquals(
            "int mcdevWrapLength(String receiver, Operation<Integer> original)",
            fix.edits.first().newText,
        )
    }

    @Test
    fun fixMissingOperationRejectsParameterNameCollision() {
        val source = wrapOperationHandlerSource(
            handler = """
                private int mcdevWrapLength(String original) {
                    return 0;
                }
            """.trimIndent(),
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val diagnostic = McDiagnostic(
            code = MixinExtrasDiagnosticCodes.MISSING_OPERATION_PARAMETER,
            severity = McSeverity.ERROR,
            message = "missing operation",
            range = site.handlerMethod!!.range,
            metadata = mapOf("annotation" to "WrapOperation"),
        )
        val fixes = service.fixesForDiagnostics(listOf(diagnostic), documentUri, source)
        assertTrue(fixes.none { it.title == "Fix WrapOperation handler signature" })
    }

    @Test
    fun uniqueResolvedContextFixesExpressionSignatureUsingBytecode() {
        val classIndex = expressionMatchSamplesClassIndex()
        val bytecodeIndex = expressionMatchSamplesBytecodeIndex()
        val service = MixinExtrasCodeActionService(
            classIndex,
            HandlerSignatureService(classIndex, bytecodeIndex),
        )
        val source = wrappedMixinSource(
            expressionMatchSamplesHandlerSource(),
            mixinTarget = "ExpressionMatchSamples",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val diagnostic = McDiagnostic(
            code = MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE,
            severity = McSeverity.ERROR,
            message = "wrong return",
            range = site.handlerMethod!!.range,
            metadata = mapOf("annotation" to "ModifyExpressionValue"),
        )
        val fix = service.fixesForDiagnostics(
            diagnostics = listOf(diagnostic),
            documentUri = documentUri,
            source = source,
            resolvedContexts = listOf(resolvedIntSampleFieldContext(site)),
        ).filterIsInstance<WorkspaceEditFix>().single { it.title == "Fix ModifyExpressionValue handler signature" }
        assertTrue(fix.edits.first().newText.contains("int mcdevHandler(int original)"))
        assertFalse(fix.edits.first().newText.contains("Object"))
    }

    @Test
    fun appliesResolvedIntLikeExpressionFixPreservingBooleanBody() {
        val classIndex = expressionMatchSamplesClassIndex()
        val bytecodeIndex = expressionMatchSamplesBytecodeIndex()
        val signatureService = HandlerSignatureService(classIndex, bytecodeIndex)
        val service = MixinExtrasCodeActionService(classIndex, signatureService)
        val diagnosticsService = MixinExtrasDiagnosticsService(classIndex, bytecodeIndex, signatureService)
        val source = wrappedMixinSource(
            expressionHandlerSource(
                prefix = """@Expression(id = "main", value = "@(true)")""",
                at = """@At(id = "main", value = "MIXINEXTRAS:EXPRESSION")""",
                suffix = "private boolean mcdevHandler(String original) { return !original; }",
                targetMethod = "intEqualsZero(I)Z",
            ),
            mixinTarget = "ExpressionMatchSamples",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val resolvedContexts = listOf(
            resolvedFor(site, ExpressionContextResolver.resolveExpressionContext(source, site)),
        )
        val diagnostics = diagnosticsService.analyze(
            MixinExtrasDiagnosticRequest(
                source = source,
                documentUri = documentUri,
                resolvedContexts = resolvedContexts,
            ),
        )
        assertTrue(
            diagnostics.any { it.code == MixinExtrasDiagnosticCodes.WRONG_ORIGINAL_VALUE_TYPE },
            diagnostics.joinToString(),
        )

        val fix = service.fixesForDiagnostics(
            diagnostics = diagnostics.filter {
                it.code == MixinExtrasDiagnosticCodes.WRONG_ORIGINAL_VALUE_TYPE
            },
            documentUri = documentUri,
            source = source,
            resolvedContexts = resolvedContexts,
        ).filterIsInstance<WorkspaceEditFix>()
            .single { it.title == "Fix ModifyExpressionValue handler signature" }
        assertEquals(
            "boolean mcdevHandler(boolean original)",
            fix.edits.first().newText,
        )

        val applied = applyFix(source, fix)
        assertTrue(applied.contains("private boolean mcdevHandler(boolean original) { return !original; }"))
        assertTrue(applied.contains("return !original;"))

        val correctedSite = HandlerSignatureService.findAnnotationSites(applied).single()
        val correctedContexts = listOf(
            resolvedFor(correctedSite, ExpressionContextResolver.resolveExpressionContext(applied, correctedSite)),
        )
        val correctedDiagnostics = diagnosticsService.analyze(
            MixinExtrasDiagnosticRequest(
                source = applied,
                documentUri = documentUri,
                resolvedContexts = correctedContexts,
            ),
        )
        assertTrue(correctedDiagnostics.isEmpty(), correctedDiagnostics.joinToString())
    }

    @Test
    fun uniqueResolvedContextGeneratesIntHandlerFromBytecode() {
        val classIndex = expressionMatchSamplesClassIndex()
        val bytecodeIndex = expressionMatchSamplesBytecodeIndex()
        val service = MixinExtrasCodeActionService(
            classIndex,
            HandlerSignatureService(classIndex, bytecodeIndex),
        )
        val source = wrappedMixinSource(
            expressionMatchSamplesHandlerSource(suffix = ""),
            mixinTarget = "ExpressionMatchSamples",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        assertNull(site.handlerMethod)
        val fixes = service.generateHandlerFixes(
            documentUri = documentUri,
            source = source,
            site = site,
            mixinTargets = listOf(expressionMatchSamplesOwner()),
            resolvedContexts = listOf(resolvedIntSampleFieldContext(site)),
        )
        val fix = fixes.filterIsInstance<WorkspaceEditFix>().single()
        assertEquals("Generate ModifyExpressionValue handler", fix.title)
        assertTrue(fix.edits.first().newText.contains("int mcdevHandler(int original)"))
        assertFalse(fix.edits.first().newText.contains("Object"))
    }

    @Test
    fun authoritativeEmptyResolvedContextSuppressesGenerationFix() {
        val source = wrappedMixinSource(
            expressionHandlerSource(
                at = """@At(value = "MIXINEXTRAS:EXPRESSION")""",
                suffix = "",
            ),
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        assertNull(site.handlerMethod)
        val fixes = expressionService.generateHandlerFixes(
            documentUri = documentUri,
            source = source,
            site = site,
            mixinTargets = listOf("com/example/target/SimpleTarget"),
            resolvedContexts = listOf(resolvedFor(site, emptyExpressionContext())),
        )
        assertTrue(fixes.isEmpty())
    }

    @Test
    fun ambiguousResolvedContextsFallbackToHandwrittenExpressionGeneration() {
        val source = wrappedMixinSource(
            expressionHandlerSource(
                prefix = """@Expression("text.length()")""",
                suffix = "",
            ),
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        assertNull(site.handlerMethod)
        val ambiguousContexts = listOf(
            resolvedFor(site, expressionContext(MixinExtrasExpression(values = listOf("text.length()")))),
            resolvedFor(
                site,
                expressionContext(MixinExtrasExpression(values = listOf("this.label"))),
            ),
        )
        assertNull(selectResolvedMixinExtrasContext(site, ambiguousContexts))
        val fix = expressionService.generateHandlerFixes(
            documentUri = documentUri,
            source = source,
            site = site,
            mixinTargets = listOf("com/example/target/SimpleTarget"),
            resolvedContexts = ambiguousContexts,
        ).filterIsInstance<WorkspaceEditFix>().single()
        assertEquals("Generate ModifyExpressionValue handler", fix.title)
        assertTrue(fix.edits.first().newText.contains("int mcdevHandler(int original)"))
        assertFalse(fix.edits.first().newText.contains("String mcdevHandler"))
    }

    @Test
    fun authoritativeEmptyResolvedContextSuppressesBogusObjectFix() {
        val source = wrappedMixinSource(
            expressionHandlerSource(
                at = """@At(value = "MIXINEXTRAS:EXPRESSION")""",
                suffix = "private float mcdevHandler(float original) { return original; }",
            ),
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val diagnostic = McDiagnostic(
            code = MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE,
            severity = McSeverity.ERROR,
            message = "wrong return",
            range = site.handlerMethod!!.range,
            metadata = mapOf("annotation" to "ModifyExpressionValue"),
        )
        val fixes = expressionService.fixesForDiagnostics(
            diagnostics = listOf(diagnostic),
            documentUri = documentUri,
            source = source,
            resolvedContexts = listOf(resolvedFor(site, emptyExpressionContext())),
        )
        assertTrue(fixes.none { it.title == "Fix ModifyExpressionValue handler signature" })
        assertTrue(fixes.none { it.title == "Generate ModifyExpressionValue handler" })
    }

    @Test
    fun emptyResolvedContextsPreservesHandwrittenExpressionFix() {
        val source = wrappedMixinSource(
            expressionHandlerSource(
                prefix = """@Expression("text.length()")""",
                suffix = "private float mcdevHandler(float original) { return original; }",
            ),
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val diagnostic = McDiagnostic(
            code = MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE,
            severity = McSeverity.ERROR,
            message = "wrong return",
            range = site.handlerMethod!!.range,
            metadata = mapOf("annotation" to "ModifyExpressionValue"),
        )
        val fix = expressionService.fixesForDiagnostics(
            diagnostics = listOf(diagnostic),
            documentUri = documentUri,
            source = source,
        ).filterIsInstance<WorkspaceEditFix>().single { it.title == "Fix ModifyExpressionValue handler signature" }
        assertTrue(fix.edits.first().newText.contains("int mcdevHandler(int original)"))
    }

    @Test
    fun ambiguousResolvedContextsFallbackToHandwrittenExpressionFix() {
        val source = wrappedMixinSource(
            expressionHandlerSource(
                prefix = """@Expression("text.length()")""",
                suffix = "private float mcdevHandler(float original) { return original; }",
            ),
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val diagnostic = McDiagnostic(
            code = MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE,
            severity = McSeverity.ERROR,
            message = "wrong return",
            range = site.handlerMethod!!.range,
            metadata = mapOf("annotation" to "ModifyExpressionValue"),
        )
        val ambiguousContexts = listOf(
            resolvedFor(site, expressionContext(MixinExtrasExpression(values = listOf("text.length()")))),
            resolvedFor(
                site,
                expressionContext(MixinExtrasExpression(values = listOf("this.label"))),
            ),
        )
        assertNull(selectResolvedMixinExtrasContext(site, ambiguousContexts))
        val fix = expressionService.fixesForDiagnostics(
            diagnostics = listOf(diagnostic),
            documentUri = documentUri,
            source = source,
            resolvedContexts = ambiguousContexts,
        ).filterIsInstance<WorkspaceEditFix>().single { it.title == "Fix ModifyExpressionValue handler signature" }
        assertTrue(fix.edits.first().newText.contains("int mcdevHandler(int original)"))
    }

    @Test
    fun fixOperationCallArgumentsEmptyInsertion() {
        val source = wrapOperationHandlerSource(
            handler = """
                private int mcdevWrapLength(String instance, Operation<Integer> original) {
                    return original.call();
                }
            """.trimIndent(),
        )
        val diagnostic = operationCallDiagnostic(source)
        val fix = operationCallFix(source, diagnostic)
        assertEquals("instance", fix.edits.single().newText)
        assertEquals("quickfix.mixinextras.fixOperationCallArguments", fix.kind)
        assertAppliedCallArguments(source, fix, "instance")
    }

    @Test
    fun fixOperationCallArgumentsExtraArgsReplacement() {
        val source = wrapOperationHandlerSource(
            handler = """
                private int mcdevWrapLength(String instance, Operation<Integer> original) {
                    return original.call(instance, instance);
                }
            """.trimIndent(),
        )
        val diagnostic = operationCallDiagnostic(source)
        val fix = operationCallFix(source, diagnostic)
        assertEquals("instance", fix.edits.single().newText)
        assertAppliedCallArguments(source, fix, "instance")
    }

    @Test
    fun fixOperationCallArgumentsUsesCustomHandlerParameterNames() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;charAt(I)C"))
                private char mcdevWrapChar(String receiver, int value, Operation<Character> original) {
                    return original.call();
                }
            }
        """.trimIndent()
        val diagnostic = operationCallDiagnostic(source)
        val fix = operationCallFix(source, diagnostic)
        assertEquals("receiver, value", fix.edits.single().newText)
        assertAppliedCallArguments(source, fix, "receiver, value")
    }

    @Test
    fun fixOperationCallArgumentsZeroExpectedArgsBecomesEmptyText() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @WrapMethod(method = "noop()V")
                private void mcdevWrapNoop(Operation<Void> original) {
                    original.call(instance);
                }
            }
        """.trimIndent()
        val diagnostic = operationCallDiagnostic(source)
        val fix = operationCallFix(source, diagnostic)
        assertEquals("", fix.edits.single().newText)
        assertAppliedCallArguments(source, fix, "")
    }

    @Test
    fun fixOperationCallArgumentsPreservesExteriorWhitespaceInsideParens() {
        val source = wrapOperationHandlerSource(
            handler = """
                private int mcdevWrapLength(String instance, Operation<Integer> original) {
                    return original.call(  instance, instance  );
                }
            """.trimIndent(),
        )
        val diagnostic = operationCallDiagnostic(source)
        val fix = operationCallFix(source, diagnostic)
        assertEquals("instance", fix.edits.single().newText)
        val applied = applyFix(source, fix)
        assertTrue(applied.contains("original.call(  instance  );"))
    }

    @Test
    fun fixOperationCallArgumentsProducesTwoFixesForTwoWrongCalls() {
        val source = wrapOperationHandlerSource(
            handler = """
                private int mcdevWrapLength(String instance, Operation<Integer> original) {
                    original.call();
                    return original.call(instance, instance);
                }
            """.trimIndent(),
        )
        val diagnostics = operationCallDiagnostics(source)
        assertEquals(2, diagnostics.size)
        val fixes = service.fixesForDiagnostics(diagnostics, documentUri, source)
            .filterIsInstance<WorkspaceEditFix>()
            .filter { it.title == "Fix Operation.call arguments" }
        assertEquals(2, fixes.size)
        assertEquals(setOf("instance"), fixes.map { it.edits.single().newText }.toSet())
        assertEquals(2, fixes.map { it.edits.single().startOffset }.toSet().size)
    }

    @Test
    fun fixOperationCallArgumentsDisambiguatesRepeatedMethodSelectorsByExactRange() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
                private int mcdevWrapLengthA(String instance, Operation<Integer> original) {
                    return original.call();
                }

                @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
                private int mcdevWrapLengthB(String instance, Operation<Integer> original) {
                    return original.call(instance, instance);
                }
            }
        """.trimIndent()
        val diagnostics = operationCallDiagnostics(source)
        assertEquals(2, diagnostics.size)
        val fixes = service.fixesForDiagnostics(diagnostics, documentUri, source)
            .filterIsInstance<WorkspaceEditFix>()
            .filter { it.title == "Fix Operation.call arguments" }
        assertEquals(2, fixes.size)
        val fixByRange = diagnostics.associate { diagnostic ->
            diagnostic.range to fixes.single { fix ->
                fix.edits.single().startOffset == rangeStart(source, diagnostic.range)
            }
        }
        assertEquals("instance", fixByRange[diagnostics[0].range]!!.edits.single().newText)
        assertEquals("instance", fixByRange[diagnostics[1].range]!!.edits.single().newText)
    }

    @Test
    fun fixOperationCallArgumentsOffersNothingForStaleDiagnosticRange() {
        val source = wrapOperationHandlerSource(
            handler = """
                private int mcdevWrapLength(String instance, Operation<Integer> original) {
                    return original.call();
                }
            """.trimIndent(),
        )
        val staleDiagnostic = McDiagnostic(
            code = MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS,
            severity = McSeverity.ERROR,
            message = "stale",
            range = McTextRange(McTextPosition(0, 0), McTextPosition(0, 1)),
        )
        assertTrue(service.fixesForDiagnostics(listOf(staleDiagnostic), documentUri, source).isEmpty())
    }

    @Test
    fun fixOperationCallArgumentsOffersNothingForAlreadyFixedCall() {
        val source = wrapOperationHandlerSource(
            handler = """
                private int mcdevWrapLength(String instance, Operation<Integer> original) {
                    return original.call(instance);
                }
            """.trimIndent(),
        )
        val diagnostic = McDiagnostic(
            code = MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS,
            severity = McSeverity.ERROR,
            message = "fixed",
            range = McTextRange(McTextPosition(0, 0), McTextPosition(0, 1)),
        )
        assertTrue(service.fixesForDiagnostics(listOf(diagnostic), documentUri, source).isEmpty())
    }

    @Test
    fun fixOperationCallArgumentsOffersNothingForRangeShiftedDiagnostic() {
        val source = wrapOperationHandlerSource(
            handler = """
                private int mcdevWrapLength(String instance, Operation<Integer> original) {
                    return original.call();
                }
            """.trimIndent(),
        )
        val diagnostic = operationCallDiagnostic(source)
        val shifted = diagnostic.copy(
            range = McTextRange(
                McTextPosition(diagnostic.range.start.line, diagnostic.range.start.character + 1),
                McTextPosition(diagnostic.range.end.line, diagnostic.range.end.character + 1),
            ),
        )
        assertTrue(service.fixesForDiagnostics(listOf(shifted), documentUri, source).isEmpty())
    }

    @Test
    fun fixOperationCallArgumentsIgnoresForgedMetadataExpectedArguments() {
        val source = wrapOperationHandlerSource(
            handler = """
                private int mcdevWrapLength(String instance, Operation<Integer> original) {
                    return original.call();
                }
            """.trimIndent(),
        )
        val diagnostic = operationCallDiagnostic(source).copy(
            metadata = mapOf(
                "annotation" to "WrapOperation",
                "method" to "draw(Ljava/lang/String;FF)V",
                "expectedArguments" to "forged, args",
            ),
        )
        val fix = operationCallFix(source, diagnostic)
        assertEquals("instance", fix.edits.single().newText)
    }

    @Test
    fun fixOperationCallArgumentsOffersNothingWhenOperationParameterMissing() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            ${MixinExtrasTestFixtures.WRAP_OPERATION_MISSING_OP.trimIndent()}
        """.trimIndent()
        val diagnostic = McDiagnostic(
            code = MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS,
            severity = McSeverity.ERROR,
            message = "wrong",
            range = HandlerSignatureService.findAnnotationSites(source).first().handlerMethod!!.range,
            metadata = mapOf("expectedArguments" to "instance"),
        )
        assertTrue(service.fixesForDiagnostics(listOf(diagnostic), documentUri, source).isEmpty())
    }

    @Test
    fun fixOperationCallArgumentsOffersNothingWhenOperationParameterMisplaced() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            ${MixinExtrasTestFixtures.WRAP_OPERATION_OP_NOT_LAST.trimIndent()}
        """.trimIndent()
        val diagnostic = McDiagnostic(
            code = MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS,
            severity = McSeverity.ERROR,
            message = "wrong",
            range = McTextRange(McTextPosition(0, 0), McTextPosition(0, 0)),
            metadata = mapOf("expectedArguments" to "instance"),
        )
        assertTrue(service.fixesForDiagnostics(listOf(diagnostic), documentUri, source).isEmpty())
    }

    @Test
    fun fixOperationCallArgumentsOffersNothingWhenSugarInterleavesRequiredParameters() {
        val source = wrapOperationHandlerSource(
            handler = """
                private int mcdevWrapLength(String instance, @Local int counter, Operation<Integer> original) {
                    return original.call();
                }
            """.trimIndent(),
        )
        val diagnostic = McDiagnostic(
            code = MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS,
            severity = McSeverity.ERROR,
            message = "wrong",
            range = McTextRange(McTextPosition(0, 0), McTextPosition(0, 0)),
            metadata = mapOf("expectedArguments" to "instance"),
        )
        assertTrue(service.fixesForDiagnostics(listOf(diagnostic), documentUri, source).isEmpty())
    }

    @Test
    fun fixOperationCallArgumentsOffersNothingWhenHandlerDeclaresMultipleOperationParameters() {
        val source = wrapOperationHandlerSource(
            handler = """
                private int mcdevWrapLength(String instance, Operation<Integer> original, Operation<Integer> duplicate) {
                    return original.call();
                }
            """.trimIndent(),
        )
        val diagnostic = McDiagnostic(
            code = MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS,
            severity = McSeverity.ERROR,
            message = "wrong",
            range = McTextRange(McTextPosition(0, 0), McTextPosition(0, 0)),
        )
        assertTrue(service.fixesForDiagnostics(listOf(diagnostic), documentUri, source).isEmpty())
    }

    @Test
    fun fixOperationCallArgumentsLeavesUnrelatedDiagnosticBehaviorUnchanged() {
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
        assertTrue(fixes.none { it.title == "Fix Operation.call arguments" })
    }

    @Test
    fun facadeSurvivesMultipleSameTitleOperationCallFixesAfterDeduplication() {
        val source = wrapOperationHandlerSource(
            handler = """
                private int mcdevWrapLength(String instance, Operation<Integer> original) {
                    original.call();
                    return original.call(instance, instance);
                }
            """.trimIndent(),
        )
        val facade = MixinServiceFacade(
            classIndex = MixinExtrasTestFixtures.classIndex,
            bytecodeIndex = MixinExtrasTestFixtures.bytecodeIndex,
        )
        val request = MixinFacadeRequest(
            bufferText = source,
            line = 0,
            character = 0,
            documentUri = documentUri,
        )
        val diagnostics = facade.diagnose(request).filter {
            it.code == MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS
        }
        assertEquals(2, diagnostics.size)
        val fixes = facade.codeActions(
            request,
            MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS,
        ).filterIsInstance<WorkspaceEditFix>()
            .filter { it.title == "Fix Operation.call arguments" }
        assertEquals(2, fixes.size)
    }

    @Test
    fun facadeForwardsSemanticResolvedContextsToCodeActions() {
        val classIndex = expressionMatchSamplesClassIndex()
        val bytecodeIndex = expressionMatchSamplesBytecodeIndex()
        val source = wrappedMixinSource(
            expressionMatchSamplesHandlerSource(),
            mixinTarget = "ExpressionMatchSamples",
        )
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val facade = MixinServiceFacade(
            classIndex = classIndex,
            bytecodeIndex = bytecodeIndex,
        )
        val request = MixinFacadeRequest(
            bufferText = source,
            line = site.handlerMethod!!.range.start.line,
            character = site.handlerMethod!!.range.start.character,
            documentUri = documentUri,
            semanticModel = MixinClassModel(
                targets = emptyList(),
                injectors = emptyList(),
                resolvedMixinExtrasContexts = listOf(resolvedIntSampleFieldContext(site)),
            ),
        )
        assertTrue(
            facade.diagnose(request).any { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE },
            "facade diagnostics must derive WRONG_RETURN_TYPE from semantic resolved context",
        )
        val fixes = facade.codeActions(
            request,
            MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE,
        )
        val fix = fixes.filterIsInstance<WorkspaceEditFix>().single { it.title == "Fix ModifyExpressionValue handler signature" }
        assertTrue(fix.edits.first().newText.contains("int mcdevHandler(int original)"))
    }

    @Test
    fun fixOperationCallArgumentsCrlfEditOffsetsMatchSourceIndex() {
        val source = crlfWrapOperationHandlerSource(
            handler = """
                private int mcdevWrapLength(String instance, Operation<Integer> original) {
                    return original.call();
                }
            """.trimIndent(),
        )
        val diagnostic = operationCallDiagnostic(source)
        val fix = operationCallFix(source, diagnostic)
        val edit = fix.edits.single()

        val callPrefix = "original.call("
        val contentStart = source.indexOf(callPrefix) + callPrefix.length
        val contentEnd = source.indexOf(')', contentStart)
        assertEquals(contentStart, edit.startOffset)
        assertEquals(contentEnd, edit.endOffset)
        assertEquals("instance", edit.newText)
        assertAppliedCallArguments(source, fix, "instance")
    }

    @Test
    fun fixOperationCallArgumentsCrlfTwoHandlersDoNotCrossCapture() {
        val source = crlfSource("""
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
                private int mcdevWrapLengthA(String instance, Operation<Integer> original) {
                    return original.call(instance);
                }

                @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
                private int mcdevWrapLengthB(String instance, Operation<Integer> original) {
                    return original.call();
                }
            }
        """)
        val diagnostics = operationCallDiagnostics(source)
        assertEquals(1, diagnostics.size)

        val fix = operationCallFix(source, diagnostics.single())
        val edit = fix.edits.single()
        val callPrefix = "original.call("
        val secondHandler = source.indexOf("mcdevWrapLengthB")
        val contentStart = source.indexOf(callPrefix, secondHandler) + callPrefix.length
        val contentEnd = source.indexOf(')', contentStart)
        assertEquals(contentStart, edit.startOffset)
        assertEquals(contentEnd, edit.endOffset)
        assertEquals("instance", edit.newText)
        assertAppliedCallArguments(source, fix, "instance")
    }

    @Test
    fun fixOperationCallArgumentsCrlfStaleDiagnosticOffersNothing() {
        val source = crlfWrapOperationHandlerSource(
            handler = """
                private int mcdevWrapLength(String instance, Operation<Integer> original) {
                    return original.call();
                }
            """.trimIndent(),
        )
        val staleDiagnostic = McDiagnostic(
            code = MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS,
            severity = McSeverity.ERROR,
            message = "stale",
            range = McTextRange(McTextPosition(0, 0), McTextPosition(0, 1)),
        )
        assertTrue(service.fixesForDiagnostics(listOf(staleDiagnostic), documentUri, source).isEmpty())
    }

    private fun crlfSource(block: String): String = block.trimIndent().replace("\n", "\r\n")

    private fun crlfWrapOperationHandlerSource(handler: String): String = crlfSource("""
        @Mixin(com.example.target.SimpleTarget.class)
        abstract class ExampleMixin {
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            $handler
        }
    """)

    private fun wrappedMixinSource(
        body: String,
        mixinTarget: String = "com.example.target.SimpleTarget",
    ): String = """
        @Mixin($mixinTarget.class)
        abstract class ExampleMixin {
        ${body.trimIndent()}
        }
    """.trimIndent()

    private fun expressionHandlerSource(
        prefix: String = "",
        at: String = """@At(value = "MIXINEXTRAS:EXPRESSION")""",
        suffix: String,
        targetMethod: String = "draw(Ljava/lang/String;FF)V",
    ): String = """
        $prefix
        @ModifyExpressionValue(method = "$targetMethod", at = $at)
        $suffix
    """.trimIndent()

    private fun expressionMatchSamplesHandlerSource(
        suffix: String = "private String mcdevHandler(String original) { return original; }",
    ): String = expressionHandlerSource(
        prefix = """
            @Definition(id = "sampleFieldRef", field = "Lio/github/mcdev/core/bytecode/fixtures/ExpressionMatchSamples;sampleField:I")
            @Expression(id = "main", value = "this.label")
        """.trimIndent(),
        at = """@At(id = "main", value = "MIXINEXTRAS:EXPRESSION")""",
        suffix = suffix,
        targetMethod = "readSampleField()I",
    )

    private fun resolvedIntSampleFieldContext(site: MixinExtrasAnnotationSite): ResolvedMixinExtrasContext =
        resolvedFor(
            site,
            ExpressionContext(
                expressionIndex = MixinExtrasExpressionIndex(
                    expressions = listOf(
                        MixinExtrasExpression(id = "main", values = listOf("@(?.sampleFieldRef)")),
                    ),
                ),
                definitionIndex = MixinExtrasDefinitionIndex(
                    definitions = listOf(
                        MixinExtrasDefinition(
                            id = "sampleFieldRef",
                            rawFieldReferences = listOf(
                                "Lio/github/mcdev/core/bytecode/fixtures/ExpressionMatchSamples;sampleField:I",
                            ),
                        ),
                    ),
                ),
            ),
        )

    private fun expressionMatchSamplesOwner(): String =
        BytecodeFixtureCompiler.internalName("ExpressionMatchSamples")

    private fun expressionMatchSamplesClassIndex(): ClassIndex {
        val owner = expressionMatchSamplesOwner()
        return FakeClassIndex(
            classes = listOf(
                ClassIndexEntry(
                    "ExpressionMatchSamples",
                    "io.github.mcdev.core.bytecode.fixtures",
                    owner,
                ),
                ClassIndexEntry(
                    "String",
                    "java.lang",
                    "java/lang/String",
                ),
            ),
            methods = mapOf(
                owner to listOf(
                    MethodIndexEntry("readSampleField", "()I", false, "readSampleField(): int"),
                    MethodIndexEntry("intEqualsZero", "(I)Z", false, "intEqualsZero(int): boolean"),
                ),
            ),
        )
    }

    private fun expressionMatchSamplesBytecodeIndex(): BytecodeIndex {
        val owner = expressionMatchSamplesOwner()
        val classBytes = BytecodeFixtureCompiler.classBytes("ExpressionMatchSamples")
        val commonSuperClassResolver = BytecodeCommonSuperClassResolver(
            classBytesLookup = { internalName ->
                if (internalName == owner) classBytes else null
            },
        )
        val delegate = FakeBytecodeIndex()
        return object : BytecodeIndex {
            override fun getAtTargetCandidates(
                ownerInternalName: String,
                methodName: String,
                methodDescriptor: String?,
                atValue: String,
            ): List<AtTargetCandidate> =
                delegate.getAtTargetCandidates(ownerInternalName, methodName, methodDescriptor, atValue)

            override fun getReturnOrdinalCount(
                ownerInternalName: String,
                methodName: String,
                methodDescriptor: String?,
            ): Int = delegate.getReturnOrdinalCount(ownerInternalName, methodName, methodDescriptor)

            override fun getClassBytes(ownerInternalName: String): ByteArray? =
                if (ownerInternalName == owner) classBytes else null

            override fun resolveCommonSuperClass(type1Descriptor: String, type2Descriptor: String): String? =
                commonSuperClassResolver.resolve(type1Descriptor, type2Descriptor)
                    ?: expressionMatchSamplesFallbackCommonSuper(type1Descriptor, type2Descriptor)
        }
    }

    private fun expressionMatchSamplesFallbackCommonSuper(type1Descriptor: String, type2Descriptor: String): String? {
        val type1 = Type.getType(type1Descriptor)
        val type2 = Type.getType(type2Descriptor)
        return when {
            type1 == type2 -> type1Descriptor
            type1.sort == Type.OBJECT && type2.sort == Type.OBJECT ->
                Type.getObjectType("java/lang/Object").descriptor
            else -> null
        }
    }

    private fun resolvedFor(
        site: MixinExtrasAnnotationSite,
        context: ExpressionContext,
    ): ResolvedMixinExtrasContext = ResolvedMixinExtrasContext(
        handlerRange = semanticHandlerRange(site),
        context = context,
    )

    private fun semanticHandlerRange(site: MixinExtrasAnnotationSite): McTextRange {
        val handlerRange = site.handlerMethod?.range ?: return site.annotationRange
        return McTextRange(site.annotationRange.start, handlerRange.end)
    }

    private fun emptyExpressionContext() = ExpressionContext(
        expressionIndex = MixinExtrasExpressionIndex(),
        definitionIndex = MixinExtrasDefinitionIndex(),
    )

    private fun expressionContext(vararg expressions: MixinExtrasExpression) = ExpressionContext(
        expressionIndex = MixinExtrasExpressionIndex(expressions.toList()),
        definitionIndex = MixinExtrasDefinitionIndex(),
    )

    private val operationCallDiagnosticsService = MixinExtrasDiagnosticsService(
        MixinExtrasTestFixtures.classIndex,
        MixinExtrasTestFixtures.bytecodeIndex,
    )

    private fun wrapOperationHandlerSource(handler: String): String = """
        @Mixin(com.example.target.SimpleTarget.class)
        abstract class ExampleMixin {
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            $handler
        }
    """.trimIndent()

    private fun wrapOperationAtArrayHandlerSource(atArray: String, handler: String): String = """
        @Mixin(com.example.target.SimpleTarget.class)
        abstract class ExampleMixin {
            @WrapOperation(
                method = "draw(Ljava/lang/String;FF)V",
                at = {
                    $atArray
                }
            )
            $handler
        }
    """.trimIndent()

    private fun wrapOperationMethodArraySource(methods: String, handler: String): String = """
        @Mixin(com.example.target.SimpleTarget.class)
        abstract class ExampleMixin {
            @WrapOperation(
                method = { $methods },
                at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I")
            )
            $handler
        }
    """.trimIndent()

    private fun methodArrayCapturedTailClassIndex(): ClassIndex = FakeClassIndex(
        classes = listOf(
            ClassIndexEntry("String", "java.lang", "java/lang/String"),
            ClassIndexEntry("SimpleTarget", "com.example.target", "com/example/target/SimpleTarget"),
        ),
        methods = mapOf(
            "com/example/target/SimpleTarget" to listOf(
                MethodIndexEntry("foo", "(I)I", false, "foo(int): int"),
                MethodIndexEntry("bar", "(J)I", false, "bar(long): int"),
            ),
        ),
    )

    private fun stringApplyClassIndex(): ClassIndex = FakeClassIndex(
        classes = listOf(
            ClassIndexEntry("StringTarget", "com.example.target", "com/example/target/StringTarget"),
            ClassIndexEntry("String", "java.lang", "java/lang/String"),
        ),
        methods = mapOf(
            "com/example/target/StringTarget" to listOf(
                MethodIndexEntry("apply", "(Ljava/lang/String;)Ljava/lang/String;", false, "apply(String): String"),
            ),
        ),
    )

    private fun stringApplyObjectReturnClassIndex(): ClassIndex = FakeClassIndex(
        classes = listOf(
            ClassIndexEntry("StringTarget", "com.example.target", "com/example/target/StringTarget"),
            ClassIndexEntry("String", "java.lang", "java/lang/String"),
        ),
        methods = mapOf(
            "com/example/target/StringTarget" to listOf(
                MethodIndexEntry("apply", "(Ljava/lang/String;)Ljava/lang/Object;", false, "apply(String): Object"),
            ),
        ),
    )

    private fun cancellableTargetClassIndex(): ClassIndex = FakeClassIndex(
        classes = FakeClassIndex.defaultClasses() + listOf(
            ClassIndexEntry("VoidTarget", "com.example.target", "com/example/target/VoidTarget"),
            ClassIndexEntry("IntTarget", "com.example.target", "com/example/target/IntTarget"),
            ClassIndexEntry(
                "CallbackInfo",
                "org.spongepowered.asm.mixin.injection.callback",
                "org/spongepowered/asm/mixin/injection/callback/CallbackInfo",
            ),
            ClassIndexEntry(
                "CallbackInfoReturnable",
                "org.spongepowered.asm.mixin.injection.callback",
                "org/spongepowered/asm/mixin/injection/callback/CallbackInfoReturnable",
            ),
        ),
        methods = FakeClassIndex.defaultMethods() + mapOf(
            "com/example/target/VoidTarget" to listOf(
                MethodIndexEntry("run", "()V", false, "run(): void"),
            ),
            "com/example/target/IntTarget" to listOf(
                MethodIndexEntry("run", "()I", false, "run(): int"),
            ),
        ),
    )

    private fun wrapOperationSignatureDiagnostic() = McDiagnostic(
        code = MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE,
        severity = McSeverity.ERROR,
        message = "wrong return",
        range = McTextRange(McTextPosition(0, 0), McTextPosition(0, 0)),
        metadata = mapOf("annotation" to "WrapOperation"),
    )

    private fun operationCallDiagnostics(source: String): List<McDiagnostic> =
        operationCallDiagnosticsService.analyze(
            MixinExtrasDiagnosticRequest(source = source, documentUri = documentUri),
        ).filter { it.code == MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS }

    private fun operationCallDiagnostic(source: String): McDiagnostic = operationCallDiagnostics(source).single()

    private fun operationCallFix(source: String, diagnostic: McDiagnostic): WorkspaceEditFix =
        service.fixesForDiagnostics(listOf(diagnostic), documentUri, source)
            .filterIsInstance<WorkspaceEditFix>()
            .single { it.title == "Fix Operation.call arguments" }

    private fun applyFix(source: String, fix: WorkspaceEditFix): String {
        var applied = source
        fix.edits.sortedByDescending { it.startOffset }.forEach { edit ->
            applied = buildString {
                append(applied.substring(0, edit.startOffset))
                append(edit.newText)
                append(applied.substring(edit.endOffset))
            }
        }
        return applied
    }

    private fun assertAppliedCallArguments(source: String, fix: WorkspaceEditFix, expectedArguments: String) {
        val applied = applyFix(source, fix)
        val openIndex = applied.indexOf("original.call(")
        assertTrue(openIndex >= 0)
        val contentStart = openIndex + "original.call(".length
        val closeIndex = applied.indexOf(')', contentStart)
        assertTrue(closeIndex >= 0)
        val actual = applied.substring(contentStart, closeIndex).trim()
        assertEquals(expectedArguments, actual)
    }

    private fun rangeStart(source: String, range: McTextRange): Int =
        AnnotationContextExtractor.toOffset(source, range.start.line, range.start.character)!!

    private fun rangeEnd(source: String, range: McTextRange): Int =
        AnnotationContextExtractor.toOffset(source, range.end.line, range.end.character)!!
}
