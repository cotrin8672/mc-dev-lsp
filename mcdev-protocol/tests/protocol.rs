use mcdev_protocol::*;
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn context() -> Value {
    json!({"protocolVersion":1,"workspaceRoot":"file:///tmp/project","documentUri":"file:///tmp/project/src/Mixin.java","languageId":"java","position":{"line":12,"character":4},"bufferText":"@Mixin(Mine","client":{"name":"mcdev.nvim","version":"0.1.0"}})
}
fn completion() -> Value {
    let mut value = context();
    value["trigger"] = json!({"kind":"manual","character":null});
    value["options"] = json!({"preferredAtTarget":"descriptor","mixinClassInsert":"import","injectMethodDescriptor":"auto"});
    value
}
fn roundtrip<T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug + PartialEq>(
    value: T,
) {
    let decoder = ProtocolPayloadDecoder;
    assert_eq!(
        decoder
            .decode_from_map::<T>(decoder.encode_to_map(&value).unwrap())
            .unwrap(),
        value
    );
}

#[test]
#[doc = "mcdev-protocol/src/test/kotlin/io/github/mcdev/protocol/McdevProtocolTest.kt::protocolMismatchUsesStructuredError"]
fn protocol_mismatch_uses_structured_error() {
    let response = protocol_mismatch::<Value>(99);
    let error = response.error.unwrap();
    assert_eq!(error.code, McdevErrorCode::ProtocolMismatch);
    assert_eq!(error.details["client"], "99");
    assert_eq!(error.details["server"], "1");
    assert_eq!(error.message, "protocol mismatch, client=99 server=1");
}
#[test]
#[doc = "mcdev-protocol/src/test/kotlin/io/github/mcdev/protocol/McdevProtocolTest.kt::projectContextObservabilityCommandsUseStableIds"]
fn stable_command_ids_cover_all_eleven_commands() {
    let ids = COMMANDS.map(Command::id);
    assert_eq!(ids.len(), 11);
    assert_eq!(ids.iter().collect::<BTreeSet<_>>().len(), 11);
    for command in COMMANDS {
        assert_eq!(Command::parse(command.id()), Some(command));
        assert_eq!(
            serde_json::from_value::<Command>(json!(command.id())).unwrap(),
            command
        );
    }
    assert_eq!(Command::Context.canonical(), Command::Diagnostics);
    assert_eq!(
        McdevCommands::RELOAD_PROJECT_CONTEXT,
        "mcdev.reloadProjectContext"
    );
    assert_eq!(McdevCommands::DUMP_CONTEXT, "mcdev.dumpContext");
    assert_eq!(McdevCommands::HOVER, "mcdev.hover");
    assert_eq!(McdevCommands::DIAGNOSTICS, "mcdev.diagnostics");
}
#[test]
#[doc = "mcdev-protocol/src/test/kotlin/io/github/mcdev/protocol/McdevProtocolTest.kt::completionItemPreservesDisplayAndInsertionSeparately"]
fn completion_item_preserves_display_and_insertion() {
    let item:McdevCompletionItemDto=serde_json::from_value(json!({"label":"setScreen(Screen): void","detail":"MinecraftClient","filterText":"setScreen MinecraftClient Screen","insertText":"m_91152_(Lnet/minecraft/client/gui/screens/Screen;)V","kind":"method","sortKey":"0200_setScreen"})).unwrap();
    assert_eq!(item.label, "setScreen(Screen): void");
    assert_eq!(
        item.insert_text,
        "m_91152_(Lnet/minecraft/client/gui/screens/Screen;)V"
    );
    assert_eq!(item.insert_text_format, "plainText");
    assert!(!item.filter_text.is_empty());
    assert!(item.additional_edits.is_empty());
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::decodesCompletionRequestFromMapPayload"]
fn decodes_completion_request_from_map_payload() {
    let request = ProtocolPayloadDecoder
        .decode_completion_request(&[completion()])
        .unwrap();
    assert_eq!(request.trigger.kind, "manual");
    assert_eq!(request.options.mixin_class_insert, "import");
    assert_eq!(request.context.workspace_root, "file:///tmp/project");
    assert_eq!(request.context.language_id, "java");
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::decodesCompletionBufferTextFallback"]
fn decodes_buffer_fallback_and_changedtick_without_overriding_direct_text() {
    let mut payload = completion();
    payload.as_object_mut().unwrap().remove("bufferText");
    payload["bufferTextFallback"] = json!("@Mixin(Fallback");
    let original = ProtocolPayloadDecoder
        .decode_completion_request(&[payload.clone()])
        .unwrap();
    assert_eq!(original.context.buffer_text, "@Mixin(Fallback");
    assert!(original.context.buffer_text_fallback_used);
    payload["changedtick"] = json!(42);
    let request = ProtocolPayloadDecoder
        .decode_completion_request(&[payload.clone()])
        .unwrap();
    assert_eq!(request.context.buffer_text, "@Mixin(Fallback");
    assert!(request.context.buffer_text_fallback_used);
    assert_eq!(request.context.document_version, Some(42));
    payload["bufferText"] = json!("");
    payload["documentVersion"] = json!(99);
    let context = ProtocolPayloadDecoder
        .decode_context_request(&[payload])
        .unwrap();
    assert!(context.buffer_text.is_empty());
    assert!(!context.buffer_text_fallback_used);
    assert_eq!(context.document_version, Some(99));
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::decodesFlatContextPayload"]
fn decodes_flat_context_payload() {
    let context = ProtocolPayloadDecoder
        .decode_context_request(&[context()])
        .unwrap();
    assert_eq!(
        context.position,
        McdevPosition {
            line: 12,
            character: 4
        }
    );
    assert_eq!(context.client.name, "mcdev.nvim");
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::decodesNestedContextPayload"]
fn decodes_nested_context_payload() {
    let request = ProtocolPayloadDecoder
        .decode_info_request(&[json!({"context":context()})])
        .unwrap();
    assert_eq!(request.context.client.version, "0.1.0");
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::decodeRoundtripPreservesCompletionRequest"]
fn decode_roundtrip_preserves_completion_request() {
    roundtrip(
        ProtocolPayloadDecoder
            .decode_completion_request(&[completion()])
            .unwrap(),
    );
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::decodeRoundtripPreservesRequestContext"]
fn decode_roundtrip_preserves_request_context() {
    roundtrip(
        ProtocolPayloadDecoder
            .decode_context_request(&[context()])
            .unwrap(),
    );
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::missingPayloadThrowsParseError"]
fn missing_payload_reports_parse_error() {
    let error = ProtocolPayloadDecoder
        .decode_completion_request(&[])
        .unwrap_err();
    assert_eq!(error.message, "missing command payload");
    assert_eq!(error.as_protocol_error().code, McdevErrorCode::ParseError);
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::missingWorkspaceRootThrowsParseError"]
fn missing_workspace_root_reports_parse_error() {
    let mut value = context();
    value.as_object_mut().unwrap().remove("workspaceRoot");
    assert!(ProtocolPayloadDecoder
        .decode_context_request(&[value])
        .is_err());
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::missingPositionThrowsParseError"]
fn missing_position_reports_parse_error() {
    let mut value = context();
    value.as_object_mut().unwrap().remove("position");
    assert_eq!(
        ProtocolPayloadDecoder
            .decode_context_request(&[value])
            .unwrap_err()
            .message,
        "missing cursor position"
    );
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::defaultsProtocolVersionWhenMissing"]
fn missing_optional_context_fields_use_legacy_defaults() {
    let mut original = context();
    original.as_object_mut().unwrap().remove("protocolVersion");
    assert_eq!(
        ProtocolPayloadDecoder
            .decode_context_request(&[original])
            .unwrap()
            .protocol_version,
        PROTOCOL_VERSION
    );
    let value = json!({"workspaceRoot":"file:///tmp/project","documentUri":"file:///tmp/project/Mixin.java","position":{}});
    let context = ProtocolPayloadDecoder
        .decode_context_request(&[value.clone()])
        .unwrap();
    assert_eq!(context.protocol_version, PROTOCOL_VERSION);
    assert_eq!(context.language_id, "java");
    assert_eq!(context.position, McdevPosition::default());
    assert_eq!(
        context.client,
        McdevClientInfo {
            name: "unknown".into(),
            version: "0.0.0".into()
        }
    );
    assert_eq!(context.document_version, None);
    assert_eq!(context.buffer_text, "");
    let request = ProtocolPayloadDecoder
        .decode_completion_request(&[value])
        .unwrap();
    assert_eq!(request.options, McdevCompletionOptions::default());
    assert_eq!(request.trigger.kind, "manual");
    assert_eq!(request.trigger.character, None);
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::decodeDiagnosticsRequestUsesContext"]
fn diagnostics_request_uses_context() {
    let request = ProtocolPayloadDecoder
        .decode_diagnostics_request(&[json!({"context":context()})])
        .unwrap();
    assert_eq!(request.context.buffer_text, "@Mixin(Mine");
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::decodesCodeActionRequestWithDiagnosticCodes"]
fn code_action_request_decodes_diagnostic_codes_and_ranges() {
    let original=ProtocolPayloadDecoder.decode_code_action_request(&[json!({"context":context(),"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":0}},"diagnosticCodes":["MIXIN_CLASS_NOT_LISTED_IN_CONFIG"]})]).unwrap();
    assert_eq!(original.diagnostic_codes.len(), 1);
    assert_eq!(
        original.diagnostic_codes[0],
        "MIXIN_CLASS_NOT_LISTED_IN_CONFIG"
    );
    let request=ProtocolPayloadDecoder.decode_code_action_request(&[json!({"context":context(),"range":{"start":{"line":1,"character":2},"end":{"line":3,"character":4}},"diagnosticCodes":["MIXIN_CLASS_NOT_LISTED_IN_CONFIG"]})]).unwrap();
    assert_eq!(
        request.diagnostic_codes,
        vec!["MIXIN_CLASS_NOT_LISTED_IN_CONFIG"]
    );
    assert_eq!(
        request.range.start,
        McdevPosition {
            line: 1,
            character: 2
        }
    );
    assert_eq!(
        request.range.end,
        McdevPosition {
            line: 3,
            character: 4
        }
    );
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::decodeRoundtripPreservesCodeActionRequest"]
fn decode_roundtrip_preserves_code_action_request() {
    roundtrip(ProtocolPayloadDecoder.decode_code_action_request(&[json!({"context":context(),"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":0}},"diagnosticCodes":["MIXIN_CLASS_NOT_LISTED_IN_CONFIG"]})]).unwrap());
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::decodeRoundtripPreservesDefinitionRequest"]
fn decode_roundtrip_preserves_definition_request() {
    roundtrip(
        ProtocolPayloadDecoder
            .decode_definition_request(&[json!({"context":context()})])
            .unwrap(),
    );
}
#[test]
fn raw_definition_resolution_names_are_uppercase_and_decoder_adapter_is_lowercase() {
    for (resolution, name) in [
        (McdevDefinitionResolution::Source, "source"),
        (McdevDefinitionResolution::Jdt, "jdt"),
        (McdevDefinitionResolution::BytecodeOnly, "bytecode_only"),
        (McdevDefinitionResolution::Unresolved, "unresolved"),
    ] {
        assert_eq!(
            serde_json::to_value(resolution).unwrap(),
            name.to_uppercase()
        );
        let payload = std::collections::BTreeMap::from([("value", resolution)]);
        let adapted = ProtocolPayloadDecoder.encode_to_map(&payload).unwrap();
        assert_eq!(adapted["value"], name);
        assert_eq!(
            serde_json::to_value(resolution).unwrap(),
            name.to_uppercase()
        );
        assert_eq!(
            serde_json::from_value::<McdevDefinitionResolution>(json!(name.to_uppercase()))
                .unwrap(),
            resolution
        );
    }
    assert!(serde_json::from_value::<McdevDefinitionResolution>(json!("other")).is_err());
    let strings = std::collections::BTreeMap::from([("resolution", "SOURCE")]);
    assert_eq!(
        ProtocolPayloadDecoder.encode_to_map(&strings).unwrap()["resolution"],
        "SOURCE"
    );
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::decodeRoundtripPreservesReferencesRequest"]
fn decode_roundtrip_preserves_references_request() {
    roundtrip(
        ProtocolPayloadDecoder
            .decode_references_request(&[json!({"context":context()})])
            .unwrap(),
    );
}
#[test]
fn every_command_decodes_to_typed_request_including_context_alias() {
    for command in COMMANDS {
        let decoded = ProtocolPayloadDecoder
            .decode_command(command.id(), &[completion()])
            .unwrap();
        assert_eq!(decoded.command(), command);
        assert_eq!(
            decoded.context().document_uri,
            "file:///tmp/project/src/Mixin.java"
        );
    }
    assert!(matches!(
        ProtocolPayloadDecoder
            .decode_command("mcdev.context", &[context()])
            .unwrap(),
        DecodedCommand::Context(_)
    ));
    assert_eq!(
        ProtocolPayloadDecoder
            .decode_command("unknown", &[context()])
            .unwrap_err()
            .message,
        "unknown mcdev command: unknown"
    );
}
#[test]
fn null_defaults_buffer_provenance_and_numeric_versions() {
    let mut value = context();
    value["protocolVersion"] = Value::Null;
    value["languageId"] = Value::Null;
    value["bufferText"] = Value::Null;
    value["bufferTextFallback"] = json!("fallback");
    value["documentVersion"] = json!("invalid");
    value["changedtick"] = json!("123");
    value["client"] = Value::Null;
    value["position"] = json!({"line":"2","character":3});
    let context = ProtocolPayloadDecoder
        .decode_context_request(&[value])
        .unwrap();
    assert_eq!(context.protocol_version, 1);
    assert_eq!(context.position.line, 2);
    assert_eq!(context.document_version, Some(123));
    assert!(context.buffer_text_fallback_used);
}
#[test]
fn malformed_payloads_return_errors_without_panicking() {
    for value in [
        Value::Null,
        json!([]),
        json!(1),
        json!("body"),
        json!({}),
        json!({"context":null}),
        json!({"context":1}),
        json!({"context":[]}),
    ] {
        assert!(ProtocolPayloadDecoder
            .decode_completion_request(&[value])
            .is_err());
    }
    for (field, value) in [
        ("position", Value::Null),
        ("position", json!([])),
        ("position", json!({"line":-1})),
        ("position", json!({"character":-2})),
        ("position", json!({"line":"wrong"})),
        ("position", json!({"line":2147483648_i64})),
        ("documentUri", Value::Null),
    ] {
        let mut payload = context();
        payload[field] = value;
        assert!(
            ProtocolPayloadDecoder
                .decode_context_request(&[payload])
                .is_err(),
            "{field}"
        );
    }
}
#[test]
fn code_action_missing_range_defaults_and_primitive_codes_match_decoder() {
    let request = ProtocolPayloadDecoder
        .decode_code_action_request(&[
            json!({"context":context(),"range":{"start":{}},"diagnosticCodes":[null,"x",1,true]}),
        ])
        .unwrap();
    assert_eq!(request.range, McdevRange::default());
    assert_eq!(request.diagnostic_codes, vec!["x", "1", "true"]);
    assert!(ProtocolPayloadDecoder
        .decode_code_action_request(&[json!({"context":context(),"range":{"start":null,"end":{}}})])
        .is_err());
    assert!(ProtocolPayloadDecoder
        .decode_code_action_request(&[json!({"context":context(),"diagnosticCodes":[{}]})])
        .is_err());
}
#[test]
fn gson_compatible_null_omission_does_not_erase_false_zero_or_array_null() {
    let decoder = ProtocolPayloadDecoder;
    let encoded=decoder.encode_to_map(&json!({"nil":null,"nested":{"nil":null,"false":false,"zero":0},"array":[null,{"nil":null}]})).unwrap();
    assert!(!encoded.contains_key("nil"));
    assert_eq!(encoded["nested"], json!({"false":false,"zero":0}));
    assert_eq!(encoded["array"], json!([null, {}]));
}
#[test]
fn envelope_defaults_errors_and_capabilities_remain_stable() {
    let envelope = McdevResponseEnvelope::success(
        McdevHoverResponse {
            contents: vec!["Hover".into()],
            range: None,
        },
        ["hover".into()],
    );
    let value = ProtocolPayloadDecoder.encode_to_map(&envelope).unwrap();
    assert_eq!(value["protocolVersion"], 1);
    assert_eq!(value["serverVersion"], "0.1.0");
    assert_eq!(value["capabilities"], json!(["hover"]));
    assert!(value.get("error").is_none());
    assert_eq!(value["result"], json!({"contents":["Hover"]}));
    roundtrip(envelope);
    let wire = ProtocolPayloadDecoder
        .encode_to_map(&protocol_mismatch::<Value>(8))
        .unwrap();
    assert_eq!(wire["error"]["code"], "PROTOCOL_MISMATCH");
    assert!(wire.get("result").is_none());
}
#[test]
fn completion_debug_preserves_every_nullable_metric_and_false_value() {
    let wire = json!({"command":"mcdev.completion","documentUri":"file:///Example.kt","languageId":"kotlin","totalMs":1,"payloadBytes":2,"bufferTextBytes":3,"bufferTextFallbackUsed":false,"projectSessionCacheHit":true,"projectSessionVersion":4,"loadSessionMs":5,"documentVersion":6,"documentCacheHit":false,"documentSnapshotMs":7,"semanticCacheHit":true,"astParseMs":8,"parseSource":"HOST","parseConfidence":"RESOLVED","usedCompilationUnit":false,"usedJavaProject":false,"bindingResolvedCount":9,"bindingFailedCount":10,"fallbackReason":"reason","semanticContextFound":true,"fallbackAnnotationContextUsed":false,"fallbackAnnotationContextReason":"fallback","semanticTargetCount":11,"semanticMemberCount":12,"completionContextKind":"METHOD","owner":"Foo","methodName":"run","methodDescriptor":"()V","candidateCacheHit":false,"candidateBuildMs":13,"candidateCountBeforeFilter":14,"candidateCountAfterFilter":15,"dtoConvertMs":16,"zeroItemReason":"none","negativeCacheHit":false,"inFlightDedupHit":true,"warnings":["warning"]});
    let debug: McdevCompletionDebugInfo = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(debug).unwrap(), wire);
}
#[test]
fn complete_project_info_and_dump_dtos_roundtrip() {
    let dump:McdevDumpContextResponse=serde_json::from_value(json!({"lines":["line"],"projectId":"project","root":"/tmp/project","platform":"FABRIC","mappings":{"sourceNamespace":"NAMED","runtimeNamespace":"INTERMEDIARY","awNamespace":"NAMED","atNamespace":null,"availableNamespaces":["NAMED","INTERMEDIARY"]},"classpath":{"entryCount":4,"projectOutputs":["classes"],"dependencyJars":["library.jar"],"minecraftJars":["minecraft.jar"],"generatedOutputs":["generated"],"capturedAt":123},"sourceSets":[{"name":"main","sourceDirectories":["src/main/java","src/main/kotlin"],"resourceDirectories":["src/main/resources"],"outputDirectory":"classes"}],"mixinConfigs":["mixins.json"],"accessWideners":["mod.accesswidener"],"accessTransformers":["META-INF/accesstransformer.cfg"],"minecraftJars":["minecraft.jar"],"indexState":"READY"})).unwrap();
    roundtrip(dump);
    let info:McdevInfoResponse=serde_json::from_value(json!({"lines":[],"buildCommit":"commit","buildTime":"time","version":"0.1.0","jarLocation":"legacy field still available","registeredCommands":COMMANDS.map(Command::id)})).unwrap();
    roundtrip(info);
    roundtrip(McdevReloadProjectContextResponse {
        status: "ok".into(),
        index_state: "READY".into(),
        classpath_entries: 4,
        lines: vec!["reloaded".into()],
    });
    roundtrip(McdevReindexResponse {
        status: "ok".into(),
        index_state: "READY".into(),
        classpath_entries: 4,
    });
}
