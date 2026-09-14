package io.github.mcdev.core.mixinextras

import kotlin.test.Test
import kotlin.test.assertEquals

class DefinitionsAnnotationParserTest {
    @Test
    fun emptyBodyReturnsEmptyIndex() {
        assertEquals(MixinExtrasDefinitionIndex(), DefinitionsAnnotationParser.parse(""))
    }

    @Test
    fun parsesSingleDefinitionViaValueEquals() {
        val parsed = DefinitionsAnnotationParser.parse("""value = @Definition(id = "main")""")
        assertEquals(
            MixinExtrasDefinitionIndex(listOf(MixinExtrasDefinition(id = "main"))),
            parsed,
        )
    }

    @Test
    fun parsesSingleDefinitionViaUnnamedShorthand() {
        val parsed = DefinitionsAnnotationParser.parse("""@Definition(id = "main")""")
        assertEquals(
            MixinExtrasDefinitionIndex(listOf(MixinExtrasDefinition(id = "main"))),
            parsed,
        )
    }

    @Test
    fun parsesDefinitionArrayPreservingOrder() {
        val body = """
            value = {
                @Definition(id = "first"),
                @Definition(method = "Lcom/example/Foo;run()V"),
                @Definition(field = "Lcom/example/Foo;count:I")
            }
        """.trimIndent()
        val parsed = DefinitionsAnnotationParser.parse(body)
        assertEquals(
            MixinExtrasDefinitionIndex(listOf(
                MixinExtrasDefinition(id = "first"),
                MixinExtrasDefinition(rawMethodReferences = listOf("Lcom/example/Foo;run()V")),
                MixinExtrasDefinition(rawFieldReferences = listOf("Lcom/example/Foo;count:I")),
            )),
            parsed,
        )
    }

    @Test
    fun parsesArrayViaUnnamedShorthand() {
        val body = """
            {
                @Definition(id = "a"),
                @Definition(id = "b")
            }
        """.trimIndent()
        val parsed = DefinitionsAnnotationParser.parse(body)
        assertEquals(
            MixinExtrasDefinitionIndex(
                listOf(
                    MixinExtrasDefinition(id = "a"),
                    MixinExtrasDefinition(id = "b"),
                ),
            ),
            parsed,
        )
    }

    @Test
    fun duplicateIdDefinitionsAreBothRetainedInDocumentOrder() {
        val body = """
            value = {
                @Definition(id = "main", method = "Lcom/example/Foo;first()V"),
                @Definition(id = "main", method = "Lcom/example/Foo;second()V")
            }
        """.trimIndent()
        val parsed = DefinitionsAnnotationParser.parse(body)
        assertEquals(
            MixinExtrasDefinitionIndex(
                listOf(
                    MixinExtrasDefinition(
                        id = "main",
                        rawMethodReferences = listOf("Lcom/example/Foo;first()V"),
                    ),
                    MixinExtrasDefinition(
                        id = "main",
                        rawMethodReferences = listOf("Lcom/example/Foo;second()V"),
                    ),
                ),
            ),
            parsed,
        )
    }

    @Test
    fun parsesEmptyArray() {
        assertEquals(MixinExtrasDefinitionIndex(), DefinitionsAnnotationParser.parse("value = { }"))
        assertEquals(MixinExtrasDefinitionIndex(), DefinitionsAnnotationParser.parse("{}"))
    }

    @Test
    fun parsesFullyQualifiedDefinition() {
        val parsed = DefinitionsAnnotationParser.parse(
            """value = @com.llamalad7.mixinextras.expression.Definition(id = "main")""",
        )
        assertEquals(
            MixinExtrasDefinitionIndex(listOf(MixinExtrasDefinition(id = "main"))),
            parsed,
        )
    }

    @Test
    fun bareDefinitionWithoutParenthesesIsExcluded() {
        assertEquals(
            MixinExtrasDefinitionIndex(),
            DefinitionsAnnotationParser.parse("value = @Definition"),
        )
    }

    @Test
    fun definitionWithoutIdIsRetainedForDiagnostics() {
        assertEquals(
            MixinExtrasDefinitionIndex(listOf(MixinExtrasDefinition())),
            DefinitionsAnnotationParser.parse("value = @Definition()"),
        )
        assertEquals(
            MixinExtrasDefinitionIndex(listOf(MixinExtrasDefinition(rawMethodReferences = listOf("Lcom/example/Foo;run()V")))),
            DefinitionsAnnotationParser.parse("""value = @Definition(method = "Lcom/example/Foo;run()V")"""),
        )
    }

    @Test
    fun malformedIdIsRetainedForDiagnostics() {
        assertEquals(
            MixinExtrasDefinitionIndex(listOf(MixinExtrasDefinition())),
            DefinitionsAnnotationParser.parse("value = @Definition(id = not-a-string)"),
        )
    }

    @Test
    fun nonOfficialDefinitionFqnIsExcluded() {
        assertEquals(
            MixinExtrasDefinitionIndex(),
            DefinitionsAnnotationParser.parse("""value = @foo.Definition(id = "main")"""),
        )
    }

    @Test
    fun trailingGarbageIsExcluded() {
        assertEquals(
            MixinExtrasDefinitionIndex(),
            DefinitionsAnnotationParser.parse("""value = @Definition(id = "main") garbage"""),
        )
        assertEquals(
            MixinExtrasDefinitionIndex(
                listOf(
                    MixinExtrasDefinition(id = "first"),
                    MixinExtrasDefinition(id = "last"),
                ),
            ),
            DefinitionsAnnotationParser.parse(
                """
                value = {
                    @Definition(id = "first"),
                    @Definition(id = "main") trailing,
                    @Definition(id = "last")
                }
                """.trimIndent(),
            ),
        )
    }

    @Test
    fun trailingGarbageRecoverySkipsNestedDelimitersAndComments() {
        val parsed = DefinitionsAnnotationParser.parse(
            """
            value = {
                @Definition(id = "first"),
                @Definition(id = "main") { nested } /* c */ "str" trailing,
                @Definition(id = "last")
            }
            """.trimIndent(),
        )
        assertEquals(
            MixinExtrasDefinitionIndex(
                listOf(
                    MixinExtrasDefinition(id = "first"),
                    MixinExtrasDefinition(id = "last"),
                ),
            ),
            parsed,
        )
    }

    @Test
    fun handlesCommentsWhitespaceAndNestedDefinitionAttributes() {
        val body = """
            // leading
            value = { /* one */ @Definition(id = "main", method = { "Lcom/example/Foo;run()V" }) }
        """.trimIndent()
        val parsed = DefinitionsAnnotationParser.parse(body)
        assertEquals(
            MixinExtrasDefinitionIndex(
                listOf(
                    MixinExtrasDefinition(
                        id = "main",
                        rawMethodReferences = listOf("Lcom/example/Foo;run()V"),
                    ),
                ),
            ),
            parsed,
        )
    }

    @Test
    fun ignoresWrongAnnotationSiblings() {
        val parsed = DefinitionsAnnotationParser.parse(
            """
            value = {
                @Definition(id = "main"),
                @Share("slot"),
                @Slice(id = "foo"),
                @Definition(method = "Lcom/example/Foo;run()V")
            }
            """.trimIndent(),
        )
        assertEquals(
            MixinExtrasDefinitionIndex(listOf(
                MixinExtrasDefinition(id = "main"),
                MixinExtrasDefinition(rawMethodReferences = listOf("Lcom/example/Foo;run()V")),
            )),
            parsed,
        )
    }

    @Test
    fun malformedDefinitionElementsAreIgnoredWithoutDiscardingValidSiblings() {
        val parsed = DefinitionsAnnotationParser.parse(
            """
            value = {
                @Definition(id = "first"),
                broken,
                @Share("x"),
                @Definition(id = "last")
            }
            """.trimIndent(),
        )
        assertEquals(
            MixinExtrasDefinitionIndex(
                listOf(
                    MixinExtrasDefinition(id = "first"),
                    MixinExtrasDefinition(id = "last"),
                ),
            ),
            parsed,
        )
    }

    @Test
    fun failClosedOnUnclosedArrayBrace() {
        assertEquals(
            MixinExtrasDefinitionIndex(),
            DefinitionsAnnotationParser.parse("""value = { @Definition(id = "main")"""),
        )
        assertEquals(
            MixinExtrasDefinitionIndex(),
            DefinitionsAnnotationParser.parse("""{ @Definition(id = "main")"""),
        )
    }

    @Test
    fun failClosedOnUnclosedDefinitionParenInSingleValue() {
        assertEquals(
            MixinExtrasDefinitionIndex(),
            DefinitionsAnnotationParser.parse("""value = @Definition(id = "main""""),
        )
        assertEquals(
            MixinExtrasDefinitionIndex(),
            DefinitionsAnnotationParser.parse("""@Definition(id = "main""""),
        )
    }

    @Test
    fun failClosedOnRootArrayTrailingGarbage() {
        assertEquals(
            MixinExtrasDefinitionIndex(),
            DefinitionsAnnotationParser.parse(
                """
                {
                    @Definition(id = "a"),
                    @Definition(id = "b")
                } garbage
                """.trimIndent(),
            ),
        )
    }

    @Test
    fun failClosedOnNamedArrayTrailingGarbage() {
        assertEquals(
            MixinExtrasDefinitionIndex(),
            DefinitionsAnnotationParser.parse(
                """
                value = {
                    @Definition(id = "a"),
                    @Definition(id = "b")
                } garbage
                """.trimIndent(),
            ),
        )
    }

    @Test
    fun failClosedOnUnknownMember() {
        assertEquals(
            MixinExtrasDefinitionIndex(),
            DefinitionsAnnotationParser.parse("""other = 1, value = @Definition(id = "main")"""),
        )
        assertEquals(
            MixinExtrasDefinitionIndex(),
            DefinitionsAnnotationParser.parse("""value = @Definition(id = "main"), other = 1"""),
        )
    }

    @Test
    fun failClosedOnDuplicateValue() {
        assertEquals(
            MixinExtrasDefinitionIndex(),
            DefinitionsAnnotationParser.parse(
                """
                value = @Definition(id = "a"),
                value = @Definition(id = "b")
                """.trimIndent(),
            ),
        )
        assertEquals(
            MixinExtrasDefinitionIndex(),
            DefinitionsAnnotationParser.parse(
                """
                value = {
                    @Definition(id = "a")
                },
                value = {
                    @Definition(id = "b")
                }
                """.trimIndent(),
            ),
        )
    }

    @Test
    fun assignsAbsoluteIdSourceRangeForSingleDefinition() {
        val source = """@Definition(id = "main")"""
        val parsed = DefinitionsAnnotationParser.parse(source)
        val definition = parsed.definitions.single()
        assertEquals("main", definition.id)
        val idStart = source.indexOf("main")
        assertEquals(idStart until idStart + 4, definition.idSourceRange)
    }

    @Test
    fun assignsAbsoluteIdSourceRangesForNestedDefinitionsArray() {
        val source = """
            {
                @Definition(id = "a"),
                @Definition(id = "beta", method = "Lcom/example/Foo;run()V")
            }
        """.trimIndent()
        val parsed = DefinitionsAnnotationParser.parse(source)
        val definitions = parsed.definitions
        assertEquals(listOf("a", "beta"), definitions.map { it.id })

        val aStart = source.indexOf("a", source.indexOf("id = \"a\""))
        assertEquals(aStart until aStart + 1, definitions[0].idSourceRange)

        val betaStart = source.indexOf("beta", source.indexOf("id = \"beta\""))
        assertEquals(betaStart until betaStart + 4, definitions[1].idSourceRange)
    }

    @Test
    fun idSourceRangeCoversFullTargetIdentifierCharacters() {
        val id = "afterInjector"
        val source = """@Definition(id = "$id")"""
        val parsed = DefinitionsAnnotationParser.parse(source)
        val idStart = source.indexOf(id)
        val idRange = parsed.definitions.single().idSourceRange!!
        assertEquals(idStart, idRange.first)
        assertEquals(idStart + id.length, idRange.last + 1)
    }

    @Test
    fun parseDefinitionAtMapsIdSourceRangeWithNonzeroHandlerRegionBase() {
        val handlerRegion = """
            @ModifyExpressionValue(method = "draw()V", at = @At("MIXINEXTRAS:EXPRESSION"))
            @Definition(id = "afterInjector")
        """.trimIndent()
        val regionStart = 250
        val definitionAt = handlerRegion.indexOf("@Definition")
        val definition = DefinitionsAnnotationParser.parseDefinitionAt(
            source = handlerRegion,
            definitionAtOffset = definitionAt,
            parsedTextBaseOffset = regionStart,
        )!!
        val idStart = regionStart + handlerRegion.indexOf("afterInjector")
        assertEquals(idStart until idStart + "afterInjector".length, definition.idSourceRange)
    }

    @Test
    fun parseWithNonzeroBaseMapsNestedDefinitionsArrayIdSourceRanges() {
        val definitionsBody = """
            {
                @Definition(id = "a"),
                @Definition(id = "beta", method = "Lcom/example/Foo;run()V")
            }
        """.trimIndent()
        val parsedTextBaseOffset = 500
        val parsed = DefinitionsAnnotationParser.parse(definitionsBody, parsedTextBaseOffset = parsedTextBaseOffset)
        val definitions = parsed.definitions
        assertEquals(listOf("a", "beta"), definitions.map { it.id })

        val aStart = parsedTextBaseOffset + definitionsBody.indexOf("a", definitionsBody.indexOf("id = \"a\""))
        assertEquals(aStart until aStart + 1, definitions[0].idSourceRange)

        val betaStart = parsedTextBaseOffset + definitionsBody.indexOf("beta", definitionsBody.indexOf("id = \"beta\""))
        assertEquals(betaStart until betaStart + 4, definitions[1].idSourceRange)
    }
}
