package io.github.mcdev.jdtls.project

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Files
import java.nio.file.Path
import java.util.concurrent.Callable
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import java.util.jar.JarEntry
import java.util.jar.JarOutputStream

class ClasspathClassBytesProviderTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun classCountDiscoversNamesWithoutReadingClassContents() {
        val jarPath = tempDir.resolve("aaa-lib.jar")
        writeJar(
            jarPath,
            linkedMapOf(
                "com/example/Alpha.class" to MINIMAL_CLASS_BYTES,
                "com/example/Beta.class" to MINIMAL_CLASS_BYTES,
                "com/example/Gamma.class" to MINIMAL_CLASS_BYTES,
            ),
        )

        val classesDir = tempDir.resolve("zzz-classes")
        val pkg = classesDir.resolve("com/example")
        Files.createDirectories(pkg)
        writeMinimalClassFile(pkg.resolve("Alpha.class"))
        writeMinimalClassFile(pkg.resolve("Beta.class"))

        val provider = ClasspathClassBytesProvider(listOf(classesDir, jarPath))
        assertEquals(0, provider.byteReadCount())
        assertFalse(catalogDelegateInitialized(provider))

        assertEquals(3, provider.classCount())
        assertEquals(0, provider.byteReadCount())
        assertTrue(catalogDelegateInitialized(provider))

        assertNotNull(provider.getClassBytes("com/example/Beta"))
        assertEquals(1, provider.byteReadCount())
    }

    @Test
    fun getClassBytesReadsLazilyAndCachesPerOwner() {
        val jarPath = tempDir.resolve("aaa-lib.jar")
        writeJar(
            jarPath,
            linkedMapOf(
                "com/example/Alpha.class" to taggedClassBytes('A'),
                "com/example/Beta.class" to taggedClassBytes('B'),
                "com/example/Gamma.class" to taggedClassBytes('G'),
            ),
        )

        val classesDir = tempDir.resolve("zzz-classes")
        val pkg = classesDir.resolve("com/example")
        Files.createDirectories(pkg)
        writeClassFile(pkg.resolve("Delta.class"), taggedClassBytes('D'))

        val provider = ClasspathClassBytesProvider(listOf(classesDir, jarPath))
        assertEquals(0, provider.byteReadCount())

        assertEquals(taggedClassBytes('A').toList(), provider.getClassBytes("com/example/Alpha")?.toList())
        assertEquals(1, provider.byteReadCount())

        assertEquals(taggedClassBytes('A').toList(), provider.getClassBytes("com/example/Alpha")?.toList())
        assertEquals(1, provider.byteReadCount())

        assertEquals(taggedClassBytes('B').toList(), provider.getClassBytes("com/example/Beta")?.toList())
        assertEquals(2, provider.byteReadCount())
    }

    @Test
    fun metadataQueriesDoNotReadClassBytes() {
        val jarPath = tempDir.resolve("aaa-lib.jar")
        writeJar(
            jarPath,
            linkedMapOf(
                "com/example/Alpha.class" to taggedClassBytes('A'),
                "com/example/Beta.class" to taggedClassBytes('B'),
            ),
        )

        val classesDir = tempDir.resolve("zzz-classes")
        val pkg = classesDir.resolve("com/example")
        Files.createDirectories(pkg)
        writeClassFile(pkg.resolve("Gamma.class"), taggedClassBytes('G'))

        val provider = ClasspathClassBytesProvider(listOf(classesDir, jarPath))
        assertEquals(3, provider.classCount())
        assertEquals(
            listOf("com/example/Alpha", "com/example/Beta"),
            provider.findInternalNames("com/example", 2),
        )
        assertEquals(
            listOf("com/example/Beta"),
            provider.findInternalNames("bet", 5),
        )
        assertEquals(0, provider.byteReadCount())
    }

    @Test
    fun getClassBytesAppliesDuplicateNameSemanticsAcrossEntries() {
        val firstDir = tempDir.resolve("aaa-classes")
        val secondDir = tempDir.resolve("zzz-classes")
        Files.createDirectories(firstDir.resolve("dup"))
        Files.createDirectories(secondDir.resolve("dup"))
        writeClassFile(firstDir.resolve("dup/Shared.class"), taggedClassBytes('1'))
        writeClassFile(firstDir.resolve("dup/FirstOnly.class"), MINIMAL_CLASS_BYTES)
        writeClassFile(secondDir.resolve("dup/Shared.class"), taggedClassBytes('2'))
        writeClassFile(secondDir.resolve("dup/SecondOnly.class"), MINIMAL_CLASS_BYTES)

        val provider = ClasspathClassBytesProvider(listOf(secondDir, firstDir))
        assertEquals(3, provider.classCount())

        assertEquals(
            taggedClassBytes('1').toList(),
            provider.getClassBytes("dup/Shared")?.toList(),
        )
        assertEquals(1, provider.byteReadCount())

        assertEquals(
            MINIMAL_CLASS_BYTES.toList(),
            provider.getClassBytes("dup/FirstOnly")?.toList(),
        )
        assertEquals(2, provider.byteReadCount())
    }

    @Test
    fun findInternalNamesAppliesDuplicateNameSemanticsAcrossEntries() {
        val firstDir = tempDir.resolve("aaa-classes")
        val secondDir = tempDir.resolve("zzz-classes")
        Files.createDirectories(firstDir.resolve("dup"))
        Files.createDirectories(secondDir.resolve("dup"))
        writeMinimalClassFile(firstDir.resolve("dup/Shared.class"))
        writeMinimalClassFile(firstDir.resolve("dup/FirstOnly.class"))
        writeMinimalClassFile(secondDir.resolve("dup/Shared.class"))
        writeMinimalClassFile(secondDir.resolve("dup/SecondOnly.class"))

        val provider = ClasspathClassBytesProvider(listOf(secondDir, firstDir))
        assertEquals(
            listOf("dup/FirstOnly", "dup/SecondOnly", "dup/Shared"),
            provider.findInternalNames("dup/", 10),
        )
        assertEquals(3, provider.classCount())
        assertEquals(0, provider.byteReadCount())
    }

    @Test
    fun concurrentGetClassBytesForSameOwnerReadsClassBytesOnce() {
        val classesDir = tempDir.resolve("classes")
        val pkg = classesDir.resolve("com/example")
        Files.createDirectories(pkg)
        val expectedBytes = taggedClassBytes('S')
        writeClassFile(pkg.resolve("Shared.class"), expectedBytes)

        val provider = ClasspathClassBytesProvider(listOf(classesDir))
        assertEquals(0, provider.byteReadCount())

        val owner = "com/example/Shared"
        val taskCount = 32
        val startGate = CountDownLatch(1)
        val executor = Executors.newFixedThreadPool(taskCount)
        try {
            val futures = (0 until taskCount).map {
                executor.submit(
                    Callable {
                        startGate.await(10, TimeUnit.SECONDS)
                        provider.getClassBytes(owner)
                    },
                )
            }
            startGate.countDown()
            val results = futures.map { it.get(30, TimeUnit.SECONDS) }
            results.forEach { bytes ->
                assertNotNull(bytes)
                assertEquals(expectedBytes.toList(), bytes.toList())
            }
            assertEquals(1, provider.byteReadCount())
        } finally {
            executor.shutdownNow()
            executor.awaitTermination(5, TimeUnit.SECONDS)
        }
    }

    @Test
    fun acceptsInjectedBytesReadCounter() {
        val counter = AtomicInteger(0)
        val classesDir = tempDir.resolve("classes")
        Files.createDirectories(classesDir.resolve("demo"))
        writeMinimalClassFile(classesDir.resolve("demo/Sample.class"))

        val provider = ClasspathClassBytesProvider(listOf(classesDir), bytesReadCounter = counter)
        assertEquals(0, counter.get())

        provider.getClassBytes("demo/Sample")
        assertEquals(1, counter.get())
        assertEquals(1, provider.byteReadCount())
    }

    private fun writeMinimalClassFile(path: Path) {
        writeClassFile(path, MINIMAL_CLASS_BYTES)
    }

    private fun writeClassFile(path: Path, bytes: ByteArray) {
        Files.createDirectories(path.parent)
        Files.write(path, bytes)
    }

    private fun writeJar(path: Path, entries: Map<String, ByteArray>) {
        Files.createDirectories(path.parent)
        JarOutputStream(Files.newOutputStream(path)).use { jar ->
            entries.forEach { (name, bytes) ->
                jar.putNextEntry(JarEntry(name))
                jar.write(bytes)
                jar.closeEntry()
            }
        }
    }

    private fun catalogDelegateInitialized(provider: ClasspathClassBytesProvider): Boolean =
        lazyDelegateInitialized(provider, "catalog\$delegate")

    private fun lazyDelegateInitialized(provider: ClasspathClassBytesProvider, fieldName: String): Boolean {
        val delegateField = ClasspathClassBytesProvider::class.java.getDeclaredField(fieldName)
        delegateField.isAccessible = true
        @Suppress("UNCHECKED_CAST")
        val lazy = delegateField.get(provider) as Lazy<*>
        return lazy.isInitialized()
    }

    private fun taggedClassBytes(tag: Char): ByteArray =
        MINIMAL_CLASS_BYTES + tag.code.toByte()

    private companion object {
        val MINIMAL_CLASS_BYTES = byteArrayOf(
            0xCA.toByte(),
            0xFE.toByte(),
            0xBA.toByte(),
            0xBE.toByte(),
            0x00,
            0x00,
            0x00,
            0x00,
        )
    }
}
