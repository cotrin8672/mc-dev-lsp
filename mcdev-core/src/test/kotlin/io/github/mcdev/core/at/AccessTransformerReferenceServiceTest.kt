package io.github.mcdev.core.at

import io.github.mcdev.core.awat.AwAtE2ETestSupport
import io.github.mcdev.core.definition.McDefinitionTarget
import io.github.mcdev.core.definition.SourceScanEntry
import io.github.mcdev.core.model.MemberKind
import io.github.mcdev.fixtures.FixturePaths
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class AccessTransformerReferenceServiceTest {
    private val service = AccessTransformerReferenceService(
        classIndex = AwAtE2ETestSupport.simpleTargetClassIndex(),
    )

    @Test
    fun findsClassOnlyReferences() {
        val source = AwAtE2ETestSupport.loadFixtureText(FixturePaths.FABRIC_AW_AT_ACCESS_TRANSFORMER)
        val target = McDefinitionTarget(
            kind = MemberKind.CLASS,
            ownerInternalName = AwAtE2ETestSupport.SIMPLE_TARGET_INTERNAL,
            ownerFqn = "com.example.target.SimpleTarget",
        )
        val references = service.findReferences(
            target,
            listOf(SourceScanEntry("file:///mod_at.cfg", source)),
        )
        assertEquals(1, references.size)
        assertEquals("at.class", references.first().metadata["source"])
    }

    @Test
    fun findsFieldReferences() {
        val source = AwAtE2ETestSupport.loadFixtureText(FixturePaths.FABRIC_AW_AT_ACCESS_TRANSFORMER)
        val target = McDefinitionTarget(
            kind = MemberKind.FIELD,
            ownerInternalName = AwAtE2ETestSupport.SIMPLE_TARGET_INTERNAL,
            ownerFqn = "com.example.target.SimpleTarget",
            name = "counter",
            descriptor = "I",
        )
        val references = service.findReferences(
            target,
            listOf(SourceScanEntry("file:///mod_at.cfg", source)),
        )
        assertEquals(1, references.size)
        assertEquals("at.field", references.first().metadata["source"])
    }

    @Test
    fun findsMethodReferences() {
        val source = AwAtE2ETestSupport.loadFixtureText(FixturePaths.FABRIC_AW_AT_ACCESS_TRANSFORMER)
        val target = McDefinitionTarget(
            kind = MemberKind.METHOD,
            ownerInternalName = AwAtE2ETestSupport.SIMPLE_TARGET_INTERNAL,
            ownerFqn = "com.example.target.SimpleTarget",
            name = "draw",
            descriptor = "(Ljava/lang/String;FF)V",
        )
        val references = service.findReferences(
            target,
            listOf(SourceScanEntry("file:///mod_at.cfg", source)),
        )
        assertEquals(1, references.size)
        assertEquals("at.method", references.first().metadata["source"])
    }

    @Test
    fun returnsEmptyWhenNoMatches() {
        val target = McDefinitionTarget(
            kind = MemberKind.METHOD,
            ownerInternalName = AwAtE2ETestSupport.SIMPLE_TARGET_INTERNAL,
            ownerFqn = "com.example.target.SimpleTarget",
            name = "missing",
            descriptor = "()V",
        )
        val references = service.findReferences(
            target,
            listOf(SourceScanEntry("file:///mod_at.cfg", "public com.example.target.SimpleTarget")),
        )
        assertTrue(references.isEmpty())
    }
}
