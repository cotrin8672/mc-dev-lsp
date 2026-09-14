package io.github.mcdev.core.mixinextras

import com.llamalad7.mixinextras.expression.impl.ExpressionService
import com.llamalad7.mixinextras.expression.impl.ast.expressions.Expression
import com.llamalad7.mixinextras.expression.impl.flow.ComplexDataException
import com.llamalad7.mixinextras.expression.impl.flow.FlowContext
import com.llamalad7.mixinextras.expression.impl.flow.FlowInterpreter
import com.llamalad7.mixinextras.expression.impl.flow.FlowValue
import com.llamalad7.mixinextras.expression.impl.flow.expansion.InsnExpander
import com.llamalad7.mixinextras.expression.impl.point.ExpressionContext
import com.llamalad7.mixinextras.expression.impl.utils.ExpressionASMUtils
import com.llamalad7.mixinextras.expression.impl.utils.ExpressionDecorations
import org.objectweb.asm.ClassReader
import org.objectweb.asm.Type
import org.objectweb.asm.tree.AbstractInsnNode
import org.objectweb.asm.tree.analysis.Analyzer
import org.objectweb.asm.tree.analysis.AnalyzerException
import org.objectweb.asm.tree.ClassNode
import org.objectweb.asm.tree.FieldInsnNode
import org.objectweb.asm.tree.MethodInsnNode
import org.objectweb.asm.tree.MethodNode
import org.objectweb.asm.tree.TypeInsnNode

object OfficialExpressionMatcher {
    init {
        ExpressionService.offerInstance(McdevExpressionService)
    }

    fun completeReceiverMembers(
        classBytes: ByteArray,
        methodName: String,
        methodDescriptor: String,
        receiverExpression: String,
        memberPrefix: String = "",
        contextType: OfficialExpressionMatchContextType = OfficialExpressionMatchContextType.WRAP_OPERATION,
        identifierPool: OfficialExpressionIdentifierPool = OfficialExpressionIdentifierPool.EMPTY,
        allowIncompleteListInputs: Boolean = false,
        commonSuperClass: CommonSuperClassResolver,
        cancellationChecker: OfficialExpressionCancellationChecker = OfficialExpressionCancellationContext.current(),
    ): OfficialExpressionMemberCompletionResult {
        val probes = OfficialExpressionMemberCompletionProbes.build(receiverExpression, identifierPool)
            ?: return OfficialExpressionMemberCompletionResult.Unavailable(
                reason = "Receiver expression cannot be represented in probe syntax: $receiverExpression",
            )

        val parsedProbes = mutableListOf<Pair<OfficialExpressionMemberKind, Expression>>()
        for (probe in probes) {
            cancellationChecker.checkCancelled()
            when (val parseResult = OfficialExpressionParser.parse(probe.source)) {
                is OfficialExpressionParseResult.Success ->
                    parsedProbes += probe.kind to parseResult.expression
                is OfficialExpressionParseResult.SyntaxFailure ->
                    return OfficialExpressionMemberCompletionResult.Unavailable(
                        reason = "Failed to parse receiver probe expression: ${parseResult.message}",
                        cause = parseResult.cause,
                    )
            }
        }

        val resolved = resolveMethodAndFlowEntries(
            classBytes = classBytes,
            methodName = methodName,
            methodDescriptor = methodDescriptor,
            commonSuperClass = commonSuperClass,
            cancellationChecker = cancellationChecker,
        )
        when (resolved) {
            is MethodFlowResolution.Unavailable ->
                return OfficialExpressionMemberCompletionResult.Unavailable(
                    reason = resolved.reason,
                    cause = resolved.cause,
                )
            is MethodFlowResolution.Available -> {
                val methodPool = try {
                    identifierPool.forMethod(classBytes, resolved.method, cancellationChecker)
                } catch (error: LocalCaptureExtractor.LocalCaptureCancellation) {
                    throw unwrapLocalCaptureCancellation(error)
                } catch (error: IllegalStateException) {
                    return OfficialExpressionMemberCompletionResult.Unavailable(error.message ?: "Local analysis unavailable", error)
                }
                val candidates = mutableListOf<OfficialExpressionMemberCandidate>()
                val seen = mutableSetOf<MemberDedupeKey>()

                for ((kind, expression) in parsedProbes) {
                    for ((virtualInsn, flow) in resolved.flowEntries) {
                        cancellationChecker.checkCancelled()
                        val sinkState = MatchSinkState(
                            rootVirtualInsn = virtualInsn,
                            cancellationChecker = cancellationChecker,
                        )
                        val context = ExpressionContext(
                            methodPool,
                            sinkState,
                            resolved.classNode,
                            resolved.method,
                            contextType.toOfficialType(),
                            allowIncompleteListInputs,
                        )
                        val matched = try {
                            expression.matches(flow, context)
                        } catch (exception: RuntimeException) {
                            unwrapFlowCancellation(exception)?.let { throw it }
                            try {
                                cancellationChecker.checkCancelled()
                            } catch (cancellation: Throwable) {
                                throw cancellation
                            }
                            return OfficialExpressionMemberCompletionResult.Unavailable(
                                reason = memberMatchFailureReason(kind, exception),
                                cause = exception,
                            )
                        }
                        if (!matched) {
                            continue
                        }
                        for ((capturedFlow, _) in sinkState.captured) {
                            cancellationChecker.checkCancelled()
                            val capturedVirtualInsn = virtualInsnOrNull(capturedFlow) ?: continue
                            val originalInsn = InsnExpander.getRepresentative(capturedFlow) ?: capturedVirtualInsn.insn
                            val candidate = candidateFromInstruction(
                                kind = kind,
                                originalInsn = originalInsn,
                                method = resolved.method,
                                decorations = convertDecorations(
                                    sinkState.decorationsFor(capturedFlow),
                                ),
                            ) ?: continue
                            if (memberPrefix.isNotEmpty() && !candidate.name.startsWith(memberPrefix)) {
                                continue
                            }
                            val dedupeKey = MemberDedupeKey(
                                kind = candidate.kind,
                                ownerInternalName = candidate.ownerInternalName,
                                name = candidate.name,
                                descriptor = candidate.descriptor,
                            )
                            if (!seen.add(dedupeKey)) {
                                continue
                            }
                            candidates += candidate
                        }
                    }
                }

                return OfficialExpressionMemberCompletionResult.Available(
                    candidates.sortedWith(
                        compareBy(
                            { it.originalInstructionIndex },
                            { it.originalInstructionOpcode },
                            { it.kind.ordinal },
                            { it.name },
                            { it.descriptor },
                        ),
                    ),
                )
            }
        }
    }

    fun match(
        classBytes: ByteArray,
        methodName: String,
        methodDescriptor: String,
        expressions: List<String>,
        contextType: OfficialExpressionMatchContextType,
        identifierPool: OfficialExpressionIdentifierPool = OfficialExpressionIdentifierPool.EMPTY,
        allowIncompleteListInputs: Boolean = false,
        commonSuperClass: CommonSuperClassResolver,
        cancellationChecker: OfficialExpressionCancellationChecker = OfficialExpressionCancellationContext.current(),
    ): OfficialExpressionMatchResult {
        val parsedExpressions = mutableListOf<Expression>()
        for (source in expressions) {
            cancellationChecker.checkCancelled()
            when (val parseResult = OfficialExpressionParser.parse(source)) {
                is OfficialExpressionParseResult.Success -> parsedExpressions += parseResult.expression
                is OfficialExpressionParseResult.SyntaxFailure ->
                    return OfficialExpressionMatchResult.Unavailable(
                        reason = "Failed to parse expression: ${parseResult.message}",
                        cause = parseResult.cause,
                    )
            }
        }

        val resolved = resolveMethodAndFlowEntries(
            classBytes = classBytes,
            methodName = methodName,
            methodDescriptor = methodDescriptor,
            commonSuperClass = commonSuperClass,
            cancellationChecker = cancellationChecker,
        )
        when (resolved) {
            is MethodFlowResolution.Unavailable ->
                return OfficialExpressionMatchResult.Unavailable(
                    reason = resolved.reason,
                    cause = resolved.cause,
                )
            is MethodFlowResolution.Available -> {
                val classNode = resolved.classNode
                val method = resolved.method
                val flowEntries = resolved.flowEntries
                val methodPool = try {
                    identifierPool.forMethod(classBytes, method, cancellationChecker)
                } catch (error: LocalCaptureExtractor.LocalCaptureCancellation) {
                    throw unwrapLocalCaptureCancellation(error)
                } catch (error: IllegalStateException) {
                    return OfficialExpressionMatchResult.Unavailable(error.message ?: "Local analysis unavailable", error)
                }

        val matches = mutableListOf<OfficialExpressionMatch>()
        val seen = mutableSetOf<DedupeKey>()

        parsedExpressions.forEachIndexed { expressionIndex, expression ->
            for ((virtualInsn, flow) in flowEntries) {
                cancellationChecker.checkCancelled()
                val sinkState = MatchSinkState(
                    rootVirtualInsn = virtualInsn,
                    cancellationChecker = cancellationChecker,
                )
                val context = ExpressionContext(
                    methodPool,
                    sinkState,
                    classNode,
                    method,
                    contextType.toOfficialType(),
                    allowIncompleteListInputs,
                )
                val matched = try {
                    expression.matches(flow, context)
                } catch (exception: RuntimeException) {
                    unwrapFlowCancellation(exception)?.let { throw it }
                    try {
                        cancellationChecker.checkCancelled()
                    } catch (cancellation: Throwable) {
                        throw cancellation
                    }
                    return OfficialExpressionMatchResult.Unavailable(
                        reason = matchFailureReason(expressionIndex, exception),
                        cause = exception,
                    )
                }
                if (!matched) {
                    continue
                }
                for ((capturedFlow, startOffset) in sinkState.captured) {
                    cancellationChecker.checkCancelled()
                    val capturedVirtualInsn = virtualInsnOrNull(capturedFlow) ?: continue
                    val originalInsn = InsnExpander.getRepresentative(capturedFlow) ?: capturedVirtualInsn.insn
                    val dedupeKey = DedupeKey(
                        expressionIndex = expressionIndex,
                        originalInsn = originalInsn,
                        captureOffset = startOffset,
                    )
                    if (!seen.add(dedupeKey)) {
                        continue
                    }
                    val instructionIndex = instructionIndex(method, originalInsn)
                    if (instructionIndex < 0) {
                        continue
                    }
                    matches += OfficialExpressionMatch(
                        expressionIndex = expressionIndex,
                        expressionStartOffset = startOffset,
                        originalInstructionOpcode = originalInsn.opcode,
                        originalInstructionIndex = instructionIndex,
                        capturedType = capturedFlow.getType().toTypeConstraint(),
                        decorations = convertDecorations(
                            sinkState.decorationsFor(capturedFlow),
                        ),
                        instructionMetadata = instructionMetadata(originalInsn),
                    )
                }
            }
        }

        return OfficialExpressionMatchResult.Available(
            matches.sortedWith(
                compareBy(
                    { it.expressionIndex },
                    { it.originalInstructionIndex },
                    { it.expressionStartOffset },
                    { it.originalInstructionOpcode },
                ),
            ),
        )
            }
        }
    }

    private sealed interface MethodFlowResolution {
        data class Available(
            val classNode: ClassNode,
            val method: MethodNode,
            val flowEntries: List<Pair<VirtualInsn, FlowValue>>,
        ) : MethodFlowResolution

        data class Unavailable(
            val reason: String,
            val cause: Throwable? = null,
        ) : MethodFlowResolution
    }

    private fun resolveMethodAndFlowEntries(
        classBytes: ByteArray,
        methodName: String,
        methodDescriptor: String,
        commonSuperClass: CommonSuperClassResolver,
        cancellationChecker: OfficialExpressionCancellationChecker,
    ): MethodFlowResolution {
        val classNode = ClassNode()
        try {
            ClassReader(classBytes).accept(classNode, 0)
        } catch (exception: RuntimeException) {
            return MethodFlowResolution.Unavailable(
                reason = "Failed to parse class bytecode",
                cause = exception,
            )
        }

        val method = classNode.methods.find { it.name == methodName && it.desc == methodDescriptor }
            ?: return MethodFlowResolution.Unavailable(
                reason = "Method not found: $methodName$methodDescriptor",
            )

        if (method.instructions == null || method.instructions.size() == 0) {
            return MethodFlowResolution.Unavailable(
                reason = "Method has no instructions: $methodName$methodDescriptor",
            )
        }

        val flowContext = McdevFlowContext(commonSuperClass)
        val flowEntries = try {
            buildFlowEntries(classNode, method, flowContext, cancellationChecker)
        } catch (exception: FlowCancellationSignal) {
            throw exception.cause
        } catch (exception: UnresolvedCommonSuperClassException) {
            return MethodFlowResolution.Unavailable(
                reason = exception.message ?: "Could not resolve common super class",
                cause = exception,
            )
        } catch (exception: RuntimeException) {
            unwrapFlowCancellation(exception)?.let { throw it }
            return MethodFlowResolution.Unavailable(
                reason = "Failed to analyze method bytecode flow",
                cause = exception,
            )
        }

        if (flowEntries.isEmpty()) {
            return MethodFlowResolution.Unavailable(
                reason = "Failed to build flow map for method: $methodName$methodDescriptor",
            )
        }

        return MethodFlowResolution.Available(
            classNode = classNode,
            method = method,
            flowEntries = flowEntries,
        )
    }

    private fun memberMatchFailureReason(kind: OfficialExpressionMemberKind, exception: RuntimeException): String {
        val detail = exception.message?.takeIf { it.isNotBlank() }
            ?: exception.javaClass.simpleName
        return "Failed to match ${kind.name.lowercase()} receiver probe: $detail"
    }

    private fun candidateFromInstruction(
        kind: OfficialExpressionMemberKind,
        originalInsn: AbstractInsnNode,
        method: MethodNode,
        decorations: Map<OfficialExpressionMatchDecorationKey, OfficialExpressionMatchDecorationValue>,
    ): OfficialExpressionMemberCandidate? {
        val instructionIndex = instructionIndex(method, originalInsn)
        if (instructionIndex < 0) {
            return null
        }
        return when (val metadata = instructionMetadata(originalInsn)) {
            is OfficialExpressionMatchInstructionMetadata.MethodInvocation ->
                if (kind == OfficialExpressionMemberKind.METHOD) {
                    OfficialExpressionMemberCandidate.MethodInvocation(
                        ownerInternalName = metadata.ownerInternalName,
                        name = metadata.name,
                        descriptor = metadata.descriptor,
                        isInterface = metadata.isInterface,
                        originalInstructionOpcode = originalInsn.opcode,
                        originalInstructionIndex = instructionIndex,
                        decorations = decorations,
                    )
                } else {
                    null
                }
            is OfficialExpressionMatchInstructionMetadata.FieldAccess ->
                if (kind == OfficialExpressionMemberKind.FIELD) {
                    OfficialExpressionMemberCandidate.FieldAccess(
                        ownerInternalName = metadata.ownerInternalName,
                        name = metadata.name,
                        descriptor = metadata.descriptor,
                        originalInstructionOpcode = originalInsn.opcode,
                        originalInstructionIndex = instructionIndex,
                        decorations = decorations,
                    )
                } else {
                    null
                }
            else -> null
        }
    }

    private fun buildFlowEntries(
        classNode: ClassNode,
        method: MethodNode,
        flowContext: McdevFlowContext,
        cancellationChecker: OfficialExpressionCancellationChecker,
    ): List<Pair<VirtualInsn, FlowValue>> {
        val interpreter = CancellableFlowInterpreter(classNode, method, flowContext, cancellationChecker)
        try {
            checkFlowCancellation(cancellationChecker)
            Analyzer(interpreter).analyze(classNode.name, method)
            checkFlowCancellation(cancellationChecker)
        } catch (exception: FlowCancellationSignal) {
            throw exception
        } catch (exception: AnalyzerException) {
            unwrapFlowCancellation(exception)?.let { throw FlowCancellationSignal(it) }
            throw RuntimeException("Failed to analyze value flow: ", exception)
        }

        checkFlowCancellation(cancellationChecker)
        val flows = interpreter.finish()
        return flows.mapNotNull { flow ->
            checkFlowCancellation(cancellationChecker)
            virtualInsnOrNull(flow)?.let { virtualInsn -> virtualInsn to flow }
        }
    }

    private fun checkFlowCancellation(cancellationChecker: OfficialExpressionCancellationChecker) {
        try {
            cancellationChecker.checkCancelled()
        } catch (throwable: Throwable) {
            throw FlowCancellationSignal(throwable)
        }
    }

    private fun matchFailureReason(expressionIndex: Int, exception: RuntimeException): String {
        val detail = exception.message?.takeIf { it.isNotBlank() }
            ?: exception.javaClass.simpleName
        return "Failed to match expression at index $expressionIndex: $detail"
    }

    private fun unwrapFlowCancellation(exception: Throwable): Throwable? {
        var current: Throwable? = exception
        while (current != null) {
            if (current is FlowCancellationSignal) {
                return current.cause
            }
            current = current.cause
        }
        return null
    }

    private fun unwrapLocalCaptureCancellation(
        exception: LocalCaptureExtractor.LocalCaptureCancellation,
    ): Throwable {
        var current: Throwable = exception.cause
        while (current is LocalCaptureExtractor.LocalCaptureCancellation) {
            current = current.cause
        }
        return current
    }

    private fun instructionMetadata(
        originalInsn: AbstractInsnNode,
    ): OfficialExpressionMatchInstructionMetadata? =
        when (originalInsn) {
            is MethodInsnNode ->
                OfficialExpressionMatchInstructionMetadata.MethodInvocation(
                    ownerInternalName = originalInsn.owner,
                    name = originalInsn.name,
                    descriptor = originalInsn.desc,
                    isInterface = originalInsn.itf,
                )
            is FieldInsnNode ->
                OfficialExpressionMatchInstructionMetadata.FieldAccess(
                    ownerInternalName = originalInsn.owner,
                    name = originalInsn.name,
                    descriptor = originalInsn.desc,
                )
            is TypeInsnNode ->
                OfficialExpressionMatchInstructionMetadata.TypeOperation(
                    typeInternalName = originalInsn.desc,
                )
            else -> null
        }

    private fun instructionIndex(method: MethodNode, target: AbstractInsnNode): Int {
        var physicalIndex = 0
        var insn = method.instructions.first
        while (insn != null) {
            if (insn === target) {
                return if (insn.opcode >= 0) physicalIndex else -1
            }
            if (insn.opcode >= 0) {
                physicalIndex++
            }
            insn = insn.next
        }
        return -1
    }

    private fun convertDecorations(
        decorations: Map<String, Any?>,
    ): Map<OfficialExpressionMatchDecorationKey, OfficialExpressionMatchDecorationValue> {
        val converted = linkedMapOf<OfficialExpressionMatchDecorationKey, OfficialExpressionMatchDecorationValue>()
        convertDecoration(
            decorations,
            ExpressionDecorations.SIMPLE_EXPRESSION_TYPE,
            OfficialExpressionMatchDecorationKey.SIMPLE_EXPRESSION_TYPE,
        ) { value ->
            (value as? Type)?.toTypeConstraint()?.let(OfficialExpressionMatchDecorationValue::TypeConstraint)
        }?.let { converted[it.first] = it.second }

        convertDecoration(
            decorations,
            ExpressionDecorations.SIMPLE_OPERATION_ARGS,
            OfficialExpressionMatchDecorationKey.SIMPLE_OPERATION_ARGS,
        ) { value ->
            (value as? Array<*>)?.filterIsInstance<Type>()?.map { it.toTypeConstraint() }
                ?.let(OfficialExpressionMatchDecorationValue::TypeConstraints)
        }?.let { converted[it.first] = it.second }

        convertDecoration(
            decorations,
            ExpressionDecorations.SIMPLE_OPERATION_PARAM_NAMES,
            OfficialExpressionMatchDecorationKey.SIMPLE_OPERATION_PARAM_NAMES,
        ) { value ->
            (value as? Array<*>)?.filterIsInstance<String>()
                ?.let(OfficialExpressionMatchDecorationValue::ParamNames)
        }?.let { converted[it.first] = it.second }

        convertDecoration(
            decorations,
            ExpressionDecorations.SIMPLE_OPERATION_RETURN_TYPE,
            OfficialExpressionMatchDecorationKey.SIMPLE_OPERATION_RETURN_TYPE,
        ) { value ->
            (value as? Type)?.toTypeConstraint()?.let(OfficialExpressionMatchDecorationValue::TypeConstraint)
        }?.let { converted[it.first] = it.second }

        convertDecoration(
            decorations,
            ExpressionDecorations.IS_STRING_CONCAT_EXPRESSION,
            OfficialExpressionMatchDecorationKey.IS_STRING_CONCAT_EXPRESSION,
        ) { value ->
            (value as? Boolean)?.let(OfficialExpressionMatchDecorationValue::Flag)
        }?.let { converted[it.first] = it.second }

        return converted
    }

    private inline fun <T> convertDecoration(
        decorations: Map<String, Any?>,
        officialKey: String,
        repoKey: OfficialExpressionMatchDecorationKey,
        convert: (Any?) -> T?,
    ): Pair<OfficialExpressionMatchDecorationKey, T>? {
        if (!decorations.containsKey(officialKey)) {
            return null
        }
        val converted = convert(decorations[officialKey]) ?: return null
        return repoKey to converted
    }

    @JvmInline
    private value class VirtualInsn(val insn: AbstractInsnNode)

    private data class MemberDedupeKey(
        val kind: OfficialExpressionMemberKind,
        val ownerInternalName: String,
        val name: String,
        val descriptor: String,
    )

    private data class DedupeKey(
        val expressionIndex: Int,
        val originalInsn: AbstractInsnNode,
        val captureOffset: Int,
    )

    private class McdevFlowContext(
        val commonSuperClass: CommonSuperClassResolver,
    ) : FlowContext

    private class FlowCancellationSignal(
        override val cause: Throwable,
    ) : RuntimeException(cause)

    private class CancellableFlowInterpreter(
        classNode: ClassNode,
        methodNode: MethodNode,
        ctx: FlowContext,
        private val cancellationChecker: OfficialExpressionCancellationChecker,
    ) : FlowInterpreter(classNode, methodNode, ctx) {
        override fun newValue(type: Type?): FlowValue? {
            checkFlowCancellation(cancellationChecker)
            return super.newValue(type)
        }

        override fun newOperation(insn: AbstractInsnNode?): FlowValue? {
            checkFlowCancellation(cancellationChecker)
            return super.newOperation(insn)
        }

        override fun copyOperation(insn: AbstractInsnNode?, value: FlowValue?): FlowValue? {
            checkFlowCancellation(cancellationChecker)
            return super.copyOperation(insn, value)
        }

        override fun unaryOperation(insn: AbstractInsnNode?, value: FlowValue?): FlowValue? {
            checkFlowCancellation(cancellationChecker)
            return super.unaryOperation(insn, value)
        }

        override fun binaryOperation(
            insn: AbstractInsnNode?,
            value1: FlowValue?,
            value2: FlowValue?,
        ): FlowValue? {
            checkFlowCancellation(cancellationChecker)
            return super.binaryOperation(insn, value1, value2)
        }

        override fun ternaryOperation(
            insn: AbstractInsnNode?,
            value1: FlowValue?,
            value2: FlowValue?,
            value3: FlowValue?,
        ): FlowValue? {
            checkFlowCancellation(cancellationChecker)
            return super.ternaryOperation(insn, value1, value2, value3)
        }

        override fun naryOperation(insn: AbstractInsnNode?, values: MutableList<out FlowValue>?): FlowValue? {
            checkFlowCancellation(cancellationChecker)
            return super.naryOperation(insn, values)
        }

        override fun returnOperation(insn: AbstractInsnNode?, value: FlowValue?, expected: FlowValue?) {
            checkFlowCancellation(cancellationChecker)
            super.returnOperation(insn, value, expected)
        }

        override fun merge(value1: FlowValue?, value2: FlowValue?): FlowValue? {
            checkFlowCancellation(cancellationChecker)
            return super.merge(value1, value2)
        }
    }

    private class UnresolvedCommonSuperClassException(
        type1: Type,
        type2: Type,
    ) : RuntimeException("Could not resolve common super class of $type1 and $type2")

    private object McdevExpressionService : ExpressionService() {
        override fun getCommonSuperClass(ctx: FlowContext, type1: Type, type2: Type): Type {
            val flowContext = ctx as McdevFlowContext
            val descriptor = flowContext.commonSuperClass.resolve(type1.descriptor, type2.descriptor)
                ?: throw UnresolvedCommonSuperClassException(type1, type2)
            return Type.getType(descriptor)
        }
    }

    private class MatchSinkState(
        private val rootVirtualInsn: VirtualInsn,
        private val cancellationChecker: OfficialExpressionCancellationChecker,
    ) : Expression.OutputSink {
        val captured = mutableListOf<Pair<FlowValue, Int>>()
        private val decorations = mutableMapOf<VirtualInsn, MutableMap<String, Any?>>()

        override fun capture(node: FlowValue, expr: Expression, ctx: ExpressionContext) {
            captured += node to expr.src.startIndex
            if (node.decorations.isNotEmpty()) {
                decorations.getOrPut(rootVirtualInsn, ::linkedMapOf).putAll(node.decorations)
            }
        }

        override fun decorate(insn: AbstractInsnNode, key: String, value: Any?) {
            decorations.getOrPut(VirtualInsn(insn), ::linkedMapOf)[key] = value
        }

        override fun decorateInjectorSpecific(insn: AbstractInsnNode, key: String, value: Any?) {
            decorate(insn, key, value)
        }

        override fun reportMatchStatus(node: FlowValue, expr: Expression, matched: Boolean) {
            checkFlowCancellation(cancellationChecker)
        }

        override fun reportPartialMatch(node: FlowValue, expr: Expression) {
            checkFlowCancellation(cancellationChecker)
        }

        fun decorationsFor(capturedFlow: FlowValue): Map<String, Any?> {
            val capturedInsn = virtualInsnOrNull(capturedFlow) ?: return emptyMap()
            return decorations[capturedInsn]?.toMap() ?: emptyMap()
        }
    }

    private fun virtualInsnOrNull(flow: FlowValue): VirtualInsn? =
        try {
            VirtualInsn(flow.insn)
        } catch (_: ComplexDataException) {
            null
        }

    private fun Type.toTypeConstraint(): OfficialExpressionTypeConstraint =
        if (this == ExpressionASMUtils.INTLIKE_TYPE) {
            OfficialExpressionTypeConstraint.IntLike
        } else {
            OfficialExpressionTypeConstraint.Exact(descriptor)
        }
}
