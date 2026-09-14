package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mixin.FakeClassIndex
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class LocalCompletionServiceTest {
    private val service = LocalCompletionService(FakeClassIndex())
    private val source = "class Mixin {}"

    @Test
    fun returnsEmptyWhenResultsEmpty() {
        assertTrue(
            service.complete(source, parameter(), emptyList(), "ordinal").isEmpty(),
        )
    }

    @Test
    fun returnsEmptyWhenAnyResultUnavailable() {
        val results = listOf(
            result(snapshots = listOf(snapshot(candidates = listOf(candidate(1, "I", isArgument = true))))),
            LocalCaptureValidationService.Result.Unavailable(point()),
        )
        assertTrue(service.complete(source, parameter(), results, "ordinal").isEmpty())
    }

    @Test
    fun returnsEmptyWhenAnySnapshotsEmpty() {
        val results = listOf(
            LocalCaptureValidationService.Result.NotFound(point(), emptyList()),
        )
        assertTrue(service.complete(source, parameter(), results, "ordinal").isEmpty())
    }

    @Test
    fun returnsEmptyWhenTargetDescriptorUnresolved() {
        val results = listOf(result(snapshots = listOf(snapshot(candidates = listOf(candidate(1, "I"))))))
        assertTrue(
            service.complete(source, parameter(typeName = "MissingType"), results, "ordinal").isEmpty(),
        )
    }

    @Test
    fun returnsEmptyForUnsupportedAttribute() {
        val results = listOf(result(snapshots = listOf(snapshot(candidates = listOf(candidate(1, "I"))))))
        assertTrue(service.complete(source, parameter(), results, "print").isEmpty())
    }

    @Test
    fun completesOrdinalValuesStableAcrossSnapshots() {
        val arg = candidate(1, "I", isArgument = true)
        val localTwo = candidate(2, "I")
        val localThree = candidate(3, "I")
        val localFour = candidate(4, "I")
        val results = listOf(
            result(
                snapshots = listOf(
                    snapshot(candidates = listOf(arg, localTwo, localThree)),
                    snapshot(candidates = listOf(arg, localTwo, localThree, localFour)),
                ),
            ),
        )

        val items = service.complete(source, parameter(), results, "ordinal")
        assertEquals(listOf("0", "1", "2"), items.map(McCompletionItem::insertText))
        assertTrue(items.all { it.kind == McCompletionKind.VALUE })
    }

    @Test
    fun completesIndexValuesPresentInEverySnapshot() {
        val arg = candidate(1, "I", isArgument = true)
        val localTwo = candidate(2, "I")
        val localThree = candidate(3, "I")
        val localFour = candidate(4, "I")
        val results = listOf(
            result(
                snapshots = listOf(
                    snapshot(candidates = listOf(arg, localTwo, localThree)),
                    snapshot(candidates = listOf(arg, localTwo, localThree, localFour)),
                ),
            ),
        )

        val items = service.complete(source, parameter(), results, "index")
        assertEquals(listOf("1", "2", "3"), items.map(McCompletionItem::insertText))
    }

    @Test
    fun completesNamesLexicallySorted() {
        val results = listOf(
            result(
                snapshots = listOf(
                    snapshot(
                        candidates = listOf(
                            candidate(1, "I", name = "zebra", isArgument = true),
                            candidate(2, "I", name = "alpha"),
                            candidate(3, "I", name = "beta"),
                        ),
                    ),
                    snapshot(
                        candidates = listOf(
                            candidate(1, "I", name = "zebra", isArgument = true),
                            candidate(2, "I", name = "alpha"),
                            candidate(3, "I", name = "beta"),
                        ),
                    ),
                ),
            ),
        )

        val items = service.complete(source, parameter(), results, "name")
        assertEquals(listOf("alpha", "beta", "zebra"), items.map(McCompletionItem::insertText))
    }

    @Test
    fun argsOnlyFiltersDerivedCandidates() {
        val arg = candidate(1, "I", name = "arg", isArgument = true)
        val local = candidate(3, "I", name = "local")
        val results = listOf(
            result(snapshots = listOf(snapshot(candidates = listOf(arg, local)))),
        )

        val items = service.complete(
            source,
            parameter(HandlerParameterSugarSpec.Local(argsOnly = true)),
            results,
            "name",
        )
        assertEquals(listOf("arg"), items.map(McCompletionItem::insertText))
    }

    @Test
    fun respectsPartialPrefixForNumericValues() {
        val results = listOf(
            result(
                snapshots = listOf(
                    snapshot(
                        candidates = listOf(
                            candidate(1, "I", isArgument = true),
                            candidate(2, "I"),
                            candidate(10, "I"),
                            candidate(11, "I"),
                        ),
                    ),
                ),
            ),
        )

        val items = service.complete(source, parameter(), results, "index", partialPrefix = "1")
        assertEquals(listOf("1", "10", "11"), items.map(McCompletionItem::insertText))
    }

    @Test
    fun respectsPartialPrefixForNames() {
        val results = listOf(
            result(
                snapshots = listOf(
                    snapshot(
                        candidates = listOf(
                            candidate(1, "I", name = "alpha", isArgument = true),
                            candidate(2, "I", name = "Alpine"),
                            candidate(3, "I", name = "beta"),
                        ),
                    ),
                ),
            ),
        )

        val items = service.complete(source, parameter(), results, "name", partialPrefix = "alp")
        assertEquals(listOf("alpha", "Alpine"), items.map(McCompletionItem::insertText))
    }

    @Test
    fun completesFirstMatchingNameAcrossSnapshots() {
        val results = listOf(
            result(
                snapshots = listOf(
                    snapshot(
                        candidates = listOf(
                            candidate(1, "I", name = "shared", isArgument = true),
                            candidate(2, "I", name = "shared"),
                        ),
                    ),
                    snapshot(
                        candidates = listOf(
                            candidate(1, "I", name = "shared", isArgument = true),
                            candidate(2, "I", name = "shared"),
                        ),
                    ),
                ),
            ),
        )

        assertEquals(listOf("shared"), service.complete(source, parameter(), results, "name").map(McCompletionItem::insertText))
    }

    private fun parameter(
        spec: HandlerParameterSugarSpec.Local = HandlerParameterSugarSpec.Local(),
        typeName: String = "int",
    ) = HandlerParameterDeclaration(
        name = "captured",
        typeName = typeName,
        typeDescriptor = null,
        isOperation = false,
        operationGenericName = null,
        isSugar = true,
        sugarSpec = spec,
    )

    private fun candidate(
        slotIndex: Int,
        descriptor: String,
        name: String? = null,
        isArgument: Boolean = false,
    ) = LocalCaptureCandidate(
        slotIndex = slotIndex,
        descriptor = descriptor,
        name = name,
        isArgument = isArgument,
    )

    private fun snapshot(candidates: List<LocalCaptureCandidate>) =
        LocalCaptureSnapshot(instructionOccurrenceIndex = 0, candidates = candidates)

    private fun point() = LocalCaptureValidationService.Point(
        owner = "com/example/Target",
        targetMethod = io.github.mcdev.core.mixin.MethodIndexEntry(
            name = "run",
            descriptor = "()V",
            isStatic = false,
            readableSignature = "run()V",
        ),
        site = MixinExtrasAnnotationSite(
            annotation = MixinExtrasAnnotation.MODIFY_EXPRESSION_VALUE,
            methodAttribute = "run",
            atValue = "MIXINEXTRAS:EXPRESSION",
            atTarget = null,
            annotationRange = McTextRange(McTextPosition(0, 0), McTextPosition(0, 1)),
            handlerMethod = null,
        ),
    )

    private fun result(
        snapshots: List<LocalCaptureSnapshot>,
    ) = LocalCaptureValidationService.Result.Resolved(
        point = point(),
        snapshots = snapshots,
        candidates = emptyList(),
    )
}
