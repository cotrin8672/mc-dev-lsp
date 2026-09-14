package io.github.mcdev.core.index

import io.github.mcdev.core.bytecode.AtTargetCandidate as BytecodeAtTargetCandidate
import io.github.mcdev.core.bytecode.AtTargetKind as BytecodeAtTargetKind
import io.github.mcdev.core.bytecode.OccurrenceResultClassification
import io.github.mcdev.core.mixin.AtTargetOperationKind
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.objectweb.asm.Opcodes

class BytecodeIndexEntryMapperTest {
    @Test
    fun toMixinAtTargetPropagatesInvokeOccurrenceMetadata() {
        val candidate = BytecodeAtTargetCandidate(
            owner = "java/lang/String",
            name = "length",
            descriptor = "()I",
            ordinal = 0,
            kind = BytecodeAtTargetKind.INVOKE_VIRTUAL,
            instructionOccurrenceIndex = 2,
            occurrenceResultClassification = OccurrenceResultClassification.RETAINED,
        )

        val mapped = BytecodeIndexEntryMapper.toMixinAtTarget(candidate)

        assertEquals(2, mapped.instructionOccurrenceIndex)
        assertEquals(OccurrenceResultClassification.RETAINED, mapped.occurrenceResultClassification)
    }

    @Test
    fun toMixinAtTargetPreservesConditionOpcode() {
        val candidate = BytecodeAtTargetCandidate(
            owner = "",
            name = "",
            descriptor = "",
            ordinal = 0,
            kind = BytecodeAtTargetKind.CONSTANT,
            constantValue = null,
            instructionOccurrenceIndex = 4,
            conditionOpcode = Opcodes.IFGE,
        )

        val mapped = BytecodeIndexEntryMapper.toMixinAtTarget(candidate)

        assertEquals(Opcodes.IFGE, mapped.conditionOpcode)
        assertNull(mapped.constantValue)
    }

    @Test
    fun toMixinAtTargetPreservesInvokeOperationKind() {
        val candidate = BytecodeAtTargetCandidate(
            owner = "java/lang/String",
            name = "length",
            descriptor = "()I",
            ordinal = 0,
            kind = BytecodeAtTargetKind.INVOKE_VIRTUAL,
        )

        val mapped = BytecodeIndexEntryMapper.toMixinAtTarget(candidate)

        assertEquals(AtTargetOperationKind.INVOKE_VIRTUAL, mapped.operationKind)
    }

    @Test
    fun toMixinAtTargetPreservesFieldReadWriteOperationKinds() {
        val getField = BytecodeAtTargetCandidate(
            owner = "com/example/Target",
            name = "value",
            descriptor = "I",
            ordinal = 0,
            kind = BytecodeAtTargetKind.FIELD_GET_INSTANCE,
        )
        val putField = getField.copy(kind = BytecodeAtTargetKind.FIELD_PUT_INSTANCE)

        assertEquals(
            AtTargetOperationKind.FIELD_GET_INSTANCE,
            BytecodeIndexEntryMapper.toMixinAtTarget(getField).operationKind,
        )
        assertEquals(
            AtTargetOperationKind.FIELD_PUT_INSTANCE,
            BytecodeIndexEntryMapper.toMixinAtTarget(putField).operationKind,
        )
    }

    @Test
    fun toMixinAtTargetMapsNonInvokeFieldKindsToNullOperationKind() {
        val candidate = BytecodeAtTargetCandidate(
            owner = "com/example/Target",
            name = "draw",
            descriptor = "()V",
            ordinal = 0,
            kind = BytecodeAtTargetKind.RETURN,
        )

        assertNull(BytecodeIndexEntryMapper.toMixinAtTarget(candidate).operationKind)
    }

    @Test
    fun toMixinAtTargetPreservesDefaultOccurrenceMetadata() {
        val candidate = BytecodeAtTargetCandidate(
            owner = "com/example/Target",
            name = "draw",
            descriptor = "()V",
            ordinal = 0,
            kind = BytecodeAtTargetKind.RETURN,
        )

        val mapped = BytecodeIndexEntryMapper.toMixinAtTarget(candidate)

        assertEquals(-1, mapped.instructionOccurrenceIndex)
        assertEquals(OccurrenceResultClassification.NOT_APPLICABLE, mapped.occurrenceResultClassification)
    }
}
