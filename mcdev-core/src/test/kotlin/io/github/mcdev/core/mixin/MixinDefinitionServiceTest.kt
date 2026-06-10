package io.github.mcdev.core.mixin

import io.github.mcdev.core.index.ProjectContextMixinIndex
import io.github.mcdev.core.mixin.e2e.MixinE2ETestSupport
import io.github.mcdev.core.model.MemberKind
import io.github.mcdev.core.project.ProjectContextBuilder
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import java.nio.file.Path

class MixinDefinitionServiceTest {
    private val fakeService = MixinDefinitionService(FakeClassIndex(), FakeBytecodeIndex())
    private val simpleService = buildSimpleTargetDefinitionService()

    @Test
    fun resolvesMixinClassTarget() {
        val source = """@Mixin(Mine"""
        val request = MixinE2ETestSupport.requestAt(source, "Mine")
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals(MemberKind.CLASS, targets.first().kind)
        assertEquals("net/minecraft/client/MinecraftClient", targets.first().ownerInternalName)
    }

    @Test
    fun resolvesMixinTargetsString() {
        val source = """@Mixin(targets = "net.minecraft.client.Mine")"""
        val request = MixinE2ETestSupport.requestAt(source, "Mine")
        val targets = fakeService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals("net/minecraft/client/MinecraftClient", targets.first().ownerInternalName)
    }

    @Test
    fun resolvesShadowField() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @Shadow private int counter;
            }
        """.trimIndent()
        val offset = source.indexOf("counter") + 3
        val request = MixinE2ETestSupport.requestAtOffset(source, offset)
        val targets = simpleService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals(MemberKind.FIELD, targets.first().kind)
        assertEquals("counter", targets.first().name)
        assertEquals(MixinE2ETestSupport.SIMPLE_TARGET_INTERNAL, targets.first().ownerInternalName)
    }

    @Test
    fun resolvesShadowMethod() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @Shadow public abstract void draw(String text, float x, float y);
            }
        """.trimIndent()
        val offset = source.indexOf("draw") + 2
        val request = MixinE2ETestSupport.requestAtOffset(source, offset)
        val targets = simpleService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals(MemberKind.METHOD, targets.first().kind)
        assertEquals("draw", targets.first().name)
        assertTrue(targets.first().descriptor!!.contains("Ljava/lang/String;"))
    }

    @Test
    fun resolvesAccessorField() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @Accessor("counter")
                abstract int getCounter();
            }
        """.trimIndent()
        val request = MixinE2ETestSupport.requestInAnnotationValue(source, "@Accessor", "counter")
        val targets = simpleService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals(MemberKind.FIELD, targets.first().kind)
        assertEquals("counter", targets.first().name)
    }

    @Test
    fun resolvesInvokerMethod() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @Invoker("draw")
                abstract void invokeDraw(String text, float x, float y);
            }
        """.trimIndent()
        val request = MixinE2ETestSupport.requestInAnnotationValue(source, "@Invoker", "draw")
        val targets = simpleService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals(MemberKind.METHOD, targets.first().kind)
        assertEquals("draw", targets.first().name)
    }

    @Test
    fun resolvesAtTargetInvokeMember() {
        val source = """
            @Mixin(com.example.target.SimpleTarget.class)
            abstract class ExampleMixin {
                @Inject(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
                private void onDraw() {}
            }
        """.trimIndent()
        val targetMarker = "target = \""
        val valueStart = source.indexOf(targetMarker) + targetMarker.length
        val lengthIndex = source.indexOf("length", valueStart)
        val request = MixinE2ETestSupport.requestAtOffset(source, lengthIndex + 2)
        val targets = simpleService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals(MemberKind.METHOD, targets.first().kind)
        assertEquals("length", targets.first().name)
        assertEquals("java/lang/String", targets.first().ownerInternalName)
    }

    @Test
    fun resolvesSimpleTargetMixinClassFromBytecodeIndex() {
        val source = """@Mixin(Simple"""
        val request = MixinE2ETestSupport.requestAt(source, "Simple")
        val targets = simpleService.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals(MixinE2ETestSupport.SIMPLE_TARGET_INTERNAL, targets.first().ownerInternalName)
        assertEquals("com.example.target.SimpleTarget", targets.first().ownerFqn)
    }

    @Test
    fun returnsEmptyOutsideMixinContext() {
        val source = "package com.example;\npublic class Plain {}"
        val targets = fakeService.definitionsAt(source, 1, 10)
        assertTrue(targets.isEmpty())
    }

    @Test
    fun atTargetParserParsesFieldDescriptor() {
        val parsed = AtTargetParser.parse("Lcom/example/target/SimpleTarget;counter:I")
        assertNotNull(parsed)
        assertEquals(MemberKind.FIELD, parsed.kind)
        assertEquals("counter", parsed.name)
        assertEquals("I", parsed.descriptor)
    }

    @Test
    fun atTargetParserParsesMethodDescriptor() {
        val parsed = AtTargetParser.parse("Lcom/example/target/SimpleTarget;draw(Ljava/lang/String;FF)V")
        assertNotNull(parsed)
        assertEquals(MemberKind.METHOD, parsed.kind)
        assertEquals("draw", parsed.name)
    }

    private fun buildSimpleTargetDefinitionService(): MixinDefinitionService {
        val provider = MixinE2ETestSupport.simpleTargetProvider()
        val index = ProjectContextMixinIndex()
        val context = ProjectContextBuilder.empty("definition-test", Path.of("."))
        return MixinDefinitionService(
            classIndex = index.buildClassIndex(context, provider),
            bytecodeIndex = index.buildBytecodeIndex(context, provider),
        )
    }
}
