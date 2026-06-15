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
}
