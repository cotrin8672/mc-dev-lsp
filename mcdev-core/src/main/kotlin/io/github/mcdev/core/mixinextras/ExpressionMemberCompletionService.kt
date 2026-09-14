package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.mixin.AnnotationContext
import io.github.mcdev.core.mixin.AnnotationSlot
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.mixin.MixinAnnotation
import io.github.mcdev.core.mixin.MixinTargetResolver
import java.security.MessageDigest

sealed interface ExpressionMemberCompletionServiceResult {
    data class Available(val candidates: List<OfficialExpressionMemberCandidate>) : ExpressionMemberCompletionServiceResult

    data object Empty : ExpressionMemberCompletionServiceResult

    data object Unavailable : ExpressionMemberCompletionServiceResult
}

fun interface ExpressionMemberMatcherRunner {
    fun completeReachableMembers(
        classBytes: ByteArray,
        methodName: String,
        methodDescriptor: String,
        receiverExpression: String,
        contextType: OfficialExpressionMatchContextType,
        identifierPool: OfficialExpressionIdentifierPool,
        commonSuperClass: CommonSuperClassResolver,
        cancellationChecker: OfficialExpressionCancellationChecker,
    ): OfficialExpressionMemberCompletionResult

    companion object {
        val DEFAULT = ExpressionMemberMatcherRunner { classBytes, methodName, methodDescriptor, receiverExpression, contextType, identifierPool, commonSuperClass, cancellationChecker ->
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
        }
    }
}

class ExpressionMemberCompletionService(
    private val classIndex: ClassIndex,
    private val bytecodeIndex: BytecodeIndex,
    private val handlerSignatures: HandlerSignatureService = HandlerSignatureService(classIndex, bytecodeIndex),
    cacheCapacity: Int = DEFAULT_CACHE_CAPACITY,
    reachableMembersCache: ExpressionMemberCompletionCache? = null,
    private val matcherRunner: ExpressionMemberMatcherRunner = ExpressionMemberMatcherRunner.DEFAULT,
) {
    private val reachableMembersCache = reachableMembersCache ?: ExpressionMemberCompletionCache(cacheCapacity)

    fun completeMembers(
        source: String,
        context: AnnotationContext,
        mixinTargetOwners: List<String>,
        resolvedContexts: List<ResolvedMixinExtrasContext> = emptyList(),
        cancellationChecker: OfficialExpressionCancellationChecker = OfficialExpressionCancellationContext.current(),
    ): ExpressionMemberCompletionServiceResult {
        if (context.annotation !in EXPRESSION_ANNOTATIONS) {
            return ExpressionMemberCompletionServiceResult.Empty
        }
        if (context.slot != AnnotationSlot.VALUE) {
            return ExpressionMemberCompletionServiceResult.Empty
        }
        if (context.expressionCompletionPosition != ExpressionCompletionPosition.AFTER_DOT) {
            return ExpressionMemberCompletionServiceResult.Empty
        }

        val decoded = context.decodedExpressionPrefix ?: return ExpressionMemberCompletionServiceResult.Empty
        val parsed = ExpressionReceiverParser.parse(decoded.prefix, decoded.cursor)
            ?: return ExpressionMemberCompletionServiceResult.Empty

        val site = ExpressionContextResolver.findEnclosingSite(source, context.valueStartOffset)
            ?: return ExpressionMemberCompletionServiceResult.Unavailable
        if (!site.atValue.equals(MIXINEXTRAS_EXPRESSION_AT_VALUE, ignoreCase = true)) {
            return ExpressionMemberCompletionServiceResult.Unavailable
        }

        val contextType = site.annotation.toOfficialExpressionMatchContextType()
            ?: return ExpressionMemberCompletionServiceResult.Unavailable

        val targetMethod = handlerSignatures.resolveTargetMethod(mixinTargetOwners, site.methodAttribute)
            ?: return ExpressionMemberCompletionServiceResult.Unavailable

        val expressionContext = ExpressionContextResolver.resolveExpressionContextForSite(
            source = source,
            site = site,
            resolvedContexts = resolvedContexts,
        ) ?: return ExpressionMemberCompletionServiceResult.Unavailable

        val typeNameResolver = ClassLiteralTypeNameResolver.forSource(source, classIndex)
        val identifierPool = OfficialExpressionIdentifierPoolBuilder.build(
            expressionContext.definitionIndex,
            typeNameResolver,
        ).pool
        val definitionIndexFingerprint = definitionIndexSemanticFingerprint(expressionContext.definitionIndex, typeNameResolver)
        val eligibleOwners = eligibleOwners(mixinTargetOwners, targetMethod)
        if (eligibleOwners.isEmpty()) {
            return ExpressionMemberCompletionServiceResult.Unavailable
        }

        val commonSuperClass = CommonSuperClassResolver { left, right ->
            bytecodeIndex.resolveCommonSuperClass(left, right)
        }
        val merged = mutableListOf<OfficialExpressionMemberCandidate>()
        val seen = linkedSetOf<MemberSemanticKey>()

        for (owner in eligibleOwners) {
            cancellationChecker.checkCancelled()
            val reachable = reachableMembersForOwner(
                owner = owner,
                receiverExpression = parsed.receiver,
                targetMethod = targetMethod,
                contextType = contextType,
                identifierPool = identifierPool,
                definitionIndexFingerprint = definitionIndexFingerprint,
                commonSuperClass = commonSuperClass,
                cancellationChecker = cancellationChecker,
            ) ?: return ExpressionMemberCompletionServiceResult.Unavailable

            for (candidate in reachable) {
                val key = MemberSemanticKey(
                    kind = candidate.kind,
                    ownerInternalName = candidate.ownerInternalName,
                    name = candidate.name,
                    descriptor = candidate.descriptor,
                )
                if (seen.add(key)) {
                    merged += candidate
                }
            }
        }

        val filtered = filterByMemberPrefix(merged, parsed.memberPrefix)
        return if (filtered.isEmpty()) {
            ExpressionMemberCompletionServiceResult.Empty
        } else {
            ExpressionMemberCompletionServiceResult.Available(filtered)
        }
    }

    private fun reachableMembersForOwner(
        owner: String,
        receiverExpression: String,
        targetMethod: MethodIndexEntry,
        contextType: OfficialExpressionMatchContextType,
        identifierPool: OfficialExpressionIdentifierPool,
        definitionIndexFingerprint: String,
        commonSuperClass: CommonSuperClassResolver,
        cancellationChecker: OfficialExpressionCancellationChecker,
    ): List<OfficialExpressionMemberCandidate>? {
        cancellationChecker.checkCancelled()
        val classBytes = bytecodeIndex.getClassBytes(owner) ?: return null

        val cacheKey = ExpressionMemberCompletionCache.CacheKey(
            receiverExpression = receiverExpression,
            ownerInternalName = owner,
            methodName = targetMethod.name,
            methodDescriptor = targetMethod.descriptor,
            contextType = contextType,
            classBytesFingerprint = classBytesFingerprint(classBytes),
            definitionIndexFingerprint = definitionIndexFingerprint,
        )

        reachableMembersCache.get(cacheKey)?.let { cached ->
            cancellationChecker.checkCancelled()
            return cached
        }

        when (
            val result = matcherRunner.completeReachableMembers(
                classBytes = classBytes,
                methodName = targetMethod.name,
                methodDescriptor = targetMethod.descriptor,
                receiverExpression = receiverExpression,
                contextType = contextType,
                identifierPool = identifierPool,
                commonSuperClass = commonSuperClass,
                cancellationChecker = cancellationChecker,
            )
        ) {
            is OfficialExpressionMemberCompletionResult.Unavailable -> return null
            is OfficialExpressionMemberCompletionResult.Available -> {
                reachableMembersCache.put(cacheKey, result.candidates)
                return result.candidates
            }
        }
    }

    private fun eligibleOwners(
        mixinTargetOwners: List<String>,
        targetMethod: MethodIndexEntry,
    ): List<String> =
        MixinTargetResolver.resolveTargets(mixinTargetOwners, classIndex)
            .filter { owner ->
                classIndex.getMethods(owner).any { method ->
                    method.name == targetMethod.name && method.descriptor == targetMethod.descriptor
                }
            }

    private data class MemberSemanticKey(
        val kind: OfficialExpressionMemberKind,
        val ownerInternalName: String,
        val name: String,
        val descriptor: String,
    )

    private companion object {
        const val DEFAULT_CACHE_CAPACITY = ExpressionMemberCompletionCache.DEFAULT_CAPACITY

        private val EXPRESSION_ANNOTATIONS = setOf(MixinAnnotation.EXPRESSION, MixinAnnotation.EXPRESSIONS)
        private const val MIXINEXTRAS_EXPRESSION_AT_VALUE = "MIXINEXTRAS:EXPRESSION"

        private fun filterByMemberPrefix(
            candidates: List<OfficialExpressionMemberCandidate>,
            memberPrefix: String,
        ): List<OfficialExpressionMemberCandidate> {
            if (memberPrefix.isEmpty()) {
                return candidates
            }
            return candidates.filter { it.name.startsWith(memberPrefix) }
        }

        private fun classBytesFingerprint(classBytes: ByteArray): String {
            val digest = MessageDigest.getInstance("SHA-256")
            digest.update(classBytes)
            return digest.digest().joinToString("") { "%02x".format(it) }
        }

        private fun definitionIndexSemanticFingerprint(
            index: MixinExtrasDefinitionIndex,
            typeNameResolver: ClassLiteralTypeNameResolver,
        ): String =
            index.definitions.joinToString("\u0000") { definition ->
                buildString {
                    append(definition.id ?: "")
                    append('\u0001')
                    append(definition.rawMethodReferences.joinToString("\u0002"))
                    append('\u0001')
                    append(definition.rawFieldReferences.joinToString("\u0002"))
                    append('\u0001')
                    append(definition.classLiteralTypeNames.joinToString("\u0002") {
                        typeNameResolver.resolve(it)?.descriptor ?: "unresolved:$it"
                    })
                    append('\u0001')
                    append(
                        definition.localSpecs.joinToString("\u0002") { local ->
                            buildString {
                                append(local.argsOnly)
                                append('|')
                                append(local.index)
                                append('|')
                                append(local.ordinal)
                                append('|')
                                append(local.names.sorted().joinToString(","))
                                append('|')
                                append(local.print)
                                append('|')
                                append(local.typeClassName?.let {
                                    typeNameResolver.resolve(it)?.descriptor ?: "unresolved:$it"
                                })
                            }
                        },
                    )
                    append('\u0001')
                    append(definition.remap?.toString() ?: "")
                }
            }
    }
}

internal fun MixinExtrasAnnotation.toOfficialExpressionMatchContextType(): OfficialExpressionMatchContextType? =
    when (this) {
        MixinExtrasAnnotation.MODIFY_EXPRESSION_VALUE ->
            OfficialExpressionMatchContextType.MODIFY_EXPRESSION_VALUE
        MixinExtrasAnnotation.MODIFY_RETURN_VALUE ->
            OfficialExpressionMatchContextType.MODIFY_RETURN_VALUE
        MixinExtrasAnnotation.MODIFY_RECEIVER ->
            OfficialExpressionMatchContextType.MODIFY_RECEIVER
        MixinExtrasAnnotation.WRAP_OPERATION ->
            OfficialExpressionMatchContextType.WRAP_OPERATION
        MixinExtrasAnnotation.WRAP_WITH_CONDITION ->
            OfficialExpressionMatchContextType.WRAP_WITH_CONDITION
        MixinExtrasAnnotation.WRAP_METHOD ->
            OfficialExpressionMatchContextType.WRAP_OPERATION
        MixinExtrasAnnotation.INJECT ->
            OfficialExpressionMatchContextType.INJECT
        MixinExtrasAnnotation.REDIRECT ->
            OfficialExpressionMatchContextType.REDIRECT
        MixinExtrasAnnotation.MODIFY_ARG ->
            OfficialExpressionMatchContextType.MODIFY_ARG
        MixinExtrasAnnotation.MODIFY_ARGS ->
            OfficialExpressionMatchContextType.MODIFY_ARGS
        MixinExtrasAnnotation.MODIFY_VARIABLE ->
            OfficialExpressionMatchContextType.MODIFY_VARIABLE
        MixinExtrasAnnotation.MODIFY_CONSTANT ->
            OfficialExpressionMatchContextType.MODIFY_CONSTANT
        else -> null
    }
