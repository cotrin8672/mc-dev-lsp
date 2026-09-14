package io.github.mcdev.jdtls.mixin

import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FieldIndexEntry
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.project.ProjectContextBuilder
import io.github.mcdev.core.project.SourceSetContext
import io.github.mcdev.jdtls.project.McdevProjectSession
import io.github.mcdev.jdtls.project.UriPathSupport
import kotlin.test.BeforeTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Files
import java.nio.file.Path

class SourceBackedClassIndexTest {
    @TempDir
    lateinit var tempDir: Path

    private lateinit var sourceDir: Path

    @BeforeTest
    fun resetScannerState() {
        SourceClassScanner.resetMetrics()
        SourceClassScanner.clearDiskCache()
        sourceDir = tempDir.resolve("src/main/java")
        Files.createDirectories(sourceDir)
    }

    @Test
    fun projectSourceQueryDefaultsCoversProjectDependenciesToFalse() {
        assertTrue(!BufferProjectSourceQuery.fromBuffer("").coversProjectDependencies())
        assertTrue(!fixedSourceQuery().coversProjectDependencies())
    }

    @Test
    fun compositeProjectSourceQueryCoversProjectDependenciesWhenAnyDelegateDoes() {
        val coversDependencies = object : ProjectSourceQuery {
            override fun findClasses(prefix: String, limit: Int): Sequence<ClassIndexEntry> = emptySequence()

            override fun findClass(internalName: String): ClassIndexEntry? = null

            override fun findClassByFqn(fqn: String): ClassIndexEntry? = null

            override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> = emptyList()

            override fun coversProjectDependencies(): Boolean = true
        }
        val composite = CompositeProjectSourceQuery(
            listOf(
                fixedSourceQuery(),
                coversDependencies,
            ),
        )

        assertTrue(composite.coversProjectDependencies())
    }

    @Test
    fun compositeProjectSourceQueryDoesNotCoverProjectDependenciesWhenNoDelegateDoes() {
        val composite = CompositeProjectSourceQuery(
            listOf(
                fixedSourceQuery(),
                BufferProjectSourceQuery.fromBuffer(""),
            ),
        )

        assertTrue(!composite.coversProjectDependencies())
    }

    @Test
    fun compositeProjectSourceQueryPrefersBufferOverJdtStyleDelegate() {
        val jdtEntry = entry("JdtCurrent", "com.example", "com/example/Current")
        val composite = CompositeProjectSourceQuery(
            listOf(
                BufferProjectSourceQuery.fromBuffer(
                    javaSource("Current", "public void bufferMethod() {}"),
                ),
                fixedSourceQuery(
                    classesByInternal = mapOf("com/example/Current" to jdtEntry),
                    classesByFqn = mapOf("com.example.Current" to jdtEntry),
                    methodsByOwner = mapOf(
                        "com/example/Current" to listOf(method("jdtMethod", "()V")),
                    ),
                ),
            ),
        )

        assertEquals("Current", composite.findClass("com/example/Current")?.simpleName)
        assertEquals("Current", composite.findClassByFqn("com.example.Current")?.simpleName)
        assertTrue(composite.getMethods("com/example/Current").any { it.name == "bufferMethod" })
        assertTrue(composite.getMethods("com/example/Current").any { it.name == "jdtMethod" })
    }

    @Test
    fun compositeProjectSourceQueryMergesClassesInDelegateOrderWithLimit() {
        val shared = entry("Shared", "com.example", "com/example/Shared")
        val firstOnly = entry("FirstOnly", "com.example", "com/example/FirstOnly")
        val secondOnly = entry("SecondOnly", "com.example", "com/example/SecondOnly")
        val composite = CompositeProjectSourceQuery(
            listOf(
                fixedSourceQuery(classesByPrefix = mapOf("com" to listOf(shared, firstOnly))),
                fixedSourceQuery(classesByPrefix = mapOf("com" to listOf(shared, secondOnly))),
            ),
        )

        assertEquals(
            listOf("com/example/Shared", "com/example/FirstOnly", "com/example/SecondOnly"),
            composite.findClasses("com", limit = 10).toList().map { it.internalName },
        )
        assertEquals(
            listOf("com/example/Shared", "com/example/FirstOnly"),
            composite.findClasses("com", limit = 2).toList().map { it.internalName },
        )
        assertTrue(composite.findClasses("com", limit = 0).toList().isEmpty())
        assertTrue(composite.findClasses("com", limit = -1).toList().isEmpty())
    }

    @Test
    fun compositeProjectSourceQueryMergesMethodsInDelegateOrderAndDedupsByNameAndDescriptor() {
        val bufferMethod = method("run", "()V", readableSignature = "run(): void [buffer]")
        val jdtDuplicate = method("run", "()V", readableSignature = "run(): void [jdt]")
        val jdtOnly = method("other", "()V")
        val composite = CompositeProjectSourceQuery(
            listOf(
                fixedSourceQuery(
                    methodsByOwner = mapOf("com/example/Target" to listOf(bufferMethod)),
                ),
                fixedSourceQuery(
                    methodsByOwner = mapOf("com/example/Target" to listOf(jdtDuplicate, jdtOnly)),
                ),
            ),
        )

        assertEquals(
            listOf("run()V", "other()V"),
            composite.getMethods("com/example/Target").map { it.name + it.descriptor },
        )
        assertEquals("run(): void [buffer]", composite.getMethods("com/example/Target").first().readableSignature)
    }

    @Test
    fun authoritativeSourceQuerySkipsDelegateForAllOperations() {
        val sourceEntry = entry("SourceOnly", "com.example", "com/example/SourceOnly")
        val delegateEntry = entry("DelegateOnly", "com.example", "com/example/DelegateOnly")
        val sourceMethod = method("sourceRun", "()V")
        val delegateMethod = method("delegateRun", "()V")
        val sourceField = field("sourceValue", "I")
        val delegateField = field("delegateValue", "Z", readableType = "boolean")
        val recordingDelegate = recordingClassIndex()
        val index = SourceBackedClassIndex(
            delegate = recordingDelegate,
            sourceQuery = fixedSourceQuery(
                classesByInternal = mapOf(
                    "com/example/SourceOnly" to sourceEntry,
                    "com/example/Shared" to entry("Shared", "com.example", "com/example/Shared"),
                ),
                classesByFqn = mapOf("com.example.SourceOnly" to sourceEntry),
                classesByPrefix = mapOf("com" to listOf(sourceEntry)),
                methodsByOwner = mapOf("com/example/Target" to listOf(sourceMethod)),
                fieldsByOwner = mapOf("com/example/Target" to listOf(sourceField)),
                coversProjectDependencies = true,
            ),
        )
        recordingDelegate.delegateData = fixedClassIndex(
            classesByInternal = mapOf("com/example/DelegateOnly" to delegateEntry),
            classesByFqn = mapOf("com.example.DelegateOnly" to delegateEntry),
            classesByPrefix = mapOf("com" to listOf(delegateEntry)),
            methodsByOwner = mapOf("com/example/Target" to listOf(delegateMethod)),
            fieldsByOwner = mapOf("com/example/Target" to listOf(delegateField)),
        )

        assertEquals(listOf(sourceEntry), index.findClasses("com", limit = 10))
        assertEquals(sourceEntry, index.findClass("com/example/SourceOnly"))
        assertEquals(sourceEntry, index.findClassByFqn("com.example.SourceOnly"))
        assertEquals(listOf(sourceMethod), index.getMethods("com/example/Target"))
        assertEquals(listOf(sourceField), index.getFields("com/example/Target"))
        assertEquals(emptyList<String>(), recordingDelegate.invocations)
    }

    @Test
    fun jdtFindTypeFailureFallsBackForExactLookupsButAuthoritativeEmptyStaysEmpty() {
        val sourceEntry = entry("Target", "com.example", "com/example/Target")
        val sourceMethod = method("run", "()V")
        val sourceField = field("value", "I")
        val index = SourceBackedClassIndex(
            delegate = fixedClassIndex(
                classesByInternal = mapOf(sourceEntry.internalName to sourceEntry),
                classesByFqn = mapOf(sourceEntry.fqn to sourceEntry),
                methodsByOwner = mapOf(sourceEntry.internalName to listOf(sourceMethod)),
                fieldsByOwner = mapOf(sourceEntry.internalName to listOf(sourceField)),
            ),
            sourceQuery = JdtProjectSourceQuery(
                ThrowingJavaProject(),
                isJdtSearchEngineAvailable = { true },
            ),
        )

        assertEquals(sourceEntry, index.findClass(sourceEntry.internalName))
        assertEquals(sourceEntry, index.findClassByFqn(sourceEntry.fqn))
        assertEquals(listOf(sourceMethod), index.getMethods(sourceEntry.internalName))
        assertEquals(listOf(sourceField), index.getFields(sourceEntry.internalName))

        val recordingDelegate = recordingClassIndex()
        recordingDelegate.delegateData = fixedClassIndex(
            classesByInternal = mapOf(sourceEntry.internalName to sourceEntry),
            classesByFqn = mapOf(sourceEntry.fqn to sourceEntry),
            methodsByOwner = mapOf(sourceEntry.internalName to listOf(sourceMethod)),
            fieldsByOwner = mapOf(sourceEntry.internalName to listOf(sourceField)),
        )
        val authoritativeEmpty = SourceBackedClassIndex(
            delegate = recordingDelegate,
            sourceQuery = fixedSourceQuery(coversProjectDependencies = true),
        )

        assertNull(authoritativeEmpty.findClass(sourceEntry.internalName))
        assertNull(authoritativeEmpty.findClassByFqn(sourceEntry.fqn))
        assertEquals(emptyList<MethodIndexEntry>(), authoritativeEmpty.getMethods(sourceEntry.internalName))
        assertEquals(emptyList<FieldIndexEntry>(), authoritativeEmpty.getFields(sourceEntry.internalName))
        assertEquals(emptyList<String>(), recordingDelegate.invocations)
    }

    @Test
    fun compositeKeepsBufferedMembersWhenJdtFindTypeFails() {
        val ownerInternalName = "com/example/Target"
        val bufferedMethod = method("run", "()V", readableSignature = "run(): void [buffer]")
        val bufferedField = field("value", "I", readableType = "int [buffer]")
        val targetOwner = ownerInternalName
        val bufferQuery = object : ProjectSourceQuery {
            override fun findClasses(prefix: String, limit: Int): Sequence<ClassIndexEntry> = emptySequence()

            override fun findClass(internalName: String): ClassIndexEntry? = null

            override fun findClassByFqn(fqn: String): ClassIndexEntry? = null

            override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> =
                if (ownerInternalName == targetOwner) listOf(bufferedMethod) else emptyList()

            override fun getFields(ownerInternalName: String): List<FieldIndexEntry> =
                if (ownerInternalName == targetOwner) listOf(bufferedField) else emptyList()
        }
        val delegateMethod = method("stale", "()V")
        val delegateField = field("stale", "Z", readableType = "boolean")
        val index = SourceBackedClassIndex(
            delegate = fixedClassIndex(
                methodsByOwner = mapOf(ownerInternalName to listOf(bufferedMethod, delegateMethod)),
                fieldsByOwner = mapOf(ownerInternalName to listOf(bufferedField, delegateField)),
            ),
            sourceQuery = CompositeProjectSourceQuery(
                listOf(
                    bufferQuery,
                    JdtProjectSourceQuery(
                        ThrowingJavaProject(),
                        isJdtSearchEngineAvailable = { true },
                    ),
                ),
            ),
        )

        assertEquals(
            listOf("run()V", "stale()V"),
            index.getMethods(ownerInternalName).map { it.name + it.descriptor },
        )
        assertEquals("run(): void [buffer]", index.getMethods(ownerInternalName).first().readableSignature)
        assertEquals(
            listOf("valueI", "staleZ"),
            index.getFields(ownerInternalName).map { it.name + it.descriptor },
        )
        assertEquals("int [buffer]", index.getFields(ownerInternalName).first().readableType)
    }

    @Test
    fun nonAuthoritativeSourceQueryFallsBackToDelegateForFindClass() {
        val delegateEntry = entry("DelegateOnly", "com.example", "com/example/DelegateOnly")
        val recordingDelegate = recordingClassIndex()
        val index = SourceBackedClassIndex(
            delegate = recordingDelegate,
            sourceQuery = fixedSourceQuery(),
        )
        recordingDelegate.delegateData = fixedClassIndex(
            classesByInternal = mapOf("com/example/DelegateOnly" to delegateEntry),
        )

        assertEquals(delegateEntry, index.findClass("com/example/DelegateOnly"))
        assertEquals(listOf("findClass:com/example/DelegateOnly"), recordingDelegate.invocations)
    }

    @Test
    fun sourceQueryTakesPrecedenceOverDelegateForClassLookup() {
        val sourceEntry = entry("SourceFoo", "com.example", "com/example/SourceFoo")
        val delegateEntry = entry("DelegateFoo", "com.other", "com/example/SourceFoo")
        val index = SourceBackedClassIndex(
            delegate = fixedClassIndex(
                classesByInternal = mapOf("com/example/SourceFoo" to delegateEntry),
                classesByFqn = mapOf("com.example.SourceFoo" to delegateEntry),
            ),
            sourceQuery = fixedSourceQuery(
                classesByInternal = mapOf("com/example/SourceFoo" to sourceEntry),
                classesByFqn = mapOf("com.example.SourceFoo" to sourceEntry),
            ),
        )

        assertEquals(sourceEntry, index.findClass("com/example/SourceFoo"))
        assertEquals(sourceEntry, index.findClassByFqn("com.example.SourceFoo"))
    }

    @Test
    fun findClassesPassesLimitThroughToSourceQuery() {
        val recordedLimits = mutableListOf<Int>()
        val index = SourceBackedClassIndex(
            delegate = fixedClassIndex(),
            sourceQuery = recordingSourceQuery(recordedLimits),
        )

        index.findClasses("com", limit = 7)

        assertEquals(listOf(7), recordedLimits)
    }

    @Test
    fun findClassesFallsBackWhenSourceSearchBecomesNonAuthoritative() {
        val sourceEntry = entry("SourceOnly", "com.example", "com/example/SourceOnly")
        val delegateEntry = entry("DelegateOnly", "com.example", "com/example/DelegateOnly")
        val recordingDelegate = recordingClassIndex()
        recordingDelegate.delegateData = fixedClassIndex(
            classesByPrefix = mapOf("com" to listOf(delegateEntry)),
        )
        var authoritative = true
        val sourceQuery = object : ProjectSourceQuery {
            override fun findClasses(prefix: String, limit: Int): Sequence<ClassIndexEntry> =
                sequence {
                    authoritative = false
                    yield(sourceEntry)
                }

            override fun findClass(internalName: String): ClassIndexEntry? = null

            override fun findClassByFqn(fqn: String): ClassIndexEntry? = null

            override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> = emptyList()

            override fun coversProjectDependencies(): Boolean = authoritative
        }
        val index = SourceBackedClassIndex(
            delegate = recordingDelegate,
            sourceQuery = sourceQuery,
        )

        assertEquals(
            listOf(sourceEntry, delegateEntry),
            index.findClasses("com", limit = 10),
        )
        assertEquals(listOf("findClasses:com:10"), recordingDelegate.invocations)
    }

    @Test
    fun authoritativeEmptyFindClassesSkipsDelegate() {
        val delegateEntry = entry("DelegateOnly", "com.example", "com/example/DelegateOnly")
        val recordingDelegate = recordingClassIndex()
        recordingDelegate.delegateData = fixedClassIndex(
            classesByPrefix = mapOf("com" to listOf(delegateEntry)),
        )
        val index = SourceBackedClassIndex(
            delegate = recordingDelegate,
            sourceQuery = fixedSourceQuery(coversProjectDependencies = true),
        )

        assertEquals(emptyList<ClassIndexEntry>(), index.findClasses("com", limit = 10))
        assertEquals(emptyList<String>(), recordingDelegate.invocations)
    }

    @Test
    fun findClassesMergesSourceBeforeDelegateAndDistinctByInternalName() {
        val shared = entry("Shared", "com.example", "com/example/Shared")
        val sourceOnly = entry("SourceOnly", "com.example", "com/example/SourceOnly")
        val delegateOnly = entry("DelegateOnly", "com.example", "com/example/DelegateOnly")
        val index = SourceBackedClassIndex(
            delegate = fixedClassIndex(
                classesByPrefix = mapOf("com" to listOf(shared, delegateOnly)),
            ),
            sourceQuery = fixedSourceQuery(
                classesByPrefix = mapOf("com" to listOf(shared, sourceOnly)),
            ),
        )

        assertEquals(
            listOf("com/example/Shared", "com/example/SourceOnly", "com/example/DelegateOnly"),
            index.findClasses("com", limit = 10).map { it.internalName },
        )
    }

    @Test
    fun getMethodsMergesSourceBeforeDelegateAndDistinctByNameAndDescriptor() {
        val sourceMethod = method("run", "()V")
        val duplicate = method("run", "()V", readableSignature = "run(): void [source]")
        val delegateOnly = method("other", "()V")
        val index = SourceBackedClassIndex(
            delegate = fixedClassIndex(
                methodsByOwner = mapOf(
                    "com/example/Target" to listOf(duplicate, delegateOnly),
                ),
            ),
            sourceQuery = fixedSourceQuery(
                methodsByOwner = mapOf(
                    "com/example/Target" to listOf(sourceMethod),
                ),
            ),
        )

        assertEquals(
            listOf("run()V", "other()V"),
            index.getMethods("com/example/Target").map { it.name + it.descriptor },
        )
        assertEquals("run(): void", index.getMethods("com/example/Target").first().readableSignature)
    }

    @Test
    fun getFieldsMergesSourceBeforeDelegateAndDistinctByNameAndDescriptor() {
        val sourceField = field("value", "I", readableType = "int")
        val duplicate = field("value", "I", readableType = "int [delegate]")
        val delegateOnly = field("other", "Z", readableType = "boolean")
        val index = SourceBackedClassIndex(
            delegate = fixedClassIndex(
                fieldsByOwner = mapOf(
                    "com/example/Target" to listOf(duplicate, delegateOnly),
                ),
            ),
            sourceQuery = fixedSourceQuery(
                fieldsByOwner = mapOf(
                    "com/example/Target" to listOf(sourceField),
                ),
            ),
        )

        assertEquals(
            listOf("valueI", "otherZ"),
            index.getFields("com/example/Target").map { it.name + it.descriptor },
        )
        assertEquals("int", index.getFields("com/example/Target").first().readableType)
    }

    @Test
    fun getFieldsFallsBackToDelegateWhenSourceQueryReturnsEmpty() {
        val field = FieldIndexEntry(name = "value", descriptor = "I", isStatic = false, readableType = "int")
        val index = SourceBackedClassIndex(
            delegate = fixedClassIndex(
                fieldsByOwner = mapOf("com/example/Target" to listOf(field)),
            ),
            sourceQuery = fixedSourceQuery(),
        )

        assertEquals(listOf(field), index.getFields("com/example/Target"))
    }

    @Test
    fun bufferSourceQueryExposesUnsavedClassAndLatestMethod() {
        val query = BufferProjectSourceQuery.fromBuffer(
            javaSource("Unsaved", "public void latestMethod() {}"),
        )

        assertEquals("com/example/Unsaved", query.findClassByFqn("com.example.Unsaved")?.internalName)
        assertEquals("com/example/Unsaved", query.findClass("com/example/Unsaved")?.internalName)
        assertTrue(query.getMethods("com/example/Unsaved").any { it.name == "latestMethod" })
    }

    @Test
    fun bufferSourceQueryReflectsEachVersionWithoutDiskAccess() {
        repeat(30) { version ->
            val index = SourceBackedClassIndex(
                delegate = EmptyClassIndex,
                sourceQuery = BufferProjectSourceQuery.fromBuffer(
                    javaSource("Current", "public void keystroke$version() {}"),
                ),
            )
            assertEquals("com/example/Current", index.findClassByFqn("com.example.Current")?.internalName)
            assertTrue(index.getMethods("com/example/Current").any { it.name == "keystroke$version" })
        }

        assertEquals(0, SourceClassScanner.diskReadCount())
        assertEquals(0, SourceClassScanner.diskParseCount())
        assertEquals(0, SourceClassScanner.overlayParseCount())
    }

    @Test
    fun perKeystrokeOverlayDoesNotRereadUnchangedDiskSources() {
        writeJava(sourceDir.resolve("com/example/Alpha.java"), "Alpha", "public void alphaRun() {}")
        writeJava(sourceDir.resolve("com/example/Beta.java"), "Beta", "public void betaRun() {}")
        val currentFile = sourceDir.resolve("com/example/Current.java")
        val currentUri = UriPathSupport.pathToUri(currentFile)

        repeat(30) { version ->
            val index = scannedIndex(
                currentUri = currentUri,
                currentBuffer = javaSource("Current", "public void keystroke$version() {}"),
            )
            assertEquals("com/example/Current", index.findClassByFqn("com.example.Current")?.internalName)
            assertEquals("com/example/Alpha", index.findClassByFqn("com.example.Alpha")?.internalName)
            assertEquals("com/example/Beta", index.findClassByFqn("com.example.Beta")?.internalName)
        }

        assertEquals(2, SourceClassScanner.diskReadCount())
        assertEquals(2, SourceClassScanner.diskParseCount())
        assertEquals(30, SourceClassScanner.overlayParseCount())
    }

    @Test
    fun modifyingOneDiskFileIncrementsOnlyThatFileReadAndParse() {
        val alphaFile = sourceDir.resolve("com/example/Alpha.java")
        val betaFile = sourceDir.resolve("com/example/Beta.java")
        writeJava(alphaFile, "Alpha", "public void alphaRun() {}")
        writeJava(betaFile, "Beta", "public void betaRun() {}")
        val currentUri = UriPathSupport.pathToUri(sourceDir.resolve("com/example/Current.java"))

        scannedIndex(currentUri, javaSource("Current", "public void first() {}"))
        assertEquals(2, SourceClassScanner.diskReadCount())
        assertEquals(2, SourceClassScanner.diskParseCount())
        assertEquals(1, SourceClassScanner.overlayParseCount())

        writeJava(alphaFile, "Alpha", "public void alphaUpdated() {}")
        val index = scannedIndex(currentUri, javaSource("Current", "public void second() {}"))
        assertTrue(index.getMethods("com/example/Alpha").any { it.name == "alphaUpdated" })

        assertEquals(3, SourceClassScanner.diskReadCount())
        assertEquals(3, SourceClassScanner.diskParseCount())
        assertEquals(2, SourceClassScanner.overlayParseCount())
    }

    @Test
    fun deletingDiskSourceRemovesItsClass() {
        val alphaFile = sourceDir.resolve("com/example/Alpha.java")
        writeJava(alphaFile, "Alpha", "public void alphaRun() {}")
        val currentUri = UriPathSupport.pathToUri(sourceDir.resolve("com/example/Current.java"))

        val beforeDelete = scannedIndex(currentUri, javaSource("Current", "public void first() {}"))
        assertEquals("com/example/Alpha", beforeDelete.findClassByFqn("com.example.Alpha")?.internalName)

        Files.delete(alphaFile)
        val afterDelete = scannedIndex(currentUri, javaSource("Current", "public void second() {}"))
        assertNull(afterDelete.findClassByFqn("com.example.Alpha"))
        assertEquals(1, SourceClassScanner.diskReadCount())
        assertEquals(1, SourceClassScanner.diskParseCount())
        assertEquals(2, SourceClassScanner.overlayParseCount())
    }

    @Test
    fun diskCacheStaysBoundedToExplicitMax() {
        assertEquals(8, SourceClassScanner.MaxDiskCacheEntries)

        repeat(SourceClassScanner.MaxDiskCacheEntries + 4) { index ->
            writeJava(
                sourceDir.resolve("com/example/Cache$index.java"),
                "Cache$index",
                "public void cache$index() {}",
            )
        }
        val currentUri = UriPathSupport.pathToUri(sourceDir.resolve("com/example/Overlay.java"))

        scannedIndex(currentUri, javaSource("Overlay", "public void overlay() {}"))
        assertTrue(SourceClassScanner.diskCacheSize() <= SourceClassScanner.MaxDiskCacheEntries)
        assertEquals(SourceClassScanner.MaxDiskCacheEntries + 4, SourceClassScanner.diskReadCount())
        assertEquals(SourceClassScanner.MaxDiskCacheEntries + 4, SourceClassScanner.diskParseCount())
        assertEquals(1, SourceClassScanner.overlayParseCount())
    }

    @Test
    fun largeSourceTreeDoesNotFullyRereadUnchangedDiskSourcesOnSecondScan() {
        val fileCount = 1_000
        repeat(fileCount) { index ->
            writeJava(
                sourceDir.resolve("com/example/Cache$index.java"),
                "Cache$index",
                "public void cache$index() {}",
            )
        }
        val currentUri = UriPathSupport.pathToUri(sourceDir.resolve("com/example/Overlay.java"))

        val firstIndex = scannedIndex(currentUri, javaSource("Overlay", "public void first() {}"))
        assertEquals("com/example/Cache0", firstIndex.findClassByFqn("com.example.Cache0")?.internalName)
        assertEquals(fileCount, SourceClassScanner.diskReadCount())
        assertEquals(fileCount, SourceClassScanner.diskParseCount())
        assertEquals(1, SourceClassScanner.overlayParseCount())

        val secondIndex = scannedIndex(currentUri, javaSource("Overlay", "public void second() {}"))
        assertEquals("com/example/Cache0", secondIndex.findClassByFqn("com.example.Cache0")?.internalName)
        assertEquals(fileCount, SourceClassScanner.diskReadCount())
        assertEquals(fileCount, SourceClassScanner.diskParseCount())
        assertEquals(2, SourceClassScanner.overlayParseCount())
    }

    private fun scannedIndex(currentUri: String, currentBuffer: String): SourceBackedClassIndex {
        val context = ProjectContextBuilder.empty("source-backed-index", tempDir).copy(
            sourceSets = listOf(
                SourceSetContext(
                    name = "main",
                    sourceDirectories = listOf(sourceDir),
                ),
            ),
        )
        val session = McdevProjectSession.create(context)
        return SourceBackedClassIndex(
            delegate = EmptyClassIndex,
            sourceQuery = ScannedProjectSourceSnapshot.fromSession(
                session = session,
                currentDocumentUri = currentUri,
                currentBufferText = currentBuffer,
            ),
        )
    }

    private fun writeJava(path: Path, className: String, body: String) {
        Files.createDirectories(path.parent)
        Files.writeString(path, javaSource(className, body))
    }

    private fun javaSource(className: String, body: String): String =
        """
            package com.example;
            public class $className {
                $body
            }
        """.trimIndent()

    private class ThrowingJavaProject {
        @Suppress("UNUSED_PARAMETER")
        fun findType(fqn: String): Any? = error("JDT model unavailable")
    }

    private fun entry(simpleName: String, packageName: String, internalName: String): ClassIndexEntry =
        ClassIndexEntry(
            simpleName = simpleName,
            packageName = packageName,
            internalName = internalName,
        )

    private fun method(
        name: String,
        descriptor: String,
        readableSignature: String = "$name(): void",
    ): MethodIndexEntry =
        MethodIndexEntry(
            name = name,
            descriptor = descriptor,
            isStatic = false,
            readableSignature = readableSignature,
        )

    private fun field(
        name: String,
        descriptor: String,
        isStatic: Boolean = false,
        readableType: String = "int",
    ): FieldIndexEntry =
        FieldIndexEntry(
            name = name,
            descriptor = descriptor,
            isStatic = isStatic,
            readableType = readableType,
        )

    private fun recordingSourceQuery(recordedLimits: MutableList<Int>): ProjectSourceQuery =
        object : ProjectSourceQuery {
            override fun findClasses(prefix: String, limit: Int): Sequence<ClassIndexEntry> {
                recordedLimits += limit
                return emptySequence()
            }

            override fun findClass(internalName: String): ClassIndexEntry? = null

            override fun findClassByFqn(fqn: String): ClassIndexEntry? = null

            override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> = emptyList()
        }

    private fun fixedSourceQuery(
        classesByInternal: Map<String, ClassIndexEntry> = emptyMap(),
        classesByFqn: Map<String, ClassIndexEntry> = emptyMap(),
        classesByPrefix: Map<String, List<ClassIndexEntry>> = emptyMap(),
        methodsByOwner: Map<String, List<MethodIndexEntry>> = emptyMap(),
        fieldsByOwner: Map<String, List<FieldIndexEntry>> = emptyMap(),
        coversProjectDependencies: Boolean = false,
    ): ProjectSourceQuery =
        object : ProjectSourceQuery {
            override fun findClasses(prefix: String, limit: Int): Sequence<ClassIndexEntry> =
                (classesByPrefix[prefix]?.asSequence() ?: emptySequence()).take(limit)

            override fun findClass(internalName: String): ClassIndexEntry? =
                classesByInternal[internalName]

            override fun findClassByFqn(fqn: String): ClassIndexEntry? =
                classesByFqn[fqn]

            override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> =
                methodsByOwner[ownerInternalName].orEmpty()

            override fun getFields(ownerInternalName: String): List<FieldIndexEntry> =
                fieldsByOwner[ownerInternalName].orEmpty()

            override fun coversProjectDependencies(): Boolean = coversProjectDependencies
        }

    private class RecordingClassIndex : ClassIndex {
        var delegateData: ClassIndex = EmptyClassIndex
        val invocations = mutableListOf<String>()

        override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> {
            invocations += "findClasses:$prefix:$limit"
            return delegateData.findClasses(prefix, limit)
        }

        override fun findClass(internalName: String): ClassIndexEntry? {
            invocations += "findClass:$internalName"
            return delegateData.findClass(internalName)
        }

        override fun findClassByFqn(fqn: String): ClassIndexEntry? {
            invocations += "findClassByFqn:$fqn"
            return delegateData.findClassByFqn(fqn)
        }

        override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> {
            invocations += "getMethods:$ownerInternalName"
            return delegateData.getMethods(ownerInternalName)
        }

        override fun getFields(ownerInternalName: String): List<FieldIndexEntry> {
            invocations += "getFields:$ownerInternalName"
            return delegateData.getFields(ownerInternalName)
        }
    }

    private fun recordingClassIndex(): RecordingClassIndex = RecordingClassIndex()

    private fun fixedClassIndex(
        classesByInternal: Map<String, ClassIndexEntry> = emptyMap(),
        classesByFqn: Map<String, ClassIndexEntry> = emptyMap(),
        classesByPrefix: Map<String, List<ClassIndexEntry>> = emptyMap(),
        methodsByOwner: Map<String, List<MethodIndexEntry>> = emptyMap(),
        fieldsByOwner: Map<String, List<FieldIndexEntry>> = emptyMap(),
    ): ClassIndex =
        object : ClassIndex {
            override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> =
                classesByPrefix[prefix].orEmpty().take(limit)

            override fun findClass(internalName: String): ClassIndexEntry? =
                classesByInternal[internalName]

            override fun findClassByFqn(fqn: String): ClassIndexEntry? =
                classesByFqn[fqn]

            override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> =
                methodsByOwner[ownerInternalName].orEmpty()

            override fun getFields(ownerInternalName: String): List<FieldIndexEntry> =
                fieldsByOwner[ownerInternalName].orEmpty()
        }

    private object EmptyClassIndex : ClassIndex {
        override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> = emptyList()

        override fun findClass(internalName: String): ClassIndexEntry? = null

        override fun findClassByFqn(fqn: String): ClassIndexEntry? = null

        override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> = emptyList()

        override fun getFields(ownerInternalName: String): List<FieldIndexEntry> = emptyList()
    }
}
