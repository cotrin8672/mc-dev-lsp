package io.github.mcdev.core.aw

import io.github.mcdev.core.completion.McCompletionKind
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class AccessWidenerCompletionServiceTest {
    private val classIndex = AwTestFixtures.classIndex
    private val service = AccessWidenerCompletionService(classIndex)
    private val mappingContext = AwTestFixtures.mappingContext()

    @Test
    fun completesDirectives() {
        val items = service.complete(context(AwSyntaxSlot.DIRECTIVE, "acc"))
        assertTrue(items.any { it.label == "accessible" })
    }

    @Test
    fun completesAllDirectives() {
        val items = service.complete(context(AwSyntaxSlot.DIRECTIVE, ""))
        assertEquals(4, items.size)
    }

    @Test
    fun completesKinds() {
        val items = service.complete(context(AwSyntaxSlot.KIND, "cl"))
        assertEquals("class", items.first().insertText)
    }

    @Test
    fun mutableKindCompletionOnlyOffersField() {
        val items = service.complete(
            context(AwSyntaxSlot.KIND, "", directive = AccessWidenerDirective.MUTABLE),
        )
        assertEquals(listOf("field"), items.map { it.label })
    }

    @Test
    fun extendableKindCompletionOnlyOffersClass() {
        val items = service.complete(
            context(AwSyntaxSlot.KIND, "", directive = AccessWidenerDirective.EXTENDABLE),
        )
        assertEquals(listOf("class"), items.map { it.label })
    }

    @Test
    fun completesClasses() {
        val items = service.complete(context(AwSyntaxSlot.OWNER, "Minecraft"))
        assertEquals("MinecraftClient", items.first().label)
        assertEquals("net/minecraft/client/MinecraftClient", items.first().insertText)
    }

    @Test
    fun classCompletionShowsInternalNameDetail() {
        val item = service.complete(context(AwSyntaxSlot.OWNER, "Game")).first { it.label == "GameRenderer" }
        assertEquals("net/minecraft/client/render/GameRenderer", item.detail)
    }

    @Test
    fun completesMethods() {
        val items = service.complete(
            context(
                AwSyntaxSlot.NAME,
                "set",
                kind = AccessWidenerKind.METHOD,
                owner = "net/minecraft/client/MinecraftClient",
            ),
        )
        assertTrue(items.any { it.label.contains("setScreen") })
    }

    @Test
    fun completesFields() {
        val items = service.complete(
            context(
                AwSyntaxSlot.NAME,
                "current",
                kind = AccessWidenerKind.FIELD,
                owner = "net/minecraft/client/MinecraftClient",
            ),
        )
        assertTrue(items.any { it.label.contains("currentScreen") })
    }

    @Test
    fun completesDescriptorsForMethods() {
        val items = service.complete(
            context(
                AwSyntaxSlot.DESCRIPTOR,
                "(Lnet/minecraft",
                kind = AccessWidenerKind.METHOD,
                owner = "net/minecraft/client/MinecraftClient",
                name = "setScreen",
            ),
        )
        assertTrue(items.any { it.insertText.contains("Screen") })
    }

    @Test
    fun completesHeaderNamespaces() {
        val items = service.complete(context(AwSyntaxSlot.HEADER_NAMESPACE, "nam"))
        assertTrue(items.any { it.label == "named" })
    }

    @Test
    fun mappingAwareOwnerInsertionUsesTargetNamespace() {
        val items = service.complete(
            context(AwSyntaxSlot.OWNER, "Minecraft", fileNamespace = io.github.mcdev.core.model.MappingNamespace.INTERMEDIARY),
            mappingContext,
        )
        assertEquals("net/minecraft/client/class_310", items.first { it.label == "MinecraftClient" }.insertText)
    }

    @Test
    fun methodCompletionInsertsNameOnlyAtNameSlot() {
        val item = service.complete(
            context(
                AwSyntaxSlot.NAME,
                "setScreen",
                kind = AccessWidenerKind.METHOD,
                owner = "net/minecraft/client/MinecraftClient",
            ),
        ).first()
        assertEquals("setScreen", item.insertText)
    }

    @Test
    fun methodCompletionIncludesDescriptorAtDescriptorSlot() {
        val item = service.complete(
            context(
                AwSyntaxSlot.DESCRIPTOR,
                "(Lnet/minecraft",
                kind = AccessWidenerKind.METHOD,
                owner = "net/minecraft/client/MinecraftClient",
                name = "setScreen",
            ),
        ).first()
        assertTrue(item.insertText.contains("("))
    }

    @Test
    fun descriptorCompletionUsesValueKind() {
        val item = service.complete(
            context(
                AwSyntaxSlot.DESCRIPTOR,
                "Lnet/minecraft",
                kind = AccessWidenerKind.FIELD,
                owner = "net/minecraft/client/MinecraftClient",
                name = "currentScreen",
            ),
        ).first()
        assertEquals(McCompletionKind.FIELD, item.kind)
    }

    @Test
    fun returnsEmptyForUnknownOwnerMembers() {
        val items = service.complete(
            context(
                AwSyntaxSlot.NAME,
                "foo",
                kind = AccessWidenerKind.METHOD,
                owner = "missing/Class",
            ),
        )
        assertTrue(items.isEmpty())
    }

    @Test
    fun completesOwnerFromPartialInternalClassPath() {
        val classIndex = io.github.mcdev.core.mixin.FakeClassIndex(
            classes = listOf(
                io.github.mcdev.core.mixin.ClassIndexEntry(
                    "SimpleTarget",
                    "com.example.target",
                    "com/example/target/SimpleTarget",
                ),
            ),
        )
        val service = AccessWidenerCompletionService(classIndex)
        val source = """
            accessWidener v2 named
            accessible class com/example/target/Simp
            """.trimIndent()
        val offset = source.lastIndexOf("Simp") + "Simp".length
        val context = AwContextExtractor.extractAtOffset(source, offset)!!
        val items = service.complete(context)
        assertTrue(items.any { it.insertText == "com/example/target/SimpleTarget" })
        assertTrue(items.any { it.metadata.source == "aw.class" })
    }

    private fun context(
        slot: AwSyntaxSlot,
        partial: String,
        directive: AccessWidenerDirective? = null,
        kind: AccessWidenerKind? = null,
        owner: String? = null,
        name: String? = null,
        fileNamespace: io.github.mcdev.core.model.MappingNamespace? = io.github.mcdev.core.model.MappingNamespace.NAMED,
    ) = AwAnnotationContext(
        slot = slot,
        partialValue = partial,
        valueStartOffset = 0,
        valueEndOffset = partial.length,
        lineStartOffset = 0,
        lineEndOffset = 0,
        lineNumber = 2,
        fileNamespace = fileNamespace,
        directive = directive,
        kind = kind,
        owner = owner,
        name = name,
        descriptor = null,
        isHeaderLine = slot == AwSyntaxSlot.HEADER_NAMESPACE,
    )
}
