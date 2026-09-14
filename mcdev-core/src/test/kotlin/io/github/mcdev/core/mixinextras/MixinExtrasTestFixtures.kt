package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.bytecode.OccurrenceResultClassification
import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.AtTargetKind
import io.github.mcdev.core.mixin.AtTargetOperationKind
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FakeBytecodeIndex
import io.github.mcdev.core.mixin.FakeClassIndex
import io.github.mcdev.core.mixin.FieldIndexEntry
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.model.MappingNamespace

object MixinExtrasTestFixtures {
    val classIndex = FakeClassIndex(
        classes = FakeClassIndex.defaultClasses() + listOf(
            ClassIndexEntry("String", "java.lang", "java/lang/String"),
            ClassIndexEntry("SimpleTarget", "com.example.target", "com/example/target/SimpleTarget"),
            ClassIndexEntry("SharedMixinTargetA", "com.example.target", "com/example/target/SharedMixinTargetA"),
            ClassIndexEntry("SharedMixinTargetB", "com.example.target", "com/example/target/SharedMixinTargetB"),
            ClassIndexEntry("LocalRef", "com.llamalad7.mixinextras.sugar.ref", "com/llamalad7/mixinextras/sugar/ref/LocalRef"),
            ClassIndexEntry(
                "NestedLocalRef",
                "com.llamalad7.mixinextras.sugar.ref.nested",
                "com/llamalad7/mixinextras/sugar/ref/nested/NestedLocalRef",
            ),
            ClassIndexEntry(
                "CallbackInfo",
                "org.spongepowered.asm.mixin.injection.callback",
                "org/spongepowered/asm/mixin/injection/callback/CallbackInfo",
            ),
            ClassIndexEntry(
                "CallbackInfoReturnable",
                "org.spongepowered.asm.mixin.injection.callback",
                "org/spongepowered/asm/mixin/injection/callback/CallbackInfoReturnable",
            ),
        ),
        methods = FakeClassIndex.defaultMethods() + mapOf(
            "java/lang/String" to listOf(
                MethodIndexEntry("length", "()I", false, "length(): int"),
            ),
            "com/example/target/SimpleTarget" to listOf(
                MethodIndexEntry("draw", "(Ljava/lang/String;FF)V", false, "draw(String, float, float): void"),
                MethodIndexEntry("draw", "(I)V", false, "draw(int): void"),
                MethodIndexEntry("compute", "()I", false, "compute(): int"),
                MethodIndexEntry("compute", "()V", false, "compute(): void"),
                MethodIndexEntry("noop", "()V", true, "noop(): void"),
            ),
            "com/example/target/SharedMixinTargetA" to listOf(
                MethodIndexEntry("shared", "()I", false, "shared(): int"),
            ),
            "com/example/target/SharedMixinTargetB" to listOf(
                MethodIndexEntry("shared", "()I", false, "shared(): int"),
            ),
            "net/minecraft/client/font/TextRenderer" to listOf(
                MethodIndexEntry(
                    "draw",
                    "(Ljava/lang/String;FFI)I",
                    false,
                    "draw(String, float, float, int): int",
                ),
            ),
        ),
        fields = FakeClassIndex.defaultFields() + mapOf(
            "com/example/target/SimpleTarget" to listOf(
                FieldIndexEntry("label", "Ljava/lang/String;", false, "String"),
            ),
        ),
    )

    val bytecodeIndex = FakeBytecodeIndex(
        candidates = FakeBytecodeIndex.defaultCandidates() + mapOf(
            "com/example/target/SimpleTarget#draw#INVOKE" to listOf(
                AtTargetCandidate(
                    owner = "java/lang/String",
                    name = "length",
                    descriptor = "()I",
                    displayLabel = "length(): int",
                    detail = "String",
                    kind = AtTargetKind.INVOKE,
                    ordinal = 0,
                    namespace = MappingNamespace.NAMED,
                    operationKind = AtTargetOperationKind.INVOKE_VIRTUAL,
                    instructionOccurrenceIndex = 1,
                    occurrenceResultClassification = OccurrenceResultClassification.IMMEDIATELY_POPPED,
                ),
            ),
        ),
    )

    val MODIFY_EXPRESSION_SOURCE = """
        @Mixin(SimpleTarget.class)
        abstract class ExampleMixin {
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "CONSTANT", args = "floatValue=0.0"))
            private float mcdevModifyX(float original) {
                return original;
            }
        }
    """

    val MODIFY_RETURN_VOID_SOURCE = """
        @Mixin(SimpleTarget.class)
        abstract class ExampleMixin {
            @ModifyReturnValue(method = "draw(Ljava/lang/String;FF)V", at = @At("RETURN"))
            private void mcdevModifyReturn() {
            }
        }
    """

    val MODIFY_RETURN_SOURCE = """
        @Mixin(SimpleTarget.class)
        abstract class ExampleMixin {
            @ModifyReturnValue(method = "compute()I", at = @At("RETURN"))
            private int mcdevModifyReturn(int original) {
                return original;
            }
        }
    """

    val WRAP_OPERATION_SOURCE = """
        @Mixin(SimpleTarget.class)
        abstract class ExampleMixin {
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance, Operation<Integer> original) {
                return original.call(instance);
            }
        }
    """

    val WRAP_OPERATION_BAD_RETURN = """
        @Mixin(SimpleTarget.class)
        abstract class ExampleMixin {
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private void mcdevWrapLength(String instance, Operation<Integer> original) {
                original.call(instance);
            }
        }
    """

    val WRAP_OPERATION_MISSING_OP = """
        @Mixin(SimpleTarget.class)
        abstract class ExampleMixin {
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(String instance) {
                return 0;
            }
        }
    """

    val WRAP_OPERATION_OP_NOT_LAST = """
        @Mixin(SimpleTarget.class)
        abstract class ExampleMixin {
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private int mcdevWrapLength(Operation<Integer> original, String instance) {
                return original.call(instance);
            }
        }
    """

    val WRAP_WITH_CONDITION_SOURCE = """
        @Mixin(SimpleTarget.class)
        abstract class ExampleMixin {
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private boolean mcdevWrapCondition(String instance) {
                return true;
            }
        }
    """

    val WRAP_WITH_CONDITION_BAD_OPERATION = """
        @Mixin(SimpleTarget.class)
        abstract class ExampleMixin {
            @WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private boolean mcdevWrapCondition(String instance, Operation<Boolean> original) {
                return original.call(instance);
            }
        }
    """

    val WRAP_METHOD_SOURCE = """
        @Mixin(SimpleTarget.class)
        abstract class ExampleMixin {
            @WrapMethod(method = "draw(Ljava/lang/String;FF)V")
            private void mcdevWrapDraw(String arg0, float arg1, float arg2, Operation<Void> original) {
                original.call(arg0, arg1, arg2);
            }
        }
    """

    val WRAP_METHOD_BAD_RECEIVER = """
        @Mixin(SimpleTarget.class)
        abstract class ExampleMixin {
            @WrapMethod(method = "draw(Ljava/lang/String;FF)V")
            private void mcdevWrapDraw(SimpleTarget instance, String arg0, float arg1, float arg2, Operation<Void> original) {
                original.call(instance, arg0, arg1, arg2);
            }
        }
    """

    val WRAP_OPERATION_NO_HANDLER = """
        @Mixin(SimpleTarget.class)
        abstract class ExampleMixin {
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
        }
    """
}
