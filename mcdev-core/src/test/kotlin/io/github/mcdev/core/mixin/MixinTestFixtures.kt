package io.github.mcdev.core.mixin

import io.github.mcdev.core.model.MappingNamespace

class FakeClassIndex(
    private val classes: List<ClassIndexEntry> = defaultClasses(),
    private val methods: Map<String, List<MethodIndexEntry>> = defaultMethods(),
    private val fields: Map<String, List<FieldIndexEntry>> = defaultFields(),
) : ClassIndex {
    override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> =
        classes.filter {
            it.simpleName.startsWith(prefix, ignoreCase = true) ||
                it.fqn.startsWith(prefix, ignoreCase = true)
        }.take(limit)

    override fun findClass(internalName: String): ClassIndexEntry? =
        classes.find { it.internalName == internalName }

    override fun findClassByFqn(fqn: String): ClassIndexEntry? =
        classes.find { it.fqn == fqn }

    override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> =
        methods[ownerInternalName].orEmpty()

    override fun getFields(ownerInternalName: String): List<FieldIndexEntry> =
        fields[ownerInternalName].orEmpty()

    companion object {
        fun defaultClasses(): List<ClassIndexEntry> = listOf(
            ClassIndexEntry("MinecraftClient", "net.minecraft.client", "net/minecraft/client/MinecraftClient"),
            ClassIndexEntry("GameRenderer", "net.minecraft.client.render", "net/minecraft/client/render/GameRenderer"),
            ClassIndexEntry("TextRenderer", "net.minecraft.client.font", "net/minecraft/client/font/TextRenderer"),
            ClassIndexEntry("DrawContext", "net.minecraft.client.gui", "net/minecraft/client/gui/DrawContext"),
        )

        fun defaultMethods(): Map<String, List<MethodIndexEntry>> = mapOf(
            "net/minecraft/client/MinecraftClient" to listOf(
                MethodIndexEntry("tick", "()V", false, "tick(): void"),
                MethodIndexEntry("setScreen", "(Lnet/minecraft/client/gui/screen/Screen;)V", false, "setScreen(Screen): void"),
                MethodIndexEntry("render", "(FJZ)V", false, "render(float, long, boolean): void"),
                MethodIndexEntry("render", "(I)V", false, "render(int): void"),
            ),
            "net/minecraft/client/font/TextRenderer" to listOf(
                MethodIndexEntry("draw", "(Ljava/lang/String;FFI)I", false, "draw(String, float, float, int): int"),
            ),
        )

        fun defaultFields(): Map<String, List<FieldIndexEntry>> = mapOf(
            "net/minecraft/client/MinecraftClient" to listOf(
                FieldIndexEntry("currentScreen", "Lnet/minecraft/client/gui/screen/Screen;", false, "Screen"),
                FieldIndexEntry("player", "Lnet/minecraft/client/player/ClientPlayerEntity;", false, "ClientPlayerEntity"),
            ),
        )
    }
}

class FakeBytecodeIndex(
    private val candidates: Map<String, List<AtTargetCandidate>> = defaultCandidates(),
    private val returnCounts: Map<String, Int> = defaultReturnCounts(),
) : BytecodeIndex {
    override fun getAtTargetCandidates(
        ownerInternalName: String,
        methodName: String,
        methodDescriptor: String?,
        atValue: String,
    ): List<AtTargetCandidate> = candidates["$ownerInternalName#$methodName#$atValue"].orEmpty()

    override fun getReturnOrdinalCount(
        ownerInternalName: String,
        methodName: String,
        methodDescriptor: String?,
    ): Int = returnCounts["$ownerInternalName#$methodName"] ?: 1

    companion object {
        fun defaultCandidates(): Map<String, List<AtTargetCandidate>> = mapOf(
            "net/minecraft/client/MinecraftClient#tick#INVOKE" to listOf(
                AtTargetCandidate(
                    owner = "net/minecraft/client/font/TextRenderer",
                    name = "draw",
                    descriptor = "(Ljava/lang/String;FFI)I",
                    displayLabel = "draw(String, float, float, int): int",
                    detail = "TextRenderer",
                    kind = AtTargetKind.INVOKE,
                    ordinal = 0,
                    namespace = MappingNamespace.NAMED,
                ),
            ),
            "net/minecraft/client/MinecraftClient#tick#FIELD" to listOf(
                AtTargetCandidate(
                    owner = "net/minecraft/client/MinecraftClient",
                    name = "currentScreen",
                    descriptor = "Lnet/minecraft/client/gui/screen/Screen;",
                    displayLabel = "currentScreen: Screen",
                    detail = "MinecraftClient",
                    kind = AtTargetKind.FIELD,
                    ordinal = 0,
                ),
            ),
        )

        fun defaultReturnCounts(): Map<String, Int> = mapOf(
            "net/minecraft/client/MinecraftClient#tick" to 2,
        )
    }
}
