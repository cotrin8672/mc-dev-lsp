package io.github.mcdev.core.mixin

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

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

    @Test
    fun resolvesSamePackageTypeThroughClassIndex() {
        val source = """
            package com.example.mixin;

            class ExampleMixin {
                @Invoker("foo")
                abstract void invokeFoo(LocalValue value);
            }
        """.trimIndent()
        val classIndex = FakeClassIndex(
            classes = FakeClassIndex.defaultClasses() + ClassIndexEntry(
                "LocalValue",
                "com.example.mixin",
                "com/example/mixin/LocalValue",
            ),
        )

        val declaration = MixinMemberDeclarationParser.parseInvokerDeclarations(source, classIndex).single()

        assertEquals(listOf("Lcom/example/mixin/LocalValue;"), declaration.parameterDescriptors)
    }

    @Test
    fun resolvesWildcardImportThroughClassIndex() {
        val source = """
            import com.example.items.*;

            class ExampleMixin {
                @Invoker("foo")
                abstract void invokeFoo(ItemStack stack);
            }
        """.trimIndent()
        val classIndex = FakeClassIndex(
            classes = FakeClassIndex.defaultClasses() + ClassIndexEntry(
                "ItemStack",
                "com.example.items",
                "com/example/items/ItemStack",
            ),
        )

        val declaration = MixinMemberDeclarationParser.parseInvokerDeclarations(source, classIndex).single()

        assertEquals(listOf("Lcom/example/items/ItemStack;"), declaration.parameterDescriptors)
    }

    @Test
    fun treatsAmbiguousWildcardImportAsUnresolved() {
        val source = """
            import com.example.alpha.*;
            import com.example.beta.*;

            class ExampleMixin {
                @Invoker("foo")
                abstract void invokeFoo(Widget widget);
            }
        """.trimIndent()
        val classIndex = FakeClassIndex(
            classes = FakeClassIndex.defaultClasses() + listOf(
                ClassIndexEntry("Widget", "com.example.alpha", "com/example/alpha/Widget"),
                ClassIndexEntry("Widget", "com.example.beta", "com/example/beta/Widget"),
            ),
        )

        val declarations = MixinMemberDeclarationParser.parseInvokerDeclarations(source, classIndex)
        val diagnostics = MixinMemberDeclarationParser.parseDeclarationDiagnostics(source, classIndex)

        assertTrue(declarations.isEmpty())
        assertTrue(diagnostics.any { it.code == MixinDiagnosticCodes.UNRESOLVED_HANDLER_DESCRIPTOR })
    }

    @Test
    fun doesNotCreateFakeDescriptorForUnknownSimpleType() {
        val source = """
            package com.example.mixin;

            class ExampleMixin {
                @Invoker("foo")
                abstract void invokeFoo(UnknownType value);
            }
        """.trimIndent()

        val declarations = MixinMemberDeclarationParser.parseInvokerDeclarations(source, FakeClassIndex())
        val diagnostics = MixinMemberDeclarationParser.parseDeclarationDiagnostics(source, FakeClassIndex())

        assertTrue(declarations.isEmpty())
        assertTrue(diagnostics.any {
            it.code == MixinDiagnosticCodes.UNRESOLVED_JAVA_TYPE &&
                it.metadata["normalizedType"] == "UnknownType"
        })
    }

    @Test
    fun recordsHandWrittenParserMetadata() {
        val source = """
            class ExampleMixin {
                @Invoker("foo")
                abstract void invokeFoo(String value);
            }
        """.trimIndent()

        val declaration = MixinMemberDeclarationParser.parseInvokerDeclarations(source).single()

        assertEquals(ParseSource.HAND_WRITTEN, declaration.parseSource)
        assertEquals(ParseConfidence.HIGH, declaration.confidence)
        assertTrue(declaration.warnings.isEmpty())
    }
}
