package io.github.mcdev.jdtls.mixin

import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FieldIndexEntry
import io.github.mcdev.core.mixin.MethodIndexEntry
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertTrue
import java.util.concurrent.CancellationException
import java.util.concurrent.CountDownLatch

class JdtProjectSourceQueryTest {
    @Test
    fun unavailableMemberApiDoesNotSuppressBytecodeFallback() {
        val query = JdtProjectSourceQuery(MissingMemberApiProject(), isJdtSearchEngineAvailable = { true })
        assertTrue(query.getMethods("example/Target").isEmpty())
        assertTrue(!query.coversProjectDependencies())
        assertTrue(query.getFields("example/Target").isEmpty())
        assertTrue(!query.coversProjectDependencies())
    }

    private class MissingMemberApiProject {
        fun findType(name: String): Any? = if (name == "example.Target") Any() else null
    }

    @Test
    fun unresolvedTypeDoesNotSuppressBytecodeFallbackDuringProjectImport() {
        val query = JdtProjectSourceQuery(FakeJavaProject(emptyMap()), isJdtSearchEngineAvailable = { true })
        assertTrue(query.getMethods("net/minecraft/client/gui/Gui").isEmpty())
        assertTrue(!query.coversProjectDependencies())
        assertNull(query.findClass("net/minecraft/client/gui/Gui"))
        assertTrue(!query.coversProjectDependencies())
    }

    @Test
    fun coversProjectDependenciesReturnsFalseWhenJdtSearchEngineIsUnavailable() {
        val query = JdtProjectSourceQuery(FakeJavaProject(emptyMap()))

        assertTrue(!query.coversProjectDependencies())
    }

    @Test
    fun coversProjectDependenciesReturnsTrueWhenJdtSearchEngineIsAvailable() {
        val query = JdtProjectSourceQuery(
            FakeJavaProject(emptyMap()),
            isJdtSearchEngineAvailable = { true },
        )

        assertTrue(query.coversProjectDependencies())
    }

    @Test
    fun findClassesReturnsEmptyWhenLimitIsZeroOrNegative() {
        val query = JdtProjectSourceQuery(FakeJavaProject(emptyMap()))

        assertTrue(query.findClasses("com", limit = 0).none())
        assertTrue(query.findClasses("com", limit = -1).none())
    }

    @Test
    fun findClassesReturnsEmptyWhenJdtSearchIsUnavailable() {
        val query = JdtProjectSourceQuery(FakeJavaProject(emptyMap()))

        assertTrue(query.findClasses("com", limit = 10).none())
    }

    @Test
    fun prefixToSearchCharsSplitsOnLastDotAndNormalizesSlashes() {
        val (packagePrefix, typePrefix) = JdtProjectSourceQuery.prefixToSearchChars("com/example/Sample")

        assertEquals("com.example", String(requireNotNull(packagePrefix)))
        assertEquals("Sample", String(typePrefix))
    }

    @Test
    fun prefixToSearchCharsUsesEntirePrefixAsTypeWhenNoDotIsPresent() {
        val (packagePrefix, typePrefix) = JdtProjectSourceQuery.prefixToSearchChars("Sample")

        assertNull(packagePrefix)
        assertEquals("Sample", String(typePrefix))
    }

    @Test
    fun requestorTypeToClassIndexEntryMapsTopLevelClass() {
        val entry = JdtProjectSourceQuery.requestorTypeToClassIndexEntry(
            packageName = "com.example".toCharArray(),
            typeName = "Sample".toCharArray(),
            enclosingTypeNames = null,
        )

        assertEquals(
            ClassIndexEntry(
                simpleName = "Sample",
                packageName = "com.example",
                internalName = "com/example/Sample",
            ),
            entry,
        )
    }

    @Test
    fun requestorTypeToClassIndexEntryMapsNestedTypesWithDollarSeparators() {
        val entry = JdtProjectSourceQuery.requestorTypeToClassIndexEntry(
            packageName = "com.example".toCharArray(),
            typeName = "Inner".toCharArray(),
            enclosingTypeNames = arrayOf("Outer".toCharArray()),
        )

        assertEquals(
            ClassIndexEntry(
                simpleName = "Outer\$Inner",
                packageName = "com.example",
                internalName = "com/example/Outer\$Inner",
            ),
            entry,
        )
    }

    @Test
    fun requestorTypeToClassIndexEntryJoinsMultipleEnclosingTypesWithDollarSeparators() {
        val entry = JdtProjectSourceQuery.requestorTypeToClassIndexEntry(
            packageName = "com.example".toCharArray(),
            typeName = "Deep".toCharArray(),
            enclosingTypeNames = arrayOf("Outer".toCharArray(), "Middle".toCharArray()),
        )

        assertEquals(
            ClassIndexEntry(
                simpleName = "Outer\$Middle\$Deep",
                packageName = "com.example",
                internalName = "com/example/Outer\$Middle\$Deep",
            ),
            entry,
        )
    }

    @Test
    fun requestorTypeToClassIndexEntryMapsInterfacesSameAsClasses() {
        val classEntry = JdtProjectSourceQuery.requestorTypeToClassIndexEntry(
            packageName = "com.example".toCharArray(),
            typeName = "Service".toCharArray(),
            enclosingTypeNames = emptyArray(),
        )
        val interfaceEntry = JdtProjectSourceQuery.requestorTypeToClassIndexEntry(
            packageName = "com.example".toCharArray(),
            typeName = "Service".toCharArray(),
            enclosingTypeNames = emptyArray(),
        )

        assertEquals(classEntry, interfaceEntry)
    }

    @Test
    fun sortClassIndexEntriesOrdersByInternalNameDeterministically() {
        val first = ClassIndexEntry("Alpha", "com.example", "com/example/Alpha")
        val second = ClassIndexEntry("Beta", "com.example", "com/example/Beta")
        val third = ClassIndexEntry("Gamma", "com.example", "com/example/Gamma")

        assertEquals(
            listOf(first, second, third),
            JdtProjectSourceQuery.sortClassIndexEntries(listOf(third, first, second)),
        )
    }

    @Test
    fun findClassAndFindClassByFqnResolveExactSourceType() {
        val sample = sourceType(
            fqn = "com.example.Sample",
            elementName = "Sample",
            packageName = "com.example",
            methods = emptyList(),
        )
        val query = JdtProjectSourceQuery(FakeJavaProject(mapOf("com.example.Sample" to sample)))

        val expected = ClassIndexEntry(
            simpleName = "Sample",
            packageName = "com.example",
            internalName = "com/example/Sample",
        )

        assertEquals(expected, query.findClass("com/example/Sample"))
        assertEquals(expected, query.findClassByFqn("com.example.Sample"))
    }

    @Test
    fun findClassPropagatesCancellationFromProjectLookup() {
        val query = JdtProjectSourceQuery(
            ThrowingJavaProject(CancellationException("cancelled")),
            isJdtSearchEngineAvailable = { true },
        )

        val thrown = assertFailsWith<CancellationException> {
            query.findClass("com/example.Cancelled")
        }

        assertEquals("cancelled", thrown.message)
    }

    @Test
    fun findClassPropagatesInterruptedExceptionFromProjectLookup() {
        val query = JdtProjectSourceQuery(
            ThrowingJavaProject(InterruptedException("interrupted")),
            isJdtSearchEngineAvailable = { true },
        )

        val thrown = assertFailsWith<InterruptedException> {
            query.findClass("com/example/Interrupted")
        }

        assertEquals("interrupted", thrown.message)
    }

    @Test
    fun findTypeFailureDisablesCoversProjectDependenciesForOperation() {
        val query = JdtProjectSourceQuery(
            ThrowingJavaProject(IllegalStateException("JDT model unavailable")),
            isJdtSearchEngineAvailable = { true },
        )

        assertNull(query.findClass("com/example/Missing"))
        assertTrue(!query.coversProjectDependencies())
        assertTrue(query.coversProjectDependencies())
    }

    @Test
    fun findTypeFailureOnOneThreadDoesNotAffectExactLookupAuthorityOnAnotherThread() {
        val authoritative = sourceType(
            fqn = "com.example.Authoritative",
            elementName = "Authoritative",
            packageName = "com.example",
            methods = emptyList(),
        )
        val javaProject = object {
            fun findType(fqn: String): Any? =
                when (fqn) {
                    "com.example.Authoritative" -> authoritative
                    "com.example.Failing" -> throw IllegalStateException("JDT model unavailable")
                    else -> null
                }
        }
        val query = JdtProjectSourceQuery(
            javaProject,
            isJdtSearchEngineAvailable = { true },
        )
        val expected = ClassIndexEntry(
            simpleName = "Authoritative",
            packageName = "com.example",
            internalName = "com/example/Authoritative",
        )

        assertEquals(expected, query.findClass("com/example/Authoritative"))

        val failureFinished = CountDownLatch(1)
        val failureThread = Thread {
            try {
                query.findClass("com/example/Failing")
            } finally {
                failureFinished.countDown()
            }
        }
        failureThread.start()
        try {
            failureFinished.await()
            assertTrue(query.coversProjectDependencies())
        } finally {
            failureThread.join()
        }
    }

    @Test
    fun missingSourceTypeDoesNotTaintTheNextSuccessfulQuery() {
        val query = JdtProjectSourceQuery(
            FakeJavaProject(mapOf("com.example.Found" to sourceType(
                fqn = "com.example.Found", elementName = "Found", packageName = "com.example", methods = emptyList(),
            ))),
            isJdtSearchEngineAvailable = { true },
        )

        assertNull(query.findClass("com/example/Missing"))
        assertTrue(!query.coversProjectDependencies())
        assertEquals("com/example/Found", query.findClass("com/example/Found")?.internalName)
        assertTrue(query.coversProjectDependencies())
    }

    @Test
    fun findClassBuildsInnerClassInternalNameWithDollarSeparator() {
        val inner = sourceType(
            fqn = "com.example.Outer.Inner",
            elementName = "Inner",
            packageName = "com.example",
            methods = emptyList(),
        )
        val query = JdtProjectSourceQuery(FakeJavaProject(mapOf("com.example.Outer.Inner" to inner)))

        assertEquals(
            ClassIndexEntry(
                simpleName = "Outer\$Inner",
                packageName = "com.example",
                internalName = "com/example/Outer\$Inner",
            ),
            query.findClass("com/example/Outer\$Inner"),
        )
    }

    @Test
    fun getMethodsReturnsOverloadedPrimitiveAndResolvedStringDescriptors() {
        val type = sourceType(
            fqn = "com.example.OverloadSamples",
            elementName = "OverloadSamples",
            packageName = "com.example",
            methods = listOf(
                FakeMethod(
                    elementName = "process",
                    parameterTypes = arrayOf("I"),
                    returnType = "V",
                ),
                FakeMethod(
                    elementName = "process",
                    parameterTypes = arrayOf("QString;"),
                    returnType = "V",
                ),
            ),
            resolveType = { erasureName ->
                when (erasureName) {
                    "String" -> arrayOf(arrayOf("java.lang", "String"))
                    else -> null
                }
            },
        )
        val query = JdtProjectSourceQuery(
            FakeJavaProject(mapOf("com.example.OverloadSamples" to type)),
        )

        val methods = query.getMethods("com/example/OverloadSamples")

        assertEquals(
            listOf("process(I)V", "process(Ljava/lang/String;)V"),
            methods.map { it.name + it.descriptor }.sorted(),
        )
        assertEquals(true, methods.all { !it.isStatic })
        assertTrue(methods.all { it.readableSignature.startsWith("process(") })
    }

    @Test
    fun getMethodsMapsConstructorsToJvmInitName() {
        val type = sourceType(
            fqn = "com.example.ConstructMe",
            elementName = "ConstructMe",
            packageName = "com.example",
            methods = listOf(
                FakeMethod(
                    elementName = "ConstructMe",
                    parameterTypes = arrayOf("I"),
                    returnType = "V",
                    constructor = true,
                ),
                FakeMethod(
                    elementName = "run",
                    parameterTypes = emptyArray(),
                    returnType = "V",
                ),
            ),
        )
        val query = JdtProjectSourceQuery(
            FakeJavaProject(mapOf("com.example.ConstructMe" to type)),
        )

        val methods = query.getMethods("com/example/ConstructMe")

        assertEquals(listOf("<init>(I)V", "run()V"), methods.map { it.name + it.descriptor })
    }

    @Test
    fun binaryTypesAreAcceptedForClassMethodAndFieldLookup() {
        val binary = sourceType(
            fqn = "com.example.BinaryOnly",
            elementName = "BinaryOnly",
            packageName = "com.example",
            binary = true,
            methods = listOf(
                FakeMethod(
                    elementName = "run",
                    parameterTypes = emptyArray(),
                    returnType = "V",
                ),
            ),
            fields = listOf(
                FakeField(
                    elementName = "value",
                    typeSignature = "I",
                ),
            ),
        )
        val query = JdtProjectSourceQuery(
            FakeJavaProject(mapOf("com.example.BinaryOnly" to binary)),
        )

        val expected = ClassIndexEntry(
            simpleName = "BinaryOnly",
            packageName = "com.example",
            internalName = "com/example/BinaryOnly",
        )

        assertEquals(expected, query.findClass("com/example/BinaryOnly"))
        assertEquals(expected, query.findClassByFqn("com.example.BinaryOnly"))
        assertEquals(listOf("run()V"), query.getMethods("com/example/BinaryOnly").map { it.name + it.descriptor })
        assertEquals(listOf("valueI"), query.getFields("com/example/BinaryOnly").map { it.name + it.descriptor })
    }

    @Test
    fun getMethodsSkipsMethodsWithUnresolvedOrAmbiguousResolvedTypes() {
        val type = sourceType(
            fqn = "com.example.SkipSamples",
            elementName = "SkipSamples",
            packageName = "com.example",
            methods = listOf(
                FakeMethod(
                    elementName = "keep",
                    parameterTypes = arrayOf("I"),
                    returnType = "V",
                ),
                FakeMethod(
                    elementName = "unresolvedParam",
                    parameterTypes = arrayOf("QMissing.Type;"),
                    returnType = "V",
                ),
                FakeMethod(
                    elementName = "ambiguousReturn",
                    parameterTypes = emptyArray(),
                    returnType = "QAmbiguous;",
                ),
            ),
            resolveType = { erasureName ->
                when (erasureName) {
                    "Ambiguous" -> arrayOf(
                        arrayOf("com.one", "Ambiguous"),
                        arrayOf("com.two", "Ambiguous"),
                    )
                    else -> null
                }
            },
        )
        val query = JdtProjectSourceQuery(
            FakeJavaProject(mapOf("com.example.SkipSamples" to type)),
        )

        val methods = query.getMethods("com/example/SkipSamples")

        assertEquals(listOf("keep(I)V"), methods.map { it.name + it.descriptor })
    }

    @Test
    fun incompleteJdtMethodResultKeepsBytecodeMethodsAvailableToMixinCompletion() {
        val type = sourceType(
            fqn = "com.example.BlockItem",
            elementName = "BlockItem",
            packageName = "com.example",
            methods = listOf(
                FakeMethod(elementName = "keep", parameterTypes = emptyArray(), returnType = "V"),
                FakeMethod(elementName = "getBlock", parameterTypes = emptyArray(), returnType = "QMissing.Type;"),
            ),
        )
        val query = JdtProjectSourceQuery(
            FakeJavaProject(mapOf("com.example.BlockItem" to type)),
            isJdtSearchEngineAvailable = { true },
        )
        assertEquals(listOf("keep()V"), query.getMethods("com/example/BlockItem").map { it.name + it.descriptor })
        assertTrue(!query.coversProjectDependencies())

        val bytecodeMethod = MethodIndexEntry(
            name = "getBlock",
            descriptor = "()Lnet/minecraft/world/level/block/Block;",
            isStatic = false,
            readableSignature = "getBlock(): Block",
        )
        fun delegate(methods: List<MethodIndexEntry>): ClassIndex = object : ClassIndex {
            override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> = emptyList()

            override fun findClass(internalName: String): ClassIndexEntry? = null

            override fun findClassByFqn(fqn: String): ClassIndexEntry? = null

            override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> = methods

            override fun getFields(ownerInternalName: String): List<FieldIndexEntry> = emptyList()
        }
        val index = SourceBackedClassIndex(delegate(listOf(bytecodeMethod)), query)

        assertEquals(
            listOf("keep()V", "getBlock()Lnet/minecraft/world/level/block/Block;"),
            index.getMethods("com/example/BlockItem").map { it.name + it.descriptor },
        )
        val sourceOnlyIndex = SourceBackedClassIndex(delegate(emptyList()), query)
        assertEquals(
            listOf("keep()V"),
            sourceOnlyIndex.getMethods("com/example/BlockItem").map { it.name + it.descriptor },
        )
    }

    @Test
    fun getMethodsMarksStaticMethodsFromFlags() {
        val type = sourceType(
            fqn = "com.example.StaticSamples",
            elementName = "StaticSamples",
            packageName = "com.example",
            methods = listOf(
                FakeMethod(
                    elementName = "instanceRun",
                    parameterTypes = emptyArray(),
                    returnType = "V",
                    flags = 0,
                ),
                FakeMethod(
                    elementName = "staticRun",
                    parameterTypes = emptyArray(),
                    returnType = "V",
                    flags = 0x0008,
                ),
            ),
        )
        val query = JdtProjectSourceQuery(
            FakeJavaProject(mapOf("com.example.StaticSamples" to type)),
        )

        val methods = query.getMethods("com/example/StaticSamples").associateBy { it.name }

        assertEquals(false, methods.getValue("instanceRun").isStatic)
        assertEquals(true, methods.getValue("staticRun").isStatic)
    }

    @Test
    fun getMethodsSkipsMethodsWhenParameterTypesContainNonStringElements() {
        val type = sourceType(
            fqn = "com.example.BadParams",
            elementName = "BadParams",
            packageName = "com.example",
            methods = listOf(
                FakeMethod(
                    elementName = "keep",
                    parameterTypes = arrayOf("I"),
                    returnType = "V",
                ),
                BadParameterTypesMethod(
                    elementName = "drop",
                    returnType = "V",
                ),
            ),
        )
        val query = JdtProjectSourceQuery(
            FakeJavaProject(mapOf("com.example.BadParams" to type)),
        )

        val methods = query.getMethods("com/example/BadParams")

        assertEquals(listOf("keep(I)V"), methods.map { it.name + it.descriptor })
    }

    @Test
    fun getMethodsSkipsMethodsWhenConstructorOrFlagsReflectionFails() {
        val type = sourceType(
            fqn = "com.example.ReflectionSamples",
            elementName = "ReflectionSamples",
            packageName = "com.example",
            methods = listOf(
                FakeMethod(
                    elementName = "keep",
                    parameterTypes = emptyArray(),
                    returnType = "V",
                ),
                MissingConstructorFlagMethod(
                    elementName = "missingConstructorFlag",
                    parameterTypes = emptyArray(),
                    returnType = "V",
                ),
                MissingFlagsMethod(
                    elementName = "missingFlags",
                    parameterTypes = emptyArray(),
                    returnType = "V",
                ),
            ),
        )
        val query = JdtProjectSourceQuery(
            FakeJavaProject(mapOf("com.example.ReflectionSamples" to type)),
        )

        val methods = query.getMethods("com/example/ReflectionSamples")

        assertEquals(listOf("keep()V"), methods.map { it.name + it.descriptor })
    }

    @Test
    fun resolveTypeAcceptsExactlyOneUniquePairAfterDeduplication() {
        val type = sourceType(
            fqn = "com.example.DedupSamples",
            elementName = "DedupSamples",
            packageName = "com.example",
            methods = listOf(
                FakeMethod(
                    elementName = "resolved",
                    parameterTypes = arrayOf("QString;"),
                    returnType = "V",
                ),
            ),
            resolveType = { erasureName ->
                when (erasureName) {
                    "String" -> arrayOf(
                        arrayOf("java.lang", "String"),
                        arrayOf("java.lang", "String"),
                    )
                    else -> null
                }
            },
        )
        val query = JdtProjectSourceQuery(
            FakeJavaProject(mapOf("com.example.DedupSamples" to type)),
        )

        val methods = query.getMethods("com/example/DedupSamples")

        assertEquals(listOf("resolved(Ljava/lang/String;)V"), methods.map { it.name + it.descriptor })
    }

    @Test
    fun getFieldsReturnsPrimitiveAndResolvedStringDescriptors() {
        val type = sourceType(
            fqn = "com.example.FieldSamples",
            elementName = "FieldSamples",
            packageName = "com.example",
            methods = emptyList(),
            fields = listOf(
                FakeField(
                    elementName = "count",
                    typeSignature = "I",
                ),
                FakeField(
                    elementName = "label",
                    typeSignature = "QString;",
                ),
            ),
            resolveType = { erasureName ->
                when (erasureName) {
                    "String" -> arrayOf(arrayOf("java.lang", "String"))
                    else -> null
                }
            },
        )
        val query = JdtProjectSourceQuery(
            FakeJavaProject(mapOf("com.example.FieldSamples" to type)),
        )

        val fields = query.getFields("com/example/FieldSamples")

        assertEquals(
            listOf("countI", "labelLjava/lang/String;"),
            fields.map { it.name + it.descriptor }.sorted(),
        )
        assertEquals("int", fields.single { it.name == "count" }.readableType)
        assertEquals("String", fields.single { it.name == "label" }.readableType)
    }

    @Test
    fun getFieldsMarksStaticFieldsFromFlags() {
        val type = sourceType(
            fqn = "com.example.StaticFieldSamples",
            elementName = "StaticFieldSamples",
            packageName = "com.example",
            methods = emptyList(),
            fields = listOf(
                FakeField(
                    elementName = "instanceValue",
                    typeSignature = "I",
                    flags = 0,
                ),
                FakeField(
                    elementName = "staticValue",
                    typeSignature = "I",
                    flags = 0x0008,
                ),
            ),
        )
        val query = JdtProjectSourceQuery(
            FakeJavaProject(mapOf("com.example.StaticFieldSamples" to type)),
        )

        val fields = query.getFields("com/example/StaticFieldSamples").associateBy { it.name }

        assertEquals(false, fields.getValue("instanceValue").isStatic)
        assertEquals(true, fields.getValue("staticValue").isStatic)
    }

    @Test
    fun getFieldsSkipsFieldsWithUnresolvedOrAmbiguousResolvedTypes() {
        val type = sourceType(
            fqn = "com.example.SkipFieldSamples",
            elementName = "SkipFieldSamples",
            packageName = "com.example",
            methods = emptyList(),
            fields = listOf(
                FakeField(
                    elementName = "keep",
                    typeSignature = "I",
                ),
                FakeField(
                    elementName = "unresolved",
                    typeSignature = "QMissing.Type;",
                ),
                FakeField(
                    elementName = "ambiguous",
                    typeSignature = "QAmbiguous;",
                ),
            ),
            resolveType = { erasureName ->
                when (erasureName) {
                    "Ambiguous" -> arrayOf(
                        arrayOf("com.one", "Ambiguous"),
                        arrayOf("com.two", "Ambiguous"),
                    )
                    else -> null
                }
            },
        )
        val query = JdtProjectSourceQuery(
            FakeJavaProject(mapOf("com.example.SkipFieldSamples" to type)),
        )

        val fields = query.getFields("com/example/SkipFieldSamples")

        assertEquals(listOf("keepI"), fields.map { it.name + it.descriptor })
    }

    @Test
    fun incompleteJdtFieldResultKeepsBytecodeFieldsAvailableToMixinCompletion() {
        val type = sourceType(
            fqn = "com.example.BlockItem",
            elementName = "BlockItem",
            packageName = "com.example",
            methods = emptyList(),
            fields = listOf(
                FakeField(elementName = "keep", typeSignature = "I"),
                FakeField(elementName = "block", typeSignature = "QMissing.Type;"),
            ),
        )
        val query = JdtProjectSourceQuery(
            FakeJavaProject(mapOf("com.example.BlockItem" to type)),
            isJdtSearchEngineAvailable = { true },
        )
        assertEquals(listOf("keepI"), query.getFields("com/example/BlockItem").map { it.name + it.descriptor })
        assertTrue(!query.coversProjectDependencies())

        val bytecodeField = FieldIndexEntry(
            name = "block",
            descriptor = "Lnet/minecraft/world/level/block/Block;",
            isStatic = false,
            readableType = "Block",
        )
        fun delegate(fields: List<FieldIndexEntry>): ClassIndex = object : ClassIndex {
            override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> = emptyList()

            override fun findClass(internalName: String): ClassIndexEntry? = null

            override fun findClassByFqn(fqn: String): ClassIndexEntry? = null

            override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> = emptyList()

            override fun getFields(ownerInternalName: String): List<FieldIndexEntry> = fields
        }
        val index = SourceBackedClassIndex(delegate(listOf(bytecodeField)), query)

        assertEquals(
            listOf("keepI", "blockLnet/minecraft/world/level/block/Block;"),
            index.getFields("com/example/BlockItem").map { it.name + it.descriptor },
        )
        val sourceOnlyIndex = SourceBackedClassIndex(delegate(emptyList()), query)
        assertEquals(
            listOf("keepI"),
            sourceOnlyIndex.getFields("com/example/BlockItem").map { it.name + it.descriptor },
        )
    }

    @Test
    fun getFieldsSkipsFieldsWhenReflectionFails() {
        val type = sourceType(
            fqn = "com.example.ReflectionFieldSamples",
            elementName = "ReflectionFieldSamples",
            packageName = "com.example",
            methods = emptyList(),
            fields = listOf(
                FakeField(
                    elementName = "keep",
                    typeSignature = "I",
                ),
                MissingTypeSignatureField(
                    elementName = "missingTypeSignature",
                ),
                MissingFlagsField(
                    elementName = "missingFlags",
                    typeSignature = "I",
                ),
            ),
        )
        val query = JdtProjectSourceQuery(
            FakeJavaProject(mapOf("com.example.ReflectionFieldSamples" to type)),
        )

        val fields = query.getFields("com/example/ReflectionFieldSamples")

        assertEquals(listOf("keepI"), fields.map { it.name + it.descriptor })
    }

    private fun sourceType(
        fqn: String,
        elementName: String,
        packageName: String,
        methods: List<Any>,
        fields: List<Any> = emptyList(),
        binary: Boolean = false,
        resolveType: (String) -> Array<Array<String>>? = { null },
    ): FakeType =
        FakeType(
            fqn = fqn,
            elementName = elementName,
            packageName = packageName,
            binary = binary,
            methods = methods,
            fields = fields,
            resolveType = resolveType,
        )

    private class FakeJavaProject(
        private val typesByFqn: Map<String, FakeType>,
    ) {
        fun findType(fqn: String): FakeType? = typesByFqn[fqn]
    }

    private class ThrowingJavaProject(
        private val failure: Throwable,
    ) {
        @Suppress("UNUSED_PARAMETER")
        fun findType(fqn: String): Any? = throw failure
    }

    private class FakePackageFragment(
        private val name: String,
    ) {
        fun getElementName(): String = name
    }

    private class FakeType(
        val fqn: String,
        private val elementName: String,
        private val packageName: String,
        private val binary: Boolean,
        private val methods: List<Any>,
        private val fields: List<Any>,
        private val resolveType: (String) -> Array<Array<String>>?,
    ) {
        fun isBinary(): Boolean = binary

        fun getElementName(): String = elementName

        fun getPackageFragment(): FakePackageFragment = FakePackageFragment(packageName)

        fun getFullyQualifiedName(separator: Char, includePackage: Boolean): String =
            fullyQualifiedName(separator, includePackage)

        fun getFullyQualifiedName(separator: Char): String =
            fullyQualifiedName(separator, includePackage = true)

        fun getFullyQualifiedName(): String = fqn

        fun getMethods(): Array<Any> = methods.toTypedArray()

        fun getFields(): Array<Any> = fields.toTypedArray()

        fun resolveType(typeName: String): Array<Array<String>>? = resolveType.invoke(typeName)

        private fun fullyQualifiedName(separator: Char, includePackage: Boolean): String {
            if (!includePackage) {
                return elementName
            }
            if (separator == '.') {
                return fqn
            }
            val classPart = if (packageName.isEmpty()) {
                fqn
            } else {
                fqn.removePrefix("$packageName.")
            }
            val classWithDollar = classPart.replace('.', '$')
            return if (packageName.isEmpty()) classWithDollar else "$packageName.$classWithDollar"
        }
    }

    private class FakeMethod(
        private val elementName: String,
        private val parameterTypes: Array<String>,
        private val returnType: String,
        private val flags: Int = 0,
        private val constructor: Boolean = false,
    ) {
        fun isConstructor(): Boolean = constructor

        fun getElementName(): String = elementName

        fun getParameterTypes(): Array<String> = parameterTypes

        fun getReturnType(): String = returnType

        fun getFlags(): Int = flags
    }

    private class FakeField(
        private val elementName: String,
        private val typeSignature: String,
        private val flags: Int = 0,
    ) {
        fun getElementName(): String = elementName

        fun getTypeSignature(): String = typeSignature

        fun getFlags(): Int = flags
    }

    private class MissingTypeSignatureField(
        private val elementName: String,
    ) {
        fun getElementName(): String = elementName

        fun getFlags(): Int = 0
    }

    private class MissingFlagsField(
        private val elementName: String,
        private val typeSignature: String,
    ) {
        fun getElementName(): String = elementName

        fun getTypeSignature(): String = typeSignature
    }

    private class BadParameterTypesMethod(
        private val elementName: String,
        private val returnType: String,
    ) {
        fun getElementName(): String = elementName

        fun getParameterTypes(): Array<Any> = arrayOf("I", 123)

        fun getReturnType(): String = returnType

        fun isConstructor(): Boolean = false

        fun getFlags(): Int = 0
    }

    private class MissingConstructorFlagMethod(
        private val elementName: String,
        private val parameterTypes: Array<String>,
        private val returnType: String,
    ) {
        fun getElementName(): String = elementName

        fun getParameterTypes(): Array<String> = parameterTypes

        fun getReturnType(): String = returnType

        fun getFlags(): Int = 0
    }

    private class MissingFlagsMethod(
        private val elementName: String,
        private val parameterTypes: Array<String>,
        private val returnType: String,
    ) {
        fun getElementName(): String = elementName

        fun getParameterTypes(): Array<String> = parameterTypes

        fun getReturnType(): String = returnType

        fun isConstructor(): Boolean = false
    }
}
