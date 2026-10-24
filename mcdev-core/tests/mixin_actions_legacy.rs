#![allow(non_snake_case)]
mod mixin_validation_support;
use mcdev_core::{
    mixin_actions::MixinCodeActionService, mixin_diagnostics::MixinDiagnosticCodes as C,
    mixin_edits::MixinConfigEditor, model::*,
};
use mixin_validation_support::*;
fn config_fix(source: &str, name: &str) -> McFix {
    let idx = LegacyIndex::default();
    let svc = MixinCodeActionService::new(&idx);
    let mut d = diag(C::MIXIN_CLASS_NOT_LISTED_IN_CONFIG, "mixinClass", name);
    d.severity = McSeverity::Warning;
    d.range = McTextRange {
        start: McTextPosition {
            line: 0,
            character: 0,
        },
        end: McTextPosition {
            line: 0,
            character: 10,
        },
    };
    let fixes = svc.fixes_for_diagnostics(
        &[d],
        "file:///Mixin.java",
        source,
        None,
        Some("mixins.json"),
        Some("example.mixin"),
        None,
    );
    assert_eq!(fixes.len(), 1);
    fixes.into_iter().next().unwrap()
}
fn array(f: &McFix) -> &str {
    match f {
        McFix::AddMixinConfigEntryFix { array_name, .. } => array_name,
        _ => panic!(),
    }
}
fn direct_config_fix(name: &str, package: Option<&str>) -> McFix {
    McFix::AddMixinConfigEntryFix {
        title: "Add mixin".into(),
        config_path: "mixins.json".into(),
        mixin_class_name: name.into(),
        mixin_package: package.map(str::to_string),
        array_name: "mixins".into(),
    }
}
#[test]
fn producesAddMixinConfigFix() {
    let idx = LegacyIndex::default();
    let d = diag(
        C::MIXIN_CLASS_NOT_LISTED_IN_CONFIG,
        "mixinClass",
        "ExampleMixin",
    );
    let fixes = MixinCodeActionService::new(&idx).fixes_for_diagnostics(
        &[d],
        "file:///Mixin.java",
        "@Mixin(MinecraftClient.class) class ExampleMixin {}",
        Some(r#"{ "mixins": [] }"#),
        Some("mixins.json"),
        Some("example.mixin"),
        None,
    );
    assert_eq!(fixes.len(), 1);
    let McFix::AddMixinConfigEntryFix {
        mixin_class_name,
        config_path,
        array_name,
        ..
    } = &fixes[0]
    else {
        panic!()
    };
    assert_eq!(mixin_class_name, "ExampleMixin");
    assert_eq!(config_path, "mixins.json");
    assert_eq!(array_name, "mixins");
}
#[test]
fn addMixinConfigFixUsesClientArrayForFabricClient() {
    let source =
        "@Environment(EnvType.CLIENT)\n@Mixin(MinecraftClient.class)\nclass ClientMixin {}";
    assert_eq!(array(&config_fix(source, "ClientMixin")), "client");
}
#[test]
fn addMixinConfigFixUsesServerArrayForNeoForgeDedicatedServer() {
    let source="@net.neoforged.api.distmarker.OnlyIn(net.neoforged.api.distmarker.Dist.DEDICATED_SERVER)\n@Mixin(MinecraftServer.class)\nclass ServerMixin {}";
    assert_eq!(array(&config_fix(source, "ServerMixin")), "server");
}
#[test]
fn addMixinConfigFixUsesMixinsArrayWhenNoSideAnnotationPresent() {
    let source = "@Mixin(MinecraftClient.class)\nclass ExampleMixin {}";
    assert_eq!(array(&config_fix(source, "ExampleMixin")), "mixins");
}
#[test]
fn addMixinConfigFixUsesMixinsArrayForConflictingSideAnnotations() {
    let source="@Environment(EnvType.CLIENT)\n@OnlyIn(Dist.DEDICATED_SERVER)\n@Mixin(MinecraftClient.class)\nclass ExampleMixin {}";
    assert_eq!(array(&config_fix(source, "ExampleMixin")), "mixins");
}
#[test]
fn applyMixinConfigFixTargetsClientArrayFromFix() {
    let source =
        "@Environment(EnvType.CLIENT)\n@Mixin(MinecraftClient.class)\nclass ClientMixin {}";
    let fix = config_fix(source, "ClientMixin");
    let content = r#"{ "client": ["ExistingClientMixin"] }"#;
    let edit = MixinCodeActionService::apply_mixin_config_fix(&fix, content).unwrap();
    let McFix::WorkspaceEditFix { metadata, .. } = &edit else {
        panic!()
    };
    assert_eq!(metadata["arrayName"], "client");
    assert_eq!(
        apply(edit, content),
        r#"{ "client": ["ExistingClientMixin", "ClientMixin"] }"#
    );
}
#[test]
fn applyMixinConfigFixTargetsServerArrayFromFix() {
    let source =
        "@OnlyIn(Dist.DEDICATED_SERVER)\n@Mixin(MinecraftServer.class)\nclass ServerMixin {}";
    let fix = config_fix(source, "ServerMixin");
    let content = r#"{ "server": ["ExistingServerMixin"] }"#;
    let edit = MixinCodeActionService::apply_mixin_config_fix(&fix, content).unwrap();
    let McFix::WorkspaceEditFix { metadata, .. } = &edit else {
        panic!()
    };
    assert_eq!(metadata["arrayName"], "server");
    assert_eq!(
        apply(edit, content),
        r#"{ "server": ["ExistingServerMixin", "ServerMixin"] }"#
    );
}
#[test]
fn applyMixinConfigFixAddsEntryDeterministically() {
    let fix = direct_config_fix("ExampleMixin", Some("example.mixin"));
    let content = r#"{ "mixins": ["AlphaMixin"] }"#;
    let edit = MixinCodeActionService::apply_mixin_config_fix(&fix, content).unwrap();
    let McFix::WorkspaceEditFix { edits, .. } = &edit else {
        panic!()
    };
    assert!(edits[0].new_text.contains("ExampleMixin"));
    let index = |s: &str| edits[0].new_text.find(s).map_or(-1, |v| v as i64);
    assert!(index("AlphaMixin") < index("ExampleMixin"));
    let final_text = apply(edit, content);
    assert!(final_text.find("AlphaMixin").unwrap() < final_text.find("ExampleMixin").unwrap());
}
fn lossless_delta(content: &str) {
    let fix = direct_config_fix("BetaMixin", None);
    let result = MixinConfigEditor::add_entry(content, "BetaMixin", "mixins").unwrap();
    let edit = MixinCodeActionService::apply_mixin_config_fix(&fix, content).unwrap();
    let McFix::WorkspaceEditFix { edits, .. } = &edit else {
        panic!()
    };
    assert_eq!(edits.len(), 1);
    let e = &edits[0];
    assert_eq!(e.start_offset, e.end_offset);
    assert!(e.start_offset > 0);
    assert!(e.end_offset < content.len());
    assert_eq!(apply_text_edits(content, edits).unwrap(), result.content);
    assert_eq!(
        &content[..e.start_offset],
        &result.content[..e.start_offset]
    );
    assert_eq!(
        &content[e.end_offset..],
        &result.content[e.start_offset + e.new_text.len()..]
    );
}
#[test]
fn applyMixinConfigFixUsesLosslessDeltaForJson5Comments() {
    lossless_delta("{\n  \"mixins\": [\"AlphaMixin\", /* keep trailing comma */ ,]\n}");
}
#[test]
fn applyMixinConfigFixUsesLosslessDeltaForCrlf() {
    lossless_delta("{\r\n  \"mixins\": [\"AlphaMixin\"]\r\n}");
}
#[test]
fn applyMixinConfigFixSkipsDuplicate() {
    let fix = direct_config_fix("ExampleMixin", None);
    assert!(MixinCodeActionService::apply_mixin_config_fix(
        &fix,
        r#"{ "mixins": ["ExampleMixin"] }"#
    )
    .is_none());
}
#[test]
fn doesNotGuessDescriptorForAmbiguousMethod() {
    let source="@Mixin(MinecraftClient.class)\nclass M {\n    @Inject(method = \"render\", at = @At(\"HEAD\"))\n    void m() {}\n}";
    let idx = LegacyIndex::default();
    let mut d = diag(C::AMBIGUOUS_INJECT_METHOD, "method", "render");
    d.severity = McSeverity::Warning;
    d.range = McTextRange {
        start: McTextPosition {
            line: 2,
            character: 20,
        },
        end: McTextPosition {
            line: 2,
            character: 28,
        },
    };
    assert!(MixinCodeActionService::new(&idx)
        .fixes_for_diagnostics(
            &[d],
            "file:///Mixin.java",
            source,
            None,
            None,
            None,
            Some(&idx)
        )
        .is_empty());
}
#[test]
fn producesGenerateAccessorFix() {
    let idx = LegacyIndex::default();
    let source = "@Mixin(MinecraftClient.class) class M { }";
    let fixes = MixinCodeActionService::new(&idx).fixes_for_diagnostics(
        &[diag(C::ACCESSOR_FIELD_NOT_FOUND, "field", "currentScreen")],
        "file:///Mixin.java",
        source,
        None,
        None,
        None,
        Some(&idx),
    );
    assert!(fixes.iter().any(|f| f.title().contains("Accessor")));
    let changed = apply(
        MixinCodeActionService::apply_fix(&fixes[0], source).unwrap(),
        source,
    );
    assert!(changed.contains("getCurrentScreen"));
}
#[test]
fn producesGenerateInvokerFix() {
    let idx = LegacyIndex::default();
    let source = "@Mixin(MinecraftClient.class) class M { }";
    let fixes = MixinCodeActionService::new(&idx).fixes_for_diagnostics(
        &[diag(C::INVOKER_METHOD_NOT_FOUND, "name", "setScreen")],
        "file:///Mixin.java",
        source,
        None,
        None,
        None,
        Some(&idx),
    );
    assert!(fixes.iter().any(|f| f.title().contains("Invoker")));
    let changed = apply(
        MixinCodeActionService::apply_fix(&fixes[0], source).unwrap(),
        source,
    );
    assert!(changed.contains("invokeSetScreen"));
}
#[test]
fn doesNotOfferQuickfixForUnresolvedHandlerDescriptor() {
    let idx = LegacyIndex::default();
    assert!(MixinCodeActionService::new(&idx)
        .fixes_for_diagnostics(
            &[diag(C::UNRESOLVED_HANDLER_DESCRIPTOR, "type", "Widget")],
            "file:///Mixin.java",
            "@Mixin(MinecraftClient.class) class M { }",
            None,
            None,
            None,
            Some(&idx)
        )
        .is_empty());
}
