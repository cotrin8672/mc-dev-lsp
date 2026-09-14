package io.github.mcdev.core.mixinextras

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class ConstantAtArgsParserTest {
    @Test
    fun parsesIntValueDiscriminator() {
        assertEquals("I", ConstantAtArgsParser.parse(listOf("intValue=42")))
        assertEquals("I", ConstantAtArgsParser.parse(listOf("intValue=-1")))
    }

    @Test
    fun parsesFloatValueDiscriminator() {
        assertEquals("F", ConstantAtArgsParser.parse(listOf("floatValue=0.0")))
        assertEquals("F", ConstantAtArgsParser.parse(listOf("floatValue=1.5")))
        assertEquals("F", ConstantAtArgsParser.parse(listOf("floatValue=1e2")))
        assertEquals("F", ConstantAtArgsParser.parse(listOf("floatValue= -3.14 ")))
    }

    @Test
    fun parsesLongValueDiscriminator() {
        assertEquals("J", ConstantAtArgsParser.parse(listOf("longValue=42")))
        assertEquals("J", ConstantAtArgsParser.parse(listOf("longValue=-9223372036854775808")))
    }

    @Test
    fun parsesDoubleValueDiscriminator() {
        assertEquals("D", ConstantAtArgsParser.parse(listOf("doubleValue=1.0")))
        assertEquals("D", ConstantAtArgsParser.parse(listOf("doubleValue=1.5")))
        assertEquals("D", ConstantAtArgsParser.parse(listOf("doubleValue=1e-3")))
        assertEquals("D", ConstantAtArgsParser.parse(listOf("doubleValue= -2.5 ")))
    }

    @Test
    fun parsesStringValueDiscriminatorIncludingEmpty() {
        assertEquals("Ljava/lang/String;", ConstantAtArgsParser.parse(listOf("stringValue=hello")))
        assertEquals("Ljava/lang/String;", ConstantAtArgsParser.parse(listOf("stringValue=")))
    }

    @Test
    fun parsesClassValueDiscriminator() {
        assertEquals("Ljava/lang/Class;", ConstantAtArgsParser.parse(listOf("classValue=java.lang.String")))
    }

    @Test
    fun parsesExplicitNullValueDiscriminator() {
        assertEquals("Ljava/lang/Object;", ConstantAtArgsParser.parse(listOf("nullValue=true")))
        assertEquals("Ljava/lang/Object;", ConstantAtArgsParser.parse(listOf("nullValue=TRUE")))
    }

    @Test
    fun ignoresUnknownNonDiscriminatorKeys() {
        assertEquals(
            "I",
            ConstantAtArgsParser.parse(listOf("intValue=1", "expandZeroConditions=true")),
        )
    }

    @Test
    fun ignoresBareUnknownKeysWithoutEquals() {
        assertEquals(
            "I",
            ConstantAtArgsParser.parse(listOf("intValue=1", "expandZeroConditions")),
        )
        assertNull(ConstantAtArgsParser.parse(listOf("expandZeroConditions")))
    }

    @Test
    fun duplicateKeysKeepLastValue() {
        assertEquals(
            "I",
            ConstantAtArgsParser.parse(listOf("intValue=1", "intValue=2")),
        )
        assertEquals(
            "I",
            ConstantAtArgsParser.parse(listOf("intValue=abc", "intValue=1")),
        )
        assertNull(ConstantAtArgsParser.parse(listOf("intValue=1", "intValue=abc")))
    }

    @Test
    fun allowsRepeatedSameDiscriminator() {
        assertEquals(
            "I",
            ConstantAtArgsParser.parse(listOf("intValue=1", "intValue=2")),
        )
    }

    @Test
    fun rejectsMalformedNumericValues() {
        assertNull(ConstantAtArgsParser.parse(listOf("intValue=abc")))
        assertNull(ConstantAtArgsParser.parse(listOf("floatValue=not-a-float")))
        assertNull(ConstantAtArgsParser.parse(listOf("longValue=1.5")))
        assertNull(ConstantAtArgsParser.parse(listOf("doubleValue=abc")))
    }

    @Test
    fun rejectsIntAndLongWhitespaceAndSuffixForms() {
        assertNull(ConstantAtArgsParser.parse(listOf("intValue= 42 ")))
        assertNull(ConstantAtArgsParser.parse(listOf("intValue=1f")))
        assertNull(ConstantAtArgsParser.parse(listOf("intValue=0x10")))
        assertNull(ConstantAtArgsParser.parse(listOf("intValue=1_000")))

        assertNull(ConstantAtArgsParser.parse(listOf("longValue= 42 ")))
        assertNull(ConstantAtArgsParser.parse(listOf("longValue=42L")))
        assertNull(ConstantAtArgsParser.parse(listOf("longValue=0x2A")))
        assertNull(ConstantAtArgsParser.parse(listOf("longValue=1_000")))
    }

    @Test
    fun acceptsFloatAndDoubleFormsSupportedByRawKotlinParsing() {
        assertEquals("F", ConstantAtArgsParser.parse(listOf("floatValue=1.5f")))
        assertEquals("F", ConstantAtArgsParser.parse(listOf("floatValue=0x1.0p0")))
        assertEquals("F", ConstantAtArgsParser.parse(listOf("floatValue=1.5d")))

        assertEquals("D", ConstantAtArgsParser.parse(listOf("doubleValue=1.0d")))
        assertEquals("D", ConstantAtArgsParser.parse(listOf("doubleValue=0x1.0p0")))
        assertEquals("D", ConstantAtArgsParser.parse(listOf("doubleValue=1.0f")))
    }

    @Test
    fun rejectsNullValueFalseWithoutAnotherDiscriminator() {
        assertNull(ConstantAtArgsParser.parse(listOf("nullValue=false")))
    }

    @Test
    fun allowsNullValueFalseWhenAnotherDiscriminatorIsValid() {
        assertEquals(
            "I",
            ConstantAtArgsParser.parse(listOf("nullValue=false", "intValue=1")),
        )
    }

    @Test
    fun rejectsConflictingDistinctDiscriminators() {
        assertNull(ConstantAtArgsParser.parse(listOf("intValue=1", "floatValue=1.0")))
        assertNull(ConstantAtArgsParser.parse(listOf("nullValue=true", "stringValue=hello")))
    }

    @Test
    fun rejectsMissingDiscriminator() {
        assertNull(ConstantAtArgsParser.parse(emptyList()))
        assertNull(ConstantAtArgsParser.parse(listOf("expandZeroConditions=true")))
    }

    @Test
    fun parsesClassValueVoidDiscriminator() {
        assertEquals("Ljava/lang/Class;", ConstantAtArgsParser.parse(listOf("classValue=void")))
    }

    @Test
    fun rejectsBlankClassValue() {
        assertNull(ConstantAtArgsParser.parse(listOf("classValue=")))
        assertNull(ConstantAtArgsParser.parse(listOf("classValue=   ")))
    }

    @Test
    fun rejectsEntryWithoutEqualsWhenNoValidDiscriminatorRemains() {
        assertNull(ConstantAtArgsParser.parse(listOf("intValue")))
    }

    @Test
    fun treatsInvalidNullValueLiteralAsFalseWithoutDiscriminator() {
        assertNull(ConstantAtArgsParser.parse(listOf("nullValue=maybe")))
    }
}
