package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mixin.AnnotationContextExtractor
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

class OperationCallValidatorTest {
    private val classIndex = MixinExtrasTestFixtures.classIndex
    private val bytecodeIndex = MixinExtrasTestFixtures.bytecodeIndex
    private val signatureService = HandlerSignatureService(classIndex, bytecodeIndex)
    private val mixinTargets = listOf("com/example/target/SimpleTarget")

    @Test
    fun acceptsValidWrapOperationCallArity() {
        val source = trimmedSource(MixinExtrasTestFixtures.WRAP_OPERATION_SOURCE)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun reportsWrongWrapOperationCallArity() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original) {
                return original.call();
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        val issues = validate(source, site, handler)
        assertEquals(1, issues.size)
        assertEquals(listOf("instance"), issues.single().expectedNames)
        assertOffsetRange(source, issues.single().argumentRange, "original.call(", ")")
    }

    @Test
    fun acceptsValidWrapMethodCallArity() {
        val source = trimmedSource(MixinExtrasTestFixtures.WRAP_METHOD_SOURCE)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun acceptsWrapMethodCallWithReceiverExtraHandlerParameter() {
        val source = trimmedSource("""
            @WrapMethod(method = "draw(Ljava/lang/String;FF)V")
            private void mcdevWrapDraw(SimpleTarget instance, String arg0, float arg1, float arg2, Operation<Void> original) {
                original.call(instance, arg0, arg1, arg2);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun excludesTrailingCaptureAndLocalFromExpectedCallArgs() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original, String arg0, @Local int counter) {
                return original.call(instance);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())

        val wrongSource = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original, String arg0, @Local int counter) {
                return original.call(instance, arg0);
            }
        """)
        val wrongSite = sites(wrongSource).first()
        val wrongHandler = enrich(wrongSite.handlerMethod!!)
        val issues = validate(wrongSource, wrongSite, wrongHandler)
        assertEquals(1, issues.size)
        assertEquals(listOf("instance"), issues.single().expectedNames)
    }

    @Test
    fun ignoresQualifiedOperationCallReceivers() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original) {
                this.original.call();
                other.original.call(instance, instance);
                return original.call(instance);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun ignoresCommasInsideBlockCommentsWhenCountingCallArguments() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original) {
                return original.call(instance /* , extra */);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun ignoresCommasInsideLineCommentsWhenCountingCallArguments() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original) {
                return original.call(instance // , extra
                );
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun treatsCommentLikeTextInsideStringAndCharLiteralsAsLiteral() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original) {
                return original.call("/* , */");
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())

        val charSource = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original) {
                return original.call(',');
            }
        """)
        val charSite = sites(charSource).first()
        val charHandler = enrich(charSite.handlerMethod!!)

        assertTrue(validate(charSource, charSite, charHandler).isEmpty())
    }

    @Test
    fun ignoresOperationCallsInsideCommentsAndStrings() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original) {
                // original.call()
                /* original.call(instance, instance) */
                String fake = "original.call(instance, instance)";
                char marker = 'original.call(instance)';
                return original.call(instance);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun handlesNestedCommasInsideCallArguments() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;charAt(I)C"))
            private char mcdevWrapChar(String instance, int arg0, Operation<Character> original) {
                return original.call(instance, nested(1, 2));
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())

        val wrongSource = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;charAt(I)C"))
            private char mcdevWrapChar(String instance, int arg0, Operation<Character> original) {
                return original.call(nested(1, 2));
            }
        """)
        val wrongSite = sites(wrongSource).first()
        val wrongHandler = enrich(wrongSite.handlerMethod!!)
        val issues = validate(wrongSource, wrongSite, wrongHandler)
        assertEquals(1, issues.size)
        assertEquals(listOf("instance", "arg0"), issues.single().expectedNames)
    }

    @Test
    fun reportsEachWrongCallIndependently() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original) {
                original.call();
                return original.call(instance, instance);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        val issues = validate(source, site, handler)
        assertEquals(2, issues.size)
        assertEquals(listOf("instance"), issues[0].expectedNames)
        assertEquals(listOf("instance"), issues[1].expectedNames)

        val firstOpen = source.indexOf("original.call(") + "original.call(".length
        val firstClose = source.indexOf(')', firstOpen)
        assertEquals(firstOpen, rangeStart(source, issues[0].argumentRange))
        assertEquals(firstOpen, rangeEnd(source, issues[0].argumentRange))

        val secondOpen = source.indexOf("original.call(", firstClose) + "original.call(".length
        val secondClose = source.indexOf(')', secondOpen)
        assertEquals(secondOpen, rangeStart(source, issues[1].argumentRange))
        assertEquals(secondClose, rangeEnd(source, issues[1].argumentRange))
    }

    @Test
    fun returnsEmptyWhenHandlerHasNoOperationParameter() {
        val source = trimmedSource(MixinExtrasTestFixtures.WRAP_OPERATION_MISSING_OP)
        val site = sites(source).first()
        val handler = site.handlerMethod!!
        assertTrue(handler.parameters.none { it.isOperation })

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun returnsEmptyForNonWrapAnnotation() {
        val source = trimmedSource(MixinExtrasTestFixtures.MODIFY_EXPRESSION_SOURCE)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun ignoresCallsWhenAbstractHandlerHasNoBody() {
        val source = trimmedSource("""
            @Mixin(SimpleTarget.class)
            abstract class ExampleMixin {
                @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
                abstract int mcdevWrapLength(String instance, Operation<Integer> original);

                private int mcdevOther(String instance, Operation<Integer> original) {
                    return original.call();
                }
            }
        """)
        val handlerStart = source.indexOf("abstract int mcdevWrapLength")
        val semicolonIndex = source.indexOf(';', handlerStart)
        val handlerEndExclusive = source.lastIndexOf(')', semicolonIndex) + 1
        val handler = enrich(
            HandlerMethodDeclaration(
                methodName = "mcdevWrapLength",
                returnTypeName = "int",
                returnTypeDescriptor = null,
                parameters = listOf(
                    HandlerParameterDeclaration(
                        name = "instance",
                        typeName = "String",
                        typeDescriptor = null,
                        isOperation = false,
                        operationGenericName = null,
                    ),
                    HandlerParameterDeclaration(
                        name = "original",
                        typeName = "Operation<Integer>",
                        typeDescriptor = null,
                        isOperation = true,
                        operationGenericName = "Integer",
                    ),
                ),
                range = offsetRange(source, handlerStart, handlerEndExclusive),
            ),
        )
        val site = sites(source).first().copy(handlerMethod = handler)

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun countsComparisonLessThanAsSingleArgument() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;charAt(I)C"))
            private char mcdevWrapChar(String instance, int arg0, Operation<Character> original) {
                return original.call(instance < arg0, arg0);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun countsComparisonGreaterThanAsSingleArgument() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;charAt(I)C"))
            private char mcdevWrapChar(String instance, int arg0, Operation<Character> original) {
                return original.call(instance > arg0, arg0);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun countsBalancedComparisonChainAsSingleArgument() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;charAt(I)C"))
            private char mcdevWrapChar(String instance, int arg0, Operation<Character> original) {
                return original.call(instance < arg0 > 0, arg0);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun acceptsComparisonOperandsSplitByTopLevelCommaWhenArityIsTwo() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;charAt(I)C"))
            private char mcdevWrapChar(String instance, int arg0, Operation<Character> original) {
                return original.call(instance < arg0, arg0 > 0);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun reportsWrongArityWhenComparisonCommaIsHiddenByGenericHeuristic() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original) {
                return original.call(instance < arg1, arg1 > 0);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        val issues = validate(source, site, handler)
        assertEquals(1, issues.size)
        assertEquals(listOf("instance"), issues.single().expectedNames)
    }

    @Test
    fun reportsWrongArityWhenComparisonChainHidesTopLevelComma() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original) {
                return original.call(instance < arg1 > 0, arg1);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        val issues = validate(source, site, handler)
        assertEquals(1, issues.size)
        assertEquals(listOf("instance"), issues.single().expectedNames)
    }

    @Test
    fun countsChainedComparisonWithoutSpacesAsSingleArgument() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;charAt(I)C"))
            private char mcdevWrapChar(String instance, int arg0, Operation<Character> original) {
                return original.call(instance<arg0>0, arg0);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun preservesNestedGenericTypeCommas() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;charAt(I)C"))
            private char mcdevWrapChar(String instance, int arg0, Operation<Character> original) {
                return original.call(new java.util.HashMap<String, java.util.List<Integer>>(), arg0);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun preservesGenericCastCommas() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original) {
                return original.call((java.util.List<String>) instance);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun preservesExplicitGenericMethodInvocationCommas() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original) {
                return original.call(helper.<String, Integer>run(instance));
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun preservesGenericTypeCommasInNewExpressionArguments() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;charAt(I)C"))
            private char mcdevWrapChar(String instance, int arg0, Operation<Character> original) {
                return original.call(new java.util.HashMap<String, Integer>(), arg0);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun returnsEmptyWhenLayoutIsUnresolved() {
        val source = trimmedSource(MixinExtrasTestFixtures.WRAP_OPERATION_OP_NOT_LAST)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun returnsEmptyWhenSugarInterleavesRequiredParametersForWrapOperation() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, @Local int counter, Operation<Integer> original) {
                return original.call();
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val expected = signatureService.expectedSignature(source, site, mixinTargets)

        assertNull(OperationCallValidator.resolveOperationCallLayout(site, handler, expected))
        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun returnsEmptyWhenSugarInterleavesRequiredParametersForWrapMethod() {
        val source = trimmedSource("""
            @WrapMethod(method = "draw(Ljava/lang/String;FF)V")
            private void mcdevWrapDraw(String arg0, @Local int counter, float arg1, float arg2, Operation<Void> original) {
                original.call();
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val expected = signatureService.expectedSignature(source, site, mixinTargets)

        assertNull(OperationCallValidator.resolveOperationCallLayout(site, handler, expected))
        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun returnsEmptyWhenExpectedSignatureIsUnavailable() {
        val source = trimmedSource("""
            @WrapOperation(method = "unknown()V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private void mcdevWrap(String instance, Operation<Void> original) {
                original.call();
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun returnsEmptyWhenHandlerHasExtraRequiredParameterBeforeOperation() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, float extra, Operation<Integer> original) {
                return original.call();
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun returnsEmptyWhenHandlerHasMultipleOperationParameters() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original, Operation<Integer> duplicate) {
                return original.call();
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun validatesDespiteWrongReturnTypeWhenLayoutIsUnambiguous() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private void mcdevWrapLength(String instance, Operation<Integer> original) {
                original.call();
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        val issues = validate(source, site, handler)
        assertEquals(1, issues.size)
        assertEquals(listOf("instance"), issues.single().expectedNames)
    }

    @Test
    fun expectedCallArgNamesUseHandlerParameterNamesNotSpecPlaceholders() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;charAt(I)C"))
            private char mcdevWrapChar(String receiver, int value, Operation<Character> original) {
                return original.call();
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)
        val expected = signatureService.expectedSignature(source, site, mixinTargets)

        val layout = OperationCallValidator.resolveOperationCallLayout(site, handler, expected)
        assertEquals(listOf("receiver", "value"), layout?.expectedCallArgNames)

        val issues = validate(source, site, handler)
        assertEquals(1, issues.size)
        assertEquals(listOf("receiver", "value"), issues.single().expectedNames)
    }

    @Test
    fun returnsEmptyWhenCallArgParameterNameIsInvalid() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original) {
                return original.call();
            }
        """)
        val site = sites(source).first()
        val enriched = enrich(site.handlerMethod!!)
        val handler = enriched.copy(
            parameters = enriched.parameters.map { param ->
                if (!param.isOperation && param.name == "instance") {
                    param.copy(name = "123invalid")
                } else {
                    param
                }
            },
        )

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun returnsEmptyWhenCallArgParameterNameIsBlank() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original) {
                return original.call();
            }
        """)
        val site = sites(source).first()
        val enriched = enrich(site.handlerMethod!!)
        val handler = enriched.copy(
            parameters = enriched.parameters.map { param ->
                if (!param.isOperation && param.name == "instance") {
                    param.copy(name = "")
                } else {
                    param
                }
            },
        )

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun crlfDiagnosticRangeMatchesActualSourceIndex() {
        val source = crlfSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original) {
                return original.call();
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        val issues = validate(source, site, handler)
        assertEquals(1, issues.size)

        val callPrefix = "original.call("
        val openIndex = source.indexOf(callPrefix) + callPrefix.length
        val closeIndex = source.indexOf(')', openIndex)
        val issue = issues.single()
        assertEquals(openIndex, toOffset(source, issue.argumentRange.start))
        assertEquals(openIndex, toOffset(source, issue.argumentRange.end))
        assertEquals(listOf("instance"), issue.expectedNames)
        assertTrue(closeIndex == openIndex)
    }

    @Test
    fun reportsWrongArgumentTypesAtSameArityWhenHandlerParametersAreSwapped() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;charAt(I)C"))
            private char mcdevWrapChar(String instance, int index, Operation<Character> original) {
                return original.call(index, instance);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        val issues = validate(source, site, handler)
        assertEquals(1, issues.size)
        assertEquals(listOf("instance", "index"), issues.single().expectedNames)
    }

    @Test
    fun acceptsCorrectArgumentOrderAtSameArity() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;charAt(I)C"))
            private char mcdevWrapChar(String instance, int index, Operation<Character> original) {
                return original.call(instance, index);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun acceptsPrimitiveAndCorrespondingBoxedWrapperAtSameArity() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;charAt(I)C"))
            private char mcdevWrapChar(String instance, int index, Operation<Character> original, Integer boxed) {
                return original.call(instance, boxed);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun reportsIncompatiblePrimitiveFamiliesAtSameArity() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;charAt(I)C"))
            private char mcdevWrapChar(String instance, int index, Operation<Character> original, long counter) {
                return original.call(instance, counter);
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        val issues = validate(source, site, handler)
        assertEquals(1, issues.size)
        assertEquals(listOf("instance", "index"), issues.single().expectedNames)
    }

    @Test
    fun ignoresUnknownCallArgumentExpressionsAtSameArity() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;charAt(I)C"))
            private char mcdevWrapChar(String instance, int index, Operation<Character> original) {
                return original.call(instance, computeIndex());
            }
        """)
        val site = sites(source).first()
        val handler = enrich(site.handlerMethod!!)

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun ignoresSameArityCallWhenHandlerParameterDescriptorIsNull() {
        val source = trimmedSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;charAt(I)C"))
            private char mcdevWrapChar(String instance, int index, Operation<Character> original) {
                return original.call(instance, index);
            }
        """)
        val site = sites(source).first()
        val enriched = enrich(site.handlerMethod!!)
        val handler = enriched.copy(
            parameters = enriched.parameters.map { param ->
                if (param.name == "index") param.copy(typeDescriptor = null) else param
            },
        )

        assertTrue(validate(source, site, handler).isEmpty())
    }

    @Test
    fun crlfTwoHandlersDoNotCrossCapture() {
        val source = crlfSource("""
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLengthA(String instance, Operation<Integer> original) {
                return original.call(instance);
            }

            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLengthB(String instance, Operation<Integer> original) {
                return original.call();
            }
        """)
        val sites = sites(source)
        assertEquals(2, sites.size)

        val issuesA = validate(source, sites[0], enrich(sites[0].handlerMethod!!))
        assertTrue(issuesA.isEmpty())

        val issuesB = validate(source, sites[1], enrich(sites[1].handlerMethod!!))
        assertEquals(1, issuesB.size)
        assertEquals(listOf("instance"), issuesB.single().expectedNames)

        val callPrefix = "original.call("
        val secondCallOpen = source.indexOf(callPrefix, source.indexOf("mcdevWrapLengthB"))
        val contentStart = secondCallOpen + callPrefix.length
        assertEquals(contentStart, toOffset(source, issuesB.single().argumentRange.start))
        assertEquals(contentStart, toOffset(source, issuesB.single().argumentRange.end))
    }

    private fun crlfSource(block: String): String = block.trimIndent().replace("\n", "\r\n")

    private fun toOffset(source: String, position: McTextPosition): Int =
        AnnotationContextExtractor.toOffset(source, position.line, position.character)!!

    private fun trimmedSource(block: String): String = block.trimIndent()

    private fun sites(source: String) = HandlerSignatureService.findAnnotationSites(source)

    private fun enrich(handler: HandlerMethodDeclaration) =
        HandlerSignatureService.enrichHandlerTypes(handler, classIndex)

    private fun validate(
        source: String,
        site: MixinExtrasAnnotationSite,
        handler: HandlerMethodDeclaration,
    ): List<OperationCallIssue> {
        val expected = signatureService.expectedSignature(source, site, mixinTargets)
        val layout = OperationCallValidator.resolveOperationCallLayout(site, handler, expected)
        return OperationCallValidator.validate(source, site, layout)
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

    private fun offsetRange(source: String, start: Int, end: Int): McTextRange =
        McTextRange(offsetToPosition(source, start), offsetToPosition(source, end))

    private fun offsetToPosition(source: String, offset: Int): McTextPosition {
        var line = 0
        var character = 0
        var index = 0
        while (index < offset && index < source.length) {
            if (source[index] == '\n') {
                line++
                character = 0
            } else {
                character++
            }
            index++
        }
        return McTextPosition(line, character)
    }
}
