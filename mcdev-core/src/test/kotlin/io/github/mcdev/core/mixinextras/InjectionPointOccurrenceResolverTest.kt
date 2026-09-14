package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.bytecode.ConstantValue
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.AtTargetKind
import io.github.mcdev.core.mixin.AtTargetOperationKind
import io.github.mcdev.core.mixin.FakeBytecodeIndex
import io.github.mcdev.core.mixin.MethodIndexEntry
import kotlin.test.Test
import org.objectweb.asm.Opcodes
import kotlin.test.assertEquals
import kotlin.test.assertNull

class InjectionPointOccurrenceResolverTest {
    private val owner = "com/example/target/SimpleTarget"
    private val targetMethod = MethodIndexEntry(
        name = "draw",
        descriptor = "(Ljava/lang/String;FF)V",
        isStatic = false,
        readableSignature = "draw(String, float, float): void",
    )
    private val annotationRange = McTextRange(
        start = McTextPosition(1, 4),
        end = McTextPosition(1, 40),
    )

    @Test
    fun invokeResolvesExactTargetOnly() {
        val lengthTarget = "Ljava/lang/String;length()I"
        val printlnTarget = "Ljava/io/PrintStream;println(Ljava/lang/String;)V"
        val resolver = resolver(
            "INVOKE" to listOf(
                invokeCandidate(
                    owner = "java/lang/String",
                    name = "length",
                    descriptor = "()I",
                    instructionOccurrenceIndex = 2,
                ),
                invokeCandidate(
                    owner = "java/io/PrintStream",
                    name = "println",
                    descriptor = "(Ljava/lang/String;)V",
                    instructionOccurrenceIndex = 5,
                ),
            ),
        )

        val lengthOnly = resolver.resolve(
            owner = owner,
            targetMethod = targetMethod,
            site = site(atValue = "INVOKE", atTarget = lengthTarget),
        )
        val printlnOnly = resolver.resolve(
            owner = owner,
            targetMethod = targetMethod,
            site = site(atValue = "INVOKE", atTarget = printlnTarget),
        )
        val missing = resolver.resolve(
            owner = owner,
            targetMethod = targetMethod,
            site = site(atValue = "INVOKE", atTarget = "Ljava/lang/Object;hashCode()I"),
        )

        assertEquals(setOf(2), lengthOnly)
        assertEquals(setOf(5), printlnOnly)
        assertEquals(emptySet(), missing)
    }

    @Test
    fun ordinalSelectsAfterExactTargetFilter() {
        val target = "Ljava/lang/String;length()I"
        val resolver = resolver(
            "INVOKE" to listOf(
                invokeCandidate(
                    owner = "java/lang/String",
                    name = "length",
                    descriptor = "()I",
                    instructionOccurrenceIndex = 1,
                ),
                invokeCandidate(
                    owner = "java/lang/String",
                    name = "length",
                    descriptor = "()I",
                    instructionOccurrenceIndex = 4,
                ),
                invokeCandidate(
                    owner = "java/lang/String",
                    name = "length",
                    descriptor = "()I",
                    instructionOccurrenceIndex = 7,
                ),
            ),
        )

        assertEquals(
            setOf(1),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(atValue = "INVOKE", atTarget = target, atOrdinal = 0),
            ),
        )
        assertEquals(
            setOf(4),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(atValue = "INVOKE", atTarget = target, atOrdinal = 1),
            ),
        )
        assertEquals(
            emptySet(),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(atValue = "INVOKE", atTarget = target, atOrdinal = 3),
            ),
        )
    }

    @Test
    fun atOrdinalSelectsBeforeAtSpecifier() {
        val target = "Ljava/lang/String;length()I"
        val resolver = resolver(
            "INVOKE" to listOf(
                invokeCandidate(
                    owner = "java/lang/String",
                    name = "length",
                    descriptor = "()I",
                    instructionOccurrenceIndex = 7,
                ),
                invokeCandidate(
                    owner = "java/io/PrintStream",
                    name = "println",
                    descriptor = "(Ljava/lang/String;)V",
                    instructionOccurrenceIndex = 2,
                ),
                invokeCandidate(
                    owner = "java/lang/String",
                    name = "length",
                    descriptor = "()I",
                    instructionOccurrenceIndex = 3,
                ),
            ),
        )

        assertEquals(
            setOf(3),
            resolver.resolve(owner, targetMethod, site(atValue = "INVOKE:FIRST", atTarget = target)),
        )
        assertEquals(
            setOf(7),
            resolver.resolve(owner, targetMethod, site(atValue = "INVOKE:LAST", atTarget = target)),
        )
        assertEquals(
            setOf(3, 7),
            resolver.resolve(owner, targetMethod, site(atValue = "INVOKE:DEFAULT", atTarget = target)),
        )
        assertEquals(
            setOf(3, 7),
            resolver.resolve(owner, targetMethod, site(atValue = "INVOKE:ALL", atTarget = target)),
        )
        assertNull(
            resolver.resolve(owner, targetMethod, site(atValue = "INVOKE:ONE", atTarget = target)),
        )
        assertEquals(
            setOf(3),
            resolver.resolve(
                owner,
                targetMethod,
                site(atValue = "INVOKE:FIRST", atTarget = target, atOrdinal = 0),
            ),
        )
        assertEquals(
            setOf(7),
            resolver.resolve(
                owner,
                targetMethod,
                site(atValue = "INVOKE:FIRST", atTarget = target, atOrdinal = 1),
            ),
        )
    }

    @Test
    fun atSpecifierOneRequiresExactlyOneBaseMatchAndMalformedFailsClosed() {
        val target = "Ljava/lang/String;length()I"
        val resolver = resolver(
            "INVOKE" to listOf(
                invokeCandidate(
                    owner = "java/lang/String",
                    name = "length",
                    descriptor = "()I",
                    instructionOccurrenceIndex = 4,
                ),
            ),
        )

        assertEquals(
            setOf(4),
            resolver.resolve(owner, targetMethod, site(atValue = "INVOKE:ONE", atTarget = target)),
        )
        assertNull(
            resolver.resolve(owner, targetMethod, site(atValue = "INVOKE:ONE", atTarget = "*")),
        )
        assertNull(
            resolver.resolve(owner, targetMethod, site(atValue = "INVOKE:UNKNOWN", atTarget = target)),
        )
        assertNull(
            resolver.resolve(owner, targetMethod, site(atValue = "INVOKE:last", atTarget = target)),
        )
        assertNull(
            resolver.resolve(owner, targetMethod, site(atValue = "INVOKE:", atTarget = target)),
        )
        assertNull(
            resolver.resolve(owner, targetMethod, site(atValue = "INVOKE:FIRST:LAST", atTarget = target)),
        )
    }

    @Test
    fun emptyConstantArgsFilterByHandlerReturnType() {
        val resolver = resolver(
            "CONSTANT" to listOf(
                constantCandidate(ConstantValue.IntValue(0), instructionOccurrenceIndex = 1),
                constantCandidate(ConstantValue.IntValue(42), instructionOccurrenceIndex = 3),
                constantCandidate(ConstantValue.StringValue("hello"), instructionOccurrenceIndex = 6),
            ),
        )

        assertEquals(
            setOf(1, 3),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(
                    atValue = "CONSTANT",
                    atArgs = emptyList(),
                    handlerMethod = handlerMethod(returnTypeDescriptor = "I"),
                ),
            ),
        )
        assertEquals(
            setOf(6),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(
                    atValue = "CONSTANT",
                    atArgs = emptyList(),
                    handlerMethod = handlerMethod(returnTypeDescriptor = "Ljava/lang/String;"),
                ),
            ),
        )
        assertNull(
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(
                    atValue = "CONSTANT",
                    atArgs = emptyList(),
                ),
            ),
        )
    }

    @Test
    fun emptyConstantArgsFilterNullByObjectHandlerReturnTypeOnly() {
        val resolver = resolver(
            "CONSTANT" to listOf(
                constantCandidate(ConstantValue.NullValue, instructionOccurrenceIndex = 2),
                constantCandidate(ConstantValue.IntValue(0), instructionOccurrenceIndex = 5),
            ),
        )

        assertEquals(
            setOf(2),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(
                    atValue = "CONSTANT",
                    atArgs = emptyList(),
                    handlerMethod = handlerMethod(returnTypeDescriptor = "Ljava/lang/Object;"),
                ),
            ),
        )
        assertEquals(
            emptySet(),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(
                    atValue = "CONSTANT",
                    atArgs = emptyList(),
                    handlerMethod = handlerMethod(returnTypeDescriptor = "Ljava/lang/String;"),
                ),
            ),
        )
        assertEquals(
            emptySet(),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(
                    atValue = "CONSTANT",
                    atArgs = emptyList(),
                    handlerMethod = handlerMethod(returnTypeDescriptor = "[I"),
                ),
            ),
        )
    }

    @Test
    fun explicitVoidClassSelectorMatchesNoConstants() {
        val resolver = resolver(
            "CONSTANT" to listOf(
                constantCandidate(ConstantValue.ClassLiteral("java/lang/String"), instructionOccurrenceIndex = 2),
            ),
        )

        assertEquals(
            emptySet(),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(
                    atValue = "CONSTANT",
                    atArgs = listOf("classValue=void"),
                    handlerMethod = handlerMethod(returnTypeDescriptor = "Ljava/lang/Class;"),
                ),
            ),
        )
    }

    @Test
    fun constantOrdinalSelectsAfterHandlerTypeFilter() {
        val resolver = resolver(
            "CONSTANT" to listOf(
                constantCandidate(ConstantValue.IntValue(0), instructionOccurrenceIndex = 1),
                constantCandidate(ConstantValue.IntValue(42), instructionOccurrenceIndex = 3),
                constantCandidate(ConstantValue.StringValue("hello"), instructionOccurrenceIndex = 6),
            ),
        )

        assertEquals(
            setOf(3),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(
                    atValue = "CONSTANT",
                    atArgs = emptyList(),
                    atOrdinal = 1,
                    handlerMethod = handlerMethod(returnTypeDescriptor = "I"),
                ),
            ),
        )
    }

    @Test
    fun constantMatchesExactValueOnly() {
        val resolver = resolver(
            "CONSTANT" to listOf(
                constantCandidate(ConstantValue.IntValue(0), instructionOccurrenceIndex = 1),
                constantCandidate(ConstantValue.IntValue(42), instructionOccurrenceIndex = 3),
                constantCandidate(ConstantValue.IntValue(42), instructionOccurrenceIndex = 6),
            ),
        )

        assertEquals(
            setOf(3, 6),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(
                    atValue = "CONSTANT",
                    atArgs = listOf("intValue=42"),
                ),
            ),
        )
        assertEquals(
            setOf(1),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(
                    atValue = "CONSTANT",
                    atArgs = listOf("intValue=0"),
                ),
            ),
        )
        assertEquals(
            emptySet(),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(
                    atValue = "CONSTANT",
                    atArgs = listOf("intValue=99"),
                ),
            ),
        )
    }

    @Test
    fun expandZeroConditionsFilterLiteralAndConditionCandidates() {
        val resolver = resolver(
            "CONSTANT" to listOf(
                constantCandidate(ConstantValue.IntValue(0), instructionOccurrenceIndex = 1),
                constantCandidate(ConstantValue.IntValue(1), instructionOccurrenceIndex = 2),
                conditionCandidate(Opcodes.IFLT, instructionOccurrenceIndex = 3),
                conditionCandidate(Opcodes.IFGE, instructionOccurrenceIndex = 5),
                conditionCandidate(Opcodes.IFGT, instructionOccurrenceIndex = 7),
                conditionCandidate(Opcodes.IFLE, instructionOccurrenceIndex = 9),
            ),
        )

        assertEquals(
            setOf(3, 5),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(atValue = "CONSTANT", expandZeroConditions = setOf(Opcodes.IFLT, Opcodes.IFGE)),
            ),
        )
        assertEquals(
            setOf(1, 3, 5),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(
                    atValue = "CONSTANT",
                    atArgs = listOf("intValue=0"),
                    expandZeroConditions = setOf(Opcodes.IFLT, Opcodes.IFGE),
                ),
            ),
        )
        assertEquals(
            setOf(3, 5),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(
                    atValue = "CONSTANT",
                    atArgs = listOf("intValue=1"),
                    expandZeroConditions = setOf(Opcodes.IFLT, Opcodes.IFGE),
                ),
            ),
        )
        assertNull(
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(
                    atValue = "CONSTANT",
                    atArgs = listOf("floatValue=0.0"),
                    expandZeroConditions = setOf(Opcodes.IFLT, Opcodes.IFGE),
                ),
            ),
        )
        assertEquals(
            setOf(1),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(atValue = "CONSTANT", atArgs = listOf("intValue=0")),
            ),
        )
        assertEquals(
            setOf(7, 9),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(atValue = "CONSTANT", expandZeroConditions = setOf(Opcodes.IFGT, Opcodes.IFLE)),
            ),
        )
        assertEquals(
            setOf(3),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(
                    atValue = "CONSTANT",
                    atArgs = listOf("intValue=0"),
                    atOrdinal = 1,
                    expandZeroConditions = setOf(Opcodes.IFLT, Opcodes.IFGE),
                ),
            ),
        )
    }

    @Test
    fun returnResolvesReturnOccurrenceIndices() {
        val resolver = resolver(
            "RETURN" to listOf(
                returnCandidate(instructionOccurrenceIndex = 8),
                returnCandidate(instructionOccurrenceIndex = 11),
            ),
        )

        assertEquals(
            setOf(8, 11),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(atValue = "RETURN", atTarget = "RETURN"),
            ),
        )
    }

    @Test
    fun expressionUsesSuppliedIndices() {
        val resolver = resolver(emptyMap())

        assertNull(
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(atValue = "MIXINEXTRAS:EXPRESSION"),
            ),
        )
        assertEquals(
            setOf(2, 5),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(atValue = "MIXINEXTRAS:EXPRESSION"),
                expressionInstructionIndices = setOf(2, 5),
            ),
        )
    }

    @Test
    fun afterShiftMovesOccurrenceIndexForward() {
        val resolver = resolver(
            "INVOKE" to listOf(
                invokeCandidate(
                    owner = "java/lang/String",
                    name = "length",
                    descriptor = "()I",
                    instructionOccurrenceIndex = 3,
                ),
            ),
        )

        assertEquals(
            setOf(4),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(
                    atValue = "INVOKE",
                    atTarget = "Ljava/lang/String;length()I",
                    atShift = AtShiftSpec.After,
                ),
            ),
        )
    }

    @Test
    fun byShiftAppliesOffset() {
        val resolver = resolver(
            "RETURN" to listOf(
                returnCandidate(instructionOccurrenceIndex = 5),
            ),
        )

        assertEquals(
            setOf(7),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(
                    atValue = "RETURN",
                    atTarget = "RETURN",
                    atShift = AtShiftSpec.By(2),
                ),
            ),
        )
    }

    @Test
    fun malformedConstantArgsFailClosed() {
        val resolver = resolver(
            "CONSTANT" to listOf(
                constantCandidate(ConstantValue.IntValue(1), instructionOccurrenceIndex = 0),
            ),
        )

        assertNull(
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(
                    atValue = "CONSTANT",
                    atArgs = listOf("intValue=1", "floatValue=1.0"),
                ),
            ),
        )
        assertNull(
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(
                    atValue = "CONSTANT",
                    atArgs = listOf("intValue=abc"),
                ),
            ),
        )
    }

    @Test
    fun unresolvedShiftFailsClosed() {
        val resolver = resolver(
            "RETURN" to listOf(
                returnCandidate(instructionOccurrenceIndex = 2),
            ),
        )

        assertNull(
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(
                    atValue = "RETURN",
                    atTarget = "RETURN",
                    atShift = AtShiftSpec.Unresolved,
                ),
            ),
        )
    }

    @Test
    fun negativeShiftedIndexFailsClosed() {
        val resolver = resolver(
            "RETURN" to listOf(
                returnCandidate(instructionOccurrenceIndex = 1),
                returnCandidate(instructionOccurrenceIndex = 0),
            ),
        )

        assertNull(
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(
                    atValue = "RETURN",
                    atTarget = "RETURN",
                    atShift = AtShiftSpec.By(-2),
                    atOrdinal = 0,
                ),
            ),
        )
        assertNull(
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(
                    atValue = "RETURN",
                    atTarget = "RETURN",
                    atShift = AtShiftSpec.By(-1),
                    atOrdinal = 0,
                ),
            ),
        )
    }

    @Test
    fun ordinalWithNegativeInstructionOccurrenceIndexFailsClosed() {
        val resolver = resolver(
            "FIELD" to listOf(
                fieldCandidate(
                    instructionOccurrenceIndex = -1,
                ),
            ),
        )

        assertNull(
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(
                    atValue = "FIELD",
                    atTarget = "Lcom/example/target/SimpleTarget;label:Ljava/lang/String;",
                    atOrdinal = 0,
                ),
            ),
        )
    }

    @Test
    fun broadSelectorFailsClosed() {
        val resolver = resolver(
            "INVOKE" to listOf(
                invokeCandidate(
                    owner = "java/lang/String",
                    name = "length",
                    descriptor = "()I",
                    instructionOccurrenceIndex = 0,
                ),
            ),
        )

        assertNull(
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(atValue = "INVOKE", atTarget = "*"),
            ),
        )
    }

    @Test
    fun unsupportedAtValueReturnsNull() {
        val resolver = resolver(emptyMap())

        assertNull(
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(atValue = "LOAD"),
            ),
        )
    }

    @Test
    fun headResolvesToFirstInstructionIndex() {
        val resolver = resolver(emptyMap())

        assertEquals(
            setOf(0),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(atValue = "HEAD"),
            ),
        )
    }

    @Test
    fun shiftedHeadAppliesShift() {
        val resolver = resolver(emptyMap())

        assertEquals(
            setOf(1),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(atValue = "HEAD", atShift = AtShiftSpec.After),
            ),
        )
        assertEquals(
            setOf(3),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(atValue = "HEAD", atShift = AtShiftSpec.By(3)),
            ),
        )
    }

    @Test
    fun tailSelectsFinalReturnFromMultipleReturns() {
        val resolver = resolver(
            "RETURN" to listOf(
                returnCandidate(instructionOccurrenceIndex = 3),
                returnCandidate(instructionOccurrenceIndex = 11),
                returnCandidate(instructionOccurrenceIndex = 7),
            ),
        )

        assertEquals(
            setOf(11),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(atValue = "TAIL"),
            ),
        )
        assertEquals(
            setOf(12),
            resolver.resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(atValue = "TAIL", atShift = AtShiftSpec.After),
            ),
        )
    }

    @Test
    fun tailWithMissingOrInvalidReturnDataFailsClosed() {
        assertNull(
            resolver(emptyMap()).resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(atValue = "TAIL"),
            ),
        )
        assertNull(
            resolver(
                "RETURN" to listOf(
                    returnCandidate(instructionOccurrenceIndex = -1),
                ),
            ).resolve(
                owner = owner,
                targetMethod = targetMethod,
                site = site(atValue = "TAIL"),
            ),
        )
    }

    @Test
    fun deduplicatesAndSortsThroughSetSemantics() {
        val resolver = resolver(
            "RETURN" to listOf(
                returnCandidate(instructionOccurrenceIndex = 5),
                returnCandidate(instructionOccurrenceIndex = 5),
                returnCandidate(instructionOccurrenceIndex = 2),
            ),
        )

        val resolved = resolver.resolve(
            owner = owner,
            targetMethod = targetMethod,
            site = site(atValue = "RETURN", atTarget = "RETURN"),
        )

        assertEquals(setOf(2, 5), resolved)
    }

    private fun resolver(vararg entries: Pair<String, List<AtTargetCandidate>>): InjectionPointOccurrenceResolver =
        InjectionPointOccurrenceResolver(
            FakeBytecodeIndex(
                candidates = entries.associate { (atValue, candidates) ->
                    "$owner#${targetMethod.name}#$atValue" to candidates
                },
            ),
        )

    private fun resolver(candidates: Map<String, List<AtTargetCandidate>>): InjectionPointOccurrenceResolver =
        InjectionPointOccurrenceResolver(FakeBytecodeIndex(candidates = candidates))

    private fun site(
        atValue: String,
        atTarget: String? = null,
        atArgs: List<String> = emptyList(),
        atOrdinal: Int? = null,
        atShift: AtShiftSpec = AtShiftSpec.Before,
        handlerMethod: HandlerMethodDeclaration? = null,
        expandZeroConditions: Set<Int> = emptySet(),
    ): MixinExtrasAnnotationSite = MixinExtrasAnnotationSite(
        annotation = MixinExtrasAnnotation.MODIFY_EXPRESSION_VALUE,
        methodAttribute = targetMethod.descriptor,
        atValue = atValue,
        atTarget = atTarget,
        atArgs = atArgs,
        atOrdinal = atOrdinal,
        atShift = atShift,
        annotationRange = annotationRange,
        handlerMethod = handlerMethod,
        expandZeroConditions = expandZeroConditions,
    )

    private fun handlerMethod(returnTypeDescriptor: String): HandlerMethodDeclaration =
        HandlerMethodDeclaration(
            methodName = "mcdevHandler",
            returnTypeName = "handlerReturn",
            returnTypeDescriptor = returnTypeDescriptor,
            parameters = emptyList(),
            range = annotationRange,
        )

    private fun invokeCandidate(
        owner: String,
        name: String,
        descriptor: String,
        instructionOccurrenceIndex: Int,
    ): AtTargetCandidate = AtTargetCandidate(
        owner = owner,
        name = name,
        descriptor = descriptor,
        displayLabel = "$name(): type",
        detail = owner.substringAfterLast('/'),
        kind = AtTargetKind.INVOKE,
        operationKind = AtTargetOperationKind.INVOKE_VIRTUAL,
        instructionOccurrenceIndex = instructionOccurrenceIndex,
    )

    private fun fieldCandidate(
        instructionOccurrenceIndex: Int,
        owner: String = "com/example/target/SimpleTarget",
        name: String = "label",
        descriptor: String = "Ljava/lang/String;",
    ): AtTargetCandidate = AtTargetCandidate(
        owner = owner,
        name = name,
        descriptor = descriptor,
        displayLabel = "$name: type",
        detail = "SimpleTarget",
        kind = AtTargetKind.FIELD,
        operationKind = AtTargetOperationKind.FIELD_GET_INSTANCE,
        instructionOccurrenceIndex = instructionOccurrenceIndex,
    )

    private fun constantCandidate(
        constantValue: ConstantValue,
        instructionOccurrenceIndex: Int,
    ): AtTargetCandidate = AtTargetCandidate(
        owner = "",
        name = "",
        descriptor = "",
        displayLabel = when (constantValue) {
            is ConstantValue.IntValue -> "ICONST_${constantValue.value}"
            else -> constantValue.toString()
        },
        detail = "",
        kind = AtTargetKind.CONSTANT,
        constantValue = constantValue,
        instructionOccurrenceIndex = instructionOccurrenceIndex,
    )

    private fun conditionCandidate(
        conditionOpcode: Int,
        instructionOccurrenceIndex: Int,
    ): AtTargetCandidate = AtTargetCandidate(
        owner = "",
        name = "",
        descriptor = "",
        displayLabel = "condition",
        detail = "",
        kind = AtTargetKind.CONSTANT,
        conditionOpcode = conditionOpcode,
        instructionOccurrenceIndex = instructionOccurrenceIndex,
    )

    private fun returnCandidate(instructionOccurrenceIndex: Int): AtTargetCandidate = AtTargetCandidate(
        owner = "",
        name = "RETURN",
        descriptor = "",
        displayLabel = "RETURN",
        detail = "",
        kind = AtTargetKind.RETURN,
        instructionOccurrenceIndex = instructionOccurrenceIndex,
    )
}
