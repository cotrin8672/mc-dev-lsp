package io.github.mcdev.core.at

import io.github.mcdev.core.mapping.ClassMapping
import io.github.mcdev.core.mapping.FieldMapping
import io.github.mcdev.core.mapping.MappingSet
import io.github.mcdev.core.mapping.MethodMapping
import io.github.mcdev.core.mapping.ProjectMappingContext
import io.github.mcdev.core.mapping.asResolver
import io.github.mcdev.core.mixin.FakeClassIndex
import io.github.mcdev.core.model.MappingNamespace
import io.github.mcdev.core.project.EmptyMappingResolver

object AtTestFixtures {
    val classIndex = FakeClassIndex()

    val fabricMappingContext: ProjectMappingContext = ProjectMappingContext(
        sourceNamespace = MappingNamespace.NAMED,
        runtimeNamespace = MappingNamespace.INTERMEDIARY,
        awNamespace = MappingNamespace.NAMED,
        atNamespace = null,
        availableNamespaces = setOf(MappingNamespace.NAMED, MappingNamespace.INTERMEDIARY),
        resolver = EmptyMappingResolver,
    )

    val forgeMappingContext: ProjectMappingContext = ProjectMappingContext(
        sourceNamespace = MappingNamespace.NAMED,
        runtimeNamespace = MappingNamespace.SRG,
        awNamespace = MappingNamespace.NAMED,
        atNamespace = MappingNamespace.SRG,
        availableNamespaces = setOf(MappingNamespace.NAMED, MappingNamespace.SRG),
        resolver = srgMappingSet().asResolver(),
    )

    fun srgMappingSet(): MappingSet {
        val minecraftClass = ClassMapping(
            mapOf(
                MappingNamespace.NAMED to "net/minecraft/client/MinecraftClient",
                MappingNamespace.SRG to "net/minecraft/client/Minecraft",
            ),
        )
        return MappingSet(
            namespaces = listOf(MappingNamespace.NAMED, MappingNamespace.SRG),
            classes = listOf(minecraftClass),
            methods = listOf(
                MethodMapping(
                    owner = minecraftClass,
                    descriptor = "(Lnet/minecraft/client/gui/screen/Screen;)V",
                    names = mapOf(
                        MappingNamespace.NAMED to "setScreen",
                        MappingNamespace.SRG to "m_91152_",
                    ),
                ),
                MethodMapping(
                    owner = minecraftClass,
                    descriptor = "()V",
                    names = mapOf(
                        MappingNamespace.NAMED to "tick",
                        MappingNamespace.SRG to "m_91398_",
                    ),
                ),
            ),
            fields = listOf(
                FieldMapping(
                    owner = minecraftClass,
                    descriptor = "Lnet/minecraft/client/gui/screen/Screen;",
                    names = mapOf(
                        MappingNamespace.NAMED to "currentScreen",
                        MappingNamespace.SRG to "f_91074_",
                    ),
                ),
                FieldMapping(
                    owner = minecraftClass,
                    descriptor = "Lnet/minecraft/client/player/ClientPlayerEntity;",
                    names = mapOf(
                        MappingNamespace.NAMED to "player",
                        MappingNamespace.SRG to "f_91075_",
                    ),
                ),
            ),
        )
    }

    fun incompleteSrgMappingContext(): ProjectMappingContext = ProjectMappingContext(
        sourceNamespace = MappingNamespace.NAMED,
        runtimeNamespace = MappingNamespace.SRG,
        awNamespace = MappingNamespace.NAMED,
        atNamespace = MappingNamespace.SRG,
        availableNamespaces = setOf(MappingNamespace.NAMED, MappingNamespace.SRG),
        resolver = MappingSet(
            namespaces = listOf(MappingNamespace.NAMED, MappingNamespace.SRG),
            classes = srgMappingSet().classes,
            methods = emptyList(),
            fields = srgMappingSet().fields,
        ).asResolver(),
    )
}
