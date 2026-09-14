package io.github.mcdev.core.bytecode

import org.objectweb.asm.ClassReader
import org.objectweb.asm.Opcodes
import org.objectweb.asm.Type
import org.objectweb.asm.tree.AbstractInsnNode
import org.objectweb.asm.tree.ClassNode
import org.objectweb.asm.tree.FieldInsnNode
import org.objectweb.asm.tree.IntInsnNode
import org.objectweb.asm.tree.JumpInsnNode
import org.objectweb.asm.tree.LdcInsnNode
import org.objectweb.asm.tree.MethodInsnNode
import org.objectweb.asm.tree.MethodNode
import org.objectweb.asm.tree.TypeInsnNode

object InstructionExtractor {
    fun extract(
        classBytes: ByteArray,
        methodName: String,
        methodDescriptor: String,
    ): List<AtTargetCandidate> {
        val classNode = ClassNode()
        ClassReader(classBytes).accept(classNode, ClassReader.SKIP_DEBUG)
        val methodNode = classNode.methods.firstOrNull { method ->
            method.name == methodName && method.desc == methodDescriptor
        } ?: return emptyList()
        val candidates = extractFromMethod(methodNode)
        return OrdinalCalculator.assignOrdinals(candidates)
    }

    private fun extractFromMethod(method: MethodNode): List<AtTargetCandidate> {
        val candidates = mutableListOf<AtTargetCandidate>()
        var instructionIndex = 0
        var insn: AbstractInsnNode? = method.instructions.first
        while (insn != null) {
            if (insn.opcode < 0) {
                insn = insn.next
                continue
            }
            when (insn) {
                is MethodInsnNode -> candidates += invokeCandidate(insn, instructionIndex)
                is FieldInsnNode -> candidates += fieldCandidate(insn, instructionIndex)
                is TypeInsnNode -> when (insn.opcode) {
                    Opcodes.NEW -> candidates += AtTargetCandidate(
                        owner = insn.desc,
                        name = "",
                        descriptor = "L${insn.desc};",
                        ordinal = 0,
                        kind = AtTargetKind.NEW,
                        instructionOccurrenceIndex = instructionIndex,
                        occurrenceResultClassification = OccurrenceResultClassification.NOT_APPLICABLE,
                    )
                    Opcodes.CHECKCAST, Opcodes.INSTANCEOF -> candidates += AtTargetCandidate(
                        owner = "",
                        name = "",
                        descriptor = "",
                        ordinal = 0,
                        kind = AtTargetKind.CONSTANT,
                        constantValue = ConstantValue.ClassLiteral(insn.desc),
                        instructionOccurrenceIndex = instructionIndex,
                        occurrenceResultClassification = OccurrenceResultClassification.NOT_APPLICABLE,
                    )
                }
                is IntInsnNode -> if (insn.opcode == Opcodes.BIPUSH || insn.opcode == Opcodes.SIPUSH) {
                    candidates += AtTargetCandidate(
                        owner = "",
                        name = "",
                        descriptor = "",
                        ordinal = 0,
                        kind = AtTargetKind.CONSTANT,
                        constantValue = ConstantValue.IntValue(insn.operand),
                        instructionOccurrenceIndex = instructionIndex,
                        occurrenceResultClassification = OccurrenceResultClassification.NOT_APPLICABLE,
                    )
                }
                is LdcInsnNode -> toConstantValue(insn.cst)?.let { constant ->
                    candidates += AtTargetCandidate(
                        owner = "",
                        name = "",
                        descriptor = "",
                        ordinal = 0,
                        kind = AtTargetKind.CONSTANT,
                        constantValue = constant,
                        instructionOccurrenceIndex = instructionIndex,
                        occurrenceResultClassification = OccurrenceResultClassification.NOT_APPLICABLE,
                    )
                }
                is JumpInsnNode -> when (insn.opcode) {
                    Opcodes.IFLT,
                    Opcodes.IFGE,
                    Opcodes.IFGT,
                    Opcodes.IFLE,
                    -> if (!isComparisonPreceded(insn)) {
                        candidates += conditionCandidate(insn, instructionIndex)
                    }
                }
                else -> when (insn.opcode) {
                    Opcodes.ICONST_M1,
                    Opcodes.ICONST_0,
                    Opcodes.ICONST_1,
                    Opcodes.ICONST_2,
                    Opcodes.ICONST_3,
                    Opcodes.ICONST_4,
                    Opcodes.ICONST_5,
                    Opcodes.LCONST_0,
                    Opcodes.LCONST_1,
                    Opcodes.FCONST_0,
                    Opcodes.FCONST_1,
                    Opcodes.FCONST_2,
                    Opcodes.DCONST_0,
                    Opcodes.DCONST_1,
                    Opcodes.ACONST_NULL,
                    -> candidates += constantCandidate(insn, instructionIndex)
                    Opcodes.IRETURN,
                    Opcodes.LRETURN,
                    Opcodes.FRETURN,
                    Opcodes.DRETURN,
                    Opcodes.ARETURN,
                    Opcodes.RETURN,
                    -> candidates += AtTargetCandidate(
                        owner = "",
                        name = "RETURN",
                        descriptor = "",
                        ordinal = 0,
                        kind = AtTargetKind.RETURN,
                        instructionOccurrenceIndex = instructionIndex,
                        occurrenceResultClassification = OccurrenceResultClassification.NOT_APPLICABLE,
                    )
                }
            }
            instructionIndex += 1
            insn = insn.next
        }
        return candidates
    }

    private fun invokeCandidate(insn: MethodInsnNode, instructionIndex: Int): AtTargetCandidate {
        val kind = when (insn.opcode) {
            Opcodes.INVOKEVIRTUAL -> AtTargetKind.INVOKE_VIRTUAL
            Opcodes.INVOKESPECIAL -> AtTargetKind.INVOKE_SPECIAL
            Opcodes.INVOKESTATIC -> AtTargetKind.INVOKE_STATIC
            Opcodes.INVOKEINTERFACE -> AtTargetKind.INVOKE_INTERFACE
            else -> error("unexpected invoke opcode: ${insn.opcode}")
        }
        return AtTargetCandidate(
            owner = insn.owner,
            name = insn.name,
            descriptor = insn.desc,
            ordinal = 0,
            kind = kind,
            instructionOccurrenceIndex = instructionIndex,
            occurrenceResultClassification = classifyMethodInvokeResult(Type.getReturnType(insn.desc), insn),
        )
    }

    private fun fieldCandidate(insn: FieldInsnNode, instructionIndex: Int): AtTargetCandidate {
        val kind = when (insn.opcode) {
            Opcodes.GETFIELD -> AtTargetKind.FIELD_GET_INSTANCE
            Opcodes.PUTFIELD -> AtTargetKind.FIELD_PUT_INSTANCE
            Opcodes.GETSTATIC -> AtTargetKind.FIELD_GET_STATIC
            Opcodes.PUTSTATIC -> AtTargetKind.FIELD_PUT_STATIC
            else -> error("unexpected field opcode: ${insn.opcode}")
        }
        val classification = when (kind) {
            AtTargetKind.FIELD_GET_INSTANCE,
            AtTargetKind.FIELD_GET_STATIC,
            -> OccurrenceResultClassification.RETAINED
            AtTargetKind.FIELD_PUT_INSTANCE,
            AtTargetKind.FIELD_PUT_STATIC,
            -> OccurrenceResultClassification.VOID
            else -> OccurrenceResultClassification.NOT_APPLICABLE
        }
        return AtTargetCandidate(
            owner = insn.owner,
            name = insn.name,
            descriptor = insn.desc,
            ordinal = 0,
            kind = kind,
            instructionOccurrenceIndex = instructionIndex,
            occurrenceResultClassification = classification,
        )
    }

    private fun constantCandidate(insn: AbstractInsnNode, instructionIndex: Int): AtTargetCandidate {
        val constant = when (insn.opcode) {
            Opcodes.ICONST_M1 -> ConstantValue.IntValue(-1)
            Opcodes.ICONST_0 -> ConstantValue.IntValue(0)
            Opcodes.ICONST_1 -> ConstantValue.IntValue(1)
            Opcodes.ICONST_2 -> ConstantValue.IntValue(2)
            Opcodes.ICONST_3 -> ConstantValue.IntValue(3)
            Opcodes.ICONST_4 -> ConstantValue.IntValue(4)
            Opcodes.ICONST_5 -> ConstantValue.IntValue(5)
            Opcodes.LCONST_0 -> ConstantValue.LongValue(0L)
            Opcodes.LCONST_1 -> ConstantValue.LongValue(1L)
            Opcodes.FCONST_0 -> ConstantValue.FloatValue(0f)
            Opcodes.FCONST_1 -> ConstantValue.FloatValue(1f)
            Opcodes.FCONST_2 -> ConstantValue.FloatValue(2f)
            Opcodes.DCONST_0 -> ConstantValue.DoubleValue(0.0)
            Opcodes.DCONST_1 -> ConstantValue.DoubleValue(1.0)
            Opcodes.ACONST_NULL -> ConstantValue.NullValue
            else -> error("unexpected constant opcode: ${insn.opcode}")
        }
        return AtTargetCandidate(
            owner = "",
            name = "",
            descriptor = "",
            ordinal = 0,
            kind = AtTargetKind.CONSTANT,
            constantValue = constant,
            instructionOccurrenceIndex = instructionIndex,
            occurrenceResultClassification = OccurrenceResultClassification.NOT_APPLICABLE,
        )
    }

    private fun conditionCandidate(insn: JumpInsnNode, instructionIndex: Int): AtTargetCandidate =
        AtTargetCandidate(
            owner = "",
            name = "",
            descriptor = "",
            ordinal = 0,
            kind = AtTargetKind.CONSTANT,
            constantValue = null,
            instructionOccurrenceIndex = instructionIndex,
            occurrenceResultClassification = OccurrenceResultClassification.NOT_APPLICABLE,
            conditionOpcode = insn.opcode,
        )

    private fun isComparisonPreceded(insn: AbstractInsnNode): Boolean {
        var previous = insn.previous
        while (previous != null && previous.opcode < 0) {
            previous = previous.previous
        }
        return when (previous?.opcode) {
            Opcodes.LCMP,
            Opcodes.FCMPL,
            Opcodes.FCMPG,
            Opcodes.DCMPL,
            Opcodes.DCMPG,
            -> true
            else -> false
        }
    }

    internal fun classifyMethodInvokeResult(
        returnType: Type,
        insn: AbstractInsnNode,
    ): OccurrenceResultClassification {
        if (returnType.sort == Type.VOID) {
            return OccurrenceResultClassification.VOID
        }
        val next = insn.next
        next ?: return OccurrenceResultClassification.RETAINED
        return when (returnType.sort) {
            Type.LONG,
            Type.DOUBLE,
            -> if (next.opcode == Opcodes.POP2) {
                OccurrenceResultClassification.IMMEDIATELY_POPPED
            } else {
                OccurrenceResultClassification.RETAINED
            }
            else -> if (next.opcode == Opcodes.POP) {
                OccurrenceResultClassification.IMMEDIATELY_POPPED
            } else {
                OccurrenceResultClassification.RETAINED
            }
        }
    }

    private fun toConstantValue(value: Any?): ConstantValue? =
        when (value) {
            is String -> ConstantValue.StringValue(value)
            is Int -> ConstantValue.IntValue(value)
            is Long -> ConstantValue.LongValue(value)
            is Float -> ConstantValue.FloatValue(value)
            is Double -> ConstantValue.DoubleValue(value)
            is Type -> ConstantValue.ClassLiteral(value.internalName)
            null -> ConstantValue.NullValue
            else -> null
        }
}
