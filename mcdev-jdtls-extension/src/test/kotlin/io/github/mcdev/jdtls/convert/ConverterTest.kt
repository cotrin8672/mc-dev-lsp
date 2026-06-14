package io.github.mcdev.jdtls.convert

import io.github.mcdev.core.codeaction.AddAtMethodDescriptorFix
import io.github.mcdev.core.codeaction.AddMixinConfigEntryFix
import io.github.mcdev.core.codeaction.RemoveDuplicateAccessWidenerEntryFix
import io.github.mcdev.core.codeaction.GenerateAccessorMethodFix
import io.github.mcdev.core.codeaction.GenerateInvokerMethodFix
import io.github.mcdev.core.codeaction.McTextEdit
import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata
import io.github.mcdev.core.mapping.MappingParseResult
import io.github.mcdev.core.mapping.TinyV2Parser
import io.github.mcdev.core.mapping.asResolver
import io.github.mcdev.core.mixin.MixinClassInsertMode
import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.fixtures.FixtureResourceLoader
import io.github.mcdev.core.definition.McDefinitionTarget
import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.core.diagnostics.McSeverity
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mixin.AnnotationContext
import io.github.mcdev.core.mixin.AnnotationSlot
import io.github.mcdev.core.mixin.MixinAnnotation
import io.github.mcdev.core.model.MemberKind
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import kotlin.test.assertIs

class ConverterTest {
    @Test
    fun completionConverterPreservesLabelAndInsertText() {
        val item = McCompletionItem(
            label = "SimpleTarget",
            detail = "com.example.target",
            documentation = "com/example/target/SimpleTarget",
            filterText = "SimpleTarget com.example.target",
            insertText = "SimpleTarget.class",
            kind = McCompletionKind.CLASS,
            sortKey = "0100_SimpleTarget",
            metadata = McCompletionMetadata(source = "mixin.target", owner = "com/example/target/SimpleTarget"),
        )
        val dto = CompletionItemConverter.toDto(item, annotationContext("@Mixin(Simple)"), "@Mixin(Simple)")
        assertEquals("SimpleTarget", dto.label)
        assertEquals("SimpleTarget.class", dto.insertText)
        assertEquals("class", dto.kind)
        assertEquals("mixin.target", dto.metadata["source"])
    }

    @Test
    fun completionConverterMapsItemAdditionalEdits() {
        val source = "@Mixin(Simple)"
        val item = McCompletionItem(
            label = "SimpleTarget",
            detail = null,
            documentation = null,
            filterText = "SimpleTarget",
            insertText = "SimpleTarget.class",
            kind = McCompletionKind.CLASS,
            sortKey = "0100",
            metadata = McCompletionMetadata(source = "mixin.target"),
            additionalEdits = listOf(McTextEdit(0, 0, "import com.example.target.SimpleTarget;\n")),
        )
        val dto = CompletionItemConverter.toDto(item, annotationContext(source), source)
        assertEquals(1, dto.additionalEdits.size)
        assertEquals("import com.example.target.SimpleTarget;\n", dto.additionalEdits.first().newText)
    }

    @Test
    fun completionConverterAddsImportAdditionalEditForImportModeMixinTarget() {
        val source = """
            package com.example.mixin;

            import org.spongepowered.asm.mixin.Mixin;

            @Mixin(Simple
        """.trimIndent()
        val item = McCompletionItem(
            label = "SimpleTarget",
            detail = "com.example.target",
            documentation = "com/example/target/SimpleTarget",
            filterText = "SimpleTarget",
            insertText = "SimpleTarget.class",
            kind = McCompletionKind.CLASS,
            sortKey = "0100",
            metadata = McCompletionMetadata(
                source = "mixin.target",
                owner = "com/example/target/SimpleTarget",
            ),
        )
        val dto = CompletionItemConverter.toDto(
            item = item,
            annotationContext = annotationContext(source, valueStart = source.indexOf("Simple"), valueEnd = source.length),
            source = source,
            convertContext = CompletionConvertContext(
                source = source,
                annotationContext = annotationContext(source, valueStart = source.indexOf("Simple"), valueEnd = source.length),
                classInsertMode = MixinClassInsertMode.IMPORT,
            ),
        )
        assertEquals(1, dto.additionalEdits.size)
        assertTrue(dto.additionalEdits.first().newText.contains("import com.example.target.SimpleTarget;"))
    }

    @Test
    fun completionConverterRemapsAtTargetInsertTextWithMappings() {
        val mappingsText = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_MAPPINGS)
        val resolver = assertIs<MappingParseResult.Success>(TinyV2Parser.parse(mappingsText)).mappings.asResolver()
        val item = McCompletionItem(
            label = "draw(String, float, float): void",
            detail = "SimpleTarget",
            documentation = "Lcom/example/target/SimpleTarget;draw(Ljava/lang/String;FF)V",
            filterText = "draw",
            insertText = "Lcom/example/target/SimpleTarget;draw(Ljava/lang/String;FF)V",
            kind = McCompletionKind.METHOD,
            sortKey = "0400_draw",
            metadata = McCompletionMetadata(
                source = "mixin.atTarget",
                owner = "com/example/target/SimpleTarget",
                name = "draw",
                descriptor = "(Ljava/lang/String;FF)V",
            ),
        )
        val dto = CompletionItemConverter.toDto(
            item = item,
            annotationContext = null,
            source = "",
            convertContext = CompletionConvertContext(
                source = "",
                annotationContext = null,
                preferredAtTarget = "descriptor",
                mappingResolver = resolver,
            ),
        )
        assertEquals(
            "Lcom/example/target/class_1;method_1(Ljava/lang/String;FF)V",
            dto.insertText,
        )
    }

    @Test
    fun completionConverterUsesShortAtTargetInsertTextWhenSmartTargetIsUnambiguous() {
        val mappingsText = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_MAPPINGS)
        val resolver = assertIs<MappingParseResult.Success>(TinyV2Parser.parse(mappingsText)).mappings.asResolver()
        val item = McCompletionItem(
            label = "draw(String, float, float): void",
            detail = "SimpleTarget",
            documentation = "Lcom/example/target/SimpleTarget;draw(Ljava/lang/String;FF)V",
            filterText = "draw",
            insertText = "Lcom/example/target/SimpleTarget;draw(Ljava/lang/String;FF)V",
            kind = McCompletionKind.METHOD,
            sortKey = "0400_draw",
            metadata = McCompletionMetadata(
                source = "mixin.atTarget",
                owner = "com/example/target/SimpleTarget",
                name = "draw",
                descriptor = "(Ljava/lang/String;FF)V",
            ),
        )
        val dto = CompletionItemConverter.toDtos(
            items = listOf(item),
            annotationContext = null,
            source = "",
            convertContext = CompletionConvertContext(
                source = "",
                annotationContext = null,
                preferredAtTarget = "smart",
                mappingResolver = resolver,
            ),
        ).single()
        assertEquals("method_1", dto.insertText)
    }

    @Test
    fun completionConverterBuildsTextEditFromAnnotationOffsets() {
        val source = "@Mixin(Simple)"
        val context = annotationContext(source, valueStart = 7, valueEnd = 13)
        val item = McCompletionItem(
            label = "SimpleTarget",
            detail = null,
            documentation = null,
            filterText = "SimpleTarget",
            insertText = "SimpleTarget.class",
            kind = McCompletionKind.CLASS,
            sortKey = "0100",
            metadata = McCompletionMetadata(source = "mixin.target"),
        )
        val dto = CompletionItemConverter.toDto(item, context, source)
        assertNotNull(dto.edit)
        assertEquals("SimpleTarget.class", dto.edit?.newText)
        assertEquals(0, dto.edit?.range?.start?.line)
        assertEquals(7, dto.edit?.range?.start?.character)
    }

    @Test
    fun diagnosticConverterMapsSeverityAndRange() {
        val diagnostic = McDiagnostic(
            code = "UNRESOLVED_MIXIN_TARGET",
            severity = McSeverity.ERROR,
            message = "Unresolved @Mixin target",
            range = McTextRange(
                start = McTextPosition(1, 2),
                end = McTextPosition(1, 10),
            ),
            metadata = mapOf("target" to "Missing"),
        )
        val dto = DiagnosticConverter.toDto(diagnostic)
        assertEquals("error", dto.severity)
        assertEquals("UNRESOLVED_MIXIN_TARGET", dto.code)
        assertEquals("Missing", dto.metadata["target"])
    }

    @Test
    fun diagnosticConverterMapsWarningSeverity() {
        val dto = DiagnosticConverter.toDto(
            McDiagnostic(
                code = "MIXIN_CLASS_NOT_LISTED_IN_CONFIG",
                severity = McSeverity.WARNING,
                message = "not listed",
                range = McTextRange(McTextPosition(0, 0), McTextPosition(0, 0)),
            ),
        )
        assertEquals("warning", dto.severity)
    }

    @Test
    fun codeActionConverterMapsMixinConfigFix() {
        val fix = AddMixinConfigEntryFix(
            title = "Add 'ExampleMixin' to mixin config",
            configPath = "mixins.json",
            mixinClassName = "ExampleMixin",
            mixinPackage = "com.example.mixin",
        )
        val dto = CodeActionConverter.toDto(
            fix = fix,
            source = "@Mixin(Simple.class) class ExampleMixin {}",
            mixinConfigContent = """{ "mixins": [] }""",
        )
        assertNotNull(dto)
        assertEquals("quickfix.mixin.config", dto?.kind)
        assertTrue(dto!!.edits.first().edits.first().newText.contains("ExampleMixin"))
    }

    @Test
    fun codeActionConverterMapsAccessorGenerateFix() {
        val source = "@Mixin(MinecraftClient.class) class M { }"
        val openBrace = source.indexOf('{') + 1
        val fix = GenerateAccessorMethodFix(
            title = "Generate @Accessor getter for 'currentScreen'",
            documentUri = "file:///Mixin.java",
            insertOffset = openBrace,
            methodSource = "    @Accessor(\"currentScreen\")\n    Screen getCurrentScreen();\n",
            fieldName = "currentScreen",
            isGetter = true,
        )
        val dto = CodeActionConverter.toDto(fix, source, mixinConfigContent = null)
        assertNotNull(dto)
        assertEquals("quickfix.mixin.generateAccessor", dto?.kind)
        assertTrue(dto!!.edits.first().edits.first().newText.contains("currentScreen"))
    }

    @Test
    fun codeActionConverterMapsRemoveDuplicateAccessWidenerFix() {
        val source = """
            accessWidener v2 named
            accessible class com/example/target/SimpleTarget
            accessible class com/example/target/SimpleTarget
        """.trimIndent()
        val fix = RemoveDuplicateAccessWidenerEntryFix(
            title = "Remove duplicate access widener entry",
            documentUri = "file:///mod.accesswidener",
            lineNumber = 3,
        )
        val dto = CodeActionConverter.toDto(fix, source, mixinConfigContent = null)
        assertNotNull(dto)
        assertEquals("quickfix.aw.removeDuplicate", dto?.kind)
        assertEquals(1, dto!!.edits.single().edits.size)
    }

    @Test
    fun codeActionConverterMapsAddAtMethodDescriptorFix() {
        val source = "public com.example.target.SimpleTarget draw"
        val fix = AddAtMethodDescriptorFix(
            title = "Add method descriptor",
            documentUri = "file:///mod_at.cfg",
            line = 1,
            memberName = "draw",
            descriptor = "(Ljava/lang/String;FF)V",
        )
        val dto = CodeActionConverter.toDto(fix, source, mixinConfigContent = null)
        assertNotNull(dto)
        assertEquals("quickfix.at.addDescriptor", dto?.kind)
        assertTrue(dto!!.edits.first().edits.first().newText.contains("draw(Ljava/lang/String;FF)V"))
    }

    @Test
    fun codeActionConverterMapsMixinExtrasFixHandlerSignature() {
        val source = """
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private void mcdevWrapLength(String instance, Operation<Integer> original) {
                original.call(instance);
            }
        """.trimIndent()
        val openBrace = source.indexOf('{') + 1
        val fix = io.github.mcdev.core.codeaction.WorkspaceEditFix(
            title = "Fix WrapOperation handler signature",
            kind = "quickfix.mixinextras.fixHandlerSignature",
            documentUri = "file:///BadMixinExtras.java",
            edits = listOf(
                McTextEdit(
                    openBrace,
                    openBrace,
                    "\n    private int mcdevWrapLength(String instance, Operation<Integer> original) {\n        return original.call(instance);\n    }\n",
                ),
            ),
        )
        val dto = CodeActionConverter.toDto(fix, source, mixinConfigContent = null)
        assertNotNull(dto)
        assertEquals("quickfix.mixinextras.fixHandlerSignature", dto?.kind)
        assertTrue(dto!!.edits.first().edits.first().newText.contains("Operation<Integer>"))
    }

    @Test
    fun codeActionConverterMapsInvokerGenerateFix() {
        val source = "@Mixin(MinecraftClient.class) class M { }"
        val openBrace = source.indexOf('{') + 1
        val fix = GenerateInvokerMethodFix(
            title = "Generate @Invoker for 'setScreen'",
            documentUri = "file:///Mixin.java",
            insertOffset = openBrace,
            methodSource = "    @Invoker(\"setScreen\")\n    void invokeSetScreen(Screen screen);\n",
            targetMethodName = "setScreen",
        )
        val dto = CodeActionConverter.toDto(fix, source, mixinConfigContent = null)
        assertNotNull(dto)
        assertEquals("quickfix.mixin.generateInvoker", dto?.kind)
        assertTrue(dto!!.edits.first().edits.first().newText.contains("setScreen"))
    }

    @Test
    fun definitionConverterMapsClassTargetMetadata() {
        val target = McDefinitionTarget(
            kind = MemberKind.CLASS,
            ownerInternalName = "com/example/target/SimpleTarget",
            ownerFqn = "com.example.target.SimpleTarget",
        )
        val location = DefinitionConverter.toLocation(
            io.github.mcdev.jdtls.definition.ResolvedDefinition(
                target = target,
                documentUri = "file:///project/SimpleTarget.java",
                range = McTextRange(McTextPosition(2, 0), McTextPosition(2, 20)),
                resolution = io.github.mcdev.protocol.McdevDefinitionResolution.SOURCE,
            ),
        )
        assertEquals("class", location.metadata["kind"])
        assertEquals("com/example/target/SimpleTarget", location.metadata["owner"])
        assertEquals("com.example.target.SimpleTarget", location.metadata["fqn"])
        assertEquals(io.github.mcdev.protocol.McdevDefinitionResolution.SOURCE, location.resolution)
        assertEquals("file:///project/SimpleTarget.java", location.documentUri)
    }

    @Test
    fun definitionConverterMapsFieldTargetMetadata() {
        val target = McDefinitionTarget(
            kind = MemberKind.FIELD,
            ownerInternalName = "com/example/target/SimpleTarget",
            ownerFqn = "com.example.target.SimpleTarget",
            name = "counter",
            descriptor = "I",
        )
        val location = DefinitionConverter.toLocation(
            io.github.mcdev.jdtls.definition.ResolvedDefinition(
                target = target,
                documentUri = "file:///project/SimpleTarget.java",
                range = McTextRange(McTextPosition(3, 16), McTextPosition(3, 23)),
                resolution = io.github.mcdev.protocol.McdevDefinitionResolution.SOURCE,
            ),
        )
        assertEquals("field", location.metadata["kind"])
        assertEquals("counter", location.metadata["name"])
        assertEquals("I", location.metadata["descriptor"])
        assertEquals(io.github.mcdev.protocol.McdevDefinitionResolution.SOURCE, location.resolution)
    }

    private fun annotationContext(
        source: String = "@Mixin(Simple)",
        valueStart: Int = 7,
        valueEnd: Int = source.length - 1,
    ): AnnotationContext =
        AnnotationContext(
            annotation = MixinAnnotation.MIXIN,
            slot = AnnotationSlot.CLASS,
            partialValue = "Simple",
            valueStartOffset = valueStart,
            valueEndOffset = valueEnd,
            annotationStartOffset = 0,
            annotationEndOffset = source.length,
        )
}
