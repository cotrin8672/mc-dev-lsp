package io.github.mcdev.core.bytecode

import kotlin.test.Test
import kotlin.test.assertEquals

class OrdinalCalculatorTest {
    @Test
    fun assignsZeroToFirstCandidate() {
        val result = OrdinalCalculator.assignOrdinals(
            listOf(invokeCandidate("abs", "(I)I")),
        )
        assertEquals(0, result.first().ordinal)
    }

    @Test
    fun incrementsOrdinalsForDuplicateInvokes() {
        val input = listOf(
            invokeCandidate("abs", "(I)I"),
            invokeCandidate("abs", "(I)I"),
            invokeCandidate("abs", "(I)I"),
        )
        val result = OrdinalCalculator.assignOrdinals(input)
        assertEquals(listOf(0, 1, 2), result.map { it.ordinal })
    }

    @Test
    fun separatesOrdinalsByMethodName() {
        val input = listOf(
            invokeCandidate("abs", "(I)I"),
            invokeCandidate("max", "(II)I"),
        )
        val result = OrdinalCalculator.assignOrdinals(input)
        assertEquals(listOf(0, 0), result.map { it.ordinal })
    }

    @Test
    fun separatesOrdinalsByDescriptor() {
        val input = listOf(
            invokeCandidate("valueOf", "(I)Ljava/lang/Integer;"),
            invokeCandidate("valueOf", "(Ljava/lang/String;)Ljava/lang/Integer;"),
        )
        val result = OrdinalCalculator.assignOrdinals(input)
        assertEquals(listOf(0, 0), result.map { it.ordinal })
    }

    @Test
    fun separatesOrdinalsByOwner() {
        val input = listOf(
            invokeCandidate("abs", "(I)I", owner = "java/lang/Math"),
            invokeCandidate("abs", "(I)I", owner = "other/Owner"),
        )
        val result = OrdinalCalculator.assignOrdinals(input)
        assertEquals(listOf(0, 0), result.map { it.ordinal })
    }

    @Test
    fun separatesOrdinalsByKind() {
        val input = listOf(
            invokeCandidate("run", "()V", kind = AtTargetKind.INVOKE_VIRTUAL),
            invokeCandidate("run", "()V", kind = AtTargetKind.INVOKE_INTERFACE),
        )
        val result = OrdinalCalculator.assignOrdinals(input)
        assertEquals(listOf(0, 0), result.map { it.ordinal })
    }

    @Test
    fun fieldOrdinalsIncrementForSameTarget() {
        val input = listOf(
            fieldCandidate("STATIC_FIELD", AtTargetKind.FIELD_GET_STATIC),
            fieldCandidate("STATIC_FIELD", AtTargetKind.FIELD_GET_STATIC),
        )
        val result = OrdinalCalculator.assignOrdinals(input)
        assertEquals(listOf(0, 1), result.map { it.ordinal })
    }

    @Test
    fun constantOrdinalsDifferByValue() {
        val input = listOf(
            constantCandidate(ConstantValue.IntValue(1)),
            constantCandidate(ConstantValue.IntValue(2)),
            constantCandidate(ConstantValue.IntValue(1)),
        )
        val result = OrdinalCalculator.assignOrdinals(input)
        assertEquals(listOf(0, 0, 1), result.map { it.ordinal })
    }

    @Test
    fun returnOrdinalsIncrementPerReturnSite() {
        val input = listOf(
            returnCandidate(),
            returnCandidate(),
        )
        val result = OrdinalCalculator.assignOrdinals(input)
        assertEquals(listOf(0, 1), result.map { it.ordinal })
    }

    private fun invokeCandidate(
        name: String,
        descriptor: String,
        owner: String = "java/lang/Math",
        kind: AtTargetKind = AtTargetKind.INVOKE_STATIC,
    ) = AtTargetCandidate(owner, name, descriptor, 0, kind)

    private fun fieldCandidate(name: String, kind: AtTargetKind) =
        AtTargetCandidate("owner/Class", name, "I", 0, kind)

    private fun constantCandidate(value: ConstantValue) =
        AtTargetCandidate("", "", "", 0, AtTargetKind.CONSTANT, value)

    private fun returnCandidate() =
        AtTargetCandidate("", "RETURN", "", 0, AtTargetKind.RETURN)
}
