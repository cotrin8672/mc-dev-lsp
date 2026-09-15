package io.github.mcdev.core.mixin

import io.github.mcdev.core.completion.McCompletionInsertTextFormat
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class MixinDeclarationSnippetServiceTest {
    private val service = MixinDeclarationSnippetService(FakeClassIndex())
    private val owner = "net/minecraft/client/MinecraftClient"

    @Test
    fun explicitAccessorOffersGetterAndSetterWithWholeEmptyNamePlaceholder() {
        val source = source("@Accessor(\"currentScreen\")")
        val items = complete(source, MixinAnnotation.ACCESSOR)

        assertEquals(2, items.size)
        assertTrue(items.any { it.label.contains("getter") && it.insertText == "public abstract Screen \${1}();\$0" })
        assertTrue(items.any { it.label.contains("setter") && it.insertText == "public abstract void \${1}(Screen value);\$0" })
        assertTrue(items.all { it.insertTextFormat == McCompletionInsertTextFormat.SNIPPET })
        assertTrue(items.all { item -> item.additionalEdits.any { edit -> edit.newText.contains("import net.minecraft.client.gui.screen.Screen;") } })
    }

    @Test
    fun staticAccessorUsesStaticBodyInsteadOfInvalidAbstractMethod() {
        val field = FieldIndexEntry("LOGGER", "Ljava/lang/String;", true, "String")
        val index = FakeClassIndex(
            methods = FakeClassIndex.defaultMethods(),
            fields = mapOf(owner to listOf(field)),
        )
        val source = source("@Accessor(\"LOGGER\")")
        val items = MixinDeclarationSnippetService(index).complete(
            source,
            MixinAnnotation.ACCESSOR,
            source.indexOf("@Accessor"),
            listOf(owner),
        )

        assertEquals(2, items.size)
        assertTrue(items.all { it.insertText.contains("throw new AssertionError();") })
        assertTrue(items.all { it.insertText.indexOf("\$0") < it.insertText.indexOf("throw new AssertionError();") })
        assertTrue(items.none { it.insertText.contains("abstract") })
        assertTrue(items.any { it.insertText.startsWith("static String \${1}(") })
        assertTrue(items.any { it.insertText.startsWith("static void \${1}(String value)") })
    }

    @Test
    fun instanceAccessorMakesConcreteMixinAbstract() {
        val source = source(
            annotation = "@Accessor(\"currentScreen\")",
            classHeader = "class ExampleMixin",
        )
        val items = complete(source, MixinAnnotation.ACCESSOR)

        assertEquals(2, items.size)
        assertTrue(items.all { item ->
            item.additionalEdits.any {
                it.startOffset == source.indexOf("class ExampleMixin") &&
                    it.newText == "abstract class"
            }
        })
    }

    @Test
    fun instanceDeclarationsInInterfaceNeedNoClassEdit() {
        val accessorSource = source(
            annotation = "@Accessor(\"currentScreen\")",
            classHeader = "interface ExampleMixin",
        )
        val invokerSource = source(
            annotation = "@Invoker(\"render\")",
            classHeader = "interface ExampleMixin",
        )

        val accessorItems = complete(accessorSource, MixinAnnotation.ACCESSOR)
        val invokerItems = complete(invokerSource, MixinAnnotation.INVOKER)

        assertEquals(2, accessorItems.size)
        assertEquals(2, invokerItems.size)
        assertTrue((accessorItems + invokerItems).all { item ->
            item.additionalEdits.none { it.newText == "abstract class" }
        })
    }

    @Test
    fun overloadedInvokerReturnsOneDeclarationPerResolvedDescriptor() {
        val source = source("@Invoker(\"render\")")
        val items = complete(source, MixinAnnotation.INVOKER)

        assertEquals(2, items.size)
        assertTrue(items.all { it.insertText.startsWith("public abstract void \${1}(") })
        assertTrue(items.map { it.metadata.descriptor }.distinct().size == 2)
        assertTrue(items.all { it.label.contains("render") })
    }

    @Test
    fun staticInvokerUsesStaticBody() {
        val method = MethodIndexEntry("factory", "(I)Lnet/minecraft/client/MinecraftClient;", true, "factory(int): MinecraftClient")
        val index = FakeClassIndex(
            methods = FakeClassIndex.defaultMethods() + (owner to FakeClassIndex.defaultMethods()[owner].orEmpty() + method),
        )
        val source = source("@Invoker(\"factory\")")
        val items = MixinDeclarationSnippetService(index).complete(
            source,
            MixinAnnotation.INVOKER,
            source.indexOf("@Invoker"),
            listOf(owner),
        )

        assertEquals(1, items.size)
        assertTrue(items.single().insertText.startsWith("static MinecraftClient \${1}(int arg0)"))
        assertTrue(items.single().insertText.contains("throw new AssertionError();"))
    }

    @Test
    fun constructorInvokerIsStaticFactoryReturningTargetType() {
        val constructor = MethodIndexEntry("<init>", "(Ljava/lang/String;)V", false, "<init>(String): void")
        val index = FakeClassIndex(
            classes = FakeClassIndex.defaultClasses() + ClassIndexEntry("MinecraftClient", "net.minecraft.client", owner),
            methods = FakeClassIndex.defaultMethods() + (owner to FakeClassIndex.defaultMethods()[owner].orEmpty() + constructor),
        )
        val source = source("@Invoker(\"<init>\")")
        val items = MixinDeclarationSnippetService(index).complete(
            source,
            MixinAnnotation.INVOKER,
            source.indexOf("@Invoker"),
            listOf(owner),
        )

        assertEquals(1, items.size)
        assertTrue(items.single().label.contains("constructor"))
        assertTrue(items.single().insertText.startsWith("static MinecraftClient \${1}(String arg0)"))
    }

    @Test
    fun invokerAcceptsJvmDescriptorSelector() {
        val source = source("@Invoker(\"render(I)V\")")
        val items = complete(source, MixinAnnotation.INVOKER)

        assertEquals(1, items.size)
        assertEquals("(I)V", items.single().metadata.descriptor)
        assertTrue(items.single().insertText.startsWith("public abstract void \${1}(int arg0)"))
    }

    @Test
    fun constructorInvokerAcceptsTargetSimpleNameAlias() {
        val constructor = MethodIndexEntry("<init>", "(Ljava/lang/String;)V", false, "<init>(String): void")
        val index = FakeClassIndex(
            classes = FakeClassIndex.defaultClasses() + ClassIndexEntry("MinecraftClient", "net.minecraft.client", owner),
            methods = FakeClassIndex.defaultMethods() + (owner to FakeClassIndex.defaultMethods()[owner].orEmpty() + constructor),
        )
        val source = source("@Invoker(\"MinecraftClient\")")
        val items = MixinDeclarationSnippetService(index).complete(
            source,
            MixinAnnotation.INVOKER,
            source.indexOf("@Invoker"),
            listOf(owner),
        )

        assertEquals(1, items.size)
        assertTrue(items.single().label.contains("constructor"))
    }

    @Test
    fun overwriteWithoutTargetOffersExplicitSignatureChoices() {
        val source = source("@Overwrite")
        val items = MixinDeclarationSnippetService(FakeClassIndex()).complete(
            source,
            MixinAnnotation.OVERWRITE,
            source.indexOf("@Overwrite"),
            listOf(owner),
        )

        assertEquals(4, items.size)
        assertTrue(items.any { it.label.contains("tick") && it.insertText == "public void \${1}() {\n    \$0\n}" })
        assertTrue(items.any { it.label.contains("render") && it.insertText.contains("long arg1") })
        assertTrue(items.all { it.insertText.contains("\${1}") })
    }

    @Test
    fun overwriteNonVoidUsesThrowingBodyStubWithEditableStopBeforeThrow() {
        val method = MethodIndexEntry("compute", "(I)I", false, "compute(int): int")
        val index = FakeClassIndex(
            methods = FakeClassIndex.defaultMethods() + (owner to FakeClassIndex.defaultMethods()[owner].orEmpty() + method),
        )
        val source = source("@Overwrite")
        val item = MixinDeclarationSnippetService(index).complete(
            source,
            MixinAnnotation.OVERWRITE,
            source.indexOf("@Overwrite"),
            listOf(owner),
        ).single { it.metadata.name == "compute" }

        assertTrue(item.insertText.contains("throw new AssertionError();"))
        assertTrue(item.insertText.indexOf("\$0") < item.insertText.indexOf("throw new AssertionError();"))
    }

    @Test
    fun mismatchedMultiTargetFieldDoesNotOfferAOneTargetDeclaration() {
        val secondOwner = "net/minecraft/client/OtherClient"
        val index = FakeClassIndex(
            methods = FakeClassIndex.defaultMethods(),
            fields = FakeClassIndex.defaultFields() + (
                secondOwner to listOf(FieldIndexEntry("currentScreen", "I", false, "int"))
                ),
        )
        val source = source("@Accessor(\"currentScreen\")")

        val items = MixinDeclarationSnippetService(index).complete(
            source,
            MixinAnnotation.ACCESSOR,
            source.indexOf("@Accessor"),
            listOf(owner, secondOwner),
        )

        assertTrue(items.isEmpty())
    }

    @Test
    fun overwriteRequiresExactlyOneMixinTarget() {
        val source = source("@Overwrite")
        val items = MixinDeclarationSnippetService(FakeClassIndex()).complete(
            source,
            MixinAnnotation.OVERWRITE,
            source.indexOf("@Overwrite"),
            listOf(owner, "net/minecraft/client/OtherClient"),
        )

        assertTrue(items.isEmpty())
    }

    @Test
    fun existingMemberAfterAnnotationDoesNotProduceDuplicateDeclaration() {
        val cases = listOf(
            "@Accessor(\"currentScreen\")\npublic abstract Screen getCurrentScreen();" to MixinAnnotation.ACCESSOR,
            "@Invoker(\"render\")\npublic abstract void render();" to MixinAnnotation.INVOKER,
            "@Overwrite\npublic void tick() {}" to MixinAnnotation.OVERWRITE,
        )

        cases.forEach { (annotation, kind) ->
            val source = source(annotation)
            assertTrue(
                complete(source, kind).isEmpty(),
                "Expected no declaration completion after an existing member for $kind",
            )
        }
    }

    @Test
    fun typeLikeTextInCommentCannotReceiveAbstractClassEdit() {
        val source = """
            /* class FakeMixin {
            @Accessor("currentScreen")
            }
             */
        """.trimIndent()
        val items = service.complete(
            source,
            MixinAnnotation.ACCESSOR,
            source.indexOf("@Accessor"),
            listOf(owner),
        )

        assertTrue(items.isNotEmpty())
        assertTrue(items.all { item -> item.additionalEdits.none { it.newText == "abstract class" } })
    }

    @Test
    fun unresolvedTargetsDoNotFabricateDeclarations() {
        val source = source("@Accessor(\"missing\")")
        val accessorItems = service.complete(
            source,
            MixinAnnotation.ACCESSOR,
            source.indexOf("@Accessor"),
            listOf(owner),
        )
        val overwriteItems = service.complete(
            source("@Overwrite"),
            MixinAnnotation.OVERWRITE,
            source.indexOf("@Accessor"),
            emptyList(),
        )

        assertTrue(accessorItems.isEmpty())
        assertTrue(overwriteItems.isEmpty())
    }

    private fun complete(source: String, annotation: MixinAnnotation) = service.complete(
        source,
        annotation,
        source.indexOf("@${annotation.simpleName}"),
        listOf(owner),
    )

    private fun source(
        annotation: String,
        classHeader: String = "abstract class ExampleMixin",
    ): String = """
        @Mixin(MinecraftClient.class)
        $classHeader {
            $annotation
        }
    """.trimIndent()
}
