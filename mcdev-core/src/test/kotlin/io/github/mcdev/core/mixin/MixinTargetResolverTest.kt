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
}
