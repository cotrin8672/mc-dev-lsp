package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.mixin.AnnotationContextExtractor
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FakeBytecodeIndex
import io.github.mcdev.core.mixin.FakeClassIndex
import io.github.mcdev.core.mixin.MethodIndexEntry
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import org.objectweb.asm.Opcodes
import org.objectweb.asm.Type

class ExpressionTypeImportCacheTest {
    @Test
    fun changingImportRecomputesBindingsButUnchangedImportReusesCandidates() {
        val index = FakeClassIndex(
            classes = listOf(ClassIndexEntry("Target", "sample", "sample/Target"),
                ClassIndexEntry("Foo", "one", "one/Foo"), ClassIndexEntry("Foo", "two", "two/Foo")),
            methods = mapOf("sample/Target" to listOf(MethodIndexEntry("run", "()I", false, "run"))),
        )
        val bytecode = object : BytecodeIndex by FakeBytecodeIndex() {
            override fun getClassBytes(ownerInternalName: String): ByteArray = byteArrayOf(1)
        }
        var calls = 0
        val service = ExpressionMemberCompletionService(index, bytecode,
            matcherRunner = ExpressionMemberMatcherRunner { _, _, _, _, _, pool, _, _ ->
                calls++
                val name = if (pool.delegate.matchesType("T", Type.getObjectType("one/Foo"))) "one" else "two"
                OfficialExpressionMemberCompletionResult.Available(listOf(
                    OfficialExpressionMemberCandidate.FieldAccess("sample/Target", name, "I", Opcodes.GETFIELD, 0),
                ))
            })
        for (pkg in listOf("one", "two", "two")) {
            val source = """
                import $pkg.Foo;
                @Mixin(sample.Target.class)
                abstract class Example {
                    @Definition(id = "T", type = Foo.class)
                    @Expression("this.")
                    @ModifyExpressionValue(method = "run()I", at = @At("MIXINEXTRAS:EXPRESSION"))
                    private int handler(int original) { return original; }
                }
            """.trimIndent()
            val context = AnnotationContextExtractor.extractAtOffset(source, source.indexOf("this.") + 5)!!
            val result = assertIs<ExpressionMemberCompletionServiceResult.Available>(
                service.completeMembers(source, context, listOf("sample/Target")))
            assertEquals(pkg, result.candidates.single().name)
        }
        assertEquals(2, calls)
    }
}
