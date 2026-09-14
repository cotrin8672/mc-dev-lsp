package io.github.mcdev.jdtls.project

import io.github.mcdev.core.bytecode.ConstantValue
import io.github.mcdev.core.bytecode.OccurrenceResultClassification
import io.github.mcdev.core.mixin.AtTargetKind
import io.github.mcdev.core.mixin.AtTargetOperationKind
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FieldIndexEntry
import io.github.mcdev.core.mixin.MethodIndexEntry
import java.util.concurrent.Callable
import java.util.concurrent.CountDownLatch
import java.util.concurrent.CyclicBarrier
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import java.util.jar.JarEntry
import java.util.jar.JarOutputStream
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertSame
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Files
import java.nio.file.Path
import javax.tools.ToolProvider

class BytecodeIndexAdapterTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun getClassBytesReturnsExactProviderBytesForExistingClass() {
        val owner = "test/ClassBytesSamples"
        val classBytes = compileClassBytesSamplesClass()
        writeClass(owner, classBytes)

        val provider = ClasspathClassBytesProvider(listOf(tempDir))
        val adapter = BytecodeIndexAdapter(provider, emptyClassIndex())

        val providerBytes = provider.getClassBytes(owner)
        val adapterBytes = adapter.getClassBytes(owner)

        assertSame(providerBytes, adapterBytes)
        assertEquals(classBytes.toList(), adapterBytes?.toList())
    }

    @Test
    fun getClassBytesReturnsNullForMissingClass() {
        val adapter = BytecodeIndexAdapter(
            ClasspathClassBytesProvider(listOf(tempDir)),
            emptyClassIndex(),
        )

        assertNull(adapter.getClassBytes("missing/Class"))
    }

    @Test
    fun getClassBytesDoesNotPopulateAdapterCachesOrEnumerateClasspath() {
        val jarPath = tempDir.resolve("lib.jar")
        writeJar(
            jarPath,
            linkedMapOf(
                "com/example/Alpha.class" to taggedClassBytes('A'),
                "com/example/Beta.class" to taggedClassBytes('B'),
            ),
        )

        val provider = ClasspathClassBytesProvider(listOf(jarPath))
        assertFalse(catalogDelegateInitialized(provider))
        assertEquals(0, provider.byteReadCount())

        val adapter = BytecodeIndexAdapter(provider, emptyClassIndex())
        assertEquals(0, candidateCacheSize(adapter))
        assertEquals(0, returnCountCacheSize(adapter))

        adapter.getClassBytes("com/example/Alpha")

        assertEquals(0, candidateCacheSize(adapter))
        assertEquals(0, returnCountCacheSize(adapter))
        assertFalse(catalogDelegateInitialized(provider))
        assertEquals(1, provider.byteReadCount())
    }

    @Test
    fun resolveCommonSuperClassResolvesSiblingSuperclassAndArrayCases() {
        val jarPath = tempDir.resolve("hierarchy.jar")
        writeJar(jarPath, compileHierarchyFixture())

        val adapter = BytecodeIndexAdapter(
            ClasspathClassBytesProvider(listOf(jarPath)),
            emptyClassIndex(),
        )

        assertEquals(
            "Lhierarchy/Base;",
            adapter.resolveCommonSuperClass("Lhierarchy/ChildOne;", "Lhierarchy/ChildTwo;"),
        )
        assertEquals(
            "Lhierarchy/Base;",
            adapter.resolveCommonSuperClass("Lhierarchy/ChildOne;", "Lhierarchy/Base;"),
        )
        assertEquals(
            "[Lhierarchy/Base;",
            adapter.resolveCommonSuperClass("[Lhierarchy/ChildOne;", "[Lhierarchy/Base;"),
        )
    }

    @Test
    fun resolveCommonSuperClassReverseOrderReusesByteReads() {
        val jarPath = tempDir.resolve("hierarchy-reverse.jar")
        writeJar(jarPath, compileHierarchyFixture())

        val provider = ClasspathClassBytesProvider(listOf(jarPath))
        val adapter = BytecodeIndexAdapter(provider, emptyClassIndex())

        val first = adapter.resolveCommonSuperClass("Lhierarchy/ChildOne;", "Lhierarchy/ChildTwo;")
        val readsAfterFirst = provider.byteReadCount()
        val second = adapter.resolveCommonSuperClass("Lhierarchy/ChildTwo;", "Lhierarchy/ChildOne;")

        assertEquals("Lhierarchy/Base;", first)
        assertEquals(first, second)
        assertEquals(readsAfterFirst, provider.byteReadCount())
    }

    @Test
    fun resolveCommonSuperClassMissingOrCorruptReturnsNull() {
        val jarPath = tempDir.resolve("hierarchy-corrupt.jar")
        writeJar(
            jarPath,
            linkedMapOf(
                "hierarchy/Corrupt.class" to CORRUPT_CLASS_BYTES,
                "hierarchy/Peer.class" to compileHierarchyPeerClass(),
            ),
        )

        val missingAdapter = BytecodeIndexAdapter(
            ClasspathClassBytesProvider(emptyList()),
            emptyClassIndex(),
        )
        assertNull(missingAdapter.resolveCommonSuperClass("Lcom/missing/Alpha;", "Lcom/missing/Beta;"))

        val corruptAdapter = BytecodeIndexAdapter(
            ClasspathClassBytesProvider(listOf(jarPath)),
            emptyClassIndex(),
        )
        assertNull(corruptAdapter.resolveCommonSuperClass("Lhierarchy/Corrupt;", "Lhierarchy/Peer;"))
    }

    @Test
    fun resolveCommonSuperClassDoesNotPopulateAdapterCachesOrEnumerateClasspath() {
        val jarPath = tempDir.resolve("hierarchy-cache.jar")
        writeJar(jarPath, compileHierarchyFixture())

        val provider = ClasspathClassBytesProvider(listOf(jarPath))
        assertFalse(catalogDelegateInitialized(provider))
        assertEquals(0, provider.byteReadCount())

        val adapter = BytecodeIndexAdapter(provider, emptyClassIndex())
        assertEquals(0, candidateCacheSize(adapter))
        assertEquals(0, returnCountCacheSize(adapter))

        adapter.resolveCommonSuperClass("Lhierarchy/ChildOne;", "Lhierarchy/ChildTwo;")

        assertEquals(0, candidateCacheSize(adapter))
        assertEquals(0, returnCountCacheSize(adapter))
        assertFalse(catalogDelegateInitialized(provider))
        assertTrue(provider.byteReadCount() > 0)
    }

    @Test
    fun overloadedMethodWithoutDescriptorUnionsDistinctInvokeCandidates() {
        val owner = "test/OverloadInvokeSamples"
        writeClass(owner, compileOverloadInvokeClass())

        val adapter = BytecodeIndexAdapter(
            ClasspathClassBytesProvider(listOf(tempDir)),
            overloadInvokeClassIndex(owner),
        )

        val allCandidates = adapter.getAtTargetCandidates(owner, "run", null, "INVOKE")
        assertEquals(2, allCandidates.size)
        assertTrue(allCandidates.any { it.name == "println" })
        assertTrue(allCandidates.any { it.name == "abs" })
        assertTrue(allCandidates.all { it.kind == AtTargetKind.INVOKE })

        val voidOnly = adapter.getAtTargetCandidates(owner, "run", "()V", "INVOKE")
        assertEquals(1, voidOnly.size)
        assertEquals("println", voidOnly.single().name)

        val intOnly = adapter.getAtTargetCandidates(owner, "run", "(I)V", "INVOKE")
        assertEquals(1, intOnly.size)
        assertEquals("abs", intOnly.single().name)
    }

    @Test
    fun returnCountCacheIsBoundedAcrossDistinctKeys() {
        val adapter = BytecodeIndexAdapter(
            ClasspathClassBytesProvider(emptyList()),
            emptyClassIndex(),
        )

        repeat(600) { index ->
            adapter.getReturnOrdinalCount("example/Target", "method$index", "()V")
        }

        val actualSize = returnCountCacheSize(adapter)
        assertTrue(
            actualSize <= 256,
            "returnCountCache size $actualSize exceeds bound of 256",
        )
    }

    @Test
    fun atTargetCandidateCacheIsBoundedAcrossDistinctKeys() {
        val adapter = BytecodeIndexAdapter(
            ClasspathClassBytesProvider(emptyList()),
            emptyClassIndex(),
        )

        repeat(600) { index ->
            adapter.getAtTargetCandidates("example/Target", "method$index", "()V", "INVOKE")
        }

        val actualSize = candidateCacheSize(adapter)
        assertTrue(
            actualSize <= 256,
            "candidateCache size $actualSize exceeds bound of 256",
        )
    }

    @Test
    fun distinctCandidateCacheKeysComputeConcurrentlyOutsideGlobalCacheLock() {
        val owner = "test/ConcurrentSamples"
        writeClass(owner, compileConcurrentSamplesClass())

        val findClassBarrier = CyclicBarrier(2)
        val activeFindClassCalls = AtomicInteger(0)
        val maxConcurrentFindClassCalls = AtomicInteger(0)
        val classIndex = concurrentSamplesClassIndex(
            owner = owner,
            onFindClass = {
                val active = activeFindClassCalls.incrementAndGet()
                maxConcurrentFindClassCalls.updateAndGet { current -> maxOf(current, active) }
                try {
                    findClassBarrier.await(10, TimeUnit.SECONDS)
                } finally {
                    activeFindClassCalls.decrementAndGet()
                }
            },
        )
        val adapter = BytecodeIndexAdapter(
            ClasspathClassBytesProvider(listOf(tempDir)),
            classIndex,
        )

        val startGate = CountDownLatch(1)
        val executor = Executors.newFixedThreadPool(2)
        try {
            val futures = listOf(
                executor.submit(
                    Callable {
                        startGate.await(10, TimeUnit.SECONDS)
                        adapter.getAtTargetCandidates(owner, "alpha", "()V", "INVOKE")
                    },
                ),
                executor.submit(
                    Callable {
                        startGate.await(10, TimeUnit.SECONDS)
                        adapter.getAtTargetCandidates(owner, "beta", "()V", "INVOKE")
                    },
                ),
            )
            startGate.countDown()
            futures.forEach { it.get(30, TimeUnit.SECONDS) }
            assertEquals(
                2,
                maxConcurrentFindClassCalls.get(),
                "distinct candidate cache keys must run convertCandidate/findClass concurrently",
            )
        } finally {
            executor.shutdownNow()
            executor.awaitTermination(5, TimeUnit.SECONDS)
        }
    }

    @Test
    fun fieldGetAndPutAtSameOwnerNameDescriptorRemainDistinct() {
        val owner = "test/FieldAccessSamples"
        writeClass(owner, compileFieldAccessClass())

        val adapter = BytecodeIndexAdapter(
            ClasspathClassBytesProvider(listOf(tempDir)),
            fieldAccessClassIndex(owner),
        )

        val candidates = adapter.getAtTargetCandidates(owner, "accessFields", "()V", "FIELD")
        assertEquals(4, candidates.size)

        val staticField = candidates.filter { it.name == "STATIC_FIELD" && it.descriptor == "I" }
        assertEquals(2, staticField.size)
        assertEquals(
            setOf(AtTargetOperationKind.FIELD_GET_STATIC, AtTargetOperationKind.FIELD_PUT_STATIC),
            staticField.mapNotNull { it.operationKind }.toSet(),
        )

        val instanceField = candidates.filter { it.name == "instanceField" && it.descriptor == "I" }
        assertEquals(2, instanceField.size)
        assertEquals(
            setOf(AtTargetOperationKind.FIELD_GET_INSTANCE, AtTargetOperationKind.FIELD_PUT_INSTANCE),
            instanceField.mapNotNull { it.operationKind }.toSet(),
        )
    }

    @Test
    fun operationKindMapsBytecodeKindsCorrectly() {
        val owner = "test/OverloadInvokeSamples"
        writeClass(owner, compileOverloadInvokeClass())

        val adapter = BytecodeIndexAdapter(
            ClasspathClassBytesProvider(listOf(tempDir)),
            overloadInvokeClassIndex(owner),
        )

        val invokeCandidates = adapter.getAtTargetCandidates(owner, "run", null, "INVOKE")
        val printlnCandidate = invokeCandidates.single { it.name == "println" }
        val absCandidate = invokeCandidates.single { it.name == "abs" }
        assertEquals(AtTargetOperationKind.INVOKE_VIRTUAL, printlnCandidate.operationKind)
        assertEquals(AtTargetOperationKind.INVOKE_STATIC, absCandidate.operationKind)

        val fieldOwner = "test/FieldAccessSamples"
        writeClass(fieldOwner, compileFieldAccessClass())
        val fieldAdapter = BytecodeIndexAdapter(
            ClasspathClassBytesProvider(listOf(tempDir)),
            fieldAccessClassIndex(fieldOwner),
        )
        val fieldCandidates = fieldAdapter.getAtTargetCandidates(fieldOwner, "accessFields", "()V", "FIELD")
        assertEquals(
            setOf(
                AtTargetOperationKind.FIELD_GET_STATIC,
                AtTargetOperationKind.FIELD_PUT_STATIC,
                AtTargetOperationKind.FIELD_GET_INSTANCE,
                AtTargetOperationKind.FIELD_PUT_INSTANCE,
            ),
            fieldCandidates.mapNotNull { it.operationKind }.toSet(),
        )
    }

    @Test
    fun distinctConstantValuesAtSameOrdinalAreBothRetained() {
        val owner = "test/ConstantSamples"
        writeClass(owner, compileConstantSamplesClass())

        val adapter = BytecodeIndexAdapter(
            ClasspathClassBytesProvider(listOf(tempDir)),
            constantSamplesClassIndex(owner),
        )

        val candidates = adapter.getAtTargetCandidates(owner, "smallInts", "()V", "CONSTANT")
        assertEquals(2, candidates.size)
        assertTrue(candidates.all { it.ordinal == 0 })
        assertTrue(candidates.all { it.kind == AtTargetKind.CONSTANT })

        val intValues = candidates
            .mapNotNull { it.constantValue as? ConstantValue.IntValue }
            .map { it.value }
            .toSet()
        assertEquals(setOf(5, -1), intValues)
    }

    @Test
    fun invokeOccurrenceClassificationAndIndexPropagateWithoutCollapsing() {
        val owner = "test/InvokeClassificationSamples"
        writeClass(owner, compileInvokeClassificationSamplesClass())

        val adapter = BytecodeIndexAdapter(
            ClasspathClassBytesProvider(listOf(tempDir)),
            invokeClassificationSamplesClassIndex(owner),
        )

        val candidates = adapter.getAtTargetCandidates(owner, "mixedAbsCalls", "()V", "INVOKE")
        val absCandidates = candidates
            .filter { it.owner == "java/lang/Math" && it.name == "abs" && it.descriptor == "(I)I" }
            .sortedBy { it.instructionOccurrenceIndex }

        assertEquals(2, absCandidates.size)
        assertEquals(
            OccurrenceResultClassification.IMMEDIATELY_POPPED,
            absCandidates[0].occurrenceResultClassification,
        )
        assertEquals(
            OccurrenceResultClassification.RETAINED,
            absCandidates[1].occurrenceResultClassification,
        )
        assertTrue(absCandidates[0].instructionOccurrenceIndex >= 0)
        assertTrue(absCandidates[1].instructionOccurrenceIndex > absCandidates[0].instructionOccurrenceIndex)
    }

    @Test
    fun unknownMethodWithoutDescriptorReturnsNoCandidates() {
        val owner = "test/OverloadInvokeSamples"
        writeClass(owner, compileOverloadInvokeClass())

        val adapter = BytecodeIndexAdapter(
            ClasspathClassBytesProvider(listOf(tempDir)),
            overloadInvokeClassIndex(owner),
        )

        assertEquals(
            emptyList(),
            adapter.getAtTargetCandidates(owner, "missing", null, "INVOKE"),
        )
    }

    private fun candidateCacheSize(adapter: BytecodeIndexAdapter): Int {
        val field = BytecodeIndexAdapter::class.java.getDeclaredField("candidateCache")
        field.isAccessible = true
        @Suppress("UNCHECKED_CAST")
        val cache = field.get(adapter) as Map<*, *>
        return cache.size
    }

    private fun returnCountCacheSize(adapter: BytecodeIndexAdapter): Int {
        val field = BytecodeIndexAdapter::class.java.getDeclaredField("returnCountCache")
        field.isAccessible = true
        @Suppress("UNCHECKED_CAST")
        val cache = field.get(adapter) as Map<*, *>
        return cache.size
    }

    private fun emptyClassIndex(): ClassIndex =
        object : ClassIndex {
            override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> = emptyList()

            override fun findClass(internalName: String): ClassIndexEntry? = null

            override fun findClassByFqn(fqn: String): ClassIndexEntry? = null

            override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> = emptyList()

            override fun getFields(ownerInternalName: String): List<FieldIndexEntry> = emptyList()
        }

    private fun writeClass(internalName: String, classBytes: ByteArray) {
        val classFile = tempDir.resolve("$internalName.class")
        Files.createDirectories(classFile.parent)
        Files.write(classFile, classBytes)
    }

    private fun concurrentSamplesClassIndex(
        owner: String,
        onFindClass: () -> Unit,
    ): ClassIndex =
        object : ClassIndex {
            override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> = emptyList()

            override fun findClass(internalName: String): ClassIndexEntry? {
                onFindClass()
                return if (internalName == owner) {
                    ClassIndexEntry("ConcurrentSamples", "test", owner)
                } else {
                    ClassIndexEntry(internalName.substringAfterLast('/'), internalName.substringBeforeLast('/'), internalName)
                }
            }

            override fun findClassByFqn(fqn: String): ClassIndexEntry? = null

            override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> =
                if (ownerInternalName == owner) {
                    listOf(
                        MethodIndexEntry("alpha", "()V", false, "alpha(): void"),
                        MethodIndexEntry("beta", "()V", false, "beta(): void"),
                    )
                } else {
                    emptyList()
                }

            override fun getFields(ownerInternalName: String): List<FieldIndexEntry> = emptyList()
        }

    private fun compileConcurrentSamplesClass(): ByteArray {
        val compiler = ToolProvider.getSystemJavaCompiler()
            ?: error("Java compiler not available; cannot compile concurrent samples fixture")
        val sourceDir = Files.createTempDirectory("mcdev-concurrent-samples-src")
        val outputDir = Files.createTempDirectory("mcdev-concurrent-samples-out")
        val sourceFile = sourceDir.resolve("test").resolve("ConcurrentSamples.java")
        Files.createDirectories(sourceFile.parent)
        Files.writeString(
            sourceFile,
            """
            package test;

            public class ConcurrentSamples {
                public void alpha() {
                    System.out.println("alpha");
                }

                public void beta() {
                    System.out.println("beta");
                }
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
            "failed to compile concurrent samples fixture:${diagnostics.joinToString(prefix = System.lineSeparator())}"
        }
        fileManager.close()

        return Files.readAllBytes(outputDir.resolve("test").resolve("ConcurrentSamples.class"))
    }

    private fun fieldAccessClassIndex(owner: String): ClassIndex =
        object : ClassIndex {
            override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> = emptyList()

            override fun findClass(internalName: String): ClassIndexEntry? =
                if (internalName == owner) {
                    ClassIndexEntry("FieldAccessSamples", "test", owner)
                } else {
                    null
                }

            override fun findClassByFqn(fqn: String): ClassIndexEntry? = null

            override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> =
                if (ownerInternalName == owner) {
                    listOf(MethodIndexEntry("accessFields", "()V", false, "accessFields(): void"))
                } else {
                    emptyList()
                }

            override fun getFields(ownerInternalName: String): List<FieldIndexEntry> = emptyList()
        }

    private fun compileFieldAccessClass(): ByteArray {
        val compiler = ToolProvider.getSystemJavaCompiler()
            ?: error("Java compiler not available; cannot compile field access fixture")
        val sourceDir = Files.createTempDirectory("mcdev-field-access-src")
        val outputDir = Files.createTempDirectory("mcdev-field-access-out")
        val sourceFile = sourceDir.resolve("test").resolve("FieldAccessSamples.java")
        Files.createDirectories(sourceFile.parent)
        Files.writeString(
            sourceFile,
            """
            package test;

            public class FieldAccessSamples {
                static int STATIC_FIELD = 1;
                int instanceField = 2;

                void accessFields() {
                    int a = STATIC_FIELD;
                    STATIC_FIELD = 3;
                    int b = instanceField;
                    instanceField = 4;
                }
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
            "failed to compile field access fixture:${diagnostics.joinToString(prefix = System.lineSeparator())}"
        }
        fileManager.close()

        return Files.readAllBytes(outputDir.resolve("test").resolve("FieldAccessSamples.class"))
    }

    private fun overloadInvokeClassIndex(owner: String): ClassIndex =
        object : ClassIndex {
            override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> = emptyList()

            override fun findClass(internalName: String): ClassIndexEntry? =
                if (internalName == owner) {
                    ClassIndexEntry("OverloadInvokeSamples", "test", owner)
                } else {
                    null
                }

            override fun findClassByFqn(fqn: String): ClassIndexEntry? = null

            override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> =
                if (ownerInternalName == owner) {
                    listOf(
                        MethodIndexEntry("run", "()V", false, "run(): void"),
                        MethodIndexEntry("run", "(I)V", false, "run(int): void"),
                    )
                } else {
                    emptyList()
                }

            override fun getFields(ownerInternalName: String): List<FieldIndexEntry> = emptyList()
        }

    private fun constantSamplesClassIndex(owner: String): ClassIndex =
        object : ClassIndex {
            override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> = emptyList()

            override fun findClass(internalName: String): ClassIndexEntry? =
                if (internalName == owner) {
                    ClassIndexEntry("ConstantSamples", "test", owner)
                } else {
                    null
                }

            override fun findClassByFqn(fqn: String): ClassIndexEntry? = null

            override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> =
                if (ownerInternalName == owner) {
                    listOf(MethodIndexEntry("smallInts", "()V", false, "smallInts(): void"))
                } else {
                    emptyList()
                }

            override fun getFields(ownerInternalName: String): List<FieldIndexEntry> = emptyList()
        }

    private fun compileConstantSamplesClass(): ByteArray {
        val compiler = ToolProvider.getSystemJavaCompiler()
            ?: error("Java compiler not available; cannot compile constant samples fixture")
        val sourceDir = Files.createTempDirectory("mcdev-constant-samples-src")
        val outputDir = Files.createTempDirectory("mcdev-constant-samples-out")
        val sourceFile = sourceDir.resolve("test").resolve("ConstantSamples.java")
        Files.createDirectories(sourceFile.parent)
        Files.writeString(
            sourceFile,
            """
            package test;

            public class ConstantSamples {
                void smallInts() {
                    int a = 5;
                    int b = -1;
                }
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
            "failed to compile constant samples fixture:${diagnostics.joinToString(prefix = System.lineSeparator())}"
        }
        fileManager.close()

        return Files.readAllBytes(outputDir.resolve("test").resolve("ConstantSamples.class"))
    }

    private fun invokeClassificationSamplesClassIndex(owner: String): ClassIndex =
        object : ClassIndex {
            override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> = emptyList()

            override fun findClass(internalName: String): ClassIndexEntry? =
                if (internalName == owner) {
                    ClassIndexEntry("InvokeClassificationSamples", "test", owner)
                } else {
                    null
                }

            override fun findClassByFqn(fqn: String): ClassIndexEntry? = null

            override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> =
                if (ownerInternalName == owner) {
                    listOf(MethodIndexEntry("mixedAbsCalls", "()V", false, "mixedAbsCalls(): void"))
                } else {
                    emptyList()
                }

            override fun getFields(ownerInternalName: String): List<FieldIndexEntry> = emptyList()
        }

    private fun compileInvokeClassificationSamplesClass(): ByteArray {
        val compiler = ToolProvider.getSystemJavaCompiler()
            ?: error("Java compiler not available; cannot compile invoke classification fixture")
        val sourceDir = Files.createTempDirectory("mcdev-invoke-classification-src")
        val outputDir = Files.createTempDirectory("mcdev-invoke-classification-out")
        val sourceFile = sourceDir.resolve("test").resolve("InvokeClassificationSamples.java")
        Files.createDirectories(sourceFile.parent)
        Files.writeString(
            sourceFile,
            """
            package test;

            public class InvokeClassificationSamples {
                void mixedAbsCalls() {
                    Math.abs(1);
                    int retained = Math.abs(2);
                }
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
            "failed to compile invoke classification fixture:${diagnostics.joinToString(prefix = System.lineSeparator())}"
        }
        fileManager.close()

        return Files.readAllBytes(outputDir.resolve("test").resolve("InvokeClassificationSamples.class"))
    }

    private fun compileOverloadInvokeClass(): ByteArray {
        val compiler = ToolProvider.getSystemJavaCompiler()
            ?: error("Java compiler not available; cannot compile overload invoke fixture")
        val sourceDir = Files.createTempDirectory("mcdev-overload-invoke-src")
        val outputDir = Files.createTempDirectory("mcdev-overload-invoke-out")
        val sourceFile = sourceDir.resolve("test").resolve("OverloadInvokeSamples.java")
        Files.createDirectories(sourceFile.parent)
        Files.writeString(
            sourceFile,
            """
            package test;

            public class OverloadInvokeSamples {
                public void run() {
                    System.out.println("void-overload");
                }

                public void run(int value) {
                    Math.abs(value);
                }
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
            "failed to compile overload invoke fixture:${diagnostics.joinToString(prefix = System.lineSeparator())}"
        }
        fileManager.close()

        return Files.readAllBytes(outputDir.resolve("test").resolve("OverloadInvokeSamples.class"))
    }

    private fun compileClassBytesSamplesClass(): ByteArray {
        val compiler = ToolProvider.getSystemJavaCompiler()
            ?: error("Java compiler not available; cannot compile class bytes samples fixture")
        val sourceDir = Files.createTempDirectory("mcdev-class-bytes-samples-src")
        val outputDir = Files.createTempDirectory("mcdev-class-bytes-samples-out")
        val sourceFile = sourceDir.resolve("test").resolve("ClassBytesSamples.java")
        Files.createDirectories(sourceFile.parent)
        Files.writeString(
            sourceFile,
            """
            package test;

            public class ClassBytesSamples {
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
            "failed to compile class bytes samples fixture:${diagnostics.joinToString(prefix = System.lineSeparator())}"
        }
        fileManager.close()

        return Files.readAllBytes(outputDir.resolve("test").resolve("ClassBytesSamples.class"))
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

    private fun catalogDelegateInitialized(provider: ClasspathClassBytesProvider): Boolean {
        val delegateField = ClasspathClassBytesProvider::class.java.getDeclaredField("catalog\$delegate")
        delegateField.isAccessible = true
        @Suppress("UNCHECKED_CAST")
        val lazy = delegateField.get(provider) as Lazy<*>
        return lazy.isInitialized()
    }

    private fun taggedClassBytes(tag: Char): ByteArray =
        MINIMAL_CLASS_BYTES + tag.code.toByte()

    private fun compileHierarchyFixture(): Map<String, ByteArray> {
        val compiler = ToolProvider.getSystemJavaCompiler()
            ?: error("Java compiler not available; cannot compile hierarchy fixture")
        val sourceDir = Files.createTempDirectory("mcdev-hierarchy-src")
        val outputDir = Files.createTempDirectory("mcdev-hierarchy-out")
        val hierarchyDir = sourceDir.resolve("hierarchy")
        Files.createDirectories(hierarchyDir)
        Files.writeString(
            hierarchyDir.resolve("Base.java"),
            """
            package hierarchy;

            public class Base {
            }
            """.trimIndent() + System.lineSeparator(),
        )
        Files.writeString(
            hierarchyDir.resolve("ChildOne.java"),
            """
            package hierarchy;

            public class ChildOne extends Base {
            }
            """.trimIndent() + System.lineSeparator(),
        )
        Files.writeString(
            hierarchyDir.resolve("ChildTwo.java"),
            """
            package hierarchy;

            public class ChildTwo extends Base {
            }
            """.trimIndent() + System.lineSeparator(),
        )

        val diagnostics = mutableListOf<String>()
        val fileManager = compiler.getStandardFileManager(null, null, null)
        val sourceFiles = listOf("Base.java", "ChildOne.java", "ChildTwo.java")
            .map { hierarchyDir.resolve(it).toFile() }
        val compilationUnits = fileManager.getJavaFileObjectsFromFiles(sourceFiles)
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
            "failed to compile hierarchy fixture:${diagnostics.joinToString(prefix = System.lineSeparator())}"
        }
        fileManager.close()

        return linkedMapOf(
            "hierarchy/Base.class" to Files.readAllBytes(outputDir.resolve("hierarchy").resolve("Base.class")),
            "hierarchy/ChildOne.class" to Files.readAllBytes(outputDir.resolve("hierarchy").resolve("ChildOne.class")),
            "hierarchy/ChildTwo.class" to Files.readAllBytes(outputDir.resolve("hierarchy").resolve("ChildTwo.class")),
        )
    }

    private fun compileHierarchyPeerClass(): ByteArray {
        val compiler = ToolProvider.getSystemJavaCompiler()
            ?: error("Java compiler not available; cannot compile hierarchy peer fixture")
        val sourceDir = Files.createTempDirectory("mcdev-hierarchy-peer-src")
        val outputDir = Files.createTempDirectory("mcdev-hierarchy-peer-out")
        val sourceFile = sourceDir.resolve("hierarchy").resolve("Peer.java")
        Files.createDirectories(sourceFile.parent)
        Files.writeString(
            sourceFile,
            """
            package hierarchy;

            public class Peer {
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
            "failed to compile hierarchy peer fixture:${diagnostics.joinToString(prefix = System.lineSeparator())}"
        }
        fileManager.close()

        return Files.readAllBytes(outputDir.resolve("hierarchy").resolve("Peer.class"))
    }

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

        val CORRUPT_CLASS_BYTES = MINIMAL_CLASS_BYTES
    }
}
