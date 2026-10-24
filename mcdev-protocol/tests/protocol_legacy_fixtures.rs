//! Audit trace and exact legacy completion fixtures. This table is not a substitute for the executable tests it names.
#![allow(dead_code)]
pub struct LegacyCase {
    pub legacy_id: &'static str,
    pub rust_test: &'static str,
    pub original_case: &'static str,
}
pub const LEGACY_CASES:&[LegacyCase]=&[
LegacyCase{legacy_id:"mcdev-protocol/src/test/kotlin/io/github/mcdev/protocol/McdevProtocolTest.kt::protocolMismatchUsesStructuredError",rust_test:"protocol_mismatch_uses_structured_error",original_case:r#"val response = protocolMismatch(99)
        assertEquals(McdevErrorCode.PROTOCOL_MISMATCH, response.error?.code)
        assertEquals("99", response.error?.details?.get("client"))
        assertEquals(McdevProtocol.VERSION.toString(), response.error?.details?.get("server"))
    }"#},
LegacyCase{legacy_id:"mcdev-protocol/src/test/kotlin/io/github/mcdev/protocol/McdevProtocolTest.kt::projectContextObservabilityCommandsUseStableIds",rust_test:"stable_command_ids_cover_all_eleven_commands",original_case:r#"assertEquals("mcdev.reloadProjectContext", McdevCommands.RELOAD_PROJECT_CONTEXT)
        assertEquals("mcdev.dumpContext", McdevCommands.DUMP_CONTEXT)
        assertEquals("mcdev.hover", McdevCommands.HOVER)
        assertEquals("mcdev.diagnostics", McdevCommands.DIAGNOSTICS)
    }"#},
LegacyCase{legacy_id:"mcdev-protocol/src/test/kotlin/io/github/mcdev/protocol/McdevProtocolTest.kt::completionItemPreservesDisplayAndInsertionSeparately",rust_test:"completion_item_preserves_display_and_insertion",original_case:r#"val item = McdevCompletionItemDto(
            label = "setScreen(Screen): void",
            detail = "MinecraftClient",
            documentation = null,
            filterText = "setScreen MinecraftClient Screen",
            insertText = "m_91152_(Lnet/minecraft/client/gui/screens/Screen;)V",
            kind = "method",
            sortKey = "0200_setScreen",
            edit = null,
        )
        assertEquals("setScreen(Screen): void", item.label)
        assertEquals("m_91152_(Lnet/minecraft/client/gui/screens/Screen;)V", item.insertText)
        assertNotNull(item.filterText)
    }
}"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::decodesCompletionRequestFromMapPayload",rust_test:"decodes_completion_request_from_map_payload",original_case:r#"val request = decoder.decodeCompletionRequest(listOf(completionPayload()))
        assertEquals("manual", request.trigger.kind)
        assertEquals("import", request.options.mixinClassInsert)
        assertEquals("file:///tmp/project", request.context.workspaceRoot)
        assertEquals("java", request.context.languageId)
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::decodesCompletionBufferTextFallback",rust_test:"decodes_buffer_fallback_and_changedtick_without_overriding_direct_text",original_case:r#"val context = contextPayload().toMutableMap()
        context.remove("bufferText")
        context["bufferTextFallback"] = "@Mixin(Fallback"
        val request = decoder.decodeCompletionRequest(listOf(context + completionOptionsPayload()))
        assertEquals("@Mixin(Fallback", request.context.bufferText)
        assertEquals(true, request.context.bufferTextFallbackUsed)
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::decodesFlatContextPayload",rust_test:"decodes_flat_context_payload",original_case:r#"val context = decoder.decodeContextRequest(listOf(contextPayload()))
        assertEquals(12, context.position.line)
        assertEquals(4, context.position.character)
        assertEquals("mcdev.nvim", context.client.name)
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::decodesNestedContextPayload",rust_test:"decodes_nested_context_payload",original_case:r#"val request = decoder.decodeInfoRequest(listOf(mapOf("context" to contextPayload())))
        assertEquals("0.1.0", request.context.client.version)
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::decodeRoundtripPreservesCompletionRequest",rust_test:"decode_roundtrip_preserves_completion_request",original_case:r#"val original = decoder.decodeCompletionRequest(listOf(completionPayload()))
        val encoded = decoder.encodeToMap(original)
        val restored = decoder.decodeFromMap(encoded, McdevCompletionRequest::class.java)
        assertEquals(original, restored)
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::decodeRoundtripPreservesRequestContext",rust_test:"decode_roundtrip_preserves_request_context",original_case:r#"val original = decoder.decodeContextRequest(listOf(contextPayload()))
        val encoded = decoder.encodeToMap(original)
        val restored = decoder.decodeFromMap(encoded, McdevRequestContext::class.java)
        assertEquals(original, restored)
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::missingPayloadThrowsParseError",rust_test:"missing_payload_reports_parse_error",original_case:r#"assertFailsWith<ProtocolDecodeException> {
            decoder.decodeCompletionRequest(emptyList())
        }
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::missingWorkspaceRootThrowsParseError",rust_test:"missing_workspace_root_reports_parse_error",original_case:r#"val payload = contextPayload().toMutableMap()
        payload.remove("workspaceRoot")
        assertFailsWith<ProtocolDecodeException> {
            decoder.decodeContextRequest(listOf(payload))
        }
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::missingPositionThrowsParseError",rust_test:"missing_position_reports_parse_error",original_case:r#"val payload = contextPayload().toMutableMap()
        payload.remove("position")
        assertFailsWith<ProtocolDecodeException> {
            decoder.decodeContextRequest(listOf(payload))
        }
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::defaultsProtocolVersionWhenMissing",rust_test:"missing_optional_context_fields_use_legacy_defaults",original_case:r#"val payload = contextPayload().toMutableMap()
        payload.remove("protocolVersion")
        val context = decoder.decodeContextRequest(listOf(payload))
        assertEquals(McdevProtocol.VERSION, context.protocolVersion)
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::decodeDiagnosticsRequestUsesContext",rust_test:"diagnostics_request_uses_context",original_case:r#"val request = decoder.decodeDiagnosticsRequest(listOf(mapOf("context" to contextPayload())))
        assertNotNull(request.context.bufferText)
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::decodesCodeActionRequestWithDiagnosticCodes",rust_test:"code_action_request_decodes_diagnostic_codes_and_ranges",original_case:r#"val request = decoder.decodeCodeActionRequest(listOf(codeActionPayload()))
        assertEquals(1, request.diagnosticCodes.size)
        assertEquals("MIXIN_CLASS_NOT_LISTED_IN_CONFIG", request.diagnosticCodes.first())
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::decodeRoundtripPreservesCodeActionRequest",rust_test:"decode_roundtrip_preserves_code_action_request",original_case:r#"val original = decoder.decodeCodeActionRequest(listOf(codeActionPayload()))
        val encoded = decoder.encodeToMap(original)
        val restored = decoder.decodeFromMap(encoded, McdevCodeActionRequest::class.java)
        assertEquals(original, restored)
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::decodeRoundtripPreservesDefinitionRequest",rust_test:"decode_roundtrip_preserves_definition_request",original_case:r#"val original = decoder.decodeDefinitionRequest(listOf(mapOf("context" to contextPayload())))
        val encoded = decoder.encodeToMap(original)
        val restored = decoder.decodeFromMap(encoded, McdevDefinitionRequest::class.java)
        assertEquals(original, restored)
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::encodeDefinitionResponseUsesLowercaseResolutionWireValues",rust_test:"definition_response_envelope_encodes_lowercase_resolution",original_case:r#"val target = McDefinitionTarget(
            kind = MemberKind.CLASS,
            ownerInternalName = "com/example/target/SimpleTarget",
            ownerFqn = "com.example.target.SimpleTarget",
        )
        val envelope = McdevResponseEnvelope(
            capabilities = setOf("definition"),
            result = McdevDefinitionResponse(
                locations = listOf(
                    DefinitionConverter.toLocation(
                        ResolvedDefinition(
                            target = target,
                            documentUri = "file:///mapped-sources/SimpleTarget.java",
                            range = McTextRange(McTextPosition(3, 0), McTextPosition(3, 20)),
                            resolution = McdevDefinitionResolution.SOURCE,
                        ),
                    ),
                ),
            ),
        )
        val encoded = decoder.encodeToMap(envelope)
        @Suppress("UNCHECKED_CAST")
        val locations = ((encoded["result"] as Map<String, Any?>)["locations"] as List<Map<String, Any?>>)
        assertEquals("source", locations.single()["resolution"])
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::decodeRoundtripPreservesReferencesRequest",rust_test:"decode_roundtrip_preserves_references_request",original_case:r#"val original = decoder.decodeReferencesRequest(listOf(mapOf("context" to contextPayload())))
        val encoded = decoder.encodeToMap(original)
        val restored = decoder.decodeFromMap(encoded, McdevReferencesRequest::class.java)
        assertEquals(original, restored)
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::completionConverterPreservesLabelAndInsertText",rust_test:"completion_preserves_label_insert_and_kind",original_case:r#"val item = McCompletionItem(
            label = "SimpleTarget",
            detail = "com.example.target",
            documentation = "com/example/target/SimpleTarget",
            filterText = "SimpleTarget com.example.target",
            insertText = "SimpleTarget.class",
            kind = McCompletionKind.CLASS,
            sortKey = "0100_SimpleTarget",
            metadata = McCompletionMetadata(source = "mixin.target", owner = "com/example/target/SimpleTarget"),
        )
        val dto = CompletionItemConverter.toDto(item, annotationContext("@Mixin(Simple)"), "@Mixin(Simple)")
        assertEquals("SimpleTarget", dto.label)
        assertEquals("SimpleTarget.class", dto.insertText)
        assertEquals("class", dto.kind)
        assertEquals("mixin.target", dto.metadata["source"])
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::completionConverterMapsItemAdditionalEdits",rust_test:"completion_preserves_additional_edits",original_case:r#"val source = "@Mixin(Simple)"
        val item = McCompletionItem(
            label = "SimpleTarget",
            detail = null,
            documentation = null,
            filterText = "SimpleTarget",
            insertText = "SimpleTarget.class",
            kind = McCompletionKind.CLASS,
            sortKey = "0100",
            metadata = McCompletionMetadata(source = "mixin.target"),
            additionalEdits = listOf(McTextEdit(0, 0, "import com.example.target.SimpleTarget;\n")),
        )
        val dto = CompletionItemConverter.toDto(item, annotationContext(source), source)
        assertEquals(1, dto.additionalEdits.size)
        assertEquals("import com.example.target.SimpleTarget;\n", dto.additionalEdits.first().newText)
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::completionConverterAddsImportAdditionalEditForImportModeMixinTarget",rust_test:"completion_adds_import_for_import_mode_mixin_target",original_case:r#"val source = """
            package com.example.mixin;

            import org.spongepowered.asm.mixin.Mixin;

            @Mixin(Simple
        """.trimIndent()
        val item = McCompletionItem(
            label = "SimpleTarget",
            detail = "com.example.target",
            documentation = "com/example/target/SimpleTarget",
            filterText = "SimpleTarget",
            insertText = "SimpleTarget.class",
            kind = McCompletionKind.CLASS,
            sortKey = "0100",
            metadata = McCompletionMetadata(
                source = "mixin.target",
                owner = "com/example/target/SimpleTarget",
            ),
        )
        val dto = CompletionItemConverter.toDto(
            item = item,
            annotationContext = annotationContext(source, valueStart = source.indexOf("Simple"), valueEnd = source.length),
            source = source,
            convertContext = CompletionConvertContext(
                source = source,
                annotationContext = annotationContext(source, valueStart = source.indexOf("Simple"), valueEnd = source.length),
                classInsertMode = MixinClassInsertMode.IMPORT,
            ),
        )
        assertEquals(1, dto.additionalEdits.size)
        assertTrue(dto.additionalEdits.first().newText.contains("import com.example.target.SimpleTarget;"))
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::kotlinMixinTargetImportsDoNotUseJavaSyntaxOrOverwriteAliasBindings",rust_test:"kotlin_imports_preserve_aliases_and_conflicting_bindings",original_case:r#"val base = "package example\n\nimport io.github.diskria.lapis.annotations.KMixin\n"
        val item = McCompletionItem(
            label = "SimpleTarget", detail = null, documentation = null, filterText = "SimpleTarget",
            insertText = "SimpleTarget::class", kind = McCompletionKind.CLASS, sortKey = "0100",
            metadata = McCompletionMetadata(source = "mixin.target", owner = "com/example/target/SimpleTarget"),
        )
        val source = "$base\n@KMixin(Simple)\nclass Patch"
        val context = annotationContext(source, source.indexOf("Simple"), source.indexOf("Simple") + 6).copy(isKotlin = true)
        val dto = CompletionItemConverter.toDto(item, context, source)
        assertEquals("SimpleTarget::class", dto.edit?.newText)
        assertEquals(listOf("import com.example.target.SimpleTarget\n"), dto.additionalEdits.map { it.newText })
        for ((extraImport, reference) in listOf(
            "import com.example.target.SimpleTarget as Target" to "Target::class",
            "import other.SimpleTarget" to "com.example.target.SimpleTarget::class",
        )) {
            val boundSource = "$base$extraImport\n@KMixin(Simple)\nclass Patch"
            val boundContext = annotationContext(boundSource, boundSource.lastIndexOf("Simple)"),
                boundSource.lastIndexOf("Simple)") + 6).copy(isKotlin = true)
            val boundDto = CompletionItemConverter.toDto(item.copy(insertText = reference), boundContext, boundSource)
            assertEquals(reference, boundDto.edit?.newText)
            assertTrue(boundDto.additionalEdits.isEmpty())
        }
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::completionConverterRemapsAtTargetInsertTextWithMappings",rust_test:"at_target_converter_remaps_descriptor_and_smart_modes",original_case:r#"val mappingsText = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_MAPPINGS)
        val resolver = assertIs<MappingParseResult.Success>(TinyV2Parser.parse(mappingsText)).mappings.asResolver()
        val item = McCompletionItem(
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
        val dto = CompletionItemConverter.toDto(
            item = item,
            annotationContext = null,
            source = "",
            convertContext = CompletionConvertContext(
                source = "",
                annotationContext = null,
                preferredAtTarget = "descriptor",
                mappingResolver = resolver,
            ),
        )
        assertEquals(
            "Lcom/example/target/class_1;method_1(Ljava/lang/String;FF)V",
            dto.insertText,
        )
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::completionConverterUsesFullAtTargetInsertTextWhenSmartTargetIsUnambiguous",rust_test:"at_target_converter_remaps_descriptor_and_smart_modes",original_case:r#"val mappingsText = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_MAPPINGS)
        val resolver = assertIs<MappingParseResult.Success>(TinyV2Parser.parse(mappingsText)).mappings.asResolver()
        val item = McCompletionItem(
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
        val dto = CompletionItemConverter.toDtos(
            items = listOf(item),
            annotationContext = null,
            source = "",
            convertContext = CompletionConvertContext(
                source = "",
                annotationContext = null,
                preferredAtTarget = "smart",
                mappingResolver = resolver,
            ),
        ).single()
        assertEquals(
            "Lcom/example/target/class_1;method_1(Ljava/lang/String;FF)V",
            dto.insertText,
        )
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::completionConverterKeepsFullDescriptorsForAmbiguousSmartTargets",rust_test:"smart_targets_keep_full_descriptors_when_ambiguous",original_case:r#"val mappingsText = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_MAPPINGS)
        val resolver = assertIs<MappingParseResult.Success>(TinyV2Parser.parse(mappingsText)).mappings.asResolver()
        val first = McCompletionItem(
            label = "draw(String, float, float): void",
            detail = "SimpleTarget",
            documentation = null,
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
        val second = first.copy(
            label = "draw(int): void",
            insertText = "Lcom/example/target/SimpleTarget;draw(I)V",
            metadata = first.metadata.copy(descriptor = "(I)V"),
        )

        val dtos = CompletionItemConverter.toDtos(
            items = listOf(first, second),
            annotationContext = null,
            source = "",
            convertContext = CompletionConvertContext(
                source = "",
                annotationContext = null,
                preferredAtTarget = "smart",
                mappingResolver = resolver,
            ),
        )

        assertEquals(
            listOf(
                "Lcom/example/target/class_1;method_1(Ljava/lang/String;FF)V",
                "Lcom/example/target/SimpleTarget;draw(I)V",
            ),
            dtos.map { it.insertText },
        )
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::completionConverterTextEditKeepsPretypedOwnerAndProducesValidTarget",rust_test:"explicit_target_range_replaces_owner_and_member_as_one_valid_target",original_case:r#"val mappingsText = FixtureResourceLoader.loadText(FixturePaths.FABRIC_BASIC_MAPPINGS)
        val resolver = assertIs<MappingParseResult.Success>(TinyV2Parser.parse(mappingsText)).mappings.asResolver()
        val source = "@At(value = \"INVOKE\", target = \"Lcom/example/target/SimpleTarget;dra\")"
        val targetStart = source.indexOf("Lcom/example/target/SimpleTarget;")
        val targetEnd = source.lastIndexOf('"')
        val item = McCompletionItem(
            label = "draw(String, float, float): void",
            detail = "SimpleTarget",
            documentation = null,
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
        val context = CompletionConvertContext(
            source = source,
            annotationContext = null,
            preferredAtTarget = "smart",
            mappingResolver = resolver,
            replacementRange = CompletionReplacementRange(targetStart, targetEnd),
        )

        val edit = CompletionItemConverter.toDto(item, null, source, context).edit
        val editedSource = source.substring(0, targetStart) + edit!!.newText + source.substring(targetEnd)
        val editedTarget = editedSource.substringAfter("target = \"").substringBefore('"')

        assertEquals("Lcom/example/target/class_1;method_1(Ljava/lang/String;FF)V", editedTarget)
        assertIs<DescriptorParseResult.Success<*>>(MemberTargetParser.parse(editedTarget))
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::completionConverterBuildsTextEditFromAnnotationOffsets",rust_test:"legacy_completion_annotation_offsets",original_case:r#"val source = "@Mixin(Simple)"
        val context = annotationContext(source, valueStart = 7, valueEnd = 13)
        val item = McCompletionItem(
            label = "SimpleTarget",
            detail = null,
            documentation = null,
            filterText = "SimpleTarget",
            insertText = "SimpleTarget.class",
            kind = McCompletionKind.CLASS,
            sortKey = "0100",
            metadata = McCompletionMetadata(source = "mixin.target"),
        )
        val dto = CompletionItemConverter.toDto(item, context, source)
        assertNotNull(dto.edit)
        assertEquals("SimpleTarget.class", dto.edit?.newText)
        assertEquals(0, dto.edit?.range?.start?.line)
        assertEquals(7, dto.edit?.range?.start?.character)
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::completionConverterUsesExplicitPartialAnnotationRangeAndPreservesImports",rust_test:"explicit_partial_annotation_range_preserves_import_and_snippet",original_case:r#"val source = "package demo;\n\n@Mix"
        val item = McCompletionItem(
            label = "Mixin",
            detail = "Mixin annotation",
            documentation = null,
            filterText = "mixin Mixin",
            insertText = "Mixin(${ '$' }{1:Target}.class)${ '$' }0",
            kind = McCompletionKind.KEYWORD,
            sortKey = "0311_mixin",
            metadata = McCompletionMetadata(
                source = "mixin.annotation",
                owner = "org.spongepowered.asm.mixin.Mixin",
            ),
            additionalEdits = listOf(
                McTextEdit(
                    startOffset = source.indexOf("package") + "package demo;".length,
                    endOffset = source.indexOf("package") + "package demo;".length,
                    newText = "\nimport org.spongepowered.asm.mixin.Mixin;",
                ),
            ),
        )
        val replacementStart = source.indexOf("Mix")
        val dto = CompletionItemConverter.toDto(
            item = item,
            annotationContext = null,
            source = source,
            convertContext = CompletionConvertContext(
                source = source,
                annotationContext = null,
                replacementRange = CompletionReplacementRange(replacementStart, source.length),
            ),
        )

        assertEquals("Mixin(${ '$' }{1:Target}.class)${ '$' }0", dto.edit?.newText)
        assertEquals(2, dto.edit?.range?.start?.line)
        assertEquals(1, dto.edit?.range?.start?.character)
        assertEquals(4, dto.edit?.range?.end?.character)
        assertEquals(1, dto.additionalEdits.size)
        assertTrue(dto.additionalEdits.single().newText.contains("import org.spongepowered.asm.mixin.Mixin;"))
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::completionConverterKeepsInjectMethodOpenQuoteOutsideTextEdit",rust_test:"inject_method_open_quote_stays_outside_replacement",original_case:r#"val marker = "/*caret*/"
        val sourceWithMarker = """
            package com.example.mixin;

            @Mixin(net.minecraft.world.item.Item.class)
            class ItemMixin {
                @Inject(method = "isF$marker", at = @At("HEAD"))
                private void injected(CallbackInfoReturnable<Boolean> cir) {}
            }
        """.trimIndent()
        val markerOffset = sourceWithMarker.indexOf(marker)
        val source = sourceWithMarker.replace(marker, "")
        val beforeMarker = sourceWithMarker.substring(0, markerOffset)
        val line = beforeMarker.count { it == '\n' }
        val character = beforeMarker.substringAfterLast('\n').length
        val model = MixinSemanticModelParser.parse(source, "file:///ItemMixin.java")
        val completionContext = SemanticCompletionContextExtractor.extract(source, line, character, model)
        val annotationContext = SemanticCompletionContextExtractor.toAnnotationContext(
            source = source,
            line = line,
            character = character,
            model = model,
            context = completionContext!!,
        )!!
        val item = McCompletionItem(
            label = "isFoil(): boolean",
            detail = "Item",
            documentation = null,
            filterText = "isFoil Item boolean",
            insertText = "isFoil",
            kind = McCompletionKind.METHOD,
            sortKey = "0200_isFoil",
            metadata = McCompletionMetadata(source = "mixin.injectMethod"),
        )

        val dto = CompletionItemConverter.toDto(item, annotationContext, source)

        assertNotNull(dto.edit)
        assertEquals("isFoil", dto.edit?.newText)
        assertEquals(source.indexOf("isF") - source.lastIndexOf('\n', source.indexOf("isF")) - 1, dto.edit?.range?.start?.character)
        assertEquals(source.indexOf("isF") + "isF".length - source.lastIndexOf('\n', source.indexOf("isF")) - 1, dto.edit?.range?.end?.character)
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::diagnosticConverterMapsSeverityAndRange",rust_test:"diagnostics_convert_all_severities_metadata_and_ranges",original_case:r#"val diagnostic = McDiagnostic(
            code = "UNRESOLVED_MIXIN_TARGET",
            severity = McSeverity.ERROR,
            message = "Unresolved @Mixin target",
            range = McTextRange(
                start = McTextPosition(1, 2),
                end = McTextPosition(1, 10),
            ),
            metadata = mapOf("target" to "Missing"),
        )
        val dto = DiagnosticConverter.toDto(diagnostic)
        assertEquals("error", dto.severity)
        assertEquals("UNRESOLVED_MIXIN_TARGET", dto.code)
        assertEquals("Missing", dto.metadata["target"])
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::diagnosticConverterMapsWarningSeverity",rust_test:"diagnostics_convert_all_severities_metadata_and_ranges",original_case:r#"val dto = DiagnosticConverter.toDto(
            McDiagnostic(
                code = "MIXIN_CLASS_NOT_LISTED_IN_CONFIG",
                severity = McSeverity.WARNING,
                message = "not listed",
                range = McTextRange(McTextPosition(0, 0), McTextPosition(0, 0)),
            ),
        )
        assertEquals("warning", dto.severity)
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::codeActionConverterMapsMixinConfigFix",rust_test:"symbolic_fix_uses_resolved_document_content_and_original_metadata",original_case:r#"val fix = AddMixinConfigEntryFix(
            title = "Add 'ExampleMixin' to mixin config",
            configPath = "mixins.json",
            mixinClassName = "ExampleMixin",
            mixinPackage = "com.example.mixin",
        )
        val dto = CodeActionConverter.toDto(
            fix = fix,
            source = "@Mixin(Simple.class) class ExampleMixin {}",
            mixinConfigContent = """{ "mixins": [] }""",
        )
        assertNotNull(dto)
        assertEquals("quickfix.mixin.config", dto?.kind)
        assertTrue(dto!!.edits.first().edits.first().newText.contains("ExampleMixin"))
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::codeActionConverterMapsAccessorGenerateFix",rust_test:"code_action_maps_accessor_and_invoker_generation_without_domain_callback",original_case:r#"val source = "@Mixin(MinecraftClient.class) class M { }"
        val openBrace = source.indexOf('{') + 1
        val fix = GenerateAccessorMethodFix(
            title = "Generate @Accessor getter for 'currentScreen'",
            documentUri = "file:///Mixin.java",
            insertOffset = openBrace,
            methodSource = "    @Accessor(\"currentScreen\")\n    Screen getCurrentScreen();\n",
            fieldName = "currentScreen",
            isGetter = true,
        )
        val dto = CodeActionConverter.toDto(fix, source, mixinConfigContent = null)
        assertNotNull(dto)
        assertEquals("quickfix.mixin.generateAccessor", dto?.kind)
        assertTrue(dto!!.edits.first().edits.first().newText.contains("currentScreen"))
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::codeActionConverterMapsRemoveDuplicateAccessWidenerFix",rust_test:"code_action_maps_remove_duplicate_access_widener_through_domain",original_case:r#"val source = """
            accessWidener v2 named
            accessible class com/example/target/SimpleTarget
            accessible class com/example/target/SimpleTarget
        """.trimIndent()
        val fix = RemoveDuplicateAccessWidenerEntryFix(
            title = "Remove duplicate access widener entry",
            documentUri = "file:///mod.accesswidener",
            lineNumber = 3,
        )
        val dto = CodeActionConverter.toDto(fix, source, mixinConfigContent = null)
        assertNotNull(dto)
        assertEquals("quickfix.aw.removeDuplicate", dto?.kind)
        assertEquals(1, dto!!.edits.single().edits.size)
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::codeActionConverterMapsAddAtMethodDescriptorFix",rust_test:"code_action_maps_at_method_descriptor_through_domain",original_case:r#"val source = "public com.example.target.SimpleTarget draw"
        val fix = AddAtMethodDescriptorFix(
            title = "Add method descriptor",
            documentUri = "file:///mod_at.cfg",
            line = 1,
            memberName = "draw",
            descriptor = "(Ljava/lang/String;FF)V",
        )
        val dto = CodeActionConverter.toDto(fix, source, mixinConfigContent = null)
        assertNotNull(dto)
        assertEquals("quickfix.at.addDescriptor", dto?.kind)
        assertTrue(dto!!.edits.first().edits.first().newText.contains("draw(Ljava/lang/String;FF)V"))
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::codeActionConverterMapsMixinExtrasFixHandlerSignature",rust_test:"code_action_maps_mixinextras_workspace_fix_and_preserves_metadata",original_case:r#"val source = """
            @WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
            private void mcdevWrapLength(String instance, Operation<Integer> original) {
                original.call(instance);
            }
        """.trimIndent()
        val openBrace = source.indexOf('{') + 1
        val fix = io.github.mcdev.core.codeaction.WorkspaceEditFix(
            title = "Fix WrapOperation handler signature",
            kind = "quickfix.mixinextras.fixHandlerSignature",
            documentUri = "file:///BadMixinExtras.java",
            edits = listOf(
                McTextEdit(
                    openBrace,
                    openBrace,
                    "\n    private int mcdevWrapLength(String instance, Operation<Integer> original) {\n        return original.call(instance);\n    }\n",
                ),
            ),
        )
        val dto = CodeActionConverter.toDto(fix, source, mixinConfigContent = null)
        assertNotNull(dto)
        assertEquals("quickfix.mixinextras.fixHandlerSignature", dto?.kind)
        assertTrue(dto!!.edits.first().edits.first().newText.contains("Operation<Integer>"))
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::codeActionConverterMapsInvokerGenerateFix",rust_test:"code_action_maps_accessor_and_invoker_generation_without_domain_callback",original_case:r#"val source = "@Mixin(MinecraftClient.class) class M { }"
        val openBrace = source.indexOf('{') + 1
        val fix = GenerateInvokerMethodFix(
            title = "Generate @Invoker for 'setScreen'",
            documentUri = "file:///Mixin.java",
            insertOffset = openBrace,
            methodSource = "    @Invoker(\"setScreen\")\n    void invokeSetScreen(Screen screen);\n",
            targetMethodName = "setScreen",
        )
        val dto = CodeActionConverter.toDto(fix, source, mixinConfigContent = null)
        assertNotNull(dto)
        assertEquals("quickfix.mixin.generateInvoker", dto?.kind)
        assertTrue(dto!!.edits.first().edits.first().newText.contains("setScreen"))
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::definitionConverterMapsClassTargetMetadata",rust_test:"definition_converter_preserves_class_and_field_metadata",original_case:r#"val target = McDefinitionTarget(
            kind = MemberKind.CLASS,
            ownerInternalName = "com/example/target/SimpleTarget",
            ownerFqn = "com.example.target.SimpleTarget",
        )
        val location = DefinitionConverter.toLocation(
            io.github.mcdev.jdtls.definition.ResolvedDefinition(
                target = target,
                documentUri = "file:///project/SimpleTarget.java",
                range = McTextRange(McTextPosition(2, 0), McTextPosition(2, 20)),
                resolution = io.github.mcdev.protocol.McdevDefinitionResolution.SOURCE,
            ),
        )
        assertEquals("class", location.metadata["kind"])
        assertEquals("com/example/target/SimpleTarget", location.metadata["owner"])
        assertEquals("com.example.target.SimpleTarget", location.metadata["fqn"])
        assertEquals(io.github.mcdev.protocol.McdevDefinitionResolution.SOURCE, location.resolution)
        assertEquals("file:///project/SimpleTarget.java", location.documentUri)
    }"#},
LegacyCase{legacy_id:"mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::definitionConverterMapsFieldTargetMetadata",rust_test:"definition_converter_preserves_class_and_field_metadata",original_case:r#"val target = McDefinitionTarget(
            kind = MemberKind.FIELD,
            ownerInternalName = "com/example/target/SimpleTarget",
            ownerFqn = "com.example.target.SimpleTarget",
            name = "counter",
            descriptor = "I",
        )
        val location = DefinitionConverter.toLocation(
            io.github.mcdev.jdtls.definition.ResolvedDefinition(
                target = target,
                documentUri = "file:///project/SimpleTarget.java",
                range = McTextRange(McTextPosition(3, 16), McTextPosition(3, 23)),
                resolution = io.github.mcdev.protocol.McdevDefinitionResolution.SOURCE,
            ),
        )
        assertEquals("field", location.metadata["kind"])
        assertEquals("counter", location.metadata["name"])
        assertEquals("I", location.metadata["descriptor"])
        assertEquals(io.github.mcdev.protocol.McdevDefinitionResolution.SOURCE, location.resolution)
    }"#},
];
pub fn completion_item(case: &str) -> mcdev_core::McCompletionItem {
    let json = match case {
        "completionConverterPreservesLabelAndInsertText" => {
            r#"{"label": "SimpleTarget", "detail": "com.example.target", "documentation": "com/example/target/SimpleTarget", "filterText": "SimpleTarget com.example.target", "insertText": "SimpleTarget.class", "kind": "CLASS", "sortKey": "0100_SimpleTarget", "metadata": {"source": "mixin.target", "owner": "com/example/target/SimpleTarget"}}"#
        }
        "completionConverterMapsItemAdditionalEdits" => {
            r#"{"label": "SimpleTarget", "detail": null, "documentation": null, "filterText": "SimpleTarget", "insertText": "SimpleTarget.class", "kind": "CLASS", "sortKey": "0100", "metadata": {"source": "mixin.target"}}"#
        }
        "completionConverterAddsImportAdditionalEditForImportModeMixinTarget" => {
            r#"{"label": "SimpleTarget", "detail": "com.example.target", "documentation": "com/example/target/SimpleTarget", "filterText": "SimpleTarget", "insertText": "SimpleTarget.class", "kind": "CLASS", "sortKey": "0100", "metadata": {"source": "mixin.target", "owner": "com/example/target/SimpleTarget"}}"#
        }
        "kotlinMixinTargetImportsDoNotUseJavaSyntaxOrOverwriteAliasBindings" => {
            r#"{"label": "SimpleTarget", "detail": null, "documentation": null, "filterText": "SimpleTarget", "insertText": "SimpleTarget::class", "kind": "CLASS", "sortKey": "0100", "metadata": {"source": "mixin.target", "owner": "com/example/target/SimpleTarget"}}"#
        }
        "completionConverterRemapsAtTargetInsertTextWithMappings" => {
            r#"{"label": "draw(String, float, float): void", "detail": "SimpleTarget", "documentation": "Lcom/example/target/SimpleTarget;draw(Ljava/lang/String;FF)V", "filterText": "draw", "insertText": "Lcom/example/target/SimpleTarget;draw(Ljava/lang/String;FF)V", "kind": "METHOD", "sortKey": "0400_draw", "metadata": {"source": "mixin.atTarget", "owner": "com/example/target/SimpleTarget", "name": "draw", "descriptor": "(Ljava/lang/String;FF)V"}}"#
        }
        "completionConverterUsesFullAtTargetInsertTextWhenSmartTargetIsUnambiguous" => {
            r#"{"label": "draw(String, float, float): void", "detail": "SimpleTarget", "documentation": "Lcom/example/target/SimpleTarget;draw(Ljava/lang/String;FF)V", "filterText": "draw", "insertText": "Lcom/example/target/SimpleTarget;draw(Ljava/lang/String;FF)V", "kind": "METHOD", "sortKey": "0400_draw", "metadata": {"source": "mixin.atTarget", "owner": "com/example/target/SimpleTarget", "name": "draw", "descriptor": "(Ljava/lang/String;FF)V"}}"#
        }
        "completionConverterKeepsFullDescriptorsForAmbiguousSmartTargets" => {
            r#"{"label": "draw(String, float, float): void", "detail": "SimpleTarget", "documentation": null, "filterText": "draw", "insertText": "Lcom/example/target/SimpleTarget;draw(Ljava/lang/String;FF)V", "kind": "METHOD", "sortKey": "0400_draw", "metadata": {"source": "mixin.atTarget", "owner": "com/example/target/SimpleTarget", "name": "draw", "descriptor": "(Ljava/lang/String;FF)V"}}"#
        }
        "completionConverterTextEditKeepsPretypedOwnerAndProducesValidTarget" => {
            r#"{"label": "draw(String, float, float): void", "detail": "SimpleTarget", "documentation": null, "filterText": "draw", "insertText": "Lcom/example/target/SimpleTarget;draw(Ljava/lang/String;FF)V", "kind": "METHOD", "sortKey": "0400_draw", "metadata": {"source": "mixin.atTarget", "owner": "com/example/target/SimpleTarget", "name": "draw", "descriptor": "(Ljava/lang/String;FF)V"}}"#
        }
        "completionConverterBuildsTextEditFromAnnotationOffsets" => {
            r#"{"label": "SimpleTarget", "detail": null, "documentation": null, "filterText": "SimpleTarget", "insertText": "SimpleTarget.class", "kind": "CLASS", "sortKey": "0100", "metadata": {"source": "mixin.target"}}"#
        }
        "completionConverterUsesExplicitPartialAnnotationRangeAndPreservesImports" => {
            r#"{"label": "Mixin", "detail": "Mixin annotation", "documentation": null, "filterText": "mixin Mixin", "insertText": "Mixin(${1:Target}.class)$0", "kind": "KEYWORD", "sortKey": "0311_mixin", "metadata": {"source": "mixin.annotation", "owner": "org.spongepowered.asm.mixin.Mixin"}}"#
        }
        "completionConverterKeepsInjectMethodOpenQuoteOutsideTextEdit" => {
            r#"{"label": "isFoil(): boolean", "detail": "Item", "documentation": null, "filterText": "isFoil Item boolean", "insertText": "isFoil", "kind": "METHOD", "sortKey": "0200_isFoil", "metadata": {"source": "mixin.injectMethod"}}"#
        }
        _ => panic!("no legacy completion fixture for {case}"),
    };
    serde_json::from_str(json).expect("original completion fixture")
}
pub fn source(case: &str) -> &'static str {
    match case {
        "completionConverterMapsItemAdditionalEdits" => r#"@Mixin(Simple)"#,
        "completionConverterAddsImportAdditionalEditForImportModeMixinTarget" => {
            r#"package com.example.mixin;

import org.spongepowered.asm.mixin.Mixin;

@Mixin(Simple"#
        }
        "completionConverterTextEditKeepsPretypedOwnerAndProducesValidTarget" => {
            r#"@At(value = "INVOKE", target = "Lcom/example/target/SimpleTarget;dra")"#
        }
        "completionConverterBuildsTextEditFromAnnotationOffsets" => r#"@Mixin(Simple)"#,
        "completionConverterUsesExplicitPartialAnnotationRangeAndPreservesImports" => {
            r#"package demo;

@Mix"#
        }
        "completionConverterKeepsInjectMethodOpenQuoteOutsideTextEdit" => {
            r#"package com.example.mixin;

@Mixin(net.minecraft.world.item.Item.class)
class ItemMixin {
    @Inject(method = "isF/*caret*/", at = @At("HEAD"))
    private void injected(CallbackInfoReturnable<Boolean> cir) {}
}"#
        }
        "codeActionConverterMapsAccessorGenerateFix" => {
            r#"@Mixin(MinecraftClient.class) class M { }"#
        }
        "codeActionConverterMapsRemoveDuplicateAccessWidenerFix" => {
            r#"accessWidener v2 named
accessible class com/example/target/SimpleTarget
accessible class com/example/target/SimpleTarget"#
        }
        "codeActionConverterMapsAddAtMethodDescriptorFix" => {
            r#"public com.example.target.SimpleTarget draw"#
        }
        "codeActionConverterMapsMixinExtrasFixHandlerSignature" => {
            r#"@WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))
private void mcdevWrapLength(String instance, Operation<Integer> original) {
    original.call(instance);
}"#
        }
        "codeActionConverterMapsInvokerGenerateFix" => {
            r#"@Mixin(MinecraftClient.class) class M { }"#
        }
        _ => panic!("no static legacy source fixture for {case}"),
    }
}
