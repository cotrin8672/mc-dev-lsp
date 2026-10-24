#[path = "protocol_legacy_fixtures.rs"]
mod legacy;
use mcdev_core::{
    mapping::{MappingResolver, TinyV2Parser},
    mixin_completion::MixinClassInsertMode,
    mixin_source::*,
    *,
};
use mcdev_protocol::{convert::*, *};
use std::collections::BTreeMap;

fn annotation_context(source: &str, start: usize, end: usize, kotlin: bool) -> AnnotationContext {
    let mut context =
        AnnotationContext::new(MixinAnnotation::Mixin, AnnotationSlot::Class, "Simple");
    context.value_start_offset = start;
    context.value_end_offset = end;
    context.annotation_end_offset = source.len();
    context.is_kotlin = kotlin;
    context
}
fn item() -> McCompletionItem {
    McCompletionItem {
        label: "SimpleTarget".into(),
        detail: Some("com.example.target".into()),
        documentation: Some("com/example/target/SimpleTarget".into()),
        filter_text: "SimpleTarget com.example.target".into(),
        insert_text: "SimpleTarget.class".into(),
        kind: McCompletionKind::Class,
        sort_key: "0100_SimpleTarget".into(),
        metadata: McCompletionMetadata {
            source: "mixin.target".into(),
            owner: Some("com/example/target/SimpleTarget".into()),
            ..Default::default()
        },
        additional_edits: vec![],
        insert_text_format: McCompletionInsertTextFormat::PlainText,
    }
}
fn target_item() -> McCompletionItem {
    McCompletionItem {
        label: "draw(String, float, float): void".into(),
        detail: Some("SimpleTarget".into()),
        documentation: None,
        filter_text: "draw".into(),
        insert_text: "Lcom/example/target/SimpleTarget;draw(Ljava/lang/String;FF)V".into(),
        kind: McCompletionKind::Method,
        sort_key: "0400_draw".into(),
        metadata: McCompletionMetadata {
            source: "mixin.atTarget".into(),
            owner: Some("com/example/target/SimpleTarget".into()),
            name: Some("draw".into()),
            descriptor: Some("(Ljava/lang/String;FF)V".into()),
            namespace: None,
        },
        additional_edits: vec![],
        insert_text_format: McCompletionInsertTextFormat::PlainText,
    }
}
fn mappings() -> impl MappingResolver {
    TinyV2Parser::parse(include_str!(
        "../../mcdev-core/tests/fixtures/fabric-basic-mappings.tiny"
    ))
    .unwrap()
    .as_resolver()
}
fn apply_dto(source: &str, edit: &McdevTextEdit) -> String {
    let start = position_to_offset(
        source,
        McTextPosition {
            line: edit.range.start.line as usize,
            character: edit.range.start.character as usize,
        },
    )
    .unwrap();
    let end = position_to_offset(
        source,
        McTextPosition {
            line: edit.range.end.line as usize,
            character: edit.range.end.character as usize,
        },
    )
    .unwrap();
    apply_text_edits(
        source,
        &[McTextEdit {
            start_offset: start,
            end_offset: end,
            new_text: edit.new_text.clone(),
        }],
    )
    .unwrap()
}

#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::completionConverterPreservesLabelAndInsertText"]
fn completion_preserves_label_insert_and_kind() {
    let source = "@Mixin(Simple)";
    let annotation = annotation_context(source, 7, 13, false);
    let dto = CompletionItemConverter::to_dto(
        &legacy::completion_item("completionConverterPreservesLabelAndInsertText"),
        &CompletionConvertContext::new(source, Some(&annotation)),
    )
    .unwrap();
    assert_eq!(dto.label, "SimpleTarget");
    assert_eq!(dto.insert_text, "SimpleTarget.class");
    assert_eq!(dto.kind, "class");
    assert_eq!(dto.metadata["source"].as_deref(), Some("mixin.target"));
    assert_eq!(dto.edit.unwrap().range.start.character, 7);
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::completionConverterMapsItemAdditionalEdits"]
fn completion_preserves_additional_edits() {
    let source = "@Mixin(Simple)";
    let annotation = annotation_context(source, 7, 13, false);
    let mut item = legacy::completion_item("completionConverterMapsItemAdditionalEdits");
    item.metadata.owner = None;
    item.additional_edits = vec![McTextEdit {
        start_offset: 0,
        end_offset: 0,
        new_text: "import com.example.target.SimpleTarget;\n".into(),
    }];
    let dto = CompletionItemConverter::to_dto(
        &item,
        &CompletionConvertContext::new(source, Some(&annotation)),
    )
    .unwrap();
    assert_eq!(dto.additional_edits.len(), 1);
    assert_eq!(
        dto.additional_edits[0].new_text,
        "import com.example.target.SimpleTarget;\n"
    );
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::completionConverterAddsImportAdditionalEditForImportModeMixinTarget"]
fn completion_adds_import_for_import_mode_mixin_target() {
    let source =
        "package com.example.mixin;\n\nimport org.spongepowered.asm.mixin.Mixin;\n\n@Mixin(Simple";
    let annotation =
        annotation_context(source, source.find("Simple").unwrap(), source.len(), false);
    let dto = CompletionItemConverter::to_dto(
        &legacy::completion_item(
            "completionConverterAddsImportAdditionalEditForImportModeMixinTarget",
        ),
        &CompletionConvertContext::new(source, Some(&annotation)),
    )
    .unwrap();
    assert_eq!(dto.additional_edits.len(), 1);
    assert!(dto.additional_edits[0]
        .new_text
        .contains("import com.example.target.SimpleTarget;"));
    let mut context = CompletionConvertContext::new(source, Some(&annotation));
    context.class_insert_mode = MixinClassInsertMode::Fqn;
    assert!(CompletionItemConverter::to_dto(
        &legacy::completion_item(
            "completionConverterAddsImportAdditionalEditForImportModeMixinTarget"
        ),
        &context
    )
    .unwrap()
    .additional_edits
    .is_empty());
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::kotlinMixinTargetImportsDoNotUseJavaSyntaxOrOverwriteAliasBindings"]
fn kotlin_imports_preserve_aliases_and_conflicting_bindings() {
    let base = "package example\n\nimport io.github.diskria.lapis.annotations.KMixin\n";
    let source = format!("{base}\n@KMixin(Simple)\nclass Patch");
    let start = source.find("Simple").unwrap();
    let annotation = annotation_context(&source, start, start + 6, true);
    let mut item = legacy::completion_item(
        "kotlinMixinTargetImportsDoNotUseJavaSyntaxOrOverwriteAliasBindings",
    );
    item.insert_text = "SimpleTarget::class".into();
    let dto = CompletionItemConverter::to_dto(
        &item,
        &CompletionConvertContext::new(&source, Some(&annotation)),
    )
    .unwrap();
    assert_eq!(dto.edit.as_ref().unwrap().new_text, "SimpleTarget::class");
    assert_eq!(
        dto.additional_edits
            .iter()
            .map(|e| e.new_text.as_str())
            .collect::<Vec<_>>(),
        vec!["import com.example.target.SimpleTarget\n"]
    );
    for (extra, reference) in [
        (
            "import com.example.target.SimpleTarget as Target",
            "Target::class",
        ),
        (
            "import other.SimpleTarget",
            "com.example.target.SimpleTarget::class",
        ),
    ] {
        let source = format!("{base}{extra}\n@KMixin(Simple)\nclass Patch");
        let start = source.rfind("Simple)").unwrap();
        let annotation = annotation_context(&source, start, start + 6, true);
        item.insert_text = reference.into();
        let dto = CompletionItemConverter::to_dto(
            &item,
            &CompletionConvertContext::new(&source, Some(&annotation)),
        )
        .unwrap();
        assert_eq!(dto.edit.unwrap().new_text, reference);
        assert!(dto.additional_edits.is_empty());
    }
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::completionConverterRemapsAtTargetInsertTextWithMappings"]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::completionConverterUsesFullAtTargetInsertTextWhenSmartTargetIsUnambiguous"]
fn at_target_converter_remaps_descriptor_and_smart_modes() {
    let resolver = mappings();
    let item = legacy::completion_item("completionConverterRemapsAtTargetInsertTextWithMappings");
    for preferred in ["descriptor", "smart", "SMART"] {
        let mut context = CompletionConvertContext::new("", None);
        context.preferred_at_target = preferred;
        context.mapping_resolver = Some(&resolver);
        let dto = if preferred == "smart" {
            CompletionItemConverter::to_dtos(std::slice::from_ref(&item), &context)
                .unwrap()
                .remove(0)
        } else {
            CompletionItemConverter::to_dto(&item, &context).unwrap()
        };
        assert_eq!(
            dto.insert_text,
            "Lcom/example/target/class_1;method_1(Ljava/lang/String;FF)V"
        );
    }
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::completionConverterKeepsFullDescriptorsForAmbiguousSmartTargets"]
fn smart_targets_keep_full_descriptors_when_ambiguous() {
    let resolver = mappings();
    let first =
        legacy::completion_item("completionConverterKeepsFullDescriptorsForAmbiguousSmartTargets");
    let mut second = first.clone();
    second.label = "draw(int): void".into();
    second.insert_text = "Lcom/example/target/SimpleTarget;draw(I)V".into();
    second.metadata.descriptor = Some("(I)V".into());
    let mut context = CompletionConvertContext::new("", None);
    context.mapping_resolver = Some(&resolver);
    let dtos = CompletionItemConverter::to_dtos(&[first, second], &context).unwrap();
    assert_eq!(
        dtos.iter()
            .map(|i| i.insert_text.as_str())
            .collect::<Vec<_>>(),
        vec![
            "Lcom/example/target/class_1;method_1(Ljava/lang/String;FF)V",
            "Lcom/example/target/SimpleTarget;draw(I)V"
        ]
    );
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::completionConverterTextEditKeepsPretypedOwnerAndProducesValidTarget"]
fn explicit_target_range_replaces_owner_and_member_as_one_valid_target() {
    let resolver = mappings();
    let source = "@At(value = \"INVOKE\", target = \"Lcom/example/target/SimpleTarget;dra\")";
    let start = source.find("Lcom/example/target/SimpleTarget;").unwrap();
    let end = source.rfind('"').unwrap();
    let mut context = CompletionConvertContext::new(source, None);
    context.mapping_resolver = Some(&resolver);
    context.replacement_range = Some(McCompletionReplacementRange {
        start_offset: start,
        end_offset: end,
    });
    let dto = CompletionItemConverter::to_dto(
        &legacy::completion_item(
            "completionConverterTextEditKeepsPretypedOwnerAndProducesValidTarget",
        ),
        &context,
    )
    .unwrap();
    let applied = apply_dto(source, &dto.edit.unwrap());
    let value = applied
        .split("target = \"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert_eq!(
        value,
        "Lcom/example/target/class_1;method_1(Ljava/lang/String;FF)V"
    );
    assert!(mcdev_core::descriptor::parse_member_target(value).is_ok());
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::completionConverterUsesExplicitPartialAnnotationRangeAndPreservesImports"]
fn explicit_partial_annotation_range_preserves_import_and_snippet() {
    let source = "package demo;\n\n@Mix";
    let mut item = legacy::completion_item(
        "completionConverterUsesExplicitPartialAnnotationRangeAndPreservesImports",
    );
    item.label = "Mixin".into();
    item.insert_text = "Mixin(${1:Target}.class)$0".into();
    item.kind = McCompletionKind::Keyword;
    item.metadata.source = "mixin.annotation".into();
    item.additional_edits = vec![McTextEdit {
        start_offset: 13,
        end_offset: 13,
        new_text: "\nimport org.spongepowered.asm.mixin.Mixin;".into(),
    }];
    let mut context = CompletionConvertContext::new(source, None);
    context.replacement_range = Some(McCompletionReplacementRange {
        start_offset: source.find("Mix").unwrap(),
        end_offset: source.len(),
    });
    let dto = CompletionItemConverter::to_dto(&item, &context).unwrap();
    assert_eq!(dto.insert_text_format, "plainText");
    let edit = dto.edit.unwrap();
    assert_eq!(
        edit.range.start,
        McdevPosition {
            line: 2,
            character: 1
        }
    );
    assert_eq!(edit.range.end.character, 4);
    assert_eq!(edit.new_text, "Mixin(${1:Target}.class)$0");
    assert_eq!(dto.additional_edits.len(), 1);
    assert!(dto.additional_edits[0]
        .new_text
        .contains("import org.spongepowered.asm.mixin.Mixin;"));
    item.insert_text_format = McCompletionInsertTextFormat::Snippet;
    assert_eq!(
        CompletionItemConverter::to_dto(&item, &context)
            .unwrap()
            .insert_text_format,
        "snippet"
    );
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::completionConverterKeepsInjectMethodOpenQuoteOutsideTextEdit"]
fn inject_method_open_quote_stays_outside_replacement() {
    let source_with_marker =
        legacy::source("completionConverterKeepsInjectMethodOpenQuoteOutsideTextEdit");
    let marker = "/*caret*/";
    let marker_offset = source_with_marker.find(marker).unwrap();
    let source = source_with_marker.replace(marker, "");
    let before_marker = &source_with_marker[..marker_offset];
    let line = before_marker.bytes().filter(|c| *c == b'\n').count();
    let character = before_marker
        .rsplit('\n')
        .next()
        .unwrap()
        .encode_utf16()
        .count();
    let model = MixinSemanticModelParser::parse(&source, "file:///ItemMixin.java");
    let context =
        SemanticCompletionContextExtractor::extract(&source, line, character, Some(&model))
            .unwrap();
    let annotation = SemanticCompletionContextExtractor::to_annotation_context(
        &source,
        line,
        character,
        Some(&model),
        &context,
    )
    .unwrap();
    let item =
        legacy::completion_item("completionConverterKeepsInjectMethodOpenQuoteOutsideTextEdit");
    let dto = CompletionItemConverter::to_dto(
        &item,
        &CompletionConvertContext::new(&source, Some(&annotation)),
    )
    .unwrap();
    let edit = dto.edit.unwrap();
    assert_eq!(edit.new_text, "isFoil");
    let start = source.find("isF").unwrap();
    let line_start = source[..start].rfind('\n').map_or(0, |p| p + 1);
    assert_eq!(edit.range.start.character, (start - line_start) as i32);
    assert_eq!(
        edit.range.end.character,
        (start + "isF".len() - line_start) as i32
    );
}

#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::diagnosticConverterMapsSeverityAndRange"]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::diagnosticConverterMapsWarningSeverity"]
fn diagnostics_convert_all_severities_metadata_and_ranges() {
    for (severity, name) in [
        (McSeverity::Error, "error"),
        (McSeverity::Warning, "warning"),
        (McSeverity::Info, "information"),
    ] {
        let mut diagnostic = McDiagnostic {
            code: "UNRESOLVED_MIXIN_TARGET".into(),
            severity,
            message: "Unresolved @Mixin target".into(),
            range: McTextRange {
                start: McTextPosition {
                    line: 1,
                    character: 2,
                },
                end: McTextPosition {
                    line: 1,
                    character: 10,
                },
            },
            metadata: BTreeMap::from([("target".into(), "Missing".into())]),
        };
        if severity == McSeverity::Warning {
            diagnostic.code = "MIXIN_CLASS_NOT_LISTED_IN_CONFIG".into();
            diagnostic.message = "not listed".into();
            diagnostic.range = McTextRange::default();
            diagnostic.metadata.clear();
        }
        let dto = DiagnosticConverter::to_dto(&diagnostic).unwrap();
        assert_eq!(dto.severity, name);
        assert_eq!(dto.code, diagnostic.code);
        if severity != McSeverity::Warning {
            assert_eq!(dto.metadata["target"], "Missing");
            assert_eq!(dto.range.start.line, 1);
            assert_eq!(dto.range.end.character, 10);
        }
    }
}
fn target(kind: MemberKind) -> McDefinitionTarget {
    McDefinitionTarget {
        kind,
        owner_internal_name: "com/example/target/SimpleTarget".into(),
        owner_fqn: Some("com.example.target.SimpleTarget".into()),
        name: None,
        descriptor: None,
        namespace: MappingNamespace::Named,
        source_range: None,
        direct_source_document_uri: None,
    }
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::definitionConverterMapsClassTargetMetadata"]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::definitionConverterMapsFieldTargetMetadata"]
fn definition_converter_preserves_class_and_field_metadata() {
    for kind in [MemberKind::Class, MemberKind::Field] {
        let mut target = target(kind);
        if kind == MemberKind::Field {
            target.name = Some("counter".into());
            target.descriptor = Some("I".into());
        }
        let resolved = ResolvedDefinition {
            target,
            document_uri: "file:///project/SimpleTarget.java".into(),
            range: if kind == MemberKind::Class {
                McTextRange {
                    start: McTextPosition {
                        line: 2,
                        character: 0,
                    },
                    end: McTextPosition {
                        line: 2,
                        character: 20,
                    },
                }
            } else {
                McTextRange {
                    start: McTextPosition {
                        line: 3,
                        character: 16,
                    },
                    end: McTextPosition {
                        line: 3,
                        character: 23,
                    },
                }
            },
            resolution: McdevDefinitionResolution::Source,
            resolution_message: None,
        };
        let location = DefinitionConverter::to_location(&resolved).unwrap();
        assert_eq!(location.document_uri, "file:///project/SimpleTarget.java");
        assert_eq!(
            location.metadata["owner"],
            "com/example/target/SimpleTarget"
        );
        assert_eq!(location.metadata["fqn"], "com.example.target.SimpleTarget");
        assert_eq!(location.resolution, McdevDefinitionResolution::Source);
        assert_eq!(
            location.metadata["kind"],
            if kind == MemberKind::Class {
                "class"
            } else {
                "field"
            }
        );
        if kind == MemberKind::Field {
            assert_eq!(location.metadata["name"], "counter");
            assert_eq!(location.metadata["descriptor"], "I");
        }
    }
}
#[test]
fn unresolved_and_bytecode_locations_do_not_claim_source_navigation() {
    for resolution in [
        McdevDefinitionResolution::BytecodeOnly,
        McdevDefinitionResolution::Unresolved,
    ] {
        let target = target(MemberKind::Class);
        let resolved = ResolvedDefinition {
            target,
            document_uri: "file:///should/not/navigate".into(),
            range: McTextRange {
                start: McTextPosition {
                    line: 9,
                    character: 9,
                },
                end: McTextPosition {
                    line: 9,
                    character: 9,
                },
            },
            resolution,
            resolution_message: Some("sources missing".into()),
        };
        let dto = DefinitionConverter::to_location(&resolved).unwrap();
        assert!(dto.document_uri.is_empty());
        assert_eq!(dto.range, McdevRange::default());
        assert_eq!(dto.resolution_message.as_deref(), Some("sources missing"));
    }
}
#[test]
fn references_keep_uri_metadata_and_source_resolution() {
    let reference = McReferenceLocation {
        document_uri: "file:///Mixin.kt".into(),
        range: McTextRange {
            start: McTextPosition {
                line: 2,
                character: 3,
            },
            end: McTextPosition {
                line: 2,
                character: 8,
            },
        },
        metadata: BTreeMap::from([("source".into(), "mixin.reference".into())]),
    };
    let dto = DefinitionConverter::reference_to_location(&reference).unwrap();
    assert_eq!(dto.metadata, reference.metadata);
    assert_eq!(dto.resolution, McdevDefinitionResolution::Source);
}
#[test]
fn utf8_offsets_become_utf16_edits_and_malformed_boundaries_fail() {
    let source = "😀日\nβ";
    let edit = text_edit_to_dto(
        source,
        &McTextEdit {
            start_offset: 4,
            end_offset: 7,
            new_text: "語".into(),
        },
    )
    .unwrap();
    assert_eq!(edit.range.start.character, 2);
    assert_eq!(edit.range.end.character, 3);
    assert_eq!(apply_dto(source, &edit), "😀語\nβ");
    assert!(text_edit_to_dto(
        source,
        &McTextEdit {
            start_offset: 1,
            end_offset: 4,
            new_text: "".into()
        }
    )
    .is_err());
}
#[test]
fn incomplete_class_search_only_counts_bounded_sources() {
    let classes = vec![item(); 50];
    assert!(CompletionItemConverter::is_incomplete(&classes));
    assert!(!CompletionItemConverter::is_incomplete(&classes[..49]));
    let mut other = item();
    other.metadata.source = "mixin.annotation".into();
    assert!(!CompletionItemConverter::is_incomplete(&vec![other; 100]));
}

struct EmptyIndex;
impl ClassIndex for EmptyIndex {
    fn find_classes(&self, _: &str, _: usize) -> Vec<ClassIndexEntry> {
        vec![]
    }
    fn find_class(&self, _: &str) -> Option<ClassIndexEntry> {
        None
    }
    fn find_class_by_fqn(&self, _: &str) -> Option<ClassIndexEntry> {
        None
    }
    fn get_methods(&self, _: &str) -> Vec<MethodIndexEntry> {
        vec![]
    }
    fn get_fields(&self, _: &str) -> Vec<FieldIndexEntry> {
        vec![]
    }
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::codeActionConverterMapsAccessorGenerateFix"]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::codeActionConverterMapsInvokerGenerateFix"]
fn code_action_maps_accessor_and_invoker_generation_without_domain_callback() {
    let source = "@Mixin(MinecraftClient.class) class M { }";
    let insert = source.find('{').unwrap() + 1;
    let fixes = [
        McFix::GenerateAccessorMethodFix {
            title: "Generate @Accessor getter for 'currentScreen'".into(),
            document_uri: "file:///Mixin.java".into(),
            insert_offset: insert,
            method_source: "    @Accessor(\"currentScreen\")\n    Screen getCurrentScreen();\n"
                .into(),
            field_name: "currentScreen".into(),
            is_getter: true,
        },
        McFix::GenerateInvokerMethodFix {
            title: "Generate @Invoker for 'setScreen'".into(),
            document_uri: "file:///Mixin.java".into(),
            insert_offset: insert,
            method_source:
                "    @Invoker(\"setScreen\")\n    void invokeSetScreen(Screen screen);\n".into(),
            target_method_name: "setScreen".into(),
        },
    ];
    for fix in fixes {
        let dto = CodeActionConverter::to_dto(&fix, source, |_| {
            panic!("direct insertion must not resolve again")
        })
        .unwrap()
        .unwrap();
        assert_eq!(dto.kind, fix.kind());
        assert_eq!(dto.edits[0].document_uri, "file:///Mixin.java");
        let applied = apply_dto(source, &dto.edits[0].edits[0]);
        assert!(
            applied.contains(if matches!(fix, McFix::GenerateAccessorMethodFix { .. }) {
                "currentScreen"
            } else {
                "setScreen"
            })
        );
        assert_eq!(dto.metadata.len(), 1);
    }
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::codeActionConverterMapsRemoveDuplicateAccessWidenerFix"]
fn code_action_maps_remove_duplicate_access_widener_through_domain() {
    let source = legacy::source("codeActionConverterMapsRemoveDuplicateAccessWidenerFix");
    let fix = McFix::RemoveDuplicateAccessWidenerEntryFix {
        title: "Remove duplicate access widener entry".into(),
        document_uri: "file:///mod.accesswidener".into(),
        line_number: 3,
    };
    let facade = mcdev_core::awat::AwAtServiceFacade::new(&EmptyIndex, None);
    let dto = CodeActionConverter::to_dto(&fix, source, |f| {
        facade.apply_fix(f, source).map(|r| (source, r))
    })
    .unwrap()
    .unwrap();
    assert_eq!(dto.kind, "quickfix.aw.removeDuplicate");
    assert_eq!(dto.edits[0].edits.len(), 1);
    assert_eq!(
        apply_dto(source, &dto.edits[0].edits[0])
            .matches("accessible class")
            .count(),
        1
    );
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::codeActionConverterMapsAddAtMethodDescriptorFix"]
fn code_action_maps_at_method_descriptor_through_domain() {
    let source = "public com.example.target.SimpleTarget draw";
    let fix = McFix::AddAtMethodDescriptorFix {
        title: "Add method descriptor".into(),
        document_uri: "file:///mod_at.cfg".into(),
        line: 1,
        member_name: "draw".into(),
        descriptor: "(Ljava/lang/String;FF)V".into(),
    };
    let facade = mcdev_core::awat::AwAtServiceFacade::new(&EmptyIndex, None);
    let dto = CodeActionConverter::to_dto(&fix, source, |f| {
        facade.apply_fix(f, source).map(|r| (source, r))
    })
    .unwrap()
    .unwrap();
    assert_eq!(dto.kind, "quickfix.at.addDescriptor");
    assert!(dto.edits[0].edits[0]
        .new_text
        .contains("draw(Ljava/lang/String;FF)V"));
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::codeActionConverterMapsMixinExtrasFixHandlerSignature"]
fn code_action_maps_mixinextras_workspace_fix_and_preserves_metadata() {
    let source = legacy::source("codeActionConverterMapsMixinExtrasFixHandlerSignature");
    let insert = source.find('{').unwrap() + 1;
    let fix=McFix::WorkspaceEditFix{title:"Fix WrapOperation handler signature".into(),kind:"quickfix.mixinextras.fixHandlerSignature".into(),document_uri:"file:///BadMixinExtras.java".into(),edits:vec![McTextEdit{start_offset:insert,end_offset:insert,new_text:"\n    private int mcdevWrapLength(String instance, Operation<Integer> original) {\n        return original.call(instance);\n    }\n".into()}],metadata:Default::default()};
    let dto = CodeActionConverter::to_dto(&fix, source, |_| panic!())
        .unwrap()
        .unwrap();
    assert_eq!(dto.kind, "quickfix.mixinextras.fixHandlerSignature");
    assert!(dto.edits[0].edits[0]
        .new_text
        .contains("Operation<Integer>"));
    assert!(dto.metadata.is_empty());
    let mut enriched = fix.clone();
    if let McFix::WorkspaceEditFix { metadata, .. } = &mut enriched {
        metadata.insert("source".into(), "mixinextras.handler".into());
    }
    let dto = CodeActionConverter::to_dto(&enriched, source, |_| None)
        .unwrap()
        .unwrap();
    assert_eq!(dto.metadata["source"], "mixinextras.handler");
}
#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::codeActionConverterMapsMixinConfigFix"]
fn symbolic_fix_uses_resolved_document_content_and_original_metadata() {
    let source = "@Mixin(Simple.class) class ExampleMixin {}";
    let config = "{ \"mixins\": [] }";
    let fix = McFix::AddMixinConfigEntryFix {
        title: "Add 'ExampleMixin' to mixin config".into(),
        config_path: "mixins.json".into(),
        mixin_class_name: "ExampleMixin".into(),
        mixin_package: Some("com.example.mixin".into()),
        array_name: "mixins".into(),
    };
    let resolved =
        mcdev_core::mixin_actions::MixinCodeActionService::apply_mixin_config_fix(&fix, config)
            .unwrap();
    let dto = CodeActionConverter::to_dto(&fix, source, |_| Some((config, resolved)))
        .unwrap()
        .unwrap();
    assert_eq!(dto.kind, "quickfix.mixin.config");
    assert!(dto.edits[0].edits[0].new_text.contains("ExampleMixin"));
    assert_eq!(dto.metadata["mixinClass"], "ExampleMixin");
    assert_eq!(dto.metadata["mixinPackage"], "com.example.mixin");
    assert_eq!(dto.metadata["configPath"], "mixins.json");
    assert!(!dto.metadata.contains_key("arrayName"));
    assert!(CodeActionConverter::to_dto(&fix, source, |_| None)
        .unwrap()
        .is_none());
}
#[test]
fn converter_rejects_unresolved_or_invalid_edits() {
    let fix = McFix::WorkspaceEditFix {
        title: "bad".into(),
        kind: "quickfix".into(),
        document_uri: "file:///Source".into(),
        edits: vec![McTextEdit {
            start_offset: 1,
            end_offset: 2,
            new_text: "".into(),
        }],
        metadata: Default::default(),
    };
    assert!(CodeActionConverter::to_dto(&fix, "😀", |_| None).is_err());
    let symbolic = McFix::RemapAccessTransformerEntryFix {
        title: "bad".into(),
        document_uri: "file:///Source".into(),
        line: 1,
    };
    assert!(CodeActionConverter::workspace_edit_to_dto(&symbolic, &symbolic, "").is_err());
}

#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/protocol/ProtocolPayloadDecoderTest.kt::encodeDefinitionResponseUsesLowercaseResolutionWireValues"]
fn definition_response_envelope_encodes_lowercase_resolution() {
    let resolved = ResolvedDefinition {
        target: target(MemberKind::Class),
        document_uri: "file:///mapped-sources/SimpleTarget.java".into(),
        range: McTextRange {
            start: McTextPosition {
                line: 3,
                character: 0,
            },
            end: McTextPosition {
                line: 3,
                character: 20,
            },
        },
        resolution: McdevDefinitionResolution::Source,
        resolution_message: None,
    };
    let envelope = McdevResponseEnvelope::success(
        McdevDefinitionResponse {
            locations: vec![DefinitionConverter::to_location(&resolved).unwrap()],
        },
        ["definition".into()],
    );
    let encoded = ProtocolPayloadDecoder.encode_to_map(&envelope).unwrap();
    assert_eq!(encoded["result"]["locations"][0]["resolution"], "source");
}

#[test]
fn nullable_completion_metadata_omits_null_on_direct_and_decoder_serialization() {
    let mut item = item();
    item.metadata.owner = None;
    let dto =
        CompletionItemConverter::to_dto(&item, &CompletionConvertContext::new("", None)).unwrap();
    let direct = serde_json::to_value(&dto).unwrap();
    let encoded = ProtocolPayloadDecoder.encode_to_map(&dto).unwrap();
    assert_eq!(
        direct["metadata"],
        serde_json::json!({"source":"mixin.target"})
    );
    assert_eq!(encoded["metadata"], direct["metadata"]);
}

#[test]
fn completion_rejects_overlapping_primary_and_additional_edits() {
    let source = "@Mixin(Simple)";
    let mut item = item();
    item.metadata.owner = None;
    item.additional_edits = vec![McTextEdit {
        start_offset: 7,
        end_offset: 10,
        new_text: "bad".into(),
    }];
    let annotation = annotation_context(source, 7, 13, false);
    assert!(CompletionItemConverter::to_dto(
        &item,
        &CompletionConvertContext::new(source, Some(&annotation))
    )
    .is_err());
}

#[test]
#[doc = "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/ConverterTest.kt::completionConverterBuildsTextEditFromAnnotationOffsets"]
fn legacy_completion_annotation_offsets() {
    let source = "@Mixin(Simple)";
    let context = annotation_context(source, 7, 13, false);
    let item = legacy::completion_item("completionConverterBuildsTextEditFromAnnotationOffsets");
    let dto = CompletionItemConverter::to_dto(
        &item,
        &CompletionConvertContext::new(source, Some(&context)),
    )
    .unwrap();
    let edit = dto.edit.unwrap();
    assert_eq!(edit.new_text, "SimpleTarget.class");
    assert_eq!(edit.range.start.line, 0);
    assert_eq!(edit.range.start.character, 7);
}

#[test]
fn method_descriptor_fix_delegates_to_core_and_emits_utf16_range() {
    let source = "// 😀\n@Inject(method = \"draw\")";
    let start = source.find("draw").unwrap();
    let fix = McFix::AddMethodDescriptorFix {
        title: "Add descriptor".into(),
        document_uri: "file:///Mixin.kt".into(),
        start_offset: start,
        end_offset: start + 4,
        method_name: "draw".into(),
        descriptor: "(I)V".into(),
    };
    let dto = CodeActionConverter::to_dto(&fix, source, |fix| {
        mcdev_core::mixin_actions::MixinCodeActionService::apply_method_descriptor_fix(fix, source)
            .map(|resolved| (source, resolved))
    })
    .unwrap()
    .unwrap();
    assert_eq!(dto.edits[0].edits[0].range.start.line, 1);
    assert!(apply_dto(source, &dto.edits[0].edits[0]).contains("method = \"draw(I)V\""));
    assert_eq!(dto.kind, "quickfix.mixin.methodDescriptor");
}
