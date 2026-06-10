package io.github.mcdev.core.index

import io.github.mcdev.core.bytecode.BytecodeIndexService
import io.github.mcdev.core.mixin.e2e.MixinE2ETestSupport
import io.github.mcdev.core.project.ProjectContextBuilder
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import java.nio.file.Path

class BytecodeClassIndexAdapterTest {
    private val provider = MixinE2ETestSupport.simpleTargetProvider()
    private val memberIndex = BytecodeIndexService().buildIndex(provider)
    private val adapter = BytecodeClassIndexAdapter(memberIndex)

    @Test
    fun findClassByInternalName() {
        val entry = adapter.findClass(MixinE2ETestSupport.SIMPLE_TARGET_INTERNAL)
        assertNotNull(entry)
        assertEquals("SimpleTarget", entry.simpleName)
        assertEquals("com.example.target", entry.packageName)
    }

    @Test
    fun findClassByFqn() {
        val entry = adapter.findClassByFqn("com.example.target.SimpleTarget")
        assertNotNull(entry)
        assertEquals(MixinE2ETestSupport.SIMPLE_TARGET_INTERNAL, entry.internalName)
    }

    @Test
    fun findClassesByPrefix() {
        val entries = adapter.findClasses("Simple")
        assertEquals(1, entries.size)
        assertEquals("SimpleTarget", entries.first().simpleName)
    }

    @Test
    fun exposesDrawMethodWithReadableSignature() {
        val methods = adapter.getMethods(MixinE2ETestSupport.SIMPLE_TARGET_INTERNAL)
        val draw = methods.find { it.name == "draw" }
        assertNotNull(draw)
        assertEquals("(Ljava/lang/String;FF)V", draw.descriptor)
        assertTrue(draw.readableSignature.contains("draw"))
        assertTrue(draw.readableSignature.contains("String"))
    }

    @Test
    fun exposesCounterFieldWithReadableType() {
        val fields = adapter.getFields(MixinE2ETestSupport.SIMPLE_TARGET_INTERNAL)
        val counter = fields.find { it.name == "counter" }
        assertNotNull(counter)
        assertEquals("I", counter.descriptor)
        assertEquals("int", counter.readableType)
    }

    @Test
    fun projectContextMixinIndexBuildsEquivalentAdapter() {
        val index = ProjectContextMixinIndex()
        val context = ProjectContextBuilder.empty("adapter-test", Path.of("."))
        val built = index.buildClassIndex(context, provider)
        assertNotNull(built.findClass(MixinE2ETestSupport.SIMPLE_TARGET_INTERNAL))
        assertTrue(built.getMethods(MixinE2ETestSupport.SIMPLE_TARGET_INTERNAL).any { it.name == "draw" })
    }
}
