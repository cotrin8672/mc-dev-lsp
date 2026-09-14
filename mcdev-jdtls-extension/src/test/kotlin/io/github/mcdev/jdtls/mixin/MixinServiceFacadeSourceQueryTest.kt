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
import io.github.mcdev.core.project.MixinConfigRef
import io.github.mcdev.core.project.ProjectContextBuilder
import io.github.mcdev.core.project.SourceSetContext
import io.github.mcdev.jdtls.project.ClasspathClassBytesProvider
import io.github.mcdev.jdtls.project.LazyClasspathClassIndex
import io.github.mcdev.jdtls.project.McdevProjectSession
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import java.net.URI
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Files
import java.nio.file.Path
import javax.tools.ToolProvider

class MixinServiceFacadeSourceQueryTest {
    @TempDir
    lateinit var tempDir: Path

    private val documentUri = "file:///ExampleMixin.java"
    private val options = MixinCompletionOptions()
    private val fakeJavaProject = Any()
    private val targetInternalName = "demo/TargetTarget"
    private val targetEntry = ClassIndexEntry(
        simpleName = "TargetTarget",
        packageName = "demo",
        internalName = targetInternalName,
    )
    private val tickMethod = MethodIndexEntry(
        name = "tick",
        descriptor = "()V",
        isStatic = false,
        readableSignature = "tick(): void",
    )

    @Test
    fun mixinClassAndInjectMethodCompletionUseAuthoritativeSourceQueryWithoutClasspath() {
        var findClassesCalls = 0
        var getMethodsCalls = 0
        val fakeQuery = authoritativeFakeQuery(
            onFindClasses = { findClassesCalls++ },
            onGetMethods = { getMethodsCalls++ },
        )
        val facade = facadeWithFakeQuery(fakeQuery)
        val session = sessionWithClasspath()

        for (prefix in listOf("T", "Ta", "Tar")) {
            val source = mixinClassSource(prefix)
            val marker = "@Mixin($prefix"
            val (line, character) = positionAt(source, source.indexOf(marker) + marker.length)
            val items = facade.complete(
                session = session,
                source = source,
                line = line,
                character = character,
                options = options,
                documentUri = documentUri,
                semanticModel = facade.semanticModel(source, documentUri),
            )
            assertTrue(items.any { it.label == "TargetTarget" }, "prefix=$prefix")
            assertClasspathUntouched(session)
        }

        for (prefix in listOf("t", "ti")) {
            val source = injectMethodSource(prefix)
            val marker = "method = \"$prefix"
            val (line, character) = positionAt(source, source.indexOf(marker) + marker.length)
            val items = facade.complete(
                session = session,
                source = source,
                line = line,
                character = character,
                options = options,
                documentUri = documentUri,
                semanticModel = facade.semanticModel(source, documentUri),
            )
            assertTrue(items.any { it.insertText == "tick" }, "prefix=$prefix")
            assertClasspathUntouched(session)
        }

        assertTrue(findClassesCalls >= 3)
        assertTrue(getMethodsCalls >= 2)
    }

    @Test
    fun customFacadeFactoryRemainsCompatibleWithProjectSourceQueryFactory() {
        var factoryCalls = 0
        val trackingFacade = CoreMixinServiceFacade(
            classIndex = emptyClassIndex(),
            bytecodeIndex = emptyBytecodeIndex(),
        )
        val facade = MixinServiceFacade(
            facadeFactory = { classIndex, bytecodeIndex ->
                factoryCalls++
                assertTrue(classIndex.findClass(targetInternalName) != null)
                assertTrue(bytecodeIndex.getAtTargetCandidates("ignored", "run", "()V", "INVOKE").isEmpty())
                trackingFacade
            },
            javaProjectResolver = { fakeJavaProject },
            projectSourceQueryFactory = { authoritativeFakeQuery() },
        )
        val source = mixinClassSource("Tar")
        val marker = "@Mixin(Tar"
        val (line, character) = positionAt(source, source.indexOf(marker) + marker.length)
        facade.complete(
            session = sessionWithClasspath(),
            source = source,
            line = line,
            character = character,
            options = options,
            documentUri = documentUri,
            semanticModel = facade.semanticModel(source, documentUri),
        )

        assertEquals(1, factoryCalls)
    }

    @Test
    fun registeredMixinSourcesPreferJdtAndTrackSavedDeletedFiles() {
        val sourceDirectory = tempDir.resolve("src/main/java")
        val resourceDirectory = tempDir.resolve("src/main/resources")
        val currentPath = sourceDirectory.resolve("com/example/mixin/CurrentMixin.java")
        val otherPath = sourceDirectory.resolve("com/example/mixin/OtherMixin.java")
        val outerPath = sourceDirectory.resolve("com/example/mixin/Outer.java")
        val configPath = resourceDirectory.resolve("mixins.json")
        Files.createDirectories(currentPath.parent)
        Files.createDirectories(configPath.parent)

        val currentSource = "package com.example.mixin; class CurrentMixin {}"
        val savedSource = "package com.example.mixin; class OtherMixin { int saved; }"
        val updatedSource = "package com.example.mixin; class OtherMixin { int updated; }"
        val outerSource = "package com.example.mixin; class Outer { class Nested {} }"
        Files.writeString(currentPath, currentSource)
        Files.writeString(otherPath, savedSource)
        Files.writeString(outerPath, outerSource)
        Files.writeString(configPath, mixinConfigJson())

        val context = ProjectContextBuilder.empty("share-source-test", tempDir).copy(
            sourceSets = listOf(SourceSetContext("main", listOf(sourceDirectory), listOf(resourceDirectory))),
            mixinConfigs = listOf(MixinConfigRef(configPath)),
        )
        val facade = MixinServiceFacade()
        val collectMethod = MixinServiceFacade::class.java.getDeclaredMethod(
            "collectRegisteredMixinSources",
            io.github.mcdev.core.project.ProjectContext::class.java,
            String::class.java,
            String::class.java,
            Any::class.java,
        ).apply { trySetAccessible() }
        fun collect(javaProject: Any? = null): List<String> {
            @Suppress("UNCHECKED_CAST")
            return (collectMethod.invoke(facade, context, currentSource, currentPath.toUri().toString(), javaProject)
                as Sequence<String>).toList()
        }

        assertEquals(listOf(savedSource, outerSource), collect())
        Files.writeString(otherPath, updatedSource)
        assertEquals(listOf(updatedSource, outerSource), collect())
        Files.delete(otherPath)
        assertEquals(listOf(outerSource), collect())
        Files.delete(configPath)
        assertTrue(collect().isEmpty())

        Files.writeString(configPath, mixinConfigJson())
        val unsavedSource = "package com.example.mixin; class OtherMixin { int unsaved; }"
        val javaProject = FakeJavaProject(
            mapOf(
                "com.example.mixin.CurrentMixin${'$'}Nested" to FakeJavaType(
                    FakeCompilationUnit("stale current source", FakeResource(currentPath.toUri())),
                ),
                "com.example.mixin.OtherMixin" to FakeJavaType(
                    FakeCompilationUnit(unsavedSource, FakeResource(otherPath.toUri())),
                ),
            ),
        )
        assertEquals(listOf(unsavedSource, outerSource), collect(javaProject))
    }

    private fun mixinConfigJson(): String =
        """
        {
          "package": "com.example.mixin",
          "mixins": ["CurrentMixin", "CurrentMixin${'$'}Nested", "OtherMixin", "Outer${'$'}Nested"]
        }
        """.trimIndent()

    private fun facadeWithFakeQuery(fakeQuery: ProjectSourceQuery): MixinServiceFacade =
        MixinServiceFacade(
            javaProjectResolver = { fakeJavaProject },
            projectSourceQueryFactory = { project ->
                assertEquals(fakeJavaProject, project)
                fakeQuery
            },
        )

    private fun authoritativeFakeQuery(
        onFindClasses: () -> Unit = {},
        onGetMethods: () -> Unit = {},
    ): ProjectSourceQuery =
        object : ProjectSourceQuery {
            override fun coversProjectDependencies(): Boolean = true

            override fun findClasses(prefix: String, limit: Int): Sequence<ClassIndexEntry> {
                onFindClasses()
                return sequenceOf(targetEntry).filter {
                    it.simpleName.startsWith(prefix, ignoreCase = true) ||
                        it.fqn.startsWith(prefix, ignoreCase = true)
                }.take(limit)
            }

            override fun findClass(internalName: String): ClassIndexEntry? =
                targetEntry.takeIf { it.internalName == internalName }

            override fun findClassByFqn(fqn: String): ClassIndexEntry? =
                targetEntry.takeIf { it.fqn == fqn }

            override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> {
                onGetMethods()
                return if (ownerInternalName == targetInternalName) listOf(tickMethod) else emptyList()
            }
        }

    private fun mixinClassSource(classPrefix: String): String =
        """
        package com.example.mixin;

        import org.spongepowered.asm.mixin.Mixin;

        @Mixin($classPrefix
        """.trimIndent()

    private fun injectMethodSource(methodPrefix: String): String =
        """
        @Mixin(demo.TargetTarget.class)
        class ExampleMixin {
            @Inject(method = "$methodPrefix")
        }
        """.trimIndent()

    private fun positionAt(source: String, offset: Int): Pair<Int, Int> {
        val before = source.substring(0, offset)
        val line = before.count { it == '\n' }
        val lastNewline = before.lastIndexOf('\n')
        val character = if (lastNewline < 0) offset else offset - lastNewline - 1
        return line to character
    }

    private fun sessionWithClasspath(): McdevProjectSession {
        writeClass(targetInternalName, compileClass("TargetTarget", "public void tick() {}"))
        val context = ProjectContextBuilder.empty("source-query-test", tempDir).copy(
            classpath = ClasspathSnapshot(projectOutputs = listOf(tempDir)),
        )
        return McdevProjectSession.create(context)
    }

    private fun assertClasspathUntouched(session: McdevProjectSession) {
        assertEquals(0, session.classBytesProvider.byteReadCount())
        assertFalse(catalogDelegateInitialized(session.classBytesProvider))
        assertEquals(0, lazyIndex(session).asmParseCount())
    }

    private fun catalogDelegateInitialized(provider: ClasspathClassBytesProvider): Boolean {
        val delegateField = ClasspathClassBytesProvider::class.java.getDeclaredField("catalog\$delegate")
        delegateField.isAccessible = true
        @Suppress("UNCHECKED_CAST")
        val lazy = delegateField.get(provider) as Lazy<*>
        return lazy.isInitialized()
    }

    private fun lazyIndex(session: McdevProjectSession): LazyClasspathClassIndex =
        session.classIndex as LazyClasspathClassIndex

    private fun writeClass(internalName: String, classBytes: ByteArray) {
        val classFile = tempDir.resolve("$internalName.class")
        Files.createDirectories(classFile.parent)
        Files.write(classFile, classBytes)
    }

    private fun compileClass(simpleName: String, members: String): ByteArray {
        val compiler = ToolProvider.getSystemJavaCompiler()
            ?: error("Java compiler not available; cannot compile source-query fixture")
        val sourceDir = Files.createTempDirectory("mcdev-source-query-src")
        val outputDir = Files.createTempDirectory("mcdev-source-query-out")
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
            "failed to compile source-query fixture:${diagnostics.joinToString(prefix = System.lineSeparator())}"
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

private class FakeJavaProject(private val types: Map<String, FakeJavaType>) {
    fun findType(fqn: String): FakeJavaType? = types[fqn]
}

private class FakeJavaType(private val compilationUnit: FakeCompilationUnit) {
    fun getCompilationUnit(): FakeCompilationUnit = compilationUnit
}

private class FakeCompilationUnit(
    private val source: String,
    private val resource: FakeResource,
) {
    fun getResource(): FakeResource = resource

    fun getSource(): String = source
}

private class FakeResource(private val locationUri: URI) {
    fun getLocationURI(): URI = locationUri
}
