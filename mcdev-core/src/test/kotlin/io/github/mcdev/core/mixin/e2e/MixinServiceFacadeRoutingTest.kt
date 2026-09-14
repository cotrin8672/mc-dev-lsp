package io.github.mcdev.core.mixin.e2e

import io.github.mcdev.core.bytecode.BytecodeFixtureCompiler
import io.github.mcdev.core.bytecode.InstructionExtractor
import io.github.mcdev.core.bytecode.OccurrenceResultClassification
import io.github.mcdev.core.index.BytecodeIndexEntryMapper
import io.github.mcdev.core.bytecode.AtTargetKind as BytecodeAtTargetKind
import io.github.mcdev.core.mixin.AnnotationContextExtractor
import io.github.mcdev.core.mixin.AnnotationContext
import io.github.mcdev.core.mixin.AnnotationSlot
import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.AtTargetKind
import io.github.mcdev.core.mixin.AtTargetOperationKind
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FakeBytecodeIndex
import io.github.mcdev.core.mixin.FakeClassIndex
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.mixin.MixinAnnotation
import io.github.mcdev.core.mixin.MixinClassModel
import io.github.mcdev.core.mixin.MixinCompletionOptions
import io.github.mcdev.core.mixin.MixinFacadeRequest
import io.github.mcdev.core.mixin.MixinServiceFacade
import io.github.mcdev.core.mixinextras.BytecodeCommonSuperClassResolver
import io.github.mcdev.core.mixinextras.MixinExtrasDiagnosticCodes
import io.github.mcdev.core.mixinextras.OfficialExpressionCancellationChecker
import io.github.mcdev.core.mixinextras.OfficialExpressionCancellationContext
import io.github.mcdev.core.model.MappingNamespace
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.objectweb.asm.ClassReader
import org.objectweb.asm.Type
import org.objectweb.asm.tree.AbstractInsnNode
import org.objectweb.asm.tree.ClassNode
import org.objectweb.asm.tree.MethodInsnNode

class MixinServiceFacadeRoutingTest {
    private val facade = MixinE2ETestSupport.fakeFacade()

    private class FacadeLocalCaptureCancellation : RuntimeException("facade local capture cancelled")

    @Test
    fun routesModifyConstantLocalOrdinalValueCompletionFromConstantSamples() {
        val helloSource = modifyConstantLocalCaptureMixinSource(
            localAttribute = "ordinal",
            constantSelector = """@Constant(stringValue = "hello")""",
            originalType = "String",
        )
        val parsed = io.github.mcdev.core.mixinextras.HandlerSignatureService
            .findSugarHandlerAnnotationSites(helloSource)
            .single()
        assertEquals("CONSTANT", parsed.atValue)
        assertEquals(listOf("stringValue=hello"), parsed.atArgs)

        val liveLocalSource = modifyConstantLocalCaptureMixinSource(
            localAttribute = "ordinal",
            constantSelector = "@Constant(intValue = 42)",
            originalType = "int",
        )
        val items = constantSamplesFacade().complete(
            requestInLocalAttributeValue(liveLocalSource, "ordinal"),
        )
        assertEquals(listOf("0"), items.map { it.insertText })
        assertTrue(items.all { it.metadata.source == "mixinextras.local" })
    }

    @Test
    fun routesLocalOrdinalValueCompletionFromFixtureSnapshots() {
        val source = localCaptureMixinSource("ordinal")
        val items = localCaptureSamplesFacade().complete(
            requestInLocalAttributeValue(source, "ordinal"),
        )
        assertEquals(listOf("0", "1"), items.map { it.insertText })
        assertTrue(items.all { it.metadata.source == "mixinextras.local" })
    }

    @Test
    fun routesLocalIndexValueCompletionFromFixtureSnapshots() {
        val source = localCaptureMixinSource("index")
        val items = localCaptureSamplesFacade().complete(
            requestInLocalAttributeValue(source, "index"),
        )
        assertEquals(listOf("2", "3"), items.map { it.insertText })
    }

    @Test
    fun handlerParameterLocalAttributeCompletionOmitsType() {
        val source = handlerParameterLocalAttributeSource()
        val items = localCaptureSamplesFacade().complete(requestInLocalAttributeName(source))
        val attributeNames = items.mapNotNull { it.metadata.name }.toSet()
        assertEquals(setOf("print", "ordinal", "index", "name", "argsOnly"), attributeNames)
        assertTrue(items.all { it.metadata.source == "mixin.attribute" })
    }

    @Test
    fun wrapMethodLocalCompletionFailsClosed() {
        val source = wrapMethodLocalCaptureMixinSource()
        val facade = localCaptureSamplesFacade()
        assertTrue(facade.complete(requestInLocalAttributeName(source)).isEmpty())
        assertTrue(facade.complete(requestInLocalAttributeValue(source, "ordinal")).isEmpty())

        val diagnostics = facade.diagnose(localCaptureDiagnoseRequest(source))
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.UNSUPPORTED_SUGAR_PARAMETER })
        assertTrue(localCaptureDiagnostics(diagnostics).isEmpty())
    }

    @Test
    fun definitionNestedLocalAttributeCompletionIncludesType() {
        val source = definitionNestedLocalAttributeSource()
        val items = facade.complete(requestInNestedDefinitionLocalAttributeName(source))
        val attributeNames = items.mapNotNull { it.metadata.name }.toSet()
        assertTrue("type" in attributeNames)
        assertTrue(items.all { it.metadata.source == "mixin.attribute" })
    }

    @Test
    fun localValueCompletionFailsClosedWithoutBytecode() {
        val source = localCaptureMixinSource("ordinal")
        val facade = MixinServiceFacade(localCaptureSamplesClassIndex(), FakeBytecodeIndex())
        val items = facade.complete(requestInLocalAttributeValue(source, "ordinal"))
        assertTrue(items.isEmpty())
    }

    @Test
    fun cancelledLocalCaptureCompletionPropagatesAndNextRequestRecovers() {
        val source = localCaptureMixinSource("ordinal")
        val facade = localCaptureSamplesFacade()
        val cancellation = FacadeLocalCaptureCancellation()

        val thrown = assertFailsWith<FacadeLocalCaptureCancellation> {
            OfficialExpressionCancellationContext.withChecker(
                OfficialExpressionCancellationChecker { throw cancellation },
            ) {
                facade.complete(requestInLocalAttributeValue(source, "ordinal"))
            }
        }
        assertTrue(thrown === cancellation)

        val recovered = facade.complete(requestInLocalAttributeValue(source, "ordinal"))
        assertEquals(listOf("0", "1"), recovered.map { it.insertText })
    }

    @Test
    fun diagnoseLocalCaptureIndex99ReportsNotFound() {
        val source = localCaptureMixinSource("index").replace("index = ", "index = 99")
        val diagnostics = localCaptureSamplesFacade().diagnose(localCaptureDiagnoseRequest(source))
        val localDiagnostics = localCaptureDiagnostics(diagnostics)
        assertEquals(1, localDiagnostics.size)
        assertEquals(MixinExtrasDiagnosticCodes.LOCAL_CAPTURE_NOT_FOUND, localDiagnostics.single().code)
    }

    @Test
    fun diagnoseImplicitLocalCaptureReportsAmbiguous() {
        val source = localCaptureMixinSource("index").replace("@Local(index = )", "@Local")
        val diagnostics = localCaptureSamplesFacade().diagnose(localCaptureDiagnoseRequest(source))
        val localDiagnostics = localCaptureDiagnostics(diagnostics)
        assertEquals(1, localDiagnostics.size)
        assertEquals(MixinExtrasDiagnosticCodes.LOCAL_CAPTURE_AMBIGUOUS, localDiagnostics.single().code)
    }

    @Test
    fun diagnoseLocalCaptureIndex3ProducesNoLocalCaptureDiagnostics() {
        val source = localCaptureMixinSource("index").replace("index = ", "index = 3")
        val diagnostics = localCaptureSamplesFacade().diagnose(localCaptureDiagnoseRequest(source))
        assertTrue(localCaptureDiagnostics(diagnostics).isEmpty())
    }

    @Test
    fun diagnoseLocalCaptureFailsClosedWithoutBytecode() {
        val source = localCaptureMixinSource("index").replace("index = ", "index = 99")
        val facade = MixinServiceFacade(localCaptureSamplesClassIndex(), FakeBytecodeIndex())
        val diagnostics = facade.diagnose(localCaptureDiagnoseRequest(source))
        assertTrue(localCaptureDiagnostics(diagnostics).isEmpty())
    }

    @Test
    fun routesInjectLocalOrdinalValueCompletionFromFixtureSnapshots() {
        val source = standardInjectLocalCaptureMixinSource("ordinal")
        val items = localCaptureSamplesFacade().complete(
            requestInLocalAttributeValue(source, "ordinal"),
        )
        assertEquals(listOf("0", "1"), items.map { it.insertText })
        assertTrue(items.all { it.metadata.source == "mixinextras.local" })
    }

    @Test
    fun routesInjectLocalOrdinalValueCompletionWhenEarlierMethodSelectorIsUnresolved() {
        val source = standardInjectLocalCaptureMixinSource("ordinal").replace(
            """method = "instanceWithArgs(Ljava/lang/String;I)I"""",
            """method = { "missing()V", "instanceWithArgs(Ljava/lang/String;I)I" }""",
        )
        val items = localCaptureSamplesFacade().complete(
            requestInLocalAttributeValue(source, "ordinal"),
        )
        assertEquals(listOf("0", "1"), items.map { it.insertText })
        assertTrue(items.all { it.metadata.source == "mixinextras.local" })
    }

    @Test
    fun diagnoseInjectLocalCaptureIndex99ReportsNotFound() {
        val source = standardInjectLocalCaptureMixinSource("index").replace("index = ", "index = 99")
        val diagnostics = localCaptureSamplesFacade().diagnose(localCaptureDiagnoseRequest(source))
        val localDiagnostics = localCaptureDiagnostics(diagnostics)
        assertEquals(1, localDiagnostics.size)
        assertEquals(MixinExtrasDiagnosticCodes.LOCAL_CAPTURE_NOT_FOUND, localDiagnostics.single().code)
    }

    @Test
    fun diagnoseLocalCaptureAtArrayDeduplicatesIdenticalDiagnostics() {
        val source = standardInjectLocalCaptureMixinSource("index")
            .replace(
                """at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I")""",
                """at = {
                    @At(value = "INVOKE", target = "Ljava/lang/String;length()I"),
                    @At(value = "INVOKE", target = "Ljava/lang/String;length()I")
                }""".trimIndent(),
            )
            .replace("index = ", "index = 99")
        val diagnostics = localCaptureSamplesFacade().diagnose(localCaptureDiagnoseRequest(source))
        assertEquals(1, localCaptureDiagnostics(diagnostics).size)
    }

    @Test
    fun diagnoseInjectHandlerDoesNotReportMixinExtrasHandlerSignatureMismatch() {
        val source = standardInjectLocalCaptureMixinSource("ordinal").replace("ordinal = ", "ordinal = 0")
        val diagnostics = localCaptureSamplesFacade().diagnose(localCaptureDiagnoseRequest(source))
        assertTrue(diagnostics.none { it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH })
    }

    @Test
    fun diagnoseStandardInjectNonTrailingLocalReportsPlacementError() {
        val source = """
            @Mixin(LocalCaptureSamples.class)
            abstract class ExampleMixin {
                @Inject(
                    method = "instanceWithArgs(Ljava/lang/String;I)I",
                    at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I")
                )
                private void mcdevHandler(
                    String message,
                    @Local(ordinal = 0) int captured,
                    CallbackInfo ci
                ) {}
            }
        """.trimIndent()
        val diagnostics = localCaptureSamplesFacade().diagnose(localCaptureDiagnoseRequest(source))
        val placement = diagnostics.single {
            it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH &&
                it.message.contains("trailing", ignoreCase = true)
        }
        assertEquals("Sugar parameters must be trailing", placement.message)
    }

    @Test
    fun diagnoseStandardInjectMethodSelectorArrayDeduplicatesCommonSugarDiagnostics() {
        val source = """
            @Mixin(LocalCaptureSamples.class)
            abstract class ExampleMixin {
                @Inject(
                    method = {
                        "missing()V",
                        "instanceWithArgs(Ljava/lang/String;I)I"
                    },
                    at = @At("HEAD")
                )
                private void mcdevHandler(
                    String message,
                    int count,
                    @Share String shared,
                    CallbackInfo ci
                ) {}
            }
        """.trimIndent()
        val diagnostics = localCaptureSamplesFacade().diagnose(localCaptureDiagnoseRequest(source))
        assertEquals(
            1,
            diagnostics.count { it.code == MixinExtrasDiagnosticCodes.INVALID_SHARE_PARAMETER_TYPE },
        )
    }

    @Test
    fun diagnoseStandardInjectInvalidShareAndCancellableTypesReportExpectedCodes() {
        val source = """
            @Mixin(LocalCaptureSamples.class)
            abstract class ExampleMixin {
                @Inject(
                    method = "instanceWithArgs(Ljava/lang/String;I)I",
                    at = @At("HEAD")
                )
                private void mcdevHandler(
                    String message,
                    int count,
                    CallbackInfoReturnable callback,
                    @Share String shared,
                    @Cancellable CallbackInfo ci
                ) {}
            }
        """.trimIndent()
        val diagnostics = localCaptureSamplesFacade().diagnose(localCaptureDiagnoseRequest(source))
        assertEquals(MixinExtrasDiagnosticCodes.INVALID_SHARE_PARAMETER_TYPE, diagnostics.single {
            it.code == MixinExtrasDiagnosticCodes.INVALID_SHARE_PARAMETER_TYPE
        }.code)
        assertEquals(MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH, diagnostics.single {
            it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH
        }.code)
    }

    @Test
    fun diagnoseStandardInjectCancellableOnlyReportsMissingOrdinaryCallback() {
        val cases = listOf(
            "run()V" to "CallbackInfo",
            "instanceWithArgs(Ljava/lang/String;I)I" to "CallbackInfoReturnable",
        )
        for ((method, callbackType) in cases) {
            val source = """
                @Mixin(LocalCaptureSamples.class)
                abstract class ExampleMixin {
                    @Inject(method = "$method", at = @At("HEAD"))
                    private void mcdevHandler(@Cancellable $callbackType callback) {}
                }
            """.trimIndent()
            val diagnostics = localCaptureSamplesFacade(includeVoidTarget = true)
                .diagnose(localCaptureDiagnoseRequest(source))
            val mismatch = diagnostics.filter {
                it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH &&
                    it.message.contains("ordinary callback parameter")
            }
            assertEquals(1, mismatch.size, "missing callback for $method: $diagnostics")
        }
    }

    @Test
    fun diagnoseStandardInjectCancellableAcceptsOrdinaryCallbackBeforeSugar() {
        val sources = listOf(
            """
                @Mixin(LocalCaptureSamples.class)
                abstract class ExampleMixin {
                    @Inject(method = "run()V", at = @At("HEAD"))
                    private void mcdevHandler(
                        CallbackInfo callback,
                        @Cancellable CallbackInfo cancellable
                    ) {}
                }
            """.trimIndent(),
            """
                @Mixin(LocalCaptureSamples.class)
                abstract class ExampleMixin {
                    @Inject(method = "instanceWithArgs(Ljava/lang/String;I)I", at = @At("HEAD"))
                    private void mcdevHandler(
                        String message,
                        int count,
                        CallbackInfoReturnable callback,
                        @Cancellable CallbackInfoReturnable cancellable
                    ) {}
                }
            """.trimIndent(),
        )
        val facade = localCaptureSamplesFacade(includeVoidTarget = true)
        for (source in sources) {
            val diagnostics = facade.diagnose(localCaptureDiagnoseRequest(source))
            assertTrue(
                diagnostics.none {
                    it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH &&
                        it.message.contains("ordinary callback parameter")
                },
                "valid callback+sugar handler reported: $diagnostics",
            )
        }
    }

    @Test
    fun diagnoseStandardInjectCancellableAcceptsCapturedLocalsAfterCallback() {
        val source = """
            @Mixin(LocalCaptureSamples.class)
            abstract class ExampleMixin {
                @Inject(
                    method = "instanceWithArgs(Ljava/lang/String;I)I",
                    at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"),
                    locals = LocalCapture.CAPTURE_FAILHARD
                )
                private void mcdevHandler(
                    String message,
                    int count,
                    CallbackInfoReturnable callback,
                    int captured,
                    @Cancellable CallbackInfoReturnable cancellable
                ) {}
            }
        """.trimIndent()
        val diagnostics = localCaptureSamplesFacade().diagnose(localCaptureDiagnoseRequest(source))
        assertTrue(
            diagnostics.none {
                it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH &&
                    it.message.contains("ordinary callback parameter")
            },
            "valid callback+captured-locals handler reported: $diagnostics",
        )
    }

    @Test
    fun diagnoseStandardInjectMissingCallbackHighlightsCancellableParameter() {
        val source = """
            @Mixin(LocalCaptureSamples.class)
            abstract class ExampleMixin {
                @Inject(method = "run()V", at = @At("HEAD"))
                private void mcdevHandler(
                    @Local(ordinal = 0) int captured,
                    @Cancellable CallbackInfo cancellable
                ) {}
            }
        """.trimIndent()
        val diagnostics = localCaptureSamplesFacade(includeVoidTarget = true)
            .diagnose(localCaptureDiagnoseRequest(source))
        val mismatch = diagnostics.single {
            it.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH &&
                it.message.contains("ordinary callback parameter")
        }
        val line = source.lines()[mismatch.range.start.line]
        assertTrue(line.substring(mismatch.range.start.character).startsWith("@Cancellable"))
    }

    @Test
    fun routesShadowMemberCompletion() {
        val context = AnnotationContext(
            annotation = MixinAnnotation.SHADOW,
            slot = AnnotationSlot.SHADOW_MEMBER,
            partialValue = "cur",
            valueStartOffset = 0,
            valueEndOffset = 3,
            annotationStartOffset = 0,
            annotationEndOffset = 0,
            mixinTargetInternalNames = listOf("net/minecraft/client/MinecraftClient"),
        )
        val items = facade.complete(
            MixinE2ETestSupport.requestAt("@Shadow", "Shadow"),
            MixinCompletionOptions(),
        )
        val shadowFields = io.github.mcdev.core.mixin.ShadowValidationService(
            io.github.mcdev.core.mixin.FakeClassIndex(),
        ).completeFields(context.mixinTargetInternalNames, context.partialValue)
        assertTrue(shadowFields.any { it.name == "currentScreen" })
        assertTrue(items.isEmpty())
    }

    @Test
    fun routesAccessorFieldCompletionEndToEnd() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Accessor("cur")
                Screen getCurrentScreen();
            }
        """.trimIndent()
        val items = facade.complete(
            MixinE2ETestSupport.requestInAnnotationValue(source, "@Accessor", "cur"),
        )
        assertTrue(items.any { it.insertText == "currentScreen" })
    }

    @Test
    fun routesInvokerMethodCompletionEndToEnd() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Invoker("set")
                void invokeSetScreen(Screen screen);
            }
        """.trimIndent()
        val items = facade.complete(
            MixinE2ETestSupport.requestInAnnotationValue(source, "@Invoker", "set"),
        )
        assertTrue(items.any { it.insertText == "setScreen" })
    }

    @Test
    fun routesExpressionValueCompletionEndToEnd() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @ModifyExpressionValue(method = "tick", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
                @Expression("th")
                private void mcdevModify() {}
            }
        """.trimIndent()
        val items = facade.complete(
            MixinE2ETestSupport.requestInAnnotationValue(source, "@Expression", "th"),
        )
        assertEquals(listOf("this"), items.map { it.insertText })
        assertTrue(items.all { it.metadata.source == "mixinextras.expressionValue" })
    }

    @Test
    fun expressionValueCompletionUsesTokenOnlyReplacementRange() {
        val source = """@Expression(value = "return x")"""
        val cursor = source.indexOf("return") + 3
        val context = AnnotationContextExtractor.extractAtOffset(source, cursor)
        assertEquals("ret", context?.partialValue)
        assertEquals(source.indexOf("return"), context?.valueStartOffset)
        assertEquals(source.indexOf("return") + "return".length, context?.valueEndOffset)
    }

    @Test
    fun routesExpressionDefinitionIdCompletionEndToEnd() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Definition(id = "first")
                @Definition(id = "second")
                @ModifyExpressionValue(method = "tick", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
                @Expression("fi")
                private void mcdevModify() {}
            }
        """.trimIndent()
        val items = facade.complete(
            MixinE2ETestSupport.requestInAnnotationValue(source, "@Expression", "fi"),
        )
        assertEquals(listOf("first"), items.filter { it.metadata.source == "mixinextras.definitionId" }.map { it.insertText })
        assertTrue(items.any { it.insertText == "first" && it.metadata.source == "mixinextras.definitionId" })
    }

    @Test
    fun routesShareIdCompletionAcrossMatchingHandlersAndExplicitNamespaces() {
        val classIndex = FakeClassIndex(
            classes = FakeClassIndex.defaultClasses() + listOf(
                ClassIndexEntry("LocalIntRef", "com.llamalad7.mixinextras.sugar.ref", "com/llamalad7/mixinextras/sugar/ref/LocalIntRef"),
                ClassIndexEntry("LocalFloatRef", "com.llamalad7.mixinextras.sugar.ref", "com/llamalad7/mixinextras/sugar/ref/LocalFloatRef"),
            ),
        )
        val facade = MixinServiceFacade(classIndex, FakeBytecodeIndex())
        val source = """
            @Mixin(MinecraftClient.class)
            abstract class First {
                @Inject(method = "tick", at = @At("HEAD"))
                private void current(@Share("sp") LocalIntRef current) {}

                @Inject(method = "tick", at = @At("HEAD"))
                private void sameTarget(@Share("speed") LocalIntRef speed) {}

                @Inject(method = "tick", at = @At("HEAD"))
                private void sameTargetDuplicate(@Share("speed") LocalIntRef speed) {}

                @Inject(method = "tick", at = @At("HEAD"))
                private void wrongType(@Share("spFloat") LocalFloatRef speed) {}

                @Inject(method = "render(I)V", at = @At("HEAD"))
                private void wrongMethod(@Share("spOther") LocalIntRef speed) {}
            }
            @Mixin(MinecraftClient.class)
            abstract class Second {
                @Inject(method = "tick", at = @At("HEAD"))
                private void explicit(@Share(value = "special", namespace = "First") LocalIntRef special) {}

                @Inject(method = "tick", at = @At("HEAD"))
                private void omitted(@Share("spForeign") LocalIntRef foreign) {}
            }
        """.trimIndent()

        val items = facade.complete(MixinE2ETestSupport.requestInAnnotationValue(source, "@Share(\"sp\")", "sp"))
        assertEquals(listOf("special", "speed"), items.map { it.insertText })
        assertTrue(items.all { it.metadata.source == "mixinextras.share" })

        val namespaceSource = source.replace("@Share(\"sp\")", "@Share(namespace = \"sp\")")
        val namespaceCursor = namespaceSource.indexOf("namespace = \"sp\"") + "namespace = \"sp".length
        assertTrue(facade.complete(MixinE2ETestSupport.requestAtOffset(namespaceSource, namespaceCursor)).isEmpty())
    }

    @Test
    fun shareIdCompletionIncludesMatchingIdsFromAdditionalSources() {
        val classIndex = FakeClassIndex(
            classes = FakeClassIndex.defaultClasses() + listOf(
                ClassIndexEntry("LocalIntRef", "com.llamalad7.mixinextras.sugar.ref", "com/llamalad7/mixinextras/sugar/ref/LocalIntRef"),
                ClassIndexEntry("LocalFloatRef", "com.llamalad7.mixinextras.sugar.ref", "com/llamalad7/mixinextras/sugar/ref/LocalFloatRef"),
            ),
        )
        val source = """
            @Mixin(MinecraftClient.class)
            abstract class First {
                @Inject(method = "tick", at = @At("HEAD"))
                private void current(@Share("sp") LocalIntRef current) {}

                @Inject(method = "tick", at = @At("HEAD"))
                private void existing(@Share("speed") LocalIntRef speed) {}
            }
        """.trimIndent()
        val additionalSource = """
            @Mixin(MinecraftClient.class)
            abstract class Second {
                @Inject(method = "tick", at = @At("HEAD"))
                private void matching(@Share(value = "spring", namespace = "First") LocalIntRef matching) {}

                @Inject(method = "tick", at = @At("HEAD"))
                private void wrongType(@Share(value = "spfloat", namespace = "First") LocalFloatRef wrongType) {}

                @Inject(method = "render(I)V", at = @At("HEAD"))
                private void wrongTarget(@Share(value = "sptarget", namespace = "First") LocalIntRef wrongTarget) {}

                @Inject(method = "tick", at = @At("HEAD"))
                private void wrongNamespace(@Share(value = "spforeign", namespace = "Foreign") LocalIntRef wrongNamespace) {}
            }
        """.trimIndent()
        val facade = MixinServiceFacade(
            classIndex = classIndex,
            bytecodeIndex = FakeBytecodeIndex(),
            shareSources = { sequenceOf(additionalSource) },
        )
        val items = facade.complete(MixinE2ETestSupport.requestInAnnotationValue(source, """@Share("sp")""", "sp"))

        assertEquals(listOf("speed", "spring"), items.map { it.insertText })
    }

    @Test
    fun routesShareIdCompletionWithJavaEscapedValues() {
        val classIndex = FakeClassIndex(
            classes = FakeClassIndex.defaultClasses() + ClassIndexEntry(
                "LocalIntRef",
                "com.llamalad7.mixinextras.sugar.ref",
                "com/llamalad7/mixinextras/sugar/ref/LocalIntRef",
            ),
        )
        val facade = MixinServiceFacade(classIndex, FakeBytecodeIndex())
        val activeMarker = """@Share("q")"""
        val source = """
            @Mixin(MinecraftClient.class)
            abstract class First {
                @Inject(method = "tick", at = @At("HEAD"))
                private void current($activeMarker LocalIntRef current) {}

                @Inject(method = "tick", at = @At("HEAD"))
                private void quoted(@Share("quote\"id") LocalIntRef quoted) {}

                @Inject(method = "tick", at = @At("HEAD"))
                private void slashed(@Share("quote\\path") LocalIntRef slashed) {}
            }
        """.trimIndent()

        val request = MixinE2ETestSupport.requestInAnnotationValue(source, activeMarker, "q")
        val items = facade.complete(request)
        val activeStart = source.indexOf(activeMarker)
        val activeValueStart = activeStart + activeMarker.indexOf('"') + 1
        val activeContext = AnnotationContextExtractor.extractAtOffset(source, activeValueStart + 1)!!
        val quoted = items.single { it.label == "quote\"id" }
        val slashed = items.single { it.label == "quote\\path" }
        assertEquals("quote\\\"id", quoted.insertText)
        assertEquals(quoted.insertText, quoted.filterText)
        assertEquals("quote\\\\path", slashed.insertText)
        assertEquals("quote\"id", quoted.metadata.name)
        assertEquals(
            source.replace(activeMarker, """@Share("quote\"id")"""),
            source.replaceRange(
                activeContext.valueStartOffset,
                activeContext.valueEndOffset,
                quoted.insertText,
            ),
        )
        assertEquals(
            source.replace(activeMarker, """@Share("quote\\path")"""),
            source.replaceRange(
                activeContext.valueStartOffset,
                activeContext.valueEndOffset,
                slashed.insertText,
            ),
        )

        val rawPrefix = "quote" + "\\" + "\\"
        val escapedActiveMarker = """@Share("$rawPrefix")"""
        val narrowedSource = source.replace(activeMarker, escapedActiveMarker)
        val narrowed = facade.complete(
            MixinE2ETestSupport.requestInAnnotationValue(narrowedSource, escapedActiveMarker, rawPrefix),
        )
        assertEquals(listOf("quote\\path"), narrowed.map { it.label })
    }

    @Test
    fun shareSourceProviderIsNotCalledForNonShareCompletion() {
        var providerCalls = 0
        val facade = MixinServiceFacade(
            classIndex = FakeClassIndex(),
            bytecodeIndex = FakeBytecodeIndex(),
            shareSources = {
                providerCalls++
                emptySequence()
            },
        )
        val source = """
            @Mixin(MinecraftClient.class)
            abstract class Example {
                @Inject(method = "tick", at = @At("HEAD"))
                private void handler() {}
            }
        """.trimIndent()

        facade.complete(MixinE2ETestSupport.requestInAnnotationValue(source, "@Inject", "tic"))

        assertEquals(0, providerCalls)
    }

    @Test
    fun routesExpressionAfterDotMemberCompletionEndToEnd() {
        val source = """
            @Mixin(ExpressionMatchSamples.class)
            abstract class ExampleMixin {
                @ModifyExpressionValue(method = "readSampleField()I", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
                @Expression("this.")
                private Object mcdev${'$'}handler(Object original) { return original; }
            }
        """.trimIndent()
        val items = expressionMatchSamplesFacade().complete(
            MixinE2ETestSupport.requestInAnnotationValue(source, "@Expression", "this."),
        ).filter { it.metadata.source == "mixinextras.expressionMember" }

        assertEquals(1, items.size)
        val field = items.single()
        assertEquals("sampleField", field.insertText)
        assertEquals("sampleField:I", field.label)
    }

    @Test
    fun routesExpressionAfterDotMemberCompletionAddsDefinitionAndImportEdits() {
        val source = """
            package com.example.mixin;

            @Mixin(ExpressionMatchSamples.class)
            abstract class ExampleMixin {
                @ModifyExpressionValue(method = "readSampleField()I", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
                @Expression("this.")
                private Object mcdev${'$'}handler(Object original) { return original; }
            }
        """.trimIndent()
        val item = expressionMatchSamplesFacade().complete(
            MixinE2ETestSupport.requestInAnnotationValue(source, "@Expression", "this."),
        ).single { it.metadata.source == "mixinextras.expressionMember" }

        assertTrue(item.additionalEdits.any { it.newText.contains("@Definition(id = \"sampleField\"") })
        assertTrue(
            item.additionalEdits.any {
                it.newText.contains("import com.llamalad7.mixinextras.expression.Definition;")
            },
        )
    }

    @Test
    fun routesExpressionAfterDotMemberCompletionFailsClosedWithoutBytecode() {
        val source = """
            @Mixin(ExpressionMatchSamples.class)
            abstract class ExampleMixin {
                @ModifyExpressionValue(method = "readSampleField()I", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
                @Expression("this.")
                private Object mcdev${'$'}handler(Object original) { return original; }
            }
        """.trimIndent()
        val facade = MixinServiceFacade(expressionMatchSamplesClassIndex(), FakeBytecodeIndex())
        val items = facade.complete(
            MixinE2ETestSupport.requestInAnnotationValue(source, "@Expression", "this."),
        )

        assertTrue(items.none { it.metadata.source == "mixinextras.expressionMember" })
    }

    @Test
    fun filtersAtTargetCandidatesForModifyExpressionValue() {
        val owner = "net/minecraft/client/MinecraftClient"
        val facade = facadeWithAtTargetCandidates(
            owner = owner,
            methodName = "draw",
            atValue = "INVOKE",
            candidates = listOf(
                atTargetInvoke("draw", "(Ljava/lang/String;FFI)I", AtTargetOperationKind.INVOKE_VIRTUAL),
                atTargetInvoke("setScreen", "(Lnet/minecraft/client/gui/screen/Screen;)V", AtTargetOperationKind.INVOKE_VIRTUAL),
                atTargetInvoke("<init>", "(Ljava/lang/String;)V", AtTargetOperationKind.INVOKE_SPECIAL),
            ),
        )
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = ""))
                private Object handler(Object original) { return original; }
            }
        """.trimIndent()
        val items = facade.complete(requestAtTarget(source))
        assertEquals(
            listOf("Lnet/minecraft/client/MinecraftClient;draw(Ljava/lang/String;FFI)I"),
            items.map { it.insertText },
        )
    }

    @Test
    fun filtersAtTargetCandidatesForModifyReturnValueFailsClosedWithoutDescriptor() {
        val owner = "net/minecraft/client/MinecraftClient"
        val facade = facadeWithAtTargetCandidates(
            owner = owner,
            methodName = "tick",
            atValue = "RETURN",
            candidates = listOf(
                AtTargetCandidate(
                    owner = owner,
                    name = "RETURN",
                    descriptor = "",
                    displayLabel = "RETURN",
                    detail = "MinecraftClient",
                    kind = AtTargetKind.RETURN,
                ),
            ),
        )
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @ModifyReturnValue(method = "tick", at = @At(value = "RETURN", target = ""))
                private int handler(int original) { return original; }
            }
        """.trimIndent()
        val items = facade.complete(requestAtTarget(source))
        assertTrue(items.isEmpty())
    }

    @Test
    fun filtersAtTargetCandidatesForModifyReceiverAndWrapWithCondition() {
        val owner = "net/minecraft/client/MinecraftClient"
        val invokeCandidates = listOf(
            atTargetInvoke(
                "draw",
                "(Ljava/lang/String;FFI)I",
                AtTargetOperationKind.INVOKE_VIRTUAL,
                occurrenceResultClassification = OccurrenceResultClassification.RETAINED,
            ),
            atTargetInvoke(
                "setScreen",
                "(Lnet/minecraft/client/gui/screen/Screen;)V",
                AtTargetOperationKind.INVOKE_STATIC,
                occurrenceResultClassification = OccurrenceResultClassification.VOID,
            ),
            atTargetInvoke(
                "tick",
                "()V",
                AtTargetOperationKind.INVOKE_VIRTUAL,
                occurrenceResultClassification = OccurrenceResultClassification.IMMEDIATELY_POPPED,
            ),
        )
        val receiverFacade = facadeWithAtTargetCandidates(owner, "tick", "INVOKE", invokeCandidates)
        val receiverSource = """
            @Mixin(MinecraftClient.class)
            class M {
                @ModifyReceiver(method = "tick()V", at = @At(value = "INVOKE", target = ""))
                private Object handler(Object receiver) { return receiver; }
            }
        """.trimIndent()
        val receiverItems = receiverFacade.complete(requestAtTarget(receiverSource))
        assertEquals(
            listOf(
                "Lnet/minecraft/client/MinecraftClient;draw(Ljava/lang/String;FFI)I",
                "Lnet/minecraft/client/MinecraftClient;tick()V",
            ),
            receiverItems.map { it.insertText },
        )

        val wrapOperationFacade = facadeWithAtTargetCandidates(
            owner = owner,
            methodName = "tick",
            atValue = "INVOKE",
            candidates = invokeCandidates + listOf(
                atTargetInvoke("<init>", "(Ljava/lang/String;)V", AtTargetOperationKind.INVOKE_SPECIAL),
            ),
        )
        val wrapOperationSource = """
            @Mixin(MinecraftClient.class)
            class M {
                @WrapOperation(method = "tick()V", at = @At(value = "INVOKE", target = ""))
                private Object handler(Object original) { return original; }
            }
        """.trimIndent()
        val wrapOperationItems = wrapOperationFacade.complete(requestAtTarget(wrapOperationSource))
        assertEquals(3, wrapOperationItems.size)
        assertTrue(wrapOperationItems.none { it.insertText.contains("<init>") })

        val wrapFacade = facadeWithAtTargetCandidates(owner, "tick", "INVOKE", invokeCandidates)
        val wrapSource = """
            @Mixin(MinecraftClient.class)
            class M {
                @WrapWithCondition(method = "tick()V", at = @At(value = "INVOKE", target = ""))
                private boolean handler() { return true; }
            }
        """.trimIndent()
        val wrapItems = wrapFacade.complete(requestAtTarget(wrapSource))
        assertEquals(
            listOf(
                "Lnet/minecraft/client/MinecraftClient;setScreen(Lnet/minecraft/client/gui/screen/Screen;)V",
                "Lnet/minecraft/client/MinecraftClient;tick()V",
            ),
            wrapItems.map { it.insertText },
        )

        val fieldOnlyFacade = facadeWithAtTargetCandidates(
            owner = owner,
            methodName = "tick",
            atValue = "FIELD",
            candidates = listOf(
                atTargetField("currentScreen", "Lnet/minecraft/client/gui/screen/Screen;", AtTargetOperationKind.FIELD_PUT_INSTANCE),
                atTargetField("currentScreen", "Lnet/minecraft/client/gui/screen/Screen;", AtTargetOperationKind.FIELD_GET_INSTANCE),
            ),
        )
        val fieldWrapSource = """
            @Mixin(MinecraftClient.class)
            class M {
                @WrapWithCondition(method = "tick()V", at = @At(value = "FIELD", target = ""))
                private boolean handler() { return true; }
            }
        """.trimIndent()
        val fieldWrapItems = fieldOnlyFacade.complete(requestAtTarget(fieldWrapSource))
        assertEquals(
            listOf("Lnet/minecraft/client/MinecraftClient;currentScreen:Lnet/minecraft/client/gui/screen/Screen;"),
            fieldWrapItems.map { it.insertText },
        )
    }

    @Test
    fun keepsWrapOperationSliceBoundaryTargetCandidatesUnfiltered() {
        val owner = "net/minecraft/client/MinecraftClient"
        val returnCandidate = AtTargetCandidate(
            owner = owner,
            name = "RETURN",
            descriptor = "",
            displayLabel = "RETURN",
            detail = "MinecraftClient",
            kind = AtTargetKind.RETURN,
        )
        val facade = facadeWithAtTargetCandidates(owner, "tick", "RETURN", listOf(returnCandidate))
        val sliceSource = """
            @Mixin(MinecraftClient.class)
            class M {
                @WrapOperation(
                    method = "tick()V",
                    at = @At(value = "INVOKE", target = ""),
                    slice = @Slice(from = @At(value = "RETURN", target = ""))
                )
                private Object handler(Object original) { return original; }
            }
        """.trimIndent()
        val sliceTargetMarker = "from = @At(value = \"RETURN\", target = \""
        val sliceRequest = MixinE2ETestSupport.requestAtOffset(
            sliceSource,
            sliceSource.indexOf(sliceTargetMarker) + sliceTargetMarker.length,
        ).copy(
            semanticModel = MixinClassModel(targets = emptyList(), injectors = emptyList()),
        )
        val sliceItems = facade.complete(sliceRequest)
        assertEquals(listOf("RETURN"), sliceItems.map { it.insertText })

        val directSource = """
            @Mixin(MinecraftClient.class)
            class M {
                @WrapOperation(method = "tick()V", at = @At(value = "RETURN", target = ""))
                private Object handler(Object original) { return original; }
            }
        """.trimIndent()
        val directItems = facade.complete(requestAtTarget(directSource))
        assertTrue(directItems.isEmpty())
    }

    @Test
    fun doesNotFilterAtTargetCandidatesForStandardInjectors() {
        val owner = "net/minecraft/client/MinecraftClient"
        val candidates = listOf(
            atTargetInvoke("draw", "(Ljava/lang/String;FFI)I", AtTargetOperationKind.INVOKE_VIRTUAL),
            atTargetInvoke("setScreen", "(Lnet/minecraft/client/gui/screen/Screen;)V", AtTargetOperationKind.INVOKE_STATIC),
        )
        val facade = facadeWithAtTargetCandidates(owner, "tick", "INVOKE", candidates)
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(method = "tick()V", at = @At(value = "INVOKE", target = ""))
                private void handler() {}
            }
        """.trimIndent()
        val items = facade.complete(requestAtTarget(source))
        assertEquals(2, items.size)
    }

    private fun requestAtTarget(source: String): MixinFacadeRequest =
        MixinE2ETestSupport.requestAtOffset(
            source,
            source.indexOf("target = \"") + "target = \"".length,
        )

    private fun requestInLocalAttributeValue(source: String, attributeName: String): MixinFacadeRequest {
        val marker = "$attributeName = "
        val valueStart = source.indexOf(marker) + marker.length
        return MixinE2ETestSupport.requestAtOffset(source, valueStart)
    }

    private fun requestInLocalAttributeName(source: String, partial: String = ""): MixinFacadeRequest {
        val marker = "@Local("
        val cursor = source.indexOf(marker) + marker.length + partial.length
        return MixinE2ETestSupport.requestAtOffset(source, cursor)
    }

    private fun requestInNestedDefinitionLocalAttributeName(source: String, partial: String = ""): MixinFacadeRequest {
        val marker = "local = { @Local("
        val cursor = source.indexOf(marker) + marker.length + partial.length
        return MixinE2ETestSupport.requestAtOffset(source, cursor)
    }

    private fun handlerParameterLocalAttributeSource(): String = """
        @Mixin(LocalCaptureSamples.class)
        abstract class ExampleMixin {
            @WrapOperation(
                method = "instanceWithArgs(Ljava/lang/String;I)I",
                at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I")
            )
            private int mcdevHandler(
                String message,
                Operation<Integer> original,
                @Local( ) int captured
            ) {
                return original.call(message);
            }
        }
    """.trimIndent()

    private fun wrapMethodLocalCaptureMixinSource(): String = """
        @Mixin(LocalCaptureSamples.class)
        abstract class ExampleMixin {
            @WrapMethod(method = "instanceWithArgs(Ljava/lang/String;I)I")
            private int mcdevHandler(@Local(ordinal = ) int captured) {
                return captured;
            }
        }
    """.trimIndent()

    private fun definitionNestedLocalAttributeSource(): String = """
        @Mixin(LocalCaptureSamples.class)
        abstract class ExampleMixin {
            @WrapOperation(
                method = "instanceWithArgs(Ljava/lang/String;I)I",
                at = @At(
                    value = "MIXINEXTRAS:EXPRESSION",
                    args = {
                        @Definition(id = "loc", local = { @Local( }),
                        @Expression("0")
                    }
                )
            )
            private int mcdevHandler(String message, Operation<Integer> original) {
                return original.call(message);
            }
        }
    """.trimIndent()

    private fun localCaptureDiagnoseRequest(source: String): MixinFacadeRequest =
        MixinFacadeRequest(bufferText = source, line = 0, character = 0)

    private fun localCaptureDiagnostics(diagnostics: List<io.github.mcdev.core.diagnostics.McDiagnostic>) =
        diagnostics.filter {
            it.code == MixinExtrasDiagnosticCodes.LOCAL_CAPTURE_NOT_FOUND ||
                it.code == MixinExtrasDiagnosticCodes.LOCAL_CAPTURE_AMBIGUOUS
        }

    private fun modifyConstantLocalCaptureMixinSource(
        localAttribute: String,
        constantSelector: String = """@Constant(stringValue = "hello")""",
        originalType: String = "String",
    ): String {
        val attributeAssignment = "$localAttribute = "
        return """
            @Mixin(ConstantSamples.class)
            abstract class ExampleMixin {
                @ModifyConstant(
                    method = "constants()V",
                    constant = $constantSelector,
                )
                private $originalType mcdevHandler(
                    $originalType original,
                    @Local($attributeAssignment) String captured
                ) {
                    return original;
                }
            }
        """.trimIndent()
    }

    private fun constantSamplesOwner(): String =
        BytecodeFixtureCompiler.internalName("ConstantSamples")

    private fun constantSamplesClassIndex(): ClassIndex {
        val owner = constantSamplesOwner()
        return FakeClassIndex(
            classes = FakeClassIndex.defaultClasses() + listOf(
                ClassIndexEntry(
                    "ConstantSamples",
                    "io.github.mcdev.core.bytecode.fixtures",
                    owner,
                ),
                ClassIndexEntry("String", "java.lang", "java/lang/String"),
            ),
            methods = FakeClassIndex.defaultMethods() + mapOf(
                owner to listOf(
                    MethodIndexEntry(
                        "constants",
                        "()V",
                        false,
                        "constants(): void",
                    ),
                ),
            ),
        )
    }

    private fun constantSamplesBytecodeIndex(owner: String): BytecodeIndex {
        val classBytes = BytecodeFixtureCompiler.classBytes("ConstantSamples")
        val constantCandidates = InstructionExtractor.extract(classBytes, "constants", "()V")
            .filter { it.kind == BytecodeAtTargetKind.CONSTANT }
            .map(BytecodeIndexEntryMapper::toMixinAtTarget)
        val delegate = FakeBytecodeIndex()
        return object : BytecodeIndex {
            override fun getAtTargetCandidates(
                ownerInternalName: String,
                methodName: String,
                methodDescriptor: String?,
                atValue: String,
            ): List<AtTargetCandidate> =
                if (ownerInternalName == owner &&
                    methodName == "constants" &&
                    atValue == "CONSTANT"
                ) {
                    constantCandidates
                } else {
                    delegate.getAtTargetCandidates(ownerInternalName, methodName, methodDescriptor, atValue)
                }

            override fun getReturnOrdinalCount(
                ownerInternalName: String,
                methodName: String,
                methodDescriptor: String?,
            ): Int = delegate.getReturnOrdinalCount(ownerInternalName, methodName, methodDescriptor)

            override fun getClassBytes(ownerInternalName: String): ByteArray? =
                if (ownerInternalName == owner) classBytes else delegate.getClassBytes(ownerInternalName)

            override fun resolveCommonSuperClass(type1Descriptor: String, type2Descriptor: String): String? =
                delegate.resolveCommonSuperClass(type1Descriptor, type2Descriptor)
        }
    }

    private fun constantSamplesFacade(): MixinServiceFacade {
        val owner = constantSamplesOwner()
        return MixinServiceFacade(
            classIndex = constantSamplesClassIndex(),
            bytecodeIndex = constantSamplesBytecodeIndex(owner),
        )
    }

    private fun localCaptureMixinSource(localAttribute: String): String {
        val attributeAssignment = "$localAttribute = "
        return """
            @Mixin(LocalCaptureSamples.class)
            abstract class ExampleMixin {
                @WrapOperation(
                    method = "instanceWithArgs(Ljava/lang/String;I)I",
                    at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I")
                )
                private int mcdevHandler(
                    String message,
                    Operation<Integer> original,
                    @Local($attributeAssignment) int captured
                ) {
                    return original.call(message);
                }
            }
        """.trimIndent()
    }

    private fun standardInjectLocalCaptureMixinSource(localAttribute: String): String {
        val attributeAssignment = "$localAttribute = "
        return """
            @Mixin(LocalCaptureSamples.class)
            abstract class ExampleMixin {
                @Inject(
                    method = "instanceWithArgs(Ljava/lang/String;I)I",
                    at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I")
                )
                private void mcdevHandler(
                    String message,
                    CallbackInfo ci,
                    @Local($attributeAssignment) int captured
                ) {}
            }
        """.trimIndent()
    }

    private fun localCaptureSamplesOwner(): String =
        BytecodeFixtureCompiler.internalName("LocalCaptureSamples")

    private fun localCaptureSamplesClassIndex(includeVoidTarget: Boolean = false): ClassIndex {
        val owner = localCaptureSamplesOwner()
        val methods = buildList {
            add(
                MethodIndexEntry(
                    "instanceWithArgs",
                    "(Ljava/lang/String;I)I",
                    false,
                    "instanceWithArgs(String, int): int",
                ),
            )
            if (includeVoidTarget) {
                add(MethodIndexEntry("run", "()V", false, "run(): void"))
            }
        }
        return FakeClassIndex(
            classes = FakeClassIndex.defaultClasses() + listOf(
                ClassIndexEntry(
                    "LocalCaptureSamples",
                    "io.github.mcdev.core.bytecode.fixtures",
                    owner,
                ),
                ClassIndexEntry("String", "java.lang", "java/lang/String"),
                ClassIndexEntry("LocalRef", "com.llamalad7.mixinextras.sugar.ref", "com/llamalad7/mixinextras/sugar/ref/LocalRef"),
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
            methods = FakeClassIndex.defaultMethods() + mapOf(owner to methods),
        )
    }

    private fun localCaptureSamplesBytecodeIndex(owner: String): BytecodeIndex {
        val classBytes = BytecodeFixtureCompiler.classBytes("LocalCaptureSamples")
        val lengthInvokeIndex = invokeIndex(
            classBytes = classBytes,
            methodName = "instanceWithArgs",
            methodDescriptor = "(Ljava/lang/String;I)I",
            owner = "java/lang/String",
            name = "length",
        )
        val delegate = FakeBytecodeIndex()
        return object : BytecodeIndex {
            override fun getAtTargetCandidates(
                ownerInternalName: String,
                methodName: String,
                methodDescriptor: String?,
                atValue: String,
            ): List<AtTargetCandidate> =
                if (ownerInternalName == owner &&
                    methodName == "instanceWithArgs" &&
                    atValue == "INVOKE"
                ) {
                    listOf(
                        AtTargetCandidate(
                            owner = "java/lang/String",
                            name = "length",
                            descriptor = "()I",
                            displayLabel = "length(): int",
                            detail = "String",
                            kind = AtTargetKind.INVOKE,
                            operationKind = AtTargetOperationKind.INVOKE_VIRTUAL,
                            instructionOccurrenceIndex = lengthInvokeIndex,
                        ),
                    )
                } else {
                    delegate.getAtTargetCandidates(ownerInternalName, methodName, methodDescriptor, atValue)
                }

            override fun getReturnOrdinalCount(
                ownerInternalName: String,
                methodName: String,
                methodDescriptor: String?,
            ): Int = delegate.getReturnOrdinalCount(ownerInternalName, methodName, methodDescriptor)

            override fun getClassBytes(ownerInternalName: String): ByteArray? =
                if (ownerInternalName == owner) classBytes else delegate.getClassBytes(ownerInternalName)

            override fun resolveCommonSuperClass(type1Descriptor: String, type2Descriptor: String): String? =
                delegate.resolveCommonSuperClass(type1Descriptor, type2Descriptor)
        }
    }

    private fun localCaptureSamplesFacade(includeVoidTarget: Boolean = false): MixinServiceFacade {
        val owner = localCaptureSamplesOwner()
        return MixinServiceFacade(
            classIndex = localCaptureSamplesClassIndex(includeVoidTarget),
            bytecodeIndex = localCaptureSamplesBytecodeIndex(owner),
        )
    }

    private fun invokeIndex(
        classBytes: ByteArray,
        methodName: String,
        methodDescriptor: String,
        owner: String,
        name: String,
    ): Int {
        val classNode = ClassNode()
        ClassReader(classBytes).accept(classNode, ClassReader.SKIP_FRAMES)
        val methodNode = classNode.methods.single { it.name == methodName && it.desc == methodDescriptor }
        var occurrenceIndex = 0
        var instruction: AbstractInsnNode? = methodNode.instructions.first
        while (instruction != null) {
            if (instruction.opcode >= 0) {
                if (instruction is MethodInsnNode && instruction.owner == owner && instruction.name == name) {
                    return occurrenceIndex
                }
                occurrenceIndex++
            }
            instruction = instruction.next
        }
        error("invoke not found: $owner.$name in $methodName$methodDescriptor")
    }

    private fun facadeWithAtTargetCandidates(
        owner: String,
        methodName: String,
        atValue: String,
        candidates: List<AtTargetCandidate>,
    ): MixinServiceFacade =
        MixinServiceFacade(
            classIndex = FakeClassIndex(),
            bytecodeIndex = FakeBytecodeIndex(
                candidates = mapOf("$owner#$methodName#$atValue" to candidates),
            ),
        )

    private fun atTargetInvoke(
        name: String,
        descriptor: String,
        operationKind: AtTargetOperationKind,
        owner: String = "net/minecraft/client/MinecraftClient",
        occurrenceResultClassification: OccurrenceResultClassification = OccurrenceResultClassification.NOT_APPLICABLE,
    ) = AtTargetCandidate(
        owner = owner,
        name = name,
        descriptor = descriptor,
        displayLabel = name,
        detail = owner.substringAfterLast('/'),
        kind = AtTargetKind.INVOKE,
        ordinal = 0,
        namespace = MappingNamespace.NAMED,
        operationKind = operationKind,
        occurrenceResultClassification = occurrenceResultClassification,
    )

    private fun atTargetField(
        name: String,
        descriptor: String,
        operationKind: AtTargetOperationKind,
        owner: String = "net/minecraft/client/MinecraftClient",
    ) = AtTargetCandidate(
        owner = owner,
        name = name,
        descriptor = descriptor,
        displayLabel = "$name: field",
        detail = owner.substringAfterLast('/'),
        kind = AtTargetKind.FIELD,
        ordinal = 0,
        operationKind = operationKind,
    )

    private fun expressionMatchSamplesFacade(): MixinServiceFacade {
        val owner = expressionMatchSamplesOwner()
        return MixinServiceFacade(
            classIndex = expressionMatchSamplesClassIndex(),
            bytecodeIndex = expressionMatchSamplesBytecodeIndex(owner),
        )
    }

    private fun expressionMatchSamplesOwner(): String =
        BytecodeFixtureCompiler.internalName("ExpressionMatchSamples")

    private fun expressionMatchSamplesClassIndex(): ClassIndex {
        val owner = expressionMatchSamplesOwner()
        return FakeClassIndex(
            classes = FakeClassIndex.defaultClasses() + listOf(
                ClassIndexEntry(
                    "ExpressionMatchSamples",
                    "io.github.mcdev.core.bytecode.fixtures",
                    owner,
                ),
            ),
            methods = FakeClassIndex.defaultMethods() + mapOf(
                owner to listOf(
                    MethodIndexEntry("readSampleField", "()I", false, "readSampleField(): int"),
                    MethodIndexEntry(
                        "trim",
                        "(Ljava/lang/String;)Ljava/lang/String;",
                        false,
                        "trim(String): String",
                    ),
                ),
            ),
        )
    }

    private fun expressionMatchSamplesBytecodeIndex(owner: String): BytecodeIndex {
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
                if (ownerInternalName == owner) classBytes else delegate.getClassBytes(ownerInternalName)

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
}
