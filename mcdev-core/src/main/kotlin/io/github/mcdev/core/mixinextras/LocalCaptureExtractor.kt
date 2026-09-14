package io.github.mcdev.core.mixinextras

import org.objectweb.asm.ClassReader
import org.objectweb.asm.Opcodes
import org.objectweb.asm.Type
import org.objectweb.asm.tree.AbstractInsnNode
import org.objectweb.asm.tree.ClassNode
import org.objectweb.asm.tree.LocalVariableNode
import org.objectweb.asm.tree.MethodNode
import org.objectweb.asm.tree.analysis.Analyzer
import org.objectweb.asm.tree.analysis.AnalyzerException
import org.objectweb.asm.tree.analysis.BasicInterpreter
import org.objectweb.asm.tree.analysis.BasicValue
import org.objectweb.asm.tree.analysis.Frame

object LocalCaptureExtractor {
    fun extract(
        classBytes: ByteArray?,
        methodName: String,
        methodDescriptor: String,
        instructionOccurrenceIndices: Set<Int>,
        cancellationChecker: OfficialExpressionCancellationChecker = OfficialExpressionCancellationChecker.NONE,
    ): LocalCaptureResult {
        checkCancellation(cancellationChecker)
        if (classBytes == null) {
            return LocalCaptureResult.Failure(LocalCaptureMissingClassBytesError)
        }
        if (classBytes.isEmpty()) {
            return LocalCaptureResult.Failure(LocalCaptureEmptyClassBytesError)
        }

        val classNode = ClassNode()
        try {
            ClassReader(classBytes).accept(classNode, ClassReader.SKIP_FRAMES)
        } catch (exception: Exception) {
            return LocalCaptureResult.Failure(
                LocalCaptureCorruptClassBytesError(
                    exception.message ?: exception.javaClass.simpleName,
                ),
            )
        }
        checkCancellation(cancellationChecker)

        val method = classNode.methods.firstOrNull { candidate ->
            candidate.name == methodName && candidate.desc == methodDescriptor
        } ?: return LocalCaptureResult.Failure(
            LocalCaptureMethodNotFoundError(methodName, methodDescriptor),
        )
        checkCancellation(cancellationChecker)

        val realInstructionInsnListIndices = realInstructionInsnListIndices(method)
        val instructionCount = realInstructionInsnListIndices.size
        for (index in instructionOccurrenceIndices) {
            checkCancellation(cancellationChecker)
            if (index < 0 || index >= instructionCount) {
                return LocalCaptureResult.Failure(
                    LocalCaptureInvalidInstructionIndexError(index, instructionCount),
                )
            }
        }

        if (instructionOccurrenceIndices.isEmpty()) {
            return LocalCaptureResult.Success(emptyList())
        }

        val frames = try {
            Analyzer(ExactDescriptorInterpreter(cancellationChecker)).analyze(classNode.name, method)
        } catch (exception: LocalCaptureCancellation) {
            if (exception.cause is LocalCaptureCancellation) {
                throw exception
            }
            throw exception.cause
        } catch (exception: AnalyzerException) {
            rethrowCancellation(exception)
            return LocalCaptureResult.Failure(
                LocalCaptureAnalysisError(exception.message ?: exception.javaClass.simpleName),
            )
        } catch (exception: RuntimeException) {
            rethrowCancellation(exception)
            return LocalCaptureResult.Failure(
                LocalCaptureAnalysisError(exception.message ?: exception.javaClass.simpleName),
            )
        }

        val isStatic = method.access and Opcodes.ACC_STATIC != 0
        val argumentSlots = argumentSlots(method)

        val snapshots = instructionOccurrenceIndices
            .sorted()
            .map { occurrenceIndex ->
                checkCancellation(cancellationChecker)
                val insnListIndex = realInstructionInsnListIndices[occurrenceIndex]
                val frame = frames[insnListIndex]
                    ?: return LocalCaptureResult.Failure(
                        LocalCaptureAnalysisError(
                            "missing frame for instruction occurrence index $occurrenceIndex",
                        ),
                    )
                val instruction = method.instructions[insnListIndex]
                LocalCaptureSnapshot(
                    instructionOccurrenceIndex = occurrenceIndex,
                    candidates = extractCandidates(
                        frame = frame,
                        ownerInternalName = classNode.name,
                        method = method,
                        instruction = instruction,
                        argumentSlots = argumentSlots,
                        excludeReceiverSlot = !isStatic,
                        cancellationChecker = cancellationChecker,
                    ),
                )
            }

        return LocalCaptureResult.Success(snapshots)
    }

    private fun realInstructionInsnListIndices(method: MethodNode): List<Int> {
        val indices = mutableListOf<Int>()
        var insnListIndex = 0
        var insn: AbstractInsnNode? = method.instructions.first
        while (insn != null) {
            if (insn.opcode >= 0) {
                indices += insnListIndex
            }
            insnListIndex++
            insn = insn.next
        }
        return indices
    }

    private fun argumentSlots(method: MethodNode): Set<Int> {
        val argumentTypes = Type.getArgumentTypes(method.desc)
        val isStatic = method.access and Opcodes.ACC_STATIC != 0
        var slot = if (isStatic) 0 else 1
        val slots = linkedSetOf<Int>()
        for (argumentType in argumentTypes) {
            slots += slot
            slot += argumentType.size
        }
        return slots
    }

    private fun extractCandidates(
        frame: Frame<BasicValue>,
        ownerInternalName: String,
        method: MethodNode,
        instruction: AbstractInsnNode,
        argumentSlots: Set<Int>,
        excludeReceiverSlot: Boolean,
        cancellationChecker: OfficialExpressionCancellationChecker,
    ): List<LocalCaptureCandidate> {
        val candidates = mutableListOf<LocalCaptureCandidate>()
        val ordinalsByDescriptor = mutableMapOf<String, Int>()
        var slot = 0
        while (slot < frame.locals) {
            checkCancellation(cancellationChecker)

            val value = frame.getLocal(slot)
            if (!isLiveLocalValue(value)) {
                slot++
                continue
            }

            val type = value.type
            if (type == null) {
                slot++
                continue
            }

            val descriptor = localVariableDescriptor(method, slot, instruction)
                ?: declaredMethodLocalDescriptor(ownerInternalName, method, slot)
                ?: type.descriptor
            val ordinal = ordinalsByDescriptor.getOrDefault(descriptor, 0)
            ordinalsByDescriptor[descriptor] = ordinal + 1
            if (excludeReceiverSlot && slot == 0) {
                slot += type.size
                continue
            }

            candidates += LocalCaptureCandidate(
                slotIndex = slot,
                descriptor = descriptor,
                name = localVariableName(method, slot, instruction),
                isArgument = slot in argumentSlots,
                ordinal = ordinal,
            )
            slot += type.size
        }
        return candidates.sortedBy { it.slotIndex }
    }

    private fun isLiveLocalValue(value: BasicValue): Boolean {
        if (value === BasicValue.UNINITIALIZED_VALUE) {
            return false
        }
        return value.type != null
    }

    private fun localVariableName(
        method: MethodNode,
        slot: Int,
        instruction: AbstractInsnNode,
    ): String? {
        val instructionIndex = method.instructions.indexOf(instruction)
        if (instructionIndex < 0) {
            return null
        }
        return method.localVariables
            ?.asSequence()
            ?.filter { localVariable -> localVariable.index == slot }
            ?.filter { localVariable -> isLocalVariableLiveAt(method, localVariable, instructionIndex) }
            ?.map(LocalVariableNode::name)
            ?.firstOrNull()
    }

    private fun localVariableDescriptor(
        method: MethodNode,
        slot: Int,
        instruction: AbstractInsnNode,
    ): String? {
        val instructionIndex = method.instructions.indexOf(instruction)
        if (instructionIndex < 0) {
            return null
        }
        return method.localVariables
            ?.asSequence()
            ?.filter { localVariable -> localVariable.index == slot }
            ?.filter { localVariable -> isLocalVariableLiveAt(method, localVariable, instructionIndex) }
            ?.map(LocalVariableNode::desc)
            ?.firstOrNull()
    }

    private fun declaredMethodLocalDescriptor(
        ownerInternalName: String,
        method: MethodNode,
        slot: Int,
    ): String? {
        val isStatic = method.access and Opcodes.ACC_STATIC != 0
        if (!isStatic && slot == 0) {
            return Type.getObjectType(ownerInternalName).descriptor
        }

        var argumentSlot = if (isStatic) 0 else 1
        for (argumentType in Type.getArgumentTypes(method.desc)) {
            if (slot == argumentSlot) {
                return argumentType.descriptor
            }
            argumentSlot += argumentType.size
        }
        return null
    }

    private fun isLocalVariableLiveAt(
        method: MethodNode,
        localVariable: LocalVariableNode,
        instructionIndex: Int,
    ): Boolean {
        val startIndex = method.instructions.indexOf(localVariable.start)
        val endIndex = method.instructions.indexOf(localVariable.end)
        if (startIndex < 0 || endIndex < 0) {
            return false
        }
        return instructionIndex in startIndex until endIndex
    }

    /**
     * Retains exact OBJECT/ARRAY descriptors in local frames. Plain [BasicInterpreter] collapses
     * all references to [Type.OBJECT], which loses live local typing needed for capture.
     */
    private class ExactDescriptorInterpreter(
        private val cancellationChecker: OfficialExpressionCancellationChecker,
    ) : BasicInterpreter(Opcodes.ASM9) {
        override fun newValue(type: Type?): BasicValue? {
            checkAnalyzerCancellation(cancellationChecker)
            if (type == null) {
                return BasicValue.UNINITIALIZED_VALUE
            }
            return when (type.sort) {
                Type.VOID -> null
                Type.OBJECT, Type.ARRAY -> BasicValue(type)
                else -> super.newValue(type)
            }
        }

        override fun newOperation(insn: AbstractInsnNode?): BasicValue? {
            checkAnalyzerCancellation(cancellationChecker)
            return super.newOperation(insn)
        }

        override fun copyOperation(insn: AbstractInsnNode?, value: BasicValue?): BasicValue? {
            checkAnalyzerCancellation(cancellationChecker)
            return super.copyOperation(insn, value)
        }

        override fun unaryOperation(insn: AbstractInsnNode?, value: BasicValue?): BasicValue? {
            checkAnalyzerCancellation(cancellationChecker)
            return super.unaryOperation(insn, value)
        }

        override fun binaryOperation(
            insn: AbstractInsnNode?,
            value1: BasicValue?,
            value2: BasicValue?,
        ): BasicValue? {
            checkAnalyzerCancellation(cancellationChecker)
            return super.binaryOperation(insn, value1, value2)
        }

        override fun ternaryOperation(
            insn: AbstractInsnNode?,
            value1: BasicValue?,
            value2: BasicValue?,
            value3: BasicValue?,
        ): BasicValue? {
            checkAnalyzerCancellation(cancellationChecker)
            return super.ternaryOperation(insn, value1, value2, value3)
        }

        override fun naryOperation(
            insn: AbstractInsnNode?,
            values: MutableList<out BasicValue>?,
        ): BasicValue? {
            checkAnalyzerCancellation(cancellationChecker)
            return super.naryOperation(insn, values)
        }

        override fun returnOperation(
            insn: AbstractInsnNode?,
            value: BasicValue?,
            expected: BasicValue?,
        ) {
            checkAnalyzerCancellation(cancellationChecker)
            super.returnOperation(insn, value, expected)
        }

        override fun merge(value: BasicValue, other: BasicValue): BasicValue {
            checkAnalyzerCancellation(cancellationChecker)
            if (value == other) {
                return value
            }
            val valueType = value.type
            val otherType = other.type
            if (valueType != null && otherType != null) {
                val valueSort = valueType.sort
                val otherSort = otherType.sort
                if ((valueSort == Type.OBJECT || valueSort == Type.ARRAY) &&
                    (otherSort == Type.OBJECT || otherSort == Type.ARRAY)
                ) {
                    return if (valueType == otherType) {
                        value
                    } else {
                        BasicValue.UNINITIALIZED_VALUE
                    }
                }
            }
            return super.merge(value, other)
        }
    }

    internal class LocalCaptureCancellation(
        override val cause: Throwable,
    ) : RuntimeException(cause)

    private fun rethrowCancellation(exception: Throwable) {
        val signal = findCancellationSignal(exception) ?: return
        if (signal.cause is LocalCaptureCancellation) {
            throw signal
        }
        throw signal.cause
    }

    private fun checkCancellation(cancellationChecker: OfficialExpressionCancellationChecker) {
        cancellationChecker.checkCancelled()
    }

    private fun checkAnalyzerCancellation(cancellationChecker: OfficialExpressionCancellationChecker) {
        try {
            cancellationChecker.checkCancelled()
        } catch (throwable: Throwable) {
            throw LocalCaptureCancellation(throwable)
        }
    }

    private fun findCancellationSignal(exception: Throwable): LocalCaptureCancellation? {
        var current: Throwable? = exception
        while (current != null) {
            if (current is LocalCaptureCancellation) {
                return current
            }
            current = current.cause
        }
        return null
    }
}
