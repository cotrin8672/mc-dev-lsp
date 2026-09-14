package io.github.mcdev.jdtls.mixin

import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FieldIndexEntry
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.mixin.MixinCompletionOptions
import io.github.mcdev.core.mixin.MixinServiceFacade as CoreMixinServiceFacade
import io.github.mcdev.core.project.ClasspathSnapshot
import io.github.mcdev.core.project.ProjectContextBuilder
import io.github.mcdev.jdtls.project.ClasspathClassBytesProvider
import io.github.mcdev.jdtls.project.McdevProjectSession
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Files
import java.nio.file.Path
import javax.tools.ToolProvider

class MixinServiceFacadeAtSpecifierTest {
    @TempDir
    lateinit var tempDir: Path

    private val documentUri = "file:///ExampleMixin.java"
    private val options = MixinCompletionOptions()
    private val fakeJavaProject = Any()
    private val specifierFqn = "org.spongepowered.asm.mixin.injection.InjectionPoint\$Specifier"

    @Test
    fun atValueCompletionIncludesCustomAtCodeValuesFromProvider() {
        var providerCalls = 0
        val facade = MixinServiceFacade(
            featureProbe = injectionPointProbe { _, _ -> null },
            javaProjectResolver = { fakeJavaProject },
            atCodeValuesProvider = { project, version ->
                assertEquals(fakeJavaProject, project)
                assertEquals(3L, version)
                providerCalls++
                listOf("MyMod:Hook", "com.example.CustomPoint")
            },
        )
        val source = atValueSource("")
        val (line, character) = cursorAfterPartial(source, "")

        val items = facade.complete(
            session = sessionWithClasspath(),
            source = source,
            line = line,
            character = character,
            options = options,
            documentUri = documentUri,
            semanticModel = facade.semanticModel(source, documentUri),
            projectSessionVersion = 3L,
        )

        assertTrue(items.any { it.insertText == "MyMod:Hook" })
        assertTrue(items.any { it.insertText == "com.example.CustomPoint" })
        assertEquals(1, providerCalls)
    }

    @Test
    fun atValueCompletionPassesProjectSessionVersionToAtCodeProvider() {
        var capturedVersion: Long? = null
        val facade = MixinServiceFacade(
            featureProbe = injectionPointProbe { _, _ -> null },
            javaProjectResolver = { fakeJavaProject },
            atCodeValuesProvider = { project, version ->
                assertEquals(fakeJavaProject, project)
                capturedVersion = version
                listOf("MYPOINT")
            },
        )
        val source = atValueSource("MY")
        val (line, character) = cursorAfterPartial(source, "MY")

        facade.complete(
            session = sessionWithClasspath(),
            source = source,
            line = line,
            character = character,
            options = options,
            documentUri = documentUri,
            semanticModel = facade.semanticModel(source, documentUri),
            projectSessionVersion = 42L,
        )

        assertEquals(42L, capturedVersion)
    }

    @Test
    fun atSpecifierCompletionWorksOnCustomAtCodeFromProvider() {
        val facade = MixinServiceFacade(
            featureProbe = injectionPointProbe { _, _ -> Any() },
            javaProjectResolver = { fakeJavaProject },
            atCodeValuesProvider = { _, _ -> listOf("CUSTOMPROBE") },
        )
        val source = atValueSource("CUSTOMPROBE:")
        val (line, character) = cursorAfterPartial(source, "CUSTOMPROBE:")

        val items = facade.complete(
            session = sessionWithClasspath(),
            source = source,
            line = line,
            character = character,
            options = options,
            documentUri = documentUri,
            semanticModel = facade.semanticModel(source, documentUri),
        )

        assertEquals(
            listOf("CUSTOMPROBE:FIRST", "CUSTOMPROBE:LAST", "CUSTOMPROBE:ONE", "CUSTOMPROBE:ALL", "CUSTOMPROBE:DEFAULT"),
            items.map { it.insertText },
        )
    }

    @Test
    fun outsideSliceCodeWithPresentTypeYieldsSpecifiers() {
        var lookupCalls = 0
        val probe = injectionPointProbe { project, fqn ->
            assertEquals(fakeJavaProject, project)
            assertEquals(specifierFqn, fqn)
            lookupCalls++
            Any()
        }
        val facade = facadeWithProbe(probe)
        val source = atValueSource("INVOKE:")
        val (line, character) = cursorAfterPartial(source, "INVOKE:")

        val items = facade.complete(
            session = sessionWithClasspath(),
            source = source,
            line = line,
            character = character,
            options = options,
            documentUri = documentUri,
            semanticModel = facade.semanticModel(source, documentUri),
            projectSessionVersion = 3L,
        )

        assertEquals(
            listOf("INVOKE:FIRST", "INVOKE:LAST", "INVOKE:ONE", "INVOKE:ALL", "INVOKE:DEFAULT"),
            items.map { it.insertText },
        )
        assertEquals(1, lookupCalls)
    }

    @Test
    fun outsideSliceCodeWithMissingTypeYieldsNoSpecifiers() {
        var lookupCalls = 0
        val probe = injectionPointProbe { project, fqn ->
            assertEquals(fakeJavaProject, project)
            assertEquals(specifierFqn, fqn)
            lookupCalls++
            null
        }
        val facade = facadeWithProbe(probe)
        val source = atValueSource("INVOKE:")
        val (line, character) = cursorAfterPartial(source, "INVOKE:")

        val items = facade.complete(
            session = sessionWithClasspath(),
            source = source,
            line = line,
            character = character,
            options = options,
            documentUri = documentUri,
            semanticModel = facade.semanticModel(source, documentUri),
        )

        assertTrue(items.isEmpty())
        assertEquals(1, lookupCalls)
    }

    @Test
    fun nullJavaProjectFailsClosedWithoutLookup() {
        var lookupCalls = 0
        val probe = injectionPointProbe { _, _ ->
            lookupCalls++
            Any()
        }
        val facade = MixinServiceFacade(
            featureProbe = probe,
            javaProjectResolver = { null },
        )
        val source = atValueSource("INVOKE:")
        val (line, character) = cursorAfterPartial(source, "INVOKE:")

        val items = facade.complete(
            session = sessionWithClasspath(),
            source = source,
            line = line,
            character = character,
            options = options,
            documentUri = documentUri,
            semanticModel = facade.semanticModel(source, documentUri),
        )

        assertTrue(items.isEmpty())
        assertEquals(0, lookupCalls)
    }

    @Test
    fun ordinaryAtPartialDoesNotInvokeInjectionPointLookup() {
        var lookupCalls = 0
        val probe = injectionPointProbe { _, _ ->
            lookupCalls++
            Any()
        }
        val facade = facadeWithProbe(probe)
        val source = atValueSource("IN")
        val (line, character) = cursorAfterPartial(source, "IN")

        val items = facade.complete(
            session = sessionWithClasspath(),
            source = source,
            line = line,
            character = character,
            options = options,
            documentUri = documentUri,
            semanticModel = facade.semanticModel(source, documentUri),
        )

        assertTrue(items.any { it.insertText == "INVOKE" })
        assertEquals(0, lookupCalls)
    }

    @Test
    fun atSpecifierCompletionDoesNotTouchClasspathClassBytesProvider() {
        val probe = injectionPointProbe { _, _ -> Any() }
        val facade = facadeWithProbe(probe)
        val session = sessionWithClasspath()
        val source = atValueSource("INVOKE:")
        val (line, character) = cursorAfterPartial(source, "INVOKE:")

        facade.complete(
            session = session,
            source = source,
            line = line,
            character = character,
            options = options,
            documentUri = documentUri,
            semanticModel = facade.semanticModel(source, documentUri),
        )

        assertEquals(0, session.classBytesProvider.byteReadCount())
        assertFalse(catalogDelegateInitialized(session.classBytesProvider))
    }

    @Test
    fun customFacadeFactoryRemainsCompatibleWithoutProbeWiring() {
        var factoryCalls = 0
        var lookupCalls = 0
        val probe = injectionPointProbe { _, _ ->
            lookupCalls++
            Any()
        }
        val trackingFacade = CoreMixinServiceFacade(
            classIndex = emptyClassIndex(),
            bytecodeIndex = emptyBytecodeIndex(),
        )
        val facade = MixinServiceFacade(
            facadeFactory = { classIndex, bytecodeIndex ->
                factoryCalls++
                assertTrue(classIndex.findClass("ignored") == null)
                assertTrue(bytecodeIndex.getAtTargetCandidates("ignored", "run", "()V", "INVOKE").isEmpty())
                trackingFacade
            },
            featureProbe = probe,
            javaProjectResolver = { fakeJavaProject },
        )
        val source = atValueSource("INVOKE:")
        val (line, character) = cursorAfterPartial(source, "INVOKE:")

        val items = facade.complete(
            session = sessionWithClasspath(),
            source = source,
            line = line,
            character = character,
            options = options,
            documentUri = documentUri,
            semanticModel = facade.semanticModel(source, documentUri),
        )

        assertEquals(1, factoryCalls)
        assertEquals(0, lookupCalls)
        assertTrue(items.isEmpty())
    }

    private fun facadeWithProbe(probe: JdtInjectionPointFeatureProbe): MixinServiceFacade =
        MixinServiceFacade(
            featureProbe = probe,
            javaProjectResolver = { fakeJavaProject },
        )

    private fun injectionPointProbe(
        onLookup: (javaProject: Any, fqn: String) -> Any?,
    ): JdtInjectionPointFeatureProbe =
        JdtInjectionPointFeatureProbe { project, fqn -> onLookup(project, fqn) }

    private fun atValueSource(partial: String): String =
        """@Inject(method = "tick", at = @At(value = "$partial"))"""

    private fun cursorAfterPartial(source: String, partial: String): Pair<Int, Int> {
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

    private fun sessionWithClasspath(): McdevProjectSession {
        writeClass("demo/Target", compileClass("Target", "public void tick() {}"))
        val context = ProjectContextBuilder.empty("at-specifier-test", tempDir).copy(
            classpath = ClasspathSnapshot(projectOutputs = listOf(tempDir)),
        )
        return McdevProjectSession.create(context)
    }

    private fun catalogDelegateInitialized(provider: ClasspathClassBytesProvider): Boolean {
        val delegateField = ClasspathClassBytesProvider::class.java.getDeclaredField("catalog\$delegate")
        delegateField.isAccessible = true
        @Suppress("UNCHECKED_CAST")
        val lazy = delegateField.get(provider) as Lazy<*>
        return lazy.isInitialized()
    }

    private fun writeClass(internalName: String, classBytes: ByteArray) {
        val classFile = tempDir.resolve("$internalName.class")
        Files.createDirectories(classFile.parent)
        Files.write(classFile, classBytes)
    }

    private fun compileClass(simpleName: String, members: String): ByteArray {
        val compiler = ToolProvider.getSystemJavaCompiler()
            ?: error("Java compiler not available; cannot compile at-specifier fixture")
        val sourceDir = Files.createTempDirectory("mcdev-at-specifier-src")
        val outputDir = Files.createTempDirectory("mcdev-at-specifier-out")
        val sourceFile = sourceDir.resolve("demo").resolve("$simpleName.java")
        Files.createDirectories(sourceFile.parent)
        Files.writeString(
            sourceFile,
            """
            package demo;

            public class $simpleName {
                $members
            }
            """.trimIndent() + System.lineSeparator(),
        )

        val diagnostics = mutableListOf<String>()
        val fileManager = compiler.getStandardFileManager(null, null, null)
        val compilationUnits = fileManager.getJavaFileObjects(sourceFile.toFile())
        val task = compiler.getTask(
            null,
            fileManager,
            { diagnostic ->
                diagnostics += diagnostic.toString()
                true
            },
            listOf("-d", outputDir.toString(), "--release", "21"),
            null,
            compilationUnits,
        )
        check(task.call()) {
            "failed to compile at-specifier fixture:${diagnostics.joinToString(prefix = System.lineSeparator())}"
        }
        fileManager.close()

        return Files.readAllBytes(outputDir.resolve("demo").resolve("$simpleName.class"))
    }

    private fun emptyClassIndex(): ClassIndex =
        object : ClassIndex {
            override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> = emptyList()

            override fun findClass(internalName: String): ClassIndexEntry? = null

            override fun findClassByFqn(fqn: String): ClassIndexEntry? = null

            override fun getMethods(ownerInternalName: String) = emptyList<MethodIndexEntry>()

            override fun getFields(ownerInternalName: String) = emptyList<FieldIndexEntry>()
        }

    private fun emptyBytecodeIndex(): BytecodeIndex =
        object : BytecodeIndex {
            override fun getAtTargetCandidates(
                ownerInternalName: String,
                methodName: String,
                methodDescriptor: String?,
                atValue: String,
            ) = emptyList<AtTargetCandidate>()

            override fun getReturnOrdinalCount(
                ownerInternalName: String,
                methodName: String,
                methodDescriptor: String?,
            ): Int = 0
        }
}
