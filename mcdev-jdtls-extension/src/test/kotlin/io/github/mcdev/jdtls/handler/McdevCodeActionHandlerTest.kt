package io.github.mcdev.jdtls.handler

import io.github.mcdev.core.mixin.MixinDiagnosticCodes
import io.github.mcdev.core.mixinextras.MixinExtrasDiagnosticCodes
import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.fixtures.FixtureResourceLoader
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.project.UriPathSupport
import io.github.mcdev.jdtls.support.JdtlsFixtureSupport
import io.github.mcdev.protocol.McdevCodeActionResponse
import io.github.mcdev.protocol.McdevErrorCode
import io.github.mcdev.protocol.McdevPosition
import io.github.mcdev.protocol.McdevProtocol
import io.github.mcdev.protocol.McdevTextEdit
import kotlin.io.path.createDirectories
import kotlin.io.path.writeText
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Path

class McdevCodeActionHandlerTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun returnsMixinConfigAddFix() {
        val handler = createHandler()
        val source = """
            package com.example.mixin;
            import com.example.target.SimpleTarget;
            import org.spongepowered.asm.mixin.Mixin;
            @Mixin(SimpleTarget.class)
            public abstract class UnlistedMixin {}
        """.trimIndent()
        val response = handler.handle(
            listOf(
                codeActionPayload(
                    source = source,
                    diagnosticCodes = listOf(MixinDiagnosticCodes.MIXIN_CLASS_NOT_LISTED_IN_CONFIG),
                ),
            ),
        )
        val result = assertIs<McdevCodeActionResponse>(response.result)
        assertTrue(result.actions.isNotEmpty())
        assertTrue(result.actions.any { it.title.contains("UnlistedMixin") })
        assertTrue(result.actions.any { it.kind == "quickfix.mixin.config" })
        assertTrue(result.actions.first().edits.isNotEmpty())
    }

    @Test
    fun missingWorkspaceRootReturnsIncompleteContextError() {
        val handler = McdevCodeActionHandler()
        val response = handler.handle(
            listOf(
                codeActionPayload(
                    source = "@Mixin(Simple)",
                    workspaceRoot = "",
                ),
            ),
        )
        assertEquals(McdevErrorCode.INCOMPLETE_PROJECT_CONTEXT, response.error?.code)
    }

    @Test
    fun protocolMismatchReturnsStructuredError() {
        val handler = createHandler()
        val payload = codeActionPayload(source = "@Mixin(Simple)").toMutableMap()
        val context = (payload["context"] as Map<String, Any?>).toMutableMap()
        context["protocolVersion"] = 99
        payload["context"] = context
        val response = handler.handle(listOf(payload))
        assertEquals(McdevErrorCode.PROTOCOL_MISMATCH, response.error?.code)
    }

    @Test
    fun returnsAmbiguousInjectMethodDescriptorFix() {
        val handler = createHandler()
        val source = """
            package com.example.mixin;
            import com.example.target.SimpleTarget;
            import org.spongepowered.asm.mixin.Mixin;
            import org.spongepowered.asm.mixin.injection.At;
            import org.spongepowered.asm.mixin.injection.Inject;
            import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
            @Mixin(SimpleTarget.class)
            public abstract class AmbiguousMixin {
                @Inject(method = "draw", at = @At("HEAD"))
                private void mcdev${"$"}onDraw(CallbackInfo ci) {}
            }
        """.trimIndent()
        val response = handler.handle(
            listOf(
                codeActionPayload(
                    source = source,
                    fileName = "AmbiguousMixin.java",
                    diagnosticCodes = listOf(MixinDiagnosticCodes.AMBIGUOUS_INJECT_METHOD),
                ),
            ),
        )
        val result = assertIs<McdevCodeActionResponse>(response.result)
        assertTrue(result.actions.none { it.kind == "quickfix.mixin.methodDescriptor" })
    }

    @Test
    fun returnsMixinExtrasFixHandlerSignatureThroughHandler() {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_MIXINEXTRAS, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        val handler = McdevCodeActionHandler(projectService = FileBasedProjectContextService())
        val source = """
            package com.example.mixin;
            import com.example.target.SimpleTarget;
            import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
            import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
            import org.spongepowered.asm.mixin.Mixin;
            import org.spongepowered.asm.mixin.injection.At;
            @Mixin(SimpleTarget.class)
            public abstract class BadMixinExtras {
                @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
                private void mcdev${"$"}wrapLength(String instance, Operation<Integer> original) {
                    original.call(instance);
                }
            }
        """.trimIndent()
        val response = handler.handle(
            listOf(
                codeActionPayload(
                    source = source,
                    fileName = "BadMixinExtras.java",
                    diagnosticCodes = listOf(MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE),
                ),
            ),
        )
        val result = assertIs<McdevCodeActionResponse>(response.result)
        assertTrue(result.actions.isNotEmpty())
        assertTrue(result.actions.any { it.kind == "quickfix.mixinextras.fixHandlerSignature" })
        assertTrue(
            result.actions.any { action ->
                action.edits.any { workspaceEdit ->
                    workspaceEdit.edits.any { textEdit -> textEdit.newText.contains("Operation<") }
                }
            },
        )
    }

    @Test
    fun keepsDistinctMixinExtrasSignatureFixesWithTheSameTitle() {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_MIXINEXTRAS, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        val handler = McdevCodeActionHandler(projectService = FileBasedProjectContextService())
        val source = """
            package com.example.mixin;
            import com.example.target.SimpleTarget;
            import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
            import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
            import org.spongepowered.asm.mixin.Mixin;
            import org.spongepowered.asm.mixin.injection.At;
            @Mixin(SimpleTarget.class)
            public abstract class BadMixinExtras {
                @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
                private void first(String instance, Operation<Integer> original) {
                    original.call(instance);
                }
                @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
                private void second(String instance, Operation<Integer> original) {
                    original.call(instance);
                }
            }
        """.trimIndent()
        val response = handler.handle(
            listOf(
                codeActionPayload(
                    source = source,
                    fileName = "BadMixinExtras.java",
                    diagnosticCodes = listOf(MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE),
                ),
            ),
        )
        val result = assertIs<McdevCodeActionResponse>(response.result)
        val fixes = result.actions.filter { it.kind == "quickfix.mixinextras.fixHandlerSignature" }
        assertEquals(2, fixes.size)
        assertTrue(fixes.map { it.edits.single().edits.single().range.start }.distinct().size == 2)
    }

    @Test
    fun keepsGenerationAtCursorWhenDiagnosticFilterIsPresent() {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_MIXINEXTRAS, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        val handler = McdevCodeActionHandler(projectService = FileBasedProjectContextService())
        val source = """
            package com.example.mixin;
            import com.example.target.SimpleTarget;
            import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
            import org.spongepowered.asm.mixin.Mixin;
            import org.spongepowered.asm.mixin.injection.At;
            @Mixin(SimpleTarget.class)
            public abstract class MissingMixinExtras {
                @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            }
        """.trimIndent()
        val annotationOffset = source.indexOf("@WrapOperation")
        val line = source.substring(0, annotationOffset).count { it == '\n' }
        val lineStart = source.lastIndexOf('\n', annotationOffset - 1) + 1
        val character = annotationOffset - lineStart
        val response = handler.handle(
            listOf(
                codeActionPayload(
                    source = source,
                    fileName = "MissingMixinExtras.java",
                    diagnosticCodes = listOf(MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE),
                    positionLine = line,
                    positionCharacter = character,
                ),
            ),
        )
        val result = assertIs<McdevCodeActionResponse>(response.result)
        assertTrue(result.actions.any { it.kind == "quickfix.mixinextras.generateHandler" })
    }

    @Test
    fun codeActionEditsIncludeWorkspaceDocumentUri() {
        val handler = createHandler()
        val source = """
            package com.example.mixin;
            import com.example.target.SimpleTarget;
            import org.spongepowered.asm.mixin.Mixin;
            @Mixin(SimpleTarget.class)
            public abstract class UnlistedMixin {}
        """.trimIndent()
        val response = handler.handle(
            listOf(
                codeActionPayload(
                    source = source,
                    diagnosticCodes = listOf(MixinDiagnosticCodes.MIXIN_CLASS_NOT_LISTED_IN_CONFIG),
                ),
            ),
        )
        val action = assertIs<McdevCodeActionResponse>(response.result).actions.first()
        assertTrue(action.edits.first().documentUri.contains("mixins.json"))
    }

    @Test
    fun codeActionMixinConfigEditDocumentUriIsFileUri() {
        val handler = createHandler()
        val source = """
            package com.example.mixin;
            import com.example.target.SimpleTarget;
            import org.spongepowered.asm.mixin.Mixin;
            @Mixin(SimpleTarget.class)
            public abstract class UnlistedMixin {}
        """.trimIndent()
        val response = handler.handle(
            listOf(
                codeActionPayload(
                    source = source,
                    diagnosticCodes = listOf(MixinDiagnosticCodes.MIXIN_CLASS_NOT_LISTED_IN_CONFIG),
                ),
            ),
        )
        val documentUri = assertIs<McdevCodeActionResponse>(response.result)
            .actions
            .first()
            .edits
            .first()
            .documentUri
        assertTrue(documentUri.startsWith("file:"), "Expected LSP file URI, got: $documentUri")
        assertEquals(UriPathSupport.pathToUri(tempDir.resolve("mixins.json")), documentUri)
    }

    @Test
    fun codeActionTargetsSelectedMixinConfigWhenEarlierConfigIsWrong() {
        val handler = createHandlerWithTwoMixinConfigs()
        val source = """
            package com.example.mixin;
            import com.example.target.SimpleTarget;
            import org.spongepowered.asm.mixin.Mixin;
            @Mixin(SimpleTarget.class)
            public abstract class UnlistedMixin {}
        """.trimIndent()
        val response = handler.handle(
            listOf(
                codeActionPayload(
                    source = source,
                    diagnosticCodes = listOf(MixinDiagnosticCodes.MIXIN_CLASS_NOT_LISTED_IN_CONFIG),
                ),
            ),
        )
        val action = assertIs<McdevCodeActionResponse>(response.result).actions.first()
        assertTrue(action.edits.first().documentUri.contains("mixins.json"))
        assertTrue(!action.edits.first().documentUri.contains("aaa-wrong.mixins.json"))
    }

    @Test
    fun codeActionPreservesJson5CommentAndTrailingCommaInSelectedMixinConfig() {
        val handler = createHandlerWithJson5MixinConfig()
        val mixinConfig = """
            {
              // package for mixins
              "package": "com.example.mixin",
              "mixins": [
                "ExampleMixin",
              ],
            }
        """.trimIndent()
        val source = """
            package com.example.mixin;
            import com.example.target.SimpleTarget;
            import org.spongepowered.asm.mixin.Mixin;
            @Mixin(SimpleTarget.class)
            public abstract class UnlistedMixin {}
        """.trimIndent()
        val response = handler.handle(
            listOf(
                codeActionPayload(
                    source = source,
                    diagnosticCodes = listOf(MixinDiagnosticCodes.MIXIN_CLASS_NOT_LISTED_IN_CONFIG),
                ),
            ),
        )
        val action = assertIs<McdevCodeActionResponse>(response.result)
            .actions
            .first { it.kind == "quickfix.mixin.config" }
        val textEdit = action.edits.single().edits.single()
        assertEquals(textEdit.range.start, textEdit.range.end)
        assertTrue(textEdit.range.start != McdevPosition(0, 0))
        assertTrue(textEdit.newText.contains("UnlistedMixin"))
        val updated = applyTextEdit(mixinConfig, textEdit)
        assertTrue(updated.contains("// package for mixins"))
        assertTrue(updated.contains("\"ExampleMixin\","))
        assertTrue(updated.contains("UnlistedMixin"))
    }

    @Test
    fun codeActionScopesMixinConfigToFabricSourceSetWhenEditingFabricJava() {
        val handler = createHandlerWithFabricForgeMixinConfigs()
        val workspaceRoot = JdtlsFixtureSupport.workspaceUri(tempDir)
        val source = """
            package com.example.mixin;
            import com.example.target.SimpleTarget;
            import org.spongepowered.asm.mixin.Mixin;
            @Mixin(SimpleTarget.class)
            public abstract class UnlistedMixin {}
        """.trimIndent()
        val response = handler.handle(
            listOf(
                codeActionPayload(
                    source = source,
                    diagnosticCodes = listOf(MixinDiagnosticCodes.MIXIN_CLASS_NOT_LISTED_IN_CONFIG),
                    documentUri = "$workspaceRoot/src/fabric/java/com/example/mixin/UnlistedMixin.java",
                ),
            ),
        )
        val action = assertIs<McdevCodeActionResponse>(response.result)
            .actions
            .first { it.kind == "quickfix.mixin.config" }
        val editUri = action.edits.single().documentUri
        assertTrue(editUri.contains("fabric.mixins.json"))
        assertTrue(!editUri.contains("forge.mixins.json"))
    }

    @Test
    fun codeActionUsesConfigThatListsMixinClass() {
        val handler = createHandlerWithListedMixinConfig()
        val source = """
            package com.example.mixin;
            import com.example.target.SimpleTarget;
            import org.spongepowered.asm.mixin.Mixin;
            @Mixin(SimpleTarget.class)
            public abstract class ListedMixin {}
        """.trimIndent()
        val response = handler.handle(
            listOf(
                codeActionPayload(
                    source = source,
                    fileName = "ListedMixin.java",
                    diagnosticCodes = listOf(MixinDiagnosticCodes.MIXIN_CLASS_NOT_LISTED_IN_CONFIG),
                ),
            ),
        )
        val result = assertIs<McdevCodeActionResponse>(response.result)
        assertTrue(result.actions.isEmpty())
    }

    private fun createHandlerWithTwoMixinConfigs(): McdevCodeActionHandler {
        val handler = createHandler()
        val resources = tempDir.resolve("src/main/resources").createDirectories()
        resources.resolve("aaa-wrong.mixins.json").writeText(
            """
            {
              "package": "com.other.mixin",
              "mixins": ["OtherMixin"]
            }
            """.trimIndent(),
        )
        return handler
    }

    private fun createHandlerWithListedMixinConfig(): McdevCodeActionHandler {
        val handler = createHandler()
        val resources = tempDir.resolve("src/main/resources").createDirectories()
        resources.resolve("aaa-wrong.mixins.json").writeText(
            """
            {
              "package": "com.other.mixin",
              "mixins": ["OtherMixin"]
            }
            """.trimIndent(),
        )
        resources.resolve("zzz-listed.mixins.json").writeText(
            """
            {
              "package": "com.example.mixin",
              "mixins": ["ListedMixin"]
            }
            """.trimIndent(),
        )
        return handler
    }

    private fun createHandlerWithFabricForgeMixinConfigs(): McdevCodeActionHandler {
        JdtlsFixtureSupport.copyFixtureResource(
            FixturePaths.FABRIC_BASIC_BUILD_GRADLE,
            tempDir.resolve("build.gradle"),
        )
        JdtlsFixtureSupport.copyFixtureResource(
            FixturePaths.FABRIC_BASIC_MAPPINGS,
            tempDir.resolve("mappings.tiny"),
        )
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        tempDir.resolve("src/fabric/java/com/example/mixin").createDirectories()
        tempDir.resolve("src/fabric/resources").createDirectories()
        tempDir.resolve("src/forge/resources").createDirectories()
        tempDir.resolve("src/fabric/resources/fabric.mixins.json").writeText(
            """
            {
              "package": "com.example.mixin",
              "mixins": ["ExampleMixin"]
            }
            """.trimIndent(),
        )
        tempDir.resolve("src/forge/resources/forge.mixins.json").writeText(
            """
            {
              "package": "com.example.mixin",
              "mixins": ["ExampleMixin"]
            }
            """.trimIndent(),
        )
        return McdevCodeActionHandler(projectService = FileBasedProjectContextService())
    }

    private fun createHandlerWithJson5MixinConfig(): McdevCodeActionHandler {
        val handler = createHandler()
        tempDir.resolve("mixins.json").writeText(
            """
            {
              // package for mixins
              "package": "com.example.mixin",
              "mixins": [
                "ExampleMixin",
              ],
            }
            """.trimIndent(),
        )
        return handler
    }

    private fun createHandler(): McdevCodeActionHandler {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_BASIC, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        return McdevCodeActionHandler(projectService = FileBasedProjectContextService())
    }

    private fun applyTextEdit(source: String, edit: McdevTextEdit): String {
        val start = positionToOffset(source, edit.range.start)
        val end = positionToOffset(source, edit.range.end)
        return buildString {
            append(source.substring(0, start))
            append(edit.newText)
            append(source.substring(end))
        }
    }

    private fun positionToOffset(source: String, position: McdevPosition): Int {
        var offset = 0
        repeat(position.line) {
            offset = source.indexOf('\n', offset).let { newline -> if (newline < 0) source.length else newline + 1 }
        }
        return (offset + position.character).coerceAtMost(source.length)
    }

    private fun codeActionPayload(
        source: String,
        diagnosticCodes: List<String> = emptyList(),
        workspaceRoot: String = JdtlsFixtureSupport.workspaceUri(tempDir),
        fileName: String = "UnlistedMixin.java",
        documentUri: String? = null,
        positionLine: Int = 0,
        positionCharacter: Int = 0,
    ): Map<String, Any?> = mapOf(
        "context" to mapOf(
            "protocolVersion" to McdevProtocol.VERSION,
            "workspaceRoot" to workspaceRoot,
            "documentUri" to (documentUri ?: "$workspaceRoot/src/main/java/com/example/mixin/$fileName"),
            "languageId" to "java",
            "position" to mapOf("line" to positionLine, "character" to positionCharacter),
            "bufferText" to source,
            "client" to mapOf("name" to "mcdev.nvim", "version" to "0.1.0"),
        ),
        "range" to mapOf(
            "start" to mapOf("line" to positionLine, "character" to positionCharacter),
            "end" to mapOf("line" to positionLine, "character" to positionCharacter),
        ),
        "diagnosticCodes" to diagnosticCodes,
    )
}
