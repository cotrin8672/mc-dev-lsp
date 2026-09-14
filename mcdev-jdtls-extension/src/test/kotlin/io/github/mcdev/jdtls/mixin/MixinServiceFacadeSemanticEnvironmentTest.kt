package io.github.mcdev.jdtls.mixin

import io.github.mcdev.core.mixin.MixinCompletionOptions
import io.github.mcdev.core.mixin.MixinSemanticModelParser
import io.github.mcdev.core.project.ClasspathSnapshot
import io.github.mcdev.core.project.ProjectContextBuilder
import io.github.mcdev.core.project.SourceSetContext
import io.github.mcdev.jdtls.project.McdevProjectSession
import io.github.mcdev.jdtls.project.UriPathSupport
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Files
import java.nio.file.Path

class MixinServiceFacadeSemanticEnvironmentTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun completeDefaultOverloadUsesSessionSemanticEnvironment() {
        val fixture = sessionFixture()
        val observedEnvironments = mutableListOf<JdtParseEnvironment>()
        val facade = MixinServiceFacade(
            semanticModelProvider = { source, documentUri, environment ->
                observedEnvironments += environment
                MixinSemanticModelParser.parse(source, documentUri)
            },
            completeOverride = { _, _, _, _, _ -> emptyList() },
        )

        facade.complete(
            session = fixture.session,
            source = fixture.source,
            line = 0,
            character = 0,
            options = MixinCompletionOptions(),
            documentUri = fixture.documentUri,
        )

        assertSingleSessionEnvironment(observedEnvironments, fixture)
    }

    @Test
    fun definitionsUsesSessionSemanticEnvironment() {
        val fixture = sessionFixture()
        val observedEnvironments = mutableListOf<JdtParseEnvironment>()
        val facade = MixinServiceFacade(
            semanticModelProvider = { source, documentUri, environment ->
                observedEnvironments += environment
                MixinSemanticModelParser.parse(source, documentUri)
            },
        )

        facade.definitions(
            session = fixture.session,
            source = fixture.source,
            line = 0,
            character = 0,
            documentUri = fixture.documentUri,
        )

        assertSingleSessionEnvironment(observedEnvironments, fixture)
    }

    private data class SessionFixture(
        val session: McdevProjectSession,
        val documentUri: String,
        val source: String,
        val expectedClasspath: List<String>,
        val expectedSourcepath: List<String>,
        val expectedUnitName: String,
    )

    private fun sessionFixture(): SessionFixture {
        val sourceRoot = tempDir.resolve("src/main/java")
        Files.createDirectories(sourceRoot)
        val context = ProjectContextBuilder.empty("semantic-env-test", tempDir).copy(
            classpath = ClasspathSnapshot(projectOutputs = listOf(tempDir)),
            sourceSets = listOf(
                SourceSetContext(
                    name = "main",
                    sourceDirectories = listOf(sourceRoot),
                ),
            ),
        )
        val session = McdevProjectSession.create(context)
        val documentUri = UriPathSupport.pathToUri(sourceRoot.resolve("com/example/Mixin.java"))
        val expectedClasspath = session.context.classpath.allEntries.map { it.toString() }
        val expectedSourcepath = session.context.sourceSets.flatMap { sourceSet ->
            sourceSet.sourceDirectories.map { it.toString() }
        }
        val expectedUnitName = "com/example/Mixin.java"
        return SessionFixture(
            session = session,
            documentUri = documentUri,
            source = """
                package com.example;
                import org.spongepowered.asm.mixin.Mixin;
                @Mixin(Object.class)
                public abstract class Mixin {}
            """.trimIndent(),
            expectedClasspath = expectedClasspath,
            expectedSourcepath = expectedSourcepath,
            expectedUnitName = expectedUnitName,
        )
    }

    private fun assertSingleSessionEnvironment(
        observedEnvironments: List<JdtParseEnvironment>,
        fixture: SessionFixture,
    ) {
        assertEquals(1, observedEnvironments.size, "SUT must invoke semanticModelProvider exactly once")
        val environment = observedEnvironments.single()
        assertEquals(fixture.expectedClasspath, environment.classpathEntries)
        assertEquals(fixture.expectedSourcepath, environment.sourcepathEntries)
        assertNotNull(fixture.expectedUnitName)
        assertEquals(fixture.expectedUnitName, environment.unitName)
    }
}
