mod mixin_validation_support;
use mcdev_core::{mixin_diagnostics::*, model::*};
use mixin_validation_support::*;
fn analyze(source: &str) -> Vec<McDiagnostic> {
    MixinDiagnosticsService::new(&Index::default(), &Bytecode)
        .analyze(&MixinDiagnosticRequest::new(source, "file:///M.java"))
}
fn code(source: &str, c: &str) -> bool {
    analyze(source).iter().any(|d| d.code == c)
}
#[test]
fn mixin_targets_resolved_unknown_fullyqualified_string_array_duplicate() {
    for s in [
        "@Mixin(UnknownClass.class) class M {}",
        "@Mixin(targets=\"missing.Missing\") class M {}",
        "@Mixin(missing.Missing.class) class M {}",
        "@Mixin({missing.Missing.class}) class M {}",
    ] {
        assert!(
            code(s, MixinDiagnosticCodes::UNRESOLVED_MIXIN_TARGET),
            "{s}"
        );
    }
    for s in [
        source(""),
        format!("@Mixin({}.class) class M {{}}", OWNER.replace('/', ".")),
    ] {
        assert!(!code(&s, MixinDiagnosticCodes::UNRESOLVED_MIXIN_TARGET));
    }
    let s = "@Mixin(targets={\"x.Target\",\"y.Target\"}) class M {}";
    let ds = analyze(s);
    let d = ds
        .iter()
        .find(|d| d.code == MixinDiagnosticCodes::DUPLICATE_MIXIN_TARGET)
        .unwrap();
    assert_eq!(d.severity, McSeverity::Warning);
    assert_eq!(d.metadata["target"], "Target");
    assert_eq!(position_to_offset(s, d.range.start), Some(0));
}
#[test]
fn config_relative_entries_subpackage_missing_and_duplicate_ranges() {
    let idx = Index::default();
    let svc = MixinDiagnosticsService::new(&idx, &Bytecode);
    let s = "package com.example.mixin.client; @Mixin(MinecraftClient.class) class ClientMixin {}";
    for (config, missing) in [
        (
            r#"{"package":"com.example.mixin","client":["client.ClientMixin"]}"#,
            false,
        ),
        (
            r#"{"package":"com.example.mixin","client":["ClientMixin"]}"#,
            true,
        ),
        (r#"{"package":"com.example.mixin","mixins":[]}"#, true),
    ] {
        let mut r = MixinDiagnosticRequest::new(s, "M.java");
        r.mixin_class_name = Some("ClientMixin");
        r.mixin_package = Some("com.example.mixin.client");
        r.mixin_config_content = Some(config);
        r.mixin_config_path = Some("mixins.json");
        let ds = svc.analyze(&r);
        let d = ds
            .iter()
            .find(|d| d.code == MixinDiagnosticCodes::MIXIN_CLASS_NOT_LISTED_IN_CONFIG);
        assert_eq!(d.is_some(), missing);
        if let Some(d) = d {
            assert_eq!(d.metadata["mixinClass"], "client.ClientMixin");
            assert_eq!(d.metadata["configPath"], "mixins.json");
        }
    }
    let config="{\r\n  \"package\":\"com.example.mixin.client\",\r\n  \"mixins\":[\"ClientMixin\",\"ClientMixin\"]\r\n}";
    let mut r = MixinDiagnosticRequest::new(s, "M.java");
    r.mixin_class_name = Some("ClientMixin");
    r.mixin_package = Some("com.example.mixin.client");
    r.mixin_config_content = Some(config);
    let d = svc
        .analyze(&r)
        .into_iter()
        .find(|d| d.code == MixinDiagnosticCodes::DUPLICATE_MIXIN_CONFIG_ENTRY)
        .unwrap();
    let start = position_to_offset(config, d.range.start).unwrap();
    let end = position_to_offset(config, d.range.end).unwrap();
    assert_eq!(&config[start..end], "ClientMixin");
    r.document_uri = "M.kt";
    assert!(!svc.analyze(&r).iter().any(|d| d.code
        == MixinDiagnosticCodes::MIXIN_CLASS_NOT_LISTED_IN_CONFIG
        || d.code == MixinDiagnosticCodes::DUPLICATE_MIXIN_CONFIG_ENTRY));
}
#[test]
fn injectors_missing_ambiguous_exact_overload_and_array() {
    for (selector, expected) in [
        (
            "missing",
            Some(MixinDiagnosticCodes::UNRESOLVED_INJECT_METHOD),
        ),
        (
            "render",
            Some(MixinDiagnosticCodes::AMBIGUOUS_INJECT_METHOD),
        ),
        ("render()V", None),
        (
            "render(J)V",
            Some(MixinDiagnosticCodes::DESCRIPTOR_MISMATCH),
        ),
        ("tick()V", None),
    ] {
        let s = source(&format!(
            "@Inject(method=\"{selector}\",at=@At(\"HEAD\")) void handler() {{}}"
        ));
        let ds = analyze(&s);
        if let Some(c) = expected {
            assert!(ds.iter().any(|d| d.code == c), "{selector}: {ds:?}");
        } else {
            assert!(ds.is_empty(), "{ds:?}");
        }
    }
    let s = source("@Inject(method={\"missing\",\"render\"},at=@At(\"HEAD\")) void handler() {}");
    assert!(code(&s, MixinDiagnosticCodes::UNRESOLVED_INJECT_METHOD));
    assert!(code(&s, MixinDiagnosticCodes::AMBIGUOUS_INJECT_METHOD));
}
#[test]
fn inject_and_at_ranges_are_absolute_with_crlf_unicode_and_concatenation() {
    for nl in ["\n", "\r\n"] {
        let s=source("// 😀\n@Inject(method=\"missing\",at=@At(value=\"INVOKE\",target=\"Lnet/minecraft/client/MinecraftClient;\"+\"absent()V\")) void h() {}").replace('\n',nl);
        let ds = analyze(&s);
        let d = ds
            .iter()
            .find(|d| d.code == MixinDiagnosticCodes::UNRESOLVED_INJECT_METHOD)
            .unwrap();
        let start = position_to_offset(&s, d.range.start).unwrap();
        let end = position_to_offset(&s, d.range.end).unwrap();
        assert!(&s[start..end].contains("missing"));
        let d = ds
            .iter()
            .find(|d| d.code == MixinDiagnosticCodes::UNRESOLVED_AT_TARGET)
            .unwrap();
        assert_eq!(d.metadata["target"], format!("L{OWNER};absent()V"));
        assert!(position_to_offset(&s, d.range.start).unwrap() > s.find("target=").unwrap());
    }
}
#[test]
fn at_targets_presence_invalid_array_ordinal_and_containing_method() {
    for (target, expected) in [
        (format!("L{OWNER};setScreen{DESC}"), None),
        (
            format!("L{OWNER};missing()V"),
            Some(MixinDiagnosticCodes::UNRESOLVED_AT_TARGET),
        ),
        (
            "not-a-descriptor".into(),
            Some(MixinDiagnosticCodes::INVALID_AT_TARGET_DESCRIPTOR),
        ),
    ] {
        let s=source(&format!("@Inject(method=\"tick()V\",at=@At(value=\"INVOKE\",target=\"{target}\")) void h() {{}}"));
        let ds = analyze(&s);
        if let Some(c) = expected {
            assert!(ds.iter().any(|d| d.code == c), "{ds:?}");
        } else {
            assert!(ds.is_empty(), "{ds:?}");
        }
    }
    let s=source(&format!("@Inject(method=\"tick()V\",at={{@At(value=\"INVOKE\",target=\"L{OWNER};absent()V\"),@At(value=\"FIELD\",target=\"L{OWNER};absent:I\")}}) void h() {{}}"));
    assert_eq!(
        analyze(&s)
            .iter()
            .filter(|d| d.code == MixinDiagnosticCodes::UNRESOLVED_AT_TARGET)
            .count(),
        2
    );
    for (ordinal, count) in [(1, 0), (2, 1), (9, 1)] {
        let s = source(&format!(
            "@Inject(method=\"tick()V\",at=@At(value=\"RETURN\",ordinal={ordinal})) void h() {{}}"
        ));
        assert_eq!(
            analyze(&s)
                .iter()
                .filter(|d| d.code == MixinDiagnosticCodes::ORDINAL_OUT_OF_RANGE)
                .count(),
            count
        );
    }
    let s=source("@Inject(method=\"render()V\",at=@At(\"HEAD\")) void first() {}\n@Inject(method=\"tick()V\",at=@At(value=\"RETURN\",ordinal=1)) void second() {}");
    assert!(!code(&s, MixinDiagnosticCodes::ORDINAL_OUT_OF_RANGE));
}
#[test]
fn unresolved_declaration_and_overwrite_validation_integrate() {
    let s = source("@Invoker void invokeSetScreen(MissingType missing);");
    assert!(code(&s, MixinDiagnosticCodes::UNRESOLVED_JAVA_TYPE));
    for (decl, c) in [
        (
            "@Overwrite public void missing() {}",
            Some(MixinDiagnosticCodes::OVERWRITE_TARGET_NOT_FOUND),
        ),
        (
            "@Overwrite public static void tick() {}",
            Some(MixinDiagnosticCodes::OVERWRITE_STATIC_MISMATCH),
        ),
        (
            "@Overwrite public int tick() {return 0;}",
            Some(MixinDiagnosticCodes::OVERWRITE_DESCRIPTOR_MISMATCH),
        ),
        ("@Overwrite public void tick() {}", None),
    ] {
        let ds = analyze(&source(decl));
        if let Some(c) = c {
            assert!(ds.iter().any(|d| d.code == c), "{ds:?}");
        } else {
            assert!(ds.is_empty(), "{ds:?}");
        }
    }
}
#[test]
fn every_original_mixin_diagnostics_case_uses_exact_input_and_assertions() {
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/mixin-diagnostics-legacy.json")).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 32);
    let mut failures = vec![];
    for row in rows {
        let id = row["id"].as_str().unwrap();
        let s = row["source"].as_str().unwrap();
        let mut idx = LegacyIndex::default();
        let mut bytes = LegacyBytecode::default();
        if id.ends_with("returnOrdinalValidationUsesContainingInjectorMethod") {
            idx.methods.insert(
                OWNER.into(),
                vec![method("foo", "()V", false), method("bar", "()V", false)],
            );
            bytes.counts =
                std::collections::BTreeMap::from([("foo".into(), 10), ("bar".into(), 2)]);
        }
        let mut r = MixinDiagnosticRequest::new(s, row["uri"].as_str().unwrap());
        r.mixin_class_name = row["mixin_class"].as_str();
        r.mixin_package = row["package"].as_str();
        r.mixin_config_content = row["config"].as_str();
        r.mixin_config_path = row["config_path"].as_str();
        let ds = MixinDiagnosticsService::new(&idx, &bytes).analyze(&r);
        for a in row["assertions"].as_array().unwrap() {
            let matches = ds
                .iter()
                .filter(|d| {
                    d.code == a["code"].as_str().unwrap()
                        && a["metadata"]
                            .as_object()
                            .unwrap()
                            .iter()
                            .all(|(k, v)| d.metadata.get(k).map(String::as_str) == v.as_str())
                        && a["message_contains"]
                            .as_str()
                            .is_none_or(|v| d.message.contains(v))
                })
                .count();
            let good = match a["kind"].as_str().unwrap() {
                "any" => matches > 0,
                "none" => matches == 0,
                "single" => matches == 1,
                _ => false,
            };
            if !good {
                failures.push(format!("{id}: assertion {a}, diagnostics={ds:?}"));
            }
        }
        if let Some(code) = row["selected_code"].as_str() {
            if let Some(d) = ds.iter().find(|d| d.code == code) {
                for (k, v) in row["metadata_equals"].as_object().unwrap() {
                    if d.metadata.get(k).map(String::as_str) != v.as_str() {
                        failures.push(format!(
                            "{id}: metadata {k} expected {v}, got {:?}",
                            d.metadata
                        ));
                    }
                }
                for check in row["range_checks"].as_array().unwrap() {
                    let text = if check["document"] == "config" {
                        r.mixin_config_content.unwrap()
                    } else {
                        s
                    };
                    let needle = check["text"].as_str().unwrap();
                    let start = text.find(needle).unwrap();
                    let expected = McTextRange {
                        start: offset_to_position(text, start).unwrap(),
                        end: offset_to_position(text, start + needle.len()).unwrap(),
                    };
                    if d.range != expected {
                        failures.push(format!(
                            "{id}: expected exact range {expected:?}, got {:?}",
                            d.range
                        ));
                    }
                }
            }
        }
        if id.ends_with("returnOrdinalValidationUsesContainingInjectorMethod") {
            let ds = ds
                .iter()
                .filter(|d| d.code == MixinDiagnosticCodes::ORDINAL_OUT_OF_RANGE)
                .collect::<Vec<_>>();
            if ds.len() != 1 {
                failures.push(format!("{id}: expected one ordinal diagnostic, got {ds:?}"));
            } else {
                let base = s.find("method = \"bar\"").unwrap();
                let start = base + s[base..].find("ordinal = 5").unwrap();
                let range = McTextRange {
                    start: offset_to_position(s, start).unwrap(),
                    end: offset_to_position(s, start + "ordinal = 5".len()).unwrap(),
                };
                if ds[0].range != range {
                    failures.push(format!("{id}: expected {range:?}, got {:?}", ds[0].range));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} original-case assertions failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
