package io.github.mcdev.jdtls.mixin

import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.AtTargetKind
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FieldIndexEntry
import io.github.mcdev.core.mixin.MethodIndexEntry
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertSame
import kotlin.test.assertTrue

class CompletionIndexCachesTest {
    @Test
    fun classIndexMemberCacheDoesNotCrossContaminateDifferentDelegates() {
        val owner = "example/Target"
        val projectSessionVersion = 1L
        val caches = CompletionIndexCaches()

        val firstDelegate = stubClassIndex(
            methods = mapOf(owner to listOf(method("first"))),
        )
        val secondDelegate = stubClassIndex(
            methods = mapOf(owner to listOf(method("second"))),
        )

        val cachedFirst = caches.classIndex(firstDelegate, projectSessionVersion)
        val cachedSecond = caches.classIndex(secondDelegate, projectSessionVersion)

        assertEquals(listOf("first"), cachedFirst.getMethods(owner).map { it.name })
        assertEquals(listOf("second"), cachedSecond.getMethods(owner).map { it.name })
    }

    @Test
    fun classIndexMemberCacheIsBoundedAcrossProjectSessionVersions() {
        val owner = "example/Target"
        val caches = CompletionIndexCaches()
        val delegate = stubClassIndex(
            methods = mapOf(owner to listOf(method("run"))),
        )

        repeat(600) { version ->
            caches.classIndex(delegate, projectSessionVersion = version.toLong()).getMethods(owner)
        }

        val actualSize = memberMethodsCacheSize(caches)
        assertTrue(
            actualSize <= 256,
            "member methods cache size $actualSize exceeds bound of 256",
        )
    }

    @Test
    fun bytecodeIndexAtTargetCacheIsBoundedAcrossProjectSessionVersions() {
        val owner = "example/Target"
        val methodName = "run"
        val methodDescriptor = "()V"
        val atValue = "INVOKE"
        val caches = CompletionIndexCaches()
        val delegate = stubBytecodeIndex(
            candidates = mapOf(
                "$owner#$methodName#$atValue" to listOf(atTargetCandidate("hook")),
            ),
        )

        repeat(600) { version ->
            caches
                .bytecodeIndex(delegate, projectSessionVersion = version.toLong())
                .getAtTargetCandidates(owner, methodName, methodDescriptor, atValue)
        }

        val actualSize = atTargetsCacheSize(caches)
        assertTrue(
            actualSize <= 256,
            "atTargets cache size $actualSize exceeds bound of 256",
        )
    }

    @Test
    fun bytecodeIndexGetClassBytesDelegatesExactBytesAndMissingStaysNull() {
        val owner = "example/Target"
        val classBytes = byteArrayOf(0xCA.toByte(), 0xFE.toByte(), 0xBA.toByte(), 0xBE.toByte())
        val projectSessionVersion = 1L
        val caches = CompletionIndexCaches()
        val delegate = stubBytecodeIndex(classBytes = mapOf(owner to classBytes))
        val wrapped = caches.bytecodeIndex(delegate, projectSessionVersion)

        caches.resetDebug()
        assertEquals(0, atTargetsCacheSize(caches))

        val delegateBytes = delegate.getClassBytes(owner)
        val wrappedBytes = wrapped.getClassBytes(owner)

        assertSame(delegateBytes, wrappedBytes)
        assertEquals(classBytes.toList(), wrappedBytes?.toList())
        assertNull(wrapped.getClassBytes("missing/Class"))

        assertEquals(0, atTargetsCacheSize(caches))
        assertEquals(CandidateCacheDebug(hit = false, buildMs = 0), caches.debug())
    }

    @Test
    fun bytecodeIndexResolveCommonSuperClassDelegatesValuesAndMissingStaysNull() {
        val childOne = "Lexample/ChildOne;"
        val childTwo = "Lexample/ChildTwo;"
        val commonSuper = "Lexample/Base;"
        val projectSessionVersion = 1L
        val caches = CompletionIndexCaches()
        val delegate = stubBytecodeIndex(
            commonSuperClass = mapOf((childOne to childTwo) to commonSuper),
        )
        val wrapped = caches.bytecodeIndex(delegate, projectSessionVersion)

        caches.resetDebug()
        assertEquals(0, atTargetsCacheSize(caches))

        assertEquals(commonSuper, wrapped.resolveCommonSuperClass(childOne, childTwo))
        assertEquals(commonSuper, delegate.resolveCommonSuperClass(childOne, childTwo))
        assertNull(wrapped.resolveCommonSuperClass("Lmissing/Alpha;", "Lmissing/Beta;"))

        assertEquals(0, atTargetsCacheSize(caches))
        assertEquals(CandidateCacheDebug(hit = false, buildMs = 0), caches.debug())
    }

    @Test
    fun expressionMemberCompletionCacheReusesSameInstanceForSameDelegatesAndVersion() {
        val projectSessionVersion = 1L
        val caches = CompletionIndexCaches()
        val classDelegate = stubClassIndex()
        val bytecodeDelegate = stubBytecodeIndex()

        val first = caches.expressionMemberCompletionCache(
            classIndexDelegate = classDelegate,
            bytecodeIndexDelegate = bytecodeDelegate,
            projectSessionVersion = projectSessionVersion,
        )
        val second = caches.expressionMemberCompletionCache(
            classIndexDelegate = classDelegate,
            bytecodeIndexDelegate = bytecodeDelegate,
            projectSessionVersion = projectSessionVersion,
        )

        assertSame(first, second)
    }

    @Test
    fun expressionMemberCompletionCacheReturnsDifferentInstancesWhenDelegateOrVersionChanges() {
        val caches = CompletionIndexCaches()
        val classDelegateOne = stubClassIndex()
        val classDelegateTwo = stubClassIndex()
        val bytecodeDelegateOne = stubBytecodeIndex()
        val bytecodeDelegateTwo = stubBytecodeIndex()
        val projectSessionVersion = 1L

        val baseline = caches.expressionMemberCompletionCache(
            classIndexDelegate = classDelegateOne,
            bytecodeIndexDelegate = bytecodeDelegateOne,
            projectSessionVersion = projectSessionVersion,
        )
        val differentClassDelegate = caches.expressionMemberCompletionCache(
            classIndexDelegate = classDelegateTwo,
            bytecodeIndexDelegate = bytecodeDelegateOne,
            projectSessionVersion = projectSessionVersion,
        )
        val differentBytecodeDelegate = caches.expressionMemberCompletionCache(
            classIndexDelegate = classDelegateOne,
            bytecodeIndexDelegate = bytecodeDelegateTwo,
            projectSessionVersion = projectSessionVersion,
        )
        val differentVersion = caches.expressionMemberCompletionCache(
            classIndexDelegate = classDelegateOne,
            bytecodeIndexDelegate = bytecodeDelegateOne,
            projectSessionVersion = 2L,
        )

        assertTrue(baseline !== differentClassDelegate)
        assertTrue(baseline !== differentBytecodeDelegate)
        assertTrue(baseline !== differentVersion)
    }

    @Test
    fun expressionMemberCompletionCacheIsBoundedAcrossProjectSessionVersions() {
        val caches = CompletionIndexCaches()
        val classDelegate = stubClassIndex()
        val bytecodeDelegate = stubBytecodeIndex()

        repeat(600) { version ->
            caches.expressionMemberCompletionCache(
                classIndexDelegate = classDelegate,
                bytecodeIndexDelegate = bytecodeDelegate,
                projectSessionVersion = version.toLong(),
            )
        }

        val actualSize = expressionMemberCompletionCacheSize(caches)
        assertTrue(
            actualSize <= 16,
            "expressionMemberCompletionCache size $actualSize exceeds bound of 16",
        )
    }

    @Test
    fun bytecodeIndexAtTargetCacheDoesNotCrossContaminateDifferentDelegates() {
        val owner = "example/Target"
        val methodName = "run"
        val methodDescriptor = "()V"
        val atValue = "INVOKE"
        val projectSessionVersion = 1L
        val caches = CompletionIndexCaches()
        val candidateKey = "$owner#$methodName#$atValue"

        val firstDelegate = stubBytecodeIndex(
            candidates = mapOf(candidateKey to listOf(atTargetCandidate("firstHook"))),
        )
        val secondDelegate = stubBytecodeIndex(
            candidates = mapOf(candidateKey to listOf(atTargetCandidate("secondHook"))),
        )

        val cachedFirst = caches.bytecodeIndex(firstDelegate, projectSessionVersion)
        val cachedSecond = caches.bytecodeIndex(secondDelegate, projectSessionVersion)

        assertEquals(
            listOf("firstHook"),
            cachedFirst.getAtTargetCandidates(owner, methodName, methodDescriptor, atValue).map { it.name },
        )
        assertEquals(
            listOf("secondHook"),
            cachedSecond.getAtTargetCandidates(owner, methodName, methodDescriptor, atValue).map { it.name },
        )
    }

    private fun atTargetCandidate(name: String): AtTargetCandidate =
        AtTargetCandidate(
            owner = "example/Hook",
            name = name,
            descriptor = "()V",
            displayLabel = name,
            detail = "Hook",
            kind = AtTargetKind.INVOKE,
        )

    private fun stubBytecodeIndex(
        candidates: Map<String, List<AtTargetCandidate>> = emptyMap(),
        classBytes: Map<String, ByteArray> = emptyMap(),
        commonSuperClass: Map<Pair<String, String>, String?> = emptyMap(),
    ): BytecodeIndex = object : BytecodeIndex {
        override fun getAtTargetCandidates(
            ownerInternalName: String,
            methodName: String,
            methodDescriptor: String?,
            atValue: String,
        ): List<AtTargetCandidate> = candidates["$ownerInternalName#$methodName#$atValue"].orEmpty()

        override fun getReturnOrdinalCount(
            ownerInternalName: String,
            methodName: String,
            methodDescriptor: String?,
        ): Int = 0

        override fun getClassBytes(ownerInternalName: String): ByteArray? =
            classBytes[ownerInternalName]

        override fun resolveCommonSuperClass(type1Descriptor: String, type2Descriptor: String): String? =
            commonSuperClass[type1Descriptor to type2Descriptor]
    }

    private fun expressionMemberCompletionCacheSize(caches: CompletionIndexCaches): Int {
        val field = CompletionIndexCaches::class.java.getDeclaredField("expressionMemberCompletionCaches")
        field.isAccessible = true
        @Suppress("UNCHECKED_CAST")
        val expressionMemberCompletionCaches = field.get(caches) as Map<*, *>
        return expressionMemberCompletionCaches.size
    }

    private fun memberMethodsCacheSize(caches: CompletionIndexCaches): Int {
        val field = CompletionIndexCaches::class.java.getDeclaredField("methods")
        field.isAccessible = true
        @Suppress("UNCHECKED_CAST")
        val methods = field.get(caches) as Map<*, *>
        return methods.size
    }

    private fun atTargetsCacheSize(caches: CompletionIndexCaches): Int {
        val field = CompletionIndexCaches::class.java.getDeclaredField("atTargets")
        field.isAccessible = true
        @Suppress("UNCHECKED_CAST")
        val atTargets = field.get(caches) as Map<*, *>
        return atTargets.size
    }

    private fun method(name: String): MethodIndexEntry =
        MethodIndexEntry(name, "()V", false, "$name(): void")

    private fun stubClassIndex(
        methods: Map<String, List<MethodIndexEntry>> = emptyMap(),
    ): ClassIndex = object : ClassIndex {
        override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> = emptyList()

        override fun findClass(internalName: String): ClassIndexEntry? = null

        override fun findClassByFqn(fqn: String): ClassIndexEntry? = null

        override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> =
            methods[ownerInternalName].orEmpty()

        override fun getFields(ownerInternalName: String): List<FieldIndexEntry> = emptyList()
    }
}
