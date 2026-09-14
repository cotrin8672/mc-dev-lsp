package io.github.mcdev.core.descriptor

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs

class MethodSelectorParserTest {
    @Test
    fun parsesInternalOwnerMethodWithDescriptor() {
        val parsed = assertSuccess("Lcom/example/Foo;run()V")
        assertEquals(Pattern.Exact("com/example/Foo"), parsed.owner)
        assertEquals(Pattern.Exact("run"), parsed.name)
        assertEquals(
            Pattern.Exact(MethodDescriptor(emptyList(), JvmType.VoidType)),
            parsed.descriptor,
        )
    }

    @Test
    fun parsesLastDotQualifiedOwnerWithSlashNormalization() {
        val parsed = assertSuccess("com.example.Foo.run()V")
        assertEquals(Pattern.Exact("com/example/Foo"), parsed.owner)
        assertEquals(Pattern.Exact("run"), parsed.name)
        assertEquals(Pattern.Exact(MethodDescriptor(emptyList(), JvmType.VoidType)), parsed.descriptor)
    }

    @Test
    fun parsesSlashQualifiedOwnerWithoutNormalizingSlashes() {
        val parsed = assertSuccess("com/example/Foo.run()V")
        assertEquals(Pattern.Exact("com/example/Foo"), parsed.owner)
        assertEquals(Pattern.Exact("run"), parsed.name)
    }

    @Test
    fun parsesOmittedOwnerAndDescriptor() {
        val parsed = assertSuccess("run")
        assertEquals(Pattern.Any, parsed.owner)
        assertEquals(Pattern.Exact("run"), parsed.name)
        assertEquals(Pattern.Any, parsed.descriptor)
    }

    @Test
    fun parsesBareNameStartingWithCapitalL() {
        val parsed = assertSuccess("Load")
        assertEquals(Pattern.Any, parsed.owner)
        assertEquals(Pattern.Exact("Load"), parsed.name)
        assertEquals(Pattern.Any, parsed.descriptor)
    }

    @Test
    fun parsesBareNameStartingWithCapitalLWithDescriptor() {
        val parsed = assertSuccess("Load()V")
        assertEquals(Pattern.Any, parsed.owner)
        assertEquals(Pattern.Exact("Load"), parsed.name)
        assertEquals(
            Pattern.Exact(MethodDescriptor(emptyList(), JvmType.VoidType)),
            parsed.descriptor,
        )
    }

    @Test
    fun parsesQualifiedNameStartingWithCapitalL() {
        val parsed = assertSuccess("Logger.info()V")
        assertEquals(Pattern.Exact("Logger"), parsed.owner)
        assertEquals(Pattern.Exact("info"), parsed.name)
        assertEquals(
            Pattern.Exact(MethodDescriptor(emptyList(), JvmType.VoidType)),
            parsed.descriptor,
        )
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
    fun parsesTerminalNameStarAsExactNameWithAnyDescriptor() {
        val parsed = assertSuccess("run*")
        assertEquals(Pattern.Any, parsed.owner)
        assertEquals(Pattern.Exact("run"), parsed.name)
        assertEquals(Pattern.Any, parsed.descriptor)
    }

    @Test
    fun parsesSpecialMethodTerminalStarAsExactNameWithAnyDescriptor() {
        val bareInit = assertSuccess("<init>*")
        assertEquals(Pattern.Any, bareInit.owner)
        assertEquals(Pattern.Exact("<init>"), bareInit.name)
        assertEquals(Pattern.Any, bareInit.descriptor)

        val bareClinit = assertSuccess("<clinit>*")
        assertEquals(Pattern.Any, bareClinit.owner)
        assertEquals(Pattern.Exact("<clinit>"), bareClinit.name)
        assertEquals(Pattern.Any, bareClinit.descriptor)

        val qualifiedInit = assertSuccess("Lcom/example/Foo;<init>*")
        assertEquals(Pattern.Exact("com/example/Foo"), qualifiedInit.owner)
        assertEquals(Pattern.Exact("<init>"), qualifiedInit.name)
        assertEquals(Pattern.Any, qualifiedInit.descriptor)

        val dotQualifiedClinit = assertSuccess("com.example.Foo.<clinit>*")
        assertEquals(Pattern.Exact("com/example/Foo"), dotQualifiedClinit.owner)
        assertEquals(Pattern.Exact("<clinit>"), dotQualifiedClinit.name)
        assertEquals(Pattern.Any, dotQualifiedClinit.descriptor)
    }

    @Test
    fun parsesStarNameWithExactDescriptor() {
        val parsed = assertSuccess("*()V")
        assertEquals(Pattern.Any, parsed.owner)
        assertEquals(Pattern.Any, parsed.name)
        assertEquals(
            Pattern.Exact(MethodDescriptor(emptyList(), JvmType.VoidType)),
            parsed.descriptor,
        )
    }

    @Test
    fun rejectsMethodNameStartingWithDigit() {
        assertFailure("1run")
        assertFailure("1run()V")
    }

    @Test
    fun rejectsMixedOwnerSeparators() {
        assertFailure("com.example/Foo.run()V")
    }

    @Test
    fun parsesGlobalAnySelector() {
        val parsed = assertSuccess("*")
        assertEquals(Pattern.Any, parsed.owner)
        assertEquals(Pattern.Any, parsed.name)
        assertEquals(Pattern.Any, parsed.descriptor)
    }

    @Test
    fun parsesConstructorAndClassInitializer() {
        val constructor = assertSuccess("Lcom/example/Foo;<init>()V")
        assertEquals(Pattern.Exact("<init>"), constructor.name)
        assertEquals(Pattern.Exact(MethodDescriptor(emptyList(), JvmType.VoidType)), constructor.descriptor)

        val clinit = assertSuccess("Lcom/example/Foo;<clinit>()V")
        assertEquals(Pattern.Exact("<clinit>"), clinit.name)
        assertEquals(Pattern.Exact(MethodDescriptor(emptyList(), JvmType.VoidType)), clinit.descriptor)
    }

    @Test
    fun parsesNestedDollarOwnerSegments() {
        val parsed = assertSuccess("Lcom/example/Outer\$Inner\$1;run(I)V")
        assertEquals(Pattern.Exact("com/example/Outer\$Inner\$1"), parsed.owner)
        assertEquals(
            Pattern.Exact(MethodDescriptor(listOf(JvmType.IntType), JvmType.VoidType)),
            parsed.descriptor,
        )
    }

    @Test
    fun parsesMethodDescriptorWithArrayTypes() {
        val parsed = assertSuccess("Lcom/example/Foo;process([Ljava/lang/String;)V")
        assertEquals(
            Pattern.Exact(
                MethodDescriptor(
                    parameters = listOf(JvmType.ArrayType(JvmType.ObjectType("java/lang/String"))),
                    returnType = JvmType.VoidType,
                ),
            ),
            parsed.descriptor,
        )
    }

    @Test
    fun rejectsEmptyInput() {
        assertFailure("")
    }

    @Test
    fun normalizesAsciiSpacesBeforeParsing() {
        assertSameAsNormalized("Lcom/example/Foo;run()V")
        assertSameAsNormalized(" com.example.Foo.run()V ")
        assertSameAsNormalized("com . example . Foo . run ( ) V")
        assertSameAsNormalized(" run ")
        assertSameAsNormalized(" * ")
        assertSameAsNormalized(" run* ")
        assertSameAsNormalized(" * ( ) V ")
    }

    @Test
    fun rejectsWhitespaceOnlyInputAfterAsciiSpaceNormalization() {
        assertFailure("   ")
    }

    @Test
    fun rejectsNonAsciiWhitespaceWithoutNormalizing() {
        assertFailure("\trun")
        assertFailure("run\n")
        assertFailure("run\u00A0")
        assertFailure("\t*")
    }

    @Test
    fun rejectsFieldColonSyntax() {
        assertFailure("Lcom/example/Foo;count:I")
    }

    @Test
    fun rejectsDottedOwnerInsideInternalForm() {
        assertFailure("Lcom.example.Foo;run()V")
    }

    @Test
    fun rejectsTrailingJunkAfterDescriptor() {
        assertFailure("Lcom/example/Foo;run()Vextra")
    }

    @Test
    fun rejectsInvalidMethodDescriptor() {
        assertFailure("Lcom/example/Foo;run(I)")
    }

    @Test
    fun rejectsMethodDescriptorWithBracketInInternalName() {
        assertFailure("run(Lfoo[Bar;)V")
    }

    @Test
    fun rejectsConstructorWithNonVoidReturn() {
        assertFailure("Lcom/example/Foo;<init>(I)I")
    }

    @Test
    fun rejectsClassInitializerWithNonEmptyDescriptor() {
        assertFailure("Lcom/example/Foo;<clinit>(I)V")
    }

    @Test
    fun rejectsMalformedWildcards() {
        assertFailure("run**")
        assertFailure("*run")
        assertFailure("Lcom/example/Foo;**")
    }

    @Test
    fun rejectsTerminalNameStarWithExplicitDescriptor() {
        assertFailure("run*()V")
        assertFailure("Lcom/example/Foo;run*()V")
        assertFailure("<init>*()V")
        assertFailure("Lcom/example/Foo;<clinit>*()V")
    }

    @Test
    fun acceptsExactStarNameWithExplicitDescriptor() {
        val parsed = assertSuccess("*()V")
        assertEquals(Pattern.Any, parsed.name)
        assertEquals(
            Pattern.Exact(MethodDescriptor(emptyList(), JvmType.VoidType)),
            parsed.descriptor,
        )
    }

    @Test
    fun rejectsMissingOwnerSemicolonForm() {
        assertFailure("Lcom/example/Foo")
    }

    @Test
    fun rejectsEmptyOwnerSegment() {
        assertFailure("Lcom//example/Foo;run()V")
        assertFailure("com..example.Foo.run()V")
    }

    @Test
    fun rejectsWildcardInOwner() {
        assertFailure("Lcom/example/*;run()V")
        assertFailure("com.example.*.run()V")
    }

    private fun assertSameAsNormalized(input: String) {
        val normalized = input.replace(" ", "")
        val expected = assertSuccess(normalized)
        val actual = assertSuccess(input)
        assertEquals(expected, actual)
    }

    private fun assertSuccess(input: String): MethodSelector {
        val parsed = assertIs<DescriptorParseResult.Success<MethodSelector>>(parseMethodSelector(input))
        return parsed.value
    }

    private fun assertFailure(input: String) {
        assertIs<DescriptorParseResult.Failure>(parseMethodSelector(input))
    }
}
