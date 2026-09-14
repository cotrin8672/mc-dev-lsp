package io.github.mcdev.core.mixin

import io.github.mcdev.core.index.ProjectContextMixinIndex
import io.github.mcdev.core.mixin.e2e.MixinE2ETestSupport
import io.github.mcdev.core.model.MemberKind
import io.github.mcdev.core.project.ProjectContextBuilder
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import java.nio.file.Path

class MixinDefinitionServiceTest {
    private val fakeService = MixinDefinitionService(FakeClassIndex(), FakeBytecodeIndex())
    private val simpleService = buildSimpleTargetDefinitionService()

    @Test
    fun resolvesMixinClassTarget() {
        val source = """@Mixin(Mine"""
        val request = MixinE2ETestSupport.requestAt(source, "Mine")
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals(MemberKind.CLASS, targets.first().kind)
        assertEquals("net/minecraft/client/MinecraftClient", targets.first().ownerInternalName)
    }

    @Test
    fun resolvesMixinTargetsString() {
        val source = """@Mixin(targets = "net.minecraft.client.Mine")"""
        val request = MixinE2ETestSupport.requestAt(source, "Mine")
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals("net/minecraft/client/MinecraftClient", targets.first().ownerInternalName)
    }

    @Test
    fun resolvesShadowField() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @Shadow private int counter;
            }
        """.trimIndent()
        val offset = source.indexOf("counter") + 3
        val request = MixinE2ETestSupport.requestAtOffset(source, offset)
        val targets = simpleService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals(MemberKind.FIELD, targets.first().kind)
        assertEquals("counter", targets.first().name)
        assertEquals(MixinE2ETestSupport.SIMPLE_TARGET_INTERNAL, targets.first().ownerInternalName)
    }

    @Test
    fun resolvesShadowMethod() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @Shadow public abstract void draw(String text, float x, float y);
            }
        """.trimIndent()
        val offset = source.indexOf("draw") + 2
        val request = MixinE2ETestSupport.requestAtOffset(source, offset)
        val targets = simpleService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals(MemberKind.METHOD, targets.first().kind)
        assertEquals("draw", targets.first().name)
        assertTrue(targets.first().descriptor!!.contains("Ljava/lang/String;"))
    }

    @Test
    fun resolvesShadowMembersWhenTheCaretIsAtTheNameEnd() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @Shadow private int counter;
                @Shadow public abstract void draw(String text, float x, float y);
            }
        """.trimIndent()

        val fieldOffset = source.indexOf("counter") + "counter".length
        val fieldRequest = MixinE2ETestSupport.requestAtOffset(source, fieldOffset)
        val fieldTarget = simpleService.definitionsAt(fieldRequest.bufferText, fieldRequest.line, fieldRequest.character).single()
        assertEquals(MemberKind.FIELD, fieldTarget.kind)
        assertEquals("counter", fieldTarget.name)

        val methodOffset = source.indexOf("draw") + "draw".length
        val methodRequest = MixinE2ETestSupport.requestAtOffset(source, methodOffset)
        val methodTarget = simpleService.definitionsAt(methodRequest.bufferText, methodRequest.line, methodRequest.character).single()
        assertEquals(MemberKind.METHOD, methodTarget.kind)
        assertEquals("draw", methodTarget.name)
    }

    @Test
    fun semanticMemberDefinitionsOnlyResolveTheExactNameToken() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @Shadow(prefix = "counter") private int counter = 0;
                @Shadow public void draw(String text, float x, float y) { int local = 0; }
                int ordinary = 0;
            }
        """.trimIndent()
        val parsed = MixinSemanticModelParser.parse(source)
        val end = MixinE2ETestSupport.offsetToLineCharacter(source, source.length)
        val broadRange = McTextRange(McTextPosition(0, 0), McTextPosition(end.first, end.second))
        val model = parsed.copy(
            members = parsed.members.map { it.copy(range = broadRange, nameRange = broadRange) },
        )

        for (needle in listOf("counter =", "local =", "ordinary =")) {
            val offset = source.indexOf(needle) + needle.indexOf('=')
            val request = MixinE2ETestSupport.requestAtOffset(source, offset)
            assertTrue(
                simpleService.definitionsAt(
                    request.bufferText,
                    request.line,
                    request.character,
                    semanticModel = model,
                ).isEmpty(),
                "definition leaked into $needle",
            )
        }

        val nameOffset = source.indexOf("counter", source.indexOf("private int")) + 2
        val nameRequest = MixinE2ETestSupport.requestAtOffset(source, nameOffset)
        val target = simpleService.definitionsAt(
            nameRequest.bufferText,
            nameRequest.line,
            nameRequest.character,
            semanticModel = model,
        ).single()
        assertEquals(MemberKind.FIELD, target.kind)
        assertEquals("counter", target.name)
    }

    @Test
    fun resolvesAccessorField() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @Accessor("counter")
                abstract int getCounter();
            }
        """.trimIndent()
        val request = MixinE2ETestSupport.requestInAnnotationValue(source, "@Accessor", "counter")
        val targets = simpleService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals(MemberKind.FIELD, targets.first().kind)
        assertEquals("counter", targets.first().name)
    }

    @Test
    fun resolvesInvokerMethod() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @Invoker("draw")
                abstract void invokeDraw(String text, float x, float y);
            }
        """.trimIndent()
        val request = MixinE2ETestSupport.requestInAnnotationValue(source, "@Invoker", "draw")
        val targets = simpleService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals(MemberKind.METHOD, targets.first().kind)
        assertEquals("draw", targets.first().name)
    }

    @Test
    fun invokerDefinitionDoesNotFallbackToWrongOverload() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @Invoker("draw")
                abstract void invokeDraw();
            }
        """.trimIndent()
        val request = MixinE2ETestSupport.requestInAnnotationValue(source, "@Invoker", "draw")
        val targets = simpleService.definitionsAt(request.bufferText, request.line, request.character)
        assertTrue(targets.isEmpty())
    }

    @Test
    fun invokerDefinitionDoesNotResolveWhenDescriptorContainsUnknownType() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @Invoker("draw")
                abstract void invokeDraw(UnknownType value);
            }
        """.trimIndent()
        val request = MixinE2ETestSupport.requestInAnnotationValue(source, "@Invoker", "draw")
        val targets = simpleService.definitionsAt(request.bufferText, request.line, request.character)
        assertTrue(targets.isEmpty())
    }

    @Test
    fun resolvesAtTargetInvokeMember() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @Inject(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
                private void onDraw() {}
            }
        """.trimIndent()
        val targetMarker = "target = \""
        val valueStart = source.indexOf(targetMarker) + targetMarker.length
        val lengthIndex = source.indexOf("length", valueStart)
        val request = MixinE2ETestSupport.requestAtOffset(source, lengthIndex + 2)
        val targets = simpleService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals(MemberKind.METHOD, targets.first().kind)
        assertEquals("length", targets.first().name)
        assertEquals("java/lang/String", targets.first().ownerInternalName)
    }

    @Test
    fun resolvesInjectorMethodForInjectAndMixinExtras() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @Inject(method = "draw(Ljava/lang/String;FF)V", at = @At("HEAD"))
                private void onDraw() {}
                @ModifyReturnValue(method = "draw(I)V", at = @At("RETURN"))
                private void onDrawInt() {}
            }
        """.trimIndent()

        val injectSelector = "draw(Ljava/lang/String;FF)V"
        val injectRequest = MixinE2ETestSupport.requestAtOffset(source, source.indexOf(injectSelector) + 2)
        val injectTarget = simpleService.definitionsAt(
            injectRequest.bufferText,
            injectRequest.line,
            injectRequest.character,
        ).single()
        assertEquals(MemberKind.METHOD, injectTarget.kind)
        assertEquals("draw", injectTarget.name)
        assertEquals("(Ljava/lang/String;FF)V", injectTarget.descriptor)

        val extrasSelector = "draw(I)V"
        val extrasRequest = MixinE2ETestSupport.requestAtOffset(source, source.indexOf(extrasSelector) + 2)
        val extrasTarget = simpleService.definitionsAt(
            extrasRequest.bufferText,
            extrasRequest.line,
            extrasRequest.character,
        ).single()
        assertEquals(MemberKind.METHOD, extrasTarget.kind)
        assertEquals("draw", extrasTarget.name)
        assertEquals("(I)V", extrasTarget.descriptor)
    }

    @Test
    fun rejectsAmbiguousInjectorMethodAndNonMethodAttribute() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @Inject(method = "draw", at = @At("HEAD"))
                private void onDraw() {}
            }
        """.trimIndent()

        val methodSelector = "draw"
        val methodRequest = MixinE2ETestSupport.requestAtOffset(source, source.indexOf(methodSelector) + 2)
        assertTrue(
            simpleService.definitionsAt(
                methodRequest.bufferText,
                methodRequest.line,
                methodRequest.character,
            ).isEmpty(),
        )

        val atValue = "HEAD"
        val atRequest = MixinE2ETestSupport.requestAtOffset(source, source.indexOf(atValue) + 2)
        assertTrue(
            simpleService.definitionsAt(
                atRequest.bufferText,
                atRequest.line,
                atRequest.character,
            ).isEmpty(),
        )
    }

    @Test
    fun resolvesSimpleTargetMixinClassFromBytecodeIndex() {
        val source = """@Mixin(Simple"""
        val request = MixinE2ETestSupport.requestAt(source, "Simple")
        val targets = simpleService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals(MixinE2ETestSupport.SIMPLE_TARGET_INTERNAL, targets.first().ownerInternalName)
        assertEquals("com.example.target.SimpleTarget", targets.first().ownerFqn)
    }

    @Test
    fun resolvesCompleteMixinTargetBeforeAmbiguousPartialPrefix() {
        val simpleTarget = ClassIndexEntry(
            simpleName = "SimpleTarget",
            packageName = "com.example.target",
            internalName = MixinE2ETestSupport.SIMPLE_TARGET_INTERNAL,
        )
        val crowdedIndex = FakeClassIndex(
            classes = (1..1000).map { number ->
                ClassIndexEntry(
                    simpleName = "Scale$number",
                    packageName = "com.example.scale",
                    internalName = "com/example/scale/Scale$number",
                )
            } + simpleTarget + ClassIndexEntry(
                simpleName = "MissingTargetExtra",
                packageName = "com.example.target",
                internalName = "com/example/target/MissingTargetExtra",
            ),
        )
        val service = MixinDefinitionService(crowdedIndex, FakeBytecodeIndex())
        val source = "@Mixin(SimpleTarget.class)"
        val targetStart = source.indexOf("SimpleTarget")
        val request = MixinE2ETestSupport.requestAtOffset(source, targetStart + 1)

        val completeTargets = service.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, completeTargets.size)
        assertEquals(MixinE2ETestSupport.SIMPLE_TARGET_INTERNAL, completeTargets.single().ownerInternalName)

        val missingSource = "@Mixin(MissingTarget.class)"
        val missingStart = missingSource.indexOf("MissingTarget")
        val missingRequest = MixinE2ETestSupport.requestAtOffset(missingSource, missingStart + 1)
        assertTrue(
            service.definitionsAt(missingRequest.bufferText, missingRequest.line, missingRequest.character).isEmpty(),
            "an unknown complete class name must not navigate to an arbitrary prefix match",
        )

        val semanticModel = MixinClassModel(
            targets = listOf(
                MixinTargetRef(
                    internalName = MixinE2ETestSupport.SIMPLE_TARGET_INTERNAL,
                    range = McTextRange(
                        start = McTextPosition(line = 0, character = targetStart),
                        end = McTextPosition(line = 0, character = targetStart + "SimpleTarget.class".length),
                    ),
                ),
            ),
            injectors = emptyList(),
        )
        val targets = service.definitionsAt(
            request.bufferText,
            request.line,
            request.character,
            semanticModel = semanticModel,
        )
        assertEquals(1, targets.size)
        assertEquals(MixinE2ETestSupport.SIMPLE_TARGET_INTERNAL, targets.single().ownerInternalName)
    }

    @Test
    fun returnsEmptyOutsideMixinContext() {
        val source = "package com.example;\npublic class Plain {}"
        val targets = fakeService.definitionsAt(source, 1, 10)
        assertTrue(targets.isEmpty())
    }

    @Test
    fun atTargetParserParsesFieldDescriptor() {
        val parsed = AtTargetParser.parse("Lcom/example/target/SimpleTarget;counter:I")
        assertNotNull(parsed)
        assertEquals(MemberKind.FIELD, parsed.kind)
        assertEquals("counter", parsed.name)
        assertEquals("I", parsed.descriptor)
    }

    @Test
    fun atTargetParserParsesMethodDescriptor() {
        val parsed = AtTargetParser.parse("Lcom/example/target/SimpleTarget;draw(Ljava/lang/String;FF)V")
        assertNotNull(parsed)
        assertEquals(MemberKind.METHOD, parsed.kind)
        assertEquals("draw", parsed.name)
    }

    @Test
    fun resolvesDefinitionExactMethod() {
        val source = """
            @Definition(id = "drawCall", method = "Lcom/example/target/SimpleTarget;draw(Ljava/lang/String;FF)V")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
            private void onDraw() {}
        """.trimIndent()
        val marker = "Lcom/example/target/SimpleTarget;draw"
        val offset = source.indexOf(marker) + "draw".length
        val request = MixinE2ETestSupport.requestAtOffset(source, offset)
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals(MemberKind.METHOD, targets.first().kind)
        assertEquals("com/example/target/SimpleTarget", targets.first().ownerInternalName)
        assertEquals("draw", targets.first().name)
        assertEquals("(Ljava/lang/String;FF)V", targets.first().descriptor)
        val selector = "Lcom/example/target/SimpleTarget;draw(Ljava/lang/String;FF)V"
        val selectorStart = source.indexOf(selector)
        val selectorEnd = selectorStart + selector.length
        val range = targets.first().sourceRange!!
        val expectedStart = MixinE2ETestSupport.offsetToLineCharacter(source, selectorStart)
        val expectedEnd = MixinE2ETestSupport.offsetToLineCharacter(source, selectorEnd)
        assertEquals(expectedStart.first, range.start.line)
        assertEquals(expectedStart.second, range.start.character)
        assertEquals(expectedEnd.first, range.end.line)
        assertEquals(expectedEnd.second, range.end.character)
    }

    @Test
    fun resolvesDefinitionExactField() {
        val source = """
            @Definition(id = "counterRef", field = "Lcom/example/target/SimpleTarget;counter:I")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
            private void onDraw() {}
        """.trimIndent()
        val request = MixinE2ETestSupport.requestInAnnotationValue(source, "@Definition", "Lcom/example/target/SimpleTarget;counter")
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals(MemberKind.FIELD, targets.first().kind)
        assertEquals("counter", targets.first().name)
        assertEquals("I", targets.first().descriptor)
    }

    @Test
    fun resolvesFullyQualifiedDefinitionExactMethod() {
        val source = """
            @com.llamalad7.mixinextras.expression.Definition(
                method = "Ljava/lang/String;trim()Ljava/lang/String;"
            )
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
            private void onDraw() {}
        """.trimIndent()
        val request = MixinE2ETestSupport.requestInAnnotationValue(
            source,
            "@com.llamalad7.mixinextras.expression.Definition",
            "Ljava/lang/String;trim",
        )
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals(MemberKind.METHOD, targets.first().kind)
        assertEquals("java/lang/String", targets.first().ownerInternalName)
        assertEquals("trim", targets.first().name)
        assertEquals("()Ljava/lang/String;", targets.first().descriptor)
    }

    @Test
    fun resolvesNestedDefinitionFieldInDefinitionsArray() {
        val source = """
            @Definitions({
                @Definition(method = "Lcom/example/Foo;first()V"),
                @Definition(field = "Lcom/example/Foo;count:I")
            })
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
            private void onDraw() {}
        """.trimIndent()
        val fieldMarker = "Lcom/example/Foo;count"
        val offset = source.indexOf(fieldMarker) + "count".length
        val request = MixinE2ETestSupport.requestAtOffset(source, offset)
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals(MemberKind.FIELD, targets.first().kind)
        assertEquals("com/example/Foo", targets.first().ownerInternalName)
        assertEquals("count", targets.first().name)
        assertEquals("I", targets.first().descriptor)
    }

    @Test
    fun resolvesDefinitionLocalTypeClassLiteral() {
        val classIndex = FakeClassIndex(
            classes = FakeClassIndex.defaultClasses() +
                ClassIndexEntry("Foo", "com.example", "com/example/Foo"),
        )
        val service = MixinDefinitionService(classIndex, FakeBytecodeIndex())
        val source = "@Definition(local = @Local(type = com.example.Foo.class ))"
        val request = MixinE2ETestSupport.requestAt(source, "com.example.Foo.")

        val targets = service.definitionsAt(request.bufferText, request.line, request.character)

        assertEquals(1, targets.size)
        assertEquals(MemberKind.CLASS, targets.first().kind)
        assertEquals("com/example/Foo", targets.first().ownerInternalName)
        assertEquals("com.example.Foo", targets.first().ownerFqn)
        val range = assertNotNull(targets.first().sourceRange)
        val typeStart = source.indexOf("com.example.Foo")
        val typeEnd = typeStart + "com.example.Foo.class".length
        val expectedStart = MixinE2ETestSupport.offsetToLineCharacter(source, typeStart)
        val expectedEnd = MixinE2ETestSupport.offsetToLineCharacter(source, typeEnd)
        assertEquals(expectedStart.first, range.start.line)
        assertEquals(expectedStart.second, range.start.character)
        assertEquals(expectedEnd.first, range.end.line)
        assertEquals(expectedEnd.second, range.end.character)
    }

    @Test
    fun resolvesDefinitionLocalTypeImplicitJavaLangClassLiteral() {
        val classIndex = FakeClassIndex(
            classes = FakeClassIndex.defaultClasses() +
                ClassIndexEntry("String", "java.lang", "java/lang/String"),
        )
        val service = MixinDefinitionService(classIndex, FakeBytecodeIndex())
        val source = "@Definition(local = @Local(type = String.class))"
        val request = MixinE2ETestSupport.requestAt(source, "String.")

        val targets = service.definitionsAt(request.bufferText, request.line, request.character)

        assertEquals("java/lang/String", targets.single().ownerInternalName)
    }

    @Test
    fun resolvesEachLocalTypeInsideDefinitionArray() {
        val classIndex = FakeClassIndex(classes = listOf(
            ClassIndexEntry("String", "java.lang", "java/lang/String"),
            ClassIndexEntry("Object", "java.lang", "java/lang/Object"),
        ))
        val service = MixinDefinitionService(classIndex, FakeBytecodeIndex())
        val source = "@Definitions({@Definition(id = \"x\", local = { @Local(type = String.class), @Local(type = Object[].class) })})"
        for (name in listOf("String", "Object")) {
            val request = MixinE2ETestSupport.requestAt(source, name)
            val targets = service.definitionsAt(request.bufferText, request.line, request.character)
            assertEquals("java/lang/$name", targets.single().ownerInternalName)
        }
    }

    @Test
    fun resolvesDefinitionTypeClassLiteral() {
        val classIndex = FakeClassIndex(
            classes = listOf(ClassIndexEntry("String", "java.lang", "java/lang/String")),
        )
        val service = MixinDefinitionService(classIndex, FakeBytecodeIndex())
        for (literal in listOf("String", "String[]", "String[][]")) {
            val source = "@Definition(type = $literal.class)"
            val request = MixinE2ETestSupport.requestAt(source, "String")
            val targets = service.definitionsAt(request.bufferText, request.line, request.character)
            assertEquals("java/lang/String", targets.single().ownerInternalName, literal)
        }
    }

    @Test
    fun resolvesDefinitionLocalTypeThroughExplicitImport() {
        val classIndex = FakeClassIndex(
            classes = listOf(ClassIndexEntry("Foo", "com.example", "com/example/Foo")),
        )
        val service = MixinDefinitionService(classIndex, FakeBytecodeIndex())
        val source = "import com.example.Foo;\n@Definition(local = @Local(type = Foo.class))"
        val request = MixinE2ETestSupport.requestAt(source, "Foo.")

        val targets = service.definitionsAt(request.bufferText, request.line, request.character)

        assertEquals("com/example/Foo", targets.single().ownerInternalName)
    }

    @Test
    fun resolvesDefinitionLocalTypeNestedClassThroughImport() {
        val classIndex = FakeClassIndex(
            classes = listOf(ClassIndexEntry("Outer\$Inner", "com.example", "com/example/Outer\$Inner")),
        )
        val service = MixinDefinitionService(classIndex, FakeBytecodeIndex())
        val source = "import com.example.Outer;\n@Definition(local = @Local(type = Outer.Inner.class))"
        val request = MixinE2ETestSupport.requestAt(source, "Outer.Inner.")

        val targets = service.definitionsAt(request.bufferText, request.line, request.character)

        assertEquals("com/example/Outer\$Inner", targets.single().ownerInternalName)
    }

    @Test
    fun definitionLocalTypeAmbiguousWildcardClassLiteralReturnsEmpty() {
        val classIndex = FakeClassIndex(
            classes = listOf(
                ClassIndexEntry("Foo", "one", "one/Foo"),
                ClassIndexEntry("Foo", "two", "two/Foo"),
            ),
        )
        val service = MixinDefinitionService(classIndex, FakeBytecodeIndex())
        val source = "import one.*;\nimport two.*;\n@Definition(local = @Local(type = Foo.class))"
        val request = MixinE2ETestSupport.requestAt(source, "Foo.")

        val targets = service.definitionsAt(request.bufferText, request.line, request.character)

        assertTrue(targets.isEmpty())
    }

    @Test
    fun definitionLocalTypeUnknownClassLiteralReturnsEmpty() {
        val source = "@Definition(local = @Local(type = Missing.class))"
        val request = MixinE2ETestSupport.requestAt(source, "Missing")

        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)

        assertTrue(targets.isEmpty())
    }

    @Test
    fun definitionMalformedSelectorReturnsEmpty() {
        val source = """
            @Definition(method = "not-a-method-selector")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
            private void onDraw() {}
        """.trimIndent()
        val request = MixinE2ETestSupport.requestInAnnotationValue(source, "@Definition", "not-a-method")
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertTrue(targets.isEmpty())
    }

    @Test
    fun definitionWildcardSelectorReturnsEmpty() {
        val source = """
            @Definition(method = "Lcom/example/Foo;run*")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
            private void onDraw() {}
        """.trimIndent()
        val request = MixinE2ETestSupport.requestInAnnotationValue(source, "@Definition", "run*")
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertTrue(targets.isEmpty())
    }

    @Test
    fun definitionOmittedDescriptorReturnsEmpty() {
        val source = """
            @Definition(method = "Lcom/example/Foo;run")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
            private void onDraw() {}
        """.trimIndent()
        val request = MixinE2ETestSupport.requestInAnnotationValue(source, "@Definition", "Lcom/example/Foo;run")
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertTrue(targets.isEmpty())
    }

    @Test
    fun definitionCursorOutsideValueReturnsEmpty() {
        val source = """
            @Definition(method = "Lcom/example/Foo;run()V")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
            private void onDraw() {}
        """.trimIndent()
        val offset = source.indexOf("method")
        val request = MixinE2ETestSupport.requestAtOffset(source, offset)
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertTrue(targets.isEmpty())
    }

    @Test
    fun definitionDoesNotMatchUnrelatedHandlerMethodString() {
        val source = """
            @Definition(id = "drawCall", method = "Lcom/example/target/SimpleTarget;draw(Ljava/lang/String;FF)V")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
            private void onDraw() {}
        """.trimIndent()
        val request = MixinE2ETestSupport.requestInAnnotationValue(source, "@ModifyExpressionValue", "draw(Ljava/lang")
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertTrue(targets.isEmpty())
    }

    @Test
    fun definitionDoesNotMatchCommentString() {
        val source = """
            // @Definition(method = "Lcom/example/Foo;run()V")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
            private void onDraw() {}
        """.trimIndent()
        val marker = "Lcom/example/Foo;run"
        val offset = source.indexOf(marker) + "run".length
        val request = MixinE2ETestSupport.requestAtOffset(source, offset)
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertTrue(targets.isEmpty())
    }

    @Test
    fun resolvesExpressionIdentifierToUniqueDefinitionId() {
        val source = expressionDefinitionHandlerSource(
            prefix = """@Definition(id = "drawCall", method = "Lcom/example/target/SimpleTarget;draw(Ljava/lang/String;FF)V")""",
            expressionValue = "drawCall",
        )
        val tokenStart = source.indexOf("drawCall", source.indexOf("@Expression"))
        val request = MixinE2ETestSupport.requestAtOffset(source, tokenStart + 2)
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals("drawCall", targets.first().name)
        assertDefinitionIdTargetRange(source, "drawCall", targets.first().sourceRange!!)
        assertEquals(null, targets.first().directSourceDocumentUri)
    }

    @Test
    fun expressionDefinitionIdSetsDirectSourceDocumentUriWhenProvided() {
        val source = expressionDefinitionHandlerSource(
            prefix = """@Definition(id = "drawCall")""",
            expressionValue = "drawCall",
        )
        val documentUri = "file:///ExampleMixin.java"
        val tokenStart = source.indexOf("drawCall", source.indexOf("@Expression"))
        val request = MixinE2ETestSupport.requestAtOffset(source, tokenStart + 2)
        val targets = fakeService.definitionsAt(
            source = request.bufferText,
            line = request.line,
            character = request.character,
            documentUri = documentUri,
        )
        assertEquals(1, targets.size)
        assertEquals(documentUri, targets.first().directSourceDocumentUri)
        assertDefinitionIdTargetRange(source, "drawCall", targets.first().sourceRange!!)
    }

    @Test
    fun definitionExactMethodDoesNotSetDirectSourceDocumentUri() {
        val source = """
            @Definition(id = "drawCall", method = "Lcom/example/target/SimpleTarget;draw(Ljava/lang/String;FF)V")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
            private void onDraw() {}
        """.trimIndent()
        val marker = "Lcom/example/target/SimpleTarget;draw"
        val offset = source.indexOf(marker) + "draw".length
        val request = MixinE2ETestSupport.requestAtOffset(source, offset)
        val targets = fakeService.definitionsAt(
            source = request.bufferText,
            line = request.line,
            character = request.character,
            documentUri = "file:///ExampleMixin.java",
        )
        assertEquals(1, targets.size)
        assertEquals(MemberKind.METHOD, targets.first().kind)
        assertEquals(null, targets.first().directSourceDocumentUri)
    }

    @Test
    fun resolvesExpressionIdentifierWithFqnAnnotations() {
        val source = """
            @com.llamalad7.mixinextras.expression.Definition(id = "lengthCall")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
            @com.llamalad7.mixinextras.expression.Expression("lengthCall")
            private int mcdev${'$'}handler(int original) { return original; }
        """.trimIndent()
        val tokenStart = source.indexOf("lengthCall", source.indexOf("@com.llamalad7.mixinextras.expression.Expression"))
        val request = MixinE2ETestSupport.requestAtOffset(source, tokenStart + 3)
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals("lengthCall", targets.first().name)
        assertDefinitionIdTargetRange(source, "lengthCall", targets.first().sourceRange!!)
    }

    @Test
    fun resolvesExpressionIdentifierFromNestedDefinitions() {
        val source = expressionDefinitionHandlerSource(
            prefix = """
                @Definitions({
                    @Definition(id = "alpha"),
                    @Definition(id = "beta", method = "Lcom/example/Foo;run()V")
                })
            """.trimIndent(),
            expressionValue = "beta",
        )
        val tokenStart = source.indexOf("beta", source.indexOf("@Expression"))
        val request = MixinE2ETestSupport.requestAtOffset(source, tokenStart + 1)
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals("beta", targets.first().name)
        assertDefinitionIdTargetRange(source, "beta", targets.first().sourceRange!!)
    }

    @Test
    fun resolvesExpressionIdentifierWithDefinitionsAfterInjector() {
        val source = """
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
            @Definition(id = "afterInjector")
            @Expression("afterInjector")
            private int mcdev${'$'}handler(int original) { return original; }
        """.trimIndent()
        val tokenStart = source.indexOf("afterInjector", source.indexOf("@Expression"))
        val request = MixinE2ETestSupport.requestAtOffset(source, tokenStart + 4)
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals("afterInjector", targets.first().name)
        assertDefinitionIdTargetRange(source, "afterInjector", targets.first().sourceRange!!)
    }

    @Test
    fun rejectsExpressionIdentifierAfterDotMemberName() {
        val source = expressionDefinitionHandlerSource(
            prefix = """@Definition(id = "lengthCall")""",
            expressionValue = "this.lengthCall",
        )
        val tokenStart = source.indexOf("lengthCall", source.indexOf("@Expression"))
        val request = MixinE2ETestSupport.requestAtOffset(source, tokenStart + 2)
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertTrue(targets.isEmpty())
    }

    @Test
    fun resolvesExpressionMemberAndMethodReferenceDefinitionIds() {
        for ((attribute, selector, expression) in listOf(
            Triple("field", "Lcom/example/target/SimpleTarget;counter:I", "this.member"),
            Triple("method", "Ljava/lang/String;length()I", "value.member()"),
            Triple("method", "Ljava/lang/String;length()I", "value::member"),
        )) {
            val source = expressionDefinitionHandlerSource(
                prefix = """@Definition(id = "member", $attribute = "$selector")""",
                expressionValue = expression,
            )
            val tokenStart = source.indexOf("member", source.indexOf("@Expression"))
            val request = MixinE2ETestSupport.requestAtOffset(source, tokenStart + 3)
            val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
            assertEquals(1, targets.size, expression)
            assertDefinitionIdTargetRange(source, "member", targets.single().sourceRange!!)
        }
    }

    @Test
    fun rejectsExpressionKeywordIdentifier() {
        val source = expressionDefinitionHandlerSource(
            prefix = """@Definition(id = "unused")""",
            expressionValue = "this",
        )
        val tokenStart = source.indexOf("this", source.indexOf("@Expression"))
        val request = MixinE2ETestSupport.requestAtOffset(source, tokenStart + 2)
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertTrue(targets.isEmpty())
    }

    @Test
    fun returnsEveryDefinitionOfRepeatedIdentifier() {
        val source = expressionDefinitionHandlerSource(
            prefix = """
                @Definition(id = "drawCall")
                @Definition(id = "drawCall")
            """.trimIndent(),
            expressionValue = "drawCall",
        )
        val tokenStart = source.indexOf("drawCall", source.indexOf("@Expression"))
        val request = MixinE2ETestSupport.requestAtOffset(source, tokenStart + 2)
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(2, targets.size)
        assertDefinitionIdTargetRange(source, "drawCall", targets[0].sourceRange!!, occurrence = 1)
        assertDefinitionIdTargetRange(source, "drawCall", targets[1].sourceRange!!, occurrence = 2)
    }

    @Test
    fun rejectsMissingExpressionDefinitionId() {
        val source = expressionDefinitionHandlerSource(
            prefix = """@Definition(id = "other")""",
            expressionValue = "missing",
        )
        val tokenStart = source.indexOf("missing", source.indexOf("@Expression"))
        val request = MixinE2ETestSupport.requestAtOffset(source, tokenStart + 2)
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertTrue(targets.isEmpty())
    }

    @Test
    fun isolatesExpressionDefinitionIdByHandler() {
        val source = """
            @Definition(id = "first")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
            @Expression("first")
            private int mcdev${'$'}handler1(int original) { return original; }

            @Definition(id = "second")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
            @Expression("second")
            private int mcdev${'$'}handler2(int original) { return original; }
        """.trimIndent()
        val tokenStart = source.indexOf("second", source.lastIndexOf("@Expression"))
        val request = MixinE2ETestSupport.requestAtOffset(source, tokenStart + 2)
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals("second", targets.first().name)
        assertDefinitionIdTargetRange(source, "second", targets.first().sourceRange!!)
    }

    @Test
    fun rejectsExpressionIdAttributeSelection() {
        val source = """
            @Definition(id = "main")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
            @Expression(id = "slice", value = "main")
            private int mcdev${'$'}handler(int original) { return original; }
        """.trimIndent()
        val expressionAnnotation = source.indexOf("@Expression(id = \"slice\"")
        val tokenStart = source.indexOf("slice", expressionAnnotation)
        val request = MixinE2ETestSupport.requestAtOffset(source, tokenStart + 2)
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertTrue(targets.isEmpty())
    }

    @Test
    fun rejectsExpressionIdentifierInCommentStringFake() {
        val source = expressionDefinitionHandlerSource(
            prefix = """@Definition(id = "drawCall")""",
            expressionValue = "/* drawCall */ 1",
        )
        val marker = "/* drawCall"
        val offset = source.indexOf(marker) + marker.length
        val request = MixinE2ETestSupport.requestAtOffset(source, offset)
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertTrue(targets.isEmpty())
    }

    @Test
    fun resolvesExpressionsContainerIdentifierToDefinitionId() {
        val source = """
            @Definition(id = "drawCall")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
            @Expressions({ "drawCall" })
            private int mcdev${'$'}handler(int original) { return original; }
        """.trimIndent()
        val tokenStart = source.indexOf("drawCall", source.indexOf("@Expressions"))
        val request = MixinE2ETestSupport.requestAtOffset(source, tokenStart + 2)
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals("drawCall", targets.first().name)
        assertDefinitionIdTargetRange(source, "drawCall", targets.first().sourceRange!!)
    }

    @Test
    fun rejectsPartialExpressionDefinitionIdentifier() {
        val source = expressionDefinitionHandlerSource(
            prefix = """@Definition(id = "drawCall")""",
            expressionValue = "drawCal",
        )
        val tokenStart = source.indexOf("drawCal", source.indexOf("@Expression"))
        val request = MixinE2ETestSupport.requestAtOffset(source, tokenStart + 4)
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertTrue(targets.isEmpty())
    }

    @Test
    fun rejectsExpressionIdentifierWithLeadingBackslashAfterJavaDecode() {
        // Java "\\\\drawCall" decodes to Expression DSL "\\drawCall" (literal backslash + identifier),
        // which is not a valid Expression identifier token; definition navigation must not succeed.
        val source = expressionDefinitionHandlerSource(
            prefix = """@Definition(id = "drawCall")""",
            expressionValue = "\\\\drawCall",
        )
        val tokenStart = source.indexOf("drawCall", source.indexOf("@Expression"))
        val request = MixinE2ETestSupport.requestAtOffset(source, tokenStart + 3)
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertTrue(targets.isEmpty())
    }

    @Test
    fun resolvesOctalEscapedExpressionTokenToPlainDefinitionId() {
        val source = expressionDefinitionHandlerSource(
            prefix = """@Definition(id = "drawCall")""",
            expressionValue = "\\144rawCall",
        )
        val escapeStart = source.indexOf("\\144", source.indexOf("@Expression"))
        val request = MixinE2ETestSupport.requestAtOffset(source, escapeStart + 2)
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals("drawCall", targets.first().name)
        assertDefinitionIdTargetRange(source, "drawCall", targets.first().sourceRange!!)
    }

    @Test
    fun resolvesPlainExpressionTokenToOctalEscapedDefinitionId() {
        val source = expressionDefinitionHandlerSource(
            prefix = """@Definition(id = "\144rawCall")""",
            expressionValue = "drawCall",
        )
        val tokenStart = source.indexOf("drawCall", source.indexOf("@Expression"))
        val request = MixinE2ETestSupport.requestAtOffset(source, tokenStart + 3)
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals("drawCall", targets.first().name)
        val idStart = source.indexOf("\\144", source.indexOf("@Definition"))
        val idEnd = source.indexOf("rawCall", idStart) + "rawCall".length
        assertDefinitionIdTargetRangeAtOffset(source, idStart, idEnd, targets.first().sourceRange!!)
    }

    @Test
    fun returnsEquivalentPlainAndEscapedDefinitionIds() {
        val source = expressionDefinitionHandlerSource(
            prefix = """
                @Definition(id = "drawCall")
                @Definition(id = "\144rawCall")
            """.trimIndent(),
            expressionValue = "drawCall",
        )
        val tokenStart = source.indexOf("drawCall", source.indexOf("@Expression"))
        val request = MixinE2ETestSupport.requestAtOffset(source, tokenStart + 3)
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(2, targets.size)
        assertEquals(2, targets.map { it.sourceRange }.distinct().size)
    }

    @Test
    fun preservesRawExpressionTokenOffsetsForOctalEscapedIdentifier() {
        val source = expressionDefinitionHandlerSource(
            prefix = """@Definition(id = "drawCall")""",
            expressionValue = "\\144rawCall",
        )
        val escapeStart = source.indexOf("\\144", source.indexOf("@Expression"))
        val context = assertNotNull(AnnotationContextExtractor.extractAtOffset(source, escapeStart + 2))
        assertEquals("drawCall", context.decodedPartialValue)
        assertEquals(escapeStart, context.valueStartOffset)
        assertEquals(source.indexOf("rawCall", escapeStart) + "rawCall".length, context.valueEndOffset)
    }

    @Test
    fun preservesDefinitionExactMethodNavigation() {
        val source = """
            @Definition(id = "drawCall", method = "Lcom/example/target/SimpleTarget;draw(Ljava/lang/String;FF)V")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
            private void onDraw() {}
        """.trimIndent()
        val marker = "Lcom/example/target/SimpleTarget;draw"
        val offset = source.indexOf(marker) + "draw".length
        val request = MixinE2ETestSupport.requestAtOffset(source, offset)
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals(MemberKind.METHOD, targets.first().kind)
        assertEquals("draw", targets.first().name)
    }

    private fun expressionDefinitionHandlerSource(prefix: String, expressionValue: String): String = """
        @Mixin(com.example.target.SimpleTarget.class)
        abstract class ExampleMixin {
            $prefix
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))
            @Expression("$expressionValue")
            private int mcdev${'$'}handler(int original) { return original; }
        }
    """.trimIndent()

    private fun assertDefinitionIdTargetRange(
        source: String,
        id: String,
        range: io.github.mcdev.core.diagnostics.McTextRange,
        occurrence: Int = 1,
    ) {
        val idMarker = "\"$id\""
        var searchFrom = 0
        repeat(occurrence - 1) {
            searchFrom = source.indexOf(idMarker, searchFrom) + 1
        }
        val idStart = source.indexOf(id, source.indexOf(idMarker, searchFrom) + 1)
        val idEnd = idStart + id.length
        assertDefinitionIdTargetRangeAtOffset(source, idStart, idEnd, range)
    }

    private fun assertDefinitionIdTargetRangeAtOffset(
        source: String,
        idStart: Int,
        idEnd: Int,
        range: io.github.mcdev.core.diagnostics.McTextRange,
    ) {
        val expectedStart = MixinE2ETestSupport.offsetToLineCharacter(source, idStart)
        val expectedEnd = MixinE2ETestSupport.offsetToLineCharacter(source, idEnd)
        assertEquals(expectedStart.first, range.start.line)
        assertEquals(expectedStart.second, range.start.character)
        assertEquals(expectedEnd.first, range.end.line)
        assertEquals(expectedEnd.second, range.end.character)
    }

    private fun buildSimpleTargetDefinitionService(): MixinDefinitionService {
        val provider = MixinE2ETestSupport.simpleTargetProvider()
        val index = ProjectContextMixinIndex()
        val context = ProjectContextBuilder.empty("definition-test", Path.of("."))
        return MixinDefinitionService(
            classIndex = index.buildClassIndex(context, provider),
            bytecodeIndex = index.buildBytecodeIndex(context, provider),
        )
    }
}
