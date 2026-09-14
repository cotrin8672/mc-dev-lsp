package io.github.mcdev.jdtls.project

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Files
import java.nio.file.Path
import javax.tools.ToolProvider

class LazyClasspathClassIndexTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun classSearchReadsNoBytesAndMemberQueriesIndexLazilyPerOwner() {
        val ownerA = "demo/Alpha"
        val ownerB = "demo/Beta"
        writeClass(ownerA, compileClass("Alpha", "public int value; public void run() {}"))
        writeClass(ownerB, compileClass("Beta", "public String name; public void go() {}"))

        val provider = ClasspathClassBytesProvider(listOf(tempDir))
        val index = LazyClasspathClassIndex(provider)

        index.findClasses("demo", 10)
        index.findClass(ownerA)
        index.findClassByFqn("demo.Alpha")
        assertEquals(0, provider.byteReadCount())
        assertEquals(0, index.asmParseCount())

        index.getMethods(ownerA)
        assertEquals(1, provider.byteReadCount())
        assertEquals(1, index.asmParseCount())
        assertTrue(index.getMethods(ownerA).any { it.name == "run" })

        index.getMethods(ownerA)
        index.getFields(ownerA)
        assertEquals(1, provider.byteReadCount())
        assertEquals(1, index.asmParseCount())
        assertTrue(index.getFields(ownerA).any { it.name == "value" })

        index.getMethods(ownerB)
        assertEquals(2, provider.byteReadCount())
        assertEquals(2, index.asmParseCount())

        index.getMethods("demo/Missing")
        index.getFields("demo/Missing")
        assertEquals(2, provider.byteReadCount())
        assertEquals(2, index.asmParseCount())
    }

    @Test
    fun exactOwnerMemberLookupBypassesProviderCatalog() {
        val owner = "demo/Alpha"
        writeClass(owner, compileClass("Alpha", "public void run() {}"))

        val provider = ClasspathClassBytesProvider(listOf(tempDir))
        val index = LazyClasspathClassIndex(provider)
        assertFalse(catalogDelegateInitialized(provider))

        index.getMethods(owner)
        assertFalse(catalogDelegateInitialized(provider))
        assertEquals(1, provider.byteReadCount())
        assertEquals(1, index.asmParseCount())

        index.getMethods("demo/Missing")
        index.getFields("demo/Missing")
        assertFalse(catalogDelegateInitialized(provider))
        assertEquals(1, provider.byteReadCount())
        assertEquals(1, index.asmParseCount())
    }

    @Test
    fun ownerIndexCacheStaysBounded() {
        val maxCachedOwners = 3
        repeat(maxCachedOwners + 2) { index ->
            val owner = "demo/Cache$index"
            writeClass(owner, compileClass("Cache$index", "public void m$index() {}"))
        }

        val provider = ClasspathClassBytesProvider(listOf(tempDir))
        val classIndex = LazyClasspathClassIndex(provider, maxCachedOwners = maxCachedOwners)

        repeat(maxCachedOwners + 2) { index ->
            classIndex.getMethods("demo/Cache$index")
        }

        assertEquals(maxCachedOwners, classIndex.cachedOwnerCount())
        assertEquals(maxCachedOwners + 2, classIndex.asmParseCount())
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
            ?: error("Java compiler not available; cannot compile lazy index fixture")
        val sourceDir = Files.createTempDirectory("mcdev-lazy-index-src")
        val outputDir = Files.createTempDirectory("mcdev-lazy-index-out")
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
            "failed to compile lazy index fixture:${diagnostics.joinToString(prefix = System.lineSeparator())}"
        }
        fileManager.close()

        return Files.readAllBytes(outputDir.resolve("demo").resolve("$simpleName.class"))
    }
}
