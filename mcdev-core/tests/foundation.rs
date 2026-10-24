use mcdev_core::descriptor::*;
use mcdev_core::mapping::{self, MappingLookupResult, MappingResolver, MappingSubject};
use mcdev_core::model::*;
use std::collections::BTreeMap;
use std::sync::Arc;

#[test]
fn descriptor_roundtrip_and_rendering() {
    for (input, readable) in [
        ("B", "byte"),
        ("C", "char"),
        ("D", "double"),
        ("F", "float"),
        ("I", "int"),
        ("J", "long"),
        ("S", "short"),
        ("Z", "boolean"),
        ("Ljava/lang/String;", "String"),
        ("[Ljava/lang/String;", "String[]"),
        ("[[I", "int[][]"),
        ("Lcom/example/Outer$Inner;", "Outer$Inner"),
    ] {
        let parsed = parse_field_descriptor(input).unwrap();
        assert_eq!(render(&parsed), readable);
        assert_eq!(to_descriptor(&parsed), input);
    }
    for input in [
        "()V",
        "(Ljava/lang/String;FFI)I",
        "([Ljava/lang/String;[[I)[[Ljava/lang/Object;",
        "(BCDFIJSZ)V",
    ] {
        let value = parse_method_descriptor(input).unwrap();
        assert_eq!(method_to_descriptor(&value), input);
    }
    assert_eq!(
        render_method(&parse_method_descriptor("(Ljava/lang/String;FFI)I").unwrap()),
        "(String, float, float, int): int"
    );
}
#[test]
fn invalid_descriptors_report_bounded_offsets() {
    for input in [
        "",
        "V",
        "[V",
        "Ljava/lang/String",
        "Ljava.lang.String;",
        "Lfoo[Bar;",
        "L;",
        "L   ;",
        "Iextra",
        "[",
        "[Q",
    ] {
        let error = parse_field_descriptor(input).expect_err(input);
        assert!(error.offset <= input.len());
    }
    for input in [
        "",
        "I",
        "(",
        "(I",
        "(I)",
        "(V)V",
        "(Lfoo[Bar;)V",
        "()VV",
        "()[V",
        "(Ljava/lang/String)V",
    ] {
        let error = parse_method_descriptor(input).expect_err(input);
        assert!(error.offset <= input.len());
    }
    assert_eq!(
        parse_method_descriptor("(I)").unwrap_err().message,
        "expected return descriptor"
    );
    assert_eq!(
        parse_field_descriptor("Ljava/lang/String")
            .unwrap_err()
            .offset,
        0
    );
}
#[test]
fn member_target_parsing_preserves_exact_method_and_field() {
    let method = "Lnet/minecraft/client/font/TextRenderer;draw(Ljava/lang/String;FFI)I";
    match parse_member_target(method).unwrap() {
        MemberTarget::Method {
            owner,
            name,
            descriptor,
        } => assert_eq!(
            format!("L{owner};{name}{}", method_to_descriptor(&descriptor)),
            method
        ),
        _ => panic!(),
    }
    let field="Lnet/minecraft/client/MinecraftClient;currentScreen:Lnet/minecraft/client/gui/screen/Screen;";
    match parse_member_target(field).unwrap() {
        MemberTarget::Field {
            owner,
            name,
            descriptor,
        } => assert_eq!(
            format!("L{owner};{name}:{}", to_descriptor(&descriptor)),
            field
        ),
        _ => panic!(),
    }
    assert!(
        matches!(parse_member_target("Lcom/example/Foo;<init>()V").unwrap(),MemberTarget::Method{name,..} if name=="<init>")
    );
    for value in [
        "com/example/Foo;run()V",
        "Lcom/example/Foo",
        "L;run()V",
        "Lcom/example/Foo;",
        "Lcom/example/Foo;run",
        "Lcom/example/Foo;bar:Ljava.lang.String",
    ] {
        assert!(parse_member_target(value).is_err(), "{value}");
    }
}
#[test]
fn method_selector_all_legacy_forms() {
    for (text, owner, name, desc) in [
        (
            "Lcom/example/Foo;run()V",
            Some("com/example/Foo"),
            Some("run"),
            Some("()V"),
        ),
        (
            "com.example.Foo.run()V",
            Some("com/example/Foo"),
            Some("run"),
            Some("()V"),
        ),
        (
            "com/example/Foo.run()V",
            Some("com/example/Foo"),
            Some("run"),
            Some("()V"),
        ),
        ("run", None, Some("run"), None),
        ("Load", None, Some("Load"), None),
        ("Load()V", None, Some("Load"), Some("()V")),
        ("Logger.info()V", Some("Logger"), Some("info"), Some("()V")),
        ("Lcom/example/Foo;", Some("com/example/Foo"), None, None),
        ("Lcom/example/Foo;*", Some("com/example/Foo"), None, None),
        ("run*", None, Some("run"), None),
        ("<init>*", None, Some("<init>"), None),
        ("<clinit>*", None, Some("<clinit>"), None),
        (
            "Lcom/example/Foo;<init>*",
            Some("com/example/Foo"),
            Some("<init>"),
            None,
        ),
        (
            "com.example.Foo.<clinit>*",
            Some("com/example/Foo"),
            Some("<clinit>"),
            None,
        ),
        ("*()V", None, None, Some("()V")),
        ("*", None, None, None),
        (
            "Lcom/example/Foo;<init>()V",
            Some("com/example/Foo"),
            Some("<init>"),
            Some("()V"),
        ),
        (
            "Lcom/example/Foo;<clinit>()V",
            Some("com/example/Foo"),
            Some("<clinit>"),
            Some("()V"),
        ),
        (
            "Lcom/example/Outer$Inner$1;run(I)V",
            Some("com/example/Outer$Inner$1"),
            Some("run"),
            Some("(I)V"),
        ),
        (
            "Lcom/example/Foo;process([Ljava/lang/String;)V",
            Some("com/example/Foo"),
            Some("process"),
            Some("([Ljava/lang/String;)V"),
        ),
    ] {
        let parsed = parse_method_selector(text).unwrap();
        assert_eq!(
            parsed.owner,
            owner
                .map(|s| Pattern::Exact(s.into()))
                .unwrap_or(Pattern::Any)
        );
        assert_eq!(
            parsed.name,
            name.map(|s| Pattern::Exact(s.into()))
                .unwrap_or(Pattern::Any)
        );
        assert_eq!(
            parsed.descriptor,
            desc.map(|s| Pattern::Exact(parse_method_descriptor(s).unwrap()))
                .unwrap_or(Pattern::Any)
        );
    }
    for text in [
        " com.example.Foo.run()V ",
        "com . example . Foo . run ( ) V",
        " run ",
        " * ",
        " run* ",
        " * ( ) V ",
    ] {
        assert_eq!(
            parse_method_selector(text),
            parse_method_selector(&text.replace(' ', ""))
        );
    }
}
#[test]
fn field_selector_all_legacy_forms() {
    for (text, owner, name, desc) in [
        (
            "Lcom/example/Foo;count:I",
            Some("com/example/Foo"),
            Some("count"),
            Some("I"),
        ),
        (
            "com.example.Foo.value:Ljava/lang/String;",
            Some("com/example/Foo"),
            Some("value"),
            Some("Ljava/lang/String;"),
        ),
        (
            "com/example/Foo.value:I",
            Some("com/example/Foo"),
            Some("value"),
            Some("I"),
        ),
        ("count", None, Some("count"), None),
        ("Length", None, Some("Length"), None),
        ("Length:I", None, Some("Length"), Some("I")),
        ("Logger.value:I", Some("Logger"), Some("value"), Some("I")),
        ("Lcom/example/Foo;", Some("com/example/Foo"), None, None),
        ("Lcom/example/Foo;*", Some("com/example/Foo"), None, None),
        ("count*", None, Some("count"), None),
        ("*:I", None, None, Some("I")),
        ("*", None, None, None),
        (
            "Lcom/example/Outer$Inner$1;value:I",
            Some("com/example/Outer$Inner$1"),
            Some("value"),
            Some("I"),
        ),
        (
            "Lcom/example/Foo;data:[I",
            Some("com/example/Foo"),
            Some("data"),
            Some("[I"),
        ),
        (
            "Lcom/example/Foo;matrix:[[Ljava/lang/String;",
            Some("com/example/Foo"),
            Some("matrix"),
            Some("[[Ljava/lang/String;"),
        ),
    ] {
        let parsed = parse_field_selector(text).unwrap();
        assert_eq!(
            parsed.owner,
            owner
                .map(|s| Pattern::Exact(s.into()))
                .unwrap_or(Pattern::Any)
        );
        assert_eq!(
            parsed.name,
            name.map(|s| Pattern::Exact(s.into()))
                .unwrap_or(Pattern::Any)
        );
        assert_eq!(
            parsed.descriptor,
            desc.map(|s| Pattern::Exact(parse_field_descriptor(s).unwrap()))
                .unwrap_or(Pattern::Any)
        );
    }
    for text in [
        " com.example.Foo.value:Ljava/lang/String; ",
        "com . example . Foo . value : Ljava/lang/String;",
        " count ",
        " * ",
        " count* ",
        " * : I ",
    ] {
        assert_eq!(
            parse_field_selector(text),
            parse_field_selector(&text.replace(' ', ""))
        );
    }
}
#[test]
fn selectors_reject_every_legacy_malformed_case() {
    for text in [
        "1run",
        "1run()V",
        "com.example/Foo.run()V",
        "",
        "   ",
        "\trun",
        "run\n",
        "run\u{a0}",
        "\t*",
        "Lcom/example/Foo;count:I",
        "Lcom.example.Foo;run()V",
        "Lcom/example/Foo;run()Vextra",
        "Lcom/example/Foo;run(I)",
        "run(Lfoo[Bar;)V",
        "Lcom/example/Foo;<init>(I)I",
        "Lcom/example/Foo;<clinit>(I)V",
        "run**",
        "*run",
        "Lcom/example/Foo;**",
        "run*()V",
        "Lcom/example/Foo;run*()V",
        "<init>*()V",
        "Lcom/example/Foo;<clinit>*()V",
        "Lcom/example/Foo",
        "Lcom//example/Foo;run()V",
        "com..example.Foo.run()V",
        "Lcom/example/*;run()V",
        "com.example.*.run()V",
    ] {
        assert!(parse_method_selector(text).is_err(), "{text}");
    }
    for text in [
        "1count",
        "1count:I",
        "com.example/Foo.value:I",
        "",
        "   ",
        "\tcount",
        "count\n",
        "count\u{a0}",
        "\t*",
        "Lcom/example/Foo;run()V",
        "run()V",
        "Lcom/example/Foo;<init>:I",
        "Lcom/example/Foo;<clinit>:I",
        "<init>:I",
        "<clinit>:I",
        "Lcom.example.Foo;count:I",
        "Lcom/example/Foo;count:Iextra",
        "Lcom/example/Foo;count:Ljava/lang/String",
        "Lcom/example/Foo;count:V",
        "Lcom/example/Foo;value:Ljava.lang.String;",
        "value:Lfoo[Bar;",
        "count**",
        "*count",
        "Lcom/example/Foo;**",
        "count*:I",
        "Lcom/example/Foo;count*:I",
        "Lcom/example/Foo",
        "Lcom//example/Foo;count:I",
        "com..example.Foo.count:I",
        "Lcom/example/*;count:I",
        "com.example.*.count:I",
        ":I",
        "Lcom/example/Foo;:I",
        "Lcom/example/Foo;count:",
        "count:",
    ] {
        assert!(parse_field_selector(text).is_err(), "{text}");
    }
}
const TINY:&str="tiny\t2\t0\tnamed\tintermediary\nc\tnet/minecraft/client/MinecraftClient\tnet/minecraft/class_310\n\tm\t(Lnet/minecraft/client/gui/screen/Screen;)V\tsetScreen\tmethod_1507\n\tf\tLnet/minecraft/client/gui/screen/Screen;\tcurrentScreen\tfield_1755\nc\tnet/minecraft/client/gui/screen/Screen\tnet/minecraft/class_437";
#[test]
fn mapping_parsing_and_remapping_legacy_cases() {
    use MappingNamespace::{Intermediary, Named, Official, Srg};
    let mappings = mapping::parse_tiny_v2(TINY).unwrap();
    let resolver = mappings.clone().as_resolver();
    assert_eq!(
        mappings.class_name("net/minecraft/client/MinecraftClient", Named, Intermediary),
        Some("net/minecraft/class_310".into())
    );
    let method = mapping::MethodRef {
        owner: "net/minecraft/client/MinecraftClient".into(),
        name: "setScreen".into(),
        descriptor: "(Lnet/minecraft/client/gui/screen/Screen;)V".into(),
        namespace: Named,
    };
    let value = resolver
        .remap_method(&method, Intermediary)
        .found()
        .unwrap();
    assert_eq!(value.owner, "net/minecraft/class_310");
    assert_eq!(value.name, "method_1507");
    assert_eq!(value.descriptor, "(Lnet/minecraft/class_437;)V");
    let field = mapping::FieldRef {
        owner: method.owner.clone(),
        name: "currentScreen".into(),
        descriptor: "Lnet/minecraft/client/gui/screen/Screen;".into(),
        namespace: Named,
    };
    let value = resolver.remap_field(&field, Intermediary).found().unwrap();
    assert_eq!(value.name, "field_1755");
    assert_eq!(value.owner, "net/minecraft/class_310");
    assert_eq!(value.descriptor, "Lnet/minecraft/class_437;");
    assert!(matches!(
        resolver.remap_class(
            &mapping::ClassRef {
                internal_name: "Unknown".into(),
                namespace: Named
            },
            Intermediary
        ),
        MappingLookupResult::Missing {
            subject: MappingSubject::Class,
            from: Named,
            to: Intermediary,
            ..
        }
    ));
    assert!(matches!(
        resolver.remap_method(
            &mapping::MethodRef {
                name: "missing".into(),
                ..method.clone()
            },
            Intermediary
        ),
        MappingLookupResult::Missing {
            subject: MappingSubject::Method,
            ..
        }
    ));
    assert!(matches!(
        resolver.remap_field(
            &mapping::FieldRef {
                name: "missing".into(),
                ..field
            },
            Intermediary
        ),
        MappingLookupResult::Missing {
            subject: MappingSubject::Field,
            ..
        }
    ));
    assert_eq!(
        resolver.remap_method(&method, Named),
        MappingLookupResult::Found(method)
    );
    assert_eq!(resolver.remap_descriptor("([[Lnet/minecraft/client/gui/screen/Screen;I)[Lnet/minecraft/client/MinecraftClient;",Named,Intermediary).found().unwrap(),"([[Lnet/minecraft/class_437;I)[Lnet/minecraft/class_310;");
    assert!(matches!(
        resolver.remap_descriptor("(V)V", Named, Intermediary),
        MappingLookupResult::InvalidDescriptor { .. }
    ));
    let srg = mapping::parse_srg(
        "# mapping\n CL: a/b c/d \nMD: a/b/m (I)V c/d/remapped (I)V\nFD: a/b/f c/d/field\n",
    )
    .unwrap();
    assert_eq!(
        srg.method_name("a/b", "m", "(I)V", Official, Srg),
        Some("remapped".into())
    );
    assert_eq!(
        srg.field_name("a/b", "f", "", Official, Srg),
        Some("field".into())
    );
}
#[test]
fn mapping_fixture_regressions() {
    use MappingNamespace::{Intermediary, Named};
    let fixture = include_str!("fixtures/fabric-basic-mappings.tiny");
    let resolver = mapping::parse_tiny_v2(fixture).unwrap().as_resolver();
    assert_eq!(
        resolver
            .remap_class(
                &mapping::ClassRef {
                    internal_name: "com/example/target/SimpleTarget".into(),
                    namespace: Named
                },
                Intermediary
            )
            .found()
            .unwrap()
            .internal_name,
        "com/example/target/class_1"
    );
    let value = resolver
        .remap_method(
            &mapping::MethodRef {
                owner: "com/example/target/SimpleTarget".into(),
                name: "draw".into(),
                descriptor: "(Ljava/lang/String;FF)V".into(),
                namespace: Named,
            },
            Intermediary,
        )
        .found()
        .unwrap();
    assert_eq!(value.name, "method_1");
    assert_eq!(value.descriptor, "(Ljava/lang/String;FF)V");
    assert_eq!(
        resolver
            .remap_field(
                &mapping::FieldRef {
                    owner: "com/example/target/SimpleTarget".into(),
                    name: "counter".into(),
                    descriptor: "I".into(),
                    namespace: Named
                },
                Intermediary
            )
            .found()
            .unwrap()
            .name,
        "field_1"
    );
    assert_eq!(
        resolver
            .remap_descriptor("(Lcom/example/target/SimpleTarget;)V", Named, Intermediary)
            .found()
            .unwrap(),
        "(Lcom/example/target/class_1;)V"
    );
}
#[test]
fn strict_mapping_rows_whitespace_namespaces_and_duplicates() {
    use MappingNamespace::{Intermediary, Named};
    for (text, line) in [
        ("", 1),
        ("tiny\t1\t0\tnamed", 1),
        ("tiny\t2\t0\tunknown", 1),
        ("tiny\t2\t0\tnamed\n\tm\t()V\trun", 2),
        ("tiny\t2\t0\tnamed\n\tf\tI\tf", 2),
        ("tiny\t2\t0\tnamed\nc\tA\tB", 2),
        ("tiny\t2\t0\tnamed\nc\tA\n\tm\t()V", 3),
        ("tiny\t2\t0\tnamed\nc\tA\n\tf\tI", 3),
        ("tiny\t2\t0\tnamed\nc\tA\n\t\tp\t0\targ", 3),
        ("tiny\t2\t0\tnamed\n # comment", 2),
    ] {
        assert_eq!(
            mapping::parse_tiny_v2(text).unwrap_err().line,
            line,
            "{text}"
        );
    }
    assert_eq!(
        mapping::parse_tiny_v2("tiny\t2\t0\tNAMED\n#comment\n  \nc\tA\n")
            .unwrap()
            .namespaces,
        vec![Named]
    );
    for text in [
        "CL: A",
        "FD: A/f",
        "FD: noOwner B/f",
        "FD: A/f B/",
        "MD: A/m ()V B/m",
        "MD: noOwner ()V B/m ()V",
        "MD: A/m ()V noOwner ()V",
        "PK: A B",
    ] {
        assert_eq!(mapping::parse_srg(text).unwrap_err().line, 1, "{text}");
    }
    let duplicate =
        mapping::parse_tiny_v2("tiny\t2\t0\tnamed\tintermediary\nc\tA\tB\nc\tA\tB").unwrap();
    assert_eq!(
        duplicate.class_name("A", Named, Intermediary),
        Some("B".into())
    );
    let conflicting =
        mapping::parse_tiny_v2("tiny\t2\t0\tnamed\tintermediary\nc\tA\tB\nc\tA\tC").unwrap();
    assert_eq!(
        conflicting.class_name("A", Named, Intermediary),
        Some("B".into())
    );
    let no_class = mapping::parse_srg("FD: A/f B/g\nMD: A/m ()V B/n ()V").unwrap();
    assert_eq!(no_class.fields[0].owner.names.len(), 1);
    assert!(mapping::CompositeMappingResolver::new(vec![]).is_err());
    assert!(mapping::as_composite_resolver(vec![]).is_err());
}
#[test]
fn composite_mapping_falls_through_missing_but_not_invalid_descriptor() {
    use MappingNamespace::{Intermediary, Named};
    let good = mapping::parse_tiny_v2(TINY).unwrap();
    let empty = Arc::new(mapping::EmptyMappingResolver);
    let resolver =
        mapping::CompositeMappingResolver::new(vec![empty, Arc::new(good.as_resolver())]).unwrap();
    assert!(resolver.namespaces().contains(&Intermediary));
    assert!(matches!(
        resolver.remap_class(
            &mapping::ClassRef {
                internal_name: "net/minecraft/client/MinecraftClient".into(),
                namespace: Named
            },
            Intermediary
        ),
        MappingLookupResult::Found(_)
    ));
    assert!(matches!(
        resolver.remap_descriptor("(I", Named, Intermediary),
        MappingLookupResult::InvalidDescriptor { .. }
    ));
}
#[test]
fn checked_unicode_positions_and_edits() {
    let text = "a😀日\r\nβ\n";
    for offset in (0..=text.len()).filter(|n| text.is_char_boundary(*n)) {
        let position = offset_to_position(text, offset).unwrap();
        assert_eq!(position_to_offset(text, position), Some(offset));
    }
    assert_eq!(
        offset_to_position(text, 5),
        Some(McTextPosition {
            line: 0,
            character: 3
        })
    );
    assert_eq!(offset_to_position(text, 2), None);
    assert_eq!(offset_to_position(text, text.len() + 1), None);
    assert_eq!(
        position_to_offset(
            text,
            McTextPosition {
                line: 0,
                character: 2
            }
        ),
        None
    );
    assert_eq!(
        position_to_offset(
            text,
            McTextPosition {
                line: 9,
                character: 0
            }
        ),
        None
    );
    assert_eq!(
        position_to_offset(
            text,
            McTextPosition {
                line: 1,
                character: 99
            }
        ),
        None
    );
    let edit = |start, end, value: &str| McTextEdit {
        start_offset: start,
        end_offset: end,
        new_text: value.into(),
    };
    assert_eq!(
        apply_text_edits("a😀日", &[edit(5, 8, "語"), edit(0, 1, "b")]).unwrap(),
        "b😀語"
    );
    for edits in [
        vec![edit(2, 3, "")],
        vec![edit(0, 99, "")],
        vec![edit(2, 1, "")],
        vec![edit(0, 5, ""), edit(1, 5, "")],
        vec![edit(1, 1, "x"), edit(1, 1, "y")],
    ] {
        assert!(apply_text_edits("a😀日", &edits).is_err());
    }
}
#[test]
fn dto_preserves_completion_metadata_and_fix_edits() {
    let item = McCompletionItem {
        label: "run".into(),
        detail: Some("detail".into()),
        documentation: Some("doc".into()),
        filter_text: "run".into(),
        insert_text: "run($0)".into(),
        kind: McCompletionKind::Method,
        sort_key: "0".into(),
        metadata: McCompletionMetadata {
            source: "mixin".into(),
            owner: Some("Foo".into()),
            name: Some("run".into()),
            descriptor: Some("()V".into()),
            namespace: Some("NAMED".into()),
        },
        additional_edits: vec![McTextEdit {
            start_offset: 0,
            end_offset: 0,
            new_text: "import X;".into(),
        }],
        insert_text_format: McCompletionInsertTextFormat::Snippet,
    };
    let value = serde_json::to_value(&item).unwrap();
    assert_eq!(value["insertTextFormat"], "SNIPPET");
    assert_eq!(value["additionalEdits"][0]["newText"], "import X;");
    assert_eq!(
        serde_json::from_value::<McCompletionItem>(value).unwrap(),
        item
    );
    let fix = McFix::WorkspaceEditFix {
        title: "fix".into(),
        kind: "quickfix".into(),
        document_uri: "file:///x".into(),
        edits: item.additional_edits,
        metadata: BTreeMap::from([("source".into(), "test".into())]),
    };
    assert_eq!(
        serde_json::from_value::<McFix>(serde_json::to_value(&fix).unwrap()).unwrap(),
        fix
    );
    assert_eq!(fix.title(), "fix");
    assert_eq!(fix.kind(), "quickfix");
}

#[test]
fn all_legacy_fix_payloads_roundtrip_without_losing_fields() {
    let title = String::from("fix");
    let uri = String::from("file:///source");
    let fixes = vec![
        McFix::AddMixinConfigEntryFix {
            title: title.clone(),
            config_path: "mixins.json".into(),
            mixin_class_name: "ClientMixin".into(),
            mixin_package: Some("example.mixin".into()),
            array_name: "client".into(),
        },
        McFix::AddMethodDescriptorFix {
            title: title.clone(),
            document_uri: uri.clone(),
            start_offset: 1,
            end_offset: 4,
            method_name: "run".into(),
            descriptor: "()V".into(),
        },
        McFix::GenerateAccessorMethodFix {
            title: title.clone(),
            document_uri: uri.clone(),
            insert_offset: 5,
            method_source: "int getCount();".into(),
            field_name: "count".into(),
            is_getter: true,
        },
        McFix::GenerateInvokerMethodFix {
            title: title.clone(),
            document_uri: uri.clone(),
            insert_offset: 5,
            method_source: "void invokeRun();".into(),
            target_method_name: "run".into(),
        },
        McFix::AddAccessTransformerEntryFix {
            title: title.clone(),
            document_uri: uri.clone(),
            modifier: "public-f".into(),
            owner: "example/Foo".into(),
            member_name: Some("run".into()),
            member_descriptor: Some("()V".into()),
            insert_line: 3,
        },
        McFix::RemapAccessTransformerEntryFix {
            title: title.clone(),
            document_uri: uri.clone(),
            line: 3,
        },
        McFix::AddAtMethodDescriptorFix {
            title: title.clone(),
            document_uri: uri.clone(),
            line: 3,
            member_name: "run".into(),
            descriptor: "()V".into(),
        },
        McFix::RemoveDuplicateAtEntryFix {
            title: title.clone(),
            document_uri: uri.clone(),
            line: 3,
        },
        McFix::GenerateAccessWidenerEntryFix {
            title: title.clone(),
            document_uri: uri.clone(),
            insert_line: 4,
            entry: "accessible class example/Foo".into(),
        },
        McFix::RemapAccessWidenerEntryFix {
            title: title.clone(),
            document_uri: uri.clone(),
            line_number: 4,
            new_line: "accessible class example/Bar".into(),
        },
        McFix::AddAccessWidenerDescriptorFix {
            title: title.clone(),
            document_uri: uri.clone(),
            start_offset: 3,
            end_offset: 4,
            descriptor: "I".into(),
        },
        McFix::FixAccessWidenerDescriptorFix {
            title: title.clone(),
            document_uri: uri.clone(),
            start_offset: 3,
            end_offset: 4,
            descriptor: "J".into(),
        },
        McFix::RemoveDuplicateAccessWidenerEntryFix {
            title: title.clone(),
            document_uri: uri.clone(),
            line_number: 4,
        },
    ];
    for fix in fixes {
        let value = serde_json::to_value(&fix).unwrap();
        assert_eq!(value["title"], "fix");
        assert_eq!(serde_json::from_value::<McFix>(value).unwrap(), fix);
        assert_eq!(fix.title(), "fix");
        assert!(fix.kind().starts_with("quickfix."));
    }
    let range = McCompletionReplacementRange {
        start_offset: 1,
        end_offset: 8,
    };
    assert_eq!(
        serde_json::to_value(range).unwrap(),
        serde_json::json!({"startOffset":1,"endOffset":8})
    );
    assert!(validate_text_edits(
        "😀",
        &[McTextEdit {
            start_offset: 0,
            end_offset: 4,
            new_text: "a".into()
        }]
    )
    .is_ok());
    assert!(validate_text_edits(
        "😀",
        &[McTextEdit {
            start_offset: 1,
            end_offset: 4,
            new_text: "a".into()
        }]
    )
    .is_err());
    assert!(parse_method_selector("Foo.").is_err());
    assert!(parse_field_selector("Foo.").is_err());
}
