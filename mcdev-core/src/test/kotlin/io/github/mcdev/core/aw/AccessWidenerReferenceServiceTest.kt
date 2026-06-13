package io.github.mcdev.core.aw

import io.github.mcdev.core.awat.AwAtE2ETestSupport
import io.github.mcdev.core.definition.McDefinitionTarget
import io.github.mcdev.core.definition.SourceScanEntry
import io.github.mcdev.core.model.MemberKind
import io.github.mcdev.fixtures.FixturePaths
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class AccessWidenerReferenceServiceTest {
    private val service = AccessWidenerReferenceService(
        classIndex = AwAtE2ETestSupport.simpleTargetClassIndex(),
        mappingContext = AwAtE2ETestSupport.fabricAwAtMappingContext(),
    )

    @Test
    fun findsClassReferencesInAccessWidenerFile() {
        val source = AwAtE2ETestSupport.loadFixtureText(FixturePaths.FABRIC_AW_AT_ACCESS_WIDENER)
        val target = McDefinitionTarget(
            kind = MemberKind.CLASS,
            ownerInternalName = AwAtE2ETestSupport.SIMPLE_TARGET_INTERNAL,
            ownerFqn = "com.example.target.SimpleTarget",
        )
        val references = service.findReferences(
            target,
            listOf(SourceScanEntry("file:///mod.accesswidener", source)),
        )
        assertTrue(references.any { it.metadata["source"] == "aw.class" })
        assertTrue(references.any { it.metadata["source"] == "aw.owner" })
    }

    @Test
    fun findsMethodReferencesInAccessWidenerFile() {
        val source = AwAtE2ETestSupport.loadFixtureText(FixturePaths.FABRIC_AW_AT_ACCESS_WIDENER)
        val target = McDefinitionTarget(
            kind = MemberKind.METHOD,
            ownerInternalName = AwAtE2ETestSupport.SIMPLE_TARGET_INTERNAL,
            ownerFqn = "com.example.target.SimpleTarget",
            name = "draw",
            descriptor = "(Ljava/lang/String;FF)V",
        )
        val references = service.findReferences(
            target,
            listOf(SourceScanEntry("file:///mod.accesswidener", source)),
        )
        assertEquals(1, references.size)
        assertEquals("aw.method", references.first().metadata["source"])
    }

    @Test
    fun findsFieldReferencesInAccessWidenerFile() {
        val source = AwAtE2ETestSupport.loadFixtureText(FixturePaths.FABRIC_AW_AT_ACCESS_WIDENER)
        val target = McDefinitionTarget(
            kind = MemberKind.FIELD,
            ownerInternalName = AwAtE2ETestSupport.SIMPLE_TARGET_INTERNAL,
            ownerFqn = "com.example.target.SimpleTarget",
            name = "counter",
            descriptor = "I",
        )
        val references = service.findReferences(
            target,
            listOf(SourceScanEntry("file:///mod.accesswidener", source)),
        )
        assertEquals(1, references.size)
        assertEquals("aw.field", references.first().metadata["source"])
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
            listOf(SourceScanEntry("file:///mod.accesswidener", "accessWidener v2 named\n")),
        )
        assertTrue(references.isEmpty())
    }
}
