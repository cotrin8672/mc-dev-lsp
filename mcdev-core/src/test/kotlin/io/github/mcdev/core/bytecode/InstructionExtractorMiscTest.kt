package io.github.mcdev.core.bytecode

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class InstructionExtractorMiscTest {
    @Test
    fun extractsNewInstructions() {
        val bytes = BytecodeFixtureCompiler.classBytes("NewSamples")
        val candidates = InstructionExtractor.extract(bytes, "createObjects", "()V")
            .filter { it.kind == AtTargetKind.NEW }
        assertEquals(2, candidates.size)
        assertTrue(candidates.any { it.owner == "java/lang/StringBuilder" })
        assertTrue(candidates.any { it.owner == "java/util/ArrayList" })
    }

    @Test
    fun newInstructionDescriptorUsesClassLiteralForm() {
        val bytes = BytecodeFixtureCompiler.classBytes("NewSamples")
        val candidate = InstructionExtractor.extract(bytes, "createObjects", "()V")
            .first { it.owner == "java/lang/StringBuilder" }
        assertEquals("Ljava/lang/StringBuilder;", candidate.descriptor)
    }

    @Test
    fun extractsStringConstant() {
        val candidates = extractConstants("constants")
        val string = candidates.first { it.constantValue is ConstantValue.StringValue }
        assertEquals("hello", (string.constantValue as ConstantValue.StringValue).value)
    }

    @Test
    fun extractsIntConstant() {
        val candidates = extractConstants("constants")
        val intValue = candidates.first { it.constantValue is ConstantValue.IntValue && (it.constantValue as ConstantValue.IntValue).value == 42 }
        assertEquals(42, (intValue.constantValue as ConstantValue.IntValue).value)
    }

    @Test
    fun extractsLongConstant() {
        val candidates = extractConstants("constants")
        val longValue = candidates.first { it.constantValue is ConstantValue.LongValue }
        assertEquals(100L, (longValue.constantValue as ConstantValue.LongValue).value)
    }

    @Test
    fun extractsFloatConstant() {
        val candidates = extractConstants("constants")
        val floatValue = candidates.first { it.constantValue is ConstantValue.FloatValue }
        assertEquals(1.5f, (floatValue.constantValue as ConstantValue.FloatValue).value)
    }

    @Test
    fun extractsDoubleConstant() {
        val candidates = extractConstants("constants")
        val doubleValue = candidates.first { it.constantValue is ConstantValue.DoubleValue }
        assertEquals(2.5, (doubleValue.constantValue as ConstantValue.DoubleValue).value)
    }

    @Test
    fun extractsClassLiteralConstant() {
        val candidates = extractConstants("constants")
        val classLiteral = candidates.first { it.constantValue is ConstantValue.ClassLiteral }
        assertEquals("java/lang/String", (classLiteral.constantValue as ConstantValue.ClassLiteral).internalName)
    }

    @Test
    fun extractsNullConstant() {
        val candidates = extractConstants("constants")
        assertTrue(candidates.any { it.constantValue == ConstantValue.NullValue })
    }

    @Test
    fun extractsSmallIntConstants() {
        val bytes = BytecodeFixtureCompiler.classBytes("ConstantSamples")
        val candidates = InstructionExtractor.extract(bytes, "smallInts", "()V")
            .filter { it.kind == AtTargetKind.CONSTANT && it.constantValue is ConstantValue.IntValue }
        assertTrue(candidates.any { (it.constantValue as ConstantValue.IntValue).value == 5 })
        assertTrue(candidates.any { (it.constantValue as ConstantValue.IntValue).value == -1 })
    }

    @Test
    fun extractsReturnInstruction() {
        val bytes = BytecodeFixtureCompiler.classBytes("ReturnSamples")
        val candidates = InstructionExtractor.extract(bytes, "returnVoid", "()V")
            .filter { it.kind == AtTargetKind.RETURN }
        assertEquals(1, candidates.size)
        assertEquals("RETURN", candidates.first().name)
    }

    @Test
    fun returnOrdinalsAreDistinctPerReturnSite() {
        val bytes = BytecodeFixtureCompiler.classBytes("ReturnSamples")
        val candidates = InstructionExtractor.extract(bytes, "multipleReturns", "(I)I")
            .filter { it.kind == AtTargetKind.RETURN }
        assertEquals(2, candidates.size)
        assertEquals(listOf(0, 1), candidates.map { it.ordinal })
    }

    @Test
    fun intReturnMethodHasReturnCandidate() {
        val bytes = BytecodeFixtureCompiler.classBytes("ReturnSamples")
        val candidates = InstructionExtractor.extract(bytes, "returnInt", "()I")
        assertTrue(candidates.any { it.kind == AtTargetKind.RETURN })
    }

    @Test
    fun objectReturnMethodHasReturnCandidate() {
        val bytes = BytecodeFixtureCompiler.classBytes("ReturnSamples")
        val candidates = InstructionExtractor.extract(bytes, "returnObject", "()Ljava/lang/String;")
        assertTrue(candidates.any { it.kind == AtTargetKind.RETURN })
    }

    private fun extractConstants(methodName: String): List<AtTargetCandidate> {
        val bytes = BytecodeFixtureCompiler.classBytes("ConstantSamples")
        return InstructionExtractor.extract(bytes, methodName, "()V")
            .filter { it.kind == AtTargetKind.CONSTANT }
    }
}
