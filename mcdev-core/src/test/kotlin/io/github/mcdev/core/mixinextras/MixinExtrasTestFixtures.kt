package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.AtTargetKind
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FakeBytecodeIndex
import io.github.mcdev.core.mixin.FakeClassIndex
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.model.MappingNamespace

object MixinExtrasTestFixtures {
    val classIndex = FakeClassIndex(
        classes = FakeClassIndex.defaultClasses() + listOf(
            ClassIndexEntry("String", "java.lang", "java/lang/String"),
            ClassIndexEntry("SimpleTarget", "com.example.target", "com/example/target/SimpleTarget"),
        ),
        methods = FakeClassIndex.defaultMethods() + mapOf(
            "java/lang/String" to listOf(
                MethodIndexEntry("length", "()I", false, "length(): int"),
            ),
            "com/example/target/SimpleTarget" to listOf(
                MethodIndexEntry("draw", "(Ljava/lang/String;FF)V", false, "draw(String, float, float): void"),
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
        fields = FakeClassIndex.defaultFields(),
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

    val MODIFY_RETURN_SOURCE = """
        @Mixin(SimpleTarget.class)
        abstract class ExampleMixin {
            @ModifyReturnValue(method = "draw(Ljava/lang/String;FF)V", at = @At("RETURN"))
            private void mcdevModifyReturn() {
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
            private boolean mcdevWrapCondition(String instance, Operation<Boolean> original) {
                return original.call(instance);
            }
        }
    """

    val WRAP_METHOD_SOURCE = """
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
