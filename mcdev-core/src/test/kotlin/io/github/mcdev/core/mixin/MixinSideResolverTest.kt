package io.github.mcdev.core.mixin

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class MixinSideResolverTest {
    @Test
    fun fabricEnvironmentClient() {
        val source = """
            @Environment(EnvType.CLIENT)
            @Mixin(MinecraftClient.class)
            public class ExampleMixin {}
        """.trimIndent()
        assertEquals(MixinSide.CLIENT, MixinSideResolver.explicitSideFromSource(source))
    }

    @Test
    fun fabricEnvironmentServer() {
        val source = """
            @Environment(EnvType.SERVER)
            @Mixin(DedicatedServer.class)
            class ExampleMixin {}
        """.trimIndent()
        assertEquals(MixinSide.SERVER, MixinSideResolver.explicitSideFromSource(source))
    }

    @Test
    fun forgeOnlyInClient() {
        val source = """
            @OnlyIn(Dist.CLIENT)
            @Mixin(Minecraft.class)
            public class ExampleMixin {}
        """.trimIndent()
        assertEquals(MixinSide.CLIENT, MixinSideResolver.explicitSideFromSource(source))
    }

    @Test
    fun forgeOnlyInDedicatedServer() {
        val source = """
            @OnlyIn(Dist.DEDICATED_SERVER)
            @Mixin(MinecraftServer.class)
            public class ExampleMixin {}
        """.trimIndent()
        assertEquals(MixinSide.SERVER, MixinSideResolver.explicitSideFromSource(source))
    }

    @Test
    fun legacySideOnlyClient() {
        val source = """
            @SideOnly(Side.CLIENT)
            @Mixin(Minecraft.class)
            public class ExampleMixin {}
        """.trimIndent()
        assertEquals(MixinSide.CLIENT, MixinSideResolver.explicitSideFromSource(source))
    }

    @Test
    fun legacySideOnlyServer() {
        val source = """
            @SideOnly(Side.SERVER)
            @Mixin(MinecraftServer.class)
            public class ExampleMixin {}
        """.trimIndent()
        assertEquals(MixinSide.SERVER, MixinSideResolver.explicitSideFromSource(source))
    }

    @Test
    fun fullyQualifiedAnnotationAndEnumValues() {
        val source = """
            @net.fabricmc.api.Environment(net.fabricmc.api.EnvType.SERVER)
            @Mixin(MinecraftServer.class)
            public class ExampleMixin {}
        """.trimIndent()
        assertEquals(MixinSide.SERVER, MixinSideResolver.explicitSideFromSource(source))
    }

    @Test
    fun neoForgeFullyQualifiedOnlyIn() {
        val source = """
            @net.neoforged.api.distmarker.OnlyIn(net.neoforged.api.distmarker.Dist.CLIENT)
            @Mixin(Minecraft.class)
            public class ExampleMixin {}
        """.trimIndent()
        assertEquals(MixinSide.CLIENT, MixinSideResolver.explicitSideFromSource(source))
    }

    @Test
    fun netMinecraftForgeFmlFullyQualifiedSideOnly() {
        val source = """
            @net.minecraftforge.fml.relauncher.SideOnly(net.minecraftforge.fml.relauncher.Side.SERVER)
            @Mixin(MinecraftServer.class)
            public class ExampleMixin {}
        """.trimIndent()
        assertEquals(MixinSide.SERVER, MixinSideResolver.explicitSideFromSource(source))
    }

    @Test
    fun allowsWhitespaceBeforeAnnotationParen() {
        val source = """
            @OnlyIn
            (Dist.CLIENT)
            @Mixin(Minecraft.class)
            public class ExampleMixin {}
        """.trimIndent()
        assertEquals(MixinSide.CLIENT, MixinSideResolver.explicitSideFromSource(source))
    }

    @Test
    fun ignoresSideAnnotationExamplesInComments() {
        val source = """
            // @Environment(EnvType.CLIENT)
            /* @OnlyIn(Dist.CLIENT) */
            @Mixin(MinecraftClient.class)
            public class ExampleMixin {}
        """.trimIndent()
        assertNull(MixinSideResolver.explicitSideFromSource(source))
    }

    @Test
    fun ignoresMethodAnnotationAfterClassDeclaration() {
        val source = """
            @Mixin(MinecraftClient.class)
            public class ExampleMixin {
                @SideOnly(Side.CLIENT)
                private void clientOnly() {}
            }
        """.trimIndent()
        assertNull(MixinSideResolver.explicitSideFromSource(source))
    }

    @Test
    fun returnsNullWhenNoSideAnnotationPresent() {
        val source = """
            @Mixin(MinecraftClient.class)
            public class ExampleMixin {}
        """.trimIndent()
        assertNull(MixinSideResolver.explicitSideFromSource(source))
    }

    @Test
    fun returnsNullForConflictingClientAndServerAnnotations() {
        val source = """
            @Environment(EnvType.CLIENT)
            @OnlyIn(Dist.DEDICATED_SERVER)
            @Mixin(MinecraftClient.class)
            public class ExampleMixin {}
        """.trimIndent()
        assertNull(MixinSideResolver.explicitSideFromSource(source))
    }

    @Test
    fun returnsNullForUnqualifiedAmbiguousEnumValue() {
        val source = """
            @Environment(CLIENT)
            @Mixin(MinecraftClient.class)
            public class ExampleMixin {}
        """.trimIndent()
        assertNull(MixinSideResolver.explicitSideFromSource(source))
    }

    @Test
    fun ignoresUnqualifiedSideAnnotationAndFindsLaterValidOne() {
        val source = """
            @Environment(CLIENT)
            @OnlyIn(Dist.CLIENT)
            @Mixin(Minecraft.class)
            public class ExampleMixin {}
        """.trimIndent()
        assertEquals(MixinSide.CLIENT, MixinSideResolver.explicitSideFromSource(source))
    }

    @Test
    fun returnsNullForMalformedSideAnnotation() {
        val source = """
            @Environment(EnvType.CLIENT
            @Mixin(MinecraftClient.class)
            public class ExampleMixin {}
        """.trimIndent()
        assertNull(MixinSideResolver.explicitSideFromSource(source))
    }
}
