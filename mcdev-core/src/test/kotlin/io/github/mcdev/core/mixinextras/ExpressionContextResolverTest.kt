package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.AtTargetKind
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FakeBytecodeIndex
import io.github.mcdev.core.mixin.FakeClassIndex
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.model.MappingNamespace
import kotlin.test.Test
import kotlin.test.assertEquals

class ExpressionContextResolverTest {
    @Test
    fun infersInvokeTypeFromFullyQualifiedReceiver() {
        val inferred = ExpressionContextResolver.inferFromExpression(
            expression = "java.lang.String.length()",
            ownerInternalName = "com/example/target/SimpleTarget",
            targetMethod = MethodIndexEntry("draw", "(Ljava/lang/String;FF)V", false, "draw(String, float, float): void"),
            bytecodeIndex = MixinExtrasTestFixtures.bytecodeIndex,
            classIndex = MixinExtrasTestFixtures.classIndex,
        )

        assertEquals("I", inferred)
    }

    @Test
    fun fullyQualifiedReceiverDisambiguatesSameNamedInvokes() {
        val classIndex = FakeClassIndex(
            classes = FakeClassIndex.defaultClasses() + listOf(
                ClassIndexEntry("First", "com.example.target", "com/example/target/First"),
                ClassIndexEntry("Second", "com.example.target", "com/example/target/Second"),
            ),
        )
        val bytecodeIndex = FakeBytecodeIndex(
            candidates = mapOf(
                "com/example/target/SimpleTarget#draw#INVOKE" to listOf(
                    AtTargetCandidate(
                        owner = "com/example/target/First",
                        name = "value",
                        descriptor = "()F",
                        displayLabel = "value(): float",
                        detail = "First",
                        kind = AtTargetKind.INVOKE,
                        namespace = MappingNamespace.NAMED,
                    ),
                    AtTargetCandidate(
                        owner = "com/example/target/Second",
                        name = "value",
                        descriptor = "()I",
                        displayLabel = "value(): int",
                        detail = "Second",
                        kind = AtTargetKind.INVOKE,
                        namespace = MappingNamespace.NAMED,
                    ),
                ),
            ),
        )

        val inferred = ExpressionContextResolver.inferFromExpression(
            expression = "com.example.target.Second.value()",
            ownerInternalName = "com/example/target/SimpleTarget",
            targetMethod = MethodIndexEntry("draw", "()V", false, "draw(): void"),
            bytecodeIndex = bytecodeIndex,
            classIndex = classIndex,
        )

        assertEquals("I", inferred)
    }
}
