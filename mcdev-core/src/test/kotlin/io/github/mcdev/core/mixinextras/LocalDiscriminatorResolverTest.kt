package io.github.mcdev.core.mixinextras

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs

class LocalDiscriminatorResolverTest {
    private fun candidate(
        slotIndex: Int,
        descriptor: String,
        name: String? = null,
        isArgument: Boolean = false,
        ordinal: Int? = null,
    ) = LocalCaptureCandidate(
        slotIndex = slotIndex,
        descriptor = descriptor,
        name = name,
        isArgument = isArgument,
        ordinal = ordinal,
    )

    private fun resolve(
        spec: HandlerParameterSugarSpec.Local,
        targetDescriptor: String,
        vararg candidates: LocalCaptureCandidate,
    ): LocalDiscriminatorResolution =
        LocalDiscriminatorResolver.resolve(spec, targetDescriptor, candidates.toList())

    @Test
    fun implicitResolvesSingleMatchingCandidate() {
        val only = candidate(1, "I", name = "count", isArgument = true)
        val result = resolve(HandlerParameterSugarSpec.Local(), "I", only)
        assertEquals(LocalDiscriminatorResolution.Resolved(only), result)
    }

    @Test
    fun implicitNotFoundWhenNoDescriptorMatch() {
        val result = resolve(
            HandlerParameterSugarSpec.Local(),
            "I",
            candidate(1, "J", isArgument = true),
        )
        assertEquals(LocalDiscriminatorResolution.NotFound, result)
    }

    @Test
    fun implicitAmbiguousWhenMultipleMatchingCandidates() {
        val first = candidate(1, "I", isArgument = true)
        val second = candidate(3, "I", isArgument = false)
        val result = resolve(HandlerParameterSugarSpec.Local(), "I", first, second)
        assertEquals(
            LocalDiscriminatorResolution.Ambiguous(listOf(first, second)),
            result,
        )
    }

    @Test
    fun ordinalSelectsZeroBasedCandidateInStableSlotOrder() {
        val slotThree = candidate(3, "I", name = "late")
        val slotOne = candidate(1, "I", name = "early", isArgument = true)
        val slotFive = candidate(5, "I", name = "last")
        val result = resolve(
            HandlerParameterSugarSpec.Local(ordinal = 1),
            "I",
            slotThree,
            slotOne,
            slotFive,
        )
        assertEquals(LocalDiscriminatorResolution.Resolved(slotThree), result)
    }

    @Test
    fun ordinalUsesReceiverCountButCannotSelectReceiver() {
        val local = candidate(1, "LExample;", ordinal = 1)

        assertEquals(
            LocalDiscriminatorResolution.NotFound,
            resolve(HandlerParameterSugarSpec.Local(ordinal = 0), "LExample;", local),
        )
        assertEquals(
            LocalDiscriminatorResolution.Resolved(local),
            resolve(HandlerParameterSugarSpec.Local(ordinal = 1), "LExample;", local),
        )
    }

    @Test
    fun ordinalMissReturnsNotFound() {
        val result = resolve(
            HandlerParameterSugarSpec.Local(ordinal = 2),
            "I",
            candidate(1, "I", isArgument = true),
            candidate(3, "I"),
        )
        assertEquals(LocalDiscriminatorResolution.NotFound, result)
    }

    @Test
    fun negativeOrdinalFallsThroughToImplicit() {
        val first = candidate(1, "I", isArgument = true)
        val second = candidate(3, "I")
        val result = resolve(
            HandlerParameterSugarSpec.Local(ordinal = -1),
            "I",
            first,
            second,
        )
        assertEquals(LocalDiscriminatorResolution.Ambiguous(listOf(first, second)), result)
    }

    @Test
    fun indexMatchesExactJvmSlot() {
        val target = candidate(16, "J", name = "wideCounter", isArgument = true)
        val result = resolve(
            HandlerParameterSugarSpec.Local(index = 16),
            "J",
            candidate(14, "I", isArgument = true),
            target,
        )
        assertEquals(LocalDiscriminatorResolution.Resolved(target), result)
    }

    @Test
    fun indexNotFoundWhenSlotHasDifferentDescriptor() {
        val result = resolve(
            HandlerParameterSugarSpec.Local(index = 2),
            "I",
            candidate(2, "J", isArgument = true),
        )
        assertEquals(LocalDiscriminatorResolution.NotFound, result)
    }

    @Test
    fun negativeIndexFallsThroughToImplicit() {
        val only = candidate(4, "D", isArgument = true)
        val result = resolve(
            HandlerParameterSugarSpec.Local(index = -1),
            "D",
            only,
        )
        assertEquals(LocalDiscriminatorResolution.Resolved(only), result)
    }

    @Test
    fun namesResolveSingleMatch() {
        val target = candidate(2, "Ljava/lang/String;", name = "message", isArgument = true)
        val result = resolve(
            HandlerParameterSugarSpec.Local(names = setOf("message")),
            "Ljava/lang/String;",
            candidate(0, "Ljava/lang/String;", name = "other", isArgument = true),
            target,
        )
        assertEquals(LocalDiscriminatorResolution.Resolved(target), result)
    }

    @Test
    fun namesNotFoundWhenNoNameMatches() {
        val result = resolve(
            HandlerParameterSugarSpec.Local(names = setOf("missing")),
            "I",
            candidate(1, "I", name = "count", isArgument = true),
        )
        assertEquals(LocalDiscriminatorResolution.NotFound, result)
    }

    @Test
    fun namesSelectFirstMatchingSlotWhenMultipleCandidatesMatch() {
        val alpha = candidate(1, "I", name = "alpha", isArgument = true)
        val beta = candidate(3, "I", name = "beta")
        val forward = resolve(
            HandlerParameterSugarSpec.Local(names = setOf("alpha", "beta")),
            "I",
            alpha,
            beta,
        )
        val reversed = resolve(
            HandlerParameterSugarSpec.Local(names = setOf("alpha", "beta")),
            "I",
            beta,
            alpha,
        )
        assertEquals(LocalDiscriminatorResolution.Resolved(alpha), forward)
        assertEquals(forward, reversed)
    }

    @Test
    fun emptyNamesFallThroughToImplicit() {
        val only = candidate(0, "Z", isArgument = true)
        val result = resolve(HandlerParameterSugarSpec.Local(names = emptySet()), "Z", only)
        assertEquals(LocalDiscriminatorResolution.Resolved(only), result)
    }

    @Test
    fun argsOnlyFiltersOutNonArgumentLocals() {
        val arg = candidate(1, "I", name = "arg", isArgument = true)
        val result = resolve(
            HandlerParameterSugarSpec.Local(argsOnly = true),
            "I",
            candidate(3, "I", name = "local"),
            arg,
        )
        assertEquals(LocalDiscriminatorResolution.Resolved(arg), result)
    }

    @Test
    fun argsOnlyImplicitAmbiguousUsesOnlyArguments() {
        val firstArg = candidate(1, "I", isArgument = true)
        val secondArg = candidate(2, "I", isArgument = true)
        val result = resolve(
            HandlerParameterSugarSpec.Local(argsOnly = true),
            "I",
            candidate(5, "I"),
            secondArg,
            firstArg,
        )
        assertEquals(
            LocalDiscriminatorResolution.Ambiguous(listOf(firstArg, secondArg)),
            result,
        )
    }

    @Test
    fun ordinalPrecedenceOverIndexAndNames() {
        val ordinalTarget = candidate(3, "I", name = "picked")
        val result = resolve(
            HandlerParameterSugarSpec.Local(
                ordinal = 0,
                index = 99,
                names = setOf("ignored"),
            ),
            "I",
            ordinalTarget,
            candidate(7, "I", name = "ignored", isArgument = true),
        )
        assertEquals(LocalDiscriminatorResolution.Resolved(ordinalTarget), result)
    }

    @Test
    fun indexPrecedenceOverNamesWhenOrdinalUnspecified() {
        val indexTarget = candidate(4, "F", name = "shared", isArgument = true)
        val result = resolve(
            HandlerParameterSugarSpec.Local(index = 4, names = setOf("shared")),
            "F",
            indexTarget,
            candidate(6, "F", name = "shared"),
        )
        assertEquals(LocalDiscriminatorResolution.Resolved(indexTarget), result)
    }

    @Test
    fun printDoesNotChangeSelection() {
        val withoutPrint = resolve(
            HandlerParameterSugarSpec.Local(),
            "I",
            candidate(1, "I", isArgument = true),
        )
        val withPrint = resolve(
            HandlerParameterSugarSpec.Local(print = true),
            "I",
            candidate(1, "I", isArgument = true),
        )
        assertEquals(withoutPrint, withPrint)
        assertIs<LocalDiscriminatorResolution.Resolved>(withPrint)
    }

    @Test
    fun stableOrderingDoesNotDependOnInputCandidateOrder() {
        val slotOne = candidate(1, "I", isArgument = true)
        val slotThree = candidate(3, "I")
        val forward = resolve(
            HandlerParameterSugarSpec.Local(ordinal = 0),
            "I",
            slotOne,
            slotThree,
        )
        val reversed = resolve(
            HandlerParameterSugarSpec.Local(ordinal = 0),
            "I",
            slotThree,
            slotOne,
        )
        assertEquals(LocalDiscriminatorResolution.Resolved(slotOne), forward)
        assertEquals(forward, reversed)
    }
}
