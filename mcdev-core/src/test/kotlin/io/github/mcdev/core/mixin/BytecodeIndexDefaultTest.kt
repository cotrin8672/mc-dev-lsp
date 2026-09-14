package io.github.mcdev.core.mixin

import kotlin.test.Test
import kotlin.test.assertNull

class BytecodeIndexDefaultTest {
    @Test
    fun fakeBytecodeIndexUsesDefaultNullGetClassBytes() {
        assertNull(FakeBytecodeIndex().getClassBytes("net/minecraft/client/MinecraftClient"))
    }

    @Test
    fun fakeBytecodeIndexUsesDefaultNullResolveCommonSuperClass() {
        assertNull(
            FakeBytecodeIndex().resolveCommonSuperClass(
                "Lhierarchy/ChildOne;",
                "Lhierarchy/ChildTwo;",
            ),
        )
    }
}
