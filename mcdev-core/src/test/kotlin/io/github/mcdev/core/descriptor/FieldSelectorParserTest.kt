package io.github.mcdev.core.descriptor

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs

class FieldSelectorParserTest {
    @Test
    fun parsesInternalOwnerFieldWithDescriptor() {
        val parsed = assertSuccess("Lcom/example/Foo;count:I")
        assertEquals(Pattern.Exact("com/example/Foo"), parsed.owner)
        assertEquals(Pattern.Exact("count"), parsed.name)
        assertEquals(Pattern.Exact(JvmType.IntType), parsed.descriptor)
    }

    @Test
    fun parsesLastDotQualifiedOwnerWithSlashNormalization() {
        val parsed = assertSuccess("com.example.Foo.value:Ljava/lang/String;")
        assertEquals(Pattern.Exact("com/example/Foo"), parsed.owner)
        assertEquals(Pattern.Exact("value"), parsed.name)
        assertEquals(
            Pattern.Exact(JvmType.ObjectType("java/lang/String")),
            parsed.descriptor,
        )
    }

    @Test
    fun parsesSlashQualifiedOwnerWithoutNormalizingSlashes() {
        val parsed = assertSuccess("com/example/Foo.value:I")
        assertEquals(Pattern.Exact("com/example/Foo"), parsed.owner)
        assertEquals(Pattern.Exact("value"), parsed.name)
        assertEquals(Pattern.Exact(JvmType.IntType), parsed.descriptor)
    }

    @Test
    fun parsesOmittedOwnerAndDescriptor() {
        val parsed = assertSuccess("count")
        assertEquals(Pattern.Any, parsed.owner)
        assertEquals(Pattern.Exact("count"), parsed.name)
        assertEquals(Pattern.Any, parsed.descriptor)
    }

    @Test
    fun parsesBareFieldNameStartingWithCapitalL() {
        val parsed = assertSuccess("Length")
        assertEquals(Pattern.Any, parsed.owner)
        assertEquals(Pattern.Exact("Length"), parsed.name)
        assertEquals(Pattern.Any, parsed.descriptor)
    }

    @Test
    fun parsesBareFieldNameStartingWithCapitalLWithDescriptor() {
        val parsed = assertSuccess("Length:I")
        assertEquals(Pattern.Any, parsed.owner)
        assertEquals(Pattern.Exact("Length"), parsed.name)
        assertEquals(Pattern.Exact(JvmType.IntType), parsed.descriptor)
    }

    @Test
    fun parsesQualifiedFieldNameStartingWithCapitalL() {
        val parsed = assertSuccess("Logger.value:I")
        assertEquals(Pattern.Exact("Logger"), parsed.owner)
        assertEquals(Pattern.Exact("value"), parsed.name)
        assertEquals(Pattern.Exact(JvmType.IntType), parsed.descriptor)
    }

    @Test
    fun parsesOwnerOnlySelector() {
        val parsed = assertSuccess("Lcom/example/Foo;")
        assertEquals(Pattern.Exact("com/example/Foo"), parsed.owner)
        assertEquals(Pattern.Any, parsed.name)
        assertEquals(Pattern.Any, parsed.descriptor)
    }

    @Test
    fun parsesExactStarNameAsAnyNamePattern() {
        val parsed = assertSuccess("Lcom/example/Foo;*")
        assertEquals(Pattern.Exact("com/example/Foo"), parsed.owner)
        assertEquals(Pattern.Any, parsed.name)
        assertEquals(Pattern.Any, parsed.descriptor)
    }

    @Test
    fun parsesTerminalNameStarAsExactPrefixWithAnyDescriptor() {
        val parsed = assertSuccess("count*")
        assertEquals(Pattern.Any, parsed.owner)
        assertEquals(Pattern.Exact("count"), parsed.name)
        assertEquals(Pattern.Any, parsed.descriptor)
    }

    @Test
    fun parsesStarNameWithExactDescriptor() {
        val parsed = assertSuccess("*:I")
        assertEquals(Pattern.Any, parsed.owner)
        assertEquals(Pattern.Any, parsed.name)
        assertEquals(Pattern.Exact(JvmType.IntType), parsed.descriptor)
    }

    @Test
    fun parsesGlobalAnySelector() {
        val parsed = assertSuccess("*")
        assertEquals(Pattern.Any, parsed.owner)
        assertEquals(Pattern.Any, parsed.name)
        assertEquals(Pattern.Any, parsed.descriptor)
    }

    @Test
    fun parsesNestedDollarOwnerSegments() {
        val parsed = assertSuccess("Lcom/example/Outer\$Inner\$1;value:I")
        assertEquals(Pattern.Exact("com/example/Outer\$Inner\$1"), parsed.owner)
        assertEquals(Pattern.Exact("value"), parsed.name)
        assertEquals(Pattern.Exact(JvmType.IntType), parsed.descriptor)
    }

    @Test
    fun parsesFieldDescriptorWithArrayTypes() {
        val parsed = assertSuccess("Lcom/example/Foo;data:[I")
        assertEquals(
            Pattern.Exact(JvmType.ArrayType(JvmType.IntType)),
            parsed.descriptor,
        )
    }

    @Test
    fun parsesNestedArrayFieldDescriptor() {
        val parsed = assertSuccess("Lcom/example/Foo;matrix:[[Ljava/lang/String;")
        assertEquals(
            Pattern.Exact(
                JvmType.ArrayType(
                    JvmType.ArrayType(JvmType.ObjectType("java/lang/String")),
                ),
            ),
            parsed.descriptor,
        )
    }

    @Test
    fun rejectsFieldNameStartingWithDigit() {
        assertFailure("1count")
        assertFailure("1count:I")
    }

    @Test
    fun rejectsMixedOwnerSeparators() {
        assertFailure("com.example/Foo.value:I")
    }

    @Test
    fun rejectsEmptyInput() {
        assertFailure("")
    }

    @Test
    fun normalizesAsciiSpacesBeforeParsing() {
        assertSameAsNormalized("Lcom/example/Foo;count:I")
        assertSameAsNormalized(" com.example.Foo.value:Ljava/lang/String; ")
        assertSameAsNormalized("com . example . Foo . value : Ljava/lang/String;")
        assertSameAsNormalized(" count ")
        assertSameAsNormalized(" * ")
        assertSameAsNormalized(" count* ")
        assertSameAsNormalized(" * : I ")
    }

    @Test
    fun rejectsWhitespaceOnlyInputAfterAsciiSpaceNormalization() {
        assertFailure("   ")
    }

    @Test
    fun rejectsNonAsciiWhitespaceWithoutNormalizing() {
        assertFailure("\tcount")
        assertFailure("count\n")
        assertFailure("count\u00A0")
        assertFailure("\t*")
    }

    @Test
    fun rejectsMethodParenthesisSyntax() {
        assertFailure("Lcom/example/Foo;run()V")
        assertFailure("run()V")
    }

    @Test
    fun rejectsInitAndClinitFieldNames() {
        assertFailure("Lcom/example/Foo;<init>:I")
        assertFailure("Lcom/example/Foo;<clinit>:I")
        assertFailure("<init>:I")
        assertFailure("<clinit>:I")
    }

    @Test
    fun rejectsDottedOwnerInsideInternalForm() {
        assertFailure("Lcom.example.Foo;count:I")
    }

    @Test
    fun rejectsTrailingJunkAfterDescriptor() {
        assertFailure("Lcom/example/Foo;count:Iextra")
    }

    @Test
    fun rejectsInvalidFieldDescriptor() {
        assertFailure("Lcom/example/Foo;count:Ljava/lang/String")
    }

    @Test
    fun rejectsVoidFieldType() {
        assertFailure("Lcom/example/Foo;count:V")
    }

    @Test
    fun rejectsDottedObjectDescriptor() {
        assertFailure("Lcom/example/Foo;value:Ljava.lang.String;")
    }

    @Test
    fun rejectsFieldDescriptorWithBracketInInternalName() {
        assertFailure("value:Lfoo[Bar;")
    }

    @Test
    fun rejectsMalformedWildcards() {
        assertFailure("count**")
        assertFailure("*count")
        assertFailure("Lcom/example/Foo;**")
    }

    @Test
    fun rejectsTerminalNameStarWithExplicitDescriptor() {
        assertFailure("count*:I")
        assertFailure("Lcom/example/Foo;count*:I")
    }

    @Test
    fun acceptsExactStarNameWithExplicitDescriptor() {
        val parsed = assertSuccess("*:I")
        assertEquals(Pattern.Any, parsed.name)
        assertEquals(Pattern.Exact(JvmType.IntType), parsed.descriptor)
    }

    @Test
    fun rejectsMissingOwnerSemicolonForm() {
        assertFailure("Lcom/example/Foo")
    }

    @Test
    fun rejectsEmptyOwnerSegment() {
        assertFailure("Lcom//example/Foo;count:I")
        assertFailure("com..example.Foo.count:I")
    }

    @Test
    fun rejectsWildcardInOwner() {
        assertFailure("Lcom/example/*;count:I")
        assertFailure("com.example.*.count:I")
    }

    @Test
    fun rejectsMissingFieldName() {
        assertFailure(":I")
        assertFailure("Lcom/example/Foo;:I")
    }

    @Test
    fun rejectsMissingFieldDescriptor() {
        assertFailure("Lcom/example/Foo;count:")
        assertFailure("count:")
    }

    private fun assertSameAsNormalized(input: String) {
        val normalized = input.replace(" ", "")
        val expected = assertSuccess(normalized)
        val actual = assertSuccess(input)
        assertEquals(expected, actual)
    }

    private fun assertSuccess(input: String): FieldSelector {
        val parsed = assertIs<DescriptorParseResult.Success<FieldSelector>>(parseFieldSelector(input))
        return parsed.value
    }

    private fun assertFailure(input: String) {
        assertIs<DescriptorParseResult.Failure>(parseFieldSelector(input))
    }
}
