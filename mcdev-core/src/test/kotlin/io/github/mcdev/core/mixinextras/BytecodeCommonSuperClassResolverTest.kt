package io.github.mcdev.core.mixinextras

import java.util.concurrent.atomic.AtomicInteger
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import org.objectweb.asm.ClassWriter
import org.objectweb.asm.Opcodes

class BytecodeCommonSuperClassResolverTest {
    @Test
    fun equalDescriptorsReturnUnchanged() {
        val resolver = resolverWithClasses(
            "hierarchy/Base" to classBytes("hierarchy/Base"),
        )

        assertEquals("Lhierarchy/Base;", resolver.resolve("Lhierarchy/Base;", "Lhierarchy/Base;"))
    }

    @Test
    fun siblingsShareCommonBase() {
        val resolver = resolverWithClasses(
            "hierarchy/Base" to classBytes("hierarchy/Base"),
            "hierarchy/ChildOne" to classBytes("hierarchy/ChildOne", superName = "hierarchy/Base"),
            "hierarchy/ChildTwo" to classBytes("hierarchy/ChildTwo", superName = "hierarchy/Base"),
        )

        assertEquals(
            "Lhierarchy/Base;",
            resolver.resolve("Lhierarchy/ChildOne;", "Lhierarchy/ChildTwo;"),
        )
    }

    @Test
    fun superclassBothArgumentOrders() {
        val resolver = resolverWithClasses(
            "hierarchy/Base" to classBytes("hierarchy/Base"),
            "hierarchy/Child" to classBytes("hierarchy/Child", superName = "hierarchy/Base"),
        )

        assertEquals(
            "Lhierarchy/Base;",
            resolver.resolve("Lhierarchy/Child;", "Lhierarchy/Base;"),
        )
        assertEquals(
            "Lhierarchy/Base;",
            resolver.resolve("Lhierarchy/Base;", "Lhierarchy/Child;"),
        )
    }

    @Test
    fun interfaceImplementation() {
        val resolver = resolverWithClasses(
            "hierarchy/Contract" to interfaceBytes("hierarchy/Contract"),
            "hierarchy/Impl" to classBytes(
                "hierarchy/Impl",
                interfaces = arrayOf("hierarchy/Contract"),
            ),
        )

        assertEquals(
            "Lhierarchy/Contract;",
            resolver.resolve("Lhierarchy/Impl;", "Lhierarchy/Contract;"),
        )
    }

    @Test
    fun unrelatedReferencesReturnObject() {
        val resolver = resolverWithClasses(
            "hierarchy/Alpha" to classBytes("hierarchy/Alpha"),
            "hierarchy/Beta" to classBytes("hierarchy/Beta"),
        )

        assertEquals(
            "Ljava/lang/Object;",
            resolver.resolve("Lhierarchy/Alpha;", "Lhierarchy/Beta;"),
        )
    }

    @Test
    fun arraysPreserveCompatibleReferenceComponentAndDimensions() {
        val resolver = resolverWithClasses(
            "hierarchy/Base" to classBytes("hierarchy/Base"),
            "hierarchy/Child" to classBytes("hierarchy/Child", superName = "hierarchy/Base"),
        )

        assertEquals(
            "[Lhierarchy/Base;",
            resolver.resolve("[Lhierarchy/Child;", "[Lhierarchy/Base;"),
        )
        assertEquals(
            "[Lhierarchy/Base;",
            resolver.resolve("[Lhierarchy/Base;", "[Lhierarchy/Child;"),
        )
        assertEquals(
            "[[Lhierarchy/Base;",
            resolver.resolve("[[Lhierarchy/Child;", "[[Lhierarchy/Base;"),
        )
        assertEquals(
            "[[Lhierarchy/Base;",
            resolver.resolve("[[Lhierarchy/Base;", "[[Lhierarchy/Child;"),
        )
    }

    @Test
    fun stringOneDimensionalWithStringTwoDimensionalReturnsObjectOneDimensional() {
        val resolver = emptyResolver()

        assertEquals(
            "[Ljava/lang/Object;",
            resolver.resolve("[Ljava/lang/String;", "[[Ljava/lang/String;"),
        )
        assertEquals(
            "[Ljava/lang/Object;",
            resolver.resolve("[[Ljava/lang/String;", "[Ljava/lang/String;"),
        )
    }

    @Test
    fun stringTwoDimensionalWithObjectOneDimensionalReturnsObjectOneDimensional() {
        val resolver = emptyResolver()

        assertEquals(
            "[Ljava/lang/Object;",
            resolver.resolve("[[Ljava/lang/String;", "[Ljava/lang/Object;"),
        )
        assertEquals(
            "[Ljava/lang/Object;",
            resolver.resolve("[Ljava/lang/Object;", "[[Ljava/lang/String;"),
        )
    }

    @Test
    fun arraysAreAssignableToCloneableAndSerializable() {
        val resolver = emptyResolver()

        assertEquals(
            "Ljava/lang/Cloneable;",
            resolver.resolve("[I", "Ljava/lang/Cloneable;"),
        )
        assertEquals(
            "Ljava/lang/Cloneable;",
            resolver.resolve("Ljava/lang/Cloneable;", "[I"),
        )
        assertEquals(
            "Ljava/io/Serializable;",
            resolver.resolve("[I", "Ljava/io/Serializable;"),
        )
        assertEquals(
            "Ljava/io/Serializable;",
            resolver.resolve("Ljava/io/Serializable;", "[I"),
        )
    }

    @Test
    fun equalPrimitiveArraysReturnUnchanged() {
        val resolver = emptyResolver()

        assertEquals("[I", resolver.resolve("[I", "[I"))
    }

    @Test
    fun incompatiblePrimitiveArraysReturnObject() {
        val resolver = emptyResolver()

        assertEquals("Ljava/lang/Object;", resolver.resolve("[I", "[J"))
        assertEquals("Ljava/lang/Object;", resolver.resolve("[J", "[I"))
    }

    @Test
    fun incompatiblePrimitiveArraysReturnObjectArrayAtMatchingDimensions() {
        val resolver = emptyResolver()

        assertEquals("[Ljava/lang/Object;", resolver.resolve("[[I", "[[J"))
        assertEquals("[Ljava/lang/Object;", resolver.resolve("[[J", "[[I"))
    }

    @Test
    fun incompatiblePrimitiveArraysReturnObjectArrayAtThreeMatchingDimensions() {
        val resolver = emptyResolver()

        assertEquals("[[Ljava/lang/Object;", resolver.resolve("[[[I", "[[[J"))
        assertEquals("[[Ljava/lang/Object;", resolver.resolve("[[[J", "[[[I"))
    }

    @Test
    fun incompatiblePrimitiveArraysAcrossDimensionsReturnObject() {
        val resolver = emptyResolver()

        assertEquals("Ljava/lang/Object;", resolver.resolve("[I", "[[J"))
        assertEquals("Ljava/lang/Object;", resolver.resolve("[[J", "[I"))
    }

    @Test
    fun mixedPrimitiveAndReferenceArraysReturnObject() {
        val resolver = emptyResolver()

        assertEquals("Ljava/lang/Object;", resolver.resolve("[I", "[Ljava/lang/String;"))
        assertEquals("Ljava/lang/Object;", resolver.resolve("[Ljava/lang/String;", "[I"))
    }

    @Test
    fun arrayDimensionMismatchWithSharedComponentTypeReturnsObjectArray() {
        val resolver = resolverWithClasses(
            "hierarchy/Leaf" to classBytes("hierarchy/Leaf"),
        )

        assertEquals(
            "[Ljava/lang/Object;",
            resolver.resolve("[Lhierarchy/Leaf;", "[[Lhierarchy/Leaf;"),
        )
        assertEquals(
            "[Ljava/lang/Object;",
            resolver.resolve("[[Lhierarchy/Leaf;", "[Lhierarchy/Leaf;"),
        )
    }

    @Test
    fun arrayDimensionMismatchWithRelatedComponentTypesReturnsObjectArray() {
        val resolver = resolverWithClasses(
            "hierarchy/Base" to classBytes("hierarchy/Base"),
            "hierarchy/Child" to classBytes("hierarchy/Child", superName = "hierarchy/Base"),
        )

        assertEquals(
            "[Ljava/lang/Object;",
            resolver.resolve("[Lhierarchy/Child;", "[[Lhierarchy/Base;"),
        )
        assertEquals(
            "[Ljava/lang/Object;",
            resolver.resolve("[[Lhierarchy/Base;", "[Lhierarchy/Child;"),
        )
    }

    @Test
    fun unequalPrimitiveDescriptorsReturnNull() {
        val resolver = emptyResolver()

        assertNull(resolver.resolve("I", "J"))
        assertNull(resolver.resolve("Z", "B"))
    }

    @Test
    fun invalidDescriptorsReturnNull() {
        val resolver = emptyResolver()

        assertNull(resolver.resolve("not-a-descriptor", "[I"))
        assertNull(resolver.resolve("[I", "Ljava/lang/Object"))
        assertNull(resolver.resolve("()V", "I"))
        assertNull(resolver.resolve("Ljava/lang/Object;junk", "[I"))
        assertNull(resolver.resolve("[Ijunk", "I"))
        assertNull(resolver.resolve("[[", "[I"))
        assertNull(resolver.resolve("[Ljava/lang/Object", "[I"))
    }

    @Test
    fun equalInvalidDescriptorsReturnNull() {
        val resolver = emptyResolver()

        assertNull(resolver.resolve("not-a-descriptor", "not-a-descriptor"))
        assertNull(resolver.resolve("()V", "()V"))
    }

    @Test
    fun missingBytesReturnNull() {
        val resolver = emptyResolver()

        assertNull(resolver.resolve("Lcom/missing/Alpha;", "Lcom/missing/Beta;"))
    }

    @Test
    fun corruptBytesReturnNull() {
        val resolver = resolverWithClasses(
            "hierarchy/Corrupt" to CORRUPT_CLASS_BYTES,
            "hierarchy/Peer" to classBytes("hierarchy/Peer"),
        )

        assertNull(resolver.resolve("Lhierarchy/Corrupt;", "Lhierarchy/Peer;"))
    }

    @Test
    fun cacheReusesHierarchyAndPairResults() {
        val counter = AtomicInteger(0)
        val resolver = resolverWithClasses(
            "hierarchy/Base" to classBytes("hierarchy/Base"),
            "hierarchy/ChildOne" to classBytes("hierarchy/ChildOne", superName = "hierarchy/Base"),
            "hierarchy/ChildTwo" to classBytes("hierarchy/ChildTwo", superName = "hierarchy/Base"),
            counter = counter,
        )

        val first = resolver.resolve("Lhierarchy/ChildOne;", "Lhierarchy/ChildTwo;")
        val readsAfterFirst = counter.get()
        val second = resolver.resolve("Lhierarchy/ChildTwo;", "Lhierarchy/ChildOne;")

        assertEquals("Lhierarchy/Base;", first)
        assertEquals(first, second)
        assertEquals(readsAfterFirst, counter.get())
    }

    @Test
    fun everyReferenceIsAssignableToObjectWithoutHierarchyBytes() {
        val resolver = emptyResolver()

        assertEquals(
            "Ljava/lang/Object;",
            resolver.resolve("Lcom/missing/Type;", "Ljava/lang/Object;"),
        )
    }

    @Test
    fun arraysAreAssignableToObject() {
        val resolver = emptyResolver()

        assertEquals(
            "Ljava/lang/Object;",
            resolver.resolve("[I", "Ljava/lang/Object;"),
        )
        assertEquals(
            "Ljava/lang/Object;",
            resolver.resolve("Ljava/lang/Object;", "[I"),
        )
    }

    @Test
    fun cyclicHierarchyDoesNotLoop() {
        val counter = AtomicInteger(0)
        val resolver = resolverWithClasses(
            "cycle/A" to classBytes("cycle/A", superName = "cycle/B"),
            "cycle/B" to classBytes("cycle/B", superName = "cycle/A"),
            "cycle/C" to classBytes("cycle/C"),
            counter = counter,
        )

        resolver.resolve("Lcycle/A;", "Lcycle/B;")
        resolver.resolve("Lcycle/B;", "Lcycle/A;")

        assertNull(resolver.resolve("Lcycle/A;", "Lcycle/C;"))
        val readsAfterFirst = counter.get()
        assertNull(resolver.resolve("Lcycle/C;", "Lcycle/A;"))
        assertEquals(readsAfterFirst, counter.get())
    }

    @Test
    fun missingBytesAreCachedWithoutRepeatedLookup() {
        val counter = AtomicInteger(0)
        val resolver = resolverWithClasses(counter = counter)

        assertNull(resolver.resolve("Lcom/missing/Alpha;", "Lcom/missing/Beta;"))
        val readsAfterFirst = counter.get()
        assertNull(resolver.resolve("Lcom/missing/Beta;", "Lcom/missing/Alpha;"))

        assertEquals(readsAfterFirst, counter.get())
    }

    @Test
    fun nonPositiveCacheSizesAreRejected() {
        assertFailsWith<IllegalArgumentException> {
            BytecodeCommonSuperClassResolver(classBytesLookup = { null }, maxCachedHierarchies = 0)
        }
        assertFailsWith<IllegalArgumentException> {
            BytecodeCommonSuperClassResolver(classBytesLookup = { null }, maxCachedPairs = 0)
        }
    }

    private fun emptyResolver(): BytecodeCommonSuperClassResolver =
        resolverWithClasses()

    private fun resolverWithClasses(
        vararg classes: Pair<String, ByteArray>,
        counter: AtomicInteger = AtomicInteger(0),
    ): BytecodeCommonSuperClassResolver {
        val bytesByName = classes.toMap()
        return BytecodeCommonSuperClassResolver(
            classBytesLookup = { internalName ->
                counter.incrementAndGet()
                bytesByName[internalName]
            },
        )
    }

    private fun classBytes(
        internalName: String,
        superName: String = "java/lang/Object",
        interfaces: Array<String> = emptyArray(),
    ): ByteArray {
        val writer = ClassWriter(0)
        writer.visit(
            Opcodes.V21,
            Opcodes.ACC_PUBLIC,
            internalName,
            null,
            superName,
            interfaces,
        )
        writer.visitEnd()
        return writer.toByteArray()
    }

    private fun interfaceBytes(
        internalName: String,
        superName: String = "java/lang/Object",
        interfaces: Array<String> = emptyArray(),
    ): ByteArray {
        val writer = ClassWriter(0)
        writer.visit(
            Opcodes.V21,
            Opcodes.ACC_PUBLIC or Opcodes.ACC_INTERFACE or Opcodes.ACC_ABSTRACT,
            internalName,
            null,
            superName,
            interfaces,
        )
        writer.visitEnd()
        return writer.toByteArray()
    }

    private companion object {
        val CORRUPT_CLASS_BYTES = byteArrayOf(
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
