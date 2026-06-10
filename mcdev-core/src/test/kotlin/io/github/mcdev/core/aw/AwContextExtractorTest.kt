package io.github.mcdev.core.aw

import io.github.mcdev.core.model.MappingNamespace
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull

class AwContextExtractorTest {
    @Test
    fun extractsDirectiveSlot() {
        val source = AwTestFixtures.VALID_AW
        val partial = "access"
        val offset = source.indexOf("accessible class") + partial.length
        val context = AwContextExtractor.extractAtOffset(source, offset)
        assertNotNull(context)
        assertEquals(AwSyntaxSlot.DIRECTIVE, context.slot)
        assertEquals(partial, context.partialValue)
    }

    @Test
    fun extractsKindSlot() {
        val source = AwTestFixtures.VALID_AW
        val offset = source.indexOf("method") + 2
        val context = AwContextExtractor.extractAtOffset(source, offset)
        assertNotNull(context)
        assertEquals(AwSyntaxSlot.KIND, context.slot)
        assertEquals("me", context.partialValue)
    }

    @Test
    fun extractsOwnerSlot() {
        val source = AwTestFixtures.VALID_AW
        val partial = "net/minecraft/client/Mine"
        val offset = source.indexOf(partial) + partial.length
        val context = AwContextExtractor.extractAtOffset(source, offset)
        assertNotNull(context)
        assertEquals(AwSyntaxSlot.OWNER, context.slot)
        assertEquals(partial, context.partialValue)
    }

    @Test
    fun extractsMemberNameSlot() {
        val source = AwTestFixtures.VALID_AW
        val partial = "setScr"
        val offset = source.indexOf(partial) + partial.length
        val context = AwContextExtractor.extractAtOffset(source, offset)
        assertNotNull(context)
        assertEquals(AwSyntaxSlot.NAME, context.slot)
        assertEquals(partial, context.partialValue)
        assertEquals(AccessWidenerKind.METHOD, context.kind)
    }

    @Test
    fun extractsDescriptorSlot() {
        val source = AwTestFixtures.VALID_AW
        val partial = "(Lnet/minecraft/client/gui/screen/Screen;"
        val offset = source.indexOf(partial) + partial.length
        val context = AwContextExtractor.extractAtOffset(source, offset)
        assertNotNull(context)
        assertEquals(AwSyntaxSlot.DESCRIPTOR, context.slot)
        assertEquals(partial, context.partialValue)
    }

    @Test
    fun extractsHeaderNamespaceSlot() {
        val source = AwTestFixtures.VALID_AW
        val offset = source.indexOf("named") + 3
        val context = AwContextExtractor.extractAtOffset(source, offset)
        assertNotNull(context)
        assertEquals(AwSyntaxSlot.HEADER_NAMESPACE, context.slot)
        assertEquals("nam", context.partialValue)
        assertEquals(MappingNamespace.NAMED, context.fileNamespace)
    }

    @Test
    fun parseFileNamespaceReadsHeader() {
        assertEquals(MappingNamespace.NAMED, AwContextExtractor.parseFileNamespace(AwTestFixtures.VALID_AW))
    }

    @Test
    fun toOffsetRoundTripsLineCharacter() {
        val source = "a\nbc"
        assertEquals(3, AwContextExtractor.toOffset(source, 1, 1))
    }

    @Test
    fun returnsNullForBlankLine() {
        val source = "accessWidener v2 named\n\naccessible class Foo"
        val offset = source.indexOf("\n\n") + 1
        assertNull(AwContextExtractor.extractAtOffset(source, offset))
    }

    @Test
    fun extractUsesLineCharacter() {
        val source = AwTestFixtures.VALID_AW
        val line = source.lineSequence().toList().indexOfFirst { it.contains("method") }
        val character = source.lineSequence().elementAt(line).indexOf("me") + 2
        val context = AwContextExtractor.extract(source, line, character)
        assertNotNull(context)
        assertEquals(AwSyntaxSlot.KIND, context.slot)
    }
}
