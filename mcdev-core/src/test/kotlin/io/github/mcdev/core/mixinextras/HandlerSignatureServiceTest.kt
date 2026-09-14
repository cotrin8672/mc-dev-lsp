package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.bytecode.BytecodeFixtureCompiler
import io.github.mcdev.core.bytecode.OccurrenceResultClassification
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.AtTargetKind
import io.github.mcdev.core.mixin.AtTargetOperationKind
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.FakeBytecodeIndex
import io.github.mcdev.core.mixin.FakeClassIndex
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FieldIndexEntry
import io.github.mcdev.core.mixin.MethodIndexEntry
import kotlin.test.Test
import org.objectweb.asm.Opcodes
import org.objectweb.asm.Type
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class HandlerSignatureServiceTest {
    private val classIndex = MixinExtrasTestFixtures.classIndex
    private val service = HandlerSignatureService(classIndex)

    private companion object {
        const val CALLBACK_INFO_DESCRIPTOR =
            "Lorg/spongepowered/asm/mixin/injection/callback/CallbackInfo;"
        const val CALLBACK_INFO_RETURNABLE_DESCRIPTOR =
            "Lorg/spongepowered/asm/mixin/injection/callback/CallbackInfoReturnable;"
    }

    @Test
    fun modifyExpressionValueExpectsOriginalFloatParameter() {
        val source = trimmedSource(MixinExtrasTestFixtures.MODIFY_EXPRESSION_SOURCE)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals("F", spec.returnTypeDescriptor)
        assertEquals(1, spec.parameters.size)
        assertEquals("F", spec.parameters.first().typeDescriptor)
        assertEquals("original", spec.parameters.first().name)
    }

    @Test
    fun modifyExpressionValueAcceptsOneConsistentIntLikeHandlerType() {
        val expressionService = HandlerSignatureService(
            expressionMatchSamplesClassIndex(),
            expressionMatchSamplesBytecodeIndex(),
        )
        val source = trimmedSource("""
            @Mixin(ExpressionMatchSamples.class)
            abstract class ExampleMixin {
                @Expression(id = "main", value = "@(true)")
                @ModifyExpressionValue(
                    method = "intEqualsZero(I)Z",
                    at = @At(id = "main", value = "MIXINEXTRAS:EXPRESSION"),
                )
                private boolean mcdevHandler(boolean original) { return !original; }
            }
        """)
        val site = sites(source).single()
        val spec = expressionService.expectedSignature(
            source,
            site,
            listOf(expressionMatchSamplesOwner()),
        )
        assertNotNull(spec)
        assertTrue(spec.acceptedReturnTypeDescriptors.contains("Z"))
        assertTrue(spec.parameters.single().acceptedTypeDescriptors.contains("Z"))
        val handler = HandlerSignatureService.enrichHandlerTypes(site.handlerMethod!!, expressionMatchSamplesClassIndex())
        assertTrue(
            expressionService.validateHandler(
                source,
                site,
                listOf(expressionMatchSamplesOwner()),
                handler,
            ).isEmpty(),
        )
    }

    @Test
    fun modifyExpressionValueRejectsMixedIntLikeHandlerTypes() {
        val expressionService = HandlerSignatureService(
            expressionMatchSamplesClassIndex(),
            expressionMatchSamplesBytecodeIndex(),
        )
        val source = trimmedSource("""
            @Mixin(ExpressionMatchSamples.class)
            abstract class ExampleMixin {
                @Expression(id = "main", value = "@(true)")
                @ModifyExpressionValue(
                    method = "intEqualsZero(I)Z",
                    at = @At(id = "main", value = "MIXINEXTRAS:EXPRESSION"),
                )
                private int mcdevHandler(boolean original) { return original ? 1 : 0; }
            }
        """)
        val site = sites(source).single()
        val handler = HandlerSignatureService.enrichHandlerTypes(site.handlerMethod!!, expressionMatchSamplesClassIndex())
        val issues = expressionService.validateHandler(
            source,
            site,
            listOf(expressionMatchSamplesOwner()),
            handler,
        )
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun modifyExpressionValueKeepsExactIntArithmeticStrict() {
        val expressionService = HandlerSignatureService(
            expressionMatchSamplesClassIndex(),
            expressionMatchSamplesBytecodeIndex(),
        )
        val source = trimmedSource("""
            @Mixin(ExpressionMatchSamples.class)
            abstract class ExampleMixin {
                @Expression(id = "main", value = "@(?+?)")
                @ModifyExpressionValue(
                    method = "add(II)I",
                    at = @At(id = "main", value = "MIXINEXTRAS:EXPRESSION"),
                )
                private boolean mcdevHandler(boolean original) { return !original; }
            }
        """)
        val site = sites(source).single()
        val spec = expressionService.expectedSignature(
            source,
            site,
            listOf(expressionMatchSamplesOwner()),
        )
        assertNotNull(spec)
        assertEquals("I", spec.returnTypeDescriptor)
        assertTrue(spec.acceptedReturnTypeDescriptors.isEmpty())
        assertTrue(spec.parameters.single().acceptedTypeDescriptors.isEmpty())

        val handler = HandlerSignatureService.enrichHandlerTypes(
            site.handlerMethod!!,
            expressionMatchSamplesClassIndex(),
        )
        val issues = expressionService.validateHandler(
            source,
            site,
            listOf(expressionMatchSamplesOwner()),
            handler,
        )
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE })
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.WRONG_ORIGINAL_VALUE_TYPE })
    }

    @Test
    fun specializesIntLikeSignatureFromExistingBooleanHandler() {
        val acceptedIntLike = setOf("I", "Z", "B", "C", "S")
        val expected = HandlerSignatureSpec(
            returnTypeDescriptor = "I",
            readableReturnType = "int",
            parameters = listOf(
                HandlerParameterSpec(
                    name = "value",
                    typeDescriptor = "I",
                    readableType = "int",
                    acceptedTypeDescriptors = acceptedIntLike,
                ),
                HandlerParameterSpec(
                    name = "original",
                    typeDescriptor = "Lcom/llamalad7/mixinextras/injector/wrapoperation/Operation;",
                    readableType = "Operation<Integer>",
                    isOperation = true,
                    operationGenericDescriptor = "I",
                    acceptedOperationGenericDescriptors = acceptedIntLike,
                ),
            ),
            acceptedReturnTypeDescriptors = acceptedIntLike,
        )
        val handler = HandlerSignatureService.enrichHandlerTypes(
            HandlerSignatureService.parseHandlerMethod(
                "boolean handler(boolean value, Operation<Integer> original) { return value; }",
                0,
            )!!,
            classIndex,
        )

        val specialized = service.specializeIntLikeSignature(expected, handler)
        assertEquals("Z", specialized.returnTypeDescriptor)
        assertEquals("boolean", specialized.readableReturnType)
        assertEquals("Z", specialized.parameters[0].typeDescriptor)
        assertEquals("boolean", specialized.parameters[0].readableType)
        assertEquals("Z", specialized.parameters[1].operationGenericDescriptor)
        assertEquals("Operation<Boolean>", specialized.parameters[1].readableType)
    }

    @Test
    fun modifyExpressionValueExposesOptionalCapturedTargetParameters() {
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
        """)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals(1, spec.parameters.size)
        assertEquals("original", spec.parameters.single().name)
        assertEquals("F", spec.parameters.single().typeDescriptor)
        assertEquals(3, spec.optionalCapturedTargetParameters.size)
        assertEquals("Ljava/lang/String;", spec.optionalCapturedTargetParameters[0].typeDescriptor)
        assertEquals("F", spec.optionalCapturedTargetParameters[1].typeDescriptor)
        assertEquals("F", spec.optionalCapturedTargetParameters[2].typeDescriptor)
    }

    @Test
    fun generatesModifyExpressionValueHandlerStubStaysMinimalWithTargetParameters() {
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
        """)
        val site = sites(source).first()
        val stub = service.generateHandlerStub(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(stub)
        assertTrue(stub.contains("float mcdevHandler(float original)"))
        assertTrue(!stub.contains("arg0"))
    }

    @Test
    fun validatesModifyExpressionValueHandlerWithPartialCapturedTargetParameterPrefix() {
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
            private float mcdevHandler(float original, String arg0, float arg1) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun rejectsModifyExpressionValueHandlerWithSkippedCapturedTargetParameter() {
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
            private float mcdevHandler(float original, float arg1) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun rejectsModifyExpressionValueHandlerWithWrongCapturedTargetParameterType() {
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
            private float mcdevHandler(float original, int arg0) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun findAnnotationSitesResolvesInjectorAnnotationsByExactIdentity() {
        val acceptedSources = listOf(
            """
                @com.llamalad7.mixinextras.injector.ModifyExpressionValue(
                    method = "compute()I",
                    at = @At("RETURN"),
                )
            """,
            """
                import com.llamalad7.mixinextras.injector.ModifyExpressionValue;

                @ModifyExpressionValue(method = "compute()I", at = @At("RETURN"))
            """,
            """
                @ModifyExpressionValue(method = "compute()I", at = @At("RETURN"))
            """,
        )
        acceptedSources.forEach { source ->
            assertEquals(1, sites(trimmedSource(source)).size)
        }

        val rejectedSources = listOf(
            """
                @com.example.ModifyExpressionValue(method = "compute()I", at = @At("RETURN"))
            """,
            """
                import com.example.ModifyExpressionValue;

                @ModifyExpressionValue(method = "compute()I", at = @At("RETURN"))
            """,
        )
        rejectedSources.forEach { source ->
            assertTrue(sites(trimmedSource(source)).isEmpty())
        }
    }

    @Test
    fun findAnnotationSitesSkipsCommentsAndNormalMethodModifiersBeforeHandler() {
        val source = trimmedSource("""
            @WrapMethod(method = "draw(Ljava/lang/String;FF)V")
            // keep this comment attached to the handler
            /* and this one too */
            private /* between modifiers */ final synchronized void mcdevWrapDraw(
                String arg0,
                float arg1,
                float arg2,
                Operation<Void> original
            ) {
                original.call(arg0, arg1, arg2);
            }
        """)
        val handler = sites(source).single().handlerMethod
        assertNotNull(handler)
        assertFalse(handler.isStatic)
        assertTrue(sourceSubstring(source, handler.range).contains("private /* between modifiers */ final synchronized void"))
    }

    @Test
    fun findAnnotationSitesIgnoresInjectorAnnotationsInCommentsStringsAndChars() {
        val source = trimmedSource("""
            // @ModifyExpressionValue(method = "fake()I", at = @At("RETURN"))
            String fakeString = "@ModifyExpressionValue(method = \"fake()I\", at = @At(\"RETURN\"))";
            char fakeChar = '@ModifyExpressionValue(method = "fake()I", at = @At("RETURN"))';
            @ModifyExpressionValue(method = "compute()I", at = @At("RETURN"))
        """)

        assertEquals("compute()I", sites(source).single().methodAttribute)
    }

    @Test
    fun findAnnotationSitesResolvesNestedAtByExactIdentity() {
        val acceptedSources = listOf(
            """
                @ModifyExpressionValue(
                    method = "compute()I",
                    at = @org.spongepowered.asm.mixin.injection.At("RETURN"),
                )
            """,
            """
                import org.spongepowered.asm.mixin.injection.At;

                @ModifyExpressionValue(method = "compute()I", at = @At("RETURN"))
            """,
            """
                @ModifyExpressionValue(method = "compute()I", at = @At("RETURN"))
            """,
        )
        acceptedSources.forEach { source ->
            assertEquals("RETURN", sites(trimmedSource(source)).single().atValue)
        }

        val rejectedSources = listOf(
            """
                @ModifyExpressionValue(
                    method = "compute()I",
                    at = @com.example.At("RETURN"),
                )
            """,
            """
                import com.example.At;

                @ModifyExpressionValue(method = "compute()I", at = @At("RETURN"))
            """,
        )
        rejectedSources.forEach { source ->
            assertNull(sites(trimmedSource(source)).single().atValue)
        }
    }

    @Test
    fun findAnnotationSitesExpandsAtArrayInSourceOrder() {
        val source = trimmedSource("""
            @ModifyExpressionValue(
                method = "compute()I",
                at = {
                    @At(value = "INVOKE", target = "Lexample/First;run()I", id = "first", ordinal = 0),
                    @At(value = "FIELD", target = "Lexample/Second;value:I", id = "second", ordinal = 2),
                }
            )
        """)

        val parsedSites = sites(source)
        assertEquals(2, parsedSites.size)
        assertEquals(listOf("INVOKE", "FIELD"), parsedSites.map { it.atValue })
        assertEquals(
            listOf("Lexample/First;run()I", "Lexample/Second;value:I"),
            parsedSites.map { it.atTarget },
        )
        assertEquals(listOf("first", "second"), parsedSites.map { it.atId })
        assertEquals(listOf(0, 2), parsedSites.map { it.atOrdinal })
    }

    @Test
    fun findAnnotationSitesIgnoresNonCodeAtTextInsideAtArray() {
        val source = trimmedSource("""
            @ModifyExpressionValue(
                method = "compute()I",
                at = {
                    // @At(value = "COMMENT", id = "comment", ordinal = 3)
                    "fake @At(value = \"STRING\", id = \"string\", ordinal = 4)",
                    '@At(value = "CHAR", id = "char", ordinal = 5)',
                    @At(value = "RETURN", id = "real", ordinal = 1),
                }
            )
        """)

        val parsedSites = sites(source)
        assertEquals(1, parsedSites.size)
        assertEquals("RETURN", parsedSites.single().atValue)
        assertEquals("real", parsedSites.single().atId)
        assertEquals(1, parsedSites.single().atOrdinal)
    }

    @Test
    fun findAnnotationSitesUsesTopLevelAtWhenSlicePrecedesIt() {
        val source = trimmedSource("""
            @ModifyExpressionValue(
                method = "compute()I",
                slice = @Slice(from = @At("HEAD")),
                at = @At("RETURN"),
            )
        """)

        assertEquals("RETURN", sites(source).single().atValue)
    }

    @Test
    fun findAnnotationSitesIgnoresStringAndCommentFakeAtBeforeRealAt() {
        val source = trimmedSource("""
            @ModifyExpressionValue(
                method = "compute()I",
                slice = @Slice(id = "fake @At(\"HEAD\")") /* fake @At("TAIL") */,
                // fake @At("BEFORE")
                at = /* fake @At("SHIFT") */ @At("RETURN"),
            )
        """)

        assertEquals("RETURN", sites(source).single().atValue)
    }

    @Test
    fun findAnnotationSitesExtractsConstantAtArgsFromSingleString() {
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
        """)
        val site = sites(source).first()
        assertEquals("CONSTANT", site.atValue)
        assertNull(site.atTarget)
        assertEquals(listOf("floatValue=0.0"), site.atArgs)
    }

    @Test
    fun findAnnotationSitesExtractsConstantAtArgsFromArrayInLexicalOrder() {
        val source = trimmedSource("""
            @ModifyExpressionValue(
                method = "draw(Ljava/lang/String;FF)V",
                at = @At(value = "CONSTANT", args = { "intValue=1", "expandZeroConditions=true" }),
            )
        """)
        val site = sites(source).first()
        assertEquals(listOf("intValue=1", "expandZeroConditions=true"), site.atArgs)
        assertNull(site.atTarget)
    }

    @Test
    fun findAnnotationSitesKeepsAtTargetReservedForTargetOnly() {
        val source = trimmedSource("""
            @ModifyExpressionValue(
                method = "draw(Ljava/lang/String;FF)V",
                at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I", args = "foo=bar"),
            )
        """)
        val site = sites(source).first()
        assertEquals("Ljava/lang/String;length()I", site.atTarget)
        assertEquals(listOf("foo=bar"), site.atArgs)
    }

    @Test
    fun findAnnotationSitesIgnoresUnrelatedNestedStringsWhenExtractingAtArgs() {
        val source = trimmedSource("""
            @ModifyExpressionValue(
                method = "draw(Ljava/lang/String;FF)V",
                at = @At(
                    value = "CONSTANT",
                    args = { "intValue=1", "expandZeroConditions=true" },
                    remap = @Constant(stringValue = "ignored"),
                ),
            )
        """)
        val site = sites(source).first()
        assertEquals(listOf("intValue=1", "expandZeroConditions=true"), site.atArgs)
    }

    @Test
    fun findAnnotationSitesExtractsConstantShorthandWithSingleArgs() {
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("CONSTANT", args = "floatValue=0.0"))
        """)
        val site = sites(source).first()
        assertEquals("CONSTANT", site.atValue)
        assertNull(site.atTarget)
        assertEquals(listOf("floatValue=0.0"), site.atArgs)
        assertNull(site.atId)
    }

    @Test
    fun findAnnotationSitesExtractsConstantShorthandWithArgsArrayAndId() {
        val source = trimmedSource("""
            @ModifyExpressionValue(
                method = "draw(Ljava/lang/String;FF)V",
                at = @At("CONSTANT", args = { "intValue=1", "expandZeroConditions=true" }, id = "main"),
            )
        """)
        val site = sites(source).first()
        assertEquals("CONSTANT", site.atValue)
        assertNull(site.atTarget)
        assertEquals(listOf("intValue=1", "expandZeroConditions=true"), site.atArgs)
        assertEquals("main", site.atId)
    }

    @Test
    fun findAnnotationSitesExtractsPlainShorthandAtValueWithoutExtraAttributes() {
        val source = trimmedSource("""
            @ModifyReturnValue(method = "compute()I", at = @At("RETURN"))
        """)
        val site = sites(source).first()
        assertEquals("RETURN", site.atValue)
        assertNull(site.atTarget)
        assertTrue(site.atArgs.isEmpty())
        assertNull(site.atId)
        assertNull(site.atOrdinal)
    }

    @Test
    fun findAnnotationSitesParsesAtOrdinalZeroAndPositive() {
        val zeroSource = trimmedSource("""
            @ModifyExpressionValue(
                method = "draw(Ljava/lang/String;FF)V",
                at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I", ordinal = 0),
            )
        """)
        val positiveSource = trimmedSource("""
            @ModifyExpressionValue(
                method = "draw(Ljava/lang/String;FF)V",
                at = @At("INVOKE", target = "Ljava/lang/String;length()I", ordinal = 2),
            )
        """)
        assertEquals(0, sites(zeroSource).first().atOrdinal)
        assertEquals(2, sites(positiveSource).first().atOrdinal)
    }

    @Test
    fun findAnnotationSitesTreatsMissingAndNegativeOneAtOrdinalAsNull() {
        val missingSource = trimmedSource("""
            @ModifyReturnValue(method = "compute()I", at = @At("RETURN"))
        """)
        val negativeOneSource = trimmedSource("""
            @ModifyExpressionValue(
                method = "draw(Ljava/lang/String;FF)V",
                at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I", ordinal = -1),
            )
        """)
        assertNull(sites(missingSource).first().atOrdinal)
        assertNull(sites(negativeOneSource).first().atOrdinal)
    }

    @Test
    fun findAnnotationSitesIgnoresMalformedAtOrdinalWithoutCorruptingLaterAttributes() {
        val malformedCases = listOf(
            """ordinal = 0x10, target = "Ljava/lang/String;length()I", id = "main"""",
            """ordinal = 1_000, target = "Ljava/lang/String;length()I", id = "main"""",
            """ordinal = 2147483648, target = "Ljava/lang/String;length()I", id = "main"""",
            """ordinal = MIXINEXTRAS:EXPRESSION, target = "Ljava/lang/String;length()I", id = "main"""",
            """ordinal = 1.5, target = "Ljava/lang/String;length()I", id = "main"""",
        )
        for (atAttributes in malformedCases) {
            val source = trimmedSource("""
                @ModifyExpressionValue(
                    method = "draw(Ljava/lang/String;FF)V",
                    at = @At(value = "INVOKE", $atAttributes),
                )
            """)
            val site = sites(source).first()
            assertNull(site.atOrdinal, "expected null ordinal for: $atAttributes")
            assertEquals("INVOKE", site.atValue)
            assertEquals("Ljava/lang/String;length()I", site.atTarget)
            assertEquals("main", site.atId)
        }
    }

    @Test
    fun findAnnotationSitesKeepsOrdinalLikeArgsTextInAtArgsOnly() {
        val source = trimmedSource("""
            @WrapOperation(
                method = "run()V",
                at = @At(value = "NEW", target = "Ljava/lang/StringBuilder;", args = "ordinal=0"),
            )
        """)
        val site = sites(source).first()
        assertNull(site.atOrdinal)
        assertEquals(listOf("ordinal=0"), site.atArgs)
    }

    @Test
    fun findAnnotationSitesDefaultsAtShiftToBefore() {
        val source = trimmedSource("""
            @ModifyReturnValue(method = "compute()I", at = @At("RETURN"))
        """)
        assertEquals(AtShiftSpec.Before, sites(source).first().atShift)
    }

    @Test
    fun findAnnotationSitesParsesAtShiftAfter() {
        val source = trimmedSource("""
            @ModifyExpressionValue(
                method = "draw(Ljava/lang/String;FF)V",
                at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I", shift = At.Shift.AFTER),
            )
        """)
        assertEquals(AtShiftSpec.After, sites(source).first().atShift)
    }

    @Test
    fun findAnnotationSitesParsesAtShiftByWithPositiveAndNegativeOffset() {
        val positiveSource = trimmedSource("""
            @ModifyExpressionValue(
                method = "draw(Ljava/lang/String;FF)V",
                at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I", shift = BY, by = 2),
            )
        """)
        val negativeSource = trimmedSource("""
            @ModifyExpressionValue(
                method = "draw(Ljava/lang/String;FF)V",
                at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I", shift = At.Shift.BY, by = -1),
            )
        """)
        val positiveShift = sites(positiveSource).first().atShift
        val negativeShift = sites(negativeSource).first().atShift
        assertEquals(AtShiftSpec.By(2), positiveShift)
        assertEquals(AtShiftSpec.By(-1), negativeShift)
    }

    @Test
    fun findAnnotationSitesParsesAtShiftByDefaultZeroWhenByOmitted() {
        val source = trimmedSource("""
            @ModifyExpressionValue(
                method = "draw(Ljava/lang/String;FF)V",
                at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I", shift = At.Shift.BY),
            )
        """)
        assertEquals(AtShiftSpec.By(0), sites(source).first().atShift)
    }

    @Test
    fun findAnnotationSitesMarksMalformedAtShiftAsUnresolved() {
        val source = trimmedSource("""
            @ModifyExpressionValue(
                method = "draw(Ljava/lang/String;FF)V",
                at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I", shift = At.Shift.MIDDLE),
            )
        """)
        assertEquals(AtShiftSpec.Unresolved, sites(source).first().atShift)
    }

    @Test
    fun findAnnotationSitesMarksMalformedAtByWhenShiftIsByAsUnresolved() {
        val source = trimmedSource("""
            @ModifyExpressionValue(
                method = "draw(Ljava/lang/String;FF)V",
                at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I", shift = BY, by = 2147483648),
            )
        """)
        assertEquals(AtShiftSpec.Unresolved, sites(source).first().atShift)
    }

    @Test
    fun findAnnotationSitesIgnoresByOnBeforeAndAfter() {
        val beforeSource = trimmedSource("""
            @ModifyExpressionValue(
                method = "draw(Ljava/lang/String;FF)V",
                at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I", shift = BEFORE, by = 3),
            )
        """)
        val afterSource = trimmedSource("""
            @ModifyExpressionValue(
                method = "draw(Ljava/lang/String;FF)V",
                at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I", shift = AFTER, by = -2),
            )
        """)
        assertEquals(AtShiftSpec.Before, sites(beforeSource).first().atShift)
        assertEquals(AtShiftSpec.After, sites(afterSource).first().atShift)
    }

    @Test
    fun findAnnotationSitesParsesAtShiftWithReorderedAttributesAndWhitespace() {
        val source = trimmedSource("""
            @ModifyExpressionValue(
                method = "draw(Ljava/lang/String;FF)V",
                at = @At(
                    by = 1 ,
                    target = "Ljava/lang/String;length()I" ,
                    shift = At.Shift.BY ,
                    value = "INVOKE" ,
                    id = "main",
                ),
            )
        """)
        val site = sites(source).first()
        assertEquals(AtShiftSpec.By(1), site.atShift)
        assertEquals("INVOKE", site.atValue)
        assertEquals("Ljava/lang/String;length()I", site.atTarget)
        assertEquals("main", site.atId)
    }

    @Test
    fun modifyExpressionValueConstantInfersHandlerTypeFromAtArgs() {
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        val cases = listOf(
            """@At(value = "CONSTANT", args = "intValue=42")""" to "I",
            """@At(value = "CONSTANT", args = "floatValue=0.0")""" to "F",
            """@At(value = "CONSTANT", args = "longValue=42")""" to "J",
            """@At(value = "CONSTANT", args = "doubleValue=1.0d")""" to "D",
            """@At(value = "CONSTANT", args = "stringValue=hello")""" to "Ljava/lang/String;",
            """@At(value = "CONSTANT", args = "stringValue=")""" to "Ljava/lang/String;",
            """@At(value = "CONSTANT", args = "classValue=java.lang.String")""" to "Ljava/lang/Class;",
            """@At(value = "CONSTANT", args = "nullValue=true")""" to "Ljava/lang/Object;",
            """@At(value = "CONSTANT", args = { "intValue=1", "expandZeroConditions=true" })""" to "I",
        )
        for ((at, expectedDescriptor) in cases) {
            val source = trimmedSource("""
                @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = $at)
            """)
            val site = sites(source).first()
            val spec = service.expectedSignature(source, site, mixinTargets)
            assertNotNull(spec, "expected signature for $at")
            assertEquals(expectedDescriptor, spec.returnTypeDescriptor, "return type for $at")
            assertEquals(expectedDescriptor, spec.parameters.single().typeDescriptor, "parameter type for $at")
            val stub = service.generateHandlerStub(source, site, mixinTargets)
            assertNotNull(stub, "expected stub for $at")
            assertTrue(stub.contains("mcdevHandler"), "stub method name for $at")
            assertTrue(stub.contains("original"), "stub original parameter for $at")
        }
    }

    @Test
    fun modifyExpressionValueConstantRejectsUnresolvedAtArgsForSignatureAndStub() {
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        val rejectedAtSelectors = listOf(
            """@At(value = "CONSTANT")""",
            """@At(value = "CONSTANT", args = "expandZeroConditions=true")""",
            """@At(value = "CONSTANT", args = { "intValue=1", "floatValue=1.0" })""",
            """@At(value = "CONSTANT", args = "nullValue=false")""",
            """@At(value = "CONSTANT", args = "intValue=abc")""",
            """@At(value = "CONSTANT", args = "classValue=")""",
        )
        for (at in rejectedAtSelectors) {
            val source = trimmedSource("""
                @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = $at)
            """)
            val site = sites(source).first()
            assertNull(service.expectedSignature(source, site, mixinTargets), "expected no signature for $at")
            assertNull(service.generateHandlerStub(source, site, mixinTargets), "expected no stub for $at")
        }
    }

    @Test
    fun modifyExpressionValueRejectsUnsupportedAtValuesWithoutFakeSignatures() {
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        val unsupportedAtSelectors = listOf(
            "RETURN" to """@At("RETURN")""",
            "HEAD" to """@At("HEAD")""",
            "TAIL" to """@At("TAIL")""",
            "LOAD" to """@At("LOAD")""",
            "STORE" to """@At("STORE")""",
            "unknown" to """@At(value = "UNKNOWN")""",
            "missing atValue" to """@At(target = "Ljava/lang/String;length()I")""",
        )
        for ((caseName, at) in unsupportedAtSelectors) {
            val source = trimmedSource("""
                @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = $at)
                private float mcdevHandler(float original) {
                    return original;
                }
            """)
            val site = sites(source).first()
            assertNull(service.expectedSignature(source, site, mixinTargets), "expected no signature for $caseName")
            assertNull(service.generateHandlerStub(source, site, mixinTargets), "expected no stub for $caseName")
            val handler = enrich(site.handlerMethod!!)
            val issues = service.validateHandler(source, site, mixinTargets, handler)
            assertTrue(
                issues.none { it.code == MixinExtrasDiagnosticCodes.WRONG_ORIGINAL_VALUE_TYPE },
                "expected no fake original type issue for $caseName but got $issues",
            )
            assertTrue(
                issues.none { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE },
                "expected no fake return type issue for $caseName but got $issues",
            )
        }
    }

    @Test
    fun modifyReturnValueReturnAtValueStillInfersMethodReturnType() {
        val source = trimmedSource("""
            @ModifyReturnValue(method = "compute()I", at = @At("RETURN"))
        """)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals("I", spec.returnTypeDescriptor)
        assertEquals("I", spec.parameters.single().typeDescriptor)
    }

    @Test
    fun modifyExpressionValueConstantValidatesHandlerForEachDiscriminator() {
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        val cases = listOf(
            "intValue=42" to "int mcdevHandler(int original) { return original; }",
            "floatValue=0.0" to "float mcdevHandler(float original) { return original; }",
            "longValue=42" to "long mcdevHandler(long original) { return original; }",
            "doubleValue=1.0d" to "double mcdevHandler(double original) { return original; }",
            "stringValue=hello" to "String mcdevHandler(String original) { return original; }",
        )
        for ((args, handlerDecl) in cases) {
            val source = trimmedSource("""
                @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "$args"))
                private $handlerDecl
            """)
            val site = sites(source).first()
            val handler = enrich(site.handlerMethod!!)
            val issues = service.validateHandler(source, site, mixinTargets, handler)
            assertTrue(issues.isEmpty(), "expected valid handler for args=$args but got $issues")
        }
    }

    @Test
    fun modifyReturnValueVoidTargetIsInvalid() {
        val source = trimmedSource(MixinExtrasTestFixtures.MODIFY_RETURN_VOID_SOURCE)
        val site = sites(source).first()
        assertNull(service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget")))
        assertNull(service.generateHandlerStub(source, site, listOf("com/example/target/SimpleTarget")))
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertEquals(1, issues.size)
        assertEquals(MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH, issues.single().code)
        assertEquals("ModifyReturnValue cannot target a void method", issues.single().message)
    }

    @Test
    fun wrapOperationExposesOptionalCapturedTargetParameters() {
        val source = trimmedSource(MixinExtrasTestFixtures.WRAP_OPERATION_SOURCE)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals(2, spec.parameters.size)
        assertEquals(3, spec.optionalCapturedTargetParameters.size)
        assertEquals("Ljava/lang/String;", spec.optionalCapturedTargetParameters[0].typeDescriptor)
        assertEquals("F", spec.optionalCapturedTargetParameters[1].typeDescriptor)
        assertEquals("F", spec.optionalCapturedTargetParameters[2].typeDescriptor)
    }

    @Test
    fun generatesWrapOperationHandlerStubStaysMinimalWithTargetParameters() {
        val source = trimmedSource(MixinExtrasTestFixtures.WRAP_OPERATION_NO_HANDLER)
        val site = sites(source).first()
        val stub = service.generateHandlerStub(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(stub)
        assertTrue(stub.contains("String instance, Operation<Integer> original"))
        assertTrue(!stub.contains("arg0"))
    }

    @Test
    fun validatesWrapOperationHandlerWithFirstCapturedTargetParameterAfterOperation() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original, String arg0) {
                return original.call(instance);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun validatesWrapOperationHandlerWithFullCapturedTargetParameterPrefixAfterOperation() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original, String arg0, float arg1, float arg2) {
                return original.call(instance);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun rejectsWrapOperationHandlerWithCapturedTargetParameterBeforeOperation() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, String arg0, Operation<Integer> original) {
                return original.call(instance);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.MISSING_OPERATION_PARAMETER })
    }

    @Test
    fun rejectsWrapOperationHandlerWithSkippedCapturedTargetParameter() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original, float arg1) {
                return original.call(instance);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun rejectsWrapOperationHandlerWithWrongCapturedTargetParameterType() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original, int arg0) {
                return original.call(instance);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun rejectsWrapOperationHandlerWithTooManyCapturedTargetParameters() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original, String arg0, float arg1, float arg2, int extra) {
                return original.call(instance);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun wrapMethodStillRequiresOperationAsLastNonSugarParameter() {
        val source = trimmedSource("""
            @WrapMethod(method = "draw(Ljava/lang/String;FF)V")
            private void mcdevWrapDraw(String arg0, float arg1, Operation<Void> original, float arg2) {
                original.call(arg0, arg1, arg2);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.MISSING_OPERATION_PARAMETER })
    }

    @Test
    fun wrapOperationExpectsReceiverOperationLast() {
        val source = trimmedSource(MixinExtrasTestFixtures.WRAP_OPERATION_SOURCE)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals(2, spec.parameters.size)
        assertEquals("String", spec.parameters.first().readableType)
        assertTrue(spec.parameters.last().isOperation)
        assertEquals("I", spec.parameters.last().operationGenericDescriptor)
        assertEquals("original", spec.parameters.last().name)
    }

    @Test
    fun wrapOperationExpectsIntReturnType() {
        val source = trimmedSource(MixinExtrasTestFixtures.WRAP_OPERATION_SOURCE)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals("I", spec.returnTypeDescriptor)
    }

    @Test
    fun wrapOperationRejectsUnsupportedAtValuesAndConstructorInvokes() {
        val selectors = listOf(
            "unsupported value" to """@At(value = "RETURN", target = "Ljava/lang/String;length()I")""",
            "missing value" to """@At(target = "Ljava/lang/String;length()I")""",
            "constructor invoke" to """@At(value = "INVOKE", target = "Ljava/lang/String;<init>()V")""",
        )
        for ((name, at) in selectors) {
            val source = trimmedSource("""
                @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = $at)
            """)
            val site = sites(source).first()
            assertNull(
                service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget")),
                name,
            )
            assertNull(
                service.generateHandlerStub(source, site, listOf("com/example/target/SimpleTarget")),
                name,
            )
        }
    }

    @Test
    fun wrapOperationInvokeSpecialUsesMixinTargetReceiver() {
        val parentOwner = "com/example/target/Parent"
        val classIndex = FakeClassIndex(
            classes = MixinExtrasTestFixtures.classIndex.findClasses("", 100) + listOf(
                ClassIndexEntry("Parent", "com.example.target", parentOwner),
            ),
            methods = FakeClassIndex.defaultMethods() + mapOf(
                "java/lang/String" to listOf(
                    MethodIndexEntry("length", "()I", false, "length(): int"),
                ),
                "com/example/target/SimpleTarget" to listOf(
                    MethodIndexEntry("draw", "(Ljava/lang/String;FF)V", false, "draw(String, float, float): void"),
                ),
                parentOwner to listOf(
                    MethodIndexEntry("foo", "()V", false, "foo(): void"),
                ),
            ),
            fields = emptyMap(),
        )
        val service = HandlerSignatureService(
            classIndex,
            FakeBytecodeIndex(
                candidates = mapOf(
                    "com/example/target/SimpleTarget#draw#INVOKE" to listOf(
                        invokeCandidate(
                            operationKind = AtTargetOperationKind.INVOKE_SPECIAL,
                            owner = parentOwner,
                            name = "foo",
                            descriptor = "()V",
                        ),
                    ),
                ),
            ),
        )
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Lcom/example/target/Parent;foo()V"))
        """)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals("Lcom/example/target/SimpleTarget;", spec.parameters.first().typeDescriptor)
        assertEquals("V", spec.parameters.last().operationGenericDescriptor)
    }

    @Test
    fun wrapOperationInvokeSpecialOrdinalUsesSelectedInvocationReceiver() {
        val parentOwner = "com/example/target/Parent"
        val classIndex = FakeClassIndex(
            classes = MixinExtrasTestFixtures.classIndex.findClasses("", 100) + listOf(
                ClassIndexEntry("Parent", "com.example.target", parentOwner),
            ),
            methods = FakeClassIndex.defaultMethods() + mapOf(
                "com/example/target/SimpleTarget" to listOf(
                    MethodIndexEntry("draw", "(Ljava/lang/String;FF)V", false, "draw(String, float, float): void"),
                ),
                parentOwner to listOf(
                    MethodIndexEntry("foo", "()V", false, "foo(): void"),
                ),
            ),
            fields = emptyMap(),
        )
        val service = HandlerSignatureService(
            classIndex,
            FakeBytecodeIndex(
                candidates = mapOf(
                    "com/example/target/SimpleTarget#draw#INVOKE" to listOf(
                        invokeCandidate(
                            operationKind = AtTargetOperationKind.INVOKE_VIRTUAL,
                            owner = parentOwner,
                            name = "foo",
                            descriptor = "()V",
                        ).copy(instructionOccurrenceIndex = 0),
                        invokeCandidate(
                            operationKind = AtTargetOperationKind.INVOKE_SPECIAL,
                            owner = parentOwner,
                            name = "foo",
                            descriptor = "()V",
                        ).copy(instructionOccurrenceIndex = 1),
                    ),
                ),
            ),
        )
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Lcom/example/target/Parent;foo()V", ordinal = 1))
        """)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals("Lcom/example/target/SimpleTarget;", spec.parameters.first().typeDescriptor)
    }

    @Test
    fun wrapOperationExpressionUsesOfficialSimpleOperationLayout() {
        val owner = expressionMatchSamplesOwner()
        val expressionService = HandlerSignatureService(
            expressionMatchSamplesClassIndex(),
            expressionMatchSamplesBytecodeIndex(),
        )
        val source = trimmedSource("""
            @Mixin(ExpressionMatchSamples.class)
            abstract class ExampleMixin {
                @Expression(id = "main", value = "@(?[?])")
                @WrapOperation(
                    method = "arrayAccess([II)V",
                    at = @At(id = "main", value = "MIXINEXTRAS:EXPRESSION"),
                )
                private int mcdevHandler(int[] array, int index, Operation<Integer> original) {
                    return original.call(array, index);
                }
            }
        """.trimIndent())
        val site = sites(source).single()
        val spec = expressionService.expectedSignature(source, site, listOf(owner))

        assertNotNull(spec)
        assertEquals("I", spec.returnTypeDescriptor)
        assertEquals(listOf("array", "index", "original"), spec.parameters.map { it.name })
        assertEquals(listOf("[I", "I"), spec.parameters.take(2).map { it.typeDescriptor })
        assertTrue(spec.parameters.last().isOperation)
        assertEquals("Operation<Integer>", spec.parameters.last().readableType)
        assertEquals("I", spec.parameters.last().operationGenericDescriptor)
        assertEquals(listOf("array", "index"), spec.operationCallArgs)
        assertEquals(listOf("[I", "I"), spec.optionalCapturedTargetParameters.map { it.typeDescriptor })

        val stub = expressionService.generateHandlerStub(source, site, listOf(owner))
        assertNotNull(stub)
        assertTrue(stub.contains("int mcdevHandler(int[] array, int index, Operation<Integer> original)"))
        assertTrue(stub.contains("return original.call(array, index);"))

        val handler = HandlerSignatureService.enrichHandlerTypes(
            site.handlerMethod!!,
            expressionMatchSamplesClassIndex(),
        )
        assertTrue(expressionService.validateHandler(source, site, listOf(owner), handler).isEmpty())
    }

    @Test
    fun wrapWithConditionExpressionUsesOfficialFieldWriteLayout() {
        val owner = expressionMatchSamplesOwner()
        val expressionService = HandlerSignatureService(
            expressionMatchSamplesClassIndex(),
            expressionMatchSamplesBytecodeIndex(),
        )
        val source = trimmedSource("""
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
        """.trimIndent())
        val site = sites(source).single()
        val mixinTargets = listOf(owner)

        val spec = expressionService.expectedSignature(source, site, mixinTargets)
        assertNotNull(spec)
        assertEquals("Z", spec.returnTypeDescriptor)
        assertEquals(listOf("L$owner;", "I"), spec.parameters.map { it.typeDescriptor })

        val stub = expressionService.generateHandlerStub(source, site, mixinTargets)
        assertNotNull(stub)
        assertTrue(stub.contains("boolean mcdevHandler(ExpressionMatchSamples expressionMatchSamples, int arg0)"))

        val handler = HandlerSignatureService.enrichHandlerTypes(site.handlerMethod!!, expressionMatchSamplesClassIndex())
        assertTrue(expressionService.validateHandler(source, site, mixinTargets, handler).isEmpty())
    }

    @Test
    fun wrapWithConditionExpressionUsesOfficialVoidInvocationLayout() {
        val owner = invokeSamplesOwner()
        val expressionService = HandlerSignatureService(invokeSamplesClassIndex(), invokeSamplesBytecodeIndex())
        val source = trimmedSource("""
            @Mixin(InvokeSamples.class)
            abstract class ExampleMixin {
                @Expression("@(?.?())")
                @WrapWithCondition(
                    method = "interfaceInvoke()V",
                    at = @At(value = "MIXINEXTRAS:EXPRESSION"),
                )
                private boolean mcdevHandler(Runnable instance) {
                    return true;
                }
            }
        """.trimIndent())
        val site = sites(source).single()
        val mixinTargets = listOf(owner)

        val spec = expressionService.expectedSignature(source, site, mixinTargets)
        assertNotNull(spec)
        assertEquals("Z", spec.returnTypeDescriptor)
        assertEquals(listOf("Ljava/lang/Runnable;"), spec.parameters.map { it.typeDescriptor })
        assertEquals(WrapWithConditionTargetStatus.VALID_VOID, expressionService.wrapWithConditionTargetStatus(
            site = site,
            mixinTargets = mixinTargets,
            source = source,
        ))
    }

    @Test
    fun wrapWithConditionExpressionRecognizesPoppedNonVoidInvocation() {
        val owner = invokeSamplesOwner()
        val expressionService = HandlerSignatureService(invokeSamplesClassIndex(), invokeSamplesBytecodeIndex())
        val source = trimmedSource("""
            @Mixin(InvokeSamples.class)
            abstract class ExampleMixin {
                @Definition(id = "stringLength", method = "Ljava/lang/String;length()I")
                @Expression("@(?.stringLength())")
                @WrapWithCondition(
                    method = "virtualInvoke()V",
                    at = @At(value = "MIXINEXTRAS:EXPRESSION"),
                )
                private boolean mcdevHandler(String instance) {
                    return true;
                }
            }
        """.trimIndent())
        val site = sites(source).single()
        val mixinTargets = listOf(owner)

        assertEquals("Ljava/lang/String;", expressionService.expectedSignature(source, site, mixinTargets)
            ?.parameters?.single()?.typeDescriptor)
        assertEquals(WrapWithConditionTargetStatus.VALID_POPPED_NON_VOID, expressionService.wrapWithConditionTargetStatus(
            site = site,
            mixinTargets = mixinTargets,
            source = source,
        ))
    }

    @Test
    fun wrapWithConditionExpressionRejectsOfficialFieldRead() {
        val owner = expressionMatchSamplesOwner()
        val expressionService = HandlerSignatureService(
            expressionMatchSamplesClassIndex(),
            expressionMatchSamplesBytecodeIndex(),
        )
        val source = trimmedSource("""
            @Mixin(ExpressionMatchSamples.class)
            abstract class ExampleMixin {
                @Expression("@(?.?)")
                @WrapWithCondition(
                    method = "readSampleField()I",
                    at = @At(value = "MIXINEXTRAS:EXPRESSION"),
                )
                private boolean mcdevHandler(ExpressionMatchSamples instance) {
                    return true;
                }
            }
        """.trimIndent())
        val site = sites(source).single()
        val mixinTargets = listOf(owner)

        assertEquals(
            WrapWithConditionTargetStatus.INVALID_INSTRUCTION,
            expressionService.wrapWithConditionTargetStatus(
                site = site,
                mixinTargets = mixinTargets,
                source = source,
            ),
        )
        assertNull(expressionService.expectedSignature(source, site, mixinTargets))
        assertNull(expressionService.generateHandlerStub(source, site, mixinTargets))
    }

    @Test
    fun modifyReceiverExpressionUsesOfficialInvocationLayout() {
        val owner = expressionMatchSamplesOwner()
        val expressionService = HandlerSignatureService(
            expressionMatchSamplesClassIndex(),
            expressionMatchSamplesBytecodeIndex(),
        )
        val source = trimmedSource("""
            @Mixin(ExpressionMatchSamples.class)
            abstract class ExampleMixin {
                @Expression("@(?.?())")
                @ModifyReceiver(
                    method = "trim(Ljava/lang/String;)Ljava/lang/String;",
                    at = @At(value = "MIXINEXTRAS:EXPRESSION"),
                )
                private String mcdevHandler(String instance, String arg0) {
                    return instance;
                }
            }
        """.trimIndent())
        val site = sites(source).single()
        val mixinTargets = listOf(owner)

        val spec = expressionService.expectedSignature(source, site, mixinTargets)
        assertNotNull(spec)
        assertEquals("Ljava/lang/String;", spec.returnTypeDescriptor)
        assertEquals(listOf("String"), spec.parameters.map { it.readableType })
        assertEquals(listOf("Ljava/lang/String;"), spec.parameters.map { it.typeDescriptor })
        assertEquals(listOf("instance"), spec.parameters.map { it.name })
        assertEquals(
            listOf("Ljava/lang/String;"),
            spec.optionalCapturedTargetParameters.map { it.typeDescriptor },
        )

        val stub = expressionService.generateHandlerStub(source, site, mixinTargets)
        assertNotNull(stub)
        assertTrue(stub.contains("String mcdevHandler(String instance)"))
        assertTrue(stub.contains("return instance;"))
        assertTrue(!stub.contains("arg0"))

        val handler = HandlerSignatureService.enrichHandlerTypes(site.handlerMethod!!, classIndex)
        assertTrue(expressionService.validateHandler(source, site, mixinTargets, handler).isEmpty())

        val unavailableService = HandlerSignatureService(expressionMatchSamplesClassIndex())
        assertNull(unavailableService.expectedSignature(source, site, mixinTargets))
    }

    @Test
    fun wrapOperationExpressionFailsClosedWithoutBytecodeIndex() {
        val expressionService = HandlerSignatureService(expressionMatchSamplesClassIndex())
        val source = trimmedSource("""
            @Mixin(ExpressionMatchSamples.class)
            abstract class ExampleMixin {
                @Expression(id = "main", value = "@(?[?])")
                @WrapOperation(
                    method = "arrayAccess([II)V",
                    at = @At(id = "main", value = "MIXINEXTRAS:EXPRESSION"),
                )
            }
        """.trimIndent())
        val site = sites(source).single()
        val mixinTargets = listOf(expressionMatchSamplesOwner())

        assertNull(expressionService.expectedSignature(source, site, mixinTargets))
        assertNull(expressionService.generateHandlerStub(source, site, mixinTargets))
    }

    @Test
    fun wrapOperationInvokeOutOfRangeOrdinalReturnsNullSignatureAndStub() {
        val service = wrapWithConditionInvokeService(
            wrapWithConditionInvokeCandidate(
                operationKind = AtTargetOperationKind.INVOKE_VIRTUAL,
                instructionOccurrenceIndex = 0,
            ),
        )
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I", ordinal = 1))
        """)
        val site = sites(source).first()
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        assertNull(service.expectedSignature(source, site, mixinTargets))
        assertNull(service.generateHandlerStub(source, site, mixinTargets))
    }

    @Test
    fun findAnnotationSitesParsesAtIdFromExpressionSelector() {
        val source = trimmedSource("""
            @ModifyExpressionValue(
                id = "injector",
                method = "draw(Ljava/lang/String;FF)V",
                at = @At(  id  =  "main"  ,  value  =  "MIXINEXTRAS:EXPRESSION"  ),
            )
        """)
        val site = sites(source).first()
        assertEquals("MIXINEXTRAS:EXPRESSION", site.atValue)
        assertEquals("main", site.atId)
    }

    @Test
    fun providedResolvedContextControlsModifyExpressionValueSignatureDespiteContradictorySource() {
        val classIndex = expressionMatchSamplesClassIndex()
        val expressionService = HandlerSignatureService(classIndex, expressionMatchSamplesBytecodeIndex())
        val owner = expressionMatchSamplesOwner()
        val source = trimmedSource(expressionMatchSamplesHandlerSource(
            suffix = "private String mcdevHandler(String original) { return original; }",
        ))
        val site = sites(source).first()
        val mixinTargets = listOf(owner)
        val resolvedContext = resolvedIntSampleFieldContext()

        val spec = expressionService.expectedSignature(source, site, mixinTargets, resolvedContext)
        assertNotNull(spec)
        assertEquals("I", spec.returnTypeDescriptor)
        assertEquals("I", spec.parameters.single().typeDescriptor)

        val stub = expressionService.generateHandlerStub(source, site, mixinTargets, resolvedContext = resolvedContext)
        assertNotNull(stub)
        assertTrue(stub.contains("int mcdevHandler(int original)"))

        val matchingHandler = enrich(
            site.handlerMethod!!.let {
                it.copy(
                    returnTypeName = "int",
                    parameters = listOf(
                        it.parameters.first().copy(typeName = "int"),
                    ),
                )
            },
        )
        assertTrue(
            expressionService.validateHandler(source, site, mixinTargets, matchingHandler, resolvedContext).isEmpty(),
        )

        val contradictoryHandler = enrich(site.handlerMethod!!)
        assertTrue(
            expressionService.validateHandler(source, site, mixinTargets, contradictoryHandler, resolvedContext)
                .any { it.code == MixinExtrasDiagnosticCodes.WRONG_ORIGINAL_VALUE_TYPE },
        )
    }

    @Test
    fun providedEmptyResolvedContextSuppressesModifyExpressionValueSignatureInsteadOfObjectFallback() {
        val expressionService = HandlerSignatureService(
            MixinExtrasTestFixtures.classIndex,
            MixinExtrasTestFixtures.bytecodeIndex,
        )
        val source = trimmedSource(expressionHandlerSource(
            prefix = """@Expression("text.length()")""",
            at = """@At(value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = "private int mcdevHandler(int original) { return original; }",
        ))
        val site = sites(source).first()
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        val resolvedContext = ExpressionContext(
            expressionIndex = MixinExtrasExpressionIndex(),
            definitionIndex = MixinExtrasDefinitionIndex(),
        )

        assertNotNull(expressionService.expectedSignature(source, site, mixinTargets))
        assertNull(expressionService.expectedSignature(source, site, mixinTargets, resolvedContext))
        assertNull(expressionService.generateHandlerStub(source, site, mixinTargets, resolvedContext = resolvedContext))

        val handler = enrich(site.handlerMethod!!)
        val issues = expressionService.validateHandler(source, site, mixinTargets, handler, resolvedContext)
        assertTrue(issues.none { it.code == MixinExtrasDiagnosticCodes.WRONG_ORIGINAL_VALUE_TYPE })
        assertTrue(issues.none { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE })
    }

    @Test
    fun modifyExpressionValueExpressionSelectorFailsClosedWithoutBytecodeIndex() {
        val source = trimmedSource(expressionHandlerSource(
            prefix = """@Expression("text.length()")""",
            at = """@At(value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = "",
        ))
        val site = sites(source).first()
        val mixinTargets = listOf("com/example/target/SimpleTarget")

        assertNull(service.expectedSignature(source, site, mixinTargets))
        assertNull(service.generateHandlerStub(source, site, mixinTargets))
    }

    @Test
    fun modifyExpressionValueExpressionSelectorFailsClosedOnOfficialNoMatch() {
        val expressionService = HandlerSignatureService(
            expressionMatchSamplesClassIndex(),
            expressionMatchSamplesBytecodeIndex(),
        )
        val owner = expressionMatchSamplesOwner()
        val source = trimmedSource("""
            @Mixin(ExpressionMatchSamples.class)
            abstract class ExampleMixin {
                @Expression("@('absent-literal')")
                @ModifyExpressionValue(method = "readSampleField()I", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
            }
        """.trimIndent())
        val site = sites(source).first()
        val mixinTargets = listOf(owner)

        assertNull(expressionService.expectedSignature(source, site, mixinTargets))
        assertNull(expressionService.generateHandlerStub(source, site, mixinTargets))
    }

    @Test
    fun modifyExpressionValueExpressionSelectorFailsClosedOnUnsupportedHandwrittenWithResolvedContext() {
        val expressionService = HandlerSignatureService(
            MixinExtrasTestFixtures.classIndex,
            MixinExtrasTestFixtures.bytecodeIndex,
        )
        val source = trimmedSource(expressionHandlerSource(
            prefix = """@Expression("text.length()")""",
            at = """@At(value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = "",
        ))
        val site = sites(source).first()
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        val resolvedContext = ExpressionContext(
            expressionIndex = MixinExtrasExpressionIndex(
                expressions = listOf(MixinExtrasExpression(values = listOf("text.length()"))),
            ),
            definitionIndex = MixinExtrasDefinitionIndex(),
        )

        assertNull(expressionService.expectedSignature(source, site, mixinTargets, resolvedContext))
        assertNull(expressionService.generateHandlerStub(source, site, mixinTargets, resolvedContext = resolvedContext))
    }

    @Test
    fun modifyExpressionValueExpressionSelectorUsesSupportedHandwrittenIntFallback() {
        val expressionService = HandlerSignatureService(
            MixinExtrasTestFixtures.classIndex,
            MixinExtrasTestFixtures.bytecodeIndex,
        )
        val source = trimmedSource(expressionHandlerSource(
            prefix = """@Expression("text.length()")""",
            at = """@At(value = "MIXINEXTRAS:EXPRESSION")""",
            suffix = "",
        ))
        val site = sites(source).first()
        val mixinTargets = listOf("com/example/target/SimpleTarget")

        val spec = expressionService.expectedSignature(source, site, mixinTargets)
        assertNotNull(spec)
        assertEquals("I", spec.returnTypeDescriptor)
        assertEquals("I", spec.parameters.single().typeDescriptor)

        val stub = expressionService.generateHandlerStub(source, site, mixinTargets)
        assertNotNull(stub)
        assertTrue(stub.contains("int mcdevHandler(int original)"))
    }

    @Test
    fun modifyExpressionValueExpressionSelectorFailsClosedOnOfficialConflict() {
        val expressionService = HandlerSignatureService(
            expressionMatchSamplesClassIndex(),
            expressionMatchSamplesBytecodeIndex(),
        )
        val owner = expressionMatchSamplesOwner()
        val source = trimmedSource("""
            @Mixin(ExpressionMatchSamples.class)
            abstract class ExampleMixin {
                @Expression(id = "main", value = "@('prefix')")
                @Expression(id = "main", value = "@(?)")
                @ModifyExpressionValue(
                    method = "stringConcat(I)Ljava/lang/String;",
                    at = @At(id = "main", value = "MIXINEXTRAS:EXPRESSION"),
                )
            }
        """.trimIndent())
        val site = sites(source).first()
        val mixinTargets = listOf(owner)

        assertNull(expressionService.expectedSignature(source, site, mixinTargets))
        assertNull(expressionService.generateHandlerStub(source, site, mixinTargets))
    }

    @Test
    fun findAnnotationSitesReturnsNullAtIdWhenAtSelectorHasNoId() {
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
        """)
        val site = sites(source).first()
        assertEquals("INVOKE", site.atValue)
        assertEquals("Ljava/lang/String;length()I", site.atTarget)
        assertNull(site.atId)
    }

    @Test
    fun findAnnotationSitesExtractsInvokeShorthandWithTargetAfterShorthandValue() {
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("INVOKE", target = "Ljava/lang/String;length()I"))
        """)
        val site = sites(source).first()
        assertEquals("INVOKE", site.atValue)
        assertEquals("Ljava/lang/String;length()I", site.atTarget)
        assertTrue(site.atArgs.isEmpty())
        assertNull(site.atId)
    }

    @Test
    fun modifyExpressionValueInvokeTargetInfersReturnTypeFromExactMethodTarget() {
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        val cases = listOf(
            """@At(value = "INVOKE", target = "Ljava/lang/String;length()I")""" to "I",
            """@At(value = "INVOKE", target = "Ljava/lang/String;toString()Ljava/lang/String;")""" to "Ljava/lang/String;",
            """@At(value = "INVOKE", target = "Ljava/lang/String;getBytes()[B")""" to "[B",
        )
        for ((at, expectedDescriptor) in cases) {
            val source = trimmedSource("""
                @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = $at)
            """)
            val site = sites(source).first()
            val spec = service.expectedSignature(source, site, mixinTargets)
            assertNotNull(spec, "expected signature for $at")
            assertEquals(expectedDescriptor, spec.returnTypeDescriptor)
            assertEquals(expectedDescriptor, spec.parameters.single().typeDescriptor)
            assertEquals("original", spec.parameters.single().name)
        }
    }

    @Test
    fun modifyExpressionValueInvokeTargetRejectsVoidMalformedMissingAndFieldTargets() {
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        val rejectedCases = listOf(
            """@At(value = "INVOKE", target = "Lcom/example/target/SimpleTarget;noop()V")""",
            """@At(value = "INVOKE", target = "not-a-member-target")""",
            """@At(value = "INVOKE")""",
            """@At(value = "INVOKE", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;")""",
        )
        for (at in rejectedCases) {
            val source = trimmedSource("""
                @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = $at)
            """)
            val site = sites(source).first()
            assertNull(service.expectedSignature(source, site, mixinTargets), "expected no signature for $at")
            assertNull(service.generateHandlerStub(source, site, mixinTargets), "expected no stub for $at")
        }
    }

    @Test
    fun generatesModifyExpressionValueInvokeHandlerStub() {
        val source = trimmedSource(MixinExtrasTestFixtures.WRAP_OPERATION_NO_HANDLER.replace("WrapOperation", "ModifyExpressionValue"))
        val site = sites(source).first()
        val stub = service.generateHandlerStub(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(stub)
        assertTrue(stub.contains("int mcdevHandler(int original)"))
        assertTrue(stub.contains("return original"))
    }

    @Test
    fun validatesModifyExpressionValueInvokeHandlerSignature() {
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevHandler(int original) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun detectsWrongOriginalValueTypeForModifyExpressionValueInvoke() {
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevHandler(float original) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.WRONG_ORIGINAL_VALUE_TYPE })
    }

    @Test
    fun modifyExpressionValueGetFieldInfersExactFieldDescriptor() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE))
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
        """)
        val site = sites(source).first()
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        val spec = service.expectedSignature(source, site, mixinTargets)
        assertNotNull(spec)
        assertEquals("Ljava/lang/String;", spec.returnTypeDescriptor)
        assertEquals("Ljava/lang/String;", spec.parameters.single().typeDescriptor)
        assertEquals("original", spec.parameters.single().name)
    }

    @Test
    fun modifyExpressionValueGetStaticFieldInfersExactFieldDescriptor() {
        val service = fieldService(
            fieldCandidate(
                name = "FLAG",
                descriptor = "I",
                operationKind = AtTargetOperationKind.FIELD_GET_STATIC,
            ),
        )
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;FLAG:I"))
        """)
        val site = sites(source).first()
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        val spec = service.expectedSignature(source, site, mixinTargets)
        assertNotNull(spec)
        assertEquals("I", spec.returnTypeDescriptor)
        assertEquals("I", spec.parameters.single().typeDescriptor)
    }

    @Test
    fun generatesModifyExpressionValueFieldGetHandlerStub() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE))
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
        """)
        val site = sites(source).first()
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        val stub = service.generateHandlerStub(source, site, mixinTargets)
        assertNotNull(stub)
        assertTrue(stub.contains("String mcdevHandler(String original)"))
        assertTrue(stub.contains("return original"))
    }

    @Test
    fun validatesModifyExpressionValueFieldGetHandlerSignature() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE))
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private String mcdevHandler(String original) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun modifyExpressionValueNewTargetInfersAllocatedTypeFromExplicitClassAndConstructor() {
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        val cases = listOf(
            """@At(value = "NEW", target = "Ljava/lang/StringBuilder;")""" to "Ljava/lang/StringBuilder;",
            """@At(value = "NEW", target = "Ljava/lang/StringBuilder;<init>()V")""" to "Ljava/lang/StringBuilder;",
            """@At(value = "NEW", target = "Ljava/lang/StringBuilder;<init>(I)V")""" to "Ljava/lang/StringBuilder;",
        )
        for ((at, expectedDescriptor) in cases) {
            val source = trimmedSource("""
                @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = $at)
            """)
            val site = sites(source).first()
            val spec = service.expectedSignature(source, site, mixinTargets)
            assertNotNull(spec, "expected signature for $at")
            assertEquals(expectedDescriptor, spec.returnTypeDescriptor)
            assertEquals(expectedDescriptor, spec.parameters.single().typeDescriptor)
            assertEquals("original", spec.parameters.single().name)
        }
    }

    @Test
    fun generatesModifyExpressionValueNewHandlerStub() {
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "NEW", target = "Ljava/lang/StringBuilder;"))
        """)
        val site = sites(source).first()
        val stub = service.generateHandlerStub(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(stub)
        assertTrue(stub.contains("StringBuilder mcdevHandler(StringBuilder original)"))
        assertTrue(stub.contains("return original"))
    }

    @Test
    fun validatesModifyExpressionValueNewHandlerSignature() {
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "NEW", target = "Ljava/lang/StringBuilder;"))
            private StringBuilder mcdevHandler(StringBuilder original) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun modifyExpressionValueNewInfersUniqueOwnerWhenTargetOmitted() {
        val caseService = newService(newCandidate("java/lang/StringBuilder"))
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "NEW"))
        """)
        val site = sites(source).first()
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        val spec = caseService.expectedSignature(source, site, mixinTargets)
        assertNotNull(spec)
        assertEquals("Ljava/lang/StringBuilder;", spec.returnTypeDescriptor)
        assertEquals("Ljava/lang/StringBuilder;", spec.parameters.single().typeDescriptor)
    }

    @Test
    fun modifyExpressionValueNewSameOwnerDuplicatesRemainUnambiguous() {
        val caseService = newService(
            newCandidate("java/lang/StringBuilder", ordinal = 0),
            newCandidate("java/lang/StringBuilder", ordinal = 1),
        )
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "NEW"))
        """)
        val site = sites(source).first()
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        val spec = caseService.expectedSignature(source, site, mixinTargets)
        assertNotNull(spec)
        assertEquals("Ljava/lang/StringBuilder;", spec.returnTypeDescriptor)
    }

    @Test
    fun modifyExpressionValueNewTargetRejectsMalformedFieldInvokeConstructorNonVoidArrayMissingIndexAndAmbiguous() {
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        val newAt = """@At(value = "NEW", target = "Ljava/lang/StringBuilder;")"""
        val rejectedCases = listOf(
            "malformed target" to service,
            "field target" to service,
            "invoke target" to service,
            "constructor non-void" to service,
            "array target" to service,
            "missing bytecode index" to service,
            "zero candidates" to newService(),
            "ambiguous owners" to newService(
                newCandidate("java/lang/StringBuilder"),
                newCandidate("java/lang/String"),
            ),
        )
        val atByCase = mapOf(
            "malformed target" to """@At(value = "NEW", target = "not-a-member-target")""",
            "field target" to """@At(value = "NEW", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;")""",
            "invoke target" to """@At(value = "NEW", target = "Ljava/lang/String;length()I")""",
            "constructor non-void" to """@At(value = "NEW", target = "Ljava/lang/StringBuilder;<init>(I)I")""",
            "array target" to """@At(value = "NEW", target = "[Ljava/lang/String;")""",
            "missing bytecode index" to """@At(value = "NEW")""",
            "zero candidates" to """@At(value = "NEW")""",
            "ambiguous owners" to """@At(value = "NEW")""",
        )
        for ((caseName, caseService) in rejectedCases) {
            val at = atByCase.getValue(caseName)
            val source = trimmedSource("""
                @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = $at)
            """)
            val site = sites(source).first()
            assertNull(caseService.expectedSignature(source, site, mixinTargets), "expected no signature for $caseName")
            assertNull(caseService.generateHandlerStub(source, site, mixinTargets), "expected no stub for $caseName")
        }
    }

    @Test
    fun modifyExpressionValueNewTargetRejectsInvalidOwnersWildcardsAndEmptyBytecodeOwner() {
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        val explicitRejections = mapOf(
            "owner spacing" to """@At(value = "NEW", target = "Lcom/example /Foo;")""",
            "owner dots" to """@At(value = "NEW", target = "Lcom.example.Foo;")""",
            "owner double slash" to """@At(value = "NEW", target = "Lcom//example/Foo;")""",
            "wildcard constructor" to """@At(value = "NEW", target = "Ljava/lang/StringBuilder;<init>*")""",
            "wildcard method name" to """@At(value = "NEW", target = "Ljava/lang/StringBuilder;length*")""",
            "trailing junk" to """@At(value = "NEW", target = "Ljava/lang/StringBuilder;junk")""",
        )
        for ((caseName, at) in explicitRejections) {
            val source = trimmedSource("""
                @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = $at)
            """)
            val site = sites(source).first()
            assertNull(service.expectedSignature(source, site, mixinTargets), caseName)
            assertNull(service.generateHandlerStub(source, site, mixinTargets), caseName)
        }

        val emptyOwnerService = newService(newCandidate(""))
        val omittedTargetSource = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "NEW"))
        """)
        val omittedSite = sites(omittedTargetSource).first()
        assertNull(
            emptyOwnerService.expectedSignature(omittedTargetSource, omittedSite, mixinTargets),
            "empty bytecode candidate owner",
        )
    }

    @Test
    fun wrapOperationNewExpectsConstructorArgsThenOperationWithoutReceiver() {
        val owner = newWrapSamplesOwner()
        val caseService = newWrapService(
            newCandidate("java/lang/StringBuilder", ordinal = 0),
            newCandidate("java/util/ArrayList", ordinal = 0),
        )
        val mixinTargets = listOf(owner)
        val source = trimmedSource("""
            @WrapOperation(method = "createObjects()V", at = @At(value = "NEW", target = "Ljava/lang/StringBuilder;"))
        """)
        val site = sites(source).first()
        val spec = caseService.expectedSignature(source, site, mixinTargets)
        assertNotNull(spec)
        assertEquals("Ljava/lang/StringBuilder;", spec.returnTypeDescriptor)
        assertEquals(1, spec.parameters.size)
        assertEquals("original", spec.parameters.single().name)
        assertTrue(spec.parameters.single().isOperation)
        assertEquals("Ljava/lang/StringBuilder;", spec.parameters.single().operationGenericDescriptor)
        assertTrue(spec.operationCallArgs.isEmpty())
        assertTrue(spec.optionalCapturedTargetParameters.isEmpty())
    }

    @Test
    fun wrapOperationNewParameterOrderPlacesConstructorArgsBeforeOperation() {
        val owner = generatedNewOwner()
        val classBytes = classBytesWithNewCalls(
            owner = owner,
            newCalls = listOf("java/lang/StringBuilder" to "(I)V"),
        )
        val caseService = newWrapService(
            newCandidate("java/lang/StringBuilder", ordinal = 0),
            mixinOwner = owner,
            methodName = "run",
            methodDescriptor = "()V",
            classBytes = classBytes,
        )
        val source = trimmedSource("""
            @WrapOperation(method = "run()V", at = @At(value = "NEW", target = "Ljava/lang/StringBuilder;<init>(I)V"))
        """)
        val site = sites(source).first()
        val spec = caseService.expectedSignature(source, site, listOf(owner))
        assertNotNull(spec)
        assertEquals(listOf("I", "Operation"), spec.parameters.map {
            if (it.isOperation) "Operation" else it.typeDescriptor
        })
        assertEquals(listOf("arg0"), spec.operationCallArgs)
    }

    @Test
    fun wrapOperationNewOrdinalSelectsMatchingConstructorSite() {
        val owner = generatedNewOwner()
        val classBytes = classBytesWithNewCalls(
            owner = owner,
            newCalls = listOf(
                "java/lang/StringBuilder" to "()V",
                "java/lang/StringBuilder" to "(I)V",
            ),
        )
        val caseService = newWrapService(
            newCandidate("java/lang/StringBuilder", ordinal = 0),
            newCandidate("java/lang/StringBuilder", ordinal = 1),
            mixinOwner = owner,
            methodName = "run",
            methodDescriptor = "()V",
            classBytes = classBytes,
        )
        val noArgSource = trimmedSource("""
            @WrapOperation(method = "run()V", at = @At(value = "NEW", target = "Ljava/lang/StringBuilder;", ordinal = 0))
        """)
        val intArgSource = trimmedSource("""
            @WrapOperation(method = "run()V", at = @At(value = "NEW", target = "Ljava/lang/StringBuilder;", ordinal = 1))
        """)
        val noArgSpec = caseService.expectedSignature(
            noArgSource,
            sites(noArgSource).first(),
            listOf(owner),
        )
        val intArgSpec = caseService.expectedSignature(
            intArgSource,
            sites(intArgSource).first(),
            listOf(owner),
        )
        assertNotNull(noArgSpec)
        assertEquals(1, noArgSpec.parameters.size)
        assertTrue(noArgSpec.parameters.single().isOperation)
        assertNotNull(intArgSpec)
        assertEquals(listOf("I", "Operation"), intArgSpec.parameters.map {
            if (it.isOperation) "Operation" else it.typeDescriptor
        })
    }

    @Test
    fun wrapOperationNewIgnoresOrdinalLikeArgsTextForFiltering() {
        val owner = generatedNewOwner()
        val classBytes = classBytesWithNewCalls(
            owner = owner,
            newCalls = listOf(
                "java/lang/StringBuilder" to "()V",
                "java/lang/StringBuilder" to "(I)V",
            ),
        )
        val caseService = newWrapService(
            newCandidate("java/lang/StringBuilder", ordinal = 0),
            newCandidate("java/lang/StringBuilder", ordinal = 1),
            mixinOwner = owner,
            methodName = "run",
            methodDescriptor = "()V",
            classBytes = classBytes,
        )
        val argsOrdinalSource = trimmedSource("""
            @WrapOperation(method = "run()V", at = @At(value = "NEW", target = "Ljava/lang/StringBuilder;", args = "ordinal=1"))
        """)
        val site = sites(argsOrdinalSource).first()
        assertNull(site.atOrdinal)
        assertEquals(listOf("ordinal=1"), site.atArgs)
        assertNull(caseService.expectedSignature(argsOrdinalSource, site, listOf(owner)))
    }

    @Test
    fun wrapOperationNewResolvesAfterInitialSuperConstructorCall() {
        val owner = generatedNewOwner()
        val classBytes = classBytesWithSuperInitThenNew(
            owner = owner,
            newOwner = "java/lang/StringBuilder",
            initDescriptor = "()V",
        )
        val caseService = newWrapService(
            newCandidate("java/lang/StringBuilder", ordinal = 0),
            mixinOwner = owner,
            methodName = "<init>",
            methodDescriptor = "()V",
            classBytes = classBytes,
        )
        val source = trimmedSource("""
            @WrapOperation(method = "<init>()V", at = @At(value = "NEW", target = "Ljava/lang/StringBuilder;"))
        """)
        val site = sites(source).first()
        val spec = caseService.expectedSignature(source, site, listOf(owner))
        assertNotNull(spec)
        assertEquals("Ljava/lang/StringBuilder;", spec.returnTypeDescriptor)
        assertEquals(1, spec.parameters.size)
        assertTrue(spec.parameters.single().isOperation)
    }

    @Test
    fun wrapOperationNewNestedSameOwnerPairsConstructorsInJvmOrder() {
        val owner = generatedNewOwner()
        val classBytes = classBytesWithNestedSameOwnerNewCalls(
            owner = owner,
            newOwner = "java/lang/StringBuilder",
            outerInit = "()V",
            innerInit = "(I)V",
        )
        val caseService = newWrapService(
            newCandidate("java/lang/StringBuilder", ordinal = 0),
            newCandidate("java/lang/StringBuilder", ordinal = 1),
            mixinOwner = owner,
            methodName = "run",
            methodDescriptor = "()V",
            classBytes = classBytes,
        )
        val outerSource = trimmedSource("""
            @WrapOperation(method = "run()V", at = @At(value = "NEW", target = "Ljava/lang/StringBuilder;", ordinal = 0))
        """)
        val innerSource = trimmedSource("""
            @WrapOperation(method = "run()V", at = @At(value = "NEW", target = "Ljava/lang/StringBuilder;", ordinal = 1))
        """)
        val explicitInnerSource = trimmedSource("""
            @WrapOperation(method = "run()V", at = @At(value = "NEW", target = "Ljava/lang/StringBuilder;<init>(I)V"))
        """)
        val outerSpec = caseService.expectedSignature(outerSource, sites(outerSource).first(), listOf(owner))
        val innerSpec = caseService.expectedSignature(innerSource, sites(innerSource).first(), listOf(owner))
        val explicitInnerSpec = caseService.expectedSignature(
            explicitInnerSource,
            sites(explicitInnerSource).first(),
            listOf(owner),
        )
        assertNotNull(outerSpec)
        assertEquals(1, outerSpec.parameters.size)
        assertTrue(outerSpec.parameters.single().isOperation)
        assertNotNull(innerSpec)
        assertEquals(listOf("I", "Operation"), innerSpec.parameters.map {
            if (it.isOperation) "Operation" else it.typeDescriptor
        })
        assertEquals(innerSpec.parameters, explicitInnerSpec?.parameters)
        assertEquals(listOf("arg0"), innerSpec.operationCallArgs)

        val ambiguousSource = trimmedSource("""
            @WrapOperation(method = "run()V", at = @At(value = "NEW", target = "Ljava/lang/StringBuilder;"))
        """)
        assertNull(caseService.expectedSignature(ambiguousSource, sites(ambiguousSource).first(), listOf(owner)))
    }

    @Test
    fun wrapOperationNewRejectsAmbiguousMalformedMissingBytesUnpairedAndMixedConstructors() {
        val owner = newWrapSamplesOwner()
        val mixinTargets = listOf(owner)
        val explicitTarget = """@At(value = "NEW", target = "Ljava/lang/StringBuilder;")"""
        val mixedConstructorsOwner = generatedNewOwner()
        val unpairedNewOwner = generatedNewOwner()
        val rejectedCases = listOf(
            "malformed target" to newWrapService(newCandidate("java/lang/StringBuilder")),
            "field target" to newWrapService(newCandidate("java/lang/StringBuilder")),
            "invoke target" to newWrapService(newCandidate("java/lang/StringBuilder")),
            "constructor non-void" to newWrapService(newCandidate("java/lang/StringBuilder")),
            "array target" to newWrapService(newCandidate("java/lang/StringBuilder")),
            "missing bytecode index" to service,
            "missing class bytes" to newWrapService(newCandidate("java/lang/StringBuilder"), provideClassBytes = false),
            "ambiguous owners" to newWrapService(
                newCandidate("java/lang/StringBuilder", ordinal = 0),
                newCandidate("java/util/ArrayList", ordinal = 0),
            ),
            "mixed constructors" to newWrapService(
                newCandidate("java/lang/StringBuilder", ordinal = 0),
                newCandidate("java/lang/StringBuilder", ordinal = 1),
                mixinOwner = mixedConstructorsOwner,
                methodName = "run",
                methodDescriptor = "()V",
                classBytes = classBytesWithNewCalls(
                    owner = mixedConstructorsOwner,
                    newCalls = listOf(
                        "java/lang/StringBuilder" to "()V",
                        "java/lang/StringBuilder" to "(I)V",
                    ),
                ),
            ),
            "unpaired new" to newWrapService(
                newCandidate("java/lang/StringBuilder", ordinal = 0),
                mixinOwner = unpairedNewOwner,
                methodName = "run",
                methodDescriptor = "()V",
                classBytes = classBytesWithUnpairedNew(unpairedNewOwner),
            ),
        )
        val atByCase = mapOf(
            "malformed target" to """@At(value = "NEW", target = "not-a-member-target")""",
            "field target" to """@At(value = "NEW", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;")""",
            "invoke target" to """@At(value = "NEW", target = "Ljava/lang/String;length()I")""",
            "constructor non-void" to """@At(value = "NEW", target = "Ljava/lang/StringBuilder;<init>(I)I")""",
            "array target" to """@At(value = "NEW", target = "[Ljava/lang/String;")""",
            "missing bytecode index" to explicitTarget,
            "missing class bytes" to explicitTarget,
            "ambiguous owners" to """@At(value = "NEW")""",
            "mixed constructors" to """@At(value = "NEW", target = "Ljava/lang/StringBuilder;")""",
            "unpaired new" to explicitTarget,
        )
        val methodByCase = mapOf(
            "mixed constructors" to "run()V",
            "unpaired new" to "run()V",
        )
        val targetsByCase = mapOf(
            "mixed constructors" to listOf(mixedConstructorsOwner),
            "unpaired new" to listOf(unpairedNewOwner),
        )
        for ((caseName, caseService) in rejectedCases) {
            val at = atByCase.getValue(caseName)
            val method = methodByCase[caseName] ?: "createObjects()V"
            val targets = targetsByCase[caseName] ?: mixinTargets
            val source = trimmedSource("""
                @WrapOperation(method = "$method", at = $at)
            """)
            val site = sites(source).first()
            assertNull(caseService.expectedSignature(source, site, targets), "expected no signature for $caseName")
            assertNull(caseService.generateHandlerStub(source, site, targets), "expected no stub for $caseName")
        }
    }

    @Test
    fun generatesWrapOperationNewHandlerStub() {
        val owner = newWrapSamplesOwner()
        val caseService = newWrapService(newCandidate("java/lang/StringBuilder", ordinal = 0))
        val source = trimmedSource("""
            @WrapOperation(method = "createObjects()V", at = @At(value = "NEW", target = "Ljava/lang/StringBuilder;"))
        """)
        val site = sites(source).first()
        val stub = caseService.generateHandlerStub(source, site, listOf(owner))
        assertNotNull(stub)
        assertTrue(stub.contains("StringBuilder mcdevHandler(Operation<StringBuilder> original)"))
        assertTrue(stub.contains("return original.call()"))
    }

    @Test
    fun validatesWrapOperationNewHandlerSignature() {
        val owner = newWrapSamplesOwner()
        val caseService = newWrapService(newCandidate("java/lang/StringBuilder", ordinal = 0))
        val source = trimmedSource("""
            @WrapOperation(method = "createObjects()V", at = @At(value = "NEW", target = "Ljava/lang/StringBuilder;"))
            private StringBuilder mcdevHandler(Operation<StringBuilder> original) {
                return original.call();
            }
        """)
        val site = sites(source).first()
        val handler = HandlerSignatureService.enrichHandlerTypes(site.handlerMethod!!, newWrapClassIndex())
        val issues = caseService.validateHandler(source, site, listOf(owner), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun modifyExpressionValueFieldTargetRejectsPutBlankMalformedMissingBytecodeUnmatchedAndAmbiguous() {
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        val fieldAt = """@At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;")"""
        val rejectedCases = listOf(
            "putfield" to fieldService(fieldCandidate(AtTargetOperationKind.FIELD_PUT_INSTANCE)),
            "putstatic" to fieldService(
                fieldCandidate(
                    name = "FLAG",
                    descriptor = "I",
                    operationKind = AtTargetOperationKind.FIELD_PUT_STATIC,
                ),
            ),
            "blank target" to fieldService(fieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE)),
            "malformed target" to fieldService(fieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE)),
            "missing bytecode" to service,
            "unmatched" to fieldService(fieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE, name = "other")),
            "ambiguous get and put" to fieldService(
                fieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE, ordinal = 0),
                fieldCandidate(AtTargetOperationKind.FIELD_PUT_INSTANCE, ordinal = 1),
            ),
        )
        val atByCase = mapOf(
            "putfield" to fieldAt,
            "putstatic" to """@At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;FLAG:I")""",
            "blank target" to """@At(value = "FIELD")""",
            "malformed target" to """@At(value = "FIELD", target = "not-a-member-target")""",
            "missing bytecode" to fieldAt,
            "unmatched" to fieldAt,
            "ambiguous get and put" to fieldAt,
        )
        for ((caseName, caseService) in rejectedCases) {
            val at = atByCase.getValue(caseName)
            val source = trimmedSource("""
                @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = $at)
            """)
            val site = sites(source).first()
            assertNull(caseService.expectedSignature(source, site, mixinTargets), "expected no signature for $caseName")
            assertNull(caseService.generateHandlerStub(source, site, mixinTargets), "expected no stub for $caseName")
        }
    }

    @Test
    fun wrapOperationConstantSelectorIsParsedFromTopLevelConstantAttribute() {
        val source = trimmedSource("""
            @WrapOperation(
                method = "draw(Ljava/lang/String;FF)V",
                constant = @Constant(classValue = java.lang.String.class),
            )
        """)
        val site = sites(source).first()
        assertTrue(site.hasConstantSelector)
        assertNull(site.atValue)
        assertNull(site.atTarget)
    }

    @Test
    fun wrapOperationConstantSelectorIgnoresConstantNestedInAtArgs() {
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
        """)
        val site = sites(source).first()
        assertEquals(MixinExtrasAnnotation.MODIFY_EXPRESSION_VALUE, site.annotation)
        assertTrue(!site.hasConstantSelector)
    }

    @Test
    fun wrapOperationConstantSelectorExpectsInstanceofHandlerSignature() {
        val source = trimmedSource("""
            @WrapOperation(
                method = "draw(Ljava/lang/String;FF)V",
                constant = @Constant(classValue = java.lang.String.class),
            )
        """)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals("Z", spec.returnTypeDescriptor)
        assertEquals(2, spec.parameters.size)
        assertEquals("Ljava/lang/Object;", spec.parameters[0].typeDescriptor)
        assertEquals("obj", spec.parameters[0].name)
        assertTrue(spec.parameters[1].isOperation)
        assertEquals("Z", spec.parameters[1].operationGenericDescriptor)
        assertEquals(listOf("obj"), spec.operationCallArgs)
        assertEquals(3, spec.optionalCapturedTargetParameters.size)
        assertEquals("Ljava/lang/String;", spec.optionalCapturedTargetParameters[0].typeDescriptor)
        assertEquals("F", spec.optionalCapturedTargetParameters[1].typeDescriptor)
        assertEquals("F", spec.optionalCapturedTargetParameters[2].typeDescriptor)
    }

    @Test
    fun validatesWrapOperationConstantSelectorHandler() {
        val source = trimmedSource("""
            @WrapOperation(
                method = "draw(Ljava/lang/String;FF)V",
                constant = @Constant(classValue = java.lang.String.class),
            )
            private boolean mcdevWrapInstanceof(Object obj, Operation<Boolean> original, String arg0) {
                return original.call(obj);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun generatesWrapOperationConstantSelectorHandlerStubStaysMinimal() {
        val source = trimmedSource("""
            @WrapOperation(
                method = "draw(Ljava/lang/String;FF)V",
                constant = @Constant(classValue = java.lang.String.class),
            )
        """)
        val site = sites(source).first()
        val stub = service.generateHandlerStub(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(stub)
        assertTrue(stub.contains("boolean mcdevHandler(Object obj, Operation<Boolean> original)"))
        assertTrue(stub.contains("return original.call(obj)"))
        assertTrue(!stub.contains("arg0"))
    }

    @Test
    fun wrapOperationWithBothAtAndConstantSelectorsReturnsNull() {
        val source = trimmedSource("""
            @WrapOperation(
                method = "draw(Ljava/lang/String;FF)V",
                at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"),
                constant = @Constant(classValue = java.lang.String.class),
            )
        """)
        val site = sites(source).first()
        assertTrue(site.hasConstantSelector)
        assertNotNull(site.atValue)
        assertNull(service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget")))
        assertNull(service.generateHandlerStub(source, site, listOf("com/example/target/SimpleTarget")))
    }

    @Test
    fun wrapOperationWithNeitherSelectorReturnsNull() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V")
        """)
        val site = sites(source).first()
        assertTrue(!site.hasConstantSelector)
        assertNull(site.atValue)
        assertNull(site.atTarget)
        assertNull(service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget")))
    }

    @Test
    fun wrapWithConditionExpectsBooleanReturnWithoutOperation() {
        val service = defaultWrapWithConditionInvokeService()
        val source = trimmedSource(MixinExtrasTestFixtures.WRAP_WITH_CONDITION_SOURCE)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals("Z", spec.returnTypeDescriptor)
        assertEquals(1, spec.parameters.size)
        assertEquals("Ljava/lang/String;", spec.parameters.first().typeDescriptor)
        assertTrue(spec.parameters.none { it.isOperation })
    }

    @Test
    fun wrapWithConditionInvokeExposesOptionalCapturedTargetParameters() {
        val service = defaultWrapWithConditionInvokeService()
        val source = trimmedSource(MixinExtrasTestFixtures.WRAP_WITH_CONDITION_SOURCE)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals(1, spec.parameters.size)
        assertEquals("instance", spec.parameters.single().name)
        assertEquals(3, spec.optionalCapturedTargetParameters.size)
        assertEquals("Ljava/lang/String;", spec.optionalCapturedTargetParameters[0].typeDescriptor)
        assertEquals("F", spec.optionalCapturedTargetParameters[1].typeDescriptor)
        assertEquals("F", spec.optionalCapturedTargetParameters[2].typeDescriptor)
    }

    @Test
    fun validatesWrapWithConditionInvokeHandlerWithPartialCapturedTargetParameterPrefix() {
        val service = defaultWrapWithConditionInvokeService()
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private boolean mcdevWrapCondition(String instance, String arg0, float arg1) {
                return true;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun validatesWrapWithConditionInvokeHandlerWithFullCapturedTargetParameterPrefix() {
        val service = defaultWrapWithConditionInvokeService()
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private boolean mcdevWrapCondition(String instance, String arg0, float arg1, float arg2) {
                return true;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun validatesWrapWithConditionInvokeHandlerWithTrailingSugarParameter() {
        val service = defaultWrapWithConditionInvokeService()
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private boolean mcdevWrapCondition(String instance, String arg0, @Local int counter) {
                return true;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        assertTrue(handler.parameters.last().isSugar)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun rejectsWrapWithConditionInvokeHandlerWithSkippedCapturedTargetParameter() {
        val service = defaultWrapWithConditionInvokeService()
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private boolean mcdevWrapCondition(String instance, float arg1) {
                return true;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun rejectsWrapWithConditionInvokeHandlerWithWrongCapturedTargetParameterType() {
        val service = defaultWrapWithConditionInvokeService()
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private boolean mcdevWrapCondition(String instance, int arg0) {
                return true;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun rejectsWrapWithConditionInvokeHandlerWithTooManyCapturedTargetParameters() {
        val service = defaultWrapWithConditionInvokeService()
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private boolean mcdevWrapCondition(String instance, String arg0, float arg1, float arg2, int extra) {
                return true;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun rejectsWrapWithConditionInvokeHandlerWithSugarBeforeCapturedArgs() {
        val service = defaultWrapWithConditionInvokeService()
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private boolean mcdevWrapCondition(String instance, @Local int counter, String arg0) {
                return true;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun rejectsWrapWithConditionHandlerWithOperationParameter() {
        val service = defaultWrapWithConditionInvokeService()
        val source = trimmedSource(MixinExtrasTestFixtures.WRAP_WITH_CONDITION_BAD_OPERATION)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun validatesCorrectWrapWithConditionHandler() {
        val service = defaultWrapWithConditionInvokeService()
        val source = trimmedSource(MixinExtrasTestFixtures.WRAP_WITH_CONDITION_SOURCE)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun wrapWithConditionInvokeVoidInstanceMethodIsValid() {
        val service = wrapWithConditionInvokeService(
            wrapWithConditionInvokeCandidate(
                operationKind = AtTargetOperationKind.INVOKE_VIRTUAL,
                owner = "java/util/ArrayList",
                name = "clear",
                descriptor = "()V",
                occurrenceResultClassification = OccurrenceResultClassification.VOID,
            ),
        )
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/util/ArrayList;clear()V"))
        """)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals("Z", spec.returnTypeDescriptor)
        assertEquals(1, spec.parameters.size)
        assertEquals("Ljava/util/ArrayList;", spec.parameters.single().typeDescriptor)
    }

    @Test
    fun wrapWithConditionInvokeVoidStaticMethodIsValid() {
        val service = wrapWithConditionInvokeService(
            wrapWithConditionInvokeCandidate(
                operationKind = AtTargetOperationKind.INVOKE_STATIC,
                owner = "java/lang/System",
                name = "gc",
                descriptor = "()V",
                occurrenceResultClassification = OccurrenceResultClassification.VOID,
            ),
        )
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/System;gc()V"))
        """)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals(0, spec.parameters.size)
    }

    @Test
    fun wrapWithConditionInvokeCategoryOnePoppedResultIsValid() {
        val service = wrapWithConditionInvokeService(
            wrapWithConditionInvokeCandidate(
                operationKind = AtTargetOperationKind.INVOKE_STATIC,
                owner = "java/lang/Math",
                name = "abs",
                descriptor = "(I)I",
                occurrenceResultClassification = OccurrenceResultClassification.IMMEDIATELY_POPPED,
            ),
        )
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/Math;abs(I)I"))
        """)
        val site = sites(source).first()
        assertNotNull(service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget")))
    }

    @Test
    fun wrapWithConditionInvokeCategoryTwoPoppedResultIsValid() {
        val service = wrapWithConditionInvokeService(
            wrapWithConditionInvokeCandidate(
                operationKind = AtTargetOperationKind.INVOKE_STATIC,
                owner = "java/lang/Math",
                name = "abs",
                descriptor = "(J)J",
                occurrenceResultClassification = OccurrenceResultClassification.IMMEDIATELY_POPPED,
            ),
        )
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/Math;abs(J)J"))
        """)
        val site = sites(source).first()
        assertNotNull(service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget")))
    }

    @Test
    fun wrapWithConditionInvokeRetainedResultIsInvalid() {
        val service = wrapWithConditionInvokeService(
            wrapWithConditionInvokeCandidate(
                operationKind = AtTargetOperationKind.INVOKE_VIRTUAL,
                occurrenceResultClassification = OccurrenceResultClassification.RETAINED,
            ),
        )
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
        """)
        val site = sites(source).first()
        assertNull(service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget")))
    }

    @Test
    fun wrapWithConditionInvokeWrongPopClassificationIsInvalid() {
        val service = wrapWithConditionInvokeService(
            wrapWithConditionInvokeCandidate(
                operationKind = AtTargetOperationKind.INVOKE_STATIC,
                owner = "java/lang/Math",
                name = "abs",
                descriptor = "(J)J",
                occurrenceResultClassification = OccurrenceResultClassification.RETAINED,
            ),
        )
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/Math;abs(J)J"))
        """)
        val site = sites(source).first()
        assertNull(service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget")))
    }

    @Test
    fun wrapWithConditionInvokeOrdinalSelectsSelectorRelativeOccurrence() {
        val popped = wrapWithConditionInvokeCandidate(
            operationKind = AtTargetOperationKind.INVOKE_VIRTUAL,
            occurrenceResultClassification = OccurrenceResultClassification.IMMEDIATELY_POPPED,
            instructionOccurrenceIndex = 1,
        )
        val retained = wrapWithConditionInvokeCandidate(
            operationKind = AtTargetOperationKind.INVOKE_VIRTUAL,
            occurrenceResultClassification = OccurrenceResultClassification.RETAINED,
            instructionOccurrenceIndex = 4,
        )
        val service = wrapWithConditionInvokeService(popped, retained)
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I", ordinal = 0))
        """)
        val site = sites(source).first()
        assertNotNull(service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget")))
        val retainedSource = source.replace("ordinal = 0", "ordinal = 1")
        val retainedSite = sites(retainedSource).first()
        assertNull(service.expectedSignature(retainedSource, retainedSite, listOf("com/example/target/SimpleTarget")))
        val omittedOrdinalSource = source.replace(", ordinal = 0", "")
        val omittedOrdinalSite = sites(omittedOrdinalSource).first()
        assertNull(service.expectedSignature(omittedOrdinalSource, omittedOrdinalSite, listOf("com/example/target/SimpleTarget")))
    }

    @Test
    fun wrapWithConditionInvokeConstructorTargetIsInvalid() {
        val service = defaultWrapWithConditionInvokeService()
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/StringBuilder;<init>()V"))
        """)
        val site = sites(source).first()
        assertNull(service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget")))
    }

    @Test
    fun wrapWithConditionInvokeMissingCandidatesOrIndexIsInvalid() {
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I", ordinal = 1))
        """)
        val site = sites(source).first()
        val missingCandidatesService = HandlerSignatureService(MixinExtrasTestFixtures.classIndex)
        assertNull(missingCandidatesService.expectedSignature(source, site, listOf("com/example/target/SimpleTarget")))
        val missingIndexService = wrapWithConditionInvokeService(
            wrapWithConditionInvokeCandidate(AtTargetOperationKind.INVOKE_VIRTUAL),
        )
        assertNull(missingIndexService.expectedSignature(source, site, listOf("com/example/target/SimpleTarget")))
    }

    @Test
    fun wrapWithConditionInvokeMultipleMixinTargetsFailWhenAnyTargetIsInvalid() {
        val ownerA = "com/example/target/SharedMixinTargetA"
        val ownerB = "com/example/target/SharedMixinTargetB"
        val drawMethod = MethodIndexEntry("draw", "(Ljava/lang/String;FF)V", false, "draw(String, float, float): void")
        val classIndex = FakeClassIndex(
            classes = listOf(
                ClassIndexEntry("SharedMixinTargetA", "com.example.target", ownerA),
                ClassIndexEntry("SharedMixinTargetB", "com.example.target", ownerB),
                ClassIndexEntry("String", "java.lang", "java/lang/String"),
            ),
            methods = mapOf(
                ownerA to listOf(drawMethod),
                ownerB to listOf(drawMethod),
                "java/lang/String" to listOf(
                    MethodIndexEntry("length", "()I", false, "length(): int"),
                ),
            ),
        )
        val validCandidate = wrapWithConditionInvokeCandidate(AtTargetOperationKind.INVOKE_VIRTUAL)
        val invalidCandidate = wrapWithConditionInvokeCandidate(
            operationKind = AtTargetOperationKind.INVOKE_VIRTUAL,
            occurrenceResultClassification = OccurrenceResultClassification.RETAINED,
        )
        val service = HandlerSignatureService(
            classIndex,
            FakeBytecodeIndex(
                candidates = mapOf(
                    "$ownerA#draw#INVOKE" to listOf(validCandidate),
                    "$ownerB#draw#INVOKE" to listOf(invalidCandidate),
                ),
            ),
        )
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
        """)
        val site = sites(source).first()
        assertNull(service.expectedSignature(source, site, listOf(ownerA, ownerB)))
    }

    @Test
    fun wrapWithConditionInvokeRequiresBytecodeIndexAndInvokeAtValue() {
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
        """)
        val site = sites(source).first()
        assertNull(service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget")))
        val wrongAtValueSource = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "intValue=0"))
        """)
        val wrongAtValueSite = sites(wrongAtValueSource).first()
        assertNull(defaultWrapWithConditionInvokeService().expectedSignature(
            wrongAtValueSource,
            wrongAtValueSite,
            listOf("com/example/target/SimpleTarget"),
        ))
    }

    @Test
    fun wrapWithConditionInvokeAtValueRejectsExactFieldTarget() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_PUT_INSTANCE))
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
        """)
        val site = sites(source).first()
        assertNull(service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget")))
    }

    @Test
    fun wrapWithConditionFieldAtValueRejectsExactMethodTarget() {
        val service = defaultWrapWithConditionInvokeService()
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Ljava/lang/String;length()I"))
        """)
        val site = sites(source).first()
        assertNull(service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget")))
    }

    @Test
    fun wrapWithConditionInvokeOrdinalRejectsLegacyNegativeInstructionOccurrenceIndex() {
        val service = wrapWithConditionInvokeService(
            wrapWithConditionInvokeCandidate(
                operationKind = AtTargetOperationKind.INVOKE_VIRTUAL,
                occurrenceResultClassification = OccurrenceResultClassification.IMMEDIATELY_POPPED,
                instructionOccurrenceIndex = -1,
            ),
        )
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I", ordinal = 0))
        """)
        val site = sites(source).first()
        assertNull(service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget")))
    }

    @Test
    fun wrapWithConditionTargetStatusVoidInvokeIsValidVoid() {
        val service = wrapWithConditionInvokeService(
            wrapWithConditionInvokeCandidate(
                operationKind = AtTargetOperationKind.INVOKE_VIRTUAL,
                owner = "java/util/ArrayList",
                name = "clear",
                descriptor = "()V",
                occurrenceResultClassification = OccurrenceResultClassification.VOID,
            ),
        )
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/util/ArrayList;clear()V"))
        """)
        val site = sites(source).first()
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        assertEquals(WrapWithConditionTargetStatus.VALID_VOID, service.wrapWithConditionTargetStatus(site, mixinTargets))
        val spec = service.expectedSignature(source, site, mixinTargets)
        assertNotNull(spec)
        assertEquals("Z", spec.returnTypeDescriptor)
        assertEquals(1, spec.parameters.size)
        assertEquals("Ljava/util/ArrayList;", spec.parameters.single().typeDescriptor)
    }

    @Test
    fun wrapWithConditionTargetStatusPoppedInvokeIsValidPoppedNonVoid() {
        val service = wrapWithConditionInvokeService(
            wrapWithConditionInvokeCandidate(
                operationKind = AtTargetOperationKind.INVOKE_STATIC,
                owner = "java/lang/Math",
                name = "abs",
                descriptor = "(I)I",
                occurrenceResultClassification = OccurrenceResultClassification.IMMEDIATELY_POPPED,
            ),
        )
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/Math;abs(I)I"))
        """)
        val site = sites(source).first()
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        assertEquals(
            WrapWithConditionTargetStatus.VALID_POPPED_NON_VOID,
            service.wrapWithConditionTargetStatus(site, mixinTargets),
        )
        assertNotNull(service.expectedSignature(source, site, mixinTargets))
    }

    @Test
    fun wrapWithConditionTargetStatusRetainedInvokeIsInvalidRetainedNonVoid() {
        val service = wrapWithConditionInvokeService(
            wrapWithConditionInvokeCandidate(
                operationKind = AtTargetOperationKind.INVOKE_VIRTUAL,
                occurrenceResultClassification = OccurrenceResultClassification.RETAINED,
            ),
        )
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
        """)
        val site = sites(source).first()
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        assertEquals(
            WrapWithConditionTargetStatus.INVALID_RETAINED_NON_VOID,
            service.wrapWithConditionTargetStatus(site, mixinTargets),
        )
        assertNull(service.expectedSignature(source, site, mixinTargets))
    }

    @Test
    fun wrapWithConditionTargetStatusFieldPutIsValidVoid() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_PUT_INSTANCE))
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
        """)
        val site = sites(source).first()
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        assertEquals(WrapWithConditionTargetStatus.VALID_VOID, service.wrapWithConditionTargetStatus(site, mixinTargets))
        val spec = service.expectedSignature(source, site, mixinTargets)
        assertNotNull(spec)
        assertEquals("Z", spec.returnTypeDescriptor)
        assertEquals(2, spec.parameters.size)
        assertEquals("Lcom/example/target/SimpleTarget;", spec.parameters[0].typeDescriptor)
        assertEquals("Ljava/lang/String;", spec.parameters[1].typeDescriptor)
    }

    @Test
    fun wrapWithConditionTargetStatusFieldGetIsInvalidInstruction() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE))
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
        """)
        val site = sites(source).first()
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        assertEquals(
            WrapWithConditionTargetStatus.INVALID_INSTRUCTION,
            service.wrapWithConditionTargetStatus(site, mixinTargets),
        )
        assertNull(service.expectedSignature(source, site, mixinTargets))
    }

    @Test
    fun wrapWithConditionTargetStatusFieldMixedGetPutWithoutOrdinalIsInvalidInstruction() {
        val service = fieldService(
            fieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE, instructionOccurrenceIndex = 0),
            fieldCandidate(AtTargetOperationKind.FIELD_PUT_INSTANCE, instructionOccurrenceIndex = 1),
        )
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
        """)
        val site = sites(source).first()
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        assertEquals(
            WrapWithConditionTargetStatus.INVALID_INSTRUCTION,
            service.wrapWithConditionTargetStatus(site, mixinTargets),
        )
        assertNull(service.expectedSignature(source, site, mixinTargets))
    }

    @Test
    fun wrapWithConditionTargetStatusFieldOrdinalSelectsPutIsValidVoid() {
        val service = fieldService(
            fieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE, instructionOccurrenceIndex = 0),
            fieldCandidate(AtTargetOperationKind.FIELD_PUT_INSTANCE, instructionOccurrenceIndex = 1),
        )
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;", ordinal = 1))
        """)
        val site = sites(source).first()
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        assertEquals(
            WrapWithConditionTargetStatus.VALID_VOID,
            service.wrapWithConditionTargetStatus(site, mixinTargets),
        )
        val spec = service.expectedSignature(source, site, mixinTargets)
        assertNotNull(spec)
        assertEquals("Z", spec.returnTypeDescriptor)
        assertEquals(2, spec.parameters.size)
        assertEquals("Lcom/example/target/SimpleTarget;", spec.parameters[0].typeDescriptor)
        assertEquals("Ljava/lang/String;", spec.parameters[1].typeDescriptor)
    }

    @Test
    fun wrapWithConditionTargetStatusFieldOrdinalSelectsGetIsInvalidInstruction() {
        val service = fieldService(
            fieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE, instructionOccurrenceIndex = 0),
            fieldCandidate(AtTargetOperationKind.FIELD_PUT_INSTANCE, instructionOccurrenceIndex = 1),
        )
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;", ordinal = 0))
        """)
        val site = sites(source).first()
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        assertEquals(
            WrapWithConditionTargetStatus.INVALID_INSTRUCTION,
            service.wrapWithConditionTargetStatus(site, mixinTargets),
        )
        assertNull(service.expectedSignature(source, site, mixinTargets))
    }

    @Test
    fun wrapWithConditionTargetStatusFieldMissingOccurrenceIndexWithOrdinalIsUnresolved() {
        val service = fieldService(
            fieldCandidate(
                AtTargetOperationKind.FIELD_PUT_INSTANCE,
                instructionOccurrenceIndex = -1,
            ),
        )
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;", ordinal = 0))
        """)
        val site = sites(source).first()
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        assertEquals(
            WrapWithConditionTargetStatus.UNRESOLVED,
            service.wrapWithConditionTargetStatus(site, mixinTargets),
        )
        assertNull(service.expectedSignature(source, site, mixinTargets))
    }

    @Test
    fun wrapWithConditionTargetStatusUnsupportedAtValueIsInvalidInstruction() {
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "intValue=0"))
        """)
        val site = sites(source).first()
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        assertEquals(
            WrapWithConditionTargetStatus.INVALID_INSTRUCTION,
            defaultWrapWithConditionInvokeService().wrapWithConditionTargetStatus(site, mixinTargets),
        )
        assertNull(defaultWrapWithConditionInvokeService().expectedSignature(source, site, mixinTargets))
    }

    @Test
    fun wrapWithConditionTargetStatusMissingBytecodeIsUnresolved() {
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
        """)
        val site = sites(source).first()
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        assertEquals(
            WrapWithConditionTargetStatus.UNRESOLVED,
            HandlerSignatureService(MixinExtrasTestFixtures.classIndex).wrapWithConditionTargetStatus(site, mixinTargets),
        )
        assertNull(HandlerSignatureService(MixinExtrasTestFixtures.classIndex).expectedSignature(source, site, mixinTargets))
    }

    @Test
    fun wrapWithConditionTargetStatusMixedOccurrencesIsInvalidRetainedNonVoid() {
        val popped = wrapWithConditionInvokeCandidate(
            operationKind = AtTargetOperationKind.INVOKE_VIRTUAL,
            occurrenceResultClassification = OccurrenceResultClassification.IMMEDIATELY_POPPED,
            instructionOccurrenceIndex = 1,
        )
        val retained = wrapWithConditionInvokeCandidate(
            operationKind = AtTargetOperationKind.INVOKE_VIRTUAL,
            occurrenceResultClassification = OccurrenceResultClassification.RETAINED,
            instructionOccurrenceIndex = 4,
        )
        val service = wrapWithConditionInvokeService(popped, retained)
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
        """)
        val site = sites(source).first()
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        assertEquals(
            WrapWithConditionTargetStatus.INVALID_RETAINED_NON_VOID,
            service.wrapWithConditionTargetStatus(site, mixinTargets),
        )
        assertNull(service.expectedSignature(source, site, mixinTargets))
    }

    @Test
    fun wrapMethodIncludesTargetParametersAndOperation() {
        val source = trimmedSource(MixinExtrasTestFixtures.WRAP_METHOD_SOURCE)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals(4, spec.parameters.size)
        assertEquals("Ljava/lang/String;", spec.parameters[0].typeDescriptor)
        assertEquals("F", spec.parameters[1].typeDescriptor)
        assertTrue(spec.parameters.last().isOperation)
        assertEquals("V", spec.returnTypeDescriptor)
        assertEquals("V", spec.parameters.last().operationGenericDescriptor)
        assertEquals(listOf("arg0", "arg1", "arg2"), spec.operationCallArgs)
    }

    @Test
    fun wrapMethodInstanceTargetOmitsReceiver() {
        val source = trimmedSource(MixinExtrasTestFixtures.WRAP_METHOD_SOURCE)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertTrue(spec.parameters.none { it.typeDescriptor == "Lcom/example/target/SimpleTarget;" })
    }

    @Test
    fun wrapMethodStaticTargetOmitsReceiver() {
        val source = trimmedSource("""
            @WrapMethod(method = "noop()V")
            private void mcdevWrapNoop(Operation<Void> original) {
                original.call();
            }
        """)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals(1, spec.parameters.size)
        assertTrue(spec.parameters.single().isOperation)
        assertEquals(emptyList<String>(), spec.operationCallArgs)
        val stub = service.generateHandlerStub(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(stub)
        assertTrue(stub.contains("static void mcdevHandler(Operation<Void> original)"))
    }

    @Test
    fun wrapMethodGenerationIsUnavailableWhenMatchingTargetsDisagreeOnStaticness() {
        val staticOwner = "example/StaticOwner"
        val instanceOwner = "example/InstanceOwner"
        val classIndex = FakeClassIndex(
            classes = listOf(
                ClassIndexEntry("StaticOwner", "example", staticOwner),
                ClassIndexEntry("InstanceOwner", "example", instanceOwner),
            ),
            methods = mapOf(
                staticOwner to listOf(MethodIndexEntry("run", "()V", true, "run(): void")),
                instanceOwner to listOf(MethodIndexEntry("run", "()V", false, "run(): void")),
            ),
        )
        val mixedService = HandlerSignatureService(classIndex)
        val source = "@WrapMethod(method = \"run()V\")"
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val targets = listOf(staticOwner, instanceOwner)
        assertNull(mixedService.expectedSignature(source, site, targets))
        assertNull(mixedService.generateHandlerStub(source, site, targets))
    }

    @Test
    fun wrapMethodHandlerStaticnessMustMatchTarget() {
        data class StaticCase(
            val name: String,
            val source: String,
            val handlerStatic: Boolean,
            val expectsMismatch: Boolean,
        )
        val cases = listOf(
            StaticCase(
                "static target with instance handler",
                """
                    @WrapMethod(method = "noop()V")
                    private void mcdevWrapNoop(Operation<Void> original) {
                        original.call();
                    }
                """,
                handlerStatic = false,
                expectsMismatch = true,
            ),
            StaticCase(
                "instance target with static handler",
                """
                    @WrapMethod(method = "draw(Ljava/lang/String;FF)V")
                    private static void mcdevWrapDraw(String arg0, float arg1, float arg2, Operation<Void> original) {
                        original.call(arg0, arg1, arg2);
                    }
                """,
                handlerStatic = true,
                expectsMismatch = true,
            ),
            StaticCase(
                "static target with static handler",
                """
                    @WrapMethod(method = "noop()V")
                    private static void mcdevWrapNoop(Operation<Void> original) {
                        original.call();
                    }
                """,
                handlerStatic = true,
                expectsMismatch = false,
            ),
            StaticCase(
                "instance target with instance handler",
                MixinExtrasTestFixtures.WRAP_METHOD_SOURCE,
                handlerStatic = false,
                expectsMismatch = false,
            ),
        )
        for (case in cases) {
            val source = trimmedSource(case.source)
            val site = sites(source).single()
            val handler = enrich(site.handlerMethod!!)
            assertEquals(case.handlerStatic, handler.isStatic, case.name)
            val issues = service.validateHandler(
                source,
                site,
                listOf("com/example/target/SimpleTarget"),
                handler,
            )
            if (!case.expectsMismatch) {
                assertTrue(issues.isEmpty(), case.name)
            } else {
                val mismatch = issues.singleOrNull {
                    it.code == MixinExtrasDiagnosticCodes.WRAP_METHOD_STATIC_MISMATCH
                }
                assertNotNull(mismatch, case.name)
                assertTrue(mismatch.message.contains(if (site.methodAttribute == "noop()V") "static" else "instance"), case.name)
            }
        }
    }

    @Test
    fun wrapMethodRejectsInitializersAndMalformedTargetDescriptors() {
        val targetOwner = "com/example/target/SimpleTarget"
        val initializerIndex = FakeClassIndex(
            classes = classIndex.findClasses("", 100_000),
            methods = mapOf(
                targetOwner to listOf(
                    MethodIndexEntry("<init>", "()V", false, "<init>(): void"),
                    MethodIndexEntry("<clinit>", "()V", true, "<clinit>(): void"),
                    MethodIndexEntry("broken", "not-a-descriptor", false, "broken"),
                ),
            ),
        )
        val initializerService = HandlerSignatureService(initializerIndex)
        for (name in listOf("<init>", "<clinit>")) {
            val source = trimmedSource("""
                @WrapMethod(method = "$name()V")
                private${if (name == "<clinit>") " static" else ""} void mcdev${name.removePrefix("<").removeSuffix(">")}(
                    Operation<Void> original
                ) {
                    original.call();
                }
            """)
            val site = sites(source).single()
            assertNull(initializerService.expectedSignature(source, site, listOf(targetOwner)))
            assertNull(initializerService.generateHandlerStub(source, site, listOf(targetOwner)))
            val handler = HandlerSignatureService.enrichHandlerTypes(site.handlerMethod!!, initializerIndex)
            val issues = initializerService.validateHandler(source, site, listOf(targetOwner), handler)
            assertTrue(
                issues.any { it.code == MixinExtrasDiagnosticCodes.WRAP_METHOD_INVALID_TARGET },
                name,
            )
        }

        val malformedSource = trimmedSource("""
            @WrapMethod(method = "broken")
        """)
        val malformedSite = sites(malformedSource).single()
        assertNull(initializerService.expectedSignature(malformedSource, malformedSite, listOf(targetOwner)))
        assertNull(initializerService.generateHandlerStub(malformedSource, malformedSite, listOf(targetOwner)))
    }

    @Test
    fun rejectsWrapMethodHandlerWithInstanceReceiver() {
        val source = trimmedSource(MixinExtrasTestFixtures.WRAP_METHOD_BAD_RECEIVER)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun validatesCorrectWrapMethodHandler() {
        val source = trimmedSource(MixinExtrasTestFixtures.WRAP_METHOD_SOURCE)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun wrapMethodValidatesSupportedSugarParameters() {
        for ((name, sourceText, expectedAnnotations) in listOf(
            Triple(
                "rejects Local",
                """
                    @WrapMethod(method = "draw(Ljava/lang/String;FF)V")
                    private void mcdevWrapDraw(String arg0, float arg1, float arg2, Operation<Void> original, @Local int counter) {
                        original.call(arg0, arg1, arg2);
                    }
                """,
                listOf("@Local"),
            ),
            Triple(
                "rejects Cancellable",
                """
                    @WrapMethod(method = "draw(Ljava/lang/String;FF)V")
                    private void mcdevWrapDraw(String arg0, float arg1, float arg2, Operation<Void> original, @Cancellable CallbackInfo ci) {
                        original.call(arg0, arg1, arg2);
                    }
                """,
                listOf("@Cancellable"),
            ),
            Triple(
                "rejects Local when target is unresolved",
                """
                    @WrapMethod(method = "nonexistent()V")
                    private void mcdevWrap(Operation<Void> original, @Local int counter) {
                        original.call();
                    }
                """,
                listOf("@Local"),
            ),
            Triple(
                "allows Share with valid ref type",
                """
                    @WrapMethod(method = "draw(Ljava/lang/String;FF)V")
                    private void mcdevWrapDraw(String arg0, float arg1, float arg2, Operation<Void> original, @Share LocalIntRef shared) {
                        original.call(arg0, arg1, arg2);
                    }
                """,
                emptyList(),
            ),
        )) {
            val source = trimmedSource(sourceText)
            val site = sites(source).first()
            val handler = enrich(site.handlerMethod!!)
            val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
            if (expectedAnnotations.isEmpty()) {
                assertTrue(issues.isEmpty(), name)
            } else {
                val unsupported = issues.filter { it.code == MixinExtrasDiagnosticCodes.UNSUPPORTED_SUGAR_PARAMETER }
                assertEquals(expectedAnnotations.size, unsupported.size, "$name: unsupported sugar issue count")
                expectedAnnotations.zip(unsupported).forEach { (expectedAnnotation, issue) ->
                    assertEquals(expectedAnnotation, sourceSubstring(source, issue.range), name)
                }
            }
        }
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
        val spec = service.expectedSignature(source, site, listOf("net/minecraft/client/MinecraftClient"))
        assertNotNull(spec)
        assertEquals(6, spec.parameters.size)
        assertEquals(listOf("textRenderer", "arg0", "arg1", "arg2", "arg3"), spec.operationCallArgs)
    }

    @Test
    fun validatesCorrectWrapOperationHandler() {
        val source = trimmedSource(MixinExtrasTestFixtures.WRAP_OPERATION_SOURCE)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun validatesWrapOperationHandlerWithTrailingLocalParameter() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original, @Local int counter) {
                return original.call(instance);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        assertTrue(handler.parameters.last().isSugar)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun validatesWrapOperationHandlerWithTrailingLocalWithArguments() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original, @Local(ordinal = 0, argsOnly = true) int counter) {
                return original.call(instance);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        assertEquals(3, handler.parameters.size)
        assertTrue(handler.parameters.last().isSugar)
        assertEquals("counter", handler.parameters.last().name)
        assertEquals("int", handler.parameters.last().typeName)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun validatesModifyExpressionValueHandlerWithTrailingShareParameter() {
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
            private float mcdevModifyX(float original, @Share LocalIntRef shared) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        assertTrue(handler.parameters.last().isSugar)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun validatesModifyExpressionValueHandlerWithTrailingShareWithArgument() {
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
            private float mcdevModifyX(float original, @Share("myRef") LocalIntRef shared) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        assertEquals(2, handler.parameters.size)
        assertTrue(handler.parameters.last().isSugar)
        assertEquals("shared", handler.parameters.last().name)
        assertEquals("LocalIntRef", handler.parameters.last().typeName)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun shareParameterWithLocalRefTypeProducesNoDiagnostic() {
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
            private float mcdevModifyX(float original, @Share LocalRef shared) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        assertEquals(
            "Lcom/llamalad7/mixinextras/sugar/ref/LocalRef;",
            handler.parameters.last().typeDescriptor,
        )
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.none { it.code == MixinExtrasDiagnosticCodes.INVALID_SHARE_PARAMETER_TYPE })
    }

    @Test
    fun shareParameterWithNestedLocalRefTypeProducesInvalidShareParameterTypeDiagnostic() {
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
            private float mcdevModifyX(float original, @Share NestedLocalRef shared) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        assertEquals(
            "Lcom/llamalad7/mixinextras/sugar/ref/nested/NestedLocalRef;",
            handler.parameters.last().typeDescriptor,
        )
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertEquals(MixinExtrasDiagnosticCodes.INVALID_SHARE_PARAMETER_TYPE, issues.single().code)
    }

    @Test
    fun shareParameterWithLookalikeLocalRefTypeProducesInvalidShareParameterTypeDiagnostic() {
        val lookalikeClassIndex = FakeClassIndex(
            classes = listOf(
                ClassIndexEntry("SimpleTarget", "com.example.target", "com/example/target/SimpleTarget"),
                ClassIndexEntry("String", "java.lang", "java/lang/String"),
                ClassIndexEntry("LocalRef", "com.llamalad7.mixinextras.sugar.ref", "com/llamalad7/mixinextras/sugar/ref/LocalRef"),
                ClassIndexEntry("LocalRef", "com.example.lookalike", "com/example/lookalike/LocalRef"),
            ),
            methods = mapOf(
                "com/example/target/SimpleTarget" to listOf(
                    MethodIndexEntry("draw", "(Ljava/lang/String;FF)V", false, "draw(String, float, float): void"),
                ),
            ),
        )
        val lookalikeService = HandlerSignatureService(lookalikeClassIndex)
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
            private float mcdevModifyX(float original, @Share com.example.lookalike.LocalRef shared) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = HandlerSignatureService.enrichHandlerTypes(site.handlerMethod!!, lookalikeClassIndex)
        assertEquals(
            "Lcom/example/lookalike/LocalRef;",
            handler.parameters.last().typeDescriptor,
        )
        val issues = lookalikeService.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertEquals(MixinExtrasDiagnosticCodes.INVALID_SHARE_PARAMETER_TYPE, issues.single().code)
    }

    @Test
    fun cancellableParameterOnVoidTargetWithCallbackInfoProducesNoDiagnostic() {
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
            private float mcdevModifyX(float original, @Cancellable CallbackInfo ci) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        assertEquals(CALLBACK_INFO_DESCRIPTOR, handler.parameters.last().typeDescriptor)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.none { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun cancellableParameterOnVoidTargetWithWrongTypeProducesHandlerSignatureMismatchOnAnnotation() {
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
            private float mcdevModifyX(float original, @Cancellable CallbackInfoReturnable cir) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        val mismatch = issues.single { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH }
        assertEquals("@Cancellable", sourceSubstring(source, mismatch.range))
    }

    @Test
    fun cancellableParameterOnNonVoidTargetWithCallbackInfoReturnableProducesNoDiagnostic() {
        val source = trimmedSource("""
            @ModifyReturnValue(method = "compute()I", at = @At("RETURN"))
            private int mcdevModifyReturn(int original, @Cancellable CallbackInfoReturnable cir) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        assertEquals(CALLBACK_INFO_RETURNABLE_DESCRIPTOR, handler.parameters.last().typeDescriptor)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.none { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun cancellableParameterOnNonVoidTargetWithWrongTypeProducesHandlerSignatureMismatchOnAnnotation() {
        val source = trimmedSource("""
            @ModifyReturnValue(method = "compute()I", at = @At("RETURN"))
            private int mcdevModifyReturn(int original, @Cancellable CallbackInfo ci) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        val mismatch = issues.single { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH }
        assertEquals("@Cancellable", sourceSubstring(source, mismatch.range))
    }

    @Test
    fun shareParameterWithOrdinaryTypeProducesInvalidShareParameterTypeDiagnostic() {
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
            private float mcdevModifyX(float original, @Share String shared) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertEquals(MixinExtrasDiagnosticCodes.INVALID_SHARE_PARAMETER_TYPE, issues.single().code)
    }

    @Test
    fun shareParameterWithFullyQualifiedShareAnnotationAcceptsLocalRef() {
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
            private float mcdevModifyX(float original, @com.llamalad7.mixinextras.sugar.Share LocalRef shared) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        assertTrue(handler.parameters.last().sugarSpec is HandlerParameterSugarSpec.Share)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.none { it.code == MixinExtrasDiagnosticCodes.INVALID_SHARE_PARAMETER_TYPE })
    }

    @Test
    fun shareParameterWithUnresolvedTypeProducesNoDiagnostic() {
        val source = trimmedSource("""
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
            private float mcdevModifyX(float original, @Share UnknownRefType shared) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        assertNull(handler.parameters.last().typeDescriptor)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.none { it.code == MixinExtrasDiagnosticCodes.INVALID_SHARE_PARAMETER_TYPE })
    }

    @Test
    fun rejectsHandlerWithSugarPrecedingRequiredBaseParameter() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(@Local(ordinal = 0) int counter, String instance, Operation<Integer> original) {
                return original.call(instance);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        assertTrue(handler.parameters.first().isSugar)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun parseHandlerParameterLocalSpecParsesNamedAttributesInArbitraryOrder() {
        val param = parseHandlerParameter(
            "@Local(name = {\"alpha\", \"beta\"}, argsOnly = true, index = 2, ordinal = 1) int counter",
        )
        assertTrue(param.isSugar)
        val spec = param.sugarSpec as HandlerParameterSugarSpec.Local
        assertTrue(spec.argsOnly)
        assertEquals(2, spec.index)
        assertEquals(1, spec.ordinal)
        assertEquals(setOf("alpha", "beta"), spec.names)
        assertEquals("int", param.typeName)
        assertEquals("counter", param.name)
    }

    @Test
    fun parseHandlerParameterLocalSpecTreatsNegativeOneIndexAndOrdinalAsUnset() {
        val param = parseHandlerParameter("@Local(index = -1, ordinal = -1) int counter")
        val spec = param.sugarSpec as HandlerParameterSugarSpec.Local
        assertNull(spec.index)
        assertNull(spec.ordinal)
    }

    @Test
    fun parseHandlerParameterLocalSpecParsesSingleNameAttribute() {
        val param = parseHandlerParameter("""@Local(name = "slot") int counter""")
        val spec = param.sugarSpec as HandlerParameterSugarSpec.Local
        assertEquals(setOf("slot"), spec.names)
    }

    @Test
    fun parseHandlerParameterLocalSpecParsesPrintTrue() {
        val param = parseHandlerParameter("@Local(print = true) int counter")
        val spec = param.sugarSpec as HandlerParameterSugarSpec.Local
        assertTrue(spec.print)
    }

    @Test
    fun parseHandlerParameterLocalSpecParsesSimpleTypeClassLiteral() {
        val param = parseHandlerParameter("@Local(type = String.class) Object value")
        val spec = param.sugarSpec as HandlerParameterSugarSpec.Local
        assertEquals("String", spec.typeClassName)
    }

    @Test
    fun parseHandlerParameterLocalSpecParsesFullyQualifiedTypeClassLiteral() {
        val param = parseHandlerParameter("@Local(type = java.lang.String.class) Object value")
        val spec = param.sugarSpec as HandlerParameterSugarSpec.Local
        assertEquals("java.lang.String", spec.typeClassName)
    }

    @Test
    fun parseHandlerParameterLocalSpecParsesPrimitiveTypeClassLiteral() {
        val param = parseHandlerParameter("@Local(type = int.class) Object value")
        val spec = param.sugarSpec as HandlerParameterSugarSpec.Local
        assertEquals("int", spec.typeClassName)
    }

    @Test
    fun parseHandlerParameterLocalSpecMalformedPrintAndTypeFailClosedToDefaults() {
        val param = parseHandlerParameter("@Local(print = broken, type = notAClass, ordinal = broken) int counter")
        val spec = param.sugarSpec as HandlerParameterSugarSpec.Local
        assertFalse(spec.print)
        assertNull(spec.typeClassName)
        assertNull(spec.ordinal)
    }

    @Test
    fun parseHandlerParameterShareSpecParsesShorthandString() {
        val param = parseHandlerParameter("""@Share("myRef") LocalIntRef shared""")
        assertTrue(param.isSugar)
        val spec = param.sugarSpec as HandlerParameterSugarSpec.Share
        assertEquals("myRef", spec.value)
        assertNull(spec.namespace)
        assertEquals("LocalIntRef", param.typeName)
        assertEquals("shared", param.name)
    }

    @Test
    fun parseHandlerParameterShareSpecParsesNamedValueAndNamespace() {
        val param = parseHandlerParameter(
            """@Share(namespace = "ns", value = "myRef") LocalIntRef shared""",
        )
        val spec = param.sugarSpec as HandlerParameterSugarSpec.Share
        assertEquals("myRef", spec.value)
        assertEquals("ns", spec.namespace)
    }

    @Test
    fun parseShareNamespacePresenceAndRefValueDescriptor() {
        val omitted = parseHandlerParameter("""@Share(value = "id") LocalRef<String> shared""")
        val explicitEmpty = parseHandlerParameter("""@Share(value = "id", namespace = "") LocalIntRef shared""")
        val malformed = parseHandlerParameter("""@Share(value = "id", namespace = SOME_CONST) LocalIntRef shared""")
        val omittedSpec = omitted.sugarSpec as HandlerParameterSugarSpec.Share
        val explicitSpec = explicitEmpty.sugarSpec as HandlerParameterSugarSpec.Share
        val malformedSpec = malformed.sugarSpec as HandlerParameterSugarSpec.Share

        assertFalse(omittedSpec.namespaceSpecified)
        assertTrue(explicitSpec.namespaceSpecified)
        assertEquals("", explicitSpec.namespace)
        assertTrue(malformedSpec.namespaceSpecified)
        assertNull(malformedSpec.namespace)
        assertEquals("Ljava/lang/Object;", HandlerSignatureService.shareRefValueDescriptor(
            "Lcom/llamalad7/mixinextras/sugar/ref/LocalRef;",
        ))
        assertEquals("I", HandlerSignatureService.shareRefValueDescriptor(
            "Lcom/llamalad7/mixinextras/sugar/ref/LocalIntRef;",
        ))
    }

    @Test
    fun parseHandlerParameterShareSpecIgnoresUnknownIdAttribute() {
        val param = parseHandlerParameter("""@Share(id = "aliasRef") LocalIntRef shared""")
        val spec = param.sugarSpec as HandlerParameterSugarSpec.Share
        assertNull(spec.value)
        assertNull(spec.namespace)
    }

    @Test
    fun parseHandlerParameterFullyQualifiedLocalAnnotation() {
        val param = parseHandlerParameter(
            "@com.llamalad7.mixinextras.sugar.Local(ordinal = 0) int counter",
        )
        assertTrue(param.isSugar)
        val spec = param.sugarSpec as HandlerParameterSugarSpec.Local
        assertEquals(0, spec.ordinal)
        assertEquals("int", param.typeName)
        assertEquals("counter", param.name)
    }

    @Test
    fun parseHandlerParameterWithoutSugarHasNullSugarSpec() {
        val param = parseHandlerParameter("String arg0")
        assertFalse(param.isSugar)
        assertNull(param.sugarSpec)
        assertEquals("String", param.typeName)
        assertEquals("arg0", param.name)
    }

    @Test
    fun parseHandlerParameterCancellableMarkerSetsSugarSpec() {
        val param = parseHandlerParameter("@Cancellable boolean cancel")
        assertTrue(param.isSugar)
        assertEquals(HandlerParameterSugarSpec.Cancellable, param.sugarSpec)
        assertEquals("boolean", param.typeName)
        assertEquals("cancel", param.name)
    }

    @Test
    fun parseHandlerParameterRecognizesOnlyOfficialSugarAnnotations() {
        data class SugarCase(
            val simpleName: String,
            val officialFqn: String,
            val expected: HandlerParameterSugarSpec,
        )
        val cases = listOf(
            SugarCase(
                "Local",
                "com.llamalad7.mixinextras.sugar.Local",
                HandlerParameterSugarSpec.Local(),
            ),
            SugarCase(
                "Share",
                "com.llamalad7.mixinextras.sugar.Share",
                HandlerParameterSugarSpec.Share(),
            ),
            SugarCase(
                "Cancellable",
                "com.llamalad7.mixinextras.sugar.Cancellable",
                HandlerParameterSugarSpec.Cancellable,
            ),
        )

        cases.forEach { case ->
            val official = parseHandlerParameter("@${case.officialFqn} int value")
            assertEquals(case.expected, official.sugarSpec, "official FQN ${case.officialFqn}")

            val importedSource =
                "import ${case.officialFqn};\nvoid handler(@${case.simpleName} int value) {}"
            val importedStart = importedSource.indexOf("void handler")
            val imported = HandlerSignatureService.parseHandlerMethod(importedSource, importedStart)!!
                .parameters.single()
            assertEquals(case.expected, imported.sugarSpec, "official import ${case.officialFqn}")

            val wrongFqn = parseHandlerParameter("@com.example.${case.simpleName} int value")
            assertFalse(wrongFqn.isSugar, "wrong FQN ${case.simpleName}")
            assertNull(wrongFqn.sugarSpec, "wrong FQN ${case.simpleName}")

            val wrongImportSource =
                "import com.example.${case.simpleName};\nvoid handler(@${case.simpleName} int value) {}"
            val wrongImportStart = wrongImportSource.indexOf("void handler")
            val wrongImport = HandlerSignatureService.parseHandlerMethod(wrongImportSource, wrongImportStart)!!
                .parameters.single()
            assertFalse(wrongImport.isSugar, "wrong import ${case.simpleName}")
            assertNull(wrongImport.sugarSpec, "wrong import ${case.simpleName}")
        }
    }

    @Test
    fun parseHandlerParameterMalformedLocalArgsFailClosedWithoutCrash() {
        val param = parseHandlerParameter("@Local(ordinal = broken) int counter")
        assertTrue(param.isSugar)
        val spec = param.sugarSpec as HandlerParameterSugarSpec.Local
        assertNull(spec.ordinal)
        assertFalse(spec.print)
        assertNull(spec.typeClassName)
        assertEquals("int", param.typeName)
        assertEquals("counter", param.name)
    }

    @Test
    fun parseHandlerParameterExistingIsSugarRemainsCompatibleForBareLocal() {
        val param = parseHandlerParameter("@Local int counter")
        assertTrue(param.isSugar)
        assertTrue(param.sugarSpec is HandlerParameterSugarSpec.Local)
    }

    @Test
    fun parseHandlerParameterRangesTwoLocalParameters() {
        val source = "void handler(String arg, @Local int a, @Local(ordinal = 0) int b) {}"
        val handler = HandlerSignatureService.parseHandlerMethod(source, 0)!!
        assertEquals(3, handler.parameters.size)

        val arg = handler.parameters[0]
        assertNull(arg.sugarAnnotationRange)
        assertEquals("String arg", sourceSubstring(source, arg.range!!))

        val firstLocal = handler.parameters[1]
        assertEquals("@Local", sourceSubstring(source, firstLocal.sugarAnnotationRange!!))
        assertEquals("@Local int a", sourceSubstring(source, firstLocal.range!!))

        val secondLocal = handler.parameters[2]
        assertEquals("@Local(ordinal = 0)", sourceSubstring(source, secondLocal.sugarAnnotationRange!!))
        assertEquals("@Local(ordinal = 0) int b", sourceSubstring(source, secondLocal.range!!))
    }

    @Test
    fun parseHandlerParameterRangesQualifiedLocalAnnotation() {
        val source =
            "void handler(@com.llamalad7.mixinextras.sugar.Local(ordinal = 1) int counter) {}"
        val param = HandlerSignatureService.parseHandlerMethod(source, 0)!!.parameters.single()
        assertEquals(
            "@com.llamalad7.mixinextras.sugar.Local(ordinal = 1)",
            sourceSubstring(source, param.sugarAnnotationRange!!),
        )
        assertEquals(
            "@com.llamalad7.mixinextras.sugar.Local(ordinal = 1) int counter",
            sourceSubstring(source, param.range!!),
        )
    }

    @Test
    fun parseHandlerParameterRangesLocalNameArrayArgs() {
        val source =
            """void handler(@Local(name = {"alpha", "beta"}, argsOnly = true) int counter) {}"""
        val param = HandlerSignatureService.parseHandlerMethod(source, 0)!!.parameters.single()
        assertEquals(
            """@Local(name = {"alpha", "beta"}, argsOnly = true)""",
            sourceSubstring(source, param.sugarAnnotationRange!!),
        )
        assertEquals(
            """@Local(name = {"alpha", "beta"}, argsOnly = true) int counter""",
            sourceSubstring(source, param.range!!),
        )
    }

    @Test
    fun detectsWrongReturnType() {
        val source = trimmedSource(MixinExtrasTestFixtures.WRAP_OPERATION_BAD_RETURN)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE })
    }

    @Test
    fun detectsMissingOperationParameter() {
        val source = trimmedSource(MixinExtrasTestFixtures.WRAP_OPERATION_MISSING_OP)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.MISSING_OPERATION_PARAMETER })
    }

    @Test
    fun detectsOperationNotLastParameter() {
        val source = trimmedSource(MixinExtrasTestFixtures.WRAP_OPERATION_OP_NOT_LAST)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
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
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.WRONG_ORIGINAL_VALUE_TYPE })
    }

    @Test
    fun generatesWrapOperationHandlerStub() {
        val source = trimmedSource(MixinExtrasTestFixtures.WRAP_OPERATION_NO_HANDLER)
        val site = sites(source).first()
        val stub = service.generateHandlerStub(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(stub)
        assertTrue(stub.contains("Operation<Integer>"))
        assertTrue(stub.contains("original.call(instance)"))
    }

    @Test
    fun generatesModifyExpressionValueHandlerStub() {
        val source = trimmedSource(
            MixinExtrasTestFixtures.WRAP_OPERATION_NO_HANDLER.replace("WrapOperation", "ModifyExpressionValue")
                .replace(
                    """at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I")""",
                    """at = @At(value = "CONSTANT", args = "floatValue=0.0")""",
                ),
        )
        val site = sites(source).first()
        val stub = service.generateHandlerStub(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(stub)
        assertTrue(stub.contains("float original"))
        assertTrue(stub.contains("return original"))
    }

    @Test
    fun generatesModifyReturnValueHandlerStub() {
        val source = trimmedSource("""
            @ModifyReturnValue(method = "compute()I", at = @At("RETURN"))
        """)
        val site = sites(source).first()
        val stub = service.generateHandlerStub(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(stub)
        assertTrue(stub.contains("int mcdevHandler(int original)"))
        assertTrue(stub.contains("return original"))
    }

    @Test
    fun modifyReturnValueExposesOptionalCapturedTargetParameters() {
        val source = trimmedSource("""
            @ModifyReturnValue(method = "draw(Ljava/lang/String;FFI)I", at = @At("RETURN"))
        """)
        val site = sites(source).first()
        val targets = listOf("net/minecraft/client/font/TextRenderer")
        val spec = service.expectedSignature(source, site, targets)
        assertNotNull(spec)
        assertEquals(1, spec.parameters.size)
        assertEquals("original", spec.parameters.single().name)
        assertEquals("I", spec.parameters.single().typeDescriptor)
        assertEquals(4, spec.optionalCapturedTargetParameters.size)
        assertEquals("Ljava/lang/String;", spec.optionalCapturedTargetParameters[0].typeDescriptor)
        assertEquals("F", spec.optionalCapturedTargetParameters[1].typeDescriptor)
        assertEquals("F", spec.optionalCapturedTargetParameters[2].typeDescriptor)
        assertEquals("I", spec.optionalCapturedTargetParameters[3].typeDescriptor)
    }

    @Test
    fun generatesModifyReturnValueHandlerStubStaysMinimalWithTargetParameters() {
        val source = trimmedSource("""
            @ModifyReturnValue(method = "draw(Ljava/lang/String;FFI)I", at = @At("RETURN"))
        """)
        val site = sites(source).first()
        val stub = service.generateHandlerStub(source, site, listOf("net/minecraft/client/font/TextRenderer"))
        assertNotNull(stub)
        assertTrue(stub.contains("int mcdevHandler(int original)"))
        assertTrue(!stub.contains("arg0"))
    }

    @Test
    fun validatesModifyReturnValueHandlerWithZeroCapturedTargetParameters() {
        val source = trimmedSource("""
            @ModifyReturnValue(method = "draw(Ljava/lang/String;FFI)I", at = @At("RETURN"))
            private int mcdevHandler(int original) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("net/minecraft/client/font/TextRenderer"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun validatesModifyReturnValueHandlerWithFirstCapturedTargetParameter() {
        val source = trimmedSource("""
            @ModifyReturnValue(method = "draw(Ljava/lang/String;FFI)I", at = @At("RETURN"))
            private int mcdevHandler(int original, String arg0) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("net/minecraft/client/font/TextRenderer"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun validatesModifyReturnValueHandlerWithPartialCapturedTargetParameterPrefix() {
        val source = trimmedSource("""
            @ModifyReturnValue(method = "draw(Ljava/lang/String;FFI)I", at = @At("RETURN"))
            private int mcdevHandler(int original, String arg0, float arg1) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("net/minecraft/client/font/TextRenderer"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun validatesModifyReturnValueHandlerWithFullCapturedTargetParameterPrefix() {
        val source = trimmedSource("""
            @ModifyReturnValue(method = "draw(Ljava/lang/String;FFI)I", at = @At("RETURN"))
            private int mcdevHandler(int original, String arg0, float arg1, float arg2, int arg3) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("net/minecraft/client/font/TextRenderer"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun validatesModifyReturnValueHandlerWithTrailingSugarParameter() {
        val source = trimmedSource("""
            @ModifyReturnValue(method = "draw(Ljava/lang/String;FFI)I", at = @At("RETURN"))
            private int mcdevHandler(int original, String arg0, @Local int counter) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        assertTrue(handler.parameters.last().isSugar)
        val issues = service.validateHandler(source, site, listOf("net/minecraft/client/font/TextRenderer"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun rejectsModifyReturnValueHandlerWithSkippedCapturedTargetParameter() {
        val source = trimmedSource("""
            @ModifyReturnValue(method = "draw(Ljava/lang/String;FFI)I", at = @At("RETURN"))
            private int mcdevHandler(int original, float arg1) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("net/minecraft/client/font/TextRenderer"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun rejectsModifyReturnValueHandlerWithWrongCapturedTargetParameterType() {
        val source = trimmedSource("""
            @ModifyReturnValue(method = "draw(Ljava/lang/String;FFI)I", at = @At("RETURN"))
            private int mcdevHandler(int original, int arg0) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("net/minecraft/client/font/TextRenderer"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun rejectsModifyReturnValueHandlerWithTooManyCapturedTargetParameters() {
        val source = trimmedSource("""
            @ModifyReturnValue(method = "draw(Ljava/lang/String;FFI)I", at = @At("RETURN"))
            private int mcdevHandler(int original, String arg0, float arg1, float arg2, int arg3, int extra) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("net/minecraft/client/font/TextRenderer"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun generatesWrapWithConditionHandlerStub() {
        val service = defaultWrapWithConditionInvokeService()
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
        """)
        val site = sites(source).first()
        val stub = service.generateHandlerStub(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(stub)
        assertTrue(stub.contains("boolean mcdevHandler"))
        assertTrue(stub.contains("String instance"))
        assertTrue(stub.contains("return true"))
        assertTrue(!stub.contains("Operation"))
        assertTrue(!stub.contains("arg0"))
    }

    @Test
    fun modifyReceiverExpectsInstanceParameterWithoutOperation() {
        val service = invokeService(invokeCandidate(AtTargetOperationKind.INVOKE_VIRTUAL))
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
        """)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals("Ljava/lang/String;", spec.returnTypeDescriptor)
        assertEquals(1, spec.parameters.size)
        assertEquals("instance", spec.parameters.single().name)
        assertEquals("Ljava/lang/String;", spec.parameters.single().typeDescriptor)
        assertTrue(spec.parameters.none { it.isOperation })
        val stub = service.generateHandlerStub(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(stub)
        assertTrue(stub.contains("return instance"))
    }

    @Test
    fun modifyReceiverUsesBytecodeTargetWhenCalleeIsAbsentFromClassIndex() {
        val classIndex = FakeClassIndex(
            classes = FakeClassIndex.defaultClasses() + listOf(
                ClassIndexEntry("SimpleTarget", "com.example.target", "com/example/target/SimpleTarget"),
            ),
            methods = FakeClassIndex.defaultMethods() + mapOf(
                "com/example/target/SimpleTarget" to listOf(
                    MethodIndexEntry("draw", "(Ljava/lang/String;FF)V", false, "draw(String, float, float): void"),
                ),
            ),
        )
        val service = HandlerSignatureService(classIndex, MixinExtrasTestFixtures.bytecodeIndex)
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
        """)
        val site = sites(source).first()

        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))

        assertNotNull(spec)
        assertEquals("Ljava/lang/String;", spec.returnTypeDescriptor)
        assertEquals("instance", spec.parameters.single().name)
    }

    @Test
    fun modifyReceiverInvokeExposesOptionalCapturedTargetParameters() {
        val service = invokeService(invokeCandidate(AtTargetOperationKind.INVOKE_VIRTUAL))
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
        """)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals(1, spec.parameters.size)
        assertEquals("instance", spec.parameters.single().name)
        assertEquals(3, spec.optionalCapturedTargetParameters.size)
        assertEquals("Ljava/lang/String;", spec.optionalCapturedTargetParameters[0].typeDescriptor)
        assertEquals("F", spec.optionalCapturedTargetParameters[1].typeDescriptor)
        assertEquals("F", spec.optionalCapturedTargetParameters[2].typeDescriptor)
    }

    @Test
    fun generatesModifyReceiverInvokeHandlerStubStaysMinimalWithTargetParameters() {
        val service = invokeService(invokeCandidate(AtTargetOperationKind.INVOKE_VIRTUAL))
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
        """)
        val site = sites(source).first()
        val stub = service.generateHandlerStub(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(stub)
        assertTrue(stub.contains("String mcdevHandler(String instance)"))
        assertTrue(!stub.contains("arg0"))
    }

    @Test
    fun validatesModifyReceiverInvokeHandlerWithZeroCapturedTargetParameters() {
        val service = invokeService(invokeCandidate(AtTargetOperationKind.INVOKE_VIRTUAL))
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private String mcdevModifyReceiver(String instance) {
                return instance;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun validatesModifyReceiverInvokeHandlerWithPartialCapturedTargetParameterPrefix() {
        val service = invokeService(invokeCandidate(AtTargetOperationKind.INVOKE_VIRTUAL))
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private String mcdevModifyReceiver(String instance, String arg0, float arg1) {
                return instance;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun validatesModifyReceiverInvokeHandlerWithFullCapturedTargetParameterPrefix() {
        val service = invokeService(invokeCandidate(AtTargetOperationKind.INVOKE_VIRTUAL))
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private String mcdevModifyReceiver(String instance, String arg0, float arg1, float arg2) {
                return instance;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun validatesModifyReceiverInvokeHandlerWithTrailingSugarParameter() {
        val service = invokeService(invokeCandidate(AtTargetOperationKind.INVOKE_VIRTUAL))
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private String mcdevModifyReceiver(String instance, String arg0, @Local int counter) {
                return instance;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        assertTrue(handler.parameters.last().isSugar)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun rejectsModifyReceiverInvokeHandlerWithSkippedCapturedTargetParameter() {
        val service = invokeService(invokeCandidate(AtTargetOperationKind.INVOKE_VIRTUAL))
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private String mcdevModifyReceiver(String instance, float arg1) {
                return instance;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun rejectsModifyReceiverInvokeHandlerWithWrongCapturedTargetParameterType() {
        val service = invokeService(invokeCandidate(AtTargetOperationKind.INVOKE_VIRTUAL))
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private String mcdevModifyReceiver(String instance, int arg0) {
                return instance;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun rejectsModifyReceiverInvokeHandlerWithTooManyCapturedTargetParameters() {
        val service = invokeService(invokeCandidate(AtTargetOperationKind.INVOKE_VIRTUAL))
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private String mcdevModifyReceiver(String instance, String arg0, float arg1, float arg2, int extra) {
                return instance;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun rejectsModifyReceiverInvokeHandlerWithSugarBeforeCapturedArgs() {
        val service = invokeService(invokeCandidate(AtTargetOperationKind.INVOKE_VIRTUAL))
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private String mcdevModifyReceiver(String instance, @Local int counter, String arg0) {
                return instance;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun modifyReceiverInvokeSpecialUsesMixinTargetReceiverNotCalleeOwner() {
        val parentOwner = "com/example/target/Parent"
        val classIndex = FakeClassIndex(
            classes = MixinExtrasTestFixtures.classIndex.findClasses("", 100) + listOf(
                ClassIndexEntry("Parent", "com.example.target", parentOwner),
            ),
            methods = FakeClassIndex.defaultMethods() + mapOf(
                "java/lang/String" to listOf(
                    MethodIndexEntry("length", "()I", false, "length(): int"),
                ),
                "com/example/target/SimpleTarget" to listOf(
                    MethodIndexEntry("draw", "(Ljava/lang/String;FF)V", false, "draw(String, float, float): void"),
                ),
                parentOwner to listOf(
                    MethodIndexEntry("foo", "()V", false, "foo(): void"),
                ),
            ),
            fields = emptyMap(),
        )
        val service = HandlerSignatureService(
            classIndex,
            FakeBytecodeIndex(
                candidates = mapOf(
                    "com/example/target/SimpleTarget#draw#INVOKE" to listOf(
                        invokeCandidate(
                            operationKind = AtTargetOperationKind.INVOKE_SPECIAL,
                            owner = parentOwner,
                            name = "foo",
                            descriptor = "()V",
                        ),
                    ),
                ),
            ),
        )
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Lcom/example/target/Parent;foo()V"))
        """)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals("Lcom/example/target/SimpleTarget;", spec.returnTypeDescriptor)
        assertEquals("simpleTarget", spec.parameters.single().name)
        assertEquals("Lcom/example/target/SimpleTarget;", spec.parameters.single().typeDescriptor)
    }

    @Test
    fun modifyReceiverInvokeVirtualRetainsCalleeOwner() {
        val service = invokeService(invokeCandidate(AtTargetOperationKind.INVOKE_VIRTUAL))
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
        """)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals("Ljava/lang/String;", spec.returnTypeDescriptor)
        assertEquals("instance", spec.parameters.single().name)
        assertEquals("Ljava/lang/String;", spec.parameters.single().typeDescriptor)
    }

    @Test
    fun modifyReceiverInvokeAmbiguousMultiTargetEffectiveReceiverReturnsNull() {
        val parentOwner = "com/example/target/Parent"
        val classIndex = FakeClassIndex(
            classes = MixinExtrasTestFixtures.classIndex.findClasses("", 100) + listOf(
                ClassIndexEntry("Parent", "com.example.target", parentOwner),
            ),
            methods = FakeClassIndex.defaultMethods() + mapOf(
                "com/example/target/SharedMixinTargetA" to listOf(
                    MethodIndexEntry("shared", "()I", false, "shared(): int"),
                ),
                "com/example/target/SharedMixinTargetB" to listOf(
                    MethodIndexEntry("shared", "()I", false, "shared(): int"),
                ),
                parentOwner to listOf(
                    MethodIndexEntry("foo", "()V", false, "foo(): void"),
                ),
            ),
            fields = emptyMap(),
        )
        val targets = listOf(
            "com/example/target/SharedMixinTargetA",
            "com/example/target/SharedMixinTargetB",
        )
        val invokeSpecial = invokeCandidate(
            operationKind = AtTargetOperationKind.INVOKE_SPECIAL,
            owner = parentOwner,
            name = "foo",
            descriptor = "()V",
        )
        val service = HandlerSignatureService(
            classIndex,
            FakeBytecodeIndex(
                candidates = mapOf(
                    "com/example/target/SharedMixinTargetA#shared#INVOKE" to listOf(invokeSpecial),
                    "com/example/target/SharedMixinTargetB#shared#INVOKE" to listOf(invokeSpecial),
                ),
            ),
        )
        val source = trimmedSource("""
            @ModifyReceiver(method = "shared()I", at = @At(value = "INVOKE", target = "Lcom/example/target/Parent;foo()V"))
        """)
        val site = sites(source).first()
        assertNull(service.expectedSignature(source, site, targets))
        assertNull(service.generateHandlerStub(source, site, targets))
    }

    @Test
    fun modifyReceiverInvokeWithoutBytecodeIndexReturnsNull() {
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
        """)
        val site = sites(source).first()
        assertNull(service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget")))
    }

    @Test
    fun modifyReceiverInvokeMissingOperationKindReturnsNull() {
        val service = invokeService(
            AtTargetCandidate(
                owner = "java/lang/String",
                name = "length",
                descriptor = "()I",
                displayLabel = "length(): int",
                detail = "String",
                kind = AtTargetKind.INVOKE,
                ordinal = 0,
            ),
        )
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
        """)
        val site = sites(source).first()
        assertNull(service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget")))
    }

    @Test
    fun modifyReceiverInvokeStaticTargetReturnsNull() {
        val service = invokeService(
            invokeCandidate(
                operationKind = AtTargetOperationKind.INVOKE_STATIC,
                owner = "com/example/target/SimpleTarget",
                name = "noop",
                descriptor = "()V",
            ),
        )
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Lcom/example/target/SimpleTarget;noop()V"))
        """)
        val site = sites(source).first()
        assertNull(service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget")))
    }

    @Test
    fun generatesWrapMethodHandlerStub() {
        val source = trimmedSource("""
            @WrapMethod(method = "draw(Ljava/lang/String;FF)V")
        """)
        val site = sites(source).first()
        val stub = service.generateHandlerStub(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(stub)
        assertTrue(stub.contains("Operation<Void>"))
        assertTrue(stub.contains("String arg0"))
        assertTrue(stub.contains("original.call(arg0, arg1, arg2)"))
        assertTrue(!stub.contains("simpleTarget"))
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
    fun unresolvedMixinTargetDoesNotSearchGlobalClasses() {
        val countingIndex = CountingClassIndex(MixinExtrasTestFixtures.classIndex)
        val unresolvedService = HandlerSignatureService(countingIndex)
        val source = trimmedSource("""
            @ModifyReturnValue(method = "compute()I", at = @At("RETURN"))
            private int mcdevHandler(int original) {
                return original;
            }
        """)
        val site = sites(source).first()
        assertNull(unresolvedService.expectedSignature(source, site, listOf("UnknownMixinTarget")))
        assertEquals(0, countingIndex.emptyPrefixFindClassesCalls)
        assertEquals(0, countingIndex.getMethodsCalls)
    }

    @Test
    fun ambiguousTargetMethodWithoutDescriptorReturnsNullSpecAndStub() {
        val source = trimmedSource("""
            @ModifyReturnValue(method = "compute", at = @At("RETURN"))
            private int mcdevHandler(int original) {
                return original;
            }
        """)
        val site = sites(source).first()
        val targets = listOf("com/example/target/SimpleTarget")
        assertNull(service.resolveTargetMethod(targets, "compute"))
        assertNull(service.expectedSignature(source, site, targets))
        assertNull(service.generateHandlerStub(source, site, targets))
    }

    @Test
    fun ambiguousTargetMethodWithoutDescriptorReportsExplicitDescriptorRequired() {
        val source = trimmedSource("""
            @ModifyReturnValue(method = "compute", at = @At("RETURN"))
            private int mcdevHandler(int original) {
                return original;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertEquals(1, issues.size)
        assertEquals(MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH, issues.single().code)
        assertEquals(
            "Ambiguous target method 'compute'; an explicit method descriptor is required",
            issues.single().message,
        )
    }

    @Test
    fun explicitDescriptorResolvesEachExactOverload() {
        val targets = listOf("com/example/target/SimpleTarget")
        val intReturn = service.resolveTargetMethod(targets, "compute()I")
        val voidReturn = service.resolveTargetMethod(targets, "compute()V")
        assertNotNull(intReturn)
        assertNotNull(voidReturn)
        assertEquals("()I", intReturn.descriptor)
        assertEquals("()V", voidReturn.descriptor)

        val drawString = service.resolveTargetMethod(targets, "draw(Ljava/lang/String;FF)V")
        val drawInt = service.resolveTargetMethod(targets, "draw(I)V")
        assertNotNull(drawString)
        assertNotNull(drawInt)
        assertEquals("(Ljava/lang/String;FF)V", drawString.descriptor)
        assertEquals("(I)V", drawInt.descriptor)
    }

    @Test
    fun identicalDescriptorAcrossMultipleMixinTargetsIsAcceptedWithoutExplicitDescriptor() {
        val targets = listOf(
            "com/example/target/SharedMixinTargetA",
            "com/example/target/SharedMixinTargetB",
        )
        val method = service.resolveTargetMethod(targets, "shared")
        assertNotNull(method)
        assertEquals("()I", method.descriptor)

        val source = trimmedSource("""
            @ModifyReturnValue(method = "shared", at = @At("RETURN"))
            private int mcdevHandler(int original) {
                return original;
            }
        """)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, targets)
        assertNotNull(spec)
        assertEquals("I", spec.returnTypeDescriptor)
    }

    @Test
    fun findAnnotationSitesParsesMultipleAnnotations() {
        val source = MixinExtrasTestFixtures.MODIFY_EXPRESSION_SOURCE + MixinExtrasTestFixtures.MODIFY_RETURN_SOURCE
        val sites = HandlerSignatureService.findAnnotationSites(source)
        assertEquals(2, sites.size)
    }

    @Test
    fun findSugarHandlerAnnotationSitesIncludesStandardInjectors() {
        val source = """
            @Mixin(LocalCaptureSamples.class)
            abstract class ExampleMixin {
                @Inject(method = "tick()V", at = @At("HEAD"))
                private void injectHandler(CallbackInfo ci) {}

                @WrapOperation(method = "tick()V", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/MinecraftClient;tick()V"))
                private Object wrapHandler(Operation<Void> original) { return original.call(); }
            }
        """.trimIndent()

        assertTrue(HandlerSignatureService.findAnnotationSites(source).none { it.annotation == MixinExtrasAnnotation.INJECT })
        assertEquals(
            listOf(MixinExtrasAnnotation.INJECT, MixinExtrasAnnotation.WRAP_OPERATION),
            HandlerSignatureService.findSugarHandlerAnnotationSites(source).map { it.annotation },
        )
    }

    @Test
    fun findSugarHandlerAnnotationSitesParsesInjectHandlerLocalParameter() {
        val source = """
            @Mixin(LocalCaptureSamples.class)
            abstract class ExampleMixin {
                @Inject(
                    method = "instanceWithArgs(Ljava/lang/String;I)I",
                    at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I")
                )
                private void mcdevHandler(String message, @Local(ordinal = ) int captured, CallbackInfo ci) {}
            }
        """.trimIndent()

        val site = HandlerSignatureService.findSugarHandlerAnnotationSites(source).single()
        assertEquals(MixinExtrasAnnotation.INJECT, site.annotation)
        val localParameter = site.handlerMethod!!.parameters.single { it.sugarSpec is HandlerParameterSugarSpec.Local }
        assertEquals("captured", localParameter.name)
    }

    @Test
    fun findSugarHandlerAnnotationSitesParsesModifyConstantConstantSelectors() {
        data class Case(
            val name: String,
            val constantAttribute: String,
            val expectedAtValue: String?,
            val expectedAtArgs: List<String>?,
            val expectedSiteCount: Int = 1,
            val expectedAtOrdinal: Int? = null,
        )
        val cases = listOf(
            Case("missing constant", "", "CONSTANT", emptyList()),
            Case("empty @Constant()", "constant = @Constant()", "CONSTANT", emptyList()),
            Case(
                "intValue",
                "constant = @Constant(intValue = 42)",
                "CONSTANT",
                listOf("intValue=42"),
            ),
            Case(
                "longValue strips suffix",
                "constant = @Constant(longValue = 100L)",
                "CONSTANT",
                listOf("longValue=100"),
            ),
            Case(
                "floatValue strips suffix",
                "constant = @Constant(floatValue = 1.5f)",
                "CONSTANT",
                listOf("floatValue=1.5"),
            ),
            Case(
                "doubleValue strips suffix",
                "constant = @Constant(doubleValue = 2.5d)",
                "CONSTANT",
                listOf("doubleValue=2.5"),
            ),
            Case(
                "stringValue hello",
                """constant = @Constant(stringValue = "hello")""",
                "CONSTANT",
                listOf("stringValue=hello"),
            ),
            Case(
                "stringValue unquotes escapes",
                "constant = @Constant(stringValue = \"hel\\\"lo\")",
                "CONSTANT",
                listOf("stringValue=hel\"lo"),
            ),
            Case(
                "stringValue decodes newline tab unicode octal",
                "constant = @Constant(stringValue = \"a\\nb\\t\\u0043\\141\")",
                "CONSTANT",
                listOf("stringValue=a\nb\tCa"),
            ),
            Case(
                "stringValue decodes space escape",
                "constant = @Constant(stringValue = \"a\\sb\")",
                "CONSTANT",
                listOf("stringValue=a b"),
            ),
            Case(
                "stringValue rejects raw newline",
                """constant = @Constant(stringValue = "bad
value")""",
                null,
                null,
            ),
            Case(
                "stringValue malformed escape fail closed",
                "constant = @Constant(stringValue = \"bad\\z\")",
                null,
                null,
            ),
            Case(
                "classValue strips .class",
                "constant = @Constant(classValue = java.lang.String.class)",
                "CONSTANT",
                listOf("classValue=java.lang.String"),
            ),
            Case(
                "nullValue",
                "constant = @Constant(nullValue = true)",
                "CONSTANT",
                listOf("nullValue=true"),
            ),
            Case(
                "nullValue false fail closed",
                "constant = @Constant(nullValue = false)",
                null,
                null,
            ),
            Case(
                "constant array preserves order",
                "constant = { @Constant(intValue = 1), @Constant(stringValue = \"a\") }",
                "CONSTANT",
                null,
                expectedSiteCount = 2,
            ),
            Case(
                "method and constant array cross-product",
                "method = { \"a()V\", \"b()V\" }, constant = { @Constant(intValue = 1), @Constant(intValue = 2) }",
                "CONSTANT",
                null,
                expectedSiteCount = 4,
            ),
            Case(
                "conflicting discriminators fail closed",
                "constant = @Constant(intValue = 1, floatValue = 1.0f)",
                null,
                null,
            ),
            Case(
                "malformed intValue fail closed",
                "constant = @Constant(intValue = abc)",
                null,
                null,
            ),
            Case(
                "expandZeroConditions scalar",
                "constant = @Constant(expandZeroConditions = Constant.Condition.LESS_THAN_ZERO)",
                "CONSTANT",
                emptyList(),
            ),
            Case(
                "expandZeroConditions array",
                "constant = @Constant(expandZeroConditions = { Constant.Condition.LESS_THAN_ZERO, Constant.Condition.GREATER_THAN_ZERO })",
                "CONSTANT",
                emptyList(),
            ),
            Case(
                "expandZeroConditions with intValue=0",
                "constant = @Constant(intValue = 0, expandZeroConditions = Constant.Condition.LESS_THAN_ZERO)",
                "CONSTANT",
                listOf("intValue=0"),
            ),
            Case(
                "non-default slice fail closed",
                "constant = @Constant(intValue = 1), slice = @Slice(from = @At(\"HEAD\"))",
                null,
                null,
            ),
            Case(
                "empty constant slice preserves selector",
                """constant = @Constant(intValue = 1, slice = "")""",
                "CONSTANT",
                listOf("intValue=1"),
            ),
            Case(
                "constant slice fail closed",
                "constant = @Constant(intValue = 1, slice = \"foo\")",
                null,
                null,
            ),
            Case(
                "empty constant array",
                "constant = {}",
                "CONSTANT",
                emptyList(),
            ),
            Case(
                "constant ordinal propagates",
                "constant = @Constant(intValue = 42, ordinal = 1)",
                "CONSTANT",
                listOf("intValue=42"),
                expectedAtOrdinal = 1,
            ),
            Case(
                "constant ordinal minus one is null",
                "constant = @Constant(intValue = 42, ordinal = -1)",
                "CONSTANT",
                listOf("intValue=42"),
                expectedAtOrdinal = null,
            ),
            Case(
                "constant log ignored",
                "constant = @Constant(intValue = 1, log = false)",
                "CONSTANT",
                listOf("intValue=1"),
            ),
            Case(
                "explicit void class does not broaden",
                "constant = @Constant(classValue = void.class)",
                "CONSTANT",
                listOf("classValue=void"),
            ),
            Case(
                "default empty slice array",
                "constant = @Constant(intValue = 1), slice = {}",
                "CONSTANT",
                listOf("intValue=1"),
            ),
        )
        for (case in cases) {
            val constantPart = case.constantAttribute.trim().let { if (it.isEmpty()) "" else "$it, " }
            val source = """
                @Mixin(ConstantSamples.class)
                abstract class ExampleMixin {
                    @ModifyConstant(
                        $constantPart method = "constants()V"
                    )
                    private void mcdevHandler(@Local int captured) {}
                }
            """.trimIndent()
            val sites = HandlerSignatureService.findSugarHandlerAnnotationSites(source)
            assertEquals(case.expectedSiteCount, sites.size, case.name)
            if (case.expectedAtValue == null) {
                assertTrue(sites.all { it.atValue == null }, case.name)
                continue
            }
            if (case.expectedAtArgs == null) {
                if (case.expectedSiteCount == 2) {
                    assertEquals(
                        listOf(listOf("intValue=1"), listOf("stringValue=a")),
                        sites.map { it.atArgs },
                        case.name,
                    )
                } else {
                    assertEquals(4, sites.size, case.name)
                    assertTrue(sites.all { it.atValue == "CONSTANT" && it.atArgs.size == 1 }, case.name)
                    assertEquals(
                        setOf("intValue=1", "intValue=2"),
                        sites.map { it.atArgs.single() }.toSet(),
                        case.name,
                    )
                    assertEquals(
                        setOf("a()V", "b()V"),
                        sites.map { it.methodAttribute }.toSet(),
                        case.name,
                    )
                }
            } else {
                val site = sites.single()
                assertEquals(case.expectedAtValue, site.atValue, case.name)
                assertEquals(case.expectedAtArgs, site.atArgs, case.name)
                assertEquals(case.expectedAtOrdinal, site.atOrdinal, case.name)
            }
        }
    }

    @Test
    fun modifyConstantExpandZeroConditionsParsesAndInfersInt() {
        fun source(condition: String) = """
            @Mixin(SimpleTarget.class)
            abstract class ExampleMixin {
                @ModifyConstant(
                    method = "draw(Ljava/lang/String;FF)V",
                    constant = @Constant(expandZeroConditions = $condition),
                )
                private int mcdevHandler(int original) { return original; }
            }
        """.trimIndent()

        val cases = listOf(
            "Constant.Condition.LESS_THAN_ZERO" to setOf(Opcodes.IFLT, Opcodes.IFGE),
            "{ Constant.Condition.LESS_THAN_ZERO, org.spongepowered.asm.mixin.injection.Constant.Condition.GREATER_THAN_ZERO }" to
                setOf(Opcodes.IFLE, Opcodes.IFGT, Opcodes.IFGE, Opcodes.IFLT),
        )
        for ((condition, expectedOpcodes) in cases) {
            val text = source(condition)
            val site = HandlerSignatureService.findSugarHandlerAnnotationSites(text).single()
            assertEquals(emptyList(), site.atArgs)
            assertEquals(expectedOpcodes, site.expandZeroConditions)
            assertEquals("I", service.expectedSignature(text, site, listOf("com/example/target/SimpleTarget"))?.returnTypeDescriptor)
        }

        val explicitText = source("Constant.Condition.LESS_THAN_ZERO").replace(
            "constant = @Constant(expandZeroConditions = Constant.Condition.LESS_THAN_ZERO)",
            "constant = @Constant(intValue = 0, expandZeroConditions = Constant.Condition.LESS_THAN_ZERO)",
        )
        val explicitSite = HandlerSignatureService.findSugarHandlerAnnotationSites(explicitText).single()
        assertEquals(listOf("intValue=0"), explicitSite.atArgs)
        assertEquals("I", service.expectedSignature(explicitText, explicitSite, listOf("com/example/target/SimpleTarget"))?.returnTypeDescriptor)

        val emptyText = source("{}")
        val emptySite = HandlerSignatureService.findSugarHandlerAnnotationSites(emptyText).single()
        assertTrue(emptySite.expandZeroConditions.isEmpty())
        assertNull(service.expectedSignature(emptyText, emptySite, listOf("com/example/target/SimpleTarget")))

        val malformedText = source("Constant.Condition.UNKNOWN")
        val malformedSite = HandlerSignatureService.findSugarHandlerAnnotationSites(malformedText).single()
        assertNull(malformedSite.atValue)
        assertNull(service.expectedSignature(malformedText, malformedSite, listOf("com/example/target/SimpleTarget")))

        val removedConditionText = source("Constant.Condition.EQUAL_TO_ZERO")
        val removedConditionSite = HandlerSignatureService.findSugarHandlerAnnotationSites(removedConditionText).single()
        assertNull(removedConditionSite.atValue)
        assertNull(service.expectedSignature(removedConditionText, removedConditionSite, listOf("com/example/target/SimpleTarget")))
    }

    @Test
    fun findSugarHandlerAnnotationSitesEmitsBothMethodSelectorsInLexicalOrderForInjectMethodArray() {
        val source = """
            @Mixin(LocalCaptureSamples.class)
            abstract class ExampleMixin {
                @Inject(
                    method = { "missing()V", "instanceWithArgs(Ljava/lang/String;I)I" },
                    at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I")
                )
                private void mcdevHandler(String message, @Local(ordinal = ) int captured, CallbackInfo ci) {}
            }
        """.trimIndent()

        val sites = HandlerSignatureService.findSugarHandlerAnnotationSites(source)
        assertEquals(
            listOf("missing()V", "instanceWithArgs(Ljava/lang/String;I)I"),
            sites.map { it.methodAttribute },
        )
    }

    @Test
    fun wrapOperationGetFieldExpectsReceiverOperationAndFieldReturn() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE))
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private String mcdevWrapGet(SimpleTarget simpleTarget, Operation<String> original) {
                return original.call(simpleTarget);
            }
        """)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals("Ljava/lang/String;", spec.returnTypeDescriptor)
        assertEquals(2, spec.parameters.size)
        assertEquals("Lcom/example/target/SimpleTarget;", spec.parameters[0].typeDescriptor)
        assertTrue(spec.parameters[1].isOperation)
        assertEquals("Ljava/lang/String;", spec.parameters[1].operationGenericDescriptor)
        assertEquals(listOf("simpleTarget"), spec.operationCallArgs)
        assertEquals(3, spec.optionalCapturedTargetParameters.size)
        assertEquals("Ljava/lang/String;", spec.optionalCapturedTargetParameters[0].typeDescriptor)
        assertEquals("F", spec.optionalCapturedTargetParameters[1].typeDescriptor)
        assertEquals("F", spec.optionalCapturedTargetParameters[2].typeDescriptor)
    }

    @Test
    fun wrapOperationGetStaticExpectsOperationOnlyAndFieldReturn() {
        val service = fieldService(
            fieldCandidate(
                name = "FLAG",
                descriptor = "I",
                operationKind = AtTargetOperationKind.FIELD_GET_STATIC,
            ),
        )
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;FLAG:I"))
            private int mcdevWrapGetStatic(Operation<Integer> original) {
                return original.call();
            }
        """)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals("I", spec.returnTypeDescriptor)
        assertEquals(1, spec.parameters.size)
        assertTrue(spec.parameters.single().isOperation)
        assertEquals("I", spec.parameters.single().operationGenericDescriptor)
        assertEquals(emptyList<String>(), spec.operationCallArgs)
    }

    @Test
    fun wrapOperationPutFieldExpectsReceiverValueAndVoidOperation() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_PUT_INSTANCE))
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private void mcdevWrapPut(SimpleTarget simpleTarget, String arg0, Operation<Void> original) {
                original.call(simpleTarget, arg0);
            }
        """)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals("V", spec.returnTypeDescriptor)
        assertEquals(3, spec.parameters.size)
        assertEquals("Lcom/example/target/SimpleTarget;", spec.parameters[0].typeDescriptor)
        assertEquals("Ljava/lang/String;", spec.parameters[1].typeDescriptor)
        assertTrue(spec.parameters[2].isOperation)
        assertEquals("V", spec.parameters[2].operationGenericDescriptor)
        assertEquals(listOf("simpleTarget", "arg0"), spec.operationCallArgs)
    }

    @Test
    fun wrapOperationPutStaticExpectsValueAndVoidOperation() {
        val service = fieldService(
            fieldCandidate(
                name = "FLAG",
                descriptor = "I",
                operationKind = AtTargetOperationKind.FIELD_PUT_STATIC,
            ),
        )
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;FLAG:I"))
            private void mcdevWrapPutStatic(int arg0, Operation<Void> original) {
                original.call(arg0);
            }
        """)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals("V", spec.returnTypeDescriptor)
        assertEquals(2, spec.parameters.size)
        assertEquals("I", spec.parameters[0].typeDescriptor)
        assertTrue(spec.parameters[1].isOperation)
        assertEquals("V", spec.parameters[1].operationGenericDescriptor)
        assertEquals(listOf("arg0"), spec.operationCallArgs)
    }

    @Test
    fun wrapWithConditionPutFieldExpectsReceiverValueAndBooleanReturn() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_PUT_INSTANCE))
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private boolean mcdevWrapPut(SimpleTarget simpleTarget, String arg0) {
                return true;
            }
        """)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals("Z", spec.returnTypeDescriptor)
        assertEquals(2, spec.parameters.size)
        assertTrue(spec.parameters.none { it.isOperation })
        assertEquals(3, spec.optionalCapturedTargetParameters.size)
        assertEquals("Ljava/lang/String;", spec.optionalCapturedTargetParameters[0].typeDescriptor)
        assertEquals("F", spec.optionalCapturedTargetParameters[1].typeDescriptor)
        assertEquals("F", spec.optionalCapturedTargetParameters[2].typeDescriptor)
    }

    @Test
    fun validatesWrapWithConditionPutFieldHandlerWithPartialCapturedTargetParameterPrefix() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_PUT_INSTANCE))
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private boolean mcdevWrapPut(SimpleTarget simpleTarget, String value, String arg0, float arg1) {
                return true;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun validatesWrapWithConditionPutFieldHandlerWithFullCapturedTargetParameterPrefix() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_PUT_INSTANCE))
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private boolean mcdevWrapPut(SimpleTarget simpleTarget, String value, String arg0, float arg1, float arg2) {
                return true;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun validatesWrapWithConditionPutFieldHandlerWithTrailingSugarParameter() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_PUT_INSTANCE))
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private boolean mcdevWrapPut(SimpleTarget simpleTarget, String value, String arg0, @Local int counter) {
                return true;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        assertTrue(handler.parameters.last().isSugar)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun rejectsWrapWithConditionPutFieldHandlerWithSkippedCapturedTargetParameter() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_PUT_INSTANCE))
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private boolean mcdevWrapPut(SimpleTarget simpleTarget, String value, float arg1) {
                return true;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun rejectsWrapWithConditionPutFieldHandlerWithWrongCapturedTargetParameterType() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_PUT_INSTANCE))
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private boolean mcdevWrapPut(SimpleTarget simpleTarget, String value, int arg0) {
                return true;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun rejectsWrapWithConditionPutFieldHandlerWithTooManyCapturedTargetParameters() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_PUT_INSTANCE))
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private boolean mcdevWrapPut(SimpleTarget simpleTarget, String value, String arg0, float arg1, float arg2, int extra) {
                return true;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun rejectsWrapWithConditionPutFieldHandlerWithSugarBeforeCapturedArgs() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_PUT_INSTANCE))
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private boolean mcdevWrapPut(SimpleTarget simpleTarget, String value, @Local int counter, String arg0) {
                return true;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun generatesWrapWithConditionPutFieldHandlerStubStaysMinimalWithTargetParameters() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_PUT_INSTANCE))
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
        """)
        val site = sites(source).first()
        val stub = service.generateHandlerStub(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(stub)
        assertTrue(stub.contains("boolean mcdevHandler(SimpleTarget simpleTarget, String arg0)"))
        assertTrue(stub.contains("return true"))
        assertTrue(!stub.contains("arg1"))
    }

    @Test
    fun wrapWithConditionPutStaticExpectsValueAndBooleanReturn() {
        val service = fieldService(
            fieldCandidate(
                name = "FLAG",
                descriptor = "I",
                operationKind = AtTargetOperationKind.FIELD_PUT_STATIC,
            ),
        )
        val source = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;FLAG:I"))
            private boolean mcdevWrapPutStatic(int arg0) {
                return true;
            }
        """)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals("Z", spec.returnTypeDescriptor)
        assertEquals(1, spec.parameters.size)
        assertEquals("I", spec.parameters.single().typeDescriptor)
        assertEquals(3, spec.optionalCapturedTargetParameters.size)
        assertEquals("Ljava/lang/String;", spec.optionalCapturedTargetParameters[0].typeDescriptor)
        assertEquals("F", spec.optionalCapturedTargetParameters[1].typeDescriptor)
        assertEquals("F", spec.optionalCapturedTargetParameters[2].typeDescriptor)
    }

    @Test
    fun wrapWithConditionFieldReadTargetsReturnNull() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE))
        val getFieldSource = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private boolean mcdevWrapGet(SimpleTarget simpleTarget) {
                return true;
            }
        """)
        val getStaticService = fieldService(
            fieldCandidate(
                name = "FLAG",
                descriptor = "I",
                operationKind = AtTargetOperationKind.FIELD_GET_STATIC,
            ),
        )
        val getStaticSource = trimmedSource("""
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;FLAG:I"))
            private boolean mcdevWrapGetStatic() {
                return true;
            }
        """)
        assertNull(service.expectedSignature(getFieldSource, sites(getFieldSource).first(), listOf("com/example/target/SimpleTarget")))
        assertNull(getStaticService.expectedSignature(getStaticSource, sites(getStaticSource).first(), listOf("com/example/target/SimpleTarget")))
    }

    @Test
    fun modifyReceiverGetFieldExpectsReceiverReturn() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE))
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private SimpleTarget mcdevModifyReceiver(SimpleTarget simpleTarget) {
                return simpleTarget;
            }
        """)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals("Lcom/example/target/SimpleTarget;", spec.returnTypeDescriptor)
        assertEquals(1, spec.parameters.size)
        assertEquals("simpleTarget", spec.parameters.single().name)
        assertEquals(3, spec.optionalCapturedTargetParameters.size)
        assertEquals("Ljava/lang/String;", spec.optionalCapturedTargetParameters[0].typeDescriptor)
        assertEquals("F", spec.optionalCapturedTargetParameters[1].typeDescriptor)
        assertEquals("F", spec.optionalCapturedTargetParameters[2].typeDescriptor)
    }

    @Test
    fun generatesModifyReceiverGetFieldHandlerStubStaysMinimalWithTargetParameters() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE))
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
        """)
        val site = sites(source).first()
        val stub = service.generateHandlerStub(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(stub)
        assertTrue(stub.contains("SimpleTarget mcdevHandler(SimpleTarget simpleTarget)"))
        assertTrue(!stub.contains("arg0"))
    }

    @Test
    fun validatesModifyReceiverGetFieldHandlerWithPartialCapturedTargetParameterPrefix() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE))
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private SimpleTarget mcdevModifyReceiver(SimpleTarget simpleTarget, String arg0) {
                return simpleTarget;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun validatesModifyReceiverGetFieldHandlerWithFullCapturedTargetParameterPrefix() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE))
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private SimpleTarget mcdevModifyReceiver(SimpleTarget simpleTarget, String arg0, float arg1, float arg2) {
                return simpleTarget;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun rejectsModifyReceiverGetFieldHandlerWithSkippedCapturedTargetParameter() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE))
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private SimpleTarget mcdevModifyReceiver(SimpleTarget simpleTarget, float arg1) {
                return simpleTarget;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun modifyReceiverPutFieldExpectsReceiverValueReturn() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_PUT_INSTANCE))
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private SimpleTarget mcdevModifyReceiver(SimpleTarget simpleTarget, String arg0) {
                return simpleTarget;
            }
        """)
        val site = sites(source).first()
        val spec = service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(spec)
        assertEquals("Lcom/example/target/SimpleTarget;", spec.returnTypeDescriptor)
        assertEquals(2, spec.parameters.size)
        assertEquals(3, spec.optionalCapturedTargetParameters.size)
        assertEquals("Ljava/lang/String;", spec.optionalCapturedTargetParameters[0].typeDescriptor)
    }

    @Test
    fun generatesModifyReceiverPutFieldHandlerStubStaysMinimalWithTargetParameters() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_PUT_INSTANCE))
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
        """)
        val site = sites(source).first()
        val stub = service.generateHandlerStub(source, site, listOf("com/example/target/SimpleTarget"))
        assertNotNull(stub)
        assertTrue(stub.contains("SimpleTarget mcdevHandler(SimpleTarget simpleTarget, String arg0)"))
        assertTrue(!stub.contains("arg1"))
    }

    @Test
    fun validatesModifyReceiverPutFieldHandlerWithPartialCapturedTargetParameterPrefix() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_PUT_INSTANCE))
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private SimpleTarget mcdevModifyReceiver(SimpleTarget simpleTarget, String value, String arg0) {
                return simpleTarget;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun validatesModifyReceiverPutFieldHandlerWithFullCapturedTargetParameterPrefix() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_PUT_INSTANCE))
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private SimpleTarget mcdevModifyReceiver(SimpleTarget simpleTarget, String value, String arg0, float arg1, float arg2) {
                return simpleTarget;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.isEmpty())
    }

    @Test
    fun rejectsModifyReceiverPutFieldHandlerWithWrongCapturedTargetParameterType() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_PUT_INSTANCE))
        val source = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private SimpleTarget mcdevModifyReceiver(SimpleTarget simpleTarget, String value, int arg0) {
                return simpleTarget;
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val issues = service.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler)
        assertTrue(issues.any { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun modifyReceiverStaticFieldTargetsReturnNull() {
        val service = fieldService(
            fieldCandidate(
                name = "FLAG",
                descriptor = "I",
                operationKind = AtTargetOperationKind.FIELD_GET_STATIC,
            ),
        )
        val getStaticSource = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;FLAG:I"))
            private SimpleTarget mcdevModifyReceiver(SimpleTarget simpleTarget) {
                return simpleTarget;
            }
        """)
        val putStaticService = fieldService(
            fieldCandidate(
                name = "FLAG",
                descriptor = "I",
                operationKind = AtTargetOperationKind.FIELD_PUT_STATIC,
            ),
        )
        val putStaticSource = trimmedSource("""
            @ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;FLAG:I"))
            private SimpleTarget mcdevModifyReceiver(SimpleTarget simpleTarget, int arg0) {
                return simpleTarget;
            }
        """)
        assertNull(service.expectedSignature(getStaticSource, sites(getStaticSource).first(), listOf("com/example/target/SimpleTarget")))
        assertNull(putStaticService.expectedSignature(putStaticSource, sites(putStaticSource).first(), listOf("com/example/target/SimpleTarget")))
    }

    @Test
    fun resolveFieldOperationKindQueriesEveryMixinTargetOwner() {
        val drawMethod = MethodIndexEntry("draw", "(Ljava/lang/String;FF)V", false, "draw(String, float, float): void")
        val classIndex = FakeClassIndex(
            classes = listOf(
                ClassIndexEntry("SharedMixinTargetA", "com.example.target", "com/example/target/SharedMixinTargetA"),
                ClassIndexEntry("SharedMixinTargetB", "com.example.target", "com/example/target/SharedMixinTargetB"),
                ClassIndexEntry("SimpleTarget", "com.example.target", "com/example/target/SimpleTarget"),
            ),
            methods = mapOf(
                "com/example/target/SharedMixinTargetA" to listOf(drawMethod),
                "com/example/target/SharedMixinTargetB" to listOf(drawMethod),
            ),
            fields = mapOf(
                "com/example/target/SimpleTarget" to listOf(
                    FieldIndexEntry("label", "Ljava/lang/String;", false, "String"),
                ),
            ),
        )
        val targets = listOf(
            "com/example/target/SharedMixinTargetA",
            "com/example/target/SharedMixinTargetB",
        )
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private String mcdevWrapGet(SimpleTarget simpleTarget, Operation<String> original) {
                return original.call(simpleTarget);
            }
        """)
        val site = sites(source).first()

        val resolveFromSecondOwner = HandlerSignatureService(
            classIndex,
            FakeBytecodeIndex(
                candidates = mapOf(
                    "com/example/target/SharedMixinTargetB#draw#FIELD" to listOf(
                        fieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE),
                    ),
                ),
            ),
        )
        assertNotNull(resolveFromSecondOwner.expectedSignature(source, site, targets))

        val conflictingKinds = HandlerSignatureService(
            classIndex,
            FakeBytecodeIndex(
                candidates = mapOf(
                    "com/example/target/SharedMixinTargetA#draw#FIELD" to listOf(
                        fieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE),
                    ),
                    "com/example/target/SharedMixinTargetB#draw#FIELD" to listOf(
                        fieldCandidate(AtTargetOperationKind.FIELD_PUT_INSTANCE),
                    ),
                ),
            ),
        )
        assertNull(conflictingKinds.expectedSignature(source, site, targets))
    }

    @Test
    fun explicitFieldOrdinalWithUnknownOperationKindForAnyMixinOwnerReturnsNullSignatureAndStub() {
        val drawMethod = MethodIndexEntry("draw", "(Ljava/lang/String;FF)V", false, "draw(String, float, float): void")
        val classIndex = FakeClassIndex(
            classes = listOf(
                ClassIndexEntry("SharedMixinTargetA", "com.example.target", "com/example/target/SharedMixinTargetA"),
                ClassIndexEntry("SharedMixinTargetB", "com.example.target", "com/example/target/SharedMixinTargetB"),
                ClassIndexEntry("SimpleTarget", "com.example.target", "com/example/target/SimpleTarget"),
            ),
            methods = mapOf(
                "com/example/target/SharedMixinTargetA" to listOf(drawMethod),
                "com/example/target/SharedMixinTargetB" to listOf(drawMethod),
            ),
        )
        val targets = listOf(
            "com/example/target/SharedMixinTargetA",
            "com/example/target/SharedMixinTargetB",
        )
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;", ordinal = 0))
            private String mcdevWrapGet(SimpleTarget simpleTarget, Operation<String> original) {
                return original.call(simpleTarget);
            }
        """)
        val site = sites(source).first()
        val service = HandlerSignatureService(
            classIndex,
            FakeBytecodeIndex(
                candidates = mapOf(
                    "com/example/target/SharedMixinTargetA#draw#FIELD" to listOf(
                        fieldCandidate(
                            AtTargetOperationKind.FIELD_GET_INSTANCE,
                            instructionOccurrenceIndex = 0,
                        ).copy(operationKind = null),
                    ),
                    "com/example/target/SharedMixinTargetB#draw#FIELD" to listOf(
                        fieldCandidate(
                            AtTargetOperationKind.FIELD_GET_INSTANCE,
                            instructionOccurrenceIndex = 0,
                        ),
                    ),
                ),
            ),
        )

        assertNull(service.expectedSignature(source, site, targets))
        assertNull(service.generateHandlerStub(source, site, targets))
    }

    @Test
    fun ambiguousFieldOperationKindsReturnNull() {
        val service = fieldService(
            fieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE, ordinal = 0),
            fieldCandidate(AtTargetOperationKind.FIELD_PUT_INSTANCE, ordinal = 1),
        )
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private String mcdevWrapGet(SimpleTarget simpleTarget, Operation<String> original) {
                return original.call(simpleTarget);
            }
        """)
        val site = sites(source).first()
        assertNull(service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget")))
    }

    @Test
    fun duplicateFieldOperationKindWithDistinctOrdinalsIsAccepted() {
        val service = fieldService(
            fieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE, ordinal = 0),
            fieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE, ordinal = 1),
        )
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private String mcdevWrapGet(SimpleTarget simpleTarget, Operation<String> original) {
                return original.call(simpleTarget);
            }
        """)
        val site = sites(source).first()
        assertNotNull(service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget")))
    }

    @Test
    fun wrapOperationFieldOrdinalSelectsRequestedOperationAndRejectsOutOfRange() {
        val service = fieldService(
            fieldCandidate(
                AtTargetOperationKind.FIELD_GET_INSTANCE,
                instructionOccurrenceIndex = 0,
            ),
            fieldCandidate(
                AtTargetOperationKind.FIELD_PUT_INSTANCE,
                instructionOccurrenceIndex = 1,
            ),
        )
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;", ordinal = 1))
        """)
        val site = sites(source).first()
        val mixinTargets = listOf("com/example/target/SimpleTarget")
        val spec = service.expectedSignature(source, site, mixinTargets)
        assertNotNull(spec)
        assertEquals("V", spec.returnTypeDescriptor)
        assertEquals(3, spec.parameters.size)
        assertEquals("V", spec.parameters.last().operationGenericDescriptor)

        val outOfRangeSource = source.replace("ordinal = 1", "ordinal = 2")
        val outOfRangeSite = sites(outOfRangeSource).first()
        assertNull(service.expectedSignature(outOfRangeSource, outOfRangeSite, mixinTargets))
        assertNull(service.generateHandlerStub(outOfRangeSource, outOfRangeSite, mixinTargets))
    }

    @Test
    fun unmatchedFieldCandidateReturnsNull() {
        val service = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE, name = "other"))
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private String mcdevWrapGet(SimpleTarget simpleTarget, Operation<String> original) {
                return original.call(simpleTarget);
            }
        """)
        val site = sites(source).first()
        assertNull(service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget")))
    }

    @Test
    fun fieldCandidateMissingOperationKindReturnsNull() {
        val service = fieldService(
            AtTargetCandidate(
                owner = "com/example/target/SimpleTarget",
                name = "label",
                descriptor = "Ljava/lang/String;",
                displayLabel = "label: String",
                detail = "SimpleTarget",
                kind = AtTargetKind.FIELD,
                ordinal = 0,
            ),
        )
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private String mcdevWrapGet(SimpleTarget simpleTarget, Operation<String> original) {
                return original.call(simpleTarget);
            }
        """)
        val site = sites(source).first()
        assertNull(service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget")))
    }

    @Test
    fun fieldTargetsWithoutBytecodeIndexReturnNull() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private String mcdevWrapGet(SimpleTarget simpleTarget, Operation<String> original) {
                return original.call(simpleTarget);
            }
        """)
        val site = sites(source).first()
        assertNull(service.expectedSignature(source, site, listOf("com/example/target/SimpleTarget")))
    }

    @Test
    fun validatesCorrectWrapOperationFieldGetHandler() {
        val fieldService = fieldService(fieldCandidate(AtTargetOperationKind.FIELD_GET_INSTANCE))
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "FIELD", target = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;"))
            private String mcdevWrapGet(SimpleTarget simpleTarget, Operation<String> original) {
                return original.call(simpleTarget);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        assertTrue(fieldService.validateHandler(source, site, listOf("com/example/target/SimpleTarget"), handler).isEmpty())
    }

    @Test
    fun operationSignatureRendererFormatsGeneric() {
        mapOf(
            "B" to "Operation<Byte>",
            "C" to "Operation<Character>",
            "D" to "Operation<Double>",
            "F" to "Operation<Float>",
            "I" to "Operation<Integer>",
            "J" to "Operation<Long>",
            "S" to "Operation<Short>",
            "Z" to "Operation<Boolean>",
            "V" to "Operation<Void>",
        ).forEach { (descriptor, expected) ->
            assertEquals(expected, OperationSignatureRenderer.renderOperationType(descriptor))
        }
        assertEquals("Operation<String>", OperationSignatureRenderer.renderOperationType("Ljava/lang/String;"))
        assertEquals("float", OperationSignatureRenderer.readableType("F"))
    }

    @Test
    fun mixinExtrasAnnotationMapsFromSimpleName() {
        assertEquals(MixinExtrasAnnotation.WRAP_OPERATION, MixinExtrasAnnotation.fromSimpleName("WrapOperation"))
        assertEquals(MixinExtrasAnnotation.EXPRESSION, MixinExtrasAnnotation.fromSimpleName("Expression"))
    }

    private fun newService(vararg candidates: AtTargetCandidate): HandlerSignatureService =
        HandlerSignatureService(
            MixinExtrasTestFixtures.classIndex,
            FakeBytecodeIndex(
                candidates = mapOf(
                    "com/example/target/SimpleTarget#draw#NEW" to candidates.toList(),
                ),
            ),
        )

    private fun newWrapSamplesOwner(): String = BytecodeFixtureCompiler.internalName("NewSamples")

    private fun generatedNewOwner(): String = "test/generated/NewWrapSamples"

    private fun newWrapClassIndex(
        mixinOwner: String = newWrapSamplesOwner(),
        methodName: String = "createObjects",
        methodDescriptor: String = "()V",
    ): ClassIndex = FakeClassIndex(
        classes = listOf(
            ClassIndexEntry(
                simpleName = mixinOwner.substringAfterLast('/'),
                packageName = mixinOwner.substringBeforeLast('/').replace('/', '.'),
                internalName = mixinOwner,
            ),
            ClassIndexEntry("StringBuilder", "java.lang", "java/lang/StringBuilder"),
        ),
        methods = mapOf(
            mixinOwner to listOf(
                MethodIndexEntry(methodName, methodDescriptor, false, "$methodName(): void"),
            ),
        ),
    )

    private fun newWrapService(
        vararg candidates: AtTargetCandidate,
        mixinOwner: String = newWrapSamplesOwner(),
        methodName: String = "createObjects",
        methodDescriptor: String = "()V",
        classBytes: ByteArray = BytecodeFixtureCompiler.classBytes("NewSamples"),
        provideClassBytes: Boolean = true,
    ): HandlerSignatureService = HandlerSignatureService(
        newWrapClassIndex(mixinOwner, methodName, methodDescriptor),
        object : BytecodeIndex {
            override fun getAtTargetCandidates(
                ownerInternalName: String,
                candidateMethodName: String,
                candidateMethodDescriptor: String?,
                atValue: String,
            ): List<AtTargetCandidate> =
                if (ownerInternalName == mixinOwner &&
                    candidateMethodName == methodName &&
                    candidateMethodDescriptor == methodDescriptor &&
                    atValue == "NEW"
                ) {
                    candidates.toList()
                } else {
                    emptyList()
                }

            override fun getReturnOrdinalCount(
                ownerInternalName: String,
                candidateMethodName: String,
                candidateMethodDescriptor: String?,
            ): Int = 1

            override fun getClassBytes(ownerInternalName: String): ByteArray? =
                if (provideClassBytes && ownerInternalName == mixinOwner) classBytes else null

            override fun resolveCommonSuperClass(type1Descriptor: String, type2Descriptor: String): String? = null
        },
    )

    private fun classBytesWithNewCalls(
        owner: String,
        methodName: String = "run",
        methodDescriptor: String = "()V",
        newCalls: List<Pair<String, String>>,
    ): ByteArray {
        val classWriter = org.objectweb.asm.ClassWriter(org.objectweb.asm.ClassWriter.COMPUTE_MAXS)
        classWriter.visit(
            org.objectweb.asm.Opcodes.V21,
            org.objectweb.asm.Opcodes.ACC_PUBLIC,
            owner,
            null,
            "java/lang/Object",
            null,
        )
        val methodVisitor = classWriter.visitMethod(
            org.objectweb.asm.Opcodes.ACC_PUBLIC,
            methodName,
            methodDescriptor,
            null,
            null,
        )
        methodVisitor.visitCode()
        for ((newOwner, initDescriptor) in newCalls) {
            methodVisitor.visitTypeInsn(org.objectweb.asm.Opcodes.NEW, newOwner)
            methodVisitor.visitInsn(org.objectweb.asm.Opcodes.DUP)
            for (argumentType in Type.getArgumentTypes(initDescriptor)) {
                when (argumentType.sort) {
                    Type.INT -> methodVisitor.visitIntInsn(org.objectweb.asm.Opcodes.BIPUSH, 42)
                    else -> error("unsupported constructor argument type: ${argumentType.descriptor}")
                }
            }
            methodVisitor.visitMethodInsn(
                org.objectweb.asm.Opcodes.INVOKESPECIAL,
                newOwner,
                "<init>",
                initDescriptor,
                false,
            )
        }
        methodVisitor.visitInsn(org.objectweb.asm.Opcodes.RETURN)
        methodVisitor.visitMaxs(0, 0)
        methodVisitor.visitEnd()
        classWriter.visitEnd()
        return classWriter.toByteArray()
    }

    private fun classBytesWithNestedSameOwnerNewCalls(
        owner: String,
        newOwner: String,
        outerInit: String,
        innerInit: String,
        methodName: String = "run",
        methodDescriptor: String = "()V",
    ): ByteArray {
        val classWriter = org.objectweb.asm.ClassWriter(org.objectweb.asm.ClassWriter.COMPUTE_MAXS)
        classWriter.visit(
            org.objectweb.asm.Opcodes.V21,
            org.objectweb.asm.Opcodes.ACC_PUBLIC,
            owner,
            null,
            "java/lang/Object",
            null,
        )
        val methodVisitor = classWriter.visitMethod(
            org.objectweb.asm.Opcodes.ACC_PUBLIC,
            methodName,
            methodDescriptor,
            null,
            null,
        )
        methodVisitor.visitCode()
        methodVisitor.visitTypeInsn(org.objectweb.asm.Opcodes.NEW, newOwner)
        methodVisitor.visitInsn(org.objectweb.asm.Opcodes.DUP)
        methodVisitor.visitTypeInsn(org.objectweb.asm.Opcodes.NEW, newOwner)
        methodVisitor.visitInsn(org.objectweb.asm.Opcodes.DUP)
        for (argumentType in Type.getArgumentTypes(innerInit)) {
            when (argumentType.sort) {
                Type.INT -> methodVisitor.visitIntInsn(org.objectweb.asm.Opcodes.BIPUSH, 42)
                else -> error("unsupported constructor argument type: ${argumentType.descriptor}")
            }
        }
        methodVisitor.visitMethodInsn(
            org.objectweb.asm.Opcodes.INVOKESPECIAL,
            newOwner,
            "<init>",
            innerInit,
            false,
        )
        methodVisitor.visitInsn(org.objectweb.asm.Opcodes.POP)
        for (argumentType in Type.getArgumentTypes(outerInit)) {
            when (argumentType.sort) {
                Type.INT -> methodVisitor.visitIntInsn(org.objectweb.asm.Opcodes.BIPUSH, 42)
                else -> error("unsupported constructor argument type: ${argumentType.descriptor}")
            }
        }
        methodVisitor.visitMethodInsn(
            org.objectweb.asm.Opcodes.INVOKESPECIAL,
            newOwner,
            "<init>",
            outerInit,
            false,
        )
        methodVisitor.visitInsn(org.objectweb.asm.Opcodes.POP)
        methodVisitor.visitInsn(org.objectweb.asm.Opcodes.RETURN)
        methodVisitor.visitMaxs(0, 0)
        methodVisitor.visitEnd()
        classWriter.visitEnd()
        return classWriter.toByteArray()
    }

    private fun classBytesWithSuperInitThenNew(
        owner: String,
        newOwner: String,
        initDescriptor: String,
        methodName: String = "<init>",
        methodDescriptor: String = "()V",
    ): ByteArray {
        val classWriter = org.objectweb.asm.ClassWriter(org.objectweb.asm.ClassWriter.COMPUTE_MAXS)
        classWriter.visit(
            org.objectweb.asm.Opcodes.V21,
            org.objectweb.asm.Opcodes.ACC_PUBLIC,
            owner,
            null,
            "java/lang/Object",
            null,
        )
        val methodVisitor = classWriter.visitMethod(
            org.objectweb.asm.Opcodes.ACC_PUBLIC,
            methodName,
            methodDescriptor,
            null,
            null,
        )
        methodVisitor.visitCode()
        methodVisitor.visitVarInsn(org.objectweb.asm.Opcodes.ALOAD, 0)
        methodVisitor.visitMethodInsn(
            org.objectweb.asm.Opcodes.INVOKESPECIAL,
            "java/lang/Object",
            "<init>",
            "()V",
            false,
        )
        methodVisitor.visitTypeInsn(org.objectweb.asm.Opcodes.NEW, newOwner)
        methodVisitor.visitInsn(org.objectweb.asm.Opcodes.DUP)
        for (argumentType in Type.getArgumentTypes(initDescriptor)) {
            when (argumentType.sort) {
                Type.INT -> methodVisitor.visitIntInsn(org.objectweb.asm.Opcodes.BIPUSH, 42)
                else -> error("unsupported constructor argument type: ${argumentType.descriptor}")
            }
        }
        methodVisitor.visitMethodInsn(
            org.objectweb.asm.Opcodes.INVOKESPECIAL,
            newOwner,
            "<init>",
            initDescriptor,
            false,
        )
        methodVisitor.visitInsn(org.objectweb.asm.Opcodes.RETURN)
        methodVisitor.visitMaxs(0, 0)
        methodVisitor.visitEnd()
        classWriter.visitEnd()
        return classWriter.toByteArray()
    }

    private fun classBytesWithUnpairedNew(owner: String): ByteArray {
        val classWriter = org.objectweb.asm.ClassWriter(org.objectweb.asm.ClassWriter.COMPUTE_MAXS)
        classWriter.visit(
            org.objectweb.asm.Opcodes.V21,
            org.objectweb.asm.Opcodes.ACC_PUBLIC,
            owner,
            null,
            "java/lang/Object",
            null,
        )
        val methodVisitor = classWriter.visitMethod(
            org.objectweb.asm.Opcodes.ACC_PUBLIC,
            "run",
            "()V",
            null,
            null,
        )
        methodVisitor.visitCode()
        methodVisitor.visitTypeInsn(org.objectweb.asm.Opcodes.NEW, "java/lang/StringBuilder")
        methodVisitor.visitInsn(org.objectweb.asm.Opcodes.RETURN)
        methodVisitor.visitMaxs(0, 0)
        methodVisitor.visitEnd()
        classWriter.visitEnd()
        return classWriter.toByteArray()
    }

    private fun newCandidate(
        owner: String,
        name: String = "",
        descriptor: String = "L$owner;",
        ordinal: Int = 0,
    ): AtTargetCandidate = AtTargetCandidate(
        owner = owner,
        name = name,
        descriptor = descriptor,
        displayLabel = owner.substringAfterLast('/'),
        detail = owner.substringAfterLast('/'),
        kind = AtTargetKind.NEW,
        ordinal = ordinal,
    )

    private fun fieldService(vararg candidates: AtTargetCandidate): HandlerSignatureService =
        HandlerSignatureService(
            MixinExtrasTestFixtures.classIndex,
            FakeBytecodeIndex(
                candidates = mapOf(
                    "com/example/target/SimpleTarget#draw#FIELD" to candidates.toList(),
                ),
            ),
        )

    private fun invokeService(
        vararg candidates: AtTargetCandidate,
        key: String = "com/example/target/SimpleTarget#draw#INVOKE",
    ): HandlerSignatureService =
        HandlerSignatureService(
            MixinExtrasTestFixtures.classIndex,
            FakeBytecodeIndex(
                candidates = mapOf(key to candidates.toList()),
            ),
        )

    private fun wrapWithConditionInvokeService(vararg candidates: AtTargetCandidate): HandlerSignatureService =
        invokeService(*candidates)

    private fun defaultWrapWithConditionInvokeService(): HandlerSignatureService =
        wrapWithConditionInvokeService(
            wrapWithConditionInvokeCandidate(AtTargetOperationKind.INVOKE_VIRTUAL),
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

    private fun invokeCandidate(
        operationKind: AtTargetOperationKind,
        owner: String = "java/lang/String",
        name: String = "length",
        descriptor: String = "()I",
        ordinal: Int = 0,
    ): AtTargetCandidate = AtTargetCandidate(
        owner = owner,
        name = name,
        descriptor = descriptor,
        displayLabel = "$name(): int",
        detail = owner.substringAfterLast('/'),
        kind = AtTargetKind.INVOKE,
        ordinal = ordinal,
        operationKind = operationKind,
    )

    private fun fieldCandidate(
        operationKind: AtTargetOperationKind,
        name: String = "label",
        descriptor: String = "Ljava/lang/String;",
        owner: String = "com/example/target/SimpleTarget",
        ordinal: Int = 0,
        instructionOccurrenceIndex: Int = -1,
    ): AtTargetCandidate = AtTargetCandidate(
        owner = owner,
        name = name,
        descriptor = descriptor,
        displayLabel = "$name: type",
        detail = "SimpleTarget",
        kind = AtTargetKind.FIELD,
        ordinal = ordinal,
        operationKind = operationKind,
        instructionOccurrenceIndex = instructionOccurrenceIndex,
    )

    private fun trimmedSource(fixture: String): String = fixture.trimIndent()

    private fun expressionHandlerSource(prefix: String, at: String, suffix: String): String = """
        @Mixin(com.example.target.SimpleTarget.class)
        abstract class ExampleMixin {
            $prefix
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = $at)
            $suffix
        }
    """.trimIndent()

    private fun expressionMatchSamplesHandlerSource(
        suffix: String = "private String mcdevHandler(String original) { return original; }",
    ): String = """
        @Mixin(ExpressionMatchSamples.class)
        abstract class ExampleMixin {
            @Definition(id = "sampleFieldRef", field = "Lio/github/mcdev/core/bytecode/fixtures/ExpressionMatchSamples;sampleField:I")
            @Expression(id = "main", value = "this.label")
            @ModifyExpressionValue(method = "readSampleField()I", at = @At(id = "main", value = "MIXINEXTRAS:EXPRESSION"))
            $suffix
        }
    """.trimIndent()

    private fun resolvedIntSampleFieldContext(): ExpressionContext = ExpressionContext(
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
    )

    private fun expressionMatchSamplesOwner(): String =
        BytecodeFixtureCompiler.internalName("ExpressionMatchSamples")

    private fun invokeSamplesOwner(): String =
        BytecodeFixtureCompiler.internalName("InvokeSamples")

    private fun invokeSamplesClassIndex(): ClassIndex {
        val owner = invokeSamplesOwner()
        return FakeClassIndex(
            classes = listOf(
                ClassIndexEntry(
                    "InvokeSamples",
                    "io.github.mcdev.core.bytecode.fixtures",
                    owner,
                ),
            ),
            methods = mapOf(
                owner to listOf(
                    MethodIndexEntry("virtualInvoke", "()V", false, "virtualInvoke(): void"),
                    MethodIndexEntry("interfaceInvoke", "()V", false, "interfaceInvoke(): void"),
                ),
            ),
        )
    }

    private fun invokeSamplesBytecodeIndex(): BytecodeIndex {
        val owner = invokeSamplesOwner()
        val classBytes = BytecodeFixtureCompiler.classBytes("InvokeSamples")
        val commonSuperClassResolver = BytecodeCommonSuperClassResolver(
            classBytesLookup = { internalName ->
                if (internalName == owner) classBytes else null
            },
        )
        val delegate = FakeBytecodeIndex()
        return object : BytecodeIndex by delegate {
            override fun getClassBytes(ownerInternalName: String): ByteArray? =
                if (ownerInternalName == owner) classBytes else null

            override fun resolveCommonSuperClass(type1Descriptor: String, type2Descriptor: String): String? =
                commonSuperClassResolver.resolve(type1Descriptor, type2Descriptor)
                    ?: expressionMatchSamplesFallbackCommonSuper(type1Descriptor, type2Descriptor)
        }
    }

    private fun expressionMatchSamplesClassIndex(): ClassIndex {
        val owner = expressionMatchSamplesOwner()
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
                    MethodIndexEntry("readSampleField", "()I", false, "readSampleField(): int"),
                    MethodIndexEntry("writeSampleField", "(I)V", false, "writeSampleField(int): void"),
                    MethodIndexEntry("add", "(II)I", false, "add(int, int): int"),
                    MethodIndexEntry("intEqualsZero", "(I)Z", false, "intEqualsZero(int): boolean"),
                    MethodIndexEntry("stringConcat", "(I)Ljava/lang/String;", false, "stringConcat(int): String"),
                    MethodIndexEntry("trim", "(Ljava/lang/String;)Ljava/lang/String;", false, "trim(String): String"),
                    MethodIndexEntry("arrayAccess", "([II)V", false, "arrayAccess(int[], int): void"),
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

    private fun sites(source: String) = HandlerSignatureService.findAnnotationSites(source)

    private fun enrich(handler: HandlerMethodDeclaration) =
        HandlerSignatureService.enrichHandlerTypes(handler, classIndex)

    private fun parseHandlerParameter(parameter: String): HandlerParameterDeclaration {
        val handler = HandlerSignatureService.parseHandlerMethod("void handler($parameter) {}", 0)
        assertNotNull(handler)
        return handler.parameters.single()
    }

    private fun sourceSubstring(source: String, range: McTextRange): String =
        source.substring(rangeStart(source, range), rangeEnd(source, range))

    private fun rangeStart(source: String, range: McTextRange): Int {
        val lineStart = source.lineSequence().take(range.start.line).sumOf { it.length + 1 }
        return lineStart + range.start.character
    }

    private fun rangeEnd(source: String, range: McTextRange): Int {
        val lineStart = source.lineSequence().take(range.end.line).sumOf { it.length + 1 }
        return lineStart + range.end.character
    }

    private class CountingClassIndex(
        private val delegate: ClassIndex,
    ) : ClassIndex {
        var emptyPrefixFindClassesCalls = 0
        var getMethodsCalls = 0

        override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> {
            if (prefix.isEmpty()) emptyPrefixFindClassesCalls++
            return delegate.findClasses(prefix, limit)
        }

        override fun findClass(internalName: String): ClassIndexEntry? =
            delegate.findClass(internalName)

        override fun findClassByFqn(fqn: String): ClassIndexEntry? =
            delegate.findClassByFqn(fqn)

        override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> {
            getMethodsCalls++
            return delegate.getMethods(ownerInternalName)
        }

        override fun getFields(ownerInternalName: String): List<FieldIndexEntry> =
            delegate.getFields(ownerInternalName)
    }
}
