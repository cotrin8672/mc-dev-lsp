package io.github.mcdev.core.mixinextras

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class DefinitionAnnotationParserTest {
    @Test
    fun emptyBodyDefaultsIdToNull() {
        val parsed = DefinitionAnnotationParser.parse("")
        assertEquals(
            MixinExtrasDefinition(),
            parsed,
        )
        assertNull(parsed.id)
    }

    @Test
    fun parsesExplicitEmptyStringId() {
        val parsed = DefinitionAnnotationParser.parse("""id = """"")
        assertEquals("", parsed.id)
        assertEquals(6 until 6, parsed.idContentRange)
    }

    @Test
    fun parsesIdAttribute() {
        val body = """id = "main""""
        val parsed = DefinitionAnnotationParser.parse(body)
        assertEquals("main", parsed.id)
        assertEquals(6 until 10, parsed.idContentRange)
        assertEquals(emptyList(), parsed.rawMethodReferences)
        assertEquals(emptyList(), parsed.rawFieldReferences)
        assertEquals(emptyList(), parsed.classLiteralTypeNames)
        assertNull(parsed.remap)
    }

    @Test
    fun decodesOctalAndStandardJavaEscapesInIdValue() {
        val parsed = DefinitionAnnotationParser.parse("""id = "\144raw\tCall"""")
        assertEquals("draw\tCall", parsed.id)
        assertEquals(6 until 19, parsed.idContentRange)
    }

    @Test
    fun decodesQuotedBackslashEscapesInIdValue() {
        val parsed = DefinitionAnnotationParser.parse("""id = "foo\\bar"""")
        assertEquals("foo\\bar", parsed.id)
    }

    @Test
    fun parsesSingleCharacterIdContentRange() {
        val parsed = DefinitionAnnotationParser.parse("""id = "x"""")
        assertEquals("x", parsed.id)
        assertEquals(6 until 7, parsed.idContentRange)
    }

    @Test
    fun withIdSourceRangeMapsBodyContentToAbsoluteHalfOpenSpan() {
        val parsed = DefinitionAnnotationParser.parse("""id = "main"""")
        parsed.withIdSourceRange(definitionBodyStartInSource = 100)
        assertEquals(106 until 110, parsed.idSourceRange)
    }

    @Test
    fun withIdSourceRangePreservesOneCharacterId() {
        val parsed = DefinitionAnnotationParser.parse("""id = "x"""")
        parsed.withIdSourceRange(definitionBodyStartInSource = 50)
        assertEquals(56 until 57, parsed.idSourceRange)
    }

    @Test
    fun withIdSourceRangeFromHandlerRegionAddsBaseExactlyOnce() {
        val parsed = DefinitionAnnotationParser.parse("""id = "main"""")
        parsed.withIdSourceRangeFromHandlerRegion(
            handlerRegionBaseOffset = 100,
            definitionBodyStartInHandlerRegion = 12,
        )
        assertEquals(118 until 122, parsed.idSourceRange)
    }

    @Test
    fun normalizesSingleMethodStringToList() {
        val parsed = DefinitionAnnotationParser.parse("""method = "Lcom/example/Foo;run()V"""")
        assertEquals(listOf("Lcom/example/Foo;run()V"), parsed.rawMethodReferences)
    }

    @Test
    fun parsesMethodArrayPreservingOrder() {
        val body = """
            method = {
                "Lcom/example/Foo;first()V",
                "Lcom/example/Foo;second()V"
            }
        """.trimIndent()
        val parsed = DefinitionAnnotationParser.parse(body)
        assertEquals(
            listOf("Lcom/example/Foo;first()V", "Lcom/example/Foo;second()V"),
            parsed.rawMethodReferences,
        )
    }

    @Test
    fun normalizesSingleFieldStringToList() {
        val parsed = DefinitionAnnotationParser.parse("""field = "Lcom/example/Foo;count:I"""")
        assertEquals(listOf("Lcom/example/Foo;count:I"), parsed.rawFieldReferences)
    }

    @Test
    fun parsesFieldArrayPreservingOrder() {
        val parsed = DefinitionAnnotationParser.parse(
            """field = { "Lcom/example/Foo;a:I", "Lcom/example/Foo;b:Ljava/lang/String;" }""",
        )
        assertEquals(
            listOf("Lcom/example/Foo;a:I", "Lcom/example/Foo;b:Ljava/lang/String;"),
            parsed.rawFieldReferences,
        )
    }

    @Test
    fun parsesSingleClassLiteralTypes() {
        val cases = mapOf(
            "int.class" to "int",
            "void.class" to "void",
            "String.class" to "String",
            "java.lang.String.class" to "java.lang.String",
            "com.example.Outer${'$'}Inner.class" to "com.example.Outer${'$'}Inner",
            "String[].class" to "String[]",
            "int[][].class" to "int[][]",
            "java.lang.String[].class" to "java.lang.String[]",
        )
        for ((input, expected) in cases) {
            val parsed = DefinitionAnnotationParser.parse("type = $input")
            assertEquals(listOf(expected), parsed.classLiteralTypeNames, "failed for $input")
        }
    }

    @Test
    fun parsesTypeArrayPreservingOrder() {
        val parsed = DefinitionAnnotationParser.parse(
            "type = { String.class, int.class, java.lang.String[].class }",
        )
        assertEquals(
            listOf("String", "int", "java.lang.String[]"),
            parsed.classLiteralTypeNames,
        )
    }

    @Test
    fun parsesRemapBoolean() {
        assertEquals(true, DefinitionAnnotationParser.parse("remap = true").remap)
        assertEquals(false, DefinitionAnnotationParser.parse("remap = false").remap)
    }

    @Test
    fun parsesCombinedUpstreamStyleDefinitionBody() {
        val body = """
            id = "main",
            method = {
                "Lnet/minecraft/client/font/TextRenderer;draw(Ljava/lang/String;FFI)I"
            },
            field = "Lnet/minecraft/client/font/TextRenderer;alpha:F",
            type = { String.class, float.class },
            remap = true
        """.trimIndent()
        val parsed = DefinitionAnnotationParser.parse(body)
        assertEquals(
            MixinExtrasDefinition(
                id = "main",
                rawMethodReferences = listOf(
                    "Lnet/minecraft/client/font/TextRenderer;draw(Ljava/lang/String;FFI)I",
                ),
                rawFieldReferences = listOf("Lnet/minecraft/client/font/TextRenderer;alpha:F"),
                classLiteralTypeNames = listOf("String", "float"),
                remap = true,
            ),
            parsed,
        )
    }

    @Test
    fun preservesSlashSequencesInsideStringLiterals() {
        val parsed = DefinitionAnnotationParser.parse(
            """method = { "foo//bar", "baz/*qux*/", "//leading" }""",
        )
        assertEquals(
            listOf("foo//bar", "baz/*qux*/", "//leading"),
            parsed.rawMethodReferences,
        )
    }

    @Test
    fun handlesCommentsWhitespaceAndEscapedStrings() {
        val body = """
            // leading comment
            id = /* block */ "main" ,
            method = { "Lcom/example/Foo;run(Ljava/lang/String;)V" , "a\"b,c" }
        """.trimIndent()
        val parsed = DefinitionAnnotationParser.parse(body)
        assertEquals("main", parsed.id)
        assertEquals(
            listOf("Lcom/example/Foo;run(Ljava/lang/String;)V", "a\"b,c"),
            parsed.rawMethodReferences,
        )
    }

    @Test
    fun ignoresUnknownAttributes() {
        val parsed = DefinitionAnnotationParser.parse(
            """
            id = "main",
            unknown = 123,
            slice = @Slice(id = "foo")
            """.trimIndent(),
        )
        assertEquals("main", parsed.id)
        assertEquals(emptyList(), parsed.rawMethodReferences)
        assertEquals(emptyList(), parsed.localSpecs)
    }

    @Test
    fun parsesSingleLocalAnnotation() {
        val parsed = DefinitionAnnotationParser.parse("""local = @Local(ordinal = 0)""")
        assertEquals(listOf(HandlerParameterSugarSpec.Local(ordinal = 0)), parsed.localSpecs)
    }

    @Test
    fun parsesLocalArrayPreservingOrder() {
        val parsed = DefinitionAnnotationParser.parse(
            """local = { @Local(ordinal = 0), @Local(name = "value") }""",
        )
        assertEquals(
            listOf(
                HandlerParameterSugarSpec.Local(ordinal = 0),
                HandlerParameterSugarSpec.Local(names = setOf("value")),
            ),
            parsed.localSpecs,
        )
    }

    @Test
    fun parsesFullyQualifiedLocalAnnotation() {
        val parsed = DefinitionAnnotationParser.parse(
            """local = @com.llamalad7.mixinextras.sugar.Local(ordinal = 1)""",
        )
        assertEquals(listOf(HandlerParameterSugarSpec.Local(ordinal = 1)), parsed.localSpecs)
    }

    @Test
    fun ignoresNonOfficialFullyQualifiedLocalAnnotation() {
        val single = DefinitionAnnotationParser.parse("""local = @com.example.Local(ordinal = 0)""")
        assertEquals(emptyList(), single.localSpecs)

        val mixed = DefinitionAnnotationParser.parse(
            """local = { @com.example.Local(ordinal = 0), @Local(ordinal = 1) }""",
        )
        assertEquals(emptyList(), mixed.localSpecs)
    }

    @Test
    fun parsesBareLocalMarkerWithoutParentheses() {
        val parsed = DefinitionAnnotationParser.parse("""local = @Local""")
        assertEquals(listOf(HandlerParameterSugarSpec.Local()), parsed.localSpecs)
    }

    @Test
    fun parsesLocalWithAllAttributes() {
        val parsed = DefinitionAnnotationParser.parse(
            """local = @Local(name = {"alpha", "beta"}, argsOnly = true, index = 2, ordinal = 1, print = true, type = java.lang.String[].class)""",
        )
        assertEquals(
            listOf(
                HandlerParameterSugarSpec.Local(
                    argsOnly = true,
                    index = 2,
                    ordinal = 1,
                    names = setOf("alpha", "beta"),
                    print = true,
                    typeClassName = "java.lang.String[]",
                ),
            ),
            parsed.localSpecs,
        )
    }

    @Test
    fun parsesLocalMinusOneIndexAndOrdinalAsUnset() {
        val parsed = DefinitionAnnotationParser.parse("""local = @Local(index = -1, ordinal = -1)""")
        assertEquals(listOf(HandlerParameterSugarSpec.Local()), parsed.localSpecs)
    }

    @Test
    fun localSugarFallbackRejectsUppercaseBooleanLiterals() {
        val parsed = DefinitionAnnotationParser.parse(
            """local = @Local(argsOnly = TRUE, print = FALSE)""",
        )
        assertEquals(
            listOf(HandlerParameterSugarSpec.Local()),
            parsed.localSpecs,
        )
    }

    @Test
    fun localSugarFallbackRejectsHexAndUnderscoreIntLiterals() {
        val parsed = DefinitionAnnotationParser.parse(
            """local = @Local(index = 0x10, ordinal = 1_0)""",
        )
        assertEquals(
            listOf(HandlerParameterSugarSpec.Local()),
            parsed.localSpecs,
        )
        assertNull(parsed.localSpecs.single().index)
        assertNull(parsed.localSpecs.single().ordinal)
    }

    @Test
    fun localSugarFallbackParsesDecimalIntLiterals() {
        val parsed = DefinitionAnnotationParser.parse("""local = @Local(index = 16)""")
        assertEquals(listOf(HandlerParameterSugarSpec.Local(index = 16)), parsed.localSpecs)
    }

    @Test
    fun malformedLocalArrayRejectsWholeAttribute() {
        val parsed = DefinitionAnnotationParser.parse(
            """
            id = "main",
            local = { @Local(ordinal = 0), @Share("x"), @Local(name = "slot"), broken, @Local(print = true) },
            method = "Lcom/example/Foo;run()V"
            """.trimIndent(),
        )
        assertEquals("main", parsed.id)
        assertEquals(emptyList(), parsed.localSpecs)
        assertEquals(listOf("Lcom/example/Foo;run()V"), parsed.rawMethodReferences)
    }

    @Test
    fun malformedAttributeValuesAreIgnoredWithoutDiscardingValidOnes() {
        val parsed = DefinitionAnnotationParser.parse(
            """
            id = not-a-string,
            method = "Lcom/example/Foo;good()V",
            field = { broken, "Lcom/example/Foo;field:I" },
            type = NotAClass,
            remap = maybe,
            unknown = @Broken(
            """.trimIndent(),
        )
        assertNull(parsed.id)
        assertEquals(listOf("Lcom/example/Foo;good()V"), parsed.rawMethodReferences)
        assertEquals(emptyList(), parsed.rawFieldReferences)
        assertEquals(emptyList(), parsed.classLiteralTypeNames)
        assertNull(parsed.remap)
    }

    @Test
    fun parsesClassLiteralsWithCommentsBetweenSegments() {
        val parsed = DefinitionAnnotationParser.parse("type = java./* pkg */lang.String.class")
        assertEquals(listOf("java.lang.String"), parsed.classLiteralTypeNames)
    }

    @Test
    fun readClassLiteralTypeNameStopsBeforeTerminalClassSuffix() {
        val cases = mapOf(
            "String.class" to "String",
            "java.lang.String.class" to "java.lang.String",
            "classification.Foo.class" to "classification.Foo",
        )
        for ((input, expected) in cases) {
            val parsed = DefinitionAnnotationParser.parse("type = $input")
            assertEquals(listOf(expected), parsed.classLiteralTypeNames, "failed for $input")
        }
    }

    @Test
    fun parsesTypeClassLiteralFollowedByAnotherAttribute() {
        val parsed = DefinitionAnnotationParser.parse("type = String.class, remap = true")
        assertEquals(listOf("String"), parsed.classLiteralTypeNames)
        assertEquals(true, parsed.remap)
    }

    @Test
    fun parsesMethodArrayWithBlockCommentImmediatelyBeforeNextString() {
        val parsed = DefinitionAnnotationParser.parse(
            """method = { "Lcom/example/Foo;first()V",/*c*/"Lcom/example/Foo;second()V" }""",
        )
        assertEquals(
            listOf("Lcom/example/Foo;first()V", "Lcom/example/Foo;second()V"),
            parsed.rawMethodReferences,
        )
    }

    @Test
    fun parsesTypeArrayWithBlockCommentImmediatelyBeforeNextClassLiteral() {
        val parsed = DefinitionAnnotationParser.parse(
            """type = { String.class,/*c*/int.class }""",
        )
        assertEquals(
            listOf("String", "int"),
            parsed.classLiteralTypeNames,
        )
    }

    @Test
    fun duplicateAttributesAppendListsAndUseLastScalarValues() {
        val parsed = DefinitionAnnotationParser.parse(
            """
            id = "first",
            method = "Lcom/example/Foo;one()V",
            id = "last",
            method = "Lcom/example/Foo;two()V",
            remap = false,
            remap = true
            """.trimIndent(),
        )
        assertEquals("last", parsed.id)
        assertEquals(
            listOf("Lcom/example/Foo;one()V", "Lcom/example/Foo;two()V"),
            parsed.rawMethodReferences,
        )
        assertEquals(true, parsed.remap)
    }

    @Test
    fun parsesEmptyStringArrays() {
        assertEquals(emptyList(), DefinitionAnnotationParser.parse("method = {}").rawMethodReferences)
        assertEquals(emptyList(), DefinitionAnnotationParser.parse("field = { }").rawFieldReferences)
    }

    @Test
    fun stringArrayRejectsMixedValidAndMalformedElements() {
        val parsed = DefinitionAnnotationParser.parse(
            """
            id = "main",
            method = { "Lcom/example/Foo;good()V", broken },
            field = { "Lcom/example/Foo;a:I", "Lcom/example/Foo;b:I" }
            """.trimIndent(),
        )
        assertEquals("main", parsed.id)
        assertEquals(emptyList(), parsed.rawMethodReferences)
        assertEquals(
            listOf("Lcom/example/Foo;a:I", "Lcom/example/Foo;b:I"),
            parsed.rawFieldReferences,
        )
    }

    @Test
    fun stringArrayRejectsEmptyCommaElements() {
        val parsed = DefinitionAnnotationParser.parse(
            """method = { "Lcom/example/Foo;good()V", , "Lcom/example/Foo;other()V" }""",
        )
        assertEquals(emptyList(), parsed.rawMethodReferences)
    }

    @Test
    fun stringArrayRejectsTrailingNontriviaAfterElement() {
        val parsed = DefinitionAnnotationParser.parse(
            """method = { "Lcom/example/Foo;good()V" extra }""",
        )
        assertEquals(emptyList(), parsed.rawMethodReferences)
    }

    @Test
    fun classLiteralArrayRejectsMixedValidAndMalformedElements() {
        val parsed = DefinitionAnnotationParser.parse(
            """
            id = "main",
            type = { String.class, NotAClass },
            method = "Lcom/example/Foo;run()V"
            """.trimIndent(),
        )
        assertEquals("main", parsed.id)
        assertEquals(emptyList(), parsed.classLiteralTypeNames)
        assertEquals(listOf("Lcom/example/Foo;run()V"), parsed.rawMethodReferences)
    }

    @Test
    fun classLiteralArrayRejectsEmptyCommaElements() {
        val parsed = DefinitionAnnotationParser.parse("""type = { String.class, , int.class }""")
        assertEquals(emptyList(), parsed.classLiteralTypeNames)
    }

    @Test
    fun classLiteralArrayRejectsTrailingNontriviaAfterElement() {
        val parsed = DefinitionAnnotationParser.parse("""type = { String.class junk }""")
        assertEquals(emptyList(), parsed.classLiteralTypeNames)
    }

    @Test
    fun localArrayRejectsMixedValidAndUnresolvedElements() {
        val parsed = DefinitionAnnotationParser.parse(
            """
            id = "main",
            local = { @Local(ordinal = 0), @Share("x") },
            method = "Lcom/example/Foo;run()V"
            """.trimIndent(),
        )
        assertEquals("main", parsed.id)
        assertEquals(emptyList(), parsed.localSpecs)
        assertEquals(listOf("Lcom/example/Foo;run()V"), parsed.rawMethodReferences)
    }

    @Test
    fun localArrayRejectsEmptyCommaElements() {
        val parsed = DefinitionAnnotationParser.parse(
            """local = { @Local(ordinal = 0), , @Local(name = "slot") }""",
        )
        assertEquals(emptyList(), parsed.localSpecs)
    }

    @Test
    fun localArrayRejectsTrailingNontriviaAfterElement() {
        val parsed = DefinitionAnnotationParser.parse(
            """local = { @Local(ordinal = 0) trailing }""",
        )
        assertEquals(emptyList(), parsed.localSpecs)
    }
}
