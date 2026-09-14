package io.github.mcdev.jdtls.handler

import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata
import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.fixtures.FixtureResourceLoader
import io.github.mcdev.jdtls.mixin.MixinServiceFacade
import io.github.mcdev.jdtls.mixin.SourceClassScanner
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.support.JdtlsFixtureSupport
import io.github.mcdev.protocol.McdevCompletionResponse
import io.github.mcdev.protocol.McdevErrorCode
import io.github.mcdev.protocol.McdevProtocol
import io.github.mcdev.protocol.McdevPosition
import io.github.mcdev.protocol.McdevTextEdit
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Files
import java.nio.file.Path

class McdevCompletionHandlerTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun returnsMixinClassCompletionsForFixture() {
        val handler = createHandler()
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_EXAMPLE_MIXIN)
        val (line, character) = JdtlsFixtureSupport.mixinCursorPosition(source, "SimpleTarget")
        val response = handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                    source = source,
                    line = line,
                    character = character,
                ),
            ),
        )
        val completion = assertIs<McdevCompletionResponse>(response.result)
        assertTrue(completion.items.isNotEmpty())
        assertTrue(completion.items.any { it.label == "SimpleTarget" })
        assertTrue(completion.items.any { it.insertText == "SimpleTarget.class" })
    }

    @Test
    fun mixinClassCompletionNeverInvokesSourceClassScannerOnProductionPath() {
        SourceClassScanner.resetMetrics()
        SourceClassScanner.clearDiskCache()
        val handler = createHandler()
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_EXAMPLE_MIXIN)
        val (line, character) = JdtlsFixtureSupport.mixinCursorPosition(source, "SimpleTarget")
        val workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir)

        repeat(30) { version ->
            val completion = assertIs<McdevCompletionResponse>(
                handler.handle(
                    listOf(
                        completionPayload(
                            workspaceRoot = workspaceRoot,
                            source = source,
                            line = line,
                            character = character,
                            documentVersion = version.toLong(),
                        ),
                    ),
                ).result,
            )
            assertTrue(completion.items.isNotEmpty())
            assertTrue(completion.items.any { it.label == "SimpleTarget" })
        }

        assertEquals(0, SourceClassScanner.diskReadCount())
        assertEquals(0, SourceClassScanner.diskParseCount())
        assertEquals(0, SourceClassScanner.overlayParseCount())
    }

    @Test
    fun mixinClassCompletionReturnsBufferDeclaredClassWhenJdtUnavailable() {
        val handler = McdevCompletionHandler(projectService = FileBasedProjectContextService())
        val source = """
            package com.example.target;

            import org.spongepowered.asm.mixin.Mixin;

            @Mixin(BufferOnly
            public class BufferOnlyTarget {
            }
        """.trimIndent()
        val (line, character) = JdtlsFixtureSupport.mixinCursorPosition(source, "BufferOnly")
        val response = handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                    source = source,
                    line = line,
                    character = character,
                    documentVersion = 1,
                ),
            ),
        )
        val completion = assertIs<McdevCompletionResponse>(response.result)
        val item = completion.items.firstOrNull { it.label == "BufferOnlyTarget" }
        assertNotNull(item)
        assertEquals("mixin.target", item.metadata["source"])
        assertEquals("BufferOnlyTarget.class", item.insertText)
        assertEquals("HAND_WRITTEN_FALLBACK", completion.debug?.parseSource)
    }

    @Test
    fun returnsEmptyItemsWhenCursorIsOutsideAnnotation() {
        val handler = createHandler()
        val source = "package com.example;\npublic class Plain {}"
        val response = handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                    source = source,
                    line = 1,
                    character = 10,
                ),
            ),
        )
        val completion = assertIs<McdevCompletionResponse>(response.result)
        assertTrue(completion.items.isEmpty())
    }

    @Test
    fun reusesSemanticModelForSameDocumentVersion() {
        val handler = createHandler()
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_EXAMPLE_MIXIN)
        val (line, character) = JdtlsFixtureSupport.mixinCursorPosition(source, "SimpleTarget")
        val payload = completionPayload(
            workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
            source = source,
            line = line,
            character = character,
            documentVersion = 42,
        )

        val first = assertIs<McdevCompletionResponse>(handler.handle(listOf(payload)).result)
        val second = assertIs<McdevCompletionResponse>(handler.handle(listOf(payload)).result)

        assertEquals(false, first.debug?.semanticCacheHit)
        assertEquals(true, second.debug?.semanticCacheHit)
        assertEquals(42, second.debug?.documentVersion)
        assertNotNull(second.debug?.astParseMs)
    }

    @Test
    fun reusesNegativeCompletionForSameZeroItemRequest() {
        val handler = createHandler()
        val source = "package com.example;\npublic class Plain {}"
        val payload = completionPayload(
            workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
            source = source,
            line = 1,
            character = 10,
            documentVersion = 7,
        )

        val first = assertIs<McdevCompletionResponse>(handler.handle(listOf(payload)).result)
        val second = assertIs<McdevCompletionResponse>(handler.handle(listOf(payload)).result)

        assertEquals("NO_COMPLETION_CONTEXT", first.debug?.zeroItemReason)
        assertEquals(true, second.debug?.negativeCacheHit)
        assertEquals("NO_COMPLETION_CONTEXT", second.debug?.zeroItemReason)
    }

    @Test
    fun reusesNegativeCompletionBeforeTtlExpires() {
        var now = 1_000L
        val handler = McdevCompletionHandler(
            projectService = FileBasedProjectContextService(),
            currentTimeMillis = { now },
        ).also { setupBasicFixture() }
        val source = "package com.example;\npublic class Plain {}"
        val payload = completionPayload(
            workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
            source = source,
            line = 1,
            character = 10,
            documentVersion = 7,
        )

        val first = assertIs<McdevCompletionResponse>(handler.handle(listOf(payload)).result)
        now += 499
        val second = assertIs<McdevCompletionResponse>(handler.handle(listOf(payload)).result)

        assertEquals(false, first.debug?.negativeCacheHit)
        assertEquals("NO_COMPLETION_CONTEXT", first.debug?.zeroItemReason)
        assertEquals(true, second.debug?.negativeCacheHit)
        assertEquals("NO_COMPLETION_CONTEXT", second.debug?.zeroItemReason)
    }

    @Test
    fun expiresNegativeCompletionAfterTtlAndRecomputes() {
        var now = 1_000L
        val handler = McdevCompletionHandler(
            projectService = FileBasedProjectContextService(),
            currentTimeMillis = { now },
        ).also { setupBasicFixture() }
        val source = "package com.example;\npublic class Plain {}"
        val payload = completionPayload(
            workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
            source = source,
            line = 1,
            character = 10,
            documentVersion = 7,
        )

        val first = assertIs<McdevCompletionResponse>(handler.handle(listOf(payload)).result)
        val second = assertIs<McdevCompletionResponse>(handler.handle(listOf(payload)).result)
        now += 501
        val third = assertIs<McdevCompletionResponse>(handler.handle(listOf(payload)).result)

        assertEquals(false, first.debug?.negativeCacheHit)
        assertEquals(true, second.debug?.negativeCacheHit)
        assertEquals(false, third.debug?.negativeCacheHit)
        assertEquals("NO_COMPLETION_CONTEXT", third.debug?.zeroItemReason)
        assertEquals(true, third.debug?.semanticCacheHit)
        assertEquals(1, handler.negativeCacheSizeForTests())
    }

    @Test
    fun capsNegativeCacheAt256DistinctRequests() {
        val handler = createHandler()
        val source = "package com.example;\npublic class Plain {}"
        val workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir)

        repeat(257) { index ->
            handler.handle(
                listOf(
                    completionPayload(
                        workspaceRoot = workspaceRoot,
                        source = source,
                        line = 1,
                        character = 10 + index,
                        documentVersion = 7,
                    ),
                ),
            )
        }

        assertTrue(handler.negativeCacheSizeForTests() <= 256)
    }

    @Test
    fun reusesCandidateCacheForSameInjectMethodCompletion() {
        val handler = createHandler()
        val source = """
            package com.example.mixin;
            import com.example.target.SimpleTarget;
            import org.spongepowered.asm.mixin.Mixin;
            import org.spongepowered.asm.mixin.injection.Inject;
            @Mixin(SimpleTarget.class)
            public abstract class CacheMixin {
                @Inject(method = "dra")
            }
        """.trimIndent()
        val marker = "method = \"dra"
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, source.indexOf(marker) + marker.length)
        val payload = completionPayload(
            workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
            source = source,
            line = line,
            character = character,
            documentVersion = 8,
        )

        assertIs<McdevCompletionResponse>(handler.handle(listOf(payload)).result)
        val second = assertIs<McdevCompletionResponse>(handler.handle(listOf(payload)).result)

        assertEquals(true, second.debug?.candidateCacheHit)
    }

    @Test
    fun missingWorkspaceRootReturnsIncompleteContextError() {
        val handler = McdevCompletionHandler()
        val response = handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = "",
                    source = "@Mixin(Simple)",
                    line = 0,
                    character = 8,
                ),
            ),
        )
        assertEquals(McdevErrorCode.INCOMPLETE_PROJECT_CONTEXT, response.error?.code)
    }

    @Test
    fun protocolMismatchReturnsStructuredError() {
        val handler = createHandler()
        val payload = completionPayload(
            workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
            source = "@Mixin(Simple)",
            line = 0,
            character = 8,
        ).toMutableMap()
        payload["protocolVersion"] = 99
        val response = handler.handle(listOf(payload))
        assertEquals(McdevErrorCode.PROTOCOL_MISMATCH, response.error?.code)
    }

    @Test
    fun completionItemsIncludeMetadataAndKind() {
        val handler = createHandler()
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_EXAMPLE_MIXIN)
        val (line, character) = JdtlsFixtureSupport.mixinCursorPosition(source, "SimpleTarget")
        val response = handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                    source = source,
                    line = line,
                    character = character,
                ),
            ),
        )
        val item = assertIs<McdevCompletionResponse>(response.result).items.first()
        assertEquals("class", item.kind)
        assertEquals("mixin.target", item.metadata["source"])
        assertNotNull(item.filterText)
    }

    @Test
    fun importModeMixinClassCompletionIncludesImportAdditionalEdit() {
        val handler = createHandler()
        val source = """
            package com.example.mixin;

            import org.spongepowered.asm.mixin.Mixin;

            @Mixin(Simple
        """.trimIndent()
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, source.length)
        val response = handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                    source = source,
                    line = line,
                    character = character,
                ),
            ),
        )
        val item = assertIs<McdevCompletionResponse>(response.result)
            .items
            .first { it.label == "SimpleTarget" }
        assertEquals(1, item.additionalEdits.size)
        assertTrue(item.additionalEdits.first().newText.contains("import com.example.target.SimpleTarget;"))
    }

    @Test
    fun atTargetCompletionRemapsInsertTextWithFixtureMappings() {
        val atTargetItem = McCompletionItem(
            label = "draw(String, float, float): void",
            detail = "SimpleTarget",
            documentation = "Lcom/example/target/SimpleTarget;draw(Ljava/lang/String;FF)V",
            filterText = "draw",
            insertText = "Lcom/example/target/SimpleTarget;draw(Ljava/lang/String;FF)V",
            kind = McCompletionKind.METHOD,
            sortKey = "0400_draw",
            metadata = McCompletionMetadata(
                source = "mixin.atTarget",
                owner = "com/example/target/SimpleTarget",
                name = "draw",
                descriptor = "(Ljava/lang/String;FF)V",
            ),
        )
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_BASIC, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        val handler = McdevCompletionHandler(
            projectService = FileBasedProjectContextService(),
            mixinFacade = MixinServiceFacade(completeOverride = { _, _, _, _, _ -> listOf(atTargetItem) }),
        )
        val source = """@At(value = "INVOKE", target = "draw")"""
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, source.indexOf("draw") + "draw".length)
        val response = handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                    source = source,
                    line = line,
                    character = character,
                ),
            ),
        )
        val item = assertIs<McdevCompletionResponse>(response.result).items.single()
        assertEquals("mixin.atTarget", item.metadata["source"])
        assertEquals(
            "Lcom/example/target/class_1;method_1(Ljava/lang/String;FF)V",
            item.insertText,
        )
        assertEquals(item.insertText, item.edit?.newText)
    }

    @Test
    fun returnsMixinExtrasMethodCompletionsThroughFacade() {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_MIXINEXTRAS, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        val handler = McdevCompletionHandler(
            projectService = FileBasedProjectContextService(),
            mixinFacade = MixinServiceFacade(),
        )
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_MIXINEXTRAS_MIXIN)
        val (line, character) = JdtlsFixtureSupport.memberCursorPosition(source, "dra")
        val response = handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                    source = source,
                    line = line,
                    character = character,
                ),
            ),
        )
        val completion = assertIs<McdevCompletionResponse>(response.result)
        assertTrue(completion.items.any { it.metadata["source"] == "mixinextras.injectMethod" })
        assertTrue(completion.items.any { it.insertText.startsWith("draw") })
    }

    @Test
    fun injectMethodCompletionHonorsAlwaysDescriptorOption() {
        val handler = createHandler()
        val source = """
            package com.example.mixin;
            import com.example.target.SimpleTarget;
            import org.spongepowered.asm.mixin.Mixin;
            import org.spongepowered.asm.mixin.injection.At;
            import org.spongepowered.asm.mixin.injection.Inject;
            import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
            @Mixin(SimpleTarget.class)
            public abstract class DescriptorMixin {
                @Inject(method = "dra", at = @At("HEAD"))
                private void mcdev${"$"}onDraw(CallbackInfo ci) {}
            }
        """.trimIndent()
        val (line, character) = JdtlsFixtureSupport.memberCursorPosition(source, "dra")
        val payload = completionPayload(
            workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
            source = source,
            line = line,
            character = character,
            injectMethodDescriptor = "always",
        )
        val response = handler.handle(listOf(payload))
        val completion = assertIs<McdevCompletionResponse>(response.result)
        assertTrue(completion.items.any { it.metadata["source"] == "mixin.injectMethod" })
        assertTrue(completion.items.any { it.insertText.contains("(Ljava/lang/String;FF)V") })
    }

    @Test
    fun returnsInjectMethodCompletionsAtEmptyOpenQuote() {
        val handler = createHandler()
        val source = """
            package com.example.mixin;
            import com.example.target.SimpleTarget;
            import org.spongepowered.asm.mixin.Mixin;
            import org.spongepowered.asm.mixin.injection.Inject;
            @Mixin(SimpleTarget.class)
            public abstract class OpenQuoteMixin {
                @Inject(method = "
            }
        """.trimIndent()
        val marker = "method = \""
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, source.indexOf(marker) + marker.length)
        val response = handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                    source = source,
                    line = line,
                    character = character,
                ),
            ),
        )
        val completion = assertIs<McdevCompletionResponse>(response.result)
        assertTrue(completion.items.any { it.metadata["source"] == "mixin.injectMethod" })
        assertTrue(completion.items.any { it.insertText.startsWith("draw") })
        assertEquals("mcdev.completion", completion.debug?.command)
        assertEquals(null, completion.debug?.zeroItemReason)
        assertTrue((completion.debug?.semanticTargetCount ?: 0) > 0)
        assertTrue(completion.debug?.parseSource != null)
    }

    @Test
    fun injectMethodEditPreservesQuotesAndReplacesTheWholeStringValue() {
        val handler = createHandler()
        val source = """
            package com.example.mixin;
            import com.example.target.SimpleTarget;
            import org.spongepowered.asm.mixin.Mixin;
            import org.spongepowered.asm.mixin.injection.Inject;
            @Mixin(SimpleTarget.class)
            public abstract class QuoteMixin {
                @Inject(method = "dra")
            }
        """.trimIndent()
        val cursorOffset = source.indexOf("dra") + 2
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, cursorOffset)
        val completion = assertIs<McdevCompletionResponse>(
            handler.handle(
                listOf(completionPayload(JdtlsFixtureSupport.workspaceUri(tempDir), source, line, character)),
            ).result,
        )
        val item = completion.items.first { it.metadata["source"] == "mixin.injectMethod" && it.insertText.startsWith("draw") }
        assertEquals("value", item.kind)
        assertEquals(
            source.replace("method = \"dra\"", "method = \"${item.insertText}\""),
            applyEdit(source, assertNotNull(item.edit)),
        )
    }

    @Test
    fun injectMethodArrayEditOnlyReplacesTheActiveElement() {
        val handler = createHandler()
        val source = """
            package com.example.mixin;
            import com.example.target.SimpleTarget;
            import org.spongepowered.asm.mixin.Mixin;
            import org.spongepowered.asm.mixin.injection.Inject;
            @Mixin(SimpleTarget.class)
            public abstract class ArrayMixin {
                @Inject(method = { "dra", "render" })
            }
        """.trimIndent()
        val cursorOffset = source.indexOf("dra") + "dra".length
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, cursorOffset)
        val completion = assertIs<McdevCompletionResponse>(
            handler.handle(
                listOf(completionPayload(JdtlsFixtureSupport.workspaceUri(tempDir), source, line, character)),
            ).result,
        )
        val item = completion.items.first { it.metadata["source"] == "mixin.injectMethod" && it.insertText.startsWith("draw") }
        assertEquals(
            source.replace("{ \"dra\", \"render\" }", "{ \"${item.insertText}\", \"render\" }"),
            applyEdit(source, assertNotNull(item.edit)),
        )
    }

    @Test
    fun incompleteMethodAttributeOffersQuotedSnippet() {
        val handler = createHandler()
        val source = """
            package com.example.mixin;
            import com.example.target.SimpleTarget;
            import org.spongepowered.asm.mixin.Mixin;
            import org.spongepowered.asm.mixin.injection.Inject;
            @Mixin(SimpleTarget.class)
            public abstract class AttributeMixin {
                @Inject(meth)
            }
        """.trimIndent()
        val cursorOffset = source.indexOf("meth") + "meth".length
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, cursorOffset)
        val completion = assertIs<McdevCompletionResponse>(
            handler.handle(
                listOf(completionPayload(JdtlsFixtureSupport.workspaceUri(tempDir), source, line, character)),
            ).result,
        )
        val method = completion.items.first { it.metadata["source"] == "mixin.attribute" && it.metadata["name"] == "method" }
        assertEquals("method = \"${'$'}{1}\"${'$'}0", method.insertText)
        assertEquals("snippet", method.insertTextFormat)
        assertEquals("method = \"${'$'}{1}\"${'$'}0", method.edit?.newText)
    }

    @Test
    fun mixinMemberCompletionDoesNotScanSourceOnlySiblingTarget() {
        SourceClassScanner.resetMetrics()
        SourceClassScanner.clearDiskCache()
        val targetDir = tempDir.resolve("src/main/java/com/example/target")
        Files.createDirectories(targetDir)
        Files.writeString(
            targetDir.resolve("SourceOnlyTarget.java"),
            """
                package com.example.target;
                public class SourceOnlyTarget {
                    public void pulse() {}
                    public int measure(String label) { return label.length(); }
                }
            """.trimIndent(),
        )
        val handler = McdevCompletionHandler(projectService = FileBasedProjectContextService())
        val source = """
            package com.example.mixin;
            import com.example.target.SourceOnlyTarget;
            import org.spongepowered.asm.mixin.Mixin;
            import org.spongepowered.asm.mixin.injection.Inject;
            @Mixin(SourceOnlyTarget.class)
            public abstract class SourceOnlyMixin {
                @Inject(method = "
            }
        """.trimIndent()
        val marker = "method = \""
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, source.indexOf(marker) + marker.length)
        val response = handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                    source = source,
                    line = line,
                    character = character,
                ),
            ),
        )
        val completion = assertIs<McdevCompletionResponse>(response.result)
        assertFalse(completion.items.any { it.insertText == "pulse" })
        assertFalse(completion.items.any { it.insertText == "measure" })
        assertEquals(0, SourceClassScanner.diskReadCount())
        assertEquals(0, SourceClassScanner.diskParseCount())
        assertEquals(0, SourceClassScanner.overlayParseCount())
    }

    @Test
    fun atTargetCompletionEndToEndWithoutFacadeOverride() {
        val handler = createHandler()
        val source = """
            package com.example.mixin;
            import com.example.target.SimpleTarget;
            import org.spongepowered.asm.mixin.Mixin;
            import org.spongepowered.asm.mixin.injection.At;
            import org.spongepowered.asm.mixin.injection.Inject;
            import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
            @Mixin(SimpleTarget.class)
            public abstract class AtTargetMixin {
                @Inject(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "L"))
                private void mcdev${"$"}onDraw(String text, float x, float y, CallbackInfo ci) {}
            }
        """.trimIndent()
        val marker = "target = \"L"
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, source.indexOf(marker) + marker.length)
        val response = handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                    source = source,
                    line = line,
                    character = character,
                ),
            ),
        )
        val completion = assertIs<McdevCompletionResponse>(response.result)
        assertTrue(completion.items.isNotEmpty())
        assertTrue(completion.items.any { it.metadata["source"] == "mixin.atTarget" })
        assertTrue(completion.items.any { it.label.contains("length") })
        assertNotNull(completion.items.first { it.metadata["source"] == "mixin.atTarget" }.edit)
    }

    @Test
    fun fqnInsertModeReturnsQualifiedClassName() {
        val handler = createHandler()
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_EXAMPLE_MIXIN)
        val (line, character) = JdtlsFixtureSupport.mixinCursorPosition(source, "SimpleTarget")
        val payload = completionPayload(
            workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
            source = source,
            line = line,
            character = character,
        ).toMutableMap()
        payload["options"] = mapOf(
            "preferredAtTarget" to "descriptor",
            "mixinClassInsert" to "fqn",
            "injectMethodDescriptor" to "auto",
        )
        val response = handler.handle(listOf(payload))
        val completion = assertIs<McdevCompletionResponse>(response.result)
        assertTrue(completion.items.any { it.insertText == "com.example.target.SimpleTarget.class" })
    }

    @Test
    fun bufferOnlyInjectAttributeCompletionSkipsProjectSessionLoad() {
        val (projectService, builderCount) = countingProjectService()
        val handler = McdevCompletionHandler(projectService = projectService)
        val source = """
            package com.example.mixin;
            import org.spongepowered.asm.mixin.injection.Inject;
            public abstract class AttributeMixin {
                @Inject(meth
            }
        """.trimIndent()
        val cursorOffset = source.indexOf("meth") + "meth".length
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, cursorOffset)
        val response = handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                    source = source,
                    line = line,
                    character = character,
                ),
            ),
        )
        val completion = assertIs<McdevCompletionResponse>(response.result)
        assertEquals(0, builderCount())
        assertTrue(completion.items.any { it.metadata["source"] == "mixin.attribute" && it.metadata["name"] == "method" })
        assertBufferOnlySessionDebug(completion)
    }

    @Test
    fun projectAwareCompletionDoesNotCacheSessionBeforeJdtProjectExists() {
        val (projectService, builderCount) = countingProjectService()
        val handler = McdevCompletionHandler(projectService = projectService)
        val source = "@Inject(meth"
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, source.length)

        val response = handler.handleProjectAware(
            listOf(
                completionPayload(
                    workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                    source = source,
                    line = line,
                    character = character,
                ),
            ),
        )

        assertEquals(McdevErrorCode.INCOMPLETE_PROJECT_CONTEXT, response.error?.code)
        assertEquals(0, builderCount())
    }

    @Test
    fun projectAwareCompletionBypassesBufferOnlyShortcut() {
        val (projectService, builderCount) = countingProjectService()
        val projectItem = McCompletionItem(
            label = "project-method",
            detail = "project",
            documentation = null,
            filterText = "project-method",
            insertText = "project-method",
            kind = McCompletionKind.METHOD,
            sortKey = "0000_project-method",
            metadata = McCompletionMetadata(source = "project-aware"),
        )
        val project = ImportingJavaProject()
        val handler = McdevCompletionHandler(
            projectService = projectService,
            mixinFacade = MixinServiceFacade(
                javaProjectResolver = { project },
                completeOverride = { _, _, _, _, _ -> listOf(projectItem) },
            ),
        )
        val source = "@Inject(meth"
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, source.length)
        val pending = handler.handleProjectAware(listOf(completionPayload(
            workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir), source = source,
            line = line, character = character,
        )))
        assertEquals(McdevErrorCode.INCOMPLETE_PROJECT_CONTEXT, pending.error?.code)
        assertEquals(0, builderCount())
        project.ready = true

        val completion = assertIs<McdevCompletionResponse>(
            handler.handleProjectAware(
                listOf(
                    completionPayload(
                        workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                        source = source,
                        line = line,
                        character = character,
                    ),
                ),
            ).result,
        )

        assertEquals(1, builderCount())
        assertEquals(listOf("project-method"), completion.items.map { it.label })
    }

    private class ImportingJavaProject {
        var ready = false
        fun findType(name: String): Any? = if (ready && name == "org.spongepowered.asm.mixin.Mixin") Any() else null
    }

    @Test
    fun bufferOnlyAtHeadValueCompletionSkipsProjectSessionLoad() {
        val (projectService, builderCount) = countingProjectService()
        val handler = McdevCompletionHandler(projectService = projectService)
        val source = """@Inject(method = "tick", at = @At(value = "HE"))"""
        val partial = "HE"
        val offset = source.indexOf("\"$partial\"") + 1 + partial.length
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, offset)
        val response = handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                    source = source,
                    line = line,
                    character = character,
                ),
            ),
        )
        val completion = assertIs<McdevCompletionResponse>(response.result)
        assertEquals(0, builderCount())
        assertTrue(completion.items.any { it.insertText == "HEAD" })
        assertBufferOnlySessionDebug(completion)
    }

    @Test
    fun bufferOnlyAtHeadValueCompletionStaysStableAcrossSequentialTyping() {
        val (projectService, builderCount) = countingProjectService()
        val handler = McdevCompletionHandler(projectService = projectService)
        for ((version, partial) in listOf("", "H", "HE", "HEA").withIndex()) {
            val source = """@Inject(method = "tick", at = @At(value = "$partial"))"""
            val offset = source.indexOf("\"$partial\"") + 1 + partial.length
            val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, offset)
            val completion = assertIs<McdevCompletionResponse>(
                handler.handle(
                    listOf(
                        completionPayload(
                            workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                            source = source,
                            line = line,
                            character = character,
                            documentVersion = version.toLong(),
                        ),
                    ),
                ).result,
            )
            assertEquals(0, builderCount())
            assertTrue(completion.items.any { it.insertText == "HEAD" })
            assertBufferOnlySessionDebug(completion)
        }
    }

    @Test
    fun bufferOnlyDefinitionIdCompletionSkipsProjectSessionLoad() {
        val (projectService, builderCount) = countingProjectService()
        val handler = McdevCompletionHandler(projectService = projectService)
        val source = definitionIdHandlerSource(expressionValue = "len")
        val tokenStart = source.indexOf("\"len\"") + 1
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, tokenStart + "len".length)
        val completion = assertIs<McdevCompletionResponse>(
            handler.handle(
                listOf(
                    completionPayload(
                        workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                        source = source,
                        line = line,
                        character = character,
                    ),
                ),
            ).result,
        )
        assertEquals(0, builderCount())
        val item = completion.items.first {
            it.metadata["source"] == "mixinextras.definitionId" && it.insertText == "lengthCall"
        }
        assertEquals("lengthCall", item.edit?.newText)
        assertEquals(
            source.replace("""@Expression("len")""", """@Expression("lengthCall")"""),
            applyEdit(source, assertNotNull(item.edit)),
        )
        val editStart = positionToOffset(source, assertNotNull(item.edit).range.start)
        val editEnd = positionToOffset(source, item.edit!!.range.end)
        assertEquals(tokenStart, editStart)
        assertEquals(tokenStart + "len".length, editEnd)
        assertBufferOnlySessionDebug(completion)
    }

    @Test
    fun bufferOnlyDefinitionIdCompletionDoesNotLeakAdjacentHandlerDefinitions() {
        val (projectService, builderCount) = countingProjectService()
        val handler = McdevCompletionHandler(projectService = projectService)
        val source = adjacentDefinitionIdHandlerSource()
        val tokenStart = source.indexOf("\"len\"") + 1
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, tokenStart + "len".length)
        val completion = assertIs<McdevCompletionResponse>(
            handler.handle(
                listOf(
                    completionPayload(
                        workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                        source = source,
                        line = line,
                        character = character,
                    ),
                ),
            ).result,
        )
        assertEquals(0, builderCount())
        val definitionIds = completion.items
            .filter { it.metadata["source"] == "mixinextras.definitionId" }
            .map { it.insertText }
        assertEquals(listOf("lengthCall"), definitionIds)
        assertBufferOnlySessionDebug(completion)
    }

    @Test
    fun bufferOnlyExpressionReturnKeywordCompletionSkipsProjectSessionLoad() {
        val (projectService, builderCount) = countingProjectService()
        val handler = McdevCompletionHandler(projectService = projectService)
        val source = """@Expression("{ ret")"""
        val cursorOffset = source.indexOf("ret") + "ret".length
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, cursorOffset)
        val completion = assertIs<McdevCompletionResponse>(
            handler.handle(
                listOf(
                    completionPayload(
                        workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                        source = source,
                        line = line,
                        character = character,
                    ),
                ),
            ).result,
        )
        assertEquals(0, builderCount())
        val item = completion.items.first { it.insertText == "return" }
        assertEquals("mixinextras.expressionValue", item.metadata["source"])
        assertEquals("return", item.edit?.newText)
        assertEquals(
            source.replace("ret", "return"),
            applyEdit(source, assertNotNull(item.edit)),
        )
        assertBufferOnlySessionDebug(completion)
    }

    @Test
    fun atTargetCompletionLoadsProjectSession() {
        val (projectService, builderCount) = countingProjectService()
        setupBasicFixture()
        val handler = McdevCompletionHandler(projectService = projectService)
        val source = """
            package com.example.mixin;
            import com.example.target.SimpleTarget;
            import org.spongepowered.asm.mixin.Mixin;
            import org.spongepowered.asm.mixin.injection.At;
            import org.spongepowered.asm.mixin.injection.Inject;
            import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
            @Mixin(SimpleTarget.class)
            public abstract class AtTargetMixin {
                @Inject(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "L"))
                private void mcdev${"$"}onDraw(String text, float x, float y, CallbackInfo ci) {}
            }
        """.trimIndent()
        val marker = "target = \"L"
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, source.indexOf(marker) + marker.length)
        handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir),
                    source = source,
                    line = line,
                    character = character,
                ),
            ),
        )
        assertEquals(1, builderCount())
    }

    @Test
    fun accessWidenerCompletionLoadsProjectSession() {
        val (projectService, builderCount) = countingProjectService()
        val handler = McdevCompletionHandler(projectService = projectService)
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_AW_AT_ACCESS_WIDENER)
        val marker = "accessible class com/example/target/Simple"
        val offset = source.indexOf(marker) + marker.length
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, offset)
        val workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir)
        handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = workspaceRoot,
                    source = source,
                    line = line,
                    character = character,
                    languageId = "accesswidener",
                    documentUri = "$workspaceRoot/src/main/resources/mod.accesswidener",
                ),
            ),
        )
        assertEquals(1, builderCount())
    }

    @Test
    fun accessTransformerCompletionLoadsProjectSession() {
        val (projectService, builderCount) = countingProjectService()
        val handler = McdevCompletionHandler(projectService = projectService)
        val source = FixtureResourceLoader.loadText(FixturePaths.FABRIC_AW_AT_ACCESS_TRANSFORMER)
        val marker = "public com.example.target.SimpleTarget counter"
        val offset = source.indexOf(marker) + marker.indexOf("counter") + 3
        val (line, character) = JdtlsFixtureSupport.offsetToPosition(source, offset)
        val workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir)
        handler.handle(
            listOf(
                completionPayload(
                    workspaceRoot = workspaceRoot,
                    source = source,
                    line = line,
                    character = character,
                    languageId = "accesstransformer",
                    documentUri = "$workspaceRoot/src/main/resources/mod_at.cfg",
                ),
            ),
        )
        assertEquals(1, builderCount())
    }

    private fun definitionIdHandlerSource(
        expressionValue: String = "len",
        definitionId: String = "lengthCall",
    ): String = """
        package com.example.mixin;
        import com.example.target.SimpleTarget;
        import com.llamalad7.mixinextras.expression.Definition;
        import com.llamalad7.mixinextras.expression.Expression;
        import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
        import org.spongepowered.asm.mixin.Mixin;
        import org.spongepowered.asm.mixin.injection.At;
        @Mixin(SimpleTarget.class)
        public abstract class DefinitionIdMixin {
            @Definition(id = "$definitionId")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
            @Expression("$expressionValue")
            private float mcdev${'$'}handler(float original) { return original; }
        }
    """.trimIndent()

    private fun adjacentDefinitionIdHandlerSource(): String = """
        package com.example.mixin;
        import com.example.target.SimpleTarget;
        import com.llamalad7.mixinextras.expression.Definition;
        import com.llamalad7.mixinextras.expression.Expression;
        import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
        import org.spongepowered.asm.mixin.Mixin;
        import org.spongepowered.asm.mixin.injection.At;
        @Mixin(SimpleTarget.class)
        public abstract class AdjacentDefinitionIdMixin {
            @Definition(id = "lengthCall")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
            @Expression("len")
            private float mcdev${'$'}handler1(float original) { return original; }

            @Definition(id = "lengthOther")
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "MIXINEXTRAS:EXPRESSION"))
            @Expression("")
            private float mcdev${'$'}handler2(float original) { return original; }
        }
    """.trimIndent()

    private fun countingProjectService(): Pair<FileBasedProjectContextService, () -> Int> {
        var count = 0
        val delegate = FileBasedProjectContextService()
        val projectService = FileBasedProjectContextService(contextBuilder = { root ->
            count++
            delegate.buildProjectContext(root)
        })
        return projectService to { count }
    }

    private fun assertBufferOnlySessionDebug(completion: McdevCompletionResponse) {
        assertEquals(0, completion.debug?.loadSessionMs)
        assertEquals(false, completion.debug?.projectSessionCacheHit)
        assertEquals(0, completion.debug?.projectSessionVersion)
        assertEquals(null, completion.debug?.documentCacheHit)
        assertEquals(null, completion.debug?.semanticCacheHit)
        assertEquals(null, completion.debug?.candidateCacheHit)
    }

    private fun createHandler(): McdevCompletionHandler {
        setupBasicFixture()
        return McdevCompletionHandler(projectService = FileBasedProjectContextService())
    }

    private fun setupBasicFixture() {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_BASIC, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
    }

    private fun applyEdit(source: String, edit: McdevTextEdit): String {
        val start = positionToOffset(source, edit.range.start)
        val end = positionToOffset(source, edit.range.end)
        return source.replaceRange(start, end, edit.newText)
    }

    private fun positionToOffset(source: String, position: McdevPosition): Int {
        var offset = 0
        repeat(position.line) {
            offset = source.indexOf('\n', offset).let { newline -> if (newline < 0) source.length else newline + 1 }
        }
        return (offset + position.character).coerceAtMost(source.length)
    }

    private fun completionPayload(
        workspaceRoot: String,
        source: String,
        line: Int,
        character: Int,
        injectMethodDescriptor: String = "auto",
        documentVersion: Long? = null,
        languageId: String = "java",
        documentUri: String = "$workspaceRoot/src/main/java/com/example/mixin/ExampleMixin.java",
    ): Map<String, Any?> = mapOf(
        "protocolVersion" to McdevProtocol.VERSION,
        "workspaceRoot" to workspaceRoot,
        "documentUri" to documentUri,
        "languageId" to languageId,
        "position" to mapOf("line" to line, "character" to character),
        "documentVersion" to documentVersion,
        "bufferText" to source,
        "client" to mapOf("name" to "mcdev.nvim", "version" to "0.1.0"),
        "trigger" to mapOf("kind" to "manual", "character" to null),
        "options" to mapOf(
            "preferredAtTarget" to "descriptor",
            "mixinClassInsert" to "import",
            "injectMethodDescriptor" to injectMethodDescriptor,
        ),
    )
}
