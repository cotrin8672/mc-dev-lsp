package io.github.mcdev.core.mixin

import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class ShadowAccessorInvokerTest {
    private val classIndex = FakeClassIndex()
    private val shadowService = ShadowValidationService(classIndex)
    private val accessorService = AccessorService(classIndex)
    private val invokerService = InvokerService(classIndex)
    private val range = McTextRange(McTextPosition(1, 4), McTextPosition(1, 20))

    @Test
    fun shadowFieldValidationPassesForValidTarget() {
        val decl = ShadowMemberDeclaration("currentScreen", false, "Lnet/minecraft/client/gui/screen/Screen;", false, range)
        val diagnostics = shadowService.validate(listOf("net/minecraft/client/MinecraftClient"), decl)
        assertTrue(diagnostics.isEmpty())
    }

    @Test
    fun shadowFieldValidationFailsForMissingTarget() {
        val decl = ShadowMemberDeclaration("missingField", false, "I", false, range)
        val diagnostics = shadowService.validate(listOf("net/minecraft/client/MinecraftClient"), decl)
        assertEquals(MixinDiagnosticCodes.SHADOW_TARGET_NOT_FOUND, diagnostics.first().code)
    }

    @Test
    fun shadowFieldStaticMismatchProducesDiagnostic() {
        val decl = ShadowMemberDeclaration(
            "currentScreen",
            false,
            "Lnet/minecraft/client/gui/screen/Screen;",
            true,
            range,
        )
        val diagnostics = shadowService.validate(listOf("net/minecraft/client/MinecraftClient"), decl)
        assertEquals(MixinDiagnosticCodes.SHADOW_STATIC_MISMATCH, diagnostics.first().code)
    }

    @Test
    fun shadowFieldValidationFailsForDescriptorMismatch() {
        val decl = ShadowMemberDeclaration("currentScreen", false, "I", false, range)
        val diagnostics = shadowService.validate(listOf("net/minecraft/client/MinecraftClient"), decl)
        assertEquals(MixinDiagnosticCodes.SHADOW_DESCRIPTOR_MISMATCH, diagnostics.first().code)
    }

    @Test
    fun shadowMethodValidationPassesForValidTarget() {
        val decl = ShadowMemberDeclaration("tick", true, "()V", false, range)
        val diagnostics = shadowService.validate(listOf("net/minecraft/client/MinecraftClient"), decl)
        assertTrue(diagnostics.isEmpty())
    }

    @Test
    fun shadowMethodValidationFailsForMissingMethod() {
        val decl = ShadowMemberDeclaration("missingMethod", true, "()V", false, range)
        val diagnostics = shadowService.validate(listOf("net/minecraft/client/MinecraftClient"), decl)
        assertEquals(MixinDiagnosticCodes.SHADOW_TARGET_NOT_FOUND, diagnostics.first().code)
    }

    @Test
    fun shadowPrefixResolutionStripsPrefix() {
        val decl = ShadowMemberDeclaration("shadow\$currentScreen", false, "Lnet/minecraft/client/gui/screen/Screen;", false, range)
        val diagnostics = shadowService.validate(listOf("net/minecraft/client/MinecraftClient"), decl, shadowPrefix = "shadow$")
        assertTrue(diagnostics.isEmpty())
    }

    @Test
    fun accessorInfersGetterFieldName() {
        val decl = AccessorMethodDeclaration("getCurrentScreen", "Lnet/minecraft/client/gui/screen/Screen;", emptyList(), null, range)
        assertEquals("currentScreen", accessorService.inferFieldName(decl))
        assertEquals(AccessorKind.GETTER, accessorService.inferKind(decl))
    }

    @Test
    fun accessorInfersSetterFieldName() {
        val decl = AccessorMethodDeclaration(
            "setCurrentScreen",
            "V",
            listOf("Lnet/minecraft/client/gui/screen/Screen;"),
            null,
            range,
        )
        assertEquals("currentScreen", accessorService.inferFieldName(decl))
        assertEquals(AccessorKind.SETTER, accessorService.inferKind(decl))
    }

    @Test
    fun accessorValidationPassesForValidGetter() {
        val decl = AccessorMethodDeclaration("getCurrentScreen", "Lnet/minecraft/client/gui/screen/Screen;", emptyList(), "currentScreen", range)
        assertTrue(accessorService.validate(listOf("net/minecraft/client/MinecraftClient"), decl).isEmpty())
    }

    @Test
    fun accessorValidationFailsForMissingField() {
        val decl = AccessorMethodDeclaration("getMissing", "I", emptyList(), "missing", range)
        assertEquals(MixinDiagnosticCodes.ACCESSOR_FIELD_NOT_FOUND, accessorService.validate(listOf("net/minecraft/client/MinecraftClient"), decl).first().code)
    }

    @Test
    fun accessorValidationFailsForSignatureMismatch() {
        val decl = AccessorMethodDeclaration("getCurrentScreen", "I", emptyList(), "currentScreen", range)
        assertEquals(MixinDiagnosticCodes.ACCESSOR_SIGNATURE_MISMATCH, accessorService.validate(listOf("net/minecraft/client/MinecraftClient"), decl).first().code)
    }

    @Test
    fun accessorFieldCompletionReturnsCandidates() {
        val items = accessorService.completeFields(listOf("net/minecraft/client/MinecraftClient"), "current")
        assertEquals("currentScreen", items.first().insertText)
    }

    @Test
    fun accessorGeneratesGetterStub() {
        val field = FakeClassIndex.defaultFields().values.first().first()
        val stub = accessorService.generateMethodStub(field, AccessorKind.GETTER)
        assertTrue(stub.contains("@Accessor(\"currentScreen\")"))
        assertTrue(stub.contains("getCurrentScreen"))
    }

    @Test
    fun invokerInfersTargetName() {
        val decl = InvokerMethodDeclaration("invokeSetScreen", listOf("Lnet/minecraft/client/gui/screen/Screen;"), "V", null, range)
        assertEquals("setScreen", invokerService.inferTargetName(decl))
    }

    @Test
    fun invokerValidationPassesForValidTarget() {
        val decl = InvokerMethodDeclaration(
            "invokeSetScreen",
            listOf("Lnet/minecraft/client/gui/screen/Screen;"),
            "V",
            "setScreen",
            range,
        )
        assertTrue(invokerService.validate(listOf("net/minecraft/client/MinecraftClient"), decl).isEmpty())
    }

    @Test
    fun invokerValidationFailsForMissingMethod() {
        val decl = InvokerMethodDeclaration("invokeMissing", emptyList(), "V", "missing", range)
        assertEquals(MixinDiagnosticCodes.INVOKER_METHOD_NOT_FOUND, invokerService.validate(listOf("net/minecraft/client/MinecraftClient"), decl).first().code)
    }

    @Test
    fun invokerValidationFailsForDescriptorMismatch() {
        val decl = InvokerMethodDeclaration("invokeSetScreen", listOf("I"), "V", "setScreen", range)
        assertEquals(MixinDiagnosticCodes.INVOKER_DESCRIPTOR_MISMATCH, invokerService.validate(listOf("net/minecraft/client/MinecraftClient"), decl).first().code)
    }

    @Test
    fun invokerMethodCompletionReturnsCandidates() {
        val items = invokerService.completeMethods(listOf("net/minecraft/client/MinecraftClient"), "set")
        assertTrue(items.any { it.insertText == "setScreen" })
    }

    @Test
    fun invokerGeneratesMethodStub() {
        val method = FakeClassIndex.defaultMethods().values.first().find { it.name == "setScreen" }!!
        val stub = invokerService.generateMethodStub(method)
        assertTrue(stub.contains("@Invoker(\"setScreen\")"))
        assertTrue(stub.contains("invokeSetScreen"))
    }
}
