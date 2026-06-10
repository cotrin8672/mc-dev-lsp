package io.github.mcdev.core.bytecode

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

class ClassMemberIndexTest {
    private val provider = BytecodeFixtureCompiler.provider()
    private val memberIndex = BytecodeFixtureCompiler.internalName("MemberIndexSamples")

    @Test
    fun indexesClassMetadata() {
        val index = ClassMemberIndexBuilder.build(provider)
        val entry = assertNotNull(index.findClass(memberIndex))
        assertEquals("MemberIndexSamples", entry.simpleName)
        assertEquals("io.github.mcdev.core.bytecode.fixtures.MemberIndexSamples", entry.qualifiedName)
        assertEquals("io/github/mcdev/core/bytecode/fixtures/BaseClass", entry.superclassInternalName)
        assertEquals(listOf("io/github/mcdev/core/bytecode/fixtures/SampleInterface"), entry.interfaceInternalNames)
    }

    @Test
    fun indexesInstanceMethods() {
        val index = ClassMemberIndexBuilder.build(provider)
        val methods = index.methodsByOwner[memberIndex].orEmpty()
        assertTrue(methods.any { it.name == "instanceMethod" && !it.isStatic })
    }

    @Test
    fun indexesStaticMethods() {
        val index = ClassMemberIndexBuilder.build(provider)
        val method = assertNotNull(index.findMethod(memberIndex, "staticMethod", "()V"))
        assertTrue(method.isStatic)
    }

    @Test
    fun indexesConstructors() {
        val index = ClassMemberIndexBuilder.build(provider)
        val ctor = assertNotNull(index.findMethod(memberIndex, "<init>", "()V"))
        assertTrue(ctor.isConstructor)
    }

    @Test
    fun indexesStaticFields() {
        val index = ClassMemberIndexBuilder.build(provider)
        val field = assertNotNull(index.findField(memberIndex, "STATIC_FIELD", "Ljava/lang/String;"))
        assertTrue(field.isStatic)
    }

    @Test
    fun indexesInstanceFields() {
        val index = ClassMemberIndexBuilder.build(provider)
        val field = assertNotNull(index.findField(memberIndex, "instanceField", "I"))
        assertTrue(!field.isStatic)
    }

    @Test
    fun indexesAllFixtureClasses() {
        val index = ClassMemberIndexBuilder.build(provider)
        assertTrue(index.classes.containsKey(BytecodeFixtureCompiler.internalName("InvokeSamples")))
        assertTrue(index.classes.containsKey(BytecodeFixtureCompiler.internalName("FieldSamples")))
        assertTrue(index.classes.containsKey(BytecodeFixtureCompiler.internalName("NewSamples")))
        assertTrue(index.classes.containsKey(BytecodeFixtureCompiler.internalName("ConstantSamples")))
        assertTrue(index.classes.containsKey(BytecodeFixtureCompiler.internalName("ReturnSamples")))
    }

    @Test
    fun indexSingleClassFromBytes() {
        val bytes = BytecodeFixtureCompiler.classBytes("ReturnSamples")
        val index = ClassMemberIndexBuilder.indexClass(bytes, BytecodeFixtureCompiler.internalName("ReturnSamples"))
        assertEquals(1, index.classes.size)
        assertTrue(index.findMethod(BytecodeFixtureCompiler.internalName("ReturnSamples"), "returnInt", "()I") != null)
    }

    @Test
    fun findMethodReturnsNullForMissingMethod() {
        val index = ClassMemberIndexBuilder.build(provider)
        assertEquals(null, index.findMethod(memberIndex, "missing", "()V"))
    }

    @Test
    fun findFieldReturnsNullForMissingField() {
        val index = ClassMemberIndexBuilder.build(provider)
        assertEquals(null, index.findField(memberIndex, "missing", "I"))
    }
}
