//! Ports of the legacy aw/*, at/* and awat/* suites. See awat-coverage.md.
use mcdev_core::{
    at::*,
    aw::*,
    awat::*,
    mapping::{
        ClassMapping, EmptyMappingResolver, FieldMapping, MappingSet, MappingSetResolver,
        MethodMapping, ProjectMappingContext,
    },
    model::*,
};
use std::{collections::BTreeMap, sync::Arc};
const OWNER: &str = "net/minecraft/client/MinecraftClient";
const FQN: &str = "net.minecraft.client.MinecraftClient";
const DESC: &str = "(Lnet/minecraft/client/gui/screen/Screen;)V";
const FD: &str = "Lnet/minecraft/client/gui/screen/Screen;";
const HEADER: &str = "accessWidener v2 named\n";

#[test]
fn columns_past_line_end_clamp_without_splitting_utf16_or_crlf() {
    let text = "a😀\r\nx";
    assert_eq!(AwContextExtractor::to_offset(text, 0, usize::MAX), Some(5));
    assert_eq!(AtTextPositions::to_offset(text, 0, usize::MAX), Some(5));
    assert_eq!(AwContextExtractor::to_offset(text, 0, 2), None);
    assert_eq!(AtTextPositions::to_offset(text, 0, 2), None);
    assert_eq!(AwContextExtractor::to_offset(text, 1, 99), Some(8));
    assert_eq!(AtTextPositions::to_offset(text, 2, 99), None);
    let index = Index::default();
    let facade = AwAtServiceFacade::new(&index, None);
    for (file_type, source) in [
        (
            AwAtFileType::AccessWidener,
            "accessWidener v2 named\naccessible class net/minecraft/client/MinecraftClient",
        ),
        (
            AwAtFileType::AccessTransformer,
            "public net.minecraft.client.MinecraftClient",
        ),
    ] {
        let line = usize::from(file_type == AwAtFileType::AccessWidener);
        let end = source.lines().last().unwrap().encode_utf16().count();
        let request = |character| AwAtFacadeRequest {
            buffer_text: source,
            line,
            character,
            document_uri: "file:///test",
            file_type,
        };
        assert_eq!(
            facade.complete(&request(end)),
            facade.complete(&request(999))
        );
        let definitions = facade.definitions_at(&request(end));
        assert!(!definitions.is_empty());
        assert_eq!(definitions, facade.definitions_at(&request(999)));
    }
}
#[derive(Clone)]
struct Index {
    methods: Vec<MethodIndexEntry>,
    fields: Vec<FieldIndexEntry>,
}
fn method(name: &str, descriptor: &str) -> MethodIndexEntry {
    MethodIndexEntry {
        name: name.into(),
        descriptor: descriptor.into(),
        is_static: false,
        readable_signature: format!("{name}{descriptor}"),
    }
}
fn field(name: &str, descriptor: &str) -> FieldIndexEntry {
    FieldIndexEntry {
        name: name.into(),
        descriptor: descriptor.into(),
        is_static: false,
        readable_type: descriptor.into(),
    }
}
impl Default for Index {
    fn default() -> Self {
        Self {
            methods: vec![method("setScreen", DESC), method("tick", "()V")],
            fields: vec![
                field("currentScreen", FD),
                field("player", "Lnet/minecraft/client/player/ClientPlayerEntity;"),
            ],
        }
    }
}
impl ClassIndex for Index {
    fn find_classes(&self, prefix: &str, limit: usize) -> Vec<ClassIndexEntry> {
        [
            ClassIndexEntry {
                simple_name: "MinecraftClient".into(),
                package_name: "net.minecraft.client".into(),
                internal_name: OWNER.into(),
            },
            ClassIndexEntry {
                simple_name: "GameRenderer".into(),
                package_name: "net.minecraft.client.render".into(),
                internal_name: "net/minecraft/client/render/GameRenderer".into(),
            },
        ]
        .into_iter()
        .filter(|c| c.simple_name.starts_with(prefix))
        .take(limit)
        .collect()
    }
    fn find_class(&self, owner: &str) -> Option<ClassIndexEntry> {
        self.find_classes("", 50)
            .into_iter()
            .find(|c| c.internal_name == owner)
    }
    fn find_class_by_fqn(&self, fqn: &str) -> Option<ClassIndexEntry> {
        self.find_classes("", 50)
            .into_iter()
            .find(|c| c.fqn() == fqn)
    }
    fn get_methods(&self, owner: &str) -> Vec<MethodIndexEntry> {
        if owner == OWNER {
            self.methods.clone()
        } else {
            vec![]
        }
    }
    fn get_fields(&self, owner: &str) -> Vec<FieldIndexEntry> {
        if owner == OWNER {
            self.fields.clone()
        } else {
            vec![]
        }
    }
}
fn mappings(ns: MappingNamespace, incomplete: bool) -> ProjectMappingContext {
    use MappingNamespace::*;
    let names = |a: &str, b: &str, c: &str| {
        BTreeMap::from([(Named, a.into()), (Intermediary, b.into()), (Srg, c.into())])
    };
    let owner = ClassMapping {
        names: names(
            OWNER,
            "net/minecraft/client/class_310",
            "net/minecraft/client/Minecraft",
        ),
    };
    let screen = ClassMapping {
        names: names(
            "net/minecraft/client/gui/screen/Screen",
            "net/minecraft/client/gui/class_437",
            "net/minecraft/client/gui/Screen",
        ),
    };
    let set = MappingSet {
        namespaces: vec![Named, Intermediary, Srg],
        classes: vec![owner.clone(), screen],
        methods: if incomplete {
            vec![]
        } else {
            vec![
                MethodMapping {
                    owner: owner.clone(),
                    descriptor: DESC.into(),
                    names: names("setScreen", "method_91152", "m_91152_"),
                },
                MethodMapping {
                    owner: owner.clone(),
                    descriptor: "()V".into(),
                    names: names("tick", "method_1", "m_91398_"),
                },
            ]
        },
        fields: vec![FieldMapping {
            owner,
            descriptor: FD.into(),
            names: names("currentScreen", "field_1755", "f_91074_"),
        }],
    };
    ProjectMappingContext {
        source_namespace: Named,
        runtime_namespace: ns,
        aw_namespace: Some(Named),
        at_namespace: if ns == Srg { Some(ns) } else { None },
        available_namespaces: set.namespaces.iter().copied().collect(),
        resolver: Arc::new(MappingSetResolver::new(set)),
    }
}
fn aw_context(s: &str) -> AwAnnotationContext {
    let at = s.find('|').expect("cursor");
    AwContextExtractor::extract_at_offset(&s.replace('|', ""), at).unwrap()
}
fn at_context(s: &str) -> AtContext {
    let at = s.find('|').expect("cursor");
    AtContextExtractor::extract_at_offset(&s.replace('|', ""), at).unwrap()
}
fn apply(fix: McFix, text: &str) -> String {
    if let McFix::WorkspaceEditFix { edits, .. } = fix {
        apply_text_edits(text, &edits).unwrap()
    } else {
        panic!("expected workspace edit")
    }
}
fn valid_aw() -> String {
    format!("{HEADER}accessible class {OWNER}\naccessible method {OWNER} setScreen {DESC}\nmutable field {OWNER} currentScreen {FD}")
}
fn aw_codes(s: &str, index: &Index, m: Option<&ProjectMappingContext>) -> Vec<String> {
    AccessWidenerDiagnosticsService::new(index)
        .analyze(s, "file:///test.aw", m)
        .into_iter()
        .map(|d| d.code)
        .collect()
}
fn at_codes(s: &str, index: &Index, m: Option<&ProjectMappingContext>) -> Vec<String> {
    AccessTransformerDiagnosticsService::new(index, m)
        .analyze(s, "file:///test.at")
        .into_iter()
        .map(|d| d.code)
        .collect()
}

#[test]
fn aw_parser_header_entries_and_invalid_inputs() {
    let f = AccessWidenerParser::parse(&valid_aw()).unwrap();
    assert_eq!(f.namespace, MappingNamespace::Named);
    assert_eq!(f.entries.len(), 3);
    assert_eq!(f.entries[1].descriptor.as_deref(), Some(DESC));
    for (s, message, line) in [
        ("", "header", 1),
        ("accessWidener v1 named", "header", 1),
        ("accessWidener v2 missing", "namespace", 1),
        ("accessWidener v2 named # comment", "header", 1),
        ("accessWidener v2 named\nbogus class X", "directive", 2),
        ("accessWidener v2 named\naccessible bad X", "kind", 2),
        ("accessWidener v2 named\naccessible class", "owner", 2),
        (
            "accessWidener v2 named\naccessible method X m (V)V",
            "descriptor",
            2,
        ),
        (
            "accessWidener v2 named\naccessible field X f V",
            "descriptor",
            2,
        ),
        (
            "accessWidener v2 named\ntransitive-accessible class X",
            "directive",
            2,
        ),
    ] {
        let e = AccessWidenerParser::parse(s).unwrap_err();
        assert!(e.message.contains(message), "{s}: {e:?}");
        assert_eq!(e.line, line);
    }
    for d in ["accessible", "extendable", "mutable", "natural"] {
        assert!(AccessWidenerParser::parse(&format!("{HEADER}{d} class X # comment\r\n")).is_ok());
    }
}
#[test]
fn at_parser_all_modifiers_class_field_method_and_errors() {
    let f = AccessTransformerParser::parse(&format!(
        "public {FQN}\nprotected-f {FQN} currentScreen\npublic+f {FQN} setScreen{DESC} # comment"
    ))
    .unwrap();
    assert_eq!(f.entries.len(), 3);
    assert!(f.entries[0].name.is_none());
    assert_eq!(f.entries[2].descriptor.as_deref(), Some(DESC));
    for m in AccessTransformerModifier::ALL {
        assert_eq!(
            AccessTransformerParser::parse(&format!("{} X", m.token()))
                .unwrap()
                .entries[0]
                .modifier,
            m
        );
    }
    for (s, m) in [
        ("bogus X", "modifier"),
        ("public", "entry"),
        ("public X a b", "entry"),
        ("public X m(V)V", "descriptor"),
    ] {
        assert!(AccessTransformerParser::parse(s)
            .unwrap_err()
            .message
            .contains(m));
    }
    assert!(AccessTransformerParser::parse(" # comment\n\r\n")
        .unwrap()
        .entries
        .is_empty());
}
#[test]
fn aw_context_all_slots_trailing_whitespace_and_replacement() {
    let cases = [
        ("acc|essible class X", AwSyntaxSlot::Directive, "acc"),
        ("accessible cl|ass X", AwSyntaxSlot::Kind, "cl"),
        (
            "accessible class net/Mine|craft",
            AwSyntaxSlot::Owner,
            "net/Mine",
        ),
        (
            "accessible method X set|Screen ()V",
            AwSyntaxSlot::Name,
            "set",
        ),
        (
            "accessible method X m (I|)V",
            AwSyntaxSlot::Descriptor,
            "(I",
        ),
        ("|", AwSyntaxSlot::Directive, ""),
        ("accessible |", AwSyntaxSlot::Kind, ""),
        ("accessible method |", AwSyntaxSlot::Owner, ""),
        ("accessible method X |", AwSyntaxSlot::Name, ""),
        ("accessible method X m |", AwSyntaxSlot::Descriptor, ""),
    ];
    for (s, slot, p) in cases {
        let input = format!("{HEADER}{s}");
        let c = aw_context(&input);
        assert_eq!(c.slot, slot, "{input}");
        assert_eq!(c.partial_value, p);
        let source = input.replace('|', "");
        assert!(source.is_char_boundary(c.value_start_offset));
        assert!(source.is_char_boundary(c.value_end_offset));
    }
    let c = aw_context("accessWidener v2 nam|ed");
    assert_eq!(c.slot, AwSyntaxSlot::HeaderNamespace);
    assert_eq!(c.partial_value, "nam");
    assert_eq!(
        AwContextExtractor::parse_file_namespace("accessWidener v2 intermediary # ok"),
        Some(MappingNamespace::Intermediary)
    );
    assert!(AwContextExtractor::extract_at_offset("", 0).is_none());
    assert!(AwContextExtractor::extract("a", 99, 0).is_none());
}
#[test]
fn at_context_all_slots_trailing_comment_and_partial_descriptor() {
    for (s, slot, p) in [
        ("pub|lic X", AtSlot::Modifier, "pub"),
        ("public net.Mine|", AtSlot::Owner, "net.Mine"),
        ("public X set|Screen()V", AtSlot::MemberName, "set"),
        ("public X m(I|)V", AtSlot::MemberDescriptor, "(I"),
        ("public |", AtSlot::Owner, ""),
        ("public X |", AtSlot::MemberName, ""),
        ("public X # comment|", AtSlot::MemberName, ""),
        ("|", AtSlot::Modifier, ""),
    ] {
        let c = at_context(s);
        assert_eq!(c.slot, slot, "{s}");
        assert_eq!(c.partial_value, p);
    }
    let l = AtContextExtractor::parse_line("public-f X f_1", 1).unwrap();
    assert_eq!(l.modifier.unwrap().text, "public-f");
    assert_eq!(l.owner.unwrap().text, "X");
    assert_eq!(l.member.unwrap().text, "f_1");
    assert!(AtContextExtractor::extract("line", 5, 0).is_none());
    assert!(AtContextExtractor::extract_at_offset("line", 99).is_none());
}
#[test]
fn unicode_offsets_and_crlf_are_checked() {
    let text = "accessWidener v2 named\r\n# 😀\r\naccessible class 日本/Target";
    let offset = text.find("Target").unwrap() + 3;
    let pos = offset_to_position(text, offset).unwrap();
    assert_eq!(position_to_offset(text, pos), Some(offset));
    let c = AwContextExtractor::extract(text, pos.line, pos.character).unwrap();
    assert_eq!(c.partial_value, "日本/Tar");
    assert_eq!(c.line_number, 3);
    assert!(AwContextExtractor::extract_at_offset(text, text.find('日').unwrap() + 1).is_none());
    assert!(position_to_offset(
        "😀",
        McTextPosition {
            line: 0,
            character: 1
        }
    )
    .is_none());
}
#[test]
fn aw_completion_keywords_kinds_namespaces() {
    let idx = Index::default();
    let svc = AccessWidenerCompletionService::new(&idx);
    for (s, expected) in [
        ("|", vec!["accessible", "extendable", "mutable", "natural"]),
        ("acc|", vec!["accessible"]),
        ("accessible cl|", vec!["class"]),
        ("mutable |", vec!["field"]),
        ("extendable |", vec!["class"]),
    ] {
        let c = aw_context(&format!("{HEADER}{s}"));
        let items = svc.complete(&c, None, Default::default());
        assert_eq!(
            items
                .iter()
                .map(|i| i.insert_text.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        assert!(items.iter().all(|i| i.metadata.source.starts_with("aw.")));
    }
    let items = svc.complete(
        &aw_context("accessWidener v2 nam|ed"),
        None,
        Default::default(),
    );
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, McCompletionKind::Value);
    assert_eq!(items[0].sort_key, "0000_named");
    assert_eq!(items[0].metadata.namespace.as_deref(), Some("NAMED"));
}
#[test]
fn aw_completion_owner_member_descriptor_display_and_mapping() {
    let idx = Index::default();
    let m = mappings(MappingNamespace::Intermediary, false);
    let svc = AccessWidenerCompletionService::new(&idx);
    let c = aw_context(&format!(
        "{HEADER}accessible class net/minecraft/client/Mine|craftClient"
    ));
    let i = svc.complete(&c, None, Default::default()).remove(0);
    assert_eq!(i.label, "MinecraftClient");
    assert_eq!(i.insert_text, OWNER);
    assert_eq!(i.detail.as_deref(), Some(OWNER));
    assert_eq!(i.documentation.as_deref(), Some(FQN));
    assert_eq!(i.metadata.source, "aw.class");
    assert_eq!(
        &format!("{HEADER}accessible class net/minecraft/client/MinecraftClient")
            [c.value_start_offset..c.value_end_offset],
        OWNER
    );
    let c = aw_context("accessWidener v2 intermediary\naccessible class Mine|");
    assert_eq!(
        svc.complete(&c, Some(&m), Default::default())[0].insert_text,
        "net/minecraft/client/class_310"
    );
    for (k, n, d) in [
        ("method", "setScreen", DESC),
        ("field", "currentScreen", FD),
    ] {
        let c = aw_context(&format!("{HEADER}accessible {k} {OWNER} {n}|"));
        let i = svc.complete(&c, None, Default::default()).remove(0);
        assert_eq!(i.insert_text, n);
        assert!(i.label.contains(n));
        assert_eq!(i.metadata.descriptor.as_deref(), Some(d));
        let c = aw_context(&format!("{HEADER}accessible {k} {OWNER} {n} |"));
        let i = svc.complete(&c, None, Default::default()).remove(0);
        assert_eq!(i.insert_text, d);
        assert_eq!(i.metadata.source, "aw.descriptor");
        assert_eq!(
            i.kind,
            if k == "method" {
                McCompletionKind::Value
            } else {
                McCompletionKind::Field
            }
        );
    }
    let c = aw_context(
        "accessWidener v2 intermediary\naccessible method net/minecraft/client/class_310 set|",
    );
    assert_eq!(
        svc.complete(&c, Some(&m), Default::default())[0].insert_text,
        "method_91152"
    );
    let c = aw_context(&format!("{HEADER}accessible method missing/X x|"));
    assert!(svc.complete(&c, None, Default::default()).is_empty());
}
#[test]
fn at_completion_modifiers_owners_mapped_members_and_empty_slots() {
    let idx = Index::default();
    let forge = mappings(MappingNamespace::Srg, false);
    let fabric = mappings(MappingNamespace::Intermediary, false);
    let plain = AccessTransformerCompletionService::new(&idx, Some(&fabric));
    assert_eq!(plain.complete(&at_context("|")).len(), 10);
    assert_eq!(plain.complete(&at_context("pub|")).len(), 3);
    let item = plain
        .complete(&at_context("public net.minecraft.client.Mine|"))
        .remove(0);
    assert_eq!(item.insert_text, FQN);
    assert_eq!(item.metadata.namespace.as_deref(), Some("NAMED"));
    for (ctx, insert) in [
        (format!("public {FQN} set|"), format!("setScreen{DESC}")),
        (format!("public {FQN} current|"), "currentScreen".into()),
    ] {
        assert_eq!(plain.complete(&at_context(&ctx))[0].insert_text, insert);
    }
    let svc = AccessTransformerCompletionService::new(&idx, Some(&forge));
    for (ctx, insert) in [
        (
            format!("public {FQN} set|"),
            "m_91152_(Lnet/minecraft/client/gui/Screen;)V",
        ),
        (format!("public {FQN} current|"), "f_91074_"),
    ] {
        let i = svc.complete(&at_context(&ctx)).remove(0);
        assert_eq!(i.insert_text, insert);
        assert!(i.detail.unwrap().starts_with("named:"));
        assert!(i.metadata.source.starts_with("at.member."));
    }
    assert!(plain
        .complete(&at_context(&format!("public {FQN} setScreen(|")))
        .is_empty());
    assert!(plain
        .complete(&at_context("public Missing set|"))
        .is_empty());
}
#[test]
fn at_insert_formatter_mapping_missing_and_owner() {
    let m = mappings(MappingNamespace::Srg, false);
    let f = mappings(MappingNamespace::Intermediary, false);
    let tick = method("tick", "()V");
    assert_eq!(
        AtMemberInsertFormatter::format_method_insert(&tick, OWNER, Some(&m)).insert_text,
        "m_91398_()V"
    );
    assert_eq!(
        AtMemberInsertFormatter::format_method_insert(&tick, OWNER, Some(&f)).insert_text,
        "tick()V"
    );
    assert!(
        !AtMemberInsertFormatter::format_method_insert(&method("unknown", "()V"), OWNER, Some(&m))
            .mapping_found
    );
    assert_eq!(
        AtMemberInsertFormatter::format_owner_insert(&Index::default().find_class(OWNER).unwrap()),
        FQN
    );
}
#[test]
fn aw_diagnostics_all_codes_partial_lines_and_duplicates() {
    let idx = Index::default();
    assert!(aw_codes(&valid_aw(), &idx, None).is_empty());
    for (entry, code) in [
        ("bogus class X", AwDiagnosticCodes::INVALID_DIRECTIVE),
        ("accessible unknown X", AwDiagnosticCodes::INVALID_KIND),
        (
            "accessible class missing/X",
            AwDiagnosticCodes::UNRESOLVED_CLASS,
        ),
        (
            "accessible method net/minecraft/client/MinecraftClient missing ()V",
            AwDiagnosticCodes::UNRESOLVED_MEMBER,
        ),
        (
            "accessible field net/minecraft/client/MinecraftClient currentScreen I",
            AwDiagnosticCodes::INVALID_DESCRIPTOR,
        ),
        (
            "mutable class net/minecraft/client/MinecraftClient",
            AwDiagnosticCodes::MUTABLE_ON_NON_FIELD,
        ),
        (
            "extendable method net/minecraft/client/MinecraftClient tick ()V",
            AwDiagnosticCodes::EXTENDABLE_INVALID_TARGET,
        ),
        (
            "accessible method X m (V)V",
            AwDiagnosticCodes::INVALID_DESCRIPTOR,
        ),
    ] {
        let source = format!("{HEADER}{entry}");
        assert!(
            aw_codes(&source, &idx, None).contains(&code.to_string()),
            "{source}"
        );
    }
    let s = format!("{HEADER}accessible class {OWNER}\naccessible class {OWNER}");
    let ds = AccessWidenerDiagnosticsService::new(&idx).analyze(&s, "", None);
    assert_eq!(ds.len(), 2);
    assert!(ds
        .iter()
        .all(|d| d.code == AwDiagnosticCodes::DUPLICATE_ENTRY
            && d.metadata.contains_key("line")
            && d.metadata.contains_key("key")));
    let m = mappings(MappingNamespace::Intermediary, false);
    let source = "accessWidener v2 official\naccessible class net/minecraft/client/class_310";
    assert!(
        aw_codes(source, &idx, Some(&m)).contains(&AwDiagnosticCodes::NAMESPACE_MISMATCH.into())
    );
    assert!(AccessWidenerDiagnosticsService::new(&idx)
        .analyze("accessWidener v2 named\n  bogus\tbad X", "", None)
        .iter()
        .any(|d| d.code == AwDiagnosticCodes::INVALID_KIND && d.range.start.character == 8));
}
#[test]
fn at_diagnostics_all_codes_and_missing_mapping() {
    let idx = Index::default();
    let forge = mappings(MappingNamespace::Srg, false);
    let missing = mappings(MappingNamespace::Srg, true);
    for (s, code) in [
        ("bad X".to_string(), AtDiagnosticCodes::INVALID_MODIFIER),
        ("public".to_string(), AtDiagnosticCodes::INVALID_MODIFIER),
        (
            "public Missing".to_string(),
            AtDiagnosticCodes::UNRESOLVED_CLASS,
        ),
        (
            format!("public {FQN} unknown"),
            AtDiagnosticCodes::UNRESOLVED_MEMBER,
        ),
        (
            format!("public {FQN} tick"),
            AtDiagnosticCodes::MISSING_METHOD_DESCRIPTOR,
        ),
        (
            format!("public {FQN} tick(I)V"),
            AtDiagnosticCodes::INVALID_DESCRIPTOR,
        ),
        (
            format!("public {FQN} tick(V)V"),
            AtDiagnosticCodes::INVALID_DESCRIPTOR,
        ),
    ] {
        assert!(at_codes(&s, &idx, None).contains(&code.to_string()), "{s}");
    }
    for s in [
        format!("public {FQN}"),
        format!("public {FQN} currentScreen"),
        format!("public {FQN} tick()V"),
    ] {
        assert!(at_codes(&s, &idx, None).is_empty());
    }
    assert_eq!(
        at_codes(&format!("public {FQN} tick()V"), &idx, Some(&forge)),
        vec![AtDiagnosticCodes::WRONG_NAMESPACE]
    );
    assert!(at_codes(&format!("public {FQN} m_91398_()V"), &idx, Some(&forge)).is_empty());
    assert_eq!(
        at_codes(&format!("public {FQN} tick()V"), &idx, Some(&missing)),
        vec![AtDiagnosticCodes::SRG_MAPPING_NOT_FOUND]
    );
    let s = format!("public {FQN}\npublic {FQN}");
    let ds = AccessTransformerDiagnosticsService::new(&idx, None).analyze(&s, "");
    assert_eq!(ds.len(), 1);
    assert_eq!(ds[0].metadata["duplicateOfLine"], "1");
}
#[test]
fn overload_policy_exact_later_overload_missing_and_mismatch() {
    let idx = Index {
        methods: vec![method("run", "()V"), method("run", "(I)V")],
        ..Default::default()
    };
    for (d, code) in [
        ("(I)V", None),
        ("(J)V", Some(AtDiagnosticCodes::INVALID_DESCRIPTOR)),
        ("", Some(AtDiagnosticCodes::MISSING_METHOD_DESCRIPTOR)),
    ] {
        let s = format!("public {FQN} run{d}");
        let ds = at_codes(&s, &idx, None);
        assert_eq!(ds, code.into_iter().map(str::to_string).collect::<Vec<_>>());
    }
    assert_eq!(
        at_codes(&format!("public {FQN} absent()V"), &idx, None),
        vec![AtDiagnosticCodes::UNRESOLVED_MEMBER]
    );
    let svc = AccessTransformerDefinitionService::new(&idx, None);
    let awsvc = AccessWidenerDefinitionService::new(&idx, None);
    for (d, expected) in [("(I)V", 1), ("(J)V", 0), ("", 0)] {
        let s = format!("public {FQN} run{d}");
        let at = s.find("run").unwrap() + 1;
        assert_eq!(svc.definitions_at_offset(&s, at).len(), expected);
        let s = format!("{HEADER}accessible method {OWNER} run {d}");
        let at = s.find("run").unwrap() + 1;
        assert_eq!(awsvc.definitions_at_offset(&s, at).len(), expected);
    }
}
#[test]
fn definitions_class_field_method_directive_and_wrong_namespace() {
    let idx = Index::default();
    let aw = AccessWidenerDefinitionService::new(&idx, None);
    let at = AccessTransformerDefinitionService::new(&idx, None);
    for (s, token, kind) in [
        (
            format!("{HEADER}accessible class {OWNER}"),
            "MinecraftClient",
            MemberKind::Class,
        ),
        (
            format!("{HEADER}accessible method {OWNER} tick ()V"),
            "tick",
            MemberKind::Method,
        ),
        (
            format!("{HEADER}accessible field {OWNER} currentScreen {FD}"),
            "currentScreen",
            MemberKind::Field,
        ),
    ] {
        let defs = aw.definitions_at_offset(&s, s.find(token).unwrap() + 1);
        assert_eq!(defs[0].kind, kind);
        assert_eq!(defs[0].owner_internal_name, OWNER);
        assert!(aw.definitions_at_offset(&s, HEADER.len() + 1).is_empty());
    }
    for (s, token, kind) in [
        (
            format!("public {FQN}"),
            "MinecraftClient",
            MemberKind::Class,
        ),
        (format!("public {FQN} tick()V"), "tick", MemberKind::Method),
        (
            format!("public {FQN} currentScreen"),
            "currentScreen",
            MemberKind::Field,
        ),
    ] {
        assert_eq!(
            at.definitions_at_offset(&s, s.find(token).unwrap() + 1)[0].kind,
            kind
        );
        assert!(at.definitions_at_offset(&s, 1).is_empty());
    }
    let m = mappings(MappingNamespace::Srg, false);
    let svc = AccessTransformerDefinitionService::new(&idx, Some(&m));
    let s = format!("public {FQN} m_91398_()V");
    assert_eq!(
        svc.definitions_at_offset(&s, s.find("m_91398_").unwrap() + 1)[0]
            .name
            .as_deref(),
        Some("tick")
    );
}
#[test]
fn editors_format_keys_insert_remove_replace_preserve_terminators() {
    let aw = AccessWidenerParser::parse(&valid_aw()).unwrap();
    for e in &aw.entries {
        let f = AccessWidenerEditor::format_entry(e);
        assert!(AccessWidenerParser::parse(&format!("{HEADER}{f}")).is_ok());
    }
    let mut class = aw.entries[0].clone();
    let key = AccessWidenerEditor::entry_key(&class);
    class.name = Some("ignored".into());
    class.descriptor = Some("I".into());
    assert_eq!(AccessWidenerEditor::entry_key(&class), key);
    assert_ne!(AccessWidenerEditor::entry_key(&aw.entries[1]), key);
    let text = "accessWidener v2 named\r\n# 😀 comment\r\n";
    let added = AccessWidenerEditor::insert_entry(text, &class, None);
    assert!(added.starts_with(text));
    assert!(added.ends_with("\r\n"));
    assert_eq!(AccessWidenerEditor::remove_line(&added, 3), text);
    assert_eq!(AccessWidenerEditor::replace_line(text, 99, "bad"), text);
    let entries = AccessTransformerParser::parse(&format!(
        "public {FQN}\nprivate-f {FQN} currentScreen\nprotected+f {FQN} tick()V"
    ))
    .unwrap()
    .entries;
    for e in entries {
        let f = AccessTransformerEditor::format_entry(&e);
        assert_eq!(
            AccessTransformerParser::parse(&f).unwrap().entries[0].descriptor,
            e.descriptor
        );
        assert!(!AccessTransformerEditor::replace_line(&f, 1, &f).changed);
        let appended = AccessTransformerEditor::append_entry("# 😀\r\n", &e);
        assert!(appended.content.starts_with("# 😀\r\n"));
        assert!(appended.content.ends_with("\r\n"));
        assert_eq!(
            AccessTransformerEditor::remove_line(&appended.content, 2).content,
            "# 😀\r\n"
        );
    }
}
#[test]
fn aw_actions_duplicate_descriptor_generate_apply_and_reparse() {
    let idx = Index::default();
    let source = format!("{HEADER}accessible method {OWNER} tick (I)V # 😀 keep\r\n");
    let ds = AccessWidenerDiagnosticsService::new(&idx).analyze(&source, "", None);
    let fixes = AccessWidenerCodeActionService::fixes_for_diagnostics(
        &ds,
        "file:///test.aw",
        &source,
        Some(&idx),
        None,
    );
    assert_eq!(fixes.len(), 1);
    assert_eq!(fixes[0].kind(), "quickfix.aw.fixDescriptor");
    let fixed = apply(
        AccessWidenerCodeActionService::apply_fix(&fixes[0], &source).unwrap(),
        &source,
    );
    assert!(fixed.ends_with(" # 😀 keep\r\n"));
    assert_eq!(
        AccessWidenerParser::parse(&fixed).unwrap().entries[0]
            .descriptor
            .as_deref(),
        Some("()V")
    );
    assert!(aw_codes(&fixed, &idx, None).is_empty());
    let source = format!("{HEADER}accessible class {OWNER}\naccessible class {OWNER}\n");
    let ds = AccessWidenerDiagnosticsService::new(&idx).analyze(&source, "", None);
    let fixes = AccessWidenerCodeActionService::fixes_for_diagnostics(
        &ds,
        "file:///test.aw",
        &source,
        Some(&idx),
        None,
    );
    let fixed = apply(
        AccessWidenerCodeActionService::apply_fix(&fixes[1], &source).unwrap(),
        &source,
    );
    assert_eq!(AccessWidenerParser::parse(&fixed).unwrap().entries.len(), 1);
    assert!(aw_codes(&fixed, &idx, None).is_empty());
    let entry = AccessWidenerParser::parse(&valid_aw())
        .unwrap()
        .entries
        .remove(0);
    let fix =
        AccessWidenerCodeActionService::generate_entry_fix("file:///test.aw", &entry, Some(2));
    let source = "accessWidener v2 named\r\n# keep\r\n";
    let generated = apply(
        AccessWidenerCodeActionService::apply_fix(&fix, source).unwrap(),
        source,
    );
    assert_eq!(
        AccessWidenerParser::parse(&generated)
            .unwrap()
            .entries
            .len(),
        1
    );
    assert!(generated.contains("# keep\r\n"));
}
#[test]
fn aw_namespace_remap_action_applies_and_reparses() {
    let idx = Index::default();
    let m = mappings(MappingNamespace::Intermediary, false);
    let source =
        "accessWidener v2 named\r\n  accessible class net/minecraft/client/class_310 # 😀\r\n";
    let ds = AccessWidenerDiagnosticsService::new(&idx).analyze(source, "", Some(&m));
    let fixes = AccessWidenerCodeActionService::fixes_for_diagnostics(
        &ds,
        "file:///test.aw",
        source,
        Some(&idx),
        Some(&m),
    );
    let fixed = apply(
        AccessWidenerCodeActionService::apply_fix(&fixes[0], source).unwrap(),
        source,
    );
    assert_eq!(
        AccessWidenerParser::parse(&fixed).unwrap().entries[0].owner,
        OWNER
    );
    assert!(fixed.contains("  accessible class"));
    assert!(fixed.ends_with(" # 😀\r\n"));
    assert!(aw_codes(&fixed, &idx, Some(&m)).is_empty());
}
#[test]
fn at_actions_missing_descriptor_duplicate_generate_and_stale_check() {
    let idx = Index::default();
    let svc = AccessTransformerCodeActionService::new(&idx, None);
    let source = format!("# 😀\r\n  public\t{FQN}\ttick # keep\r\n");
    let ds = AccessTransformerDiagnosticsService::new(&idx, None).analyze(&source, "");
    let fixes = svc.fixes_for_diagnostics(&ds, "file:///test.at", &source);
    assert_eq!(fixes.len(), 1);
    assert_eq!(fixes[0].kind(), "quickfix.at.addDescriptor");
    let fixed = apply(svc.apply_fix(&fixes[0], &source).unwrap(), &source);
    assert!(fixed.contains(&format!("  public\t{FQN}\ttick()V # keep\r\n")));
    assert!(at_codes(&fixed, &idx, None).is_empty());
    assert!(svc
        .apply_fix(&fixes[0], &source.replace("tick", "other"))
        .is_none());
    let source = format!("public {FQN}\npublic {FQN}\n");
    let ds = AccessTransformerDiagnosticsService::new(&idx, None).analyze(&source, "");
    let fixes = svc.fixes_for_diagnostics(&ds, "file:///test.at", &source);
    let fixed = apply(svc.apply_fix(&fixes[0], &source).unwrap(), &source);
    assert_eq!(
        AccessTransformerParser::parse(&fixed)
            .unwrap()
            .entries
            .len(),
        1
    );
    let c = at_context(&format!("public-f {FQN} currentScreen|"));
    let fix =
        AccessTransformerCodeActionService::generate_entry_fix(&c, "file:///test.at").unwrap();
    let text = apply(svc.apply_fix(&fix, "# keep\r\n").unwrap(), "# keep\r\n");
    assert_eq!(
        AccessTransformerParser::parse(&text).unwrap().entries[0].modifier,
        AccessTransformerModifier::PublicRemoveFinal
    );
}
#[test]
fn at_remap_actions_exact_overload_and_missing_mapping() {
    let idx = Index::default();
    let m = mappings(MappingNamespace::Srg, false);
    let svc = AccessTransformerCodeActionService::new(&idx, Some(&m));
    let source = format!("  public {FQN}\ttick()V # keep\r\n");
    let ds = AccessTransformerDiagnosticsService::new(&idx, Some(&m)).analyze(&source, "");
    let fixes = svc.fixes_for_diagnostics(&ds, "file:///test.at", &source);
    let fixed = apply(svc.apply_fix(&fixes[0], &source).unwrap(), &source);
    assert!(fixed.contains("\tm_91398_()V # keep\r\n"));
    assert!(at_codes(&fixed, &idx, Some(&m)).is_empty());
    let overloaded = Index {
        methods: vec![method("tick", "(I)V"), method("tick", "()V")],
        ..Default::default()
    };
    let svc = AccessTransformerCodeActionService::new(&overloaded, Some(&m));
    for (d, offered) in [("", false), ("(J)V", false), ("()V", true)] {
        let source = format!("public {FQN} tick{d}");
        let ds =
            AccessTransformerDiagnosticsService::new(&overloaded, Some(&m)).analyze(&source, "");
        let fs = svc.fixes_for_diagnostics(&ds, "file:///test.at", &source);
        assert_eq!(!fs.is_empty(), offered, "{source}");
        if offered {
            let result = apply(svc.apply_fix(&fs[0], &source).unwrap(), &source);
            assert!(result.ends_with("m_91398_()V"));
        }
    }
    let empty = mappings(MappingNamespace::Srg, true);
    let svc = AccessTransformerCodeActionService::new(&idx, Some(&empty));
    let source = format!("public {FQN} tick()V");
    let ds = AccessTransformerDiagnosticsService::new(&idx, Some(&empty)).analyze(&source, "");
    let fs = svc.fixes_for_diagnostics(&ds, "file:///test.at", &source);
    assert_eq!(fs.len(), 1);
    assert!(svc.apply_fix(&fs[0], &source).is_none());
}
#[test]
fn references_crossfile_owner_member_descriptor_and_missing() {
    let idx = Index::default();
    let s = valid_aw();
    let defs = AccessWidenerDefinitionService::new(&idx, None);
    let sources=vec![SourceScanEntry{document_uri:"file:///test.aw".into(),text:s.clone()},SourceScanEntry{document_uri:"file:///test.at".into(),text:format!("public {FQN}\npublic {FQN} currentScreen\npublic {FQN} setScreen{DESC}\npublic {FQN} setScreen()V")},SourceScanEntry{document_uri:"file:///ignore.java".into(),text:s.clone()}];
    let facade = AwAtServiceFacade::new(&idx, None);
    for (token, aw_count, at_count) in [
        ("MinecraftClient", 3, 1),
        ("setScreen", 1, 1),
        ("currentScreen", 1, 1),
    ] {
        let target = defs
            .definitions_at_offset(&s, s.find(token).unwrap() + 1)
            .remove(0);
        assert_eq!(
            AccessWidenerReferenceService::new(&idx, None)
                .find_references(&target, &sources[..1])
                .len(),
            aw_count
        );
        assert_eq!(
            AccessTransformerReferenceService::new(&idx)
                .find_references(&target, &sources[1..2])
                .len(),
            at_count
        );
        assert_eq!(
            facade.find_references(&target, &sources).len(),
            aw_count + at_count
        );
        let mut missing = target.clone();
        missing.owner_internal_name = "missing/X".into();
        missing.owner_fqn = Some("missing.X".into());
        assert!(facade.find_references(&missing, &sources).is_empty());
    }
}
#[test]
fn routing_and_facade_completion_diagnostics_definitions_actions() {
    for (lang, path, expected) in [
        (
            "accesswidener",
            "file:///x",
            Some(AwAtFileType::AccessWidener),
        ),
        (
            "accesstransformer",
            "file:///x",
            Some(AwAtFileType::AccessTransformer),
        ),
        (
            "",
            "file:///x.accesswidener",
            Some(AwAtFileType::AccessWidener),
        ),
        ("", "file:///x.aw", Some(AwAtFileType::AccessWidener)),
        (
            "",
            "file:///foo_at.cfg",
            Some(AwAtFileType::AccessTransformer),
        ),
        (
            "",
            "file:///accesstransformer.cfg",
            Some(AwAtFileType::AccessTransformer),
        ),
        ("", "file:///x.at", Some(AwAtFileType::AccessTransformer)),
        ("java", "file:///x.java", None),
    ] {
        assert_eq!(AwAtBufferSupport::detect_file_type(lang, path), expected);
    }
    let idx = Index::default();
    let svc = AwAtServiceFacade::new(&idx, None);
    for (kind, text, insert) in [
        (
            AwAtFileType::AccessWidener,
            format!("{HEADER}acc"),
            "accessible",
        ),
        (AwAtFileType::AccessTransformer, "pub".into(), "public"),
    ] {
        assert!(svc
            .complete_at_offset(kind, &text, text.len())
            .iter()
            .any(|i| i.insert_text == insert));
    }
    let text = format!("public {FQN} tick");
    let pos = offset_to_position(&text, text.len()).unwrap();
    let r = AwAtFacadeRequest {
        buffer_text: &text,
        line: pos.line,
        character: pos.character,
        document_uri: "file:///test.at",
        file_type: AwAtFileType::AccessTransformer,
    };
    assert_eq!(
        svc.diagnose(&r)[0].code,
        AtDiagnosticCodes::MISSING_METHOD_DESCRIPTOR
    );
    assert_eq!(svc.code_actions(&r, None).len(), 1);
    assert!(svc.code_actions(&r, Some("other")).is_empty());
    assert_eq!(svc.definitions_at(&r)[0].name.as_deref(), Some("tick"));
    let fix = svc.code_actions(&r, None).remove(0);
    let result = apply(svc.apply_fix(&fix, &text).unwrap(), &text);
    assert!(svc.analyze(r.file_type, &result, r.document_uri).is_empty());
}
#[test]
fn fixture_facade_bytecode_index_mapped_completions_and_diagnostics() {
    use mcdev_core::{
        bytecode::{BytecodeClassIndexAdapter, ClassMemberIndex},
        mapping::TinyV2Parser,
    };
    let bytes = include_bytes!("fixtures/awat-simple-target.class.bin");
    let index = BytecodeClassIndexAdapter {
        member_index: ClassMemberIndex::index_class(bytes, None, None).unwrap(),
    };
    let set = TinyV2Parser::parse(include_str!(
        "../../mcdev-test-fixtures/src/main/resources/fixtures/fabric-aw-at/mappings.tiny"
    ))
    .unwrap();
    let m = ProjectMappingContext {
        source_namespace: MappingNamespace::Named,
        runtime_namespace: MappingNamespace::Intermediary,
        aw_namespace: Some(MappingNamespace::Named),
        at_namespace: Some(MappingNamespace::Intermediary),
        available_namespaces: set.namespaces.iter().copied().collect(),
        resolver: Arc::new(MappingSetResolver::new(set)),
    };
    let svc = AwAtServiceFacade::new(&index, Some(&m));
    let aw=include_str!("../../mcdev-test-fixtures/src/main/resources/fixtures/fabric-aw-at/src/main/resources/mod.accesswidener");
    let at=include_str!("../../mcdev-test-fixtures/src/main/resources/fixtures/fabric-aw-at/src/main/resources/mod_at.cfg");
    let cases=[(AwAtFileType::AccessWidener,"accessWidener v2 named\nacc","accessible","aw.directive"),(AwAtFileType::AccessWidener,"accessWidener v2 named\naccessible cl","class","aw.kind"),(AwAtFileType::AccessWidener,"accessWidener v2 named\naccessible class com/example/target/Simp","com/example/target/SimpleTarget","aw.class"),(AwAtFileType::AccessWidener,"accessWidener v2 named\naccessible method com/example/target/SimpleTarget dr","draw","aw.method"),(AwAtFileType::AccessWidener,"accessWidener v2 named\nmutable field com/example/target/SimpleTarget cou","counter","aw.field"),(AwAtFileType::AccessTransformer,"pub","public","at.modifier"),(AwAtFileType::AccessTransformer,"public com.example.target.Simp","com.example.target.SimpleTarget","at.class"),(AwAtFileType::AccessTransformer,"public com.example.target.SimpleTarget dr","method_1(Ljava/lang/String;FF)V","at.member.method"),(AwAtFileType::AccessWidener,"accessWidener v2 named\naccessible method com/example/target/SimpleTarget draw (Ljava/lang/Str","(Ljava/lang/String;FF)V","aw.descriptor"),(AwAtFileType::AccessTransformer,"public com.example.target.SimpleTarget cou","field_1","at.member.field")];
    for (kind, text, insert, source) in cases {
        assert!(
            svc.complete_at_offset(kind, text, text.len())
                .iter()
                .any(|c| c.insert_text == insert && c.metadata.source == source),
            "{text}"
        );
    }
    for (kind, text) in [
        (AwAtFileType::AccessWidener, aw),
        (AwAtFileType::AccessTransformer, at),
    ] {
        assert!(svc
            .analyze(kind, text, "")
            .iter()
            .all(|d| !d.code.contains("UNRESOLVED") && !d.code.contains("unresolved")));
    }
    for (token, kind, descriptor) in [
        ("com/example/target/SimpleTarget", MemberKind::Class, None),
        ("draw", MemberKind::Method, Some("(Ljava/lang/String;FF)V")),
        ("counter", MemberKind::Field, Some("I")),
    ] {
        let offset = aw.find(token).unwrap() + token.len();
        let pos = offset_to_position(aw, offset).unwrap();
        let target = svc
            .definitions_at(&AwAtFacadeRequest {
                buffer_text: aw,
                line: pos.line,
                character: pos.character,
                document_uri: "file:///test.aw",
                file_type: AwAtFileType::AccessWidener,
            })
            .remove(0);
        assert_eq!(target.kind, kind);
        assert_eq!(target.descriptor.as_deref(), descriptor);
    }
    let text = "accessWidener v2 named\naccessible method com/example/target/SimpleTarget static";
    assert_eq!(
        svc.complete_at_offset(AwAtFileType::AccessWidener, text, text.len())[0].insert_text,
        "staticMethod"
    );
    let duplicate="accessWidener v2 named\naccessible class com/example/target/SimpleTarget\naccessible class com/example/target/SimpleTarget";
    let r = AwAtFacadeRequest {
        buffer_text: duplicate,
        line: 0,
        character: 0,
        document_uri: "file:///mod.aw",
        file_type: AwAtFileType::AccessWidener,
    };
    assert!(matches!(
        svc.code_actions(&r, Some(AwDiagnosticCodes::DUPLICATE_ENTRY))[0],
        McFix::RemoveDuplicateAccessWidenerEntryFix { .. }
    ));
    let local = AwAtServiceFacade::new(&index, None);
    let text = "public com.example.target.SimpleTarget staticMethod";
    let r = AwAtFacadeRequest {
        buffer_text: text,
        line: 0,
        character: 0,
        document_uri: "file:///mod.at",
        file_type: AwAtFileType::AccessTransformer,
    };
    let fix = local
        .code_actions(&r, Some(AtDiagnosticCodes::MISSING_METHOD_DESCRIPTOR))
        .remove(0);
    let changed = apply(local.apply_fix(&fix, text).unwrap(), text);
    assert!(local
        .analyze(r.file_type, &changed, r.document_uri)
        .is_empty());
}
#[test]
fn missing_slot_replacement_never_overwrites_modifier_or_owner() {
    let idx = Index::default();
    let svc = AwAtServiceFacade::new(&idx, None);
    for(marked,expected)in[("public |","public net.minecraft.client.MinecraftClient"),("public net.minecraft.client.MinecraftClient |","public net.minecraft.client.MinecraftClient setScreen(Lnet/minecraft/client/gui/screen/Screen;)V")]{let at=marked.find('|').unwrap();let source=marked.replace('|',"");let c=AtContextExtractor::extract_at_offset(&source,at).unwrap();assert_eq!(c.value_start_offset,at);assert_eq!(c.value_end_offset,at);let items=svc.complete_at_offset(AwAtFileType::AccessTransformer,&source,at);let selected=items.iter().find(|i|expected.ends_with(&i.insert_text)).unwrap();assert_eq!(apply_text_edits(&source,&[McTextEdit{start_offset:c.value_start_offset,end_offset:c.value_end_offset,new_text:selected.insert_text.clone()}]).unwrap(),expected);}
}
#[test]
fn add_aw_descriptor_fix_and_full_whitespace_remapping() {
    let text = "accessWidener v2 named\naccessible method X m # keep\r\n";
    let at = text.find(" # keep").unwrap();
    let fix = McFix::AddAccessWidenerDescriptorFix {
        title: "Add".into(),
        document_uri: "file:///test.aw".into(),
        start_offset: at,
        end_offset: at,
        descriptor: "()V".into(),
    };
    let result = apply(
        AccessWidenerCodeActionService::apply_fix(&fix, text).unwrap(),
        text,
    );
    assert_eq!(
        AccessWidenerParser::parse(&result).unwrap().entries[0]
            .descriptor
            .as_deref(),
        Some("()V")
    );
    assert!(result.ends_with(" # keep\r\n"));
    let idx = Index::default();
    let m = mappings(MappingNamespace::Intermediary, false);
    let source="accessWidener v2 named\r\n\taccessible\tclass  net/minecraft/client/class_310\t  # keep 😀\r\n";
    let ds = AccessWidenerDiagnosticsService::new(&idx).analyze(source, "", Some(&m));
    let fix = AccessWidenerCodeActionService::fixes_for_diagnostics(
        &ds,
        "file:///test.aw",
        source,
        Some(&idx),
        Some(&m),
    )
    .remove(0);
    let fixed = apply(
        AccessWidenerCodeActionService::apply_fix(&fix, source).unwrap(),
        source,
    );
    assert_eq!(
        fixed,
        source.replace("net/minecraft/client/class_310", OWNER)
    );
}
#[test]
fn remaining_legacy_helper_and_action_cases() {
    let idx = Index::default();
    assert_eq!(
        AccessWidenerEditor::format_class_entry(AccessWidenerDirective::Accessible, OWNER),
        format!("accessible class {OWNER}")
    );
    assert_eq!(
        AccessWidenerEditor::format_method_entry(
            AccessWidenerDirective::Accessible,
            OWNER,
            "setScreen",
            DESC
        ),
        format!("accessible method {OWNER} setScreen {DESC}")
    );
    assert_eq!(
        AccessWidenerEditor::format_field_entry(
            AccessWidenerDirective::Mutable,
            OWNER,
            "currentScreen",
            FD
        ),
        format!("mutable field {OWNER} currentScreen {FD}")
    );
    assert!(!AccessWidenerEditor::remove_line(&valid_aw(), 3).contains("setScreen"));
    let e = AccessWidenerParser::parse(&valid_aw())
        .unwrap()
        .entries
        .remove(0);
    assert_eq!(
        AccessWidenerEditor::entry_key(&e),
        format!("ACCESSIBLE|CLASS|{OWNER}")
    );
    assert_eq!(AwContextExtractor::to_offset("a\nbc", 1, 1), Some(3));
    assert_eq!(
        AwContextExtractor::parse_file_namespace(&valid_aw()),
        Some(MappingNamespace::Named)
    );
    let blank = "accessWidener v2 named\n\naccessible class Foo";
    assert_eq!(
        AwContextExtractor::extract_at_offset(blank, blank.find("\n\n").unwrap() + 1)
            .unwrap()
            .slot,
        AwSyntaxSlot::Directive
    );
    let game = AccessWidenerCompletionService::new(&idx)
        .complete(
            &aw_context(&format!("{HEADER}accessible class Game|")),
            None,
            Default::default(),
        )
        .remove(0);
    assert_eq!(game.label, "GameRenderer");
    assert_eq!(
        game.detail.as_deref(),
        Some("net/minecraft/client/render/GameRenderer")
    );
    for (text,kind,name) in [("accessWidener v2 named\naccessible method net/minecraft/client/MinecraftClient tick",MemberKind::Method,"tick"),("accessWidener v2 named\naccessible field net/minecraft/client/MinecraftClient currentScreen",MemberKind::Field,"currentScreen")] {let def=AccessWidenerDefinitionService::new(&idx,None).definitions_at_offset(text,text.len()).remove(0);assert_eq!(def.kind,kind);assert_eq!(def.name.as_deref(),Some(name));}
    let mut e = AccessTransformerParser::parse(&format!("public {FQN} tick()V"))
        .unwrap()
        .entries
        .remove(0);
    let key = AccessTransformerEditor::entry_key(&e);
    e.descriptor = Some("(I)V".into());
    assert_ne!(AccessTransformerEditor::entry_key(&e), key);
    let c = at_context(&format!("public {FQN}|"));
    let generated =
        AccessTransformerCodeActionService::generate_entry_fix(&c, "file:///test.at").unwrap();
    let text = apply(
        AccessTransformerCodeActionService::new(&idx, None)
            .apply_fix(&generated, "")
            .unwrap(),
        "",
    );
    assert_eq!(text, format!("public {FQN}\n"));
    let m = mappings(MappingNamespace::Srg, false);
    let fix = McFix::RemapAccessTransformerEntryFix {
        title: "Remap".into(),
        document_uri: "file:///test.at".into(),
        line: 1,
    };
    let svc = AccessTransformerCodeActionService::new(&idx, Some(&m));
    let source = format!("public {FQN} tick");
    let changed = apply(svc.apply_fix(&fix, &source).unwrap(), &source);
    assert_eq!(changed, format!("public {FQN} m_91398_()V"));
    let overload = Index {
        methods: vec![method("tick", "(I)V"), method("tick", "()V")],
        ..Default::default()
    };
    assert!(AccessTransformerCodeActionService::new(&overload, Some(&m))
        .apply_fix(&fix, &format!("public {FQN} tick(J)V"))
        .is_none());
}
#[test]
fn explicit_entry_remap_retains_missing_mapping_information_in_identity_namespace() {
    let mut m = mappings(MappingNamespace::Named, true);
    m.at_namespace = Some(MappingNamespace::Named);
    m.resolver = Arc::new(EmptyMappingResolver);
    assert!(
        AtMemberInsertFormatter::format_method_insert(&method("tick", "()V"), OWNER, Some(&m))
            .mapping_found
    );
    assert!(
        !AtMemberInsertFormatter::remap_method_for_entry(OWNER, &method("tick", "()V"), &m)
            .mapping_found
    );
    assert!(
        !AtMemberInsertFormatter::remap_field_for_entry(OWNER, &field("currentScreen", FD), &m)
            .mapping_found
    );
}
#[test]
fn namespace_fallback_without_data_does_not_invent_names() {
    let idx = Index::default();
    let m = ProjectMappingContext {
        source_namespace: MappingNamespace::Named,
        runtime_namespace: MappingNamespace::Srg,
        aw_namespace: Some(MappingNamespace::Intermediary),
        at_namespace: Some(MappingNamespace::Srg),
        available_namespaces: Default::default(),
        resolver: Arc::new(EmptyMappingResolver),
    };
    assert_eq!(
        AwNamespaceHelper::insert_owner(
            OWNER,
            Some(MappingNamespace::Intermediary),
            Some(m.resolver.as_ref())
        ),
        OWNER
    );
    assert!(AwNamespaceHelper::resolve_owner_in_index("missing/X", None, &idx, Some(&m)).is_none());
    assert!(!AwNamespaceHelper::has_namespace_mismatch(
        "missing/X",
        None,
        &idx,
        Some(&m)
    ));
    assert_eq!(
        AtMemberInsertFormatter::format_field_insert(&field("currentScreen", FD), OWNER, Some(&m))
            .insert_text,
        "currentScreen"
    );
}
