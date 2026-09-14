package io.github.mcdev.jdtls.project

import io.github.mcdev.core.project.ClasspathSnapshot
import io.github.mcdev.core.project.ProjectContextBuilder
import io.github.mcdev.core.project.ProjectIndexState
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Files
import java.nio.file.Path
import javax.tools.ToolProvider

class McdevProjectSessionTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun createAndReindexStayLazyUntilMemberQuery() {
        val ownerA = "demo/Alpha"
        val ownerB = "demo/Beta"
        writeClass(ownerA, compileClass("Alpha", "public int value; public void run() {}"))
        writeClass(ownerB, compileClass("Beta", "public String name; public void go() {}"))

        val context = ProjectContextBuilder.empty("session-test", tempDir).copy(
            classpath = ClasspathSnapshot(projectOutputs = listOf(tempDir)),
        )

        val session = McdevProjectSession.create(context)
        assertEquals(ProjectIndexState.READY, session.context.indexState)
        assertStartupLazy(session)

        val reindexed = session.reindex()
        assertEquals(ProjectIndexState.READY, reindexed.context.indexState)
        assertStartupLazy(reindexed)

        session.classIndex.findClasses("demo", 10)
        session.classIndex.findClass(ownerA)
        session.classIndex.findClassByFqn("demo.Beta")
        assertEquals(0, session.classBytesProvider.byteReadCount())
        assertEquals(0, lazyIndex(session).asmParseCount())

        session.classIndex.getMethods(ownerA)
        assertEquals(1, session.classBytesProvider.byteReadCount())
        assertEquals(1, lazyIndex(session).asmParseCount())
        assertTrue(session.classIndex.getMethods(ownerA).any { it.name == "run" })

        reindexed.classIndex.findClasses("Alpha", 10)
        reindexed.classIndex.findClassByFqn("demo.Beta")
        assertEquals(0, reindexed.classBytesProvider.byteReadCount())
        assertEquals(0, lazyIndex(reindexed).asmParseCount())

        reindexed.classIndex.getMethods(ownerB)
        assertEquals(1, reindexed.classBytesProvider.byteReadCount())
        assertEquals(1, lazyIndex(reindexed).asmParseCount())
        assertTrue(reindexed.classIndex.getMethods(ownerB).any { it.name == "go" })
    }

    @Test
    fun createAndReindexWithEmptyClasspathStayNotReadyWithoutCatalogScan() {
        val context = ProjectContextBuilder.empty("session-test-empty", tempDir)

        val session = McdevProjectSession.create(context)
        assertEquals(ProjectIndexState.NOT_READY, session.context.indexState)
        assertStartupLazy(session)

        val reindexed = session.reindex()
        assertEquals(ProjectIndexState.NOT_READY, reindexed.context.indexState)
        assertStartupLazy(reindexed)
    }

    private fun assertStartupLazy(session: McdevProjectSession) {
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
            ?: error("Java compiler not available; cannot compile session fixture")
        val sourceDir = Files.createTempDirectory("mcdev-session-src")
        val outputDir = Files.createTempDirectory("mcdev-session-out")
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
            "failed to compile session fixture:${diagnostics.joinToString(prefix = System.lineSeparator())}"
        }
        fileManager.close()

        return Files.readAllBytes(outputDir.resolve("demo").resolve("$simpleName.class"))
    }
}
