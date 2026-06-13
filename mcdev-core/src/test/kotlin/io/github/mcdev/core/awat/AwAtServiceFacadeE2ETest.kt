package io.github.mcdev.core.awat

import io.github.mcdev.core.aw.AwDiagnosticCodes
import io.github.mcdev.core.at.AtDiagnosticCodes
import io.github.mcdev.core.codeaction.AddAtMethodDescriptorFix
import io.github.mcdev.core.codeaction.RemoveDuplicateAccessWidenerEntryFix
import io.github.mcdev.fixtures.FixturePaths
import kotlin.test.Test
import kotlin.test.assertIs
import kotlin.test.assertTrue

class AwAtServiceFacadeE2ETest {
    private val facade = AwAtE2ETestSupport.indexedFacade()

    @Test
    fun completesAccessWidenerDirectives() {
        val source = """
            accessWidener v2 named
            acc
        """.trimIndent()
        val items = facade.complete(
            AwAtE2ETestSupport.requestAtOffset(source, source.length, AwAtFileType.ACCESS_WIDENER),
        )
        assertTrue(items.any { it.insertText == "accessible" && it.metadata.source == "aw.directive" })
    }

    @Test
    fun completesAccessWidenerKinds() {
        val source = """
            accessWidener v2 named
            accessible cl
        """.trimIndent()
        val items = facade.complete(
            AwAtE2ETestSupport.requestAtOffset(source, source.length, AwAtFileType.ACCESS_WIDENER),
        )
        assertTrue(items.any { it.insertText == "class" && it.metadata.source == "aw.kind" })
    }

    @Test
    fun completesAccessWidenerClassesFromFixture() {
        val source = AwAtE2ETestSupport.loadFixtureText(FixturePaths.FABRIC_AW_AT_ACCESS_WIDENER)
        val marker = "accessible class com/example/target/Simple"
        val offset = source.indexOf(marker) + marker.length
        val items = facade.complete(
            AwAtE2ETestSupport.requestAtOffset(source, offset, AwAtFileType.ACCESS_WIDENER),
        )
        assertTrue(items.isNotEmpty())
        assertTrue(items.any { it.label == "SimpleTarget" })
    }

    @Test
    fun completesAccessWidenerMethodsFromFixture() {
        val source = """
            accessWidener v2 named
            accessible method com/example/target/SimpleTarget dr
        """.trimIndent()
        val items = facade.complete(
            AwAtE2ETestSupport.requestAtOffset(source, source.length, AwAtFileType.ACCESS_WIDENER),
        )
        assertTrue(items.any { it.insertText.startsWith("draw") && it.metadata.source == "aw.method" })
    }

    @Test
    fun completesAccessWidenerFieldsFromFixture() {
        val source = """
            accessWidener v2 named
            mutable field com/example/target/SimpleTarget cou
        """.trimIndent()
        val items = facade.complete(
            AwAtE2ETestSupport.requestAtOffset(source, source.length, AwAtFileType.ACCESS_WIDENER),
        )
        assertTrue(items.any { it.insertText.startsWith("counter") && it.metadata.source == "aw.field" })
    }

    @Test
    fun completesAccessTransformerModifiers() {
        val source = "pub"
        val items = facade.complete(
            AwAtE2ETestSupport.requestAtOffset(source, source.length, AwAtFileType.ACCESS_TRANSFORMER),
        )
        assertTrue(items.any { it.insertText == "public" && it.metadata.source == "at.modifier" })
    }

    @Test
    fun completesAccessTransformerClassesFromFixture() {
        val source = AwAtE2ETestSupport.loadFixtureText(FixturePaths.FABRIC_AW_AT_ACCESS_TRANSFORMER)
        val marker = "public com.example.target.Simp"
        val offset = source.indexOf(marker) + marker.length
        val items = facade.complete(
            AwAtE2ETestSupport.requestAtOffset(source, offset, AwAtFileType.ACCESS_TRANSFORMER),
        )
        assertTrue(items.isNotEmpty())
        assertTrue(items.any { it.insertText == "com.example.target.SimpleTarget" })
    }

    @Test
    fun completesAccessTransformerMembersWithIntermediaryNames() {
        val source = "public com.example.target.SimpleTarget dr"
        val items = facade.complete(
            AwAtE2ETestSupport.requestAtOffset(source, source.length, AwAtFileType.ACCESS_TRANSFORMER),
        )
        assertTrue(items.any { it.insertText == "method_1(Ljava/lang/String;FF)V" && it.metadata.source == "at.member.method" })
    }

    @Test
    fun completesAccessWidenerMethodDescriptorSlotFromFixture() {
        val source = """
            accessWidener v2 named
            accessible method com/example/target/SimpleTarget draw (Ljava/lang/Str
        """.trimIndent()
        val items = facade.complete(
            AwAtE2ETestSupport.requestAtOffset(source, source.length, AwAtFileType.ACCESS_WIDENER),
        )
        assertTrue(items.any { it.insertText == "(Ljava/lang/String;FF)V" && it.metadata.source == "aw.descriptor" })
    }

    @Test
    fun completesAccessTransformerFieldWithIntermediaryInsertText() {
        val source = "public com.example.target.SimpleTarget cou"
        val items = facade.complete(
            AwAtE2ETestSupport.requestAtOffset(source, source.length, AwAtFileType.ACCESS_TRANSFORMER),
        )
        assertTrue(items.any { it.insertText == "field_1" && it.metadata.source == "at.member.field" })
    }

    @Test
    fun codeActionsOffersRemoveDuplicateFixForAccessWidener() {
        val source = """
            accessWidener v2 named
            accessible class com/example/target/SimpleTarget
            accessible class com/example/target/SimpleTarget
        """.trimIndent()
        val fixes = facade.codeActions(
            AwAtE2ETestSupport.requestAtOffset(source, 0, AwAtFileType.ACCESS_WIDENER),
            AwDiagnosticCodes.DUPLICATE_ENTRY,
        )
        assertIs<RemoveDuplicateAccessWidenerEntryFix>(fixes.first())
    }

    @Test
    fun codeActionsOffersAddDescriptorFixForMissingMethodDescriptor() {
        val source = "public com.example.target.SimpleTarget draw"
        val localFacade = AwAtServiceFacade(
            classIndex = AwAtE2ETestSupport.simpleTargetClassIndex(),
            mappingContext = null,
        )
        val fixes = localFacade.codeActions(
            AwAtE2ETestSupport.requestAtOffset(source, 0, AwAtFileType.ACCESS_TRANSFORMER),
            AtDiagnosticCodes.MISSING_METHOD_DESCRIPTOR,
        )
        assertIs<AddAtMethodDescriptorFix>(fixes.first())
    }

    @Test
    fun diagnosesUnresolvedAccessWidenerClass() {
        val source = """
            accessWidener v2 named
            accessible class com/example/missing/Missing
        """.trimIndent()
        val diagnostics = facade.diagnose(
            AwAtE2ETestSupport.requestAtOffset(source, 0, AwAtFileType.ACCESS_WIDENER),
        )
        assertTrue(diagnostics.any { it.code == AwDiagnosticCodes.UNRESOLVED_CLASS })
    }

    @Test
    fun diagnosesValidFixtureAccessWidenerWithoutUnresolvedTargets() {
        val source = AwAtE2ETestSupport.loadFixtureText(FixturePaths.FABRIC_AW_AT_ACCESS_WIDENER)
        val diagnostics = facade.diagnose(
            AwAtE2ETestSupport.requestAtOffset(source, 0, AwAtFileType.ACCESS_WIDENER),
        )
        assertTrue(diagnostics.none { it.code == AwDiagnosticCodes.UNRESOLVED_CLASS })
        assertTrue(diagnostics.none { it.code == AwDiagnosticCodes.UNRESOLVED_MEMBER })
    }

    @Test
    fun diagnosesValidFixtureAccessTransformerWithoutUnresolvedTargets() {
        val source = AwAtE2ETestSupport.loadFixtureText(FixturePaths.FABRIC_AW_AT_ACCESS_TRANSFORMER)
        val diagnostics = facade.diagnose(
            AwAtE2ETestSupport.requestAtOffset(source, 0, AwAtFileType.ACCESS_TRANSFORMER),
        )
        assertTrue(diagnostics.none { it.code == AtDiagnosticCodes.UNRESOLVED_CLASS })
        assertTrue(diagnostics.none { it.code == AtDiagnosticCodes.UNRESOLVED_MEMBER })
    }
}
