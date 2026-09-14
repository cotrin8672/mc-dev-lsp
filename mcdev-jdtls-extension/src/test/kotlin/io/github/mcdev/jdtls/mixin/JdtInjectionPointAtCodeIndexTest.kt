package io.github.mcdev.jdtls.mixin

import java.lang.reflect.Method
import java.lang.reflect.Proxy
import java.util.concurrent.Callable
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class JdtInjectionPointAtCodeIndexTest {
    private val injectionPointFqn = "org.spongepowered.asm.mixin.injection.InjectionPoint"
    private val atCodeFqn = "org.spongepowered.asm.mixin.injection.InjectionPoint\$AtCode"
    private val spongeBuiltinFqn =
        "org.spongepowered.asm.mixin.injection.ModifyConstantInjectionPoint"

    @Test
    fun annotatedDefaultNamespaceReturnsBareValue() {
        val index = indexWithSubtypes(
            FakeSubtype("com.example.AnnotatedDefault", atCodeValue = "MYPOINT"),
        )

        assertEquals(listOf("MYPOINT"), index.getValues(FakeJavaProject("p1"), 0L))
    }

    @Test
    fun annotatedCustomNamespacePreservesDeclaredText() {
        val index = indexWithSubtypes(
            FakeSubtype(
                fqn = "com.example.AnnotatedCustom",
                atCodeNamespace = "MyMod",
                atCodeValue = "Hook",
            ),
        )

        assertEquals(listOf("MyMod:Hook"), index.getValues(FakeJavaProject("p1"), 0L))
    }

    @Test
    fun unannotatedCustomFqnIsIncludedAsRuntimeFallback() {
        val customFqn = "com.example.UnannotatedCustomPoint"
        val index = indexWithSubtypes(FakeSubtype(customFqn))

        assertEquals(listOf(customFqn), index.getValues(FakeJavaProject("p1"), 0L))
    }

    @Test
    fun unannotatedSpongeBuiltInSubclassIsExcluded() {
        val index = indexWithSubtypes(FakeSubtype(spongeBuiltinFqn))

        assertEquals(emptyList(), index.getValues(FakeJavaProject("p1"), 0L))
    }

    @Test
    fun missingInjectionPointFailsClosedToEmpty() {
        val index = JdtInjectionPointAtCodeIndex(
            hierarchyQuery = InjectionPointHierarchyQuery { emptyList() },
        )

        assertEquals(emptyList(), index.getValues(FakeJavaProject("missing"), 0L))
    }

    @Test
    fun nullProjectFailsClosedToEmpty() {
        val index = JdtInjectionPointAtCodeIndex()

        assertEquals(emptyList(), index.getValues(null, 0L))
    }

    @Test
    fun transientHierarchyFailureDoesNotCacheEmptyAndRetriesOnNextRequest() {
        val builds = AtomicInteger(0)
        val index = JdtInjectionPointAtCodeIndex(
            hierarchyQuery = InjectionPointHierarchyQuery { _ ->
                if (builds.incrementAndGet() == 1) {
                    throw IllegalStateException("transient hierarchy failure")
                }
                listOf(
                    InjectionPointHierarchySubtype(
                        type = FakeType("com.example.AnnotatedDefault"),
                        fqn = "com.example.AnnotatedDefault",
                    ),
                )
            },
            atCodeReader = AtCodeTypeAnnotationReader { _, _ -> "MYPOINT" },
        )
        val project = FakeJavaProject("transient-failure")

        assertEquals(emptyList(), index.getValues(project, 99L))
        assertEquals(listOf("MYPOINT"), index.getValues(project, 99L))
        assertEquals(2, builds.get())
        assertEquals(listOf("MYPOINT"), index.getValues(project, 99L))
        assertEquals(2, builds.get())
    }

    @Test
    fun cachesExactlyOnceForSameProjectIdentityAndVersion() {
        var builds = 0
        val index = countingIndex(onBuild = { builds++ })
        val project = FakeJavaProject("cache-hit")

        assertEquals(listOf("MYPOINT"), index.getValues(project, 7L))
        assertEquals(listOf("MYPOINT"), index.getValues(project, 7L))
        assertEquals(1, builds)
    }

    @Test
    fun recomputesWhenProjectSessionVersionChanges() {
        var builds = 0
        val index = countingIndex(onBuild = { builds++ })
        val project = FakeJavaProject("version-change")

        assertEquals(listOf("MYPOINT"), index.getValues(project, 1L))
        assertEquals(listOf("MYPOINT"), index.getValues(project, 2L))
        assertEquals(2, builds)
    }

    @Test
    fun concurrentIdenticalRequestsPerformSingleHierarchyBuild() {
        val builds = AtomicInteger(0)
        val readyGate = CountDownLatch(32)
        val startGate = CountDownLatch(1)
        val index = JdtInjectionPointAtCodeIndex(
            hierarchyQuery = InjectionPointHierarchyQuery { project ->
                assertEquals("same-key", (project as FakeJavaProject).id)
                builds.incrementAndGet()
                startGate.await(5, TimeUnit.SECONDS)
                listOf(
                    InjectionPointHierarchySubtype(
                        type = FakeType("com.example.AnnotatedDefault"),
                        fqn = "com.example.AnnotatedDefault",
                    ),
                )
            },
            atCodeReader = AtCodeTypeAnnotationReader { _, _ -> "MYPOINT" },
        )
        val project = FakeJavaProject("same-key")
        val executor = Executors.newFixedThreadPool(32)
        val results = Array(32) { emptyList<String>() }
        try {
            repeat(32) { indexInArray ->
                executor.submit {
                    readyGate.countDown()
                    results[indexInArray] = index.getValues(project, 42L)
                }
            }
            assertTrue(readyGate.await(5, TimeUnit.SECONDS), "worker threads did not become ready")
            val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
            while (builds.get() < 1 && System.nanoTime() < deadline) {
                Thread.yield()
            }
            assertEquals(1, builds.get(), "expected exactly one hierarchy build")
            startGate.countDown()
            executor.shutdown()
            assertTrue(executor.awaitTermination(5, TimeUnit.SECONDS), "executor did not finish in time")
            assertEquals(1, builds.get())
            assertTrue(results.all { it == listOf("MYPOINT") })
        } finally {
            startGate.countDown()
            executor.shutdownNow()
            executor.awaitTermination(5, TimeUnit.SECONDS)
        }
    }

    @Test
    fun distinctProjectsDoNotGloballySerializeHierarchyBuild() {
        val builds = AtomicInteger(0)
        val firstReady = CountDownLatch(1)
        val secondReady = CountDownLatch(1)
        val release = CountDownLatch(2)
        val index = JdtInjectionPointAtCodeIndex(
            hierarchyQuery = InjectionPointHierarchyQuery { project ->
                val fakeProject = project as FakeJavaProject
                builds.incrementAndGet()
                when (fakeProject.id) {
                    "project-a" -> {
                        firstReady.countDown()
                        release.await(5, TimeUnit.SECONDS)
                    }
                    "project-b" -> {
                        secondReady.countDown()
                        release.await(5, TimeUnit.SECONDS)
                    }
                    else -> error("unexpected project ${fakeProject.id}")
                }
                listOf(
                    InjectionPointHierarchySubtype(
                        type = FakeType("com.example.${fakeProject.id}"),
                        fqn = "com.example.${fakeProject.id}",
                    ),
                )
            },
            atCodeReader = AtCodeTypeAnnotationReader { type, _ ->
                (type as FakeType).fqn.substringAfterLast('.')
            },
        )
        val executor = Executors.newFixedThreadPool(2)
        try {
            val firstFuture = executor.submit(
                Callable {
                    index.getValues(FakeJavaProject("project-a"), 1L)
                },
            )
            val secondFuture = executor.submit(
                Callable {
                    index.getValues(FakeJavaProject("project-b"), 1L)
                },
            )
            assertTrue(firstReady.await(5, TimeUnit.SECONDS), "project-a did not begin build")
            assertTrue(secondReady.await(5, TimeUnit.SECONDS), "project-b did not begin build")
            assertEquals(2, builds.get(), "expected both projects to build concurrently")
            release.countDown()
            release.countDown()
            assertEquals(listOf("project-a"), firstFuture.get(5, TimeUnit.SECONDS))
            assertEquals(listOf("project-b"), secondFuture.get(5, TimeUnit.SECONDS))
        } finally {
            release.countDown()
            release.countDown()
            executor.shutdownNow()
            executor.awaitTermination(5, TimeUnit.SECONDS)
        }
    }

    @Test
    fun formatAtCodeValuePreservesDeclaredNamespaceAndValue() {
        assertEquals("mixin:HEAD", formatAtCodeValue(namespace = "mixin", value = "HEAD"))
        assertEquals("MyMod:Hook", formatAtCodeValue(namespace = "MyMod", value = "Hook"))
        assertEquals("VALUE", formatAtCodeValue(namespace = null, value = "VALUE"))
        assertEquals("VALUE", formatAtCodeValue(namespace = "", value = "VALUE"))
        assertEquals("VALUE", formatAtCodeValue(namespace = "   ", value = "VALUE"))
    }

    @Test
    fun reflectiveHierarchyQueryUsesTypeNewTypeHierarchyAndDollarFqn() {
        val annotatedSubtype = createReflectiveJdtType(
            fqn = "com.example.AnnotatedPoint",
            annotations = listOf(
                createReflectiveJdtAnnotation(
                    elementName = atCodeFqn,
                    pairs = mapOf("value" to "MYPOINT"),
                ),
            ),
        )
        val hierarchyBuiltOn = AtomicReference<Any>()
        val injectionPoint = createReflectiveJdtType(
            fqn = injectionPointFqn,
            subtypes = listOf(annotatedSubtype),
            onHierarchyBuilt = { hierarchyBuiltOn.set(it) },
        )
        val project = createReflectiveJdtJavaProject(
            types = mapOf(injectionPointFqn to injectionPoint),
        )

        val subtypes = ReflectiveInjectionPointHierarchyQuery.querySubtypes(project)

        assertEquals(1, subtypes.size)
        assertEquals("com.example.AnnotatedPoint", subtypes.single().fqn)
        assertEquals(injectionPoint, hierarchyBuiltOn.get())
    }

    @Test
    fun reflectiveHierarchyQueryPrefersDollarSeparatorFqnWithDotFallback() {
        val subtype = createReflectiveJdtType(
            fqn = "com.example.FallbackOnly",
            dollarFqn = null,
        )
        val injectionPoint = createReflectiveJdtType(
            fqn = injectionPointFqn,
            subtypes = listOf(subtype),
        )
        val project = createReflectiveJdtJavaProject(
            types = mapOf(injectionPointFqn to injectionPoint),
        )

        val subtypes = ReflectiveInjectionPointHierarchyQuery.querySubtypes(project)

        assertEquals(listOf("com.example.FallbackOnly"), subtypes.map { it.fqn })
    }

    @Test
    fun reflectiveAtCodeReaderMatchesElementNameVariantsViaMemberValuePairs() {
        val reader = ReflectiveAtCodeTypeAnnotationReader
        val variants = listOf(
            atCodeFqn,
            "org.spongepowered.asm.mixin.injection.InjectionPoint.AtCode",
            "AtCode",
        )
        for (elementName in variants) {
            val type = createReflectiveJdtType(
                fqn = "com.example.Variant",
                annotations = listOf(
                    createReflectiveJdtAnnotation(
                        elementName = elementName,
                        pairs = mapOf("value" to "POINT_${elementName.hashCode()}"),
                    ),
                ),
            )
            assertEquals(
                "POINT_${elementName.hashCode()}",
                reader.readDeclaredAtCode(type, atCodeFqn),
            )
        }
    }

    @Test
    fun reflectiveAtCodeReaderIgnoresNonExistentAnnotations() {
        val type = createReflectiveJdtType(
            fqn = "com.example.Missing",
            annotations = listOf(
                createReflectiveJdtAnnotation(
                    elementName = atCodeFqn,
                    pairs = mapOf("value" to "SHOULD_NOT_READ"),
                    exists = false,
                ),
            ),
        )

        assertEquals(null, ReflectiveAtCodeTypeAnnotationReader.readDeclaredAtCode(type, atCodeFqn))
    }

    @Test
    fun reflectiveAtCodeReaderFormatsNamespaceFromMemberValuePairs() {
        val bareType = createReflectiveJdtType(
            fqn = "com.example.Bare",
            annotations = listOf(
                createReflectiveJdtAnnotation(
                    elementName = atCodeFqn,
                    pairs = mapOf("value" to "HEAD"),
                ),
            ),
        )
        val mixinType = createReflectiveJdtType(
            fqn = "com.example.MixinNamespace",
            annotations = listOf(
                createReflectiveJdtAnnotation(
                    elementName = atCodeFqn,
                    pairs = mapOf("namespace" to "mixin", "value" to "HEAD"),
                ),
            ),
        )
        val customType = createReflectiveJdtType(
            fqn = "com.example.CustomNamespace",
            annotations = listOf(
                createReflectiveJdtAnnotation(
                    elementName = atCodeFqn,
                    pairs = mapOf("namespace" to "MyMod", "value" to "Hook"),
                ),
            ),
        )
        val reader = ReflectiveAtCodeTypeAnnotationReader

        assertEquals("HEAD", reader.readDeclaredAtCode(bareType, atCodeFqn))
        assertEquals("mixin:HEAD", reader.readDeclaredAtCode(mixinType, atCodeFqn))
        assertEquals("MyMod:Hook", reader.readDeclaredAtCode(customType, atCodeFqn))
    }

    @Test
    fun reflectiveEndToEndIndexUsesProductionReadersAgainstJdtShapedFakes() {
        val annotatedSubtype = createReflectiveJdtType(
            fqn = "com.example.EndToEnd",
            annotations = listOf(
                createReflectiveJdtAnnotation(
                    elementName = "AtCode",
                    pairs = mapOf("namespace" to "mixin", "value" to "TAIL"),
                ),
            ),
        )
        val unannotatedCustom = createReflectiveJdtType(fqn = "com.example.CustomFallback")
        val unannotatedSponge = createReflectiveJdtType(fqn = spongeBuiltinFqn)
        val injectionPoint = createReflectiveJdtType(
            fqn = injectionPointFqn,
            subtypes = listOf(annotatedSubtype, unannotatedCustom, unannotatedSponge),
        )
        val project = createReflectiveJdtJavaProject(
            types = mapOf(injectionPointFqn to injectionPoint),
        )
        val index = JdtInjectionPointAtCodeIndex()

        assertEquals(
            listOf("com.example.CustomFallback", "mixin:TAIL"),
            index.getValues(project, 0L),
        )
    }

    private fun indexWithSubtypes(vararg subtypes: FakeSubtype): JdtInjectionPointAtCodeIndex =
        JdtInjectionPointAtCodeIndex(
            hierarchyQuery = InjectionPointHierarchyQuery { _ ->
                subtypes.map { subtype ->
                    InjectionPointHierarchySubtype(
                        type = FakeType(subtype.fqn),
                        fqn = subtype.fqn,
                    )
                }
            },
            atCodeReader = AtCodeTypeAnnotationReader { type, _ ->
                val fakeType = type as FakeType
                val subtype = subtypes.first { it.fqn == fakeType.fqn }
                subtype.atCodeValue?.let { value ->
                    formatAtCodeValue(subtype.atCodeNamespace, value)
                }
            },
        )

    private fun countingIndex(onBuild: () -> Unit): JdtInjectionPointAtCodeIndex =
        JdtInjectionPointAtCodeIndex(
            hierarchyQuery = InjectionPointHierarchyQuery { _ ->
                onBuild()
                listOf(
                    InjectionPointHierarchySubtype(
                        type = FakeType("com.example.AnnotatedDefault"),
                        fqn = "com.example.AnnotatedDefault",
                    ),
                )
            },
            atCodeReader = AtCodeTypeAnnotationReader { _, _ -> "MYPOINT" },
        )

    private data class FakeSubtype(
        val fqn: String,
        val atCodeNamespace: String? = null,
        val atCodeValue: String? = null,
    )

    private class FakeJavaProject(val id: String)

    private class FakeType(val fqn: String)

    private fun createReflectiveJdtJavaProject(types: Map<String, Any>): Any {
        val iJavaProjectClass = Class.forName("org.eclipse.jdt.core.IJavaProject")
        return Proxy.newProxyInstance(
            iJavaProjectClass.classLoader,
            arrayOf(iJavaProjectClass),
        ) { self, method, args ->
            when (method.name) {
                "findType" -> types[args[0] as String]
                else -> defaultProxyValue(self, method, args)
            }
        }
    }

    private data class ReflectiveJdtTypeSpec(
        val fqn: String,
        val dollarFqn: String?,
        val subtypes: List<Any>,
        val annotations: List<Any>,
        val onHierarchyBuilt: ((Any) -> Unit)?,
        var root: Any? = null,
    )

    private fun createReflectiveJdtType(
        fqn: String,
        dollarFqn: String? = fqn,
        subtypes: List<Any> = emptyList(),
        annotations: List<Any> = emptyList(),
        onHierarchyBuilt: ((Any) -> Unit)? = null,
    ): Any {
        val iTypeClass = Class.forName("org.eclipse.jdt.core.IType")
        val spec = ReflectiveJdtTypeSpec(
            fqn = fqn,
            dollarFqn = dollarFqn,
            subtypes = subtypes,
            annotations = annotations,
            onHierarchyBuilt = onHierarchyBuilt,
        )
        val proxy = Proxy.newProxyInstance(
            iTypeClass.classLoader,
            arrayOf(iTypeClass),
        ) { self, method, args ->
            when (method.name) {
                "newTypeHierarchy" -> {
                    onHierarchyBuilt?.invoke(self)
                    createReflectiveJdtHierarchy(
                        root = spec.root ?: self,
                        subtypes = spec.subtypes,
                    )
                }
                "getFullyQualifiedName" -> when (method.parameterCount) {
                    0 -> spec.fqn
                    1 -> {
                        val separator = (args[0] as Character).charValue()
                        check(separator == '$') { "unexpected separator $separator" }
                        spec.dollarFqn ?: error("dollarFqn unavailable")
                    }
                    else -> error("unexpected getFullyQualifiedName overload")
                }
                "getAnnotations" -> typedArray(
                    Class.forName("org.eclipse.jdt.core.IAnnotation"),
                    spec.annotations,
                )
                else -> defaultProxyValue(self, method, args)
            }
        }
        spec.root = proxy
        return proxy
    }

    private fun createReflectiveJdtHierarchy(root: Any, subtypes: List<Any>): Any {
        val iTypeHierarchyClass = Class.forName("org.eclipse.jdt.core.ITypeHierarchy")
        val iTypeClass = Class.forName("org.eclipse.jdt.core.IType")
        return Proxy.newProxyInstance(
            iTypeHierarchyClass.classLoader,
            arrayOf(iTypeHierarchyClass),
        ) { self, method, args ->
            when (method.name) {
                "getAllSubtypes" -> {
                    check(args[0] === root) { "getAllSubtypes called with unexpected root" }
                    typedArray(iTypeClass, subtypes)
                }
                else -> defaultProxyValue(self, method, args)
            }
        }
    }

    private fun createReflectiveJdtAnnotation(
        elementName: String,
        pairs: Map<String, String>,
        exists: Boolean = true,
    ): Any {
        val annotationClass = Class.forName("org.eclipse.jdt.core.IAnnotation")
        val pairClass = Class.forName("org.eclipse.jdt.core.IMemberValuePair")
        val memberPairs = pairs.map { (name, value) ->
            Proxy.newProxyInstance(
                pairClass.classLoader,
                arrayOf(pairClass),
            ) { self, method, _ ->
                when (method.name) {
                    "getMemberName" -> name
                    "getValue" -> value
                    else -> defaultProxyValue(self, method, null)
                }
            }
        }
        return Proxy.newProxyInstance(
            annotationClass.classLoader,
            arrayOf(annotationClass),
        ) { self, method, _ ->
            when (method.name) {
                "exists" -> exists
                "getElementName" -> elementName
                "getMemberValuePairs" -> typedArray(pairClass, memberPairs)
                else -> defaultProxyValue(self, method, null)
            }
        }
    }

    private fun typedArray(componentType: Class<*>, elements: List<Any>): Any {
        val array = java.lang.reflect.Array.newInstance(componentType, elements.size)
        elements.forEachIndexed { index, element ->
            java.lang.reflect.Array.set(array, index, element)
        }
        return array
    }

    private fun defaultProxyValue(proxy: Any, method: Method, args: Array<Any?>?): Any? =
        when (method.name) {
            "equals" -> proxy === args?.get(0)
            "hashCode" -> System.identityHashCode(proxy)
            "toString" -> "${proxy.javaClass.name}@${System.identityHashCode(proxy)}"
            else -> throw UnsupportedOperationException("unexpected method ${method.name}")
        }
}
