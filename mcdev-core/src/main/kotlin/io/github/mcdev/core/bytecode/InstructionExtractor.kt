package io.github.mcdev.core.bytecode

import org.objectweb.asm.ClassReader
import org.objectweb.asm.MethodVisitor
import org.objectweb.asm.Opcodes
import org.objectweb.asm.Type

object InstructionExtractor {
    fun extract(
        classBytes: ByteArray,
        methodName: String,
        methodDescriptor: String,
    ): List<AtTargetCandidate> {
        val reader = ClassReader(classBytes)
        val collector = InstructionCollector(methodName, methodDescriptor)
        reader.accept(collector, ClassReader.SKIP_DEBUG or ClassReader.SKIP_FRAMES)
        return OrdinalCalculator.assignOrdinals(collector.candidates)
    }

    private class InstructionCollector(
        private val targetMethodName: String,
        private val targetMethodDescriptor: String,
    ) : org.objectweb.asm.ClassVisitor(Opcodes.ASM9) {
        val candidates = mutableListOf<AtTargetCandidate>()

        override fun visitMethod(
            access: Int,
            name: String,
            descriptor: String,
            signature: String?,
            exceptions: Array<out String>?,
        ): MethodVisitor? {
            if (name != targetMethodName || descriptor != targetMethodDescriptor) {
                return null
            }
            return object : MethodVisitor(Opcodes.ASM9) {
                override fun visitMethodInsn(
                    opcode: Int,
                    owner: String,
                    name: String,
                    descriptor: String,
                    isInterface: Boolean,
                ) {
                    val kind = when (opcode) {
                        Opcodes.INVOKEVIRTUAL -> AtTargetKind.INVOKE_VIRTUAL
                        Opcodes.INVOKESPECIAL -> AtTargetKind.INVOKE_SPECIAL
                        Opcodes.INVOKESTATIC -> AtTargetKind.INVOKE_STATIC
                        Opcodes.INVOKEINTERFACE -> AtTargetKind.INVOKE_INTERFACE
                        else -> return
                    }
                    candidates += AtTargetCandidate(
                        owner = owner,
                        name = name,
                        descriptor = descriptor,
                        ordinal = 0,
                        kind = kind,
                    )
                }

                override fun visitFieldInsn(opcode: Int, owner: String, name: String, descriptor: String) {
                    val kind = when (opcode) {
                        Opcodes.GETFIELD -> AtTargetKind.FIELD_GET_INSTANCE
                        Opcodes.PUTFIELD -> AtTargetKind.FIELD_PUT_INSTANCE
                        Opcodes.GETSTATIC -> AtTargetKind.FIELD_GET_STATIC
                        Opcodes.PUTSTATIC -> AtTargetKind.FIELD_PUT_STATIC
                        else -> return
                    }
                    candidates += AtTargetCandidate(
                        owner = owner,
                        name = name,
                        descriptor = descriptor,
                        ordinal = 0,
                        kind = kind,
                    )
                }

                override fun visitTypeInsn(opcode: Int, type: String) {
                    if (opcode != Opcodes.NEW) return
                    candidates += AtTargetCandidate(
                        owner = type,
                        name = "",
                        descriptor = "L$type;",
                        ordinal = 0,
                        kind = AtTargetKind.NEW,
                    )
                }

                override fun visitLdcInsn(value: Any?) {
                    val constant = toConstantValue(value) ?: return
                    candidates += AtTargetCandidate(
                        owner = "",
                        name = "",
                        descriptor = "",
                        ordinal = 0,
                        kind = AtTargetKind.CONSTANT,
                        constantValue = constant,
                    )
                }

                override fun visitIntInsn(opcode: Int, operand: Int) {
                    if (opcode == Opcodes.BIPUSH || opcode == Opcodes.SIPUSH) {
                        addIntConstant(operand)
                    }
                }

                override fun visitInsn(opcode: Int) {
                    when (opcode) {
                        Opcodes.ICONST_M1 -> addIntConstant(-1)
                        Opcodes.ICONST_0 -> addIntConstant(0)
                        Opcodes.ICONST_1 -> addIntConstant(1)
                        Opcodes.ICONST_2 -> addIntConstant(2)
                        Opcodes.ICONST_3 -> addIntConstant(3)
                        Opcodes.ICONST_4 -> addIntConstant(4)
                        Opcodes.ICONST_5 -> addIntConstant(5)
                        Opcodes.LCONST_0 -> addLongConstant(0L)
                        Opcodes.LCONST_1 -> addLongConstant(1L)
                        Opcodes.FCONST_0 -> addFloatConstant(0f)
                        Opcodes.FCONST_1 -> addFloatConstant(1f)
                        Opcodes.FCONST_2 -> addFloatConstant(2f)
                        Opcodes.DCONST_0 -> addDoubleConstant(0.0)
                        Opcodes.DCONST_1 -> addDoubleConstant(1.0)
                        Opcodes.ACONST_NULL -> addNullConstant()
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
                        )
                    }
                }

                private fun addIntConstant(value: Int) {
                    candidates += AtTargetCandidate(
                        owner = "",
                        name = "",
                        descriptor = "",
                        ordinal = 0,
                        kind = AtTargetKind.CONSTANT,
                        constantValue = ConstantValue.IntValue(value),
                    )
                }

                private fun addLongConstant(value: Long) {
                    candidates += AtTargetCandidate(
                        owner = "",
                        name = "",
                        descriptor = "",
                        ordinal = 0,
                        kind = AtTargetKind.CONSTANT,
                        constantValue = ConstantValue.LongValue(value),
                    )
                }

                private fun addFloatConstant(value: Float) {
                    candidates += AtTargetCandidate(
                        owner = "",
                        name = "",
                        descriptor = "",
                        ordinal = 0,
                        kind = AtTargetKind.CONSTANT,
                        constantValue = ConstantValue.FloatValue(value),
                    )
                }

                private fun addDoubleConstant(value: Double) {
                    candidates += AtTargetCandidate(
                        owner = "",
                        name = "",
                        descriptor = "",
                        ordinal = 0,
                        kind = AtTargetKind.CONSTANT,
                        constantValue = ConstantValue.DoubleValue(value),
                    )
                }

                private fun addNullConstant() {
                    candidates += AtTargetCandidate(
                        owner = "",
                        name = "",
                        descriptor = "",
                        ordinal = 0,
                        kind = AtTargetKind.CONSTANT,
                        constantValue = ConstantValue.NullValue,
                    )
                }
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
