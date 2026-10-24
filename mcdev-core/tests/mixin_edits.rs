use mcdev_core::{apply_text_edits, mixin_edits::*};
use std::collections::BTreeSet;
fn append(content: &str, name: &str, array: &str) -> MixinConfigEditResult {
    MixinConfigEditor::add_entry(content, name, array).unwrap()
}
fn check_delta(before: &str, result: &MixinConfigEditResult) {
    let delta = result.delta.as_ref().expect("lossless delta");
    assert_eq!(delta.start_offset, delta.end_offset);
    assert_eq!(
        format!(
            "{}{}{}",
            &before[..delta.start_offset],
            delta.new_text,
            &before[delta.end_offset..]
        ),
        result.content
    );
    assert_eq!(
        &before[..delta.start_offset],
        &result.content[..delta.start_offset]
    );
    assert_eq!(
        &before[delta.end_offset..],
        &result.content[delta.start_offset + delta.new_text.len()..]
    );
}
#[test]
fn config_parse_object_root_array_json5_and_common() {
    let content="{/* comment */\"package\":\"example.mixin\",\"mixins\":[\"B\",\"A\",],\"client\":[\"A\",\"C\"],\"common\":[\"D\"],}";
    let config = MixinConfigEditor::parse(content, "mixins.json").unwrap();
    assert_eq!(config.path, "mixins.json");
    assert_eq!(config.package_name.as_deref(), Some("example.mixin"));
    assert_eq!(config.mixins, vec!["B", "A"]);
    assert_eq!(config.client, vec!["A", "C"]);
    assert_eq!(
        MixinConfigEditor::list_mixin_classes(content).unwrap(),
        vec!["A", "B", "C", "D"]
    );
    for name in ["A", "C", "D"] {
        assert!(MixinConfigEditor::contains_entry(content, name).unwrap());
    }
    assert!(!MixinConfigEditor::contains_entry(content, "Missing").unwrap());
    assert_eq!(
        MixinConfigEditor::parse("[\"A\",\"B\"]", "")
            .unwrap()
            .mixins,
        vec!["A", "B"]
    );
    assert!(MixinConfigEditor::parse("true", "")
        .unwrap()
        .mixins
        .is_empty());
}
#[test]
fn config_preserves_order_and_idempotency() {
    let before = r#"{ "mixins": ["ZuluMixin", "AlphaMixin"] }"#;
    let result = append(before, "MikeMixin", "mixins");
    assert_eq!(
        result.content,
        r#"{ "mixins": ["ZuluMixin", "AlphaMixin", "MikeMixin"] }"#
    );
    check_delta(before, &result);
    let again = append(&result.content, "MikeMixin", "mixins");
    assert!(!again.added);
    assert_eq!(again.content, result.content);
    assert!(again.delta.is_none());
}
#[test]
fn config_lossless_ignores_comment_nested_and_string_key_text() {
    for before in [
        "{\n // \"client\":[\"CommentMixin\"],\n \"mixins\":[\"AlphaMixin\"]\n}",
        "{\"nested\":{\"mixins\":[\"Nested\"]},\"mixins\":[\"AlphaMixin\"]}",
        "{\"note\":\"mixins: [\\\"FakeMixin\\\"]\",\"mixins\":[\"AlphaMixin\"]}",
    ] {
        let result = append(before, "BetaMixin", "mixins");
        check_delta(before, &result);
        assert_eq!(
            result.content,
            before.replace("[\"AlphaMixin\"]", "[\"AlphaMixin\", \"BetaMixin\"]")
        );
    }
}
#[test]
fn config_lossless_empty_array_and_comment_only_empty() {
    for before in ["{\"mixins\":[]}", "{\"mixins\":[\n // no entries yet\n ]}"] {
        let result = append(before, "AlphaMixin", "mixins");
        check_delta(before, &result);
        assert_eq!(result.delta.as_ref().unwrap().new_text, "\"AlphaMixin\"");
        assert_eq!(
            MixinConfigEditor::parse(&result.content, "")
                .unwrap()
                .mixins,
            vec!["AlphaMixin"]
        );
    }
}
#[test]
fn config_lossless_trailing_commas_and_comments() {
    for before in [
        "{\"mixins\":[\"AlphaMixin\",],}",
        "{\"mixins\":[\"AlphaMixin\", /* keep */ ,]}",
    ] {
        let result = append(before, "BetaMixin", "mixins");
        check_delta(before, &result);
        assert_eq!(result.delta.as_ref().unwrap().new_text, "\"BetaMixin\",");
        assert!(result.content.contains("\"BetaMixin\",]"));
    }
}
#[test]
fn config_lossless_preserves_crlf_client_and_server() {
    for array in ["mixins", "client", "server"] {
        let before = format!("{{\r\n  \"{array}\": [\"Old\"]\r\n}}");
        let result = append(&before, "New", array);
        check_delta(&before, &result);
        assert_eq!(
            result.content,
            before.replace("[\"Old\"]", "[\"Old\", \"New\"]")
        );
        assert_eq!(result.array_name, array);
    }
}
#[test]
fn config_lossless_escapes_and_decodes_duplicate_unicode() {
    for name in ["Mixin\"Quote\\Slash", "日本\nMixin", "Emoji😀"] {
        let before = "{\"mixins\":[]}";
        let result = append(before, name, "mixins");
        check_delta(before, &result);
        assert!(MixinConfigEditor::contains_entry(&result.content, name).unwrap());
        assert!(!append(&result.content, name, "mixins").added);
    }
    assert!(!append("{\"mixins\":[\"\\u0041\"]}", "A", "mixins").added);
}
#[test]
fn config_lossless_missing_array_preserves_indentation_and_lf() {
    let empty = append("{}", "AlphaMixin", "mixins");
    assert_eq!(
        empty.content,
        "{\n  \"mixins\": [\n    \"AlphaMixin\"\n  ]\n}"
    );
    check_delta("{}", &empty);
    let before = "{\n  // package config\n  \"package\": \"example.mixin\",\n}";
    let result = append(before, "NewMixin", "mixins");
    assert_eq!(result.content,"{\n  // package config\n  \"package\": \"example.mixin\",\n  \"mixins\": [\n    \"NewMixin\"\n  ]\n}");
    check_delta(before, &result);
    let before = "{\r\n\t\"package\":\"a\"\r\n}";
    let result = append(before, "New", "client");
    check_delta(before, &result);
    assert!(result
        .content
        .contains(",\r\n\t\"client\": [\r\n\t  \"New\""));
}
#[test]
fn config_fallback_root_arrays_common_and_non_object() {
    let root = append("[\"Z\",\"A\"]", "M", "mixins");
    assert!(root.added);
    assert!(root.delta.is_none());
    assert_eq!(
        MixinConfigEditor::parse(&root.content, "").unwrap().mixins,
        vec!["A", "M", "Z"]
    );
    let common = append("{\"common\":[\"Z\"]}", "A", "common");
    assert_eq!(
        MixinConfigEditor::parse(&common.content, "")
            .unwrap()
            .common,
        vec!["A", "Z"]
    );
    assert!(!append(&common.content, "A", "common").added);
    assert_eq!(
        MixinConfigEditor::parse(&append("true", "A", "mixins").content, "")
            .unwrap()
            .mixins,
        vec!["A"]
    );
}
#[test]
fn config_aborts_missing_delimiters_invalid_unicode_and_wrong_array_type() {
    for content in [
        "{\"mixins\":[\"AlphaMixin\"",
        r#"{"mixins":["\u12"]}"#,
        r#"{"mixins":["\uGGGG"]}"#,
        r#"{"mixins":true}"#,
        "{\"mixins\":[]} /* unterminated",
    ] {
        assert!(
            MixinConfigEditor::add_entry(content, "B", "mixins").is_err(),
            "{content}"
        );
    }
    assert!(MixinConfigEditor::parse("{\"mixins\":true}", "").is_err());
}
#[test]
fn config_single_quoted_json5_lossless_and_parsing() {
    let before = "{'package':'example.mixin','mixins':['A',],}";
    let result = append(before, "B", "mixins");
    check_delta(before, &result);
    assert_eq!(
        MixinConfigEditor::parse(&result.content, "")
            .unwrap()
            .mixins,
        vec!["A", "B"]
    );
}

#[test]
fn config_unquoted_json5_key_preserves_unicode_comments_and_lossless_delta() {
    let before = "{\n  // keep 😀\n  mixins: ['AlphaMixin'],\n}";
    let result = append(before, "BetaMixin", "mixins");
    check_delta(before, &result);
    assert_eq!(
        result.content,
        "{\n  // keep 😀\n  mixins: ['AlphaMixin', \"BetaMixin\"],\n}"
    );
}
fn reference(source: &str, internal: &str) -> TypeReference {
    MixinImportEditBuilder::reference_for_internal_name(source, internal, &BTreeSet::new())
}
#[test]
fn java_import_edit_after_package_and_imports_idempotent() {
    for source in [
        "package example;\n\nclass Patch {}",
        "package example;\nimport other.Other;\nclass Patch {}",
    ] {
        let edit = MixinImportEditBuilder::build_import_edit(source, "target.Target").unwrap();
        let applied = apply_text_edits(source, &[edit]).unwrap();
        assert!(applied.contains("import target.Target;\n"));
        assert!(MixinImportEditBuilder::build_import_edit(&applied, "target.Target").is_none());
        assert!(applied.find("import target.Target;").unwrap() > source.find("package").unwrap());
    }
    assert!(MixinImportEditBuilder::build_import_edit(
        "package target;\nclass Patch {}",
        "target.Target"
    )
    .is_none());
    assert!(MixinImportEditBuilder::build_import_edit("class Patch {}", "Target").is_none());
    assert_eq!(
        reference("// import target.Target;\nclass Patch {}", "target/Target")
            .import_fqn
            .as_deref(),
        Some("target.Target")
    );
}
#[test]
fn import_reference_nested_conflicts_wildcards_generated_and_java_lang() {
    assert_eq!(
        reference(
            "import package.Outer.Inner;\nclass Patch {}",
            "package/Outer$Inner"
        )
        .text,
        "package.Outer.Inner"
    );
    assert!(reference("", "java/lang/String").import_fqn.is_none());
    assert_eq!(
        reference("import other.Target;\nclass Patch {}", "target/Target").text,
        "target.Target"
    );
    assert_eq!(
        reference("import other.*;\nclass Patch {}", "target/Target").text,
        "target.Target"
    );
    assert_eq!(
        MixinImportEditBuilder::reference_for_internal_name(
            "",
            "target/Target",
            &BTreeSet::from(["other/Target".into()])
        )
        .text,
        "target.Target"
    );
    assert_eq!(
        reference("package target;\nclass Patch {}", "target/Target").text,
        "Target"
    );
    assert_eq!(
        reference("import target.Target;\nclass Patch {}", "target/Target").import_fqn,
        None
    );
    assert_eq!(reference("", "target/").text, "target/");
}
#[test]
fn kotlin_import_aliases_crlf_and_collisions() {
    let source = "package example\r\n\r\nimport other.Target as Other\r\n\r\nclass Patch\r\n";
    let edit = MixinImportEditBuilder::build_import_edit(source, "target.Target").unwrap();
    let applied = apply_text_edits(source, &[edit]).unwrap();
    assert_eq!(applied,"package example\r\n\r\nimport other.Target as Other\r\nimport target.Target\r\n\r\nclass Patch\r\n");
    assert!(MixinImportEditBuilder::build_import_edit(&applied, "target.Target").is_none());
    assert!(MixinImportEditBuilder::build_import_edit(source, "other.Target").is_none());
    assert_eq!(reference(source, "other/Target").text, "Other");
    assert_eq!(
        reference("import other.Target\nclass Patch", "target/Target").text,
        "target.Target"
    );
    assert_eq!(
        reference(
            "import target.Outer.Inner as Nested\nclass Patch",
            "target/Outer$Inner"
        )
        .text,
        "Nested"
    );
    assert!(MixinImportEditBuilder::build_import_edit(
        "import target.*\nclass Patch",
        "target.Target"
    )
    .is_none());
}
#[test]
fn kotlin_imports_follow_whole_file_annotations() {
    for header in [
        "@file:Suppress(\"unused\")",
        "@file:Suppress(\n    \"unused\"\n)",
        "@file:[\n    Suppress(\"unused\")\n]",
    ] {
        let body = "@io.github.diskria.lapis.annotations.KMixin(Tar::class) class Patch\n";
        let source = format!("{header}\n\n{body}");
        let edit = MixinImportEditBuilder::build_import_edit(&source, "target.Target").unwrap();
        assert_eq!(
            apply_text_edits(&source, &[edit]).unwrap(),
            format!("{header}\nimport target.Target\n\n{body}")
        );
    }
}
#[test]
fn side_resolver_all_platforms_qualified_and_whitespace() {
    for (annotation, value, side) in [
        ("Environment", "EnvType.CLIENT", MixinSide::Client),
        ("Environment", "EnvType.SERVER", MixinSide::Server),
        ("OnlyIn", "Dist.CLIENT", MixinSide::Client),
        ("OnlyIn", "Dist.DEDICATED_SERVER", MixinSide::Server),
        ("SideOnly", "Side.CLIENT", MixinSide::Client),
        ("SideOnly", "Side.SERVER", MixinSide::Server),
        (
            "net.fabricmc.api.Environment",
            "net.fabricmc.api.EnvType.CLIENT",
            MixinSide::Client,
        ),
        (
            "net.neoforged.api.distmarker.OnlyIn",
            "net.neoforged.api.distmarker.Dist.DEDICATED_SERVER",
            MixinSide::Server,
        ),
        (
            "cpw.mods.fml.relauncher.SideOnly",
            "cpw.mods.fml.relauncher.Side.SERVER",
            MixinSide::Server,
        ),
        (
            "net.minecraftforge.fml.relauncher.SideOnly",
            "net.minecraftforge.fml.relauncher.Side.CLIENT",
            MixinSide::Client,
        ),
    ] {
        let source = format!("@{annotation} ( value = {value}, )\nclass Patch {{}}");
        assert_eq!(
            MixinSideResolver::explicit_side_from_source(&source),
            Some(side),
            "{source}"
        );
    }
}
#[test]
fn side_ignores_comments_members_strings_and_ambiguous_values() {
    for source in [
        "// @Environment(EnvType.CLIENT)\nclass Patch {}",
        "/* @OnlyIn(Dist.CLIENT) */class Patch{}",
        "class Patch { @Environment(EnvType.CLIENT) void m(){} }",
        "@Environment(CLIENT) class Patch{}",
        "@Environment(EnvType.CLIENT) @OnlyIn(Dist.DEDICATED_SERVER) class Patch{}",
        "@other.Environment(EnvType.CLIENT) class Patch{}",
        "@Environment(EnvType.CLIENT class Patch{}",
        "@Environment class Patch{}",
        "class Patch{}",
        "@Other(\"@Environment(EnvType.CLIENT)\") class Patch{}",
    ] {
        assert_eq!(
            MixinSideResolver::explicit_side_from_source(source),
            None,
            "{source}"
        );
    }
    assert_eq!(
        MixinSideResolver::explicit_side_from_source(
            "@Environment(CLIENT) @OnlyIn(Dist.CLIENT) class Patch{}"
        ),
        Some(MixinSide::Client)
    );
}
