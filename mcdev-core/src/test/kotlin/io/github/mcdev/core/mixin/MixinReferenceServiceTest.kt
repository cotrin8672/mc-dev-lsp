package io.github.mcdev.core.mixin

import io.github.mcdev.core.definition.McDefinitionTarget
import io.github.mcdev.core.definition.SourceScanEntry
import io.github.mcdev.core.model.MemberKind
import io.github.mcdev.core.model.MappingNamespace
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class MixinReferenceServiceTest {
    private val service = MixinReferenceService()

    @Test
    fun findsMixinClassReferences() {
        val source = """
            @Mixin(SimpleTarget.class)
            abstract class ExampleMixin {}
        """.trimIndent()
        val target = McDefinitionTarget(
            kind = MemberKind.CLASS,
            ownerInternalName = "com/example/target/SimpleTarget",
            ownerFqn = "com.example.target.SimpleTarget",
        )
        val references = service.findReferences(
            target,
            listOf(SourceScanEntry("file:///Mixin.java", source)),
        )
        assertEquals(1, references.size)
        assertEquals("mixin.class", references.first().metadata["source"])
    }

    @Test
    fun findsShadowFieldReferences() {
        val source = """
            @Mixin(SimpleTarget.class)
            abstract class ExampleMixin {
                @Shadow private int counter;
            }
        """.trimIndent()
        val target = McDefinitionTarget(
            kind = MemberKind.FIELD,
            ownerInternalName = "com/example/target/SimpleTarget",
            ownerFqn = "com.example.target.SimpleTarget",
            name = "counter",
            descriptor = "I",
        )
        val references = service.findReferences(
            target,
            listOf(SourceScanEntry("file:///Mixin.java", source)),
        )
        assertEquals(1, references.size)
        assertEquals("mixin.shadow", references.first().metadata["source"])
    }

    @Test
    fun findsInvokerReferences() {
        val source = """
            @Mixin(SimpleTarget.class)
            abstract class ExampleMixin {
                @Invoker("draw")
                abstract void invokeDraw(String text, float x, float y);
            }
        """.trimIndent()
        val target = McDefinitionTarget(
            kind = MemberKind.METHOD,
            ownerInternalName = "com/example/target/SimpleTarget",
            ownerFqn = "com.example.target.SimpleTarget",
            name = "draw",
            descriptor = "(Ljava/lang/String;FF)V",
        )
        val references = service.findReferences(
            target,
            listOf(SourceScanEntry("file:///Mixin.java", source)),
        )
        assertTrue(references.any { it.metadata["source"] == "mixin.invoker" })
    }

    @Test
    fun findsReferencesAcrossMultipleSources() {
        val first = SourceScanEntry("file:///A.java", """@Mixin(SimpleTarget.class) class A {}""")
        val second = SourceScanEntry("file:///B.java", """@Mixin(targets = "com.example.target.SimpleTarget") class B {}""")
        val target = McDefinitionTarget(
            kind = MemberKind.CLASS,
            ownerInternalName = "com/example/target/SimpleTarget",
            ownerFqn = "com.example.target.SimpleTarget",
            namespace = MappingNamespace.NAMED,
        )
        val references = service.findReferences(target, listOf(first, second))
        assertEquals(2, references.size)
    }

    @Test
    fun findsAccessorFieldReferences() {
        val source = """
            @Mixin(SimpleTarget.class)
            abstract class ExampleMixin {
                @Accessor("counter")
                abstract int getCounter();
            }
        """.trimIndent()
        val target = McDefinitionTarget(
            kind = MemberKind.FIELD,
            ownerInternalName = "com/example/target/SimpleTarget",
            ownerFqn = "com.example.target.SimpleTarget",
            name = "counter",
            descriptor = "I",
        )
        val references = service.findReferences(
            target,
            listOf(SourceScanEntry("file:///Mixin.java", source)),
        )
        assertEquals(1, references.size)
        assertEquals("mixin.accessor", references.first().metadata["source"])
    }

    @Test
    fun returnsEmptyWhenNoMatches() {
        val target = McDefinitionTarget(
            kind = MemberKind.CLASS,
            ownerInternalName = "com/example/missing/Missing",
            ownerFqn = "com.example.missing.Missing",
        )
        val references = service.findReferences(
            target,
            listOf(SourceScanEntry("file:///Mixin.java", "class Plain {}")),
        )
        assertTrue(references.isEmpty())
    }
}
