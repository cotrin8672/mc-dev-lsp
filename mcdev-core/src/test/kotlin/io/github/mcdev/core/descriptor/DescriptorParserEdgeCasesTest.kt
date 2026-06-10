package io.github.mcdev.core.descriptor

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue

class DescriptorParserEdgeCasesTest {
    @Test
    fun rejectsMethodDescriptorMissingReturnType() {
        val parsed = parseMethodDescriptor("(I)")
        assertIs<DescriptorParseResult.Failure>(parsed)
    }

    @Test
    fun rejectsMethodDescriptorWithVoidParameter() {
        val parsed = parseMethodDescriptor("(V)V")
        assertIs<DescriptorParseResult.Failure>(parsed)
    }

    @Test
    fun rejectsFieldDescriptorUsingDotNotation() {
        val parsed = parseFieldDescriptor("Ljava.lang.String;")
        assertIs<DescriptorParseResult.Failure>(parsed)
    }

    @Test
    fun rendersNestedArrayDescriptor() {
        val parsed = assertIs<DescriptorParseResult.Success<JvmType>>(parseFieldDescriptor("[[I"))
        assertEquals("int[][]", DescriptorRenderer.render(parsed.value))
        assertEquals("[[I", DescriptorRenderer.toDescriptor(parsed.value))
    }

    @Test
    fun rendersConstructorTarget() {
        val parsed = assertIs<DescriptorParseResult.Success<MemberTarget>>(
            MemberTargetParser.parse("Lcom/example/Foo;<init>()V"),
        )
        val target = assertIs<MemberTarget.Method>(parsed.value)
        assertEquals("<init>", target.name)
        assertEquals("()", DescriptorRenderer.renderMethod(target.descriptor).substringBefore(':'))
    }

    @Test
    fun rejectsMemberTargetMissingOwnerSemicolon() {
        val parsed = MemberTargetParser.parse("Lcom/example/Foo")
        assertIs<DescriptorParseResult.Failure>(parsed)
    }

    @Test
    fun rejectsMemberTargetWithInvalidFieldDescriptor() {
        val parsed = MemberTargetParser.parse("Lcom/example/Foo;bar:Ljava.lang.String")
        assertIs<DescriptorParseResult.Failure>(parsed)
    }

    @Test
    fun exactTargetRenderingMatchesInputForMethod() {
        val input = "Lnet/minecraft/client/font/TextRenderer;draw(Ljava/lang/String;FFI)I"
        val parsed = assertIs<DescriptorParseResult.Success<MemberTarget>>(MemberTargetParser.parse(input))
        val method = assertIs<MemberTarget.Method>(parsed.value)
        val rendered = "L${method.owner};${method.name}${DescriptorRenderer.toDescriptor(method.descriptor)}"
        assertEquals(input, rendered)
    }

    @Test
    fun exactTargetRenderingMatchesInputForField() {
        val input = "Lnet/minecraft/client/MinecraftClient;currentScreen:Lnet/minecraft/client/gui/screen/Screen;"
        val parsed = assertIs<DescriptorParseResult.Success<MemberTarget>>(MemberTargetParser.parse(input))
        val field = assertIs<MemberTarget.Field>(parsed.value)
        val rendered = "L${field.owner};${field.name}:${DescriptorRenderer.toDescriptor(field.descriptor)}"
        assertEquals(input, rendered)
    }

    @Test
    fun parseFailureIncludesOffset() {
        val parsed = parseFieldDescriptor("Ljava/lang/String")
        val failure = assertIs<DescriptorParseResult.Failure>(parsed)
        assertTrue(failure.error.offset >= 0)
    }
}
