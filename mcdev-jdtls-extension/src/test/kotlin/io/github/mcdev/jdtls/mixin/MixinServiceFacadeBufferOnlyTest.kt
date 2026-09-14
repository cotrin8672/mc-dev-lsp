package io.github.mcdev.jdtls.mixin

import io.github.mcdev.core.mixin.AnnotationSlot
import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FieldIndexEntry
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.mixin.MixinAnnotation
import io.github.mcdev.core.mixin.MixinCompletionOptions
import io.github.mcdev.core.mixin.MixinServiceFacade as CoreMixinServiceFacade
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class MixinServiceFacadeBufferOnlyTest {
    private val documentUri = "file:///ExampleMixin.java"
    private val languageId = "java"
    private val options = MixinCompletionOptions()
    private val fakeJavaProject = Any()
    private val specifierFqn = "org.spongepowered.asm.mixin.injection.InjectionPoint\$Specifier"

    @Test
    fun injectorAttributeCompletionWorksWithoutJavaProjectResolver() {
        var resolverCalls = 0
        val facade = MixinServiceFacade(
            javaProjectResolver = {
                resolverCalls++
                fakeJavaProject
            },
        )
        val source = """@Inject(meth"""
        val (line, character) = positionAt(source, source.indexOf("meth") + "meth".length)

        val bufferOnly = facade.tryCompleteBufferOnly(
            source = source,
            line = line,
            character = character,
            options = options,
            documentUri = documentUri,
            languageId = languageId,
        )

        assertNotNull(bufferOnly)
        assertEquals(MixinAnnotation.INJECT, bufferOnly.context.annotation)
        assertEquals(AnnotationSlot.ATTRIBUTE, bufferOnly.context.slot)
        assertTrue(bufferOnly.result.items.any { it.filterText == "method" })
        assertEquals(0, resolverCalls)
    }

    @Test
    fun atHeadCompletionResolvesJavaProjectOnceForCustomAtCodeDiscovery() {
        var resolverCalls = 0
        var providerCalls = 0
        val facade = MixinServiceFacade(
            javaProjectResolver = {
                resolverCalls++
                fakeJavaProject
            },
            atCodeValuesProvider = { project, version ->
                assertEquals(fakeJavaProject, project)
                assertEquals(0L, version)
                providerCalls++
                emptyList()
            },
        )
        val source = """@Inject(method = "tick", at = @At(value = "HE"))"""
        val (line, character) = cursorInsideAtValue(source, "HE")

        val bufferOnly = facade.tryCompleteBufferOnly(
            source = source,
            line = line,
            character = character,
            options = options,
            documentUri = documentUri,
            languageId = languageId,
        )

        assertNotNull(bufferOnly)
        assertEquals(MixinAnnotation.AT, bufferOnly.context.annotation)
        assertEquals(AnnotationSlot.VALUE, bufferOnly.context.slot)
        assertTrue(bufferOnly.result.items.any { it.insertText == "HEAD" })
        assertEquals(1, resolverCalls)
        assertEquals(1, providerCalls)
    }

    @Test
    fun atValueCompletionIncludesCustomAtCodeValuesFromProvider() {
        var resolverCalls = 0
        var providerCalls = 0
        val facade = MixinServiceFacade(
            javaProjectResolver = {
                resolverCalls++
                fakeJavaProject
            },
            atCodeValuesProvider = { project, version ->
                assertEquals(fakeJavaProject, project)
                assertEquals(0L, version)
                providerCalls++
                listOf("MyMod:Hook", "com.example.CustomPoint")
            },
        )
        val source = """@Inject(method = "tick", at = @At(value = ""))"""
        val (line, character) = cursorInsideAtValue(source, "")

        val bufferOnly = facade.tryCompleteBufferOnly(
            source = source,
            line = line,
            character = character,
            options = options,
            documentUri = documentUri,
            languageId = languageId,
        )

        assertNotNull(bufferOnly)
        assertTrue(bufferOnly.result.items.any { it.insertText == "MyMod:Hook" })
        assertTrue(bufferOnly.result.items.any { it.insertText == "com.example.CustomPoint" })
        assertEquals(1, resolverCalls)
        assertEquals(1, providerCalls)
    }

    @Test
    fun atSpecifierCompletionWorksOnCustomAtCodeFromProvider() {
        var lookupCalls = 0
        val probe = JdtInjectionPointFeatureProbe { project, fqn ->
            assertEquals(fakeJavaProject, project)
            assertEquals(specifierFqn, fqn)
            lookupCalls++
            Any()
        }
        val facade = MixinServiceFacade(
            featureProbe = probe,
            javaProjectResolver = { fakeJavaProject },
            atCodeValuesProvider = { _, _ -> listOf("CUSTOMPROBE") },
        )
        val source = """@Inject(method = "tick", at = @At(value = "CUSTOMPROBE:"))"""
        val (line, character) = cursorInsideAtValue(source, "CUSTOMPROBE:")

        val bufferOnly = facade.tryCompleteBufferOnly(
            source = source,
            line = line,
            character = character,
            options = options,
            documentUri = documentUri,
            languageId = languageId,
        )

        assertNotNull(bufferOnly)
        assertEquals(
            listOf("CUSTOMPROBE:FIRST", "CUSTOMPROBE:LAST", "CUSTOMPROBE:ONE", "CUSTOMPROBE:ALL", "CUSTOMPROBE:DEFAULT"),
            bufferOnly.result.items.map { it.insertText },
        )
        assertEquals(1, lookupCalls)
    }

    @Test
    fun atSpecifierCompletionUsesLazyJavaProjectResolver() {
        var resolverCalls = 0
        var lookupCalls = 0
        val probe = JdtInjectionPointFeatureProbe { project, fqn ->
            assertEquals(fakeJavaProject, project)
            assertEquals(specifierFqn, fqn)
            lookupCalls++
            Any()
        }
        val facade = MixinServiceFacade(
            featureProbe = probe,
            javaProjectResolver = {
                resolverCalls++
                fakeJavaProject
            },
        )
        val source = """@Inject(method = "tick", at = @At(value = "INVOKE:"))"""
        val (line, character) = cursorInsideAtValue(source, "INVOKE:")

        val bufferOnly = facade.tryCompleteBufferOnly(
            source = source,
            line = line,
            character = character,
            options = options,
            documentUri = documentUri,
            languageId = languageId,
        )

        assertNotNull(bufferOnly)
        assertEquals(
            listOf("INVOKE:FIRST", "INVOKE:LAST", "INVOKE:ONE", "INVOKE:ALL", "INVOKE:DEFAULT"),
            bufferOnly.result.items.map { it.insertText },
        )
        assertEquals(1, resolverCalls)
        assertEquals(1, lookupCalls)
    }

    @Test
    fun atTargetAndMixinClassReturnNull() {
        val facade = MixinServiceFacade()
        var resolverCalls = 0
        val trackingFacade = MixinServiceFacade(
            javaProjectResolver = {
                resolverCalls++
                fakeJavaProject
            },
        )

        val atTargetSource = """@Inject(method = "tick", at = @At(value = "INVOKE", target = "Lnet/minecraft/"))"""
        val atTargetPartial = "Lnet/minecraft/"
        val (atTargetLine, atTargetCharacter) = positionAt(
            atTargetSource,
            atTargetSource.indexOf(atTargetPartial) + atTargetPartial.length,
        )
        assertNull(
            trackingFacade.tryCompleteBufferOnly(
                source = atTargetSource,
                line = atTargetLine,
                character = atTargetCharacter,
                options = options,
                documentUri = documentUri,
                languageId = languageId,
            ),
        )

        val mixinClassSource = """@Mixin(MinecraftClient.class)"""
        val mixinPartial = "MinecraftClient"
        val (mixinLine, mixinCharacter) = positionAt(
            mixinClassSource,
            mixinClassSource.indexOf(mixinPartial) + mixinPartial.length,
        )
        assertNull(
            facade.tryCompleteBufferOnly(
                source = mixinClassSource,
                line = mixinLine,
                character = mixinCharacter,
                options = options,
                documentUri = documentUri,
                languageId = languageId,
            ),
        )
        assertEquals(0, resolverCalls)
    }

    @Test
    fun customFacadeFactoryReturnsNull() {
        var factoryCalls = 0
        val facade = MixinServiceFacade(
            facadeFactory = { classIndex, bytecodeIndex ->
                factoryCalls++
                CoreMixinServiceFacade(classIndex = classIndex, bytecodeIndex = bytecodeIndex)
            },
        )
        val source = """@Inject(meth"""
        val (line, character) = positionAt(source, source.indexOf("meth") + "meth".length)

        assertNull(
            facade.tryCompleteBufferOnly(
                source = source,
                line = line,
                character = character,
                options = options,
                documentUri = documentUri,
                languageId = languageId,
            ),
        )
        assertEquals(0, factoryCalls)
    }

    @Test
    fun expressionReturnKeywordCompletionWorksWithoutJavaProjectResolver() {
        var resolverCalls = 0
        val facade = MixinServiceFacade(
            javaProjectResolver = {
                resolverCalls++
                fakeJavaProject
            },
        )
        val source = """@Expression("{ ret")"""
        val tokenStart = source.indexOf("ret")
        val (line, character) = positionAt(source, tokenStart + "ret".length)

        val bufferOnly = facade.tryCompleteBufferOnly(
            source = source,
            line = line,
            character = character,
            options = options,
            documentUri = documentUri,
            languageId = languageId,
        )

        assertNotNull(bufferOnly)
        assertEquals(MixinAnnotation.EXPRESSION, bufferOnly.context.annotation)
        assertEquals(AnnotationSlot.VALUE, bufferOnly.context.slot)
        assertTrue(bufferOnly.result.items.any { it.insertText == "return" })
        assertEquals(0, resolverCalls)
    }

    @Test
    fun expressionsArrayValueNullKeywordCompletionWorksWithoutJavaProjectResolver() {
        var resolverCalls = 0
        val facade = MixinServiceFacade(
            javaProjectResolver = {
                resolverCalls++
                fakeJavaProject
            },
        )
        val source = """@Expressions(value = { "nu" })"""
        val tokenStart = source.indexOf("nu")
        val (line, character) = positionAt(source, tokenStart + "nu".length)

        val bufferOnly = facade.tryCompleteBufferOnly(
            source = source,
            line = line,
            character = character,
            options = options,
            documentUri = documentUri,
            languageId = languageId,
        )

        assertNotNull(bufferOnly)
        assertEquals(MixinAnnotation.EXPRESSIONS, bufferOnly.context.annotation)
        assertEquals(AnnotationSlot.VALUE, bufferOnly.context.slot)
        assertTrue(bufferOnly.result.items.any { it.insertText == "null" })
        assertEquals(0, resolverCalls)
    }

    @Test
    fun definitionIdCompletionWorksWithoutJavaProjectResolver() {
        var resolverCalls = 0
        val facade = MixinServiceFacade(
            javaProjectResolver = {
                resolverCalls++
                fakeJavaProject
            },
        )
        val source = definitionIdHandlerSource(expressionValue = "len")
        val tokenStart = source.indexOf("\"len\"") + 1
        val (line, character) = positionAt(source, tokenStart + "len".length)

        val bufferOnly = facade.tryCompleteBufferOnly(
            source = source,
            line = line,
            character = character,
            options = options,
            documentUri = documentUri,
            languageId = languageId,
        )

        assertNotNull(bufferOnly)
        assertEquals(MixinAnnotation.EXPRESSION, bufferOnly.context.annotation)
        assertEquals(AnnotationSlot.VALUE, bufferOnly.context.slot)
        val definitionIds = bufferOnly.result.items
            .filter { it.metadata.source == "mixinextras.definitionId" }
            .map { it.insertText }
        assertEquals(listOf("lengthCall"), definitionIds)
        assertEquals(0, resolverCalls)
    }

    @Test
    fun definitionIdCompletionDoesNotLeakAdjacentHandlerDefinitions() {
        var resolverCalls = 0
        val facade = MixinServiceFacade(
            javaProjectResolver = {
                resolverCalls++
                fakeJavaProject
            },
        )
        val source = adjacentDefinitionIdHandlerSource()
        val tokenStart = source.indexOf("\"len\"") + 1
        val (line, character) = positionAt(source, tokenStart + "len".length)

        val bufferOnly = facade.tryCompleteBufferOnly(
            source = source,
            line = line,
            character = character,
            options = options,
            documentUri = documentUri,
            languageId = languageId,
        )

        assertNotNull(bufferOnly)
        val definitionIds = bufferOnly.result.items
            .filter { it.metadata.source == "mixinextras.definitionId" }
            .map { it.insertText }
        assertEquals(listOf("lengthCall"), definitionIds)
        assertEquals(0, resolverCalls)
    }

    @Test
    fun expressionIdAndAfterDotReturnNull() {
        var resolverCalls = 0
        val facade = MixinServiceFacade(
            javaProjectResolver = {
                resolverCalls++
                fakeJavaProject
            },
        )

        val idSource = """@Expression(id = "main")"""
        val idCursor = idSource.indexOf("main") + 2
        val (idLine, idCharacter) = positionAt(idSource, idCursor)
        assertNull(
            facade.tryCompleteBufferOnly(
                source = idSource,
                line = idLine,
                character = idCharacter,
                options = options,
                documentUri = documentUri,
                languageId = languageId,
            ),
        )

        val afterDotSource = """@Expression("this.")"""
        val afterDotCursor = afterDotSource.indexOf('.') + 1
        val (afterDotLine, afterDotCharacter) = positionAt(afterDotSource, afterDotCursor)
        assertNull(
            facade.tryCompleteBufferOnly(
                source = afterDotSource,
                line = afterDotLine,
                character = afterDotCharacter,
                options = options,
                documentUri = documentUri,
                languageId = languageId,
            ),
        )
        assertEquals(0, resolverCalls)
    }

    private fun definitionIdHandlerSource(
        expressionValue: String = "len",
        definitionId: String = "lengthCall",
    ): String = """
        package com.example.mixin;
        import com.example.target.SimpleTarget;
        import com.llamalad7.mixinextras.expression.Definition;
        import com.llamalad7.mixinextras.expression.Expression;
        import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
        import org.spongepowered.asm.mixin.Mixin;
        import org.spongepowered.asm.mixin.injection.At;
        @Mixin(SimpleTarget.class)
        public abstract class DefinitionIdMixin {
            @Definition(id = "$definitionId")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
            @Expression("$expressionValue")
            private float mcdev${'$'}handler(float original) { return original; }
        }
    """.trimIndent()

    private fun adjacentDefinitionIdHandlerSource(): String = """
        package com.example.mixin;
        import com.example.target.SimpleTarget;
        import com.llamalad7.mixinextras.expression.Definition;
        import com.llamalad7.mixinextras.expression.Expression;
        import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
        import org.spongepowered.asm.mixin.Mixin;
        import org.spongepowered.asm.mixin.injection.At;
        @Mixin(SimpleTarget.class)
        public abstract class AdjacentDefinitionIdMixin {
            @Definition(id = "lengthCall")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
            @Expression("len")
            private float mcdev${'$'}handler1(float original) { return original; }

            @Definition(id = "lengthOther")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
            @Expression("")
            private float mcdev${'$'}handler2(float original) { return original; }
        }
    """.trimIndent()

    private fun cursorInsideAtValue(source: String, partial: String): Pair<Int, Int> {
        val offset = source.indexOf("\"$partial\"") + 1 + partial.length
        return positionAt(source, offset)
    }

    private fun positionAt(source: String, offset: Int): Pair<Int, Int> {
        val before = source.substring(0, offset)
        val line = before.count { it == '\n' }
        val lastNewline = before.lastIndexOf('\n')
        val character = if (lastNewline < 0) offset else offset - lastNewline - 1
        return line to character
    }
}
