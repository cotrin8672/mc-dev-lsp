package io.github.mcdev.core.mixin.e2e

import io.github.mcdev.core.bytecode.InMemoryClassBytesProvider
import io.github.mcdev.core.index.ProjectContextMixinIndex
import io.github.mcdev.core.mixin.FakeBytecodeIndex
import io.github.mcdev.core.mixin.FakeClassIndex
import io.github.mcdev.core.mixin.MixinFacadeRequest
import io.github.mcdev.core.mixin.MixinServiceFacade
import io.github.mcdev.core.project.ProjectContextBuilder
import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.fixtures.FixtureResourceLoader
import java.nio.file.Path

internal object MixinE2ETestSupport {
    const val SIMPLE_TARGET_INTERNAL = "com/example/target/SimpleTarget"

    fun loadFixtureText(path: String): String = FixtureResourceLoader.loadText(path)

    fun simpleTargetProvider(): InMemoryClassBytesProvider {
        val bytes = FixtureResourceLoader.loadBytes(FixturePaths.SIMPLE_TARGET_CLASS)
        return InMemoryClassBytesProvider(mapOf(SIMPLE_TARGET_INTERNAL to bytes))
    }

    fun fakeFacade(): MixinServiceFacade =
        MixinServiceFacade(FakeClassIndex(), FakeBytecodeIndex())

    fun simpleTargetFacade(): MixinServiceFacade {
        val provider = simpleTargetProvider()
        val index = ProjectContextMixinIndex()
        val context = ProjectContextBuilder.empty("e2e", Path.of("."))
        return MixinServiceFacade(
            classIndex = index.buildClassIndex(context, provider),
            bytecodeIndex = index.buildBytecodeIndex(context, provider),
        )
    }

    fun requestAt(source: String, needle: String, documentUri: String = "file:///Mixin.java"): MixinFacadeRequest {
        val offset = source.indexOf(needle)
        require(offset >= 0) { "needle not found: $needle" }
        return requestAtOffset(source, offset + needle.length, documentUri)
    }

    fun requestInAnnotationValue(
        source: String,
        annotationMarker: String,
        valuePrefix: String,
        documentUri: String = "file:///Mixin.java",
    ): MixinFacadeRequest {
        val annotationStart = source.indexOf(annotationMarker)
        require(annotationStart >= 0) { "annotation marker not found: $annotationMarker" }
        val valueStart = source.indexOf('"', annotationStart) + 1
        require(valueStart > annotationStart) { "annotation value quote not found after: $annotationMarker" }
        return requestAtOffset(source, valueStart + valuePrefix.length, documentUri)
    }

    fun requestAtOffset(
        source: String,
        offset: Int,
        documentUri: String = "file:///Mixin.java",
    ): MixinFacadeRequest {
        val (line, character) = offsetToLineCharacter(source, offset.coerceIn(0, source.length))
        return MixinFacadeRequest(
            bufferText = source,
            line = line,
            character = character,
            documentUri = documentUri,
        )
    }

    fun offsetToLineCharacter(source: String, offset: Int): Pair<Int, Int> {
        var line = 0
        var character = 0
        var i = 0
        while (i < offset && i < source.length) {
            if (source[i] == '\n') {
                line++
                character = 0
            } else {
                character++
            }
            i++
        }
        return line to character
    }
}
