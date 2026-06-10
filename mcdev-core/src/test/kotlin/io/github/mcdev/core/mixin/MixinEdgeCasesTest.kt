package io.github.mcdev.core.mixin

import io.github.mcdev.core.bytecode.ConstantValue
import io.github.mcdev.core.mapping.MappingParseResult
import io.github.mcdev.core.mapping.MappingResolver
import io.github.mcdev.core.mapping.TinyV2Parser
import io.github.mcdev.core.mapping.asResolver
import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.fixtures.FixtureResourceLoader
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class MixinImportEditBuilderTest {
    @Test
    fun generatesImportEditAfterPackageDeclaration() {
        val source = """
            package com.example.mixin;

            @Mixin(Mine
        """.trimIndent()
        val edit = MixinImportEditBuilder.buildImportEdit(source, "net.minecraft.client.MinecraftClient")
        assertNotNull(edit)
        assertEquals("import net.minecraft.client.MinecraftClient;\n", edit.newText.trimStart('\n'))
        assertTrue(edit.startOffset > source.indexOf("com.example.mixin"))
    }

    @Test
    fun generatesImportEditAfterExistingImports() {
        val source = """
            package com.example.mixin;

            import org.spongepowered.asm.mixin.Mixin;

            @Mixin(Mine
        """.trimIndent()
        val edit = MixinImportEditBuilder.buildImportEdit(source, "net.minecraft.client.MinecraftClient")
        assertNotNull(edit)
        assertEquals("import net.minecraft.client.MinecraftClient;\n", edit.newText.trimStart('\n'))
        assertTrue(edit.startOffset > source.indexOf("org.spongepowered.asm.mixin.Mixin"))
    }

    @Test
    fun skipsImportWhenAlreadyPresent() {
        val source = """
            package com.example.mixin;

            import net.minecraft.client.MinecraftClient;

            @Mixin(Mine
        """.trimIndent()
        assertNull(MixinImportEditBuilder.buildImportEdit(source, "net.minecraft.client.MinecraftClient"))
    }

    @Test
    fun skipsImportForSamePackageTarget() {
        val source = """
            package com.example.mixin;

            @Mixin(Example
        """.trimIndent()
        assertNull(MixinImportEditBuilder.buildImportEdit(source, "com.example.mixin.ExampleMixin"))
    }

    @Test
    fun importEditPairsWithImportModeCompletionInsertText() {
        val classIndex = FakeClassIndex()
        val service = MixinTargetCompletionService(classIndex)
        val context = AnnotationContext(
            annotation = MixinAnnotation.MIXIN,
            slot = AnnotationSlot.CLASS,
            partialValue = "Minecraft",
            valueStartOffset = 7,
            valueEndOffset = 16,
            annotationStartOffset = 0,
            annotationEndOffset = 30,
        )
        val source = "package com.example.mixin;\n\n@Mixin(Minecraft"
        val item = service.complete(context, MixinCompletionOptions(MixinClassInsertMode.IMPORT)).first()
        val edit = MixinImportEditBuilder.buildImportEdit(source, "net.minecraft.client.MinecraftClient")
        assertNotNull(edit)
        assertEquals("MinecraftClient.class", item.insertText)
    }
}

class MixinEdgeCasesTest {
    private val classIndex = FakeClassIndex()
    private val bytecodeIndex = FakeBytecodeIndex()
    private val diagnosticsService = MixinDiagnosticsService(classIndex, bytecodeIndex)
    private val targetCompletion = MixinTargetCompletionService(classIndex)

    @Test
    fun mixinArrayClassFormParsesMultipleTargets() {
        val source = """@Mixin({ MinecraftClient.class, GameRenderer.class }) class M {}"""
        val targets = AnnotationContextExtractor.parseMixinTargetValues(source, 0)
        assertEquals(listOf("MinecraftClient", "GameRenderer"), targets)
    }

    @Test
    fun completesClassInsideMixinArrayFormUsingManualContext() {
        val context = AnnotationContext(
            annotation = MixinAnnotation.MIXIN,
            slot = AnnotationSlot.CLASS,
            partialValue = "Game",
            valueStartOffset = 0,
            valueEndOffset = 4,
            annotationStartOffset = 0,
            annotationEndOffset = 0,
        )
        val items = targetCompletion.complete(context)
        assertTrue(items.any { it.label == "GameRenderer" })
        assertEquals("GameRenderer.class", items.first { it.label == "GameRenderer" }.insertText)
    }

    @Test
    fun overwriteAnnotationIsUnsupported() {
        assertNull(MixinAnnotation.fromSimpleName("Overwrite"))
        val source = """@Overwrite public void tick() {}"""
        assertNull(AnnotationContextExtractor.extractAtOffset(source, source.indexOf("Overwrite") + 3))
    }

    @Test
    fun reportsDuplicateMixinClassTargetInArrayForm() {
        val source = """@Mixin({ MinecraftClient.class, MinecraftClient.class }) class M {}"""
        val diagnostics = diagnosticsService.analyze(
            MixinDiagnosticRequest(
                source = source,
                documentUri = "file:///Mixin.java",
                mixinClassName = null,
                mixinPackage = null,
                mixinConfigContent = null,
                mixinConfigPath = null,
            ),
        )
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.DUPLICATE_MIXIN_TARGET })
    }

    @Test
    fun reportsDuplicateMixinStringTarget() {
        val source = """@Mixin(targets = { "net.minecraft.client.MinecraftClient", "net.minecraft.client.MinecraftClient" }) class M {}"""
        val diagnostics = diagnosticsService.analyze(
            MixinDiagnosticRequest(
                source = source,
                documentUri = "file:///Mixin.java",
                mixinClassName = null,
                mixinPackage = null,
                mixinConfigContent = null,
                mixinConfigPath = null,
            ),
        )
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.DUPLICATE_MIXIN_TARGET })
    }

    @Test
    fun reportsDuplicateMixinConfigEntry() {
        val source = """@Mixin(MinecraftClient.class) class ExampleMixin {}"""
        val config = """{ "package": "example.mixin", "mixins": ["ExampleMixin", "OtherMixin", "ExampleMixin"] }"""
        val diagnostics = diagnosticsService.analyze(
            MixinDiagnosticRequest(
                source = source,
                documentUri = "file:///Mixin.java",
                mixinClassName = "ExampleMixin",
                mixinPackage = "example.mixin",
                mixinConfigContent = config,
                mixinConfigPath = "mixins.json",
            ),
        )
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.DUPLICATE_MIXIN_CONFIG_ENTRY })
    }

    @Test
    fun reportsOrdinalOutOfRangeForReturnAt() {
        val source = """
            @Mixin(MinecraftClient.class)
            class M {
                @Inject(method = "tick", at = @At(value = "RETURN", ordinal = 5))
                void m() {}
            }
        """.trimIndent()
        val diagnostics = diagnosticsService.analyze(
            MixinDiagnosticRequest(
                source = source,
                documentUri = "file:///Mixin.java",
                mixinClassName = null,
                mixinPackage = null,
                mixinConfigContent = null,
                mixinConfigPath = null,
            ),
        )
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.ORDINAL_OUT_OF_RANGE })
    }

    @Test
    fun constantAtCompletionIncludesStringValueHint() {
        val candidate = AtTargetCandidate(
            owner = "com/example/Foo",
            name = "",
            descriptor = "",
            displayLabel = "\"hello\"",
            detail = "Foo",
            kind = AtTargetKind.CONSTANT,
            constantValue = ConstantValue.StringValue("hello"),
        )
        val item = AtTargetCompletionService().complete(
            AnnotationContext(
                annotation = MixinAnnotation.AT,
                slot = AnnotationSlot.TARGET,
                partialValue = "",
                valueStartOffset = 0,
                valueEndOffset = 0,
                annotationStartOffset = 0,
                annotationEndOffset = 0,
                atValue = "CONSTANT",
            ),
            listOf(candidate),
        ).first()
        assertEquals("""@Constant(stringValue = "hello")""", item.documentation)
    }

    @Test
    fun constantAtCompletionIncludesIntValueHint() {
        val candidate = AtTargetCandidate(
            owner = "com/example/Foo",
            name = "",
            descriptor = "",
            displayLabel = "42",
            detail = "Foo",
            kind = AtTargetKind.CONSTANT,
            constantValue = ConstantValue.IntValue(42),
        )
        val item = AtTargetCompletionService().complete(
            AnnotationContext(
                annotation = MixinAnnotation.AT,
                slot = AnnotationSlot.TARGET,
                partialValue = "",
                valueStartOffset = 0,
                valueEndOffset = 0,
                annotationStartOffset = 0,
                annotationEndOffset = 0,
                atValue = "CONSTANT",
            ),
            listOf(candidate),
        ).first()
        assertEquals("@Constant(intValue = 42)", item.documentation)
    }
}

class AtTargetMappingTest {
    private val formatter = AtTargetInsertFormatter()
    private val resolver: MappingResolver by lazy {
        val mappingsText = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_MAPPINGS)
        assertIs<MappingParseResult.Success>(TinyV2Parser.parse(mappingsText)).mappings.asResolver()
    }
    private val candidate = AtTargetCandidate(
        owner = "com/example/target/SimpleTarget",
        name = "draw",
        descriptor = "(Ljava/lang/String;FF)V",
        displayLabel = "draw(String, float, float): void",
        detail = "SimpleTarget",
        kind = AtTargetKind.INVOKE,
    )

    @Test
    fun preservesNamedDescriptorWithoutResolver() {
        assertEquals(
            "Lcom/example/target/SimpleTarget;draw(Ljava/lang/String;FF)V",
            formatter.formatInsert(candidate, resolver = null),
        )
    }

    @Test
    fun remapsNamedMethodTargetToIntermediaryUsingFixtureMappings() {
        assertEquals(
            "Lcom/example/target/class_1;method_1(Ljava/lang/String;FF)V",
            formatter.formatInsert(candidate, resolver),
        )
    }

    @Test
    fun leavesReturnTargetsUnchangedWhenRemapping() {
        val returnCandidate = candidate.copy(
            name = "RETURN",
            descriptor = "",
            displayLabel = "RETURN",
            kind = AtTargetKind.RETURN,
        )
        assertEquals("RETURN", formatter.formatInsert(returnCandidate, resolver))
    }
}
