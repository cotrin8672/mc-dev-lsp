package io.github.mcdev.core.aw

import io.github.mcdev.core.awat.AwAtE2ETestSupport
import io.github.mcdev.core.awat.AwAtFileType
import io.github.mcdev.core.model.MemberKind
import io.github.mcdev.fixtures.FixturePaths
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class AccessWidenerDefinitionServiceTest {
    private val service = AccessWidenerDefinitionService(
        classIndex = AwAtE2ETestSupport.simpleTargetClassIndex(),
        mappingContext = AwAtE2ETestSupport.fabricAwAtMappingContext(),
    )

    @Test
    fun resolvesOwnerToClassDefinition() {
        val source = AwAtE2ETestSupport.loadFixtureText(FixturePaths.FABRIC_AW_AT_ACCESS_WIDENER)
        val marker = "accessible class com/example/target/Simple"
        val offset = source.indexOf(marker) + "accessible class ".length + "com/example/target/Simple".length
        val request = AwAtE2ETestSupport.requestAtOffset(source, offset, AwAtFileType.ACCESS_WIDENER)
        val targets = service.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals(MemberKind.CLASS, targets.first().kind)
        assertEquals(AwAtE2ETestSupport.SIMPLE_TARGET_INTERNAL, targets.first().ownerInternalName)
    }

    @Test
    fun resolvesMethodNameToMethodDefinition() {
        val source = AwAtE2ETestSupport.loadFixtureText(FixturePaths.FABRIC_AW_AT_ACCESS_WIDENER)
        val marker = "accessible method com/example/target/SimpleTarget draw"
        val offset = source.indexOf(marker) + marker.indexOf("draw") + 2
        val request = AwAtE2ETestSupport.requestAtOffset(source, offset, AwAtFileType.ACCESS_WIDENER)
        val targets = service.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals(MemberKind.METHOD, targets.first().kind)
        assertEquals("draw", targets.first().name)
        assertTrue(targets.first().descriptor!!.contains("Ljava/lang/String;"))
    }

    @Test
    fun methodDefinitionDoesNotFallbackWhenDescriptorMismatches() {
        val source = """
            accessWidener v2 named
            accessible method com/example/target/SimpleTarget draw ()V
        """.trimIndent()
        val marker = "draw"
        val request = AwAtE2ETestSupport.requestAtOffset(
            source,
            source.indexOf(marker) + marker.length,
            AwAtFileType.ACCESS_WIDENER,
        )
        val targets = service.definitionsAt(request.bufferText, request.line, request.character)
        assertTrue(targets.isEmpty())
    }

    @Test
    fun resolvesFieldNameToFieldDefinition() {
        val source = AwAtE2ETestSupport.loadFixtureText(FixturePaths.FABRIC_AW_AT_ACCESS_WIDENER)
        val marker = "mutable field com/example/target/SimpleTarget counter"
        val offset = source.indexOf(marker) + marker.indexOf("counter") + 3
        val request = AwAtE2ETestSupport.requestAtOffset(source, offset, AwAtFileType.ACCESS_WIDENER)
        val targets = service.definitionsAt(request.bufferText, request.line, request.character)
        assertEquals(1, targets.size)
        assertEquals(MemberKind.FIELD, targets.first().kind)
        assertEquals("counter", targets.first().name)
        assertEquals("I", targets.first().descriptor)
    }

    @Test
    fun returnsEmptyOnDirectiveSlot() {
        val source = """
            accessWidener v2 named
            access
        """.trimIndent()
        val request = AwAtE2ETestSupport.requestAtOffset(source, source.length, AwAtFileType.ACCESS_WIDENER)
        val targets = service.definitionsAt(request.bufferText, request.line, request.character)
        assertTrue(targets.isEmpty())
    }
}
