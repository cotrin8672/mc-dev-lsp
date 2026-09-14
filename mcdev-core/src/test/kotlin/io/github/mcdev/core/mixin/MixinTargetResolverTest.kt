package io.github.mcdev.core.mixin

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class MixinTargetResolverTest {
    private val classIndex = FakeClassIndex()

    @Test
    fun resolvesSimpleClassNameThroughIndex() {
        assertEquals(
            "net/minecraft/client/MinecraftClient",
            MixinTargetResolver.resolveTarget("MinecraftClient", classIndex),
        )
    }

    @Test
    fun resolvesFqnThroughIndex() {
        assertEquals(
            "net/minecraft/client/MinecraftClient",
            MixinTargetResolver.resolveTarget("net.minecraft.client.MinecraftClient", classIndex),
        )
    }

    @Test
    fun returnsNullForUnknownTarget() {
        assertNull(MixinTargetResolver.resolveTarget("UnknownClass", classIndex))
    }

    @Test
    fun resolvesTargetsFromSource() {
        val source = "@Mixin(MinecraftClient.class)\nclass M {}"
        assertEquals(
            listOf("net/minecraft/client/MinecraftClient"),
            MixinTargetResolver.resolveTargetsFromSource(source, classIndex),
        )
    }

    @Test
    fun resolvesImportedSimpleClassLiteralWhenSimpleNameIsAmbiguous() {
        val classIndex = FakeClassIndex(
            classes = FakeClassIndex.defaultClasses() + listOf(
                ClassIndexEntry("Item", "net.minecraft.world.item", "net/minecraft/world/item/Item"),
                ClassIndexEntry("Item", "com.example.other", "com/example/other/Item"),
            ),
        )
        val source = """
            import net.minecraft.world.item.Item;
            @Mixin(Item.class)
            class ItemMixin {}
        """.trimIndent()

        assertEquals(
            listOf("net/minecraft/world/item/Item"),
            MixinTargetResolver.resolveTargetsFromSource(source, classIndex),
        )
    }

    @Test
    fun resolvesSamePackageAndNestedClassWhenBindingIsUnavailable() {
        val classIndex = FakeClassIndex(
            classes = listOf(
                ClassIndexEntry("Target", "example.mixin", "example/mixin/Target"),
                ClassIndexEntry("Outer\$Inner", "example.mixin", "example/mixin/Outer\$Inner"),
            ),
        )
        val source = """
            package example.mixin;
            import example.mixin.Outer;
            @Mixin({ Target.class, Outer.Inner.class })
            class MixinClass {}
        """.trimIndent()

        assertEquals(
            listOf("example/mixin/Target", "example/mixin/Outer\$Inner"),
            MixinTargetResolver.resolveTargetsFromSource(source, classIndex),
        )
    }

    @Test
    fun explicitImportWinsAndAmbiguousWildcardDoesNotGuess() {
        val classIndex = FakeClassIndex(
            classes = listOf(
                ClassIndexEntry("Target", "one", "one/Target"),
                ClassIndexEntry("Target", "two", "two/Target"),
            ),
        )
        assertEquals(
            listOf("one/Target"),
            MixinTargetResolver.resolveTargetsFromSource(
                "import one.Target;\nimport two.*;\n@Mixin(Target.class) class M {}",
                classIndex,
            ),
        )
        assertEquals(
            emptyList(),
            MixinTargetResolver.resolveTargetsFromSource(
                "import one.*;\nimport two.*;\n@Mixin(Target.class) class M {}",
                classIndex,
            ),
        )
    }
}
