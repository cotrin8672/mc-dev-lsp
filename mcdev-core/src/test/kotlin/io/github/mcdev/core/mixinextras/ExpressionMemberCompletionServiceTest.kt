package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.bytecode.BytecodeFixtureCompiler
import io.github.mcdev.core.mixin.AnnotationContext
import io.github.mcdev.core.mixin.AnnotationContextExtractor
import io.github.mcdev.core.mixin.AnnotationSlot
import io.github.mcdev.core.mixin.DecodedExpressionPrefix
import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FakeBytecodeIndex
import io.github.mcdev.core.mixin.FakeClassIndex
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.mixin.MixinAnnotation
import java.util.concurrent.atomic.AtomicInteger
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertTrue
import org.objectweb.asm.Opcodes
import org.objectweb.asm.Type

class ExpressionMemberCompletionServiceTest {
    private val owner = BytecodeFixtureCompiler.internalName("ExpressionMatchSamples")
    private val classIndex = expressionMatchSamplesClassIndex()
    private val bytecodeIndex = expressionMatchSamplesBytecodeIndex()
    private val service = ExpressionMemberCompletionService(classIndex, bytecodeIndex)

    @Test
    fun returnsReachableMembersForStandardInjectHandlerWithExpression() {
        val fieldResult = complete(
            expressionValue = "this.",
            targetMethod = "readSampleField()I",
            injector = "Inject",
        )
        val fieldCandidate = assertIs<OfficialExpressionMemberCandidate.FieldAccess>(
            requireAvailable(fieldResult).candidates.single(),
        )
        assertEquals("sampleField", fieldCandidate.name)
        assertEquals("I", fieldCandidate.descriptor)
        assertEquals(Opcodes.GETFIELD, fieldCandidate.originalInstructionOpcode)

        val methodResult = complete(
            expressionValue = "value.",
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
            injector = "Inject",
        )
        val methodCandidate = assertIs<OfficialExpressionMemberCandidate.MethodInvocation>(
            requireAvailable(methodResult).candidates.single(),
        )
        assertEquals("trim", methodCandidate.name)
        assertEquals("()Ljava/lang/String;", methodCandidate.descriptor)
        assertEquals(Opcodes.INVOKEVIRTUAL, methodCandidate.originalInstructionOpcode)
    }

    @Test
    fun returnsOnlyReachableFieldAndMethodCandidates() {
        val fieldResult = complete(
            expressionValue = "this.",
            targetMethod = "readSampleField()I",
        )
        val fieldCandidate = assertIs<OfficialExpressionMemberCandidate.FieldAccess>(
            requireAvailable(fieldResult).candidates.single(),
        )
        assertEquals("sampleField", fieldCandidate.name)
        assertEquals("I", fieldCandidate.descriptor)
        assertEquals(Opcodes.GETFIELD, fieldCandidate.originalInstructionOpcode)

        val methodResult = complete(
            expressionValue = "value.",
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
        )
        val methodCandidate = assertIs<OfficialExpressionMemberCandidate.MethodInvocation>(
            requireAvailable(methodResult).candidates.single(),
        )
        assertEquals("trim", methodCandidate.name)
        assertEquals("()Ljava/lang/String;", methodCandidate.descriptor)
        assertEquals(Opcodes.INVOKEVIRTUAL, methodCandidate.originalInstructionOpcode)
    }

    @Test
    fun excludesUnrelatedClassMembers() {
        val result = complete(
            expressionValue = "value.",
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
        )
        val available = requireAvailable(result)
        assertEquals(1, available.candidates.size)
        assertTrue(available.candidates.none { it.name == "sampleField" })
        assertTrue(available.candidates.none { it.name == "readSampleField" })
    }

    @Test
    fun filtersCandidatesByMemberPrefix() {
        val matching = requireAvailable(
            complete(
                expressionValue = "value.tr",
                targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
            ),
        )
        assertEquals(listOf("trim"), matching.candidates.map { it.name })

        assertEquals(
            ExpressionMemberCompletionServiceResult.Empty,
            complete(
                expressionValue = "value.xyz",
                targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
            ),
        )
    }

    @Test
    fun failsClosedForAmbiguousTargetMethod() {
        val source = expressionHandlerSource(
            mixinTarget = "SimpleTarget",
            targetMethod = "draw",
            expressionValue = "this.",
        )
        val context = expressionValueContextFromSource(source, valuePrefix = "this.")
        val result = service.completeMembers(
            source = source,
            context = context,
            mixinTargetOwners = listOf("com/example/target/SimpleTarget"),
        )
        assertEquals(ExpressionMemberCompletionServiceResult.Unavailable, result)
    }

    @Test
    fun failsClosedWhenClassBytesMissing() {
        val missingBytesIndex = object : BytecodeIndex by bytecodeIndex {
            override fun getClassBytes(ownerInternalName: String): ByteArray? = null
        }
        val missingBytesService = ExpressionMemberCompletionService(classIndex, missingBytesIndex)
        val source = expressionHandlerSource(
            targetMethod = "readSampleField()I",
            expressionValue = "this.",
        )
        val context = expressionValueContextFromSource(source, valuePrefix = "this.")
        val result = missingBytesService.completeMembers(
            source = source,
            context = context,
            mixinTargetOwners = listOf(owner),
        )
        assertEquals(ExpressionMemberCompletionServiceResult.Unavailable, result)
    }

    @Test
    fun deduplicatesCandidatesDeterministicallyAcrossOwners() {
        val multiOwnerClassIndex = FakeClassIndex(
            classes = FakeClassIndex.defaultClasses() + listOf(
                ClassIndexEntry(
                    "ExpressionMatchSamples",
                    "io.github.mcdev.core.bytecode.fixtures",
                    owner,
                ),
                ClassIndexEntry(
                    "SharedMixinTargetA",
                    "com.example.target",
                    "com/example/target/SharedMixinTargetA",
                ),
                ClassIndexEntry(
                    "SharedMixinTargetB",
                    "com.example.target",
                    "com/example/target/SharedMixinTargetB",
                ),
            ),
            methods = FakeClassIndex.defaultMethods() + mapOf(
                owner to listOf(
                    MethodIndexEntry("readSampleField", "()I", false, "readSampleField(): int"),
                ),
                "com/example/target/SharedMixinTargetA" to listOf(
                    MethodIndexEntry("readSampleField", "()I", false, "readSampleField(): int"),
                ),
                "com/example/target/SharedMixinTargetB" to listOf(
                    MethodIndexEntry("readSampleField", "()I", false, "readSampleField(): int"),
                ),
            ),
        )
        val classBytes = BytecodeFixtureCompiler.classBytes("ExpressionMatchSamples")
        val multiOwnerBytecodeIndex = multiOwnerExpressionMatchSamplesBytecodeIndex(classBytes)
        val multiOwnerService = ExpressionMemberCompletionService(multiOwnerClassIndex, multiOwnerBytecodeIndex)

        val source = expressionHandlerSource(
            targetMethod = "readSampleField()I",
            expressionValue = "this.",
        )
        val context = expressionValueContextFromSource(source, valuePrefix = "this.")
        val result = multiOwnerService.completeMembers(
            source = source,
            context = context,
            mixinTargetOwners = listOf(
                "com/example/target/SharedMixinTargetA",
                owner,
                "com/example/target/SharedMixinTargetB",
            ),
        )

        val available = requireAvailable(result)
        assertEquals(1, available.candidates.size)
        val candidate = assertIs<OfficialExpressionMemberCandidate.FieldAccess>(available.candidates.single())
        assertEquals("sampleField", candidate.name)

        repeat(5) {
            val repeated = multiOwnerService.completeMembers(
                source = source,
                context = context,
                mixinTargetOwners = listOf(
                    "com/example/target/SharedMixinTargetA",
                    owner,
                    "com/example/target/SharedMixinTargetB",
                ),
            )
            assertEquals(available.candidates, requireAvailable(repeated).candidates)
        }
    }

    @Test
    fun propagatesCancellation() {
        val cancellation = MemberCompletionCancellationMarker()
        val source = expressionHandlerSource(
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
            expressionValue = "value.",
        )
        val context = expressionValueContextFromSource(source, valuePrefix = "value.")

        val thrown = assertFailsWith<MemberCompletionCancellationMarker> {
            service.completeMembers(
                source = source,
                context = context,
                mixinTargetOwners = listOf(owner),
                cancellationChecker = OfficialExpressionCancellationChecker {
                    if (Thread.currentThread().stackTrace.any { frame ->
                            frame.className.contains("MatchSinkState") &&
                                (frame.methodName == "reportMatchStatus" ||
                                    frame.methodName == "reportPartialMatch")
                        }
                    ) {
                        throw cancellation
                    }
                },
            )
        }
        assertTrue(thrown === cancellation)
    }

    @Test
    fun returnsEmptyForNonAfterDotPositions() {
        val source = expressionHandlerSource(
            targetMethod = "readSampleField()I",
            expressionValue = "this",
        )
        val context = expressionValueContextFromSource(source, valuePrefix = "this")
            .copy(expressionCompletionPosition = ExpressionCompletionPosition.VALUE_START)

        val result = service.completeMembers(
            source = source,
            context = context,
            mixinTargetOwners = listOf(owner),
        )
        assertEquals(ExpressionMemberCompletionServiceResult.Empty, result)
    }

    @Test
    fun sharedCacheReusesMatcherAcrossDistinctServiceInstancesWhenOnlyMemberPrefixChanges() {
        val invocationCount = AtomicInteger(0)
        val sharedCache = ExpressionMemberCompletionCache()
        val countingMatcher = countingMatcherRunner(invocationCount)
        val firstService = ExpressionMemberCompletionService(
            classIndex = classIndex,
            bytecodeIndex = bytecodeIndex,
            reachableMembersCache = sharedCache,
            matcherRunner = countingMatcher,
        )
        val secondService = ExpressionMemberCompletionService(
            classIndex = classIndex,
            bytecodeIndex = bytecodeIndex,
            reachableMembersCache = sharedCache,
            matcherRunner = countingMatcher,
        )

        val first = complete(
            service = firstService,
            expressionValue = "value.t",
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
        )
        assertEquals(1, invocationCount.get())
        assertEquals(listOf("trim"), requireAvailable(first).candidates.map { it.name })

        val second = complete(
            service = secondService,
            expressionValue = "value.tr",
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
        )
        assertEquals(1, invocationCount.get())
        assertEquals(listOf("trim"), requireAvailable(second).candidates.map { it.name })
    }

    @Test
    fun reusesCachedReachableMembersWhenOnlyMemberPrefixChanges() {
        val invocationCount = AtomicInteger(0)
        val countingService = countingService(invocationCount)

        val first = complete(
            service = countingService,
            expressionValue = "value.t",
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
        )
        assertEquals(1, invocationCount.get())
        assertEquals(listOf("trim"), requireAvailable(first).candidates.map { it.name })

        val second = complete(
            service = countingService,
            expressionValue = "value.tr",
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
        )
        assertEquals(1, invocationCount.get())
        assertEquals(listOf("trim"), requireAvailable(second).candidates.map { it.name })
    }

    @Test
    fun filtersCachedCandidatesByMemberPrefixWithoutReRunningMatcher() {
        val invocationCount = AtomicInteger(0)
        val countingService = countingService(invocationCount)

        requireAvailable(
            complete(
                service = countingService,
                expressionValue = "value.",
                targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
            ),
        )
        assertEquals(1, invocationCount.get())

        assertEquals(
            listOf("trim"),
            requireAvailable(
                complete(
                    service = countingService,
                    expressionValue = "value.tr",
                    targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
                ),
            ).candidates.map { it.name },
        )
        assertEquals(1, invocationCount.get())

        assertEquals(
            ExpressionMemberCompletionServiceResult.Empty,
            complete(
                service = countingService,
                expressionValue = "value.xyz",
                targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
            ),
        )
        assertEquals(1, invocationCount.get())
    }

    @Test
    fun cacheMissesWhenReceiverExpressionChanges() {
        val invocationCount = AtomicInteger(0)
        val countingService = countingService(invocationCount)

        complete(
            service = countingService,
            expressionValue = "value.",
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
        )
        assertEquals(1, invocationCount.get())

        complete(
            service = countingService,
            expressionValue = "this.",
            targetMethod = "readSampleField()I",
        )
        assertEquals(2, invocationCount.get())
    }

    @Test
    fun cacheMissesWhenClassBytesContentChanges() {
        val invocationCount = AtomicInteger(0)
        val classBytesHolder = MutableClassBytesHolder(BytecodeFixtureCompiler.classBytes("ExpressionMatchSamples"))
        val countingService = countingService(invocationCount, mutableClassBytesIndex(classBytesHolder))

        complete(
            service = countingService,
            expressionValue = "value.",
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
        )
        assertEquals(1, invocationCount.get())

        classBytesHolder.bytes = classBytesHolder.bytes.copyOf().also { it[0] = (it[0].toInt() xor 0x01).toByte() }
        complete(
            service = countingService,
            expressionValue = "value.",
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
        )
        assertEquals(2, invocationCount.get())
    }

    @Test
    fun cacheMissesWhenDefinitionSemanticContentChanges() {
        val invocationCount = AtomicInteger(0)
        val countingService = countingService(invocationCount)

        val sourceWithoutDefinition = expressionHandlerSource(
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
            expressionValue = "value.",
        )
        val contextWithoutDefinition = expressionValueContextFromSource(sourceWithoutDefinition, valuePrefix = "value.")
        countingService.completeMembers(
            source = sourceWithoutDefinition,
            context = contextWithoutDefinition,
            mixinTargetOwners = listOf(owner),
        )
        assertEquals(1, invocationCount.get())

        val sourceWithDefinition = expressionHandlerSourceWithDefinition(
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
            expressionValue = "value.",
            definitionId = "probe",
            definitionMethod = "Lcom/example/Foo;bar()V",
        )
        val contextWithDefinition = expressionValueContextFromSource(sourceWithDefinition, valuePrefix = "value.")
        countingService.completeMembers(
            source = sourceWithDefinition,
            context = contextWithDefinition,
            mixinTargetOwners = listOf(owner),
        )
        assertEquals(2, invocationCount.get())
    }

    @Test
    fun cacheMissesWhenTargetMethodOrContextTypeChanges() {
        val invocationCount = AtomicInteger(0)
        val countingService = countingService(invocationCount)

        complete(
            service = countingService,
            expressionValue = "value.",
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
        )
        assertEquals(1, invocationCount.get())

        complete(
            service = countingService,
            expressionValue = "this.",
            targetMethod = "readSampleField()I",
        )
        assertEquals(2, invocationCount.get())

        val wrapSource = wrapOperationHandlerSource(
            targetMethod = "readSampleField()I",
            expressionValue = "this.",
        )
        val wrapContext = expressionValueContextFromSource(wrapSource, valuePrefix = "this.")
        countingService.completeMembers(
            source = wrapSource,
            context = wrapContext,
            mixinTargetOwners = listOf(owner),
        )
        assertEquals(3, invocationCount.get())
    }

    @Test
    fun doesNotCacheUnavailableOrCancelledMatcherResults() {
        val unavailableCount = AtomicInteger(0)
        var returnUnavailable = true
        val unavailableService = ExpressionMemberCompletionService(
            classIndex = classIndex,
            bytecodeIndex = bytecodeIndex,
            matcherRunner = countingMatcherRunner(unavailableCount) { _, _, _, _, _, _, _, _ ->
                if (returnUnavailable) {
                    OfficialExpressionMemberCompletionResult.Unavailable(reason = "test unavailable")
                } else {
                    OfficialExpressionMatcher.completeReceiverMembers(
                        classBytes = BytecodeFixtureCompiler.classBytes("ExpressionMatchSamples"),
                        methodName = "trim",
                        methodDescriptor = "(Ljava/lang/String;)Ljava/lang/String;",
                        receiverExpression = "value",
                        contextType = OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE,
                        commonSuperClass = CommonSuperClassResolver { left, right ->
                            bytecodeIndex.resolveCommonSuperClass(left, right)
                        },
                    )
                }
            },
        )
        val unavailableSource = expressionHandlerSource(
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
            expressionValue = "value.",
        )
        val unavailableContext = expressionValueContextFromSource(unavailableSource, valuePrefix = "value.")
        assertEquals(
            ExpressionMemberCompletionServiceResult.Unavailable,
            unavailableService.completeMembers(
                source = unavailableSource,
                context = unavailableContext,
                mixinTargetOwners = listOf(owner),
            ),
        )
        assertEquals(1, unavailableCount.get())
        returnUnavailable = false
        requireAvailable(
            unavailableService.completeMembers(
                source = unavailableSource,
                context = unavailableContext,
                mixinTargetOwners = listOf(owner),
            ),
        )
        assertEquals(2, unavailableCount.get())

        val cancellationCount = AtomicInteger(0)
        var shouldCancel = true
        val cancellation = MemberCompletionCancellationMarker()
        val cancellationService = ExpressionMemberCompletionService(
            classIndex = classIndex,
            bytecodeIndex = bytecodeIndex,
            matcherRunner = countingMatcherRunner(cancellationCount) { classBytes, methodName, methodDescriptor, receiverExpression, contextType, identifierPool, commonSuperClass, cancellationChecker ->
                if (shouldCancel) {
                    throw cancellation
                }
                OfficialExpressionMatcher.completeReceiverMembers(
                    classBytes = classBytes,
                    methodName = methodName,
                    methodDescriptor = methodDescriptor,
                    receiverExpression = receiverExpression,
                    memberPrefix = "",
                    contextType = contextType,
                    identifierPool = identifierPool,
                    commonSuperClass = commonSuperClass,
                    cancellationChecker = cancellationChecker,
                )
            },
        )
        val cancellationSource = expressionHandlerSource(
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
            expressionValue = "value.",
        )
        val cancellationContext = expressionValueContextFromSource(cancellationSource, valuePrefix = "value.")
        assertFailsWith<MemberCompletionCancellationMarker> {
            cancellationService.completeMembers(
                source = cancellationSource,
                context = cancellationContext,
                mixinTargetOwners = listOf(owner),
            )
        }
        assertEquals(1, cancellationCount.get())
        shouldCancel = false
        requireAvailable(
            cancellationService.completeMembers(
                source = cancellationSource,
                context = cancellationContext,
                mixinTargetOwners = listOf(owner),
            ),
        )
        assertEquals(2, cancellationCount.get())
    }

    @Test
    fun evictsOldestCacheEntryAndReRunsMatcherOnCapacityMiss() {
        val invocationCount = AtomicInteger(0)
        val boundedService = ExpressionMemberCompletionService(
            classIndex = classIndex,
            bytecodeIndex = bytecodeIndex,
            cacheCapacity = 2,
            matcherRunner = countingMatcherRunner(invocationCount),
        )

        complete(
            service = boundedService,
            expressionValue = "value.",
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
        )
        assertEquals(1, invocationCount.get())

        complete(
            service = boundedService,
            expressionValue = "this.",
            targetMethod = "readSampleField()I",
        )
        assertEquals(2, invocationCount.get())

        val wrapSource = wrapOperationHandlerSource(
            targetMethod = "readSampleField()I",
            expressionValue = "this.",
        )
        val wrapContext = expressionValueContextFromSource(wrapSource, valuePrefix = "this.")
        boundedService.completeMembers(
            source = wrapSource,
            context = wrapContext,
            mixinTargetOwners = listOf(owner),
        )
        assertEquals(3, invocationCount.get())

        complete(
            service = boundedService,
            expressionValue = "value.",
            targetMethod = "trim(Ljava/lang/String;)Ljava/lang/String;",
        )
        assertEquals(4, invocationCount.get())
    }

    @Test
    fun returnsEmptyForNonExpressionAnnotation() {
        val context = AnnotationContext(
            annotation = MixinAnnotation.DEFINITION,
            slot = AnnotationSlot.VALUE,
            partialValue = "this.",
            valueStartOffset = 0,
            valueEndOffset = 5,
            annotationStartOffset = 0,
            annotationEndOffset = 0,
            expressionCompletionPosition = ExpressionCompletionPosition.AFTER_DOT,
            decodedExpressionPrefix = DecodedExpressionPrefix("this.", 5),
        )
        val result = service.completeMembers(
            source = expressionHandlerSource(
                targetMethod = "readSampleField()I",
                expressionValue = "this.",
            ),
            context = context,
            mixinTargetOwners = listOf(owner),
        )
        assertEquals(ExpressionMemberCompletionServiceResult.Empty, result)
    }

    private fun complete(
        expressionValue: String,
        targetMethod: String,
        service: ExpressionMemberCompletionService = this.service,
        injector: String = "ModifyExpressionValue",
    ): ExpressionMemberCompletionServiceResult {
        val source = expressionHandlerSource(
            targetMethod = targetMethod,
            expressionValue = expressionValue,
            injector = injector,
        )
        val context = expressionValueContextFromSource(source, valuePrefix = expressionValue)
        return service.completeMembers(
            source = source,
            context = context,
            mixinTargetOwners = listOf(owner),
        )
    }

    private fun countingService(
        invocationCount: AtomicInteger,
        bytecodeIndex: BytecodeIndex = this.bytecodeIndex,
    ): ExpressionMemberCompletionService =
        ExpressionMemberCompletionService(
            classIndex = classIndex,
            bytecodeIndex = bytecodeIndex,
            matcherRunner = countingMatcherRunner(invocationCount),
        )

    private fun countingMatcherRunner(
        invocationCount: AtomicInteger,
        delegate: ExpressionMemberMatcherRunner = ExpressionMemberMatcherRunner.DEFAULT,
    ): ExpressionMemberMatcherRunner =
        ExpressionMemberMatcherRunner { classBytes, methodName, methodDescriptor, receiverExpression, contextType, identifierPool, commonSuperClass, cancellationChecker ->
            invocationCount.incrementAndGet()
            delegate.completeReachableMembers(
                classBytes = classBytes,
                methodName = methodName,
                methodDescriptor = methodDescriptor,
                receiverExpression = receiverExpression,
                contextType = contextType,
                identifierPool = identifierPool,
                commonSuperClass = commonSuperClass,
                cancellationChecker = cancellationChecker,
            )
        }

    private fun mutableClassBytesIndex(classBytesHolder: MutableClassBytesHolder): BytecodeIndex {
        val delegate = expressionMatchSamplesBytecodeIndexForOwners(setOf(owner), classBytesHolder.bytes)
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
                if (ownerInternalName == owner) classBytesHolder.bytes else delegate.getClassBytes(ownerInternalName)

            override fun resolveCommonSuperClass(type1Descriptor: String, type2Descriptor: String): String? =
                delegate.resolveCommonSuperClass(type1Descriptor, type2Descriptor)
        }
    }

    private class MutableClassBytesHolder(var bytes: ByteArray)

    private fun expressionHandlerSourceWithDefinition(
        mixinTarget: String = "ExpressionMatchSamples",
        targetMethod: String,
        expressionValue: String,
        definitionId: String,
        definitionMethod: String,
    ): String = """
        @Mixin($mixinTarget.class)
        abstract class ExampleMixin {
            @ModifyExpressionValue(method = "$targetMethod", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
            @Definition(id = "$definitionId", method = "$definitionMethod")
            @Expression("$expressionValue")
            private Object mcdev${'$'}handler(Object original) { return original; }
        }
    """.trimIndent()

    private fun wrapOperationHandlerSource(
        mixinTarget: String = "ExpressionMatchSamples",
        targetMethod: String,
        expressionValue: String,
    ): String = """
        @Mixin($mixinTarget.class)
        abstract class ExampleMixin {
            @WrapOperation(method = "$targetMethod", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
            @Expression("$expressionValue")
            private Object mcdev${'$'}handler(Operation original) { return original.call(); }
        }
    """.trimIndent()

    private fun expressionHandlerSource(
        mixinTarget: String = "ExpressionMatchSamples",
        targetMethod: String,
        expressionValue: String,
        injector: String = "ModifyExpressionValue",
    ): String = if (injector == "Inject") {
        """
            @Mixin($mixinTarget.class)
            abstract class ExampleMixin {
                @Expression("$expressionValue")
                @Inject(method = "$targetMethod", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
                private void mcdev${'$'}handler(CallbackInfo ci) {}
            }
        """.trimIndent()
    } else {
        """
            @Mixin($mixinTarget.class)
            abstract class ExampleMixin {
                @$injector(method = "$targetMethod", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
                @Expression("$expressionValue")
                private Object mcdev${'$'}handler(Object original) { return original; }
            }
        """.trimIndent()
    }

    private fun expressionValueContextFromSource(
        source: String,
        valuePrefix: String,
    ): AnnotationContext {
        val annotationStart = source.indexOf("@Expression")
        val valueStart = source.indexOf('"', annotationStart) + 1
        val cursor = valueStart + valuePrefix.length
        return AnnotationContextExtractor.extractAtOffset(source, cursor)
            ?: error("failed to extract annotation context at offset $cursor")
    }

    private fun requireAvailable(
        result: ExpressionMemberCompletionServiceResult,
    ): ExpressionMemberCompletionServiceResult.Available =
        when (result) {
            is ExpressionMemberCompletionServiceResult.Available -> result
            is ExpressionMemberCompletionServiceResult.Empty ->
                error("expected Available but got Empty")
            is ExpressionMemberCompletionServiceResult.Unavailable ->
                error("expected Available but got Unavailable")
        }

    private fun expressionMatchSamplesClassIndex(): ClassIndex =
        FakeClassIndex(
            classes = listOf(
                ClassIndexEntry(
                    "ExpressionMatchSamples",
                    "io.github.mcdev.core.bytecode.fixtures",
                    owner,
                ),
                ClassIndexEntry("SimpleTarget", "com.example.target", "com/example/target/SimpleTarget"),
            ),
            methods = mapOf(
                owner to listOf(
                    MethodIndexEntry("readSampleField", "()I", false, "readSampleField(): int"),
                    MethodIndexEntry(
                        "trim",
                        "(Ljava/lang/String;)Ljava/lang/String;",
                        false,
                        "trim(String): String",
                    ),
                ),
                "com/example/target/SimpleTarget" to listOf(
                    MethodIndexEntry("draw", "(Ljava/lang/String;FF)V", false, "draw(String, float, float): void"),
                    MethodIndexEntry("draw", "(I)V", false, "draw(int): void"),
                ),
            ),
        )

    private fun expressionMatchSamplesBytecodeIndex(): BytecodeIndex =
        expressionMatchSamplesBytecodeIndexForOwners(setOf(owner))

    private fun multiOwnerExpressionMatchSamplesBytecodeIndex(classBytes: ByteArray): BytecodeIndex =
        expressionMatchSamplesBytecodeIndexForOwners(
            owners = setOf(
                owner,
                "com/example/target/SharedMixinTargetA",
                "com/example/target/SharedMixinTargetB",
            ),
            classBytes = classBytes,
        )

    private fun expressionMatchSamplesBytecodeIndexForOwners(
        owners: Set<String>,
        classBytes: ByteArray = BytecodeFixtureCompiler.classBytes("ExpressionMatchSamples"),
    ): BytecodeIndex {
        val commonSuperClassResolver = BytecodeCommonSuperClassResolver(
            classBytesLookup = { internalName ->
                if (internalName in owners) classBytes else null
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
                if (ownerInternalName in owners) classBytes else null

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

    private class MemberCompletionCancellationMarker : RuntimeException("member completion cancellation")
}
