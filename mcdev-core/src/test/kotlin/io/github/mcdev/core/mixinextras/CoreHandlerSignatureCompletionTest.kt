package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.bytecode.BytecodeFixtureCompiler
import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.AnnotationContextExtractor
import io.github.mcdev.core.mixin.AnnotationSlot
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FakeClassIndex
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.mixin.MixinFacadeRequest
import io.github.mcdev.core.mixin.MixinServiceFacade
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class CoreHandlerSignatureCompletionTest {
    @Test
    fun injectUsesTargetParametersAndGenericReturnableCallback() {
        val source = source(
            "SimpleTarget",
            "@Inject(method = \"compute()I\", at = @At(\"RETURN\"))",
        )
        val site = HandlerSignatureService.findSugarHandlerAnnotationSites(source).single()
        val spec = service().expectedSignature(source, site, listOf(SIMPLE_TARGET))

        assertNotNull(spec)
        assertEquals("V", spec.returnTypeDescriptor)
        assertEquals(listOf("ci"), spec.parameters.map { it.name })
        assertEquals(CALLBACK_INFO_RETURNABLE, spec.parameters.last().typeDescriptor)
        assertEquals("Ljava/lang/Integer;", spec.parameters.last().genericTypeDescriptor)
    }

    @Test
    fun injectWithCapturedLocalsFailsClosed() {
        val source = source(
            "SimpleTarget",
            "@Inject(method = \"compute()I\", at = @At(\"RETURN\"), locals = LocalCapture.CAPTURE_FAILSOFT)",
        )
        val site = HandlerSignatureService.findSugarHandlerAnnotationSites(source).single()

        assertEquals(null, service().expectedSignature(source, site, listOf(SIMPLE_TARGET)))
    }

    @Test
    fun constructorHandlersOnlyGenerateAtReturn() {
        val constructor = MethodIndexEntry("<init>", "()V", false, "<init>(): void")
        val classIndex = FakeClassIndex(
            classes = FakeClassIndex.defaultClasses() + ClassIndexEntry(
                "SimpleTarget",
                "com.example.target",
                SIMPLE_TARGET,
            ),
            methods = FakeClassIndex.defaultMethods() + mapOf(SIMPLE_TARGET to listOf(constructor)),
        )
        val signatureService = HandlerSignatureService(classIndex)
        val headSource = source("SimpleTarget", "@Inject(method = \"<init>()V\", at = @At(\"HEAD\"))")
        val returnSource = source("SimpleTarget", "@Inject(method = \"<init>()V\", at = @At(\"RETURN\"))")
        val headSite = HandlerSignatureService.findSugarHandlerAnnotationSites(headSource).single()
        val returnSite = HandlerSignatureService.findSugarHandlerAnnotationSites(returnSource).single()

        assertNull(signatureService.expectedSignature(headSource, headSite, listOf(SIMPLE_TARGET)))
        assertNotNull(signatureService.expectedSignature(returnSource, returnSite, listOf(SIMPLE_TARGET)))
    }

    @Test
    fun multiTargetResolutionRequiresTheSameMethodOnEveryOwner() {
        val targetA = "com/example/target/SharedA"
        val targetB = "com/example/target/SharedB"
        val classes = FakeClassIndex.defaultClasses() + listOf(
            ClassIndexEntry("SharedA", "com.example.target", targetA),
            ClassIndexEntry("SharedB", "com.example.target", targetB),
        )
        val source = source("SharedA", "@Inject(method = \"shared()I\", at = @At(\"RETURN\"))")
        val site = HandlerSignatureService.findSugarHandlerAnnotationSites(source).single()
        val missingOwnerIndex = FakeClassIndex(
            classes = classes,
            methods = FakeClassIndex.defaultMethods() + mapOf(
                targetA to listOf(MethodIndexEntry("shared", "()I", false, "shared(): int")),
                targetB to emptyList(),
            ),
        )
        val mismatchedOwnerIndex = FakeClassIndex(
            classes = classes,
            methods = FakeClassIndex.defaultMethods() + mapOf(
                targetA to listOf(MethodIndexEntry("shared", "()I", false, "shared(): int")),
                targetB to listOf(MethodIndexEntry("shared", "()V", false, "shared(): void")),
            ),
        )

        assertNull(HandlerSignatureService(missingOwnerIndex).expectedSignature(source, site, listOf(targetA, targetB)))
        assertNull(HandlerSignatureService(mismatchedOwnerIndex).expectedSignature(source, site, listOf(targetA, targetB)))
    }

    @Test
    fun redirectArraySelectorsDoNotFallBackToOrdinaryFieldSignatures() {
        val values = listOf("length", "get", "set")
        for (arrayOperation in values) {
            val source = source(
                "SimpleTarget",
                "@Redirect(method = \"compute()I\", at = @At(value = \"FIELD\", target = \"Lcom/example/target/SimpleTarget;label:Ljava/lang/String;\", args = \"array=$arrayOperation\"))",
            )
            val site = HandlerSignatureService.findSugarHandlerAnnotationSites(source).single()
            assertNull(
                service().expectedSignature(source, site, listOf(SIMPLE_TARGET)),
                "array=$arrayOperation must remain unresolved until array bytecode semantics exist",
            )
        }
    }

    @Test
    fun redirectInvokeUsesReceiverAndInvocationArguments() {
        val source = source(
            "SimpleTarget",
            "@Redirect(method = \"draw(Ljava/lang/String;FF)V\", at = @At(value = \"INVOKE\", target = \"Ljava/lang/String;length()I\"))",
        )
        val site = HandlerSignatureService.findSugarHandlerAnnotationSites(source).single()
        val spec = service().expectedSignature(source, site, listOf(SIMPLE_TARGET))

        assertNotNull(spec)
        assertEquals("I", spec.returnTypeDescriptor)
        assertEquals(listOf("instance"), spec.parameters.map { it.name })
        assertEquals("Ljava/lang/String;", spec.parameters.single().typeDescriptor)
    }

    @Test
    fun modifyArgAndModifyArgsUseTheirCoreContracts() {
        val modifyArgSource = source(
            "MinecraftClient",
            "@ModifyArg(method = \"tick()V\", at = @At(value = \"INVOKE\", target = \"Lnet/minecraft/client/font/TextRenderer;draw(Ljava/lang/String;FFI)I\"), index = 0)",
        )
        val modifyArgsSource = source(
            "MinecraftClient",
            "@ModifyArgs(method = \"tick()V\", at = @At(value = \"INVOKE\", target = \"Lnet/minecraft/client/font/TextRenderer;draw(Ljava/lang/String;FFI)I\"))",
        )

        val modifyArg = HandlerSignatureService.findSugarHandlerAnnotationSites(modifyArgSource).single()
        val modifyArgs = HandlerSignatureService.findSugarHandlerAnnotationSites(modifyArgsSource).single()
        val service = service()
        val argSpec = service.expectedSignature(modifyArgSource, modifyArg, listOf(MINECRAFT_CLIENT))
        val argsSpec = service.expectedSignature(modifyArgsSource, modifyArgs, listOf(MINECRAFT_CLIENT))

        assertNotNull(argSpec)
        assertEquals("Ljava/lang/String;", argSpec.returnTypeDescriptor)
        assertEquals(listOf("Ljava/lang/String;"), argSpec.parameters.map { it.typeDescriptor })
        assertNotNull(argsSpec)
        assertEquals("V", argsSpec.returnTypeDescriptor)
        assertEquals("Lorg/spongepowered/asm/mixin/injection/invoke/arg/Args;", argsSpec.parameters.single().typeDescriptor)
    }

    @Test
    fun modifyVariableUsesUniqueLoadSlotFromBytecode() {
        val owner = BytecodeFixtureCompiler.internalName("LocalCaptureSamples")
        val source = source(
            "LocalCaptureSamples",
            "@ModifyVariable(method = \"instanceWithArgs(Ljava/lang/String;I)I\", at = @At(\"LOAD\"), index = 2)",
        )
        val classIndex = FakeClassIndex(
            classes = FakeClassIndex.defaultClasses() + ClassIndexEntry(
                "LocalCaptureSamples",
                "io.github.mcdev.core.bytecode.fixtures",
                owner,
            ),
            methods = FakeClassIndex.defaultMethods() + mapOf(
                owner to listOf(MethodIndexEntry("instanceWithArgs", "(Ljava/lang/String;I)I", false, "instanceWithArgs(String, int): int")),
            ),
        )
        val service = HandlerSignatureService(classIndex, FixtureBytecodeIndex(owner))
        val site = HandlerSignatureService.findSugarHandlerAnnotationSites(source).single()
        val spec = service.expectedSignature(source, site, listOf(owner))

        assertNotNull(spec)
        assertEquals("I", spec.returnTypeDescriptor)
        assertEquals(listOf("I"), spec.parameters.map { it.typeDescriptor })
    }

    @Test
    fun completionUsesEmptyNameTabstopAndBodyTabstop() {
        val source = source(
            "SimpleTarget",
            "@Inject(method = \"compute()I\", at = @At(\"RETURN\"))",
        )
        val signatureService = HandlerSignatureService(
            MixinExtrasTestFixtures.classIndex,
            MixinExtrasTestFixtures.bytecodeIndex,
        )
        val service = MixinExtrasCodeActionService(
            classIndex = MixinExtrasTestFixtures.classIndex,
            signatureService = signatureService,
        )
        val item = service.completeHandler(
            source = source,
            annotationStartOffset = source.indexOf("@Inject"),
            mixinTargets = listOf(SIMPLE_TARGET),
        ).single()

        assertTrue(
            item.insertText.contains("private void \${1}(CallbackInfoReturnable<Integer> ci)"),
            item.insertText,
        )
        assertTrue(item.insertText.contains("\$0"))
        assertTrue(
            item.insertText.contains("\n        \$0\n        // TODO\n"),
            item.insertText,
        )
        assertTrue(item.insertTextFormat.name == "SNIPPET")
        assertTrue(item.insertText.contains("CallbackInfoReturnable"))
        assertTrue(item.insertText.startsWith("\n    private void \${1}"), item.insertText)

        val annotation = "@Inject(method = \"compute()I\", at = @At(\"RETURN\"))"
        val nextLineSource = source.substring(0, source.indexOf(annotation) + annotation.length) + "\n    "
        val nextLineItem = service.completeHandler(
            source = nextLineSource,
            annotationStartOffset = nextLineSource.indexOf("@Inject"),
            mixinTargets = listOf(SIMPLE_TARGET),
            cursorOffset = nextLineSource.length,
        ).single()
        assertTrue(nextLineItem.insertText.startsWith("private void \${1}"), nextLineItem.insertText)
    }

    @Test
    fun generatedCoreStubsUseCompilablePlaceholderBodies() {
        val signatureService = HandlerSignatureService(
            MixinExtrasTestFixtures.classIndex,
            MixinExtrasTestFixtures.bytecodeIndex,
        )
        val service = MixinExtrasCodeActionService(
            classIndex = MixinExtrasTestFixtures.classIndex,
            signatureService = signatureService,
        )
        val redirectSource = source(
            "SimpleTarget",
            "@Redirect(method = \"draw(Ljava/lang/String;FF)V\", at = @At(value = \"INVOKE\", target = \"Ljava/lang/String;length()I\"))",
        )
        val modifyArgSource = source(
            "MinecraftClient",
            "@ModifyArg(method = \"tick()V\", at = @At(value = \"INVOKE\", target = \"Lnet/minecraft/client/font/TextRenderer;draw(Ljava/lang/String;FFI)I\"), index = 0)",
        )
        val redirectSite = HandlerSignatureService.findSugarHandlerAnnotationSites(redirectSource).single()
        val modifyArgSite = HandlerSignatureService.findSugarHandlerAnnotationSites(modifyArgSource).single()
        assertNotNull(signatureService.expectedSignature(redirectSource, redirectSite, listOf(SIMPLE_TARGET)))
        assertNotNull(signatureService.expectedSignature(modifyArgSource, modifyArgSite, listOf(MINECRAFT_CLIENT)))
        val redirectItem = service.completeHandler(
            redirectSource,
            redirectSource.indexOf("@Redirect"),
            listOf(SIMPLE_TARGET),
        ).single()
        val modifyArgItem = service.completeHandler(
            modifyArgSource,
            modifyArgSource.indexOf("@ModifyArg"),
            listOf(MINECRAFT_CLIENT),
        ).single()

        assertTrue(redirectItem.insertText.contains("throw new UnsupportedOperationException();"))
        assertTrue(modifyArgItem.insertText.contains("return original;"))
    }

    @Test
    fun facadeRoutesImmediatelyAfterAnnotationToHandlerSnippet() {
        val source = """
            @Mixin(MinecraftClient.class)
            abstract class ExampleMixin {
                @Inject(method = "tick()V", at = @At("HEAD"))
            }
        """.trimIndent()
        val annotation = "@Inject(method = \"tick()V\", at = @At(\"HEAD\"))"
        val offset = source.indexOf(annotation) + annotation.length
        assertTrue(AnnotationContextExtractor.findInjectorAnnotationOffsets(source).contains(source.indexOf("@Inject")))
        val (line, character) = lineCharacter(source, offset)
        val context = AnnotationContextExtractor.extractAtOffset(source, offset)
        assertNotNull(context)
        assertEquals(AnnotationSlot.HANDLER, context.slot)
        val items = MixinServiceFacade(FakeClassIndex(), io.github.mcdev.core.mixin.FakeBytecodeIndex()).complete(
            MixinFacadeRequest(
                bufferText = source,
                line = line,
                character = character,
                documentUri = "file:///Mixin.java",
            ),
        )

        assertEquals(1, items.size)
        assertTrue(items.single().metadata.source == "mixin.handler")
    }

    @Test
    fun facadeRoutesAccessorInvokerAndOverwriteToDeclarationSnippets() {
        val cases = listOf(
            "@Accessor(\"label\")" to "Accessor",
            "@Invoker(\"compute()I\")" to "Invoker",
            "@Overwrite" to "Overwrite",
        )
        val facade = MixinServiceFacade(
            MixinExtrasTestFixtures.classIndex,
            MixinExtrasTestFixtures.bytecodeIndex,
        )

        for ((annotation, name) in cases) {
            val source = source("SimpleTarget", annotation)
            val annotationOffset = source.indexOf(annotation)
            val offset = annotationOffset + annotation.length
            val (line, character) = lineCharacter(source, offset)
            val items = facade.complete(
                MixinFacadeRequest(
                    bufferText = source,
                    line = line,
                    character = character,
                    documentUri = "file:///Mixin.java",
                ),
            )

            assertTrue(items.isNotEmpty(), "facade should offer @$name declaration snippets")
            assertTrue(items.all { it.metadata.source == "mixin.declaration" })
            assertTrue(items.all { it.insertText.contains("\${1}") })
        }
    }

    @Test
    fun facadeNormalizesDeclarationSnippetIndentationAtBothInsertionBoundaries() {
        val facade = MixinServiceFacade(
            MixinExtrasTestFixtures.classIndex,
            MixinExtrasTestFixtures.bytecodeIndex,
        )
        val annotation = "@Overwrite"
        val sameLineSource = source("SimpleTarget", annotation)
        val sameLineOffset = sameLineSource.indexOf(annotation) + annotation.length
        val sameLine = facade.complete(
            requestAt(sameLineSource, sameLineOffset),
        ).first { it.metadata.source == "mixin.declaration" }

        assertTrue(sameLine.insertText.startsWith("\n    public"), sameLine.insertText)
        assertTrue(sameLine.insertText.contains("\n        \$0\n        throw new AssertionError();\n    }"), sameLine.insertText)
        val sameLineApplied = sameLineSource.substring(0, sameLineOffset) +
            sameLine.insertText + sameLineSource.substring(sameLineOffset)
        assertTrue(sameLineApplied.contains("\n    public"), sameLineApplied)

        val closingBrace = sameLineSource.lastIndexOf('}')
        val indentedSource = sameLineSource.substring(0, closingBrace) +
            "    " + sameLineSource.substring(closingBrace)
        val indentedOffset = closingBrace + 4
        val indented = facade.complete(requestAt(indentedSource, indentedOffset))
            .first { it.metadata.source == "mixin.declaration" }

        assertTrue(indented.insertText.startsWith("public"), indented.insertText)
        assertTrue(indented.insertText.contains("\n        \$0\n        throw new AssertionError();\n    }"), indented.insertText)
        val indentedApplied = indentedSource.substring(0, indentedOffset) +
            indented.insertText + indentedSource.substring(indentedOffset)
        assertTrue(indentedApplied.contains("    public"), indentedApplied)
    }

    private fun service(): HandlerSignatureService =
        HandlerSignatureService(MixinExtrasTestFixtures.classIndex, MixinExtrasTestFixtures.bytecodeIndex)

    private fun source(target: String, annotation: String): String = """
        @Mixin($target.class)
        abstract class ExampleMixin {
            $annotation
        }
    """.trimIndent()

    private fun lineCharacter(source: String, offset: Int): Pair<Int, Int> {
        var line = 0
        var character = 0
        for (index in 0 until offset) {
            if (source[index] == '\n') {
                line++
                character = 0
            } else {
                character++
            }
        }
        return line to character
    }

    private fun requestAt(source: String, offset: Int): MixinFacadeRequest {
        val (line, character) = lineCharacter(source, offset)
        return MixinFacadeRequest(
            bufferText = source,
            line = line,
            character = character,
            documentUri = "file:///Mixin.java",
        )
    }

    private class FixtureBytecodeIndex(
        private val owner: String,
    ) : BytecodeIndex {
        private val classBytes = BytecodeFixtureCompiler.classBytes("LocalCaptureSamples")

        override fun getAtTargetCandidates(
            ownerInternalName: String,
            methodName: String,
            methodDescriptor: String?,
            atValue: String,
        ): List<AtTargetCandidate> = emptyList()

        override fun getReturnOrdinalCount(
            ownerInternalName: String,
            methodName: String,
            methodDescriptor: String?,
        ): Int = 1

        override fun getClassBytes(ownerInternalName: String): ByteArray? =
            classBytes.takeIf { ownerInternalName == owner }
    }

    private companion object {
        const val SIMPLE_TARGET = "com/example/target/SimpleTarget"
        const val MINECRAFT_CLIENT = "net/minecraft/client/MinecraftClient"
        const val CALLBACK_INFO_RETURNABLE =
            "Lorg/spongepowered/asm/mixin/injection/callback/CallbackInfoReturnable;"
    }
}
