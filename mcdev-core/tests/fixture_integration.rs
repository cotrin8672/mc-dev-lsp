#[path = "fixtures/resources/mod.rs"]
mod resources;
use mcdev_core::{
    mapping::TinyV2Parser,
    project::{ModPlatform, PlatformDetector},
    MappingNamespace,
};
use sha2::{Digest, Sha256};
#[test]
fn fixture_resource_loader_test_loads_all_required_fixture_resources() {
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/resources/manifest.json")).unwrap();
    for resource in manifest.as_array().unwrap() {
        let bytes = resources::bytes(resource["resource"].as_str().unwrap());
        assert_eq!(bytes.len() as u64, resource["bytes"].as_u64().unwrap());
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            resource["sha256"].as_str().unwrap()
        );
    }
    let original: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/resources/source_manifest.json")).unwrap();
    assert_eq!(original.as_array().unwrap().len(), 49);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    for resource in original.as_array().unwrap() {
        let bytes = std::fs::read(root.join(resource["replacement"].as_str().unwrap())).unwrap();
        assert_eq!(
            format!("{:X}", Sha256::digest(&bytes)),
            resource["sha256"].as_str().unwrap()
        );
    }
}
#[test]
fn fixture_resource_loader_test_loads_fabric_basic_build_snippet() {
    let text = resources::text("fixtures/fabric-basic/build.gradle");
    assert!(text.contains("fabric-loom"));
    assert!(text.contains("mixin:0.8.7"));
}
#[test]
fn fixture_resource_loader_test_loads_compiled_simple_target_class_bytes() {
    let bytes = resources::bytes("fixtures/shared/classes/com/example/target/SimpleTarget.class");
    assert!(bytes.len() > 4);
    assert_eq!(&bytes[..4], &[0xca, 0xfe, 0xba, 0xbe]);
}
#[test]
fn fixture_resource_loader_test_lists_resources_under_fixture_root() {
    let names = resources::list("fixtures/fabric-basic");
    assert!(names.contains(&"fixtures/fabric-basic/mixins.json"));
    assert!(
        names.contains(&"fixtures/fabric-basic/src/main/java/com/example/mixin/ExampleMixin.java")
    );
}
#[test]
fn fixture_mapping_test_parses_fabric_basic_tiny_mappings() {
    let mappings =
        TinyV2Parser::parse(resources::text("fixtures/fabric-basic/mappings.tiny")).unwrap();
    use MappingNamespace::*;
    assert_eq!(
        mappings.class_name("com/example/target/SimpleTarget", Named, Intermediary),
        Some("com/example/target/class_1".into())
    );
    assert_eq!(
        mappings.method_name(
            "com/example/target/SimpleTarget",
            "draw",
            "(Ljava/lang/String;FF)V",
            Named,
            Intermediary
        ),
        Some("method_1".into())
    );
    assert_eq!(
        mappings.field_name(
            "com/example/target/SimpleTarget",
            "counter",
            "I",
            Named,
            Intermediary
        ),
        Some("field_1".into())
    );
}
fn validated(path: &str) -> serde_json::Value {
    let value = resources::config(path);
    assert_eq!(resources::validate_config(&value), Vec::<String>::new());
    let parsed = resources::parsed_config(path).unwrap();
    assert_eq!(parsed.required, value["required"].as_bool().unwrap());
    assert_eq!(parsed.min_version, value["minVersion"].as_str().unwrap());
    assert_eq!(parsed.package_name, value["package"].as_str().unwrap());
    assert_eq!(
        parsed.compatibility_level,
        value["compatibilityLevel"].as_str().unwrap()
    );
    assert_eq!(
        parsed.mixins,
        value["mixins"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    );
    value
}
#[test]
fn fixture_mixin_config_test_validates_fabric_basic_mixin_config_structure() {
    let value = validated("fixtures/fabric-basic/mixins.json");
    assert_eq!(value["package"], "com.example.mixin");
    assert_eq!(value["mixins"], serde_json::json!(["ExampleMixin"]));
}
#[test]
fn fixture_mixin_config_test_validates_fabric_mixin_extras_config_structure() {
    assert_eq!(
        validated("fixtures/fabric-mixinextras/mixins.json")["mixins"],
        serde_json::json!(["MixinExtrasExample"])
    );
}
#[test]
fn fixture_mixin_config_test_validates_broken_diagnostics_mixin_config_structure() {
    assert!(
        validated("fixtures/broken-diagnostics/mixins.json")["mixins"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
#[test]
fn fixture_mixin_config_test_broken_diagnostics_mixin_source_contains_invalid_targets() {
    let text = resources::text(
        "fixtures/broken-diagnostics/src/main/java/com/example/mixin/BrokenMixin.java",
    );
    assert!(text.contains("missingMethod"));
    assert!(text.contains("com/example/missing/Missing"));
}
#[test]
fn fixture_mixin_config_test_fabric_mixin_extras_source_contains_mixin_extras_annotations() {
    let text = resources::text(
        "fixtures/fabric-mixinextras/src/main/java/com/example/mixin/MixinExtrasExample.java",
    );
    for annotation in [
        "@ModifyExpressionValue",
        "@ModifyReturnValue",
        "@WrapOperation",
    ] {
        assert!(text.contains(annotation));
    }
}
#[test]
fn fixture_mixin_config_test_forge_basic_fixture_contains_forge_gradle_marker() {
    let text = resources::text("fixtures/forge-basic/build.gradle");
    assert_eq!(PlatformDetector::detect(&[text.into()]), ModPlatform::Forge);
    assert!(text.contains("mixin:0.8.7"));
}
#[test]
fn fixture_mixin_config_test_neoforge_basic_fixture_contains_neo_forge_marker() {
    assert_eq!(
        PlatformDetector::detect(&[resources::text("fixtures/neoforge-basic/build.gradle").into()]),
        ModPlatform::NeoForge
    );
}
#[test]
fn fixture_mixin_config_test_validates_forge_basic_mixin_config_structure() {
    assert_eq!(
        validated("fixtures/forge-basic/mixins.json")["mixins"],
        serde_json::json!(["ForgeExampleMixin"])
    );
}
#[test]
fn fixture_mixin_config_test_validates_neo_forge_basic_mixin_config_structure() {
    assert_eq!(
        validated("fixtures/neoforge-basic/mixins.json")["mixins"],
        serde_json::json!(["NeoForgeExampleMixin"])
    );
}
