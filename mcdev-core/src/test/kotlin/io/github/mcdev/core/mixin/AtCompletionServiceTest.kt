package io.github.mcdev.core.mixin

import io.github.mcdev.core.bytecode.OccurrenceResultClassification
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.model.MappingNamespace
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class AtCompletionServiceTest {
    private val valueService = AtValueCompletionService()
    private val specifierProbeService = AtValueCompletionService(injectionPointSpecifierSupported = { true })
    private val targetService = AtTargetCompletionService()

    @Test
    fun completesAtValueHead() {
        val context = atValueContext("HE")
        val items = valueService.complete(context)
        assertTrue(items.any { it.insertText == "HEAD" })
    }

    @Test
    fun completesModifyReturnValueTail() {
        val items = valueService.complete(
            atValueContext("TA", parentInjector = MixinAnnotation.MODIFY_RETURN_VALUE),
        )
        assertTrue(items.any { it.insertText == "TAIL" })
        assertTrue(
            valueService.complete(
                atValueContext("", parentInjector = MixinAnnotation.MODIFY_RETURN_VALUE),
            ).any { it.insertText == "MIXINEXTRAS:EXPRESSION" },
        )
    }

    @Test
    fun completesAllAtValues() {
        val context = atValueContext("")
        val items = valueService.complete(context)
        assertEquals(
            listOf(
                "HEAD",
                "CTOR_HEAD",
                "RETURN",
                "TAIL",
                "INVOKE",
                "INVOKE_STRING",
                "INVOKE_ASSIGN",
                "FIELD",
                "NEW",
                "CONSTANT",
                "LOAD",
                "STORE",
                "MIXINEXTRAS:EXPRESSION",
            ),
            items.map { it.insertText },
        )
    }

    @Test
    fun wrapOperationAtValueCompletionUsesAllValuesInsideSlice() {
        val insideSlice = valueService.complete(
            atValueContext(
                partial = "",
                atInsideSlice = true,
                parentInjector = MixinAnnotation.WRAP_OPERATION,
            ),
        )
        assertTrue(
            insideSlice.map { it.insertText }.containsAll(listOf("HEAD", "TAIL", "RETURN")),
        )

        val outsideSlice = valueService.complete(
            atValueContext(
                partial = "",
                atInsideSlice = false,
                parentInjector = MixinAnnotation.WRAP_OPERATION,
            ),
        )
        assertEquals(
            listOf("INVOKE", "FIELD", "NEW", "MIXINEXTRAS:EXPRESSION"),
            outsideSlice.map { it.insertText },
        )
    }

    @Test
    fun atValueCompletionFiltersByPrefix() {
        val context = atValueContext("RET")
        val items = valueService.complete(context)
        assertEquals(1, items.size)
        assertEquals("RETURN", items.first().insertText)
    }

    @Test
    fun completesAtInvokeTarget() {
        val context = atTargetContext("")
        val candidate = FakeBytecodeIndex.defaultCandidates().values.first().first()
        val items = targetService.complete(context, listOf(candidate))
        assertEquals(
            "Lnet/minecraft/client/font/TextRenderer;draw(Ljava/lang/String;FFI)I",
            items.first().insertText,
        )
    }

    @Test
    fun atTargetLabelDiffersFromInsertText() {
        val context = atTargetContext("")
        val candidate = FakeBytecodeIndex.defaultCandidates().values.first().first()
        val item = targetService.complete(context, listOf(candidate)).first()
        assertEquals("draw(String, float, float, int): int", item.label)
        assertTrue(item.insertText.startsWith("L"))
    }

    @Test
    fun completesAtFieldTarget() {
        val candidate = FakeBytecodeIndex.defaultCandidates()["net/minecraft/client/MinecraftClient#tick#FIELD"]!!.first()
        val items = targetService.complete(atTargetContext(""), listOf(candidate))
        assertEquals(
            "Lnet/minecraft/client/MinecraftClient;currentScreen:Lnet/minecraft/client/gui/screen/Screen;",
            items.first().insertText,
        )
    }

    @Test
    fun formatTargetForNewClassOnly() {
        val candidate = AtTargetCandidate(
            owner = "net/minecraft/client/gui/DrawContext",
            name = "",
            descriptor = "",
            displayLabel = "DrawContext",
            detail = "",
            kind = AtTargetKind.NEW,
        )
        assertEquals("Lnet/minecraft/client/gui/DrawContext;", targetService.formatTarget(candidate))
    }

    @Test
    fun formatTargetForConstructor() {
        val candidate = AtTargetCandidate(
            owner = "net/minecraft/client/gui/DrawContext",
            name = "<init>",
            descriptor = "()V",
            displayLabel = "DrawContext.<init>()",
            detail = "",
            kind = AtTargetKind.NEW,
        )
        assertEquals("Lnet/minecraft/client/gui/DrawContext;<init>()V", targetService.formatTarget(candidate))
    }

    @Test
    fun atTargetFiltersByPartial() {
        val candidate = FakeBytecodeIndex.defaultCandidates().values.first().first()
        val items = targetService.complete(atTargetContext("TextRenderer"), listOf(candidate))
        assertTrue(items.isNotEmpty())
        val empty = targetService.complete(atTargetContext("zzzzz"), listOf(candidate))
        assertTrue(empty.isEmpty())
    }

    @Test
    fun atTargetDeduplicatesRepeatedIdenticalInvokeOccurrences() {
        val first = invokeCandidate(
            displayLabel = "draw(String, float, float, int): int [occurrence 0]",
            instructionOccurrenceIndex = 0,
            occurrenceResultClassification = OccurrenceResultClassification.RETAINED,
        )
        val second = invokeCandidate(
            displayLabel = "draw(String, float, float, int): int [occurrence 1]",
            instructionOccurrenceIndex = 1,
            occurrenceResultClassification = OccurrenceResultClassification.VOID,
        )
        val items = targetService.complete(atTargetContext(""), listOf(first, second))
        assertEquals(1, items.size)
        assertEquals(
            "Lnet/minecraft/client/font/TextRenderer;draw(Ljava/lang/String;FFI)I",
            items.single().insertText,
        )
        assertEquals(first.displayLabel, items.single().label)
    }

    @Test
    fun atTargetKeepsDistinctInvokeOverloads() {
        val drawString = invokeCandidate(
            name = "draw",
            descriptor = "(Ljava/lang/String;FFI)I",
            displayLabel = "draw(String, float, float, int): int",
        )
        val drawMatrix = invokeCandidate(
            name = "draw",
            descriptor = "(Ljava/lang/String;Lorg/joml/Matrix4f;Lnet/minecraft/client/font/TextRenderer\$TextLayerType;II)V",
            displayLabel = "draw(String, Matrix4f, TextLayerType, int, int): void",
        )
        val items = targetService.complete(atTargetContext(""), listOf(drawString, drawMatrix, drawString))
        assertEquals(2, items.size)
        assertEquals(
            listOf(
                "Lnet/minecraft/client/font/TextRenderer;draw(Ljava/lang/String;FFI)I",
                "Lnet/minecraft/client/font/TextRenderer;draw(Ljava/lang/String;Lorg/joml/Matrix4f;Lnet/minecraft/client/font/TextRenderer\$TextLayerType;II)V",
            ),
            items.map { it.insertText },
        )
    }

    @Test
    fun atTargetDeduplicatesFieldGetAndPutWithSameSelector() {
        val fieldSelector = "Lnet/minecraft/client/MinecraftClient;currentScreen:Lnet/minecraft/client/gui/screen/Screen;"
        val get = fieldCandidate(
            displayLabel = "get currentScreen: Screen",
            operationKind = AtTargetOperationKind.FIELD_GET_INSTANCE,
        )
        val put = fieldCandidate(
            displayLabel = "put currentScreen: Screen",
            operationKind = AtTargetOperationKind.FIELD_PUT_INSTANCE,
        )
        val items = targetService.complete(atTargetContext(""), listOf(get, put))
        assertEquals(1, items.size)
        assertEquals(fieldSelector, items.single().insertText)
        assertEquals(get.displayLabel, items.single().label)
    }

    @Test
    fun atTargetPrefixFilteringUnchangedAfterDeduplication() {
        val matching = invokeCandidate(displayLabel = "draw(String, float, float, int): int")
        val duplicate = invokeCandidate(
            displayLabel = "draw duplicate occurrence",
            instructionOccurrenceIndex = 2,
            occurrenceResultClassification = OccurrenceResultClassification.IMMEDIATELY_POPPED,
        )
        val other = invokeCandidate(
            name = "width",
            descriptor = "(Ljava/lang/String;)I",
            displayLabel = "width(String): int",
        )
        val filtered = targetService.complete(atTargetContext("draw"), listOf(matching, duplicate, other))
        assertEquals(1, filtered.size)
        assertEquals(matching.displayLabel, filtered.single().label)
        assertEquals(
            "Lnet/minecraft/client/font/TextRenderer;draw(Ljava/lang/String;FFI)I",
            filtered.single().insertText,
        )

        val unmatched = targetService.complete(atTargetContext("zzzzz"), listOf(matching, duplicate))
        assertTrue(unmatched.isEmpty())
    }

    @Test
    fun atTargetKeepsDistinctConstantValues() {
        val stringConstant = AtTargetCandidate(
            owner = "",
            name = "",
            descriptor = "",
            displayLabel = "\"hello\"",
            detail = "String",
            kind = AtTargetKind.CONSTANT,
        )
        val intConstant = AtTargetCandidate(
            owner = "",
            name = "",
            descriptor = "",
            displayLabel = "42",
            detail = "int",
            kind = AtTargetKind.CONSTANT,
        )
        val items = targetService.complete(atTargetContext(""), listOf(stringConstant, intConstant, stringConstant))
        assertEquals(2, items.size)
        assertEquals(listOf("\"hello\"", "42"), items.map { it.insertText })
    }

    @Test
    fun atTargetItemMetadataUnchangedForFirstDuplicate() {
        val first = invokeCandidate(
            displayLabel = "draw(String, float, float, int): int",
            detail = "TextRenderer",
        )
        val second = invokeCandidate(
            displayLabel = "draw later occurrence",
            detail = "Other detail",
            instructionOccurrenceIndex = 3,
        )
        val item = targetService.complete(atTargetContext(""), listOf(first, second)).single()
        assertEquals(first.displayLabel, item.label)
        assertEquals(first.detail, item.detail)
        assertEquals("draw draw(String, float, float, int): int TextRenderer", item.filterText)
        assertEquals(McCompletionKind.VALUE, item.kind)
        assertEquals("0400_draw", item.sortKey)
        assertEquals("mixin.atTarget", item.metadata?.source)
        assertEquals(first.owner, item.metadata?.owner)
        assertEquals(first.name, item.metadata?.name)
        assertEquals(first.descriptor, item.metadata?.descriptor)
        assertEquals(MappingNamespace.NAMED.name, item.metadata?.namespace)
    }

    @Test
    fun completesAtSpecifiersInsideSliceWithEmptySuffix() {
        val context = atValueContext("INVOKE:", atInsideSlice = true)
        val items = valueService.complete(context)
        assertEquals(
            listOf("INVOKE:FIRST", "INVOKE:LAST", "INVOKE:ONE", "INVOKE:ALL", "INVOKE:DEFAULT"),
            items.map { it.insertText },
        )
    }

    @Test
    fun atSpecifierCompletionFiltersBySuffixPrefix() {
        val context = atValueContext("INVOKE:F", atInsideSlice = true)
        val items = valueService.complete(context)
        assertEquals(listOf("INVOKE:FIRST"), items.map { it.insertText })
    }

    @Test
    fun atSpecifierCompletionUsesAtInsideSlice() {
        val insideSlice = valueService.complete(atValueContext("INVOKE:", atInsideSlice = true))
        val outsideSlice = valueService.complete(atValueContext("INVOKE:", atInsideSlice = false))
        assertEquals(5, insideSlice.size)
        assertTrue(outsideSlice.isEmpty())
    }

    @Test
    fun atSpecifierCompletionUsesInjectionPointSpecifierProbe() {
        val context = atValueContext("INVOKE:", atInsideSlice = false)
        val items = specifierProbeService.complete(context)
        assertEquals(
            listOf("INVOKE:FIRST", "INVOKE:LAST", "INVOKE:ONE", "INVOKE:ALL", "INVOKE:DEFAULT"),
            items.map { it.insertText },
        )
    }

    @Test
    fun atSpecifierProbeSkippedForOrdinaryPartials() {
        val (probe, probeCalls) = countingThrowingProbe()
        val service = AtValueCompletionService(injectionPointSpecifierSupported = probe)
        assertTrue(service.complete(atValueContext("")).isNotEmpty())
        assertTrue(service.complete(atValueContext("IN")).isNotEmpty())
        assertEquals(0, probeCalls())
    }

    @Test
    fun atSpecifierProbeSkippedForUnknownCode() {
        val (probe, probeCalls) = countingThrowingProbe()
        val service = AtValueCompletionService(injectionPointSpecifierSupported = probe)
        assertTrue(service.complete(atValueContext("FOO:", atInsideSlice = true)).isEmpty())
        assertEquals(0, probeCalls())
    }

    @Test
    fun atSpecifierProbeSkippedForInvalidInjectorCode() {
        val (probe, probeCalls) = countingThrowingProbe()
        val service = AtValueCompletionService(injectionPointSpecifierSupported = probe)
        val context = atValueContext(
            partial = "INVOKE:",
            atInsideSlice = false,
            parentInjector = MixinAnnotation.MODIFY_RETURN_VALUE,
        )
        assertTrue(service.complete(context).isEmpty())
        assertEquals(0, probeCalls())
    }

    @Test
    fun atSpecifierProbeSkippedInsideSlice() {
        val (probe, probeCalls) = countingThrowingProbe()
        val service = AtValueCompletionService(injectionPointSpecifierSupported = probe)
        assertEquals(5, service.complete(atValueContext("INVOKE:", atInsideSlice = true)).size)
        assertEquals(0, probeCalls())
    }

    @Test
    fun atSpecifierProbeCalledOnceOutsideSlice() {
        val (probe, probeCalls) = countingThrowingProbe()
        val service = AtValueCompletionService(injectionPointSpecifierSupported = probe)
        assertEquals(5, service.complete(atValueContext("INVOKE:", atInsideSlice = false)).size)
        assertEquals(1, probeCalls())
    }

    @Test
    fun atSpecifierCompletionUsesFullSliceValuesWithParentInjector() {
        val context = atValueContext(
            partial = "INVOKE:",
            atInsideSlice = true,
            parentInjector = MixinAnnotation.MODIFY_RETURN_VALUE,
        )
        assertEquals(
            listOf("INVOKE:FIRST", "INVOKE:LAST", "INVOKE:ONE", "INVOKE:ALL", "INVOKE:DEFAULT"),
            valueService.complete(context).map { it.insertText },
        )
    }

    @Test
    fun atSpecifierCompletionRejectsUnknownCode() {
        val context = atValueContext("FOO:", atInsideSlice = true)
        assertTrue(valueService.complete(context).isEmpty())
    }

    @Test
    fun completesAtSpecifiersForNamespacedCode() {
        val context = atValueContext("MIXINEXTRAS:EXPRESSION:", atInsideSlice = true)
        val items = valueService.complete(context)
        assertEquals(
            listOf(
                "MIXINEXTRAS:EXPRESSION:FIRST",
                "MIXINEXTRAS:EXPRESSION:LAST",
                "MIXINEXTRAS:EXPRESSION:ONE",
                "MIXINEXTRAS:EXPRESSION:ALL",
                "MIXINEXTRAS:EXPRESSION:DEFAULT",
            ),
            items.map { it.insertText },
        )
    }

    @Test
    fun returnsEmptyForNonAtAnnotation() {
        val context = AnnotationContext(
            annotation = MixinAnnotation.INJECT,
            slot = AnnotationSlot.VALUE,
            partialValue = "HE",
            valueStartOffset = 0,
            valueEndOffset = 2,
            annotationStartOffset = 0,
            annotationEndOffset = 5,
        )
        assertTrue(valueService.complete(context).isEmpty())
    }

    @Test
    fun additionalAtValuesCompletesCustomNamespacedPrefix() {
        val service = AtValueCompletionService(
            additionalAtValues = { listOf("MYMOD:CUSTOM") },
        )
        val items = service.complete(atValueContext("MYMOD:C"))
        assertEquals(listOf("MYMOD:CUSTOM"), items.map { it.insertText })
    }

    @Test
    fun additionalAtValuesCompletesFqnPrefix() {
        val fqn = "com.example.mymod.MyInjectionPoint"
        val service = AtValueCompletionService(
            additionalAtValues = { listOf(fqn) },
        )
        val items = service.complete(atValueContext("com.example.mymod"))
        assertEquals(listOf(fqn), items.map { it.insertText })
    }

    @Test
    fun additionalAtValuesDeduplicatesCaseInsensitively() {
        val service = AtValueCompletionService(
            additionalAtValues = { listOf("head", "HEAD", "Head") },
        )
        val items = service.complete(atValueContext(""))
        assertEquals(1, items.count { it.insertText.equals("HEAD", ignoreCase = true) })
        assertEquals("HEAD", items.first { it.insertText.equals("HEAD", ignoreCase = true) }.insertText)
    }

    @Test
    fun additionalAtValuesDoesNotReintroduceDisallowedBuiltIn() {
        val service = AtValueCompletionService(
            additionalAtValues = { listOf("INVOKE", "HEAD", "MYMOD:CUSTOM") },
        )
        val context = atValueContext("", parentInjector = MixinAnnotation.MODIFY_RETURN_VALUE)
        val items = service.complete(context)
        assertEquals(
            listOf("RETURN", "TAIL", "MIXINEXTRAS:EXPRESSION", "MYMOD:CUSTOM"),
            items.map { it.insertText },
        )
    }

    @Test
    fun additionalAtValuesSpecifierCompletionForCustomCode() {
        val service = AtValueCompletionService(
            additionalAtValues = { listOf("MYMOD:CUSTOM") },
        )
        val context = atValueContext("MYMOD:CUSTOM:", atInsideSlice = true)
        val items = service.complete(context)
        assertEquals(
            listOf("MYMOD:CUSTOM:FIRST", "MYMOD:CUSTOM:LAST", "MYMOD:CUSTOM:ONE", "MYMOD:CUSTOM:ALL", "MYMOD:CUSTOM:DEFAULT"),
            items.map { it.insertText },
        )
    }

    private fun countingThrowingProbe(): Pair<() -> Boolean, () -> Int> {
        var calls = 0
        val probe: () -> Boolean = {
            calls += 1
            if (calls > 1) error("injectionPointSpecifierSupported invoked more than once")
            true
        }
        return probe to { calls }
    }

    private fun atValueContext(
        partial: String,
        atInsideSlice: Boolean = false,
        parentInjector: MixinAnnotation? = null,
    ) = AnnotationContext(
        annotation = MixinAnnotation.AT,
        slot = AnnotationSlot.VALUE,
        partialValue = partial,
        valueStartOffset = 0,
        valueEndOffset = partial.length,
        annotationStartOffset = 0,
        annotationEndOffset = 0,
        parentInjectorAnnotation = parentInjector,
        atInsideSlice = atInsideSlice,
    )

    private fun invokeCandidate(
        owner: String = "net/minecraft/client/font/TextRenderer",
        name: String = "draw",
        descriptor: String = "(Ljava/lang/String;FFI)I",
        displayLabel: String = "draw(String, float, float, int): int",
        detail: String = "TextRenderer",
        instructionOccurrenceIndex: Int = 0,
        occurrenceResultClassification: OccurrenceResultClassification = OccurrenceResultClassification.RETAINED,
    ) = AtTargetCandidate(
        owner = owner,
        name = name,
        descriptor = descriptor,
        displayLabel = displayLabel,
        detail = detail,
        kind = AtTargetKind.INVOKE,
        ordinal = 0,
        namespace = MappingNamespace.NAMED,
        instructionOccurrenceIndex = instructionOccurrenceIndex,
        occurrenceResultClassification = occurrenceResultClassification,
    )

    private fun fieldCandidate(
        owner: String = "net/minecraft/client/MinecraftClient",
        name: String = "currentScreen",
        descriptor: String = "Lnet/minecraft/client/gui/screen/Screen;",
        displayLabel: String = "currentScreen: Screen",
        detail: String = "MinecraftClient",
        operationKind: AtTargetOperationKind,
    ) = AtTargetCandidate(
        owner = owner,
        name = name,
        descriptor = descriptor,
        displayLabel = displayLabel,
        detail = detail,
        kind = AtTargetKind.FIELD,
        ordinal = 0,
        operationKind = operationKind,
    )

    private fun atTargetContext(partial: String) = AnnotationContext(
        annotation = MixinAnnotation.AT,
        slot = AnnotationSlot.TARGET,
        partialValue = partial,
        valueStartOffset = 0,
        valueEndOffset = partial.length,
        annotationStartOffset = 0,
        annotationEndOffset = 0,
        atValue = "INVOKE",
    )
}
