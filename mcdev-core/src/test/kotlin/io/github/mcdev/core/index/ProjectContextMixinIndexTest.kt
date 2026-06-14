package io.github.mcdev.core.index

import io.github.mcdev.core.mixin.e2e.MixinE2ETestSupport
import io.github.mcdev.core.project.ProjectContextBuilder
import kotlin.test.Test
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import java.nio.file.Path

class ProjectContextMixinIndexTest {
    private val provider = MixinE2ETestSupport.simpleTargetProvider()
    private val context = ProjectContextBuilder.empty("project-index", Path.of("."))
    private val index = ProjectContextMixinIndex()

    @Test
    fun buildsClassIndexFromProvider() {
        val classIndex = index.buildClassIndex(context, provider)
        assertNotNull(classIndex.findClassByFqn("com.example.target.SimpleTarget"))
    }

    @Test
    fun buildsBytecodeIndexFromProvider() {
        val bytecodeIndex = index.buildBytecodeIndex(context, provider)
        val candidates = bytecodeIndex.getAtTargetCandidates(
            MixinE2ETestSupport.SIMPLE_TARGET_INTERNAL,
            "draw",
            "(Ljava/lang/String;FF)V",
            "RETURN",
        )
        assertTrue(candidates.isNotEmpty())
    }

    @Test
    fun classAndBytecodeIndexShareClasspath() {
        val classIndex = index.buildClassIndex(context, provider)
        val bytecodeIndex = index.buildBytecodeIndex(context, provider)
        assertTrue(classIndex.getMethods(MixinE2ETestSupport.SIMPLE_TARGET_INTERNAL).isNotEmpty())
        assertTrue(
            bytecodeIndex.getReturnOrdinalCount(
                MixinE2ETestSupport.SIMPLE_TARGET_INTERNAL,
                "draw",
                "(Ljava/lang/String;FF)V",
            ) >= 1,
        )
    }
}
