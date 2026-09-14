package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.bytecode.BytecodeFixtureCompiler
import io.github.mcdev.core.bytecode.OccurrenceResultClassification
import io.github.mcdev.core.diagnostics.McSeverity
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.AtTargetKind
import io.github.mcdev.core.mixin.AtTargetOperationKind
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FakeBytecodeIndex
import io.github.mcdev.core.mixin.FakeClassIndex
import io.github.mcdev.core.mixin.MethodIndexEntry
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

class MixinExtrasDiagnosticsServiceTest {
    private val service = MixinExtrasDiagnosticsService(
        MixinExtrasTestFixtures.classIndex,
        MixinExtrasTestFixtures.bytecodeIndex,
    )

    @Test
    fun malformedDefinitionValueHighlightsTheBadValueAndKeepsValidFieldBinding() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @Definition(id = "value", method = 123, field = "Lcom/example/target/SimpleTarget;counter:I")
                @Expression("this.value")
                @Inject(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
                private void handler(CallbackInfo ci) {}
            }
        """.trimIndent()
        val issue = analyze(source).single { it.code == MixinExtrasDiagnosticCodes.INVALID_DEFINITION }
        assertEquals("123", source.lines()[issue.range.start.line]
            .substring(issue.range.start.character, issue.range.end.character))
        val context = ExpressionContextResolver.resolveExpressionContext(source,
            HandlerSignatureService.findSugarHandlerAnnotationSites(source).single())
        val built = OfficialExpressionIdentifierPoolBuilder.build(context.definitionIndex)
        assertTrue(built.pool.delegate.memberExists("value"))
    }

    @Test
    fun invalidDefinitionsAreReportedAtTheirOwnAnnotationAndValidSiblingsRemainUsable() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @Definition(id = "bad", method = "not a method selector(")
                @Definition(field = "Lcom/example/target/SimpleTarget;counter:I")
                @Definition(id = "good", field = "Lcom/example/target/SimpleTarget;counter:I")
                @Expression("this.good")
                @Inject(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
                private void handler(CallbackInfo ci) {}
            }
        """.trimIndent()
        val issues = analyze(source).filter { it.code == MixinExtrasDiagnosticCodes.INVALID_DEFINITION }
        assertEquals(2, issues.size, issues.toString())
        assertTrue(issues.any { source.lines()[it.range.start.line].contains("id = \"bad\"") })
        assertTrue(issues.any { source.lines()[it.range.start.line].contains("@Definition(field") })
        val site = HandlerSignatureService.findSugarHandlerAnnotationSites(source).single()
        val context = ExpressionContextResolver.resolveExpressionContext(source, site)
        val built = OfficialExpressionIdentifierPoolBuilder.build(context.definitionIndex)
        assertTrue(built.pool.delegate.memberExists("good"), "invalid sibling must not discard a valid binding")
    }

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
    fun deduplicatesIdenticalWrongReturnDiagnosticAcrossMultipleAtSites() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @ModifyExpressionValue(
                    method = "draw(Ljava/lang/String;FF)V",
                    at = {
                        @At(value = "CONSTANT", args = "floatValue=0.0"),
                        @At(value = "CONSTANT", args = "floatValue=1.0"),
                    },
                )
                private int mcdevHandler(float original) { return original; }
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        assertEquals(1, diagnostics.count { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE })
    }

    @Test
    fun retainsWrongReturnDiagnosticWhenLaterAtSiteRequiresDifferentType() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @ModifyExpressionValue(
                    method = "draw(Ljava/lang/String;FF)V",
                    at = {
                        @At(value = "CONSTANT", args = "floatValue=0.0"),
                        @At(value = "CONSTANT", args = "intValue=0"),
                    },
                )
                private float mcdevHandler(float original) { return original; }
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        val wrongReturn = diagnostics.filter { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE }
        assertEquals(1, wrongReturn.size)
        assertTrue(wrongReturn.single().message.contains("int"))
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
    fun noDiagnosticsForValidModifyReturnValue() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            ${MixinExtrasTestFixtures.MODIFY_RETURN_SOURCE.trimIndent()}
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.none { it.severity == McSeverity.ERROR })
    }

    @Test
    fun validatesModifyReturnValueAgainstEachMixinTarget() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class FirstMixin {
                @ModifyReturnValue(method = "compute()I", at = @At("RETURN"))
                private int first(int original) { return original; }
            }
            @Mixin(com.example.target.SharedMixinTargetA.class)
            abstract class SecondMixin {
                @ModifyReturnValue(method = "shared()I", at = @At("RETURN"))
                private void second(int original) {}
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE })
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
    fun expressionAtValueWithoutExpressionAnnotationProducesError() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
                private int mcdev${'$'}handler(int original) { return original; }
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.MISSING_EXPRESSION_ANNOTATION })
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.UNSUPPORTED_EXPRESSION_CONTEXT })
    }

    @Test
    fun expressionAtValueValidatesHandlerAgainstInferredInvokeType() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
                @Expression("text.length()")
                private int mcdev${'$'}handler(int original) { return original; }
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.UNSUPPORTED_EXPRESSION_CONTEXT })
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE })
    }

    @Test
    fun expressionAtValueReportsWrongReturnTypeWhenInferredTypeDiffers() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
                @Expression("text.length()")
                private float mcdev${'$'}handler(float original) { return original; }
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE })
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.UNSUPPORTED_EXPRESSION_CONTEXT })
    }

    @Test
    fun noUnsupportedExpressionContextForVoidArrayStoreWrapOperation() {
        val source = """
            @Mixin(ExpressionMatchSamples.class)
            abstract class ExampleMixin {
                @Expression(id = "main", value = "?[?]=?")
                @WrapOperation(
                    method = "arrayStore([III)V",
                    at = @At(id = "main", value = "MIXINEXTRAS:EXPRESSION"),
                )
                private void mcdevHandler(int[] array, int index, int value, Operation<Void> original) {
                    original.call(array, index, value);
                }
            }
        """.trimIndent()
        val diagnostics = analyze(
            source,
            service = MixinExtrasDiagnosticsService(
                expressionMatchSamplesClassIndex(),
                expressionMatchSamplesBytecodeIndex(),
            ),
        )
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.UNSUPPORTED_EXPRESSION_CONTEXT })
    }

    @Test
    fun noDiagnosticsForValidWrapWithConditionExpressionFieldWrite() {
        val source = """
            @Mixin(ExpressionMatchSamples.class)
            abstract class ExampleMixin {
                @Expression("?.?=?")
                @WrapWithCondition(
                    method = "writeSampleField(I)V",
                    at = @At(value = "MIXINEXTRAS:EXPRESSION"),
                )
                private boolean mcdevHandler(ExpressionMatchSamples instance, int arg0) {
                    return true;
                }
            }
        """.trimIndent()
        val diagnostics = analyze(
            source,
            service = MixinExtrasDiagnosticsService(
                expressionMatchSamplesClassIndex(),
                expressionMatchSamplesBytecodeIndex(),
            ),
        )

        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRAP_WITH_CONDITION_INVALID_TARGET })
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun expressionAtIdWithoutMatchingExpressionProducesMissingExpressionDiagnostic() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @ModifyExpressionValue(
                    method = "draw(Ljava/lang/String;FF)V",
                    at = @At(id = "main", value = "MIXINEXTRAS:EXPRESSION"),
                )
                @Expression(id = "other", value = "text.length()")
                private int mcdev${'$'}handler(int original) { return original; }
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.MISSING_EXPRESSION_ANNOTATION })
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.UNSUPPORTED_EXPRESSION_CONTEXT })
    }

    @Test
    fun expressionBeforeInjectorWithMatchingAtIdInfersHandlerType() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @Expression(id = "main", value = "text.length()")
                @ModifyExpressionValue(
                    method = "draw(Ljava/lang/String;FF)V",
                    at = @At(id = "main", value = "MIXINEXTRAS:EXPRESSION"),
                )
                private int mcdev${'$'}handler(int original) { return original; }
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.MISSING_EXPRESSION_ANNOTATION })
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.UNSUPPORTED_EXPRESSION_CONTEXT })
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE })
    }

    @Test
    fun multipleMatchingExpressionsWithSameInferredTypeValidateHandler() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @Expression(id = "main", value = "text.length()")
                @ModifyExpressionValue(
                    method = "draw(Ljava/lang/String;FF)V",
                    at = @At(id = "main", value = "MIXINEXTRAS:EXPRESSION"),
                )
                @Expression(id = "main", value = "text.length()")
                private int mcdev${'$'}handler(int original) { return original; }
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.MISSING_EXPRESSION_ANNOTATION })
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.UNSUPPORTED_EXPRESSION_CONTEXT })
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE })
    }

    @Test
    fun conflictingMatchedExpressionTypesProduceUnsupportedContextDiagnostic() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @Expression(id = "main", value = "text.length()")
                @Expression(id = "main", value = "this.label")
                @ModifyExpressionValue(
                    method = "draw(Ljava/lang/String;FF)V",
                    at = @At(id = "main", value = "MIXINEXTRAS:EXPRESSION"),
                )
                private int mcdev${'$'}handler(int original) { return original; }
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.MISSING_EXPRESSION_ANNOTATION })
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.UNSUPPORTED_EXPRESSION_CONTEXT })
    }

    @Test
    fun commentFakeExpressionIsIgnoredForDiagnostics() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                // @Expression("fake.length()")
                @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
                @Expression("text.length()")
                private int mcdev${'$'}handler(int original) { return original; }
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.MISSING_EXPRESSION_ANNOTATION })
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.UNSUPPORTED_EXPRESSION_CONTEXT })
    }

    @Test
    fun reportsInvalidShareParameterTypeThroughDiagnosticsService() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
                private float mcdevModifyX(float original, @Share String shared) {
                    return original;
                }
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.INVALID_SHARE_PARAMETER_TYPE })
        assertEquals(McSeverity.ERROR, diagnostics.first { it.code == MixinExtrasDiagnosticCodes.INVALID_SHARE_PARAMETER_TYPE }.severity)
    }

    @Test
    fun reportsOneShareTypeConflictAcrossInjectAndExtrasAtSites() {
        val targetOwner = "com/example/target/SimpleTarget"
        val targetClassIndex = FakeClassIndex(
            classes = listOf(
                ClassIndexEntry("SimpleTarget", "com.example.target", targetOwner),
                ClassIndexEntry("LocalIntRef", "com.llamalad7.mixinextras.sugar.ref", "com/llamalad7/mixinextras/sugar/ref/LocalIntRef"),
                ClassIndexEntry("LocalFloatRef", "com.llamalad7.mixinextras.sugar.ref", "com/llamalad7/mixinextras/sugar/ref/LocalFloatRef"),
            ),
            methods = mapOf(
                targetOwner to listOf(MethodIndexEntry("draw", "(Ljava/lang/String;FF)V", false, "draw(String, float, float): void")),
            ),
        )
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @Inject(method = "draw(Ljava/lang/String;FF)V", at = @At("HEAD"))
                private void first(@Share("slot") LocalIntRef shared) {}

                @ModifyExpressionValue(
                    method = "draw(Ljava/lang/String;FF)V",
                    at = {
                        @At(value = "CONSTANT", args = "floatValue=0.0"),
                        @At(value = "CONSTANT", args = "floatValue=1.0"),
                    },
                )
                private float second(float original, @Share("slot") LocalFloatRef shared) { return original; }
            }
        """.trimIndent()
        val conflicts = analyze(
            source,
            service = MixinExtrasDiagnosticsService(targetClassIndex, FakeBytecodeIndex()),
        ).filter { it.code == MixinExtrasDiagnosticCodes.SHARE_TYPE_CONFLICT }
        assertEquals(1, conflicts.size)
        assertEquals(McSeverity.ERROR, conflicts.single().severity)
    }

    @Test
    fun noShareTypeConflictAcrossTopLevelMixinsWithOmittedNamespace() {
        val targetOwner = "com/example/target/SimpleTarget"
        val classIndex = FakeClassIndex(
            classes = MixinExtrasTestFixtures.classIndex.findClasses("", 100_000) + listOf(
                ClassIndexEntry("LocalIntRef", "com.llamalad7.mixinextras.sugar.ref", "com/llamalad7/mixinextras/sugar/ref/LocalIntRef"),
                ClassIndexEntry("LocalFloatRef", "com.llamalad7.mixinextras.sugar.ref", "com/llamalad7/mixinextras/sugar/ref/LocalFloatRef"),
            ),
            methods = mapOf(targetOwner to MixinExtrasTestFixtures.classIndex.getMethods(targetOwner)),
        )
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class FirstMixin {
                @Inject(method = "draw(Ljava/lang/String;FF)V", at = @At("HEAD"))
                private void first(@Share("slot") LocalIntRef shared) {}
            }
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class SecondMixin {
                @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
                private float second(float original, @Share("slot") LocalFloatRef shared) { return original; }
            }
        """.trimIndent()
        val conflicts = analyze(source, service = MixinExtrasDiagnosticsService(classIndex, FakeBytecodeIndex()))
            .filter { it.code == MixinExtrasDiagnosticCodes.SHARE_TYPE_CONFLICT }
        assertTrue(conflicts.isEmpty())
    }

    @Test
    fun ignoresShareFromNestedNonMixinClass() {
        val targetOwner = "com/example/target/SimpleTarget"
        val classIndex = FakeClassIndex(
            classes = MixinExtrasTestFixtures.classIndex.findClasses("", 100_000) + listOf(
                ClassIndexEntry("LocalIntRef", "com.llamalad7.mixinextras.sugar.ref", "com/llamalad7/mixinextras/sugar/ref/LocalIntRef"),
                ClassIndexEntry("LocalFloatRef", "com.llamalad7.mixinextras.sugar.ref", "com/llamalad7/mixinextras/sugar/ref/LocalFloatRef"),
            ),
            methods = mapOf(targetOwner to MixinExtrasTestFixtures.classIndex.getMethods(targetOwner)),
        )
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class OuterMixin {
                @Inject(method = "draw(Ljava/lang/String;FF)V", at = @At("HEAD"))
                private void outer(@Share("slot") LocalIntRef shared) {}

                class Helper {
                    @Inject(method = "draw(Ljava/lang/String;FF)V", at = @At("HEAD"))
                    private void helper(@Share("slot") LocalFloatRef shared) {}
                }
            }
        """.trimIndent()
        val conflicts = analyze(source, service = MixinExtrasDiagnosticsService(classIndex, FakeBytecodeIndex()))
            .filter { it.code == MixinExtrasDiagnosticCodes.SHARE_TYPE_CONFLICT }
        assertTrue(conflicts.isEmpty())
    }

    @Test
    fun reportsShareTypeConflictAcrossTopLevelMixinsWithExplicitNamespace() {
        val targetOwner = "com/example/target/SimpleTarget"
        val classIndex = FakeClassIndex(
            classes = MixinExtrasTestFixtures.classIndex.findClasses("", 100_000) + listOf(
                ClassIndexEntry("LocalIntRef", "com.llamalad7.mixinextras.sugar.ref", "com/llamalad7/mixinextras/sugar/ref/LocalIntRef"),
                ClassIndexEntry("LocalFloatRef", "com.llamalad7.mixinextras.sugar.ref", "com/llamalad7/mixinextras/sugar/ref/LocalFloatRef"),
            ),
            methods = mapOf(targetOwner to MixinExtrasTestFixtures.classIndex.getMethods(targetOwner)),
        )
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class FirstMixin {
                @Inject(method = "draw(Ljava/lang/String;FF)V", at = @At("HEAD"))
                private void first(@Share(value = "slot", namespace = "shared") LocalIntRef shared) {}
            }
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class SecondMixin {
                @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
                private float second(float original, @Share(value = "slot", namespace = "shared") LocalFloatRef shared) { return original; }
            }
        """.trimIndent()
        val conflicts = analyze(source, service = MixinExtrasDiagnosticsService(classIndex, FakeBytecodeIndex()))
            .filter { it.code == MixinExtrasDiagnosticCodes.SHARE_TYPE_CONFLICT }
        assertEquals(1, conflicts.size)
        assertEquals(McSeverity.ERROR, conflicts.single().severity)
    }

    @Test
    fun wrapWithConditionValidVoidInvokeProducesNoTargetDiagnostic() {
        val service = diagnosticsService(
            wrapWithConditionInvokeBytecodeIndex(
                wrapWithConditionInvokeCandidate(
                    operationKind = AtTargetOperationKind.INVOKE_VIRTUAL,
                    owner = "java/util/ArrayList",
                    name = "clear",
                    descriptor = "()V",
                    occurrenceResultClassification = OccurrenceResultClassification.VOID,
                ),
            ),
        )
        val diagnostics = analyze(
            wrapWithConditionSource(
                at = """@At(value = "INVOKE", target = "Ljava/util/ArrayList;clear()V")""",
                handler = """
                    private boolean mcdevWrapCondition(java.util.ArrayList instance) {
                        return true;
                    }
                """.trimIndent(),
            ),
            service = service,
        )
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRAP_WITH_CONDITION_INVALID_TARGET })
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRAP_WITH_CONDITION_POPPED_NON_VOID })
        assertTrue(diagnostics.none { it.severity == McSeverity.ERROR })
    }

    @Test
    fun wrapWithConditionValidPoppedNonVoidProducesWarning() {
        val service = diagnosticsService(
            wrapWithConditionInvokeBytecodeIndex(
                wrapWithConditionInvokeCandidate(
                    operationKind = AtTargetOperationKind.INVOKE_STATIC,
                    owner = "java/lang/Math",
                    name = "abs",
                    descriptor = "(I)I",
                    occurrenceResultClassification = OccurrenceResultClassification.IMMEDIATELY_POPPED,
                ),
            ),
        )
        val diagnostics = analyze(
            wrapWithConditionSource(
                at = """@At(value = "INVOKE", target = "Ljava/lang/Math;abs(I)I")""",
                handler = """
                    private boolean mcdevWrapCondition(int arg0) {
                        return true;
                    }
                """.trimIndent(),
            ),
            service = service,
        )
        val warning = diagnostics.single { it.code == MixinExtrasDiagnosticCodes.WRAP_WITH_CONDITION_POPPED_NON_VOID }
        assertEquals(McSeverity.WARNING, warning.severity)
        assertEquals("WrapWithCondition is targeting a non-void instruction", warning.message)
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRAP_WITH_CONDITION_INVALID_TARGET })
    }

    @Test
    fun wrapWithConditionValidPoppedNonVoidStillValidatesHandlerSignature() {
        val service = diagnosticsService(
            wrapWithConditionInvokeBytecodeIndex(
                wrapWithConditionInvokeCandidate(
                    operationKind = AtTargetOperationKind.INVOKE_STATIC,
                    owner = "java/lang/Math",
                    name = "abs",
                    descriptor = "(I)I",
                    occurrenceResultClassification = OccurrenceResultClassification.IMMEDIATELY_POPPED,
                ),
            ),
        )
        val diagnostics = analyze(
            wrapWithConditionSource(
                at = """@At(value = "INVOKE", target = "Ljava/lang/Math;abs(I)I")""",
                handler = """
                    private int mcdevWrapCondition(int arg0) {
                        return arg0;
                    }
                """.trimIndent(),
            ),
            service = service,
        )
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.WRAP_WITH_CONDITION_POPPED_NON_VOID })
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE })
    }

    @Test
    fun wrapWithConditionRetainedNonVoidProducesInvalidTargetError() {
        val service = diagnosticsService(
            wrapWithConditionInvokeBytecodeIndex(
                wrapWithConditionInvokeCandidate(
                    operationKind = AtTargetOperationKind.INVOKE_VIRTUAL,
                    occurrenceResultClassification = OccurrenceResultClassification.RETAINED,
                ),
            ),
        )
        val diagnostics = analyze(
            wrapWithConditionSource(
                handler = """
                    private boolean mcdevWrapCondition(String instance) {
                        return true;
                    }
                """.trimIndent(),
            ),
            service = service,
        )
        val error = diagnostics.single { it.code == MixinExtrasDiagnosticCodes.WRAP_WITH_CONDITION_INVALID_TARGET }
        assertEquals(McSeverity.ERROR, error.severity)
        assertEquals("WrapWithCondition cannot target a retained non-void invocation", error.message)
    }

    @Test
    fun wrapWithConditionRetainedInvalidSuppressesMisleadingSignatureOnlySuccess() {
        val service = diagnosticsService(
            wrapWithConditionInvokeBytecodeIndex(
                wrapWithConditionInvokeCandidate(
                    operationKind = AtTargetOperationKind.INVOKE_VIRTUAL,
                    occurrenceResultClassification = OccurrenceResultClassification.RETAINED,
                ),
            ),
        )
        val diagnostics = analyze(
            wrapWithConditionSource(
                handler = """
                    private boolean mcdevWrapCondition(String instance) {
                        return true;
                    }
                """.trimIndent(),
            ),
            service = service,
        )
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.WRAP_WITH_CONDITION_INVALID_TARGET })
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE })
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun wrapWithConditionInvalidInstructionFieldGetProducesError() {
        val service = diagnosticsService(
            wrapWithConditionFieldBytecodeIndex(
                wrapWithConditionFieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE),
            ),
        )
        val diagnostics = analyze(
            wrapWithConditionSource(
                at = """@At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;")""",
                handler = """
                    private boolean mcdevWrapCondition(com.example.target.SimpleTarget instance) {
                        return true;
                    }
                """.trimIndent(),
            ),
            service = service,
        )
        val error = diagnostics.single { it.code == MixinExtrasDiagnosticCodes.WRAP_WITH_CONDITION_INVALID_TARGET }
        assertEquals(McSeverity.ERROR, error.severity)
        assertEquals(
            "WrapWithCondition only supports void method invocations, immediately popped invocations, and field writes",
            error.message,
        )
    }

    @Test
    fun wrapWithConditionUnsupportedAtValueProducesInvalidInstructionError() {
        val diagnostics = analyze(
            wrapWithConditionSource(
                at = """@At(value = "CONSTANT", args = "intValue=0")""",
                handler = """
                    private boolean mcdevWrapCondition() {
                        return true;
                    }
                """.trimIndent(),
            ),
            service = diagnosticsService(FakeBytecodeIndex()),
        )
        val error = diagnostics.single { it.code == MixinExtrasDiagnosticCodes.WRAP_WITH_CONDITION_INVALID_TARGET }
        assertEquals(McSeverity.ERROR, error.severity)
        assertEquals(
            "WrapWithCondition only supports void method invocations, immediately popped invocations, and field writes",
            error.message,
        )
    }

    @Test
    fun wrapWithConditionMissingBytecodeProducesNoInvalidTargetDiagnostic() {
        val service = MixinExtrasDiagnosticsService(MixinExtrasTestFixtures.classIndex, FakeBytecodeIndex())
        val diagnostics = analyze(
            wrapWithConditionSource(
                handler = """
                    private boolean mcdevWrapCondition(String instance) {
                        return true;
                    }
                """.trimIndent(),
            ),
            service = service,
        )
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRAP_WITH_CONDITION_INVALID_TARGET })
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRAP_WITH_CONDITION_POPPED_NON_VOID })
    }

    @Test
    fun wrapWithConditionTargetDiagnosticUsesAnnotationRangeAndMetadata() {
        val service = diagnosticsService(
            wrapWithConditionInvokeBytecodeIndex(
                wrapWithConditionInvokeCandidate(
                    operationKind = AtTargetOperationKind.INVOKE_VIRTUAL,
                    occurrenceResultClassification = OccurrenceResultClassification.RETAINED,
                ),
            ),
        )
        val diagnostics = analyze(
            wrapWithConditionSource(
                handler = """
                    private boolean mcdevWrapCondition(String instance) {
                        return true;
                    }
                """.trimIndent(),
            ),
            service = service,
        )
        val diagnostic = diagnostics.single { it.code == MixinExtrasDiagnosticCodes.WRAP_WITH_CONDITION_INVALID_TARGET }
        val site = HandlerSignatureService.findAnnotationSites(
            wrapWithConditionSource(
                handler = """
                    private boolean mcdevWrapCondition(String instance) {
                        return true;
                    }
                """.trimIndent(),
            ),
        ).single()
        assertEquals(site.annotationRange, diagnostic.range)
        assertEquals("WrapWithCondition", diagnostic.metadata["annotation"])
        assertEquals("draw(Ljava/lang/String;FF)V", diagnostic.metadata["method"])
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
    fun selectorIgnoresHandlerRangesThatOnlyTouchAtEndpoint() {
        val annotationRange = McTextRange(McTextPosition(0, 0), McTextPosition(0, 1))
        val handlerRange = McTextRange(McTextPosition(3, 4), McTextPosition(3, 40))
        val site = testSite(annotationRange = annotationRange, handlerRange = handlerRange)
        val touchesStart = resolvedContext(
            handlerRange = McTextRange(McTextPosition(3, 0), McTextPosition(3, 4)),
        )
        val touchesEnd = resolvedContext(
            handlerRange = McTextRange(McTextPosition(3, 40), McTextPosition(3, 50)),
        )
        assertNull(selectResolvedMixinExtrasContext(site, listOf(touchesStart)))
        assertNull(selectResolvedMixinExtrasContext(site, listOf(touchesEnd)))
        assertNull(selectResolvedMixinExtrasContext(site, listOf(touchesStart, touchesEnd)))
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

    @Test
    fun authoritativeEmptyResolvedContextReportsMissingExpressionDespiteHandwrittenAnnotation() {
        val body = expressionHandlerSource(
            prefix = """@Expression("text.length()")""",
            suffix = "private int mcdevHandler(int original) { return original; }",
        )
        val source = wrappedMixinSource(body)
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val diagnostics = analyze(
            source,
            resolvedContexts = listOf(
                resolvedFor(site, emptyExpressionContext()),
            ),
        )
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.MISSING_EXPRESSION_ANNOTATION })
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.UNSUPPORTED_EXPRESSION_CONTEXT })
    }

    @Test
    fun selectedResolvedExpressionPreventsMissingExpressionButReportsUnsupportedWithoutBytecode() {
        val body = expressionHandlerSource(
            suffix = "private int mcdevHandler(int original) { return original; }",
        )
        val source = wrappedMixinSource(body)
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val diagnostics = analyze(
            source,
            resolvedContexts = listOf(
                resolvedFor(
                    site,
                    expressionContext(MixinExtrasExpression(values = listOf("text.length()"))),
                ),
            ),
        )
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.MISSING_EXPRESSION_ANNOTATION })
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.UNSUPPORTED_EXPRESSION_CONTEXT })
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE })
    }

    @Test
    fun selectedResolvedExpressionPreventsMissingExpressionWithoutWrongReturnTypeWithoutBytecode() {
        val body = expressionHandlerSource(
            prefix = """@Expression("this.label")""",
            suffix = "private float mcdevHandler(float original) { return original; }",
        )
        val source = wrappedMixinSource(body)
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val diagnostics = analyze(
            source,
            resolvedContexts = listOf(
                resolvedFor(
                    site,
                    expressionContext(MixinExtrasExpression(values = listOf("text.length()"))),
                ),
            ),
        )
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.MISSING_EXPRESSION_ANNOTATION })
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.UNSUPPORTED_EXPRESSION_CONTEXT })
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE })
    }

    @Test
    fun noWrongOperationCallDiagnosticForValidWrapOperationCall() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            ${MixinExtrasTestFixtures.WRAP_OPERATION_SOURCE.trimIndent()}
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS })
    }

    @Test
    fun noWrongOperationCallDiagnosticForValidWrapMethodCall() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            ${MixinExtrasTestFixtures.WRAP_METHOD_SOURCE.trimIndent()}
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS })
    }

    @Test
    fun reportsWrongOperationCallArityForEmptyArguments() {
        val source = wrapOperationHandlerSource(
            handler = """
                private int mcdevWrapLength(String instance, Operation<Integer> original) {
                    return original.call();
                }
            """.trimIndent(),
        )
        val diagnostic = analyze(source).single { it.code == MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS }
        assertEquals(McSeverity.ERROR, diagnostic.severity)
        assertEquals("Operation.call called with the wrong number of arguments", diagnostic.message)
        assertEquals("instance", diagnostic.metadata["expectedArguments"])
        assertOffsetRange(source, diagnostic.range, "original.call(", ")")
    }

    @Test
    fun reportsWrongOperationCallArityForExtraArguments() {
        val source = wrapOperationHandlerSource(
            handler = """
                private int mcdevWrapLength(String instance, Operation<Integer> original) {
                    return original.call(instance, instance);
                }
            """.trimIndent(),
        )
        val diagnostic = analyze(source).single { it.code == MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS }
        assertEquals("instance", diagnostic.metadata["expectedArguments"])
        assertOffsetRange(source, diagnostic.range, "original.call(", ")")
    }

    @Test
    fun noWrongOperationCallDiagnosticForWrapMethodReceiverExtraParameter() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @WrapMethod(method = "draw(Ljava/lang/String;FF)V")
                private void mcdevWrapDraw(com.example.target.SimpleTarget instance, String arg0, float arg1, float arg2, Operation<Void> original) {
                    original.call(instance, arg0, arg1, arg2);
                }
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS })
    }

    @Test
    fun ignoresQualifiedOperationCallsAndCommentStringFakeCalls() {
        val source = wrapOperationHandlerSource(
            handler = """
                private int mcdevWrapLength(String instance, Operation<Integer> original) {
                    this.original.call();
                    other.original.call(instance, instance);
                    // original.call()
                    String fake = "original.call(instance, instance)";
                    return original.call(instance);
                }
            """.trimIndent(),
        )
        val diagnostics = analyze(source)
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS })
    }

    @Test
    fun reportsEachWrongOperationCallIndependently() {
        val source = wrapOperationHandlerSource(
            handler = """
                private int mcdevWrapLength(String instance, Operation<Integer> original) {
                    original.call();
                    return original.call(instance, instance);
                }
            """.trimIndent(),
        )
        val diagnostics = analyze(source).filter {
            it.code == MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS
        }
        assertEquals(2, diagnostics.size)
        assertEquals("instance", diagnostics[0].metadata["expectedArguments"])
        assertEquals("instance", diagnostics[1].metadata["expectedArguments"])

        val firstOpen = source.indexOf("original.call(") + "original.call(".length
        val firstClose = source.indexOf(')', firstOpen)
        assertEquals(firstOpen, rangeStart(source, diagnostics[0].range))
        assertEquals(firstOpen, rangeEnd(source, diagnostics[0].range))

        val secondOpen = source.indexOf("original.call(", firstClose) + "original.call(".length
        val secondClose = source.indexOf(')', secondOpen)
        assertEquals(secondOpen, rangeStart(source, diagnostics[1].range))
        assertEquals(secondClose, rangeEnd(source, diagnostics[1].range))
    }

    @Test
    fun wrongOperationCallDiagnosticUsesArgumentRangeAndMetadata() {
        val source = wrapOperationHandlerSource(
            handler = """
                private int mcdevWrapLength(String instance, Operation<Integer> original) {
                    return original.call();
                }
            """.trimIndent(),
        )
        val diagnostic = analyze(source).single { it.code == MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS }
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        assertEquals("WrapOperation", diagnostic.metadata["annotation"])
        assertEquals("draw(Ljava/lang/String;FF)V", diagnostic.metadata["method"])
        assertEquals("instance", diagnostic.metadata["expectedArguments"])
        assertOffsetRange(source, diagnostic.range, "original.call(", ")")
        assertTrue(diagnostic.range != site.annotationRange)
    }

    @Test
    fun wrongOperationCallDiagnosticUsesHandlerParameterNamesNotSpecPlaceholders() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;charAt(I)C"))
                private char mcdevWrapChar(String receiver, int value, Operation<Character> original) {
                    return original.call();
                }
            }
        """.trimIndent()
        val diagnostic = analyze(source).single { it.code == MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS }
        assertEquals("receiver, value", diagnostic.metadata["expectedArguments"])
    }

    @Test
    fun noWrongOperationCallDiagnosticWhenHandlerHasNoOperationParameter() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            ${MixinExtrasTestFixtures.WRAP_OPERATION_MISSING_OP.trimIndent()}
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS })
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.MISSING_OPERATION_PARAMETER })
    }

    @Test
    fun noWrongOperationCallDiagnosticWhenRequiredParameterMissingBeforeOperation() {
        val source = wrapOperationHandlerSource(
            handler = """
                private int mcdevWrapLength(Operation<Integer> original) {
                    return original.call();
                }
            """.trimIndent(),
        )
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.MISSING_OPERATION_PARAMETER })
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS })
    }

    @Test
    fun noWrongOperationCallDiagnosticWhenExtraParameterBeforeOperation() {
        val source = wrapOperationHandlerSource(
            handler = """
                private int mcdevWrapLength(String instance, float extra, Operation<Integer> original) {
                    return original.call();
                }
            """.trimIndent(),
        )
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.MISSING_OPERATION_PARAMETER })
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS })
    }

    @Test
    fun noWrongOperationCallDiagnosticWhenHandlerDeclaresMultipleOperationParameters() {
        val source = wrapOperationHandlerSource(
            handler = """
                private int mcdevWrapLength(String instance, Operation<Integer> original, Operation<Integer> duplicate) {
                    return original.call();
                }
            """.trimIndent(),
        )
        val diagnostics = analyze(source)
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS })
    }

    @Test
    fun noWrongOperationCallDiagnosticWhenOperationParameterIsMisplaced() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            ${MixinExtrasTestFixtures.WRAP_OPERATION_OP_NOT_LAST.trimIndent()}
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.MISSING_OPERATION_PARAMETER })
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS })
    }

    @Test
    fun noWrongOperationCallDiagnosticWhenExpectedSignatureIsUnresolved() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @WrapOperation(method = "unknown()V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
                private void mcdevWrap(String instance, Operation<Void> original) {
                    original.call();
                }
            }
        """.trimIndent()
        val diagnostics = analyze(source)
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS })
    }

    @Test
    fun reportsWrongOperationCallDespiteWrongReturnTypeWhenLayoutIsValid() {
        val source = wrapOperationHandlerSource(
            handler = """
                private void mcdevWrapLength(String instance, Operation<Integer> original) {
                    original.call();
                }
            """.trimIndent(),
        )
        val diagnostics = analyze(source)
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE })
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS })
    }

    @Test
    fun emptyResolvedContextsPreservesHandwrittenExpressionFallback() {
        val body = expressionHandlerSource(
            prefix = """@Expression("text.length()")""",
            suffix = "private int mcdevHandler(int original) { return original; }",
        )
        val diagnostics = analyze(wrappedMixinSource(body))
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.MISSING_EXPRESSION_ANNOTATION })
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.UNSUPPORTED_EXPRESSION_CONTEXT })
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE })
    }

    private fun analyze(
        source: String,
        resolvedContexts: List<ResolvedMixinExtrasContext> = emptyList(),
        service: MixinExtrasDiagnosticsService = this.service,
    ) = service.analyze(
        MixinExtrasDiagnosticRequest(
            source = source,
            documentUri = "file:///Example.java",
            resolvedContexts = resolvedContexts,
        ),
    )

    private fun diagnosticsService(bytecodeIndex: BytecodeIndex) =
        MixinExtrasDiagnosticsService(MixinExtrasTestFixtures.classIndex, bytecodeIndex)

    private fun wrapWithConditionInvokeBytecodeIndex(vararg candidates: AtTargetCandidate): BytecodeIndex =
        FakeBytecodeIndex(
            candidates = mapOf(
                "com/example/target/SimpleTarget#draw#INVOKE" to candidates.toList(),
            ),
        )

    private fun wrapWithConditionFieldBytecodeIndex(vararg candidates: AtTargetCandidate): BytecodeIndex =
        FakeBytecodeIndex(
            candidates = mapOf(
                "com/example/target/SimpleTarget#draw#FIELD" to candidates.toList(),
            ),
        )

    private fun wrapWithConditionInvokeCandidate(
        operationKind: AtTargetOperationKind,
        owner: String = "java/lang/String",
        name: String = "length",
        descriptor: String = "()I",
        instructionOccurrenceIndex: Int = 0,
        occurrenceResultClassification: OccurrenceResultClassification = OccurrenceResultClassification.IMMEDIATELY_POPPED,
    ): AtTargetCandidate = AtTargetCandidate(
        owner = owner,
        name = name,
        descriptor = descriptor,
        displayLabel = "$name(): type",
        detail = owner.substringAfterLast('/'),
        kind = AtTargetKind.INVOKE,
        operationKind = operationKind,
        instructionOccurrenceIndex = instructionOccurrenceIndex,
        occurrenceResultClassification = occurrenceResultClassification,
    )

    private fun wrapWithConditionFieldCandidate(
        operationKind: AtTargetOperationKind,
        name: String = "label",
        descriptor: String = "Ljava/lang/String;",
        owner: String = "com/example/target/SimpleTarget",
        instructionOccurrenceIndex: Int = 0,
    ): AtTargetCandidate = AtTargetCandidate(
        owner = owner,
        name = name,
        descriptor = descriptor,
        displayLabel = "$name: type",
        detail = "SimpleTarget",
        kind = AtTargetKind.FIELD,
        operationKind = operationKind,
        instructionOccurrenceIndex = instructionOccurrenceIndex,
    )

    private fun wrapOperationHandlerSource(handler: String): String = """
        @Mixin(com.example.target.SimpleTarget.class)
        abstract class ExampleMixin {
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            $handler
        }
    """.trimIndent()

    private fun wrapWithConditionSource(
        at: String = """@At(value = "INVOKE", target = "Ljava/lang/String;length()I")""",
        handler: String,
    ): String = """
        @Mixin(com.example.target.SimpleTarget.class)
        abstract class ExampleMixin {
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = $at)
            $handler
        }
    """.trimIndent()

    private fun wrappedMixinSource(body: String): String = """
        @Mixin(com.example.target.SimpleTarget.class)
        abstract class ExampleMixin {
        ${body.trimIndent()}
        }
    """.trimIndent()

    private fun expressionHandlerSource(
        prefix: String = "",
        at: String = """@At(value = "MIXINEXTRAS:EXPRESSION")""",
        suffix: String,
    ): String = """
        $prefix
        @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = $at)
        $suffix
    """.trimIndent()

    private fun resolvedFor(
        site: MixinExtrasAnnotationSite,
        context: ExpressionContext,
    ): ResolvedMixinExtrasContext = ResolvedMixinExtrasContext(
        handlerRange = site.handlerMethod?.range ?: site.annotationRange,
        context = context,
    )

    private fun resolvedContext(
        handlerRange: McTextRange,
        context: ExpressionContext = emptyExpressionContext(),
    ): ResolvedMixinExtrasContext = ResolvedMixinExtrasContext(handlerRange = handlerRange, context = context)

    private fun emptyExpressionContext() = ExpressionContext(
        expressionIndex = MixinExtrasExpressionIndex(),
        definitionIndex = MixinExtrasDefinitionIndex(),
    )

    private fun expressionContext(vararg expressions: MixinExtrasExpression) = ExpressionContext(
        expressionIndex = MixinExtrasExpressionIndex(expressions.toList()),
        definitionIndex = MixinExtrasDefinitionIndex(),
    )

    private fun expressionMatchSamplesClassIndex(): FakeClassIndex {
        val owner = BytecodeFixtureCompiler.internalName("ExpressionMatchSamples")
        return FakeClassIndex(
            classes = listOf(
                ClassIndexEntry(
                    "ExpressionMatchSamples",
                    "io.github.mcdev.core.bytecode.fixtures",
                    owner,
                ),
            ),
            methods = mapOf(
                owner to listOf(
                    MethodIndexEntry("arrayStore", "([III)V", false, "arrayStore(int[], int, int): void"),
                    MethodIndexEntry("writeSampleField", "(I)V", false, "writeSampleField(int): void"),
                ),
            ),
        )
    }

    private fun expressionMatchSamplesBytecodeIndex(): BytecodeIndex {
        val owner = BytecodeFixtureCompiler.internalName("ExpressionMatchSamples")
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
        }
    }

    private fun assertOffsetRange(source: String, range: McTextRange, callPrefix: String, closeParen: String) {
        val openIndex = source.indexOf(callPrefix)
        assertTrue(openIndex >= 0, "Expected call prefix not found: $callPrefix")
        val contentStart = openIndex + callPrefix.length
        val closeIndex = source.indexOf(closeParen, contentStart)
        assertTrue(closeIndex >= 0, "Expected closing paren not found")
        val expectedStart = contentStart
        val expectedEnd = if (closeIndex == contentStart) contentStart else {
            var start = contentStart
            var end = closeIndex
            while (start < end && source[start].isWhitespace()) start++
            while (end > start && source[end - 1].isWhitespace()) end--
            if (start >= end) contentStart else end
        }
        assertEquals(expectedStart, rangeStart(source, range))
        assertEquals(expectedEnd, rangeEnd(source, range))
    }

    private fun rangeStart(source: String, range: McTextRange): Int {
        val lineStart = source.lineSequence().take(range.start.line).sumOf { it.length + 1 }
        return lineStart + range.start.character
    }

    private fun rangeEnd(source: String, range: McTextRange): Int {
        val lineStart = source.lineSequence().take(range.end.line).sumOf { it.length + 1 }
        return lineStart + range.end.character
    }

    private fun testSite(
        annotationRange: McTextRange,
        handlerRange: McTextRange? = null,
    ): MixinExtrasAnnotationSite = MixinExtrasAnnotationSite(
        annotation = MixinExtrasAnnotation.MODIFY_EXPRESSION_VALUE,
        methodAttribute = "draw(Ljava/lang/String;FF)V",
        atValue = "MIXINEXTRAS:EXPRESSION",
        atTarget = null,
        annotationRange = annotationRange,
        handlerMethod = handlerRange?.let { range ->
            HandlerMethodDeclaration(
                methodName = "mcdevHandler",
                returnTypeName = "int",
                returnTypeDescriptor = "I",
                parameters = emptyList(),
                range = range,
            )
        },
    )
}
