package io.github.mcdev.core.aw

import io.github.mcdev.core.mapping.ClassRef
import io.github.mcdev.core.mapping.FieldRef
import io.github.mcdev.core.mapping.MappingLookupResult
import io.github.mcdev.core.mapping.MappingResolver
import io.github.mcdev.core.mapping.MethodRef
import io.github.mcdev.core.mapping.ProjectMappingContext
import io.github.mcdev.core.mapping.MappingSubject
import io.github.mcdev.core.mixin.FakeClassIndex
import io.github.mcdev.core.model.MappingNamespace

object AwTestFixtures {
    val VALID_AW = """
        accessWidener v2 named
        accessible class net/minecraft/client/MinecraftClient
        accessible method net/minecraft/client/MinecraftClient setScreen (Lnet/minecraft/client/gui/screen/Screen;)V
        mutable field net/minecraft/client/MinecraftClient currentScreen Lnet/minecraft/client/gui/screen/Screen;
        """.trimIndent()

    val INTERMEDIARY_AW = """
        accessWidener v2 intermediary
        accessible class net/minecraft/client/class_310
        accessible method net/minecraft/client/class_310 method_91152 (Lnet/minecraft/client/gui/class_437;)V
        """.trimIndent()

    fun mappingContext(): ProjectMappingContext = ProjectMappingContext(
        sourceNamespace = MappingNamespace.NAMED,
        runtimeNamespace = MappingNamespace.INTERMEDIARY,
        awNamespace = MappingNamespace.NAMED,
        atNamespace = null,
        availableNamespaces = setOf(MappingNamespace.NAMED, MappingNamespace.INTERMEDIARY),
        resolver = FakeAwMappingResolver,
    )

    val classIndex = FakeClassIndex()
}

object FakeAwMappingResolver : MappingResolver {
    override val namespaces: Set<MappingNamespace> = setOf(MappingNamespace.NAMED, MappingNamespace.INTERMEDIARY)

    private val classMap = mapOf(
        MappingNamespace.NAMED to "net/minecraft/client/MinecraftClient",
        MappingNamespace.INTERMEDIARY to "net/minecraft/client/class_310",
    )

    override fun remapClass(ref: ClassRef, to: MappingNamespace): MappingLookupResult<ClassRef> {
        if (ref.namespace == to) return MappingLookupResult.Found(ref)
        val named = when (ref.namespace) {
            MappingNamespace.NAMED -> ref.internalName
            MappingNamespace.INTERMEDIARY -> classMap[MappingNamespace.NAMED]
            else -> null
        }
        val intermediary = classMap[MappingNamespace.INTERMEDIARY]
        return when (to) {
            MappingNamespace.NAMED -> if (ref.internalName == intermediary) {
                MappingLookupResult.Found(ClassRef(classMap[MappingNamespace.NAMED]!!, MappingNamespace.NAMED))
            } else if (ref.namespace == MappingNamespace.NAMED) {
                MappingLookupResult.Found(ref)
            } else {
                MappingLookupResult.Missing(MappingSubject.CLASS, owner = ref.internalName, from = ref.namespace, to = to)
            }
            MappingNamespace.INTERMEDIARY -> if (ref.internalName == classMap[MappingNamespace.NAMED]) {
                MappingLookupResult.Found(ClassRef(intermediary!!, MappingNamespace.INTERMEDIARY))
            } else if (ref.namespace == MappingNamespace.INTERMEDIARY) {
                MappingLookupResult.Found(ref)
            } else {
                MappingLookupResult.Missing(MappingSubject.CLASS, owner = ref.internalName, from = ref.namespace, to = to)
            }
            else -> MappingLookupResult.Missing(MappingSubject.CLASS, owner = ref.internalName, from = ref.namespace, to = to)
        }
    }

    override fun remapField(ref: FieldRef, to: MappingNamespace): MappingLookupResult<FieldRef> {
        if (ref.namespace == to) return MappingLookupResult.Found(ref)
        val owner = when (val ownerResult = remapClass(ClassRef(ref.owner, ref.namespace), to)) {
            is MappingLookupResult.Found -> ownerResult.value.internalName
            is MappingLookupResult.Missing -> return MappingLookupResult.Missing(
                MappingSubject.FIELD,
                owner = ref.owner,
                name = ref.name,
                descriptor = ref.descriptor,
                from = ref.namespace,
                to = to,
            )
            is MappingLookupResult.InvalidDescriptor -> return MappingLookupResult.InvalidDescriptor(
                ownerResult.descriptor,
                ownerResult.message,
                ownerResult.offset,
            )
        }
        val name = when {
            ref.name == "currentScreen" && to == MappingNamespace.INTERMEDIARY -> "field_1755"
            ref.name == "field_1755" && to == MappingNamespace.NAMED -> "currentScreen"
            ref.namespace == to -> ref.name
            else -> return MappingLookupResult.Missing(
                MappingSubject.FIELD,
                owner = ref.owner,
                name = ref.name,
                descriptor = ref.descriptor,
                from = ref.namespace,
                to = to,
            )
        }
        return MappingLookupResult.Found(FieldRef(owner, name, ref.descriptor, to))
    }

    override fun remapMethod(ref: MethodRef, to: MappingNamespace): MappingLookupResult<MethodRef> {
        if (ref.namespace == to) return MappingLookupResult.Found(ref)
        val owner = when (val ownerResult = remapClass(ClassRef(ref.owner, ref.namespace), to)) {
            is MappingLookupResult.Found -> ownerResult.value.internalName
            is MappingLookupResult.Missing -> return MappingLookupResult.Missing(
                MappingSubject.METHOD,
                owner = ref.owner,
                name = ref.name,
                descriptor = ref.descriptor,
                from = ref.namespace,
                to = to,
            )
            is MappingLookupResult.InvalidDescriptor -> return MappingLookupResult.InvalidDescriptor(
                ownerResult.descriptor,
                ownerResult.message,
                ownerResult.offset,
            )
        }
        val name = when {
            ref.name == "setScreen" && to == MappingNamespace.INTERMEDIARY -> "method_91152"
            ref.name == "method_91152" && to == MappingNamespace.NAMED -> "setScreen"
            ref.namespace == to -> ref.name
            else -> return MappingLookupResult.Missing(
                MappingSubject.METHOD,
                owner = ref.owner,
                name = ref.name,
                descriptor = ref.descriptor,
                from = ref.namespace,
                to = to,
            )
        }
        return MappingLookupResult.Found(MethodRef(owner, name, ref.descriptor, to))
    }

    override fun remapDescriptor(
        descriptor: String,
        from: MappingNamespace,
        to: MappingNamespace,
    ): MappingLookupResult<String> = if (from == to) {
        MappingLookupResult.Found(descriptor)
    } else {
        MappingLookupResult.Found(descriptor)
    }
}
