package io.github.mcdev.core.descriptor

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs

class DescriptorParserTest {
    @Test
    fun parsesPrimitiveFieldDescriptor() {
        val parsed = assertIs<DescriptorParseResult.Success<JvmType>>(parseFieldDescriptor("I"))
        assertEquals(JvmType.IntType, parsed.value)
        assertEquals("int", DescriptorRenderer.render(parsed.value))
    }

    @Test
    fun parsesObjectAndArrayDescriptors() {
        val parsed = assertIs<DescriptorParseResult.Success<JvmType>>(parseFieldDescriptor("[Ljava/lang/String;"))
        assertEquals("String[]", DescriptorRenderer.render(parsed.value))
        assertEquals("[Ljava/lang/String;", DescriptorRenderer.toDescriptor(parsed.value))
    }

    @Test
    fun parsesMethodDescriptor() {
        val parsed = assertIs<DescriptorParseResult.Success<MethodDescriptor>>(parseMethodDescriptor("(Ljava/lang/String;FFI)I"))
        assertEquals("(String, float, float, int): int", DescriptorRenderer.renderMethod(parsed.value))
        assertEquals("(Ljava/lang/String;FFI)I", DescriptorRenderer.toDescriptor(parsed.value))
    }

    @Test
    fun rejectsInvalidObjectDescriptor() {
        val parsed = parseFieldDescriptor("Ljava/lang/String")
        assertIs<DescriptorParseResult.Failure>(parsed)
    }

    @Test
    fun parsesMethodTarget() {
        val parsed = assertIs<DescriptorParseResult.Success<MemberTarget>>(
            MemberTargetParser.parse("Lnet/minecraft/client/font/TextRenderer;draw(Ljava/lang/String;FFI)I"),
        )
        val target = assertIs<MemberTarget.Method>(parsed.value)
        assertEquals("net/minecraft/client/font/TextRenderer", target.owner)
        assertEquals("draw", target.name)
        assertEquals("(String, float, float, int): int", DescriptorRenderer.renderMethod(target.descriptor))
    }

    @Test
    fun parsesFieldTarget() {
        val parsed = assertIs<DescriptorParseResult.Success<MemberTarget>>(
            MemberTargetParser.parse("Lnet/minecraft/client/MinecraftClient;currentScreen:Lnet/minecraft/client/gui/screen/Screen;"),
        )
        val target = assertIs<MemberTarget.Field>(parsed.value)
        assertEquals("currentScreen", target.name)
        assertEquals("Screen", DescriptorRenderer.render(target.descriptor))
    }
}
