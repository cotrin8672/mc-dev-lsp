package io.github.mcdev.core.mixin

import kotlin.test.Test
import kotlin.test.assertEquals

class MixinMemberDeclarationParserTest {
    @Test
    fun parsesMultilineInvokerDeclaration() {
        val source = """
            import java.lang.String;

            class ExampleMixin {
                @Invoker("draw")
                public abstract void invokeDraw(
                    String text,
                    float x,
                    float y
                );
            }
        """.trimIndent()

        val declaration = MixinMemberDeclarationParser.parseInvokerDeclarations(source).single()

        assertEquals("invokeDraw", declaration.methodName)
        assertEquals("draw", declaration.explicitTargetName)
        assertEquals(listOf("Ljava/lang/String;", "F", "F"), declaration.parameterDescriptors)
        assertEquals("V", declaration.returnTypeDescriptor)
    }

    @Test
    fun parsesInvokerWithAnnotatedParameters() {
        val source = """
            import javax.annotation.Nullable;

            class ExampleMixin {
                @Invoker("draw")
                abstract void invokeDraw(@Nullable String text, float x, float y);
            }
        """.trimIndent()

        val declaration = MixinMemberDeclarationParser.parseInvokerDeclarations(source).single()

        assertEquals(listOf("Ljava/lang/String;", "F", "F"), declaration.parameterDescriptors)
    }

    @Test
    fun erasesGenericAccessorReturnType() {
        val source = """
            import java.util.List;
            import net.minecraft.item.ItemStack;

            class ExampleMixin {
                @Accessor("items")
                abstract List<ItemStack> getItems();
            }
        """.trimIndent()

        val declaration = MixinMemberDeclarationParser.parseAccessorDeclarations(source).single()

        assertEquals("Ljava/util/List;", declaration.returnTypeDescriptor)
    }

    @Test
    fun parsesShadowWithFullyQualifiedType() {
        val source = """
            class ExampleMixin {
                @Shadow
                private java.util.List<java.lang.String> names;
            }
        """.trimIndent()

        val declaration = MixinMemberDeclarationParser.parseShadowDeclarations(source).single()

        assertEquals("Ljava/util/List;", declaration.descriptor)
    }

    @Test
    fun resolvesCallbackInfoImportsAndVarargs() {
        val source = """
            import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

            class ExampleMixin {
                @Invoker("accept")
                abstract void invokeAccept(String... values, CallbackInfo ci);
            }
        """.trimIndent()

        val declaration = MixinMemberDeclarationParser.parseInvokerDeclarations(source).single()

        assertEquals(
            listOf("[Ljava/lang/String;", "Lorg/spongepowered/asm/mixin/injection/callback/CallbackInfo;"),
            declaration.parameterDescriptors,
        )
    }

    @Test
    fun resolvesImportedInnerClassAsJvmInternalName() {
        val source = """
            import com.example.Outer.Inner;

            class ExampleMixin {
                @Shadow
                private Inner value;
            }
        """.trimIndent()

        val declaration = MixinMemberDeclarationParser.parseShadowDeclarations(source).single()

        assertEquals("Lcom/example/Outer\$Inner;", declaration.descriptor)
    }
}
