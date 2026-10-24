mod mixin_validation_support;
use mcdev_core::{
    mixin_actions::*, mixin_diagnostics::MixinDiagnosticCodes as C, mixin_edits::MixinConfigEditor,
    mixin_source::*, model::*,
};
use mixin_validation_support::*;
#[test]
fn config_actions_choose_side_and_preserve_lossless_json5_and_crlf() {
    let idx = Index::default();
    let svc = MixinCodeActionService::new(&idx);
    let d = diag(
        C::MIXIN_CLASS_NOT_LISTED_IN_CONFIG,
        "mixinClass",
        "ExampleMixin",
    );
    for (annotation, array) in [
        ("@Environment(EnvType.CLIENT)", "client"),
        ("@OnlyIn(Dist.DEDICATED_SERVER)", "server"),
        ("", "mixins"),
        (
            "@Environment(EnvType.CLIENT) @OnlyIn(Dist.DEDICATED_SERVER)",
            "mixins",
        ),
    ] {
        let source = format!("{annotation} @Mixin(MinecraftClient.class) class M {{}}");
        let fixes = svc.fixes_for_diagnostics(
            std::slice::from_ref(&d),
            "M.java",
            &source,
            None,
            Some("mixins.json"),
            Some("example.mixin"),
            Some(&idx),
        );
        assert_eq!(fixes.len(), 1);
        let McFix::AddMixinConfigEntryFix {
            array_name,
            config_path,
            mixin_class_name,
            ..
        } = &fixes[0]
        else {
            panic!()
        };
        assert_eq!(array_name, array);
        assert_eq!(config_path, "mixins.json");
        assert_eq!(mixin_class_name, "ExampleMixin");
        for nl in ["\n", "\r\n"] {
            let content = format!("{{{nl}  // keep 😀{nl}  {array}: ['AlphaMixin'],{nl}}}");
            let changed = apply(
                MixinCodeActionService::apply_fix(&fixes[0], &content).unwrap(),
                &content,
            );
            let config = MixinConfigEditor::parse(&changed, "mixins.json").unwrap();
            let names = match array {
                "client" => config.client,
                "server" => config.server,
                _ => config.mixins,
            };
            assert!(names.contains(&"ExampleMixin".into()));
            assert!(changed.contains("// keep 😀"));
            if nl == "\r\n" {
                assert_eq!(
                    changed.matches('\n').count(),
                    changed.matches("\r\n").count()
                );
            }
            assert!(MixinCodeActionService::apply_fix(&fixes[0], &changed).is_none());
        }
    }
}
#[test]
fn method_descriptor_fix_applies_to_whole_document_and_checks_boundaries() {
    let source = "// 😀\r\n@Inject(method=\"render\",at=@At(\"HEAD\"))";
    let start = source.find("render").unwrap();
    let fix = McFix::AddMethodDescriptorFix {
        title: "Add descriptor".into(),
        document_uri: "M.java".into(),
        start_offset: start,
        end_offset: start + 6,
        method_name: "render".into(),
        descriptor: "(FJZ)V".into(),
    };
    let changed = apply(
        MixinCodeActionService::apply_fix(&fix, source).unwrap(),
        source,
    );
    assert_eq!(changed, source.replace("render", "render(FJZ)V"));
    let McFix::AddMethodDescriptorFix {
        start_offset: _,
        end_offset,
        method_name,
        descriptor,
        title,
        document_uri,
    } = fix
    else {
        panic!()
    };
    let start_offset = source.find('😀').unwrap() + 1;
    assert!(MixinCodeActionService::apply_fix(
        &McFix::AddMethodDescriptorFix {
            title,
            document_uri,
            start_offset,
            end_offset,
            method_name,
            descriptor
        },
        source
    )
    .is_none());
}
#[test]
fn ambiguous_unresolved_and_missing_metadata_never_guess_unsafe_fixes() {
    let idx = Index::default();
    let svc = MixinCodeActionService::new(&idx);
    let s = source("");
    for d in [
        diag(C::AMBIGUOUS_INJECT_METHOD, "method", "render"),
        diag(C::UNRESOLVED_HANDLER_DESCRIPTOR, "type", "Widget"),
        diag(C::LOW_CONFIDENCE_MEMBER_PARSE, "field", "counter"),
        diag(C::ACCESSOR_FIELD_NOT_FOUND, "field", "absent"),
        diag(C::INVOKER_METHOD_NOT_FOUND, "name", "render"),
    ] {
        assert!(svc
            .fixes_for_diagnostics(&[d], "M.java", &s, None, None, None, Some(&idx))
            .is_empty());
    }
}
#[test]
fn generate_accessor_and_invoker_fixes_apply_and_reparse_declarations() {
    let idx = Index::default();
    let svc = MixinCodeActionService::new(&idx);
    let s = source("");
    for (code, key, name, kind) in [
        (
            C::ACCESSOR_FIELD_NOT_FOUND,
            "field",
            "counter",
            MixinMemberAnnotationKind::Accessor,
        ),
        (
            C::INVOKER_METHOD_NOT_FOUND,
            "name",
            "setScreen",
            MixinMemberAnnotationKind::Invoker,
        ),
    ] {
        let ds = vec![diag(code, key, name)];
        let fixes = svc.fixes_for_diagnostics(&ds, "M.java", &s, None, None, None, Some(&idx));
        assert_eq!(fixes.len(), 1);
        let changed = apply(
            MixinCodeActionService::apply_fix(&fixes[0], &s).unwrap(),
            &s,
        );
        let members = MixinMemberDeclarationParser::parse_members(&changed, Some(&idx));
        let member = members.iter().find(|m| m.annotation_kind == kind).unwrap();
        assert!(member.method_descriptor.is_some(), "{changed}\n{members:?}");
        assert!(changed.contains(name));
    }
}
#[test]
fn kotlin_generation_and_descriptor_literals_are_language_aware() {
    let idx = Index::default();
    let svc = MixinCodeActionService::new(&idx);
    let s="import net.minecraft.client.MinecraftClient\nimport org.spongepowered.asm.mixin.Mixin\n@Mixin(MinecraftClient::class)\ninterface M {}";
    for (code, key, name) in [
        (C::ACCESSOR_FIELD_NOT_FOUND, "field", "counter"),
        (C::INVOKER_METHOD_NOT_FOUND, "name", "tick"),
    ] {
        let fixes = svc.fixes_for_diagnostics(
            &[diag(code, key, name)],
            "M.kt",
            s,
            None,
            None,
            None,
            Some(&idx),
        );
        assert_eq!(
            fixes.len(),
            1,
            "targets={:?} imports={:?} code={code}",
            MixinTargetResolver::resolve_targets_from_source(s, &idx),
            JavaTypeDescriptorResolver::imports_for(s)
        );
        let changed = apply(MixinCodeActionService::apply_fix(&fixes[0], s).unwrap(), s);
        assert!(changed.contains("fun "));
        assert!(!changed.contains("void "));
        assert!(
            MixinMemberDeclarationParser::parse_members(&changed, Some(&idx))
                .iter()
                .any(|m| m.method_descriptor.is_some())
        );
    }
    let s = "fun x() { val x = \"render\" }";
    let at = s.find("render").unwrap();
    let f = McFix::AddMethodDescriptorFix {
        title: "Add".into(),
        document_uri: "M.kt".into(),
        start_offset: at,
        end_offset: at + 6,
        method_name: "render".into(),
        descriptor: "(Lfoo/Outer$Inner;)V".into(),
    };
    let changed = apply(MixinCodeActionService::apply_fix(&f, s).unwrap(), s);
    assert!(changed.contains("Outer\\$Inner"));
}
