package io.github.mcdev.core.bytecode

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class InstructionExtractorFieldTest {
    private val fieldBytes = BytecodeFixtureCompiler.classBytes("FieldSamples")
    private val owner = BytecodeFixtureCompiler.internalName("FieldSamples")

    @Test
    fun extractsStaticFieldGet() {
        val candidates = extract("accessFields")
        assertTrue(
            candidates.any {
                it.kind == AtTargetKind.FIELD_GET_STATIC &&
                    it.owner == owner &&
                    it.name == "STATIC_FIELD" &&
                    it.descriptor == "I"
            },
        )
    }

    @Test
    fun extractsStaticFieldPut() {
        val candidates = extract("accessFields")
        assertTrue(
            candidates.any {
                it.kind == AtTargetKind.FIELD_PUT_STATIC &&
                    it.owner == owner &&
                    it.name == "STATIC_FIELD"
            },
        )
    }

    @Test
    fun extractsInstanceFieldGet() {
        val candidates = extract("accessFields")
        assertTrue(
            candidates.any {
                it.kind == AtTargetKind.FIELD_GET_INSTANCE &&
                    it.owner == owner &&
                    it.name == "instanceField" &&
                    it.descriptor == "I"
            },
        )
    }

    @Test
    fun extractsInstanceFieldPut() {
        val candidates = extract("accessFields")
        assertTrue(
            candidates.any {
                it.kind == AtTargetKind.FIELD_PUT_INSTANCE &&
                    it.owner == owner &&
                    it.name == "instanceField"
            },
        )
    }

    @Test
    fun fieldAccessesHaveFourKindsInSample() {
        val kinds = extract("accessFields")
            .map { it.kind }
            .filter { it.name.startsWith("FIELD_") }
            .toSet()
        assertEquals(
            setOf(
                AtTargetKind.FIELD_GET_STATIC,
                AtTargetKind.FIELD_PUT_STATIC,
                AtTargetKind.FIELD_GET_INSTANCE,
                AtTargetKind.FIELD_PUT_INSTANCE,
            ),
            kinds,
        )
    }

    @Test
    fun duplicateFieldGetsReceiveDistinctOrdinals() {
        val candidates = extract("duplicateFieldGets")
            .filter { it.kind == AtTargetKind.FIELD_GET_STATIC && it.name == "STATIC_FIELD" }
        assertEquals(2, candidates.size)
        assertEquals(listOf(0, 1), candidates.map { it.ordinal })
    }

    @Test
    fun duplicateFieldGetsShareDescriptor() {
        val candidates = extract("duplicateFieldGets")
            .filter { it.kind == AtTargetKind.FIELD_GET_STATIC }
        assertTrue(candidates.all { it.descriptor == "I" })
    }

    private fun extract(methodName: String): List<AtTargetCandidate> =
        InstructionExtractor.extract(fieldBytes, methodName, "()V")
}
