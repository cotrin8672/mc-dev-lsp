//! Original ProjectModelTest and FileBasedProjectContextServiceTest assertions.
use mcdev_core::project::*;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "mcdev-project-stateful-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn dir(&self, p: &str) -> PathBuf {
        let p = self.0.join(p);
        fs::create_dir_all(&p).unwrap();
        p
    }
    fn write(&self, p: &str, text: &str) -> PathBuf {
        let p = self.0.join(p);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, text).unwrap();
        p
    }
    fn fixture(&self, name: &str) {
        fn copy(from: &Path, to: &Path) {
            fs::create_dir_all(to).unwrap();
            for e in fs::read_dir(from).unwrap() {
                let e = e.unwrap();
                if e.file_type().unwrap().is_dir() {
                    copy(&e.path(), &to.join(e.file_name()))
                } else {
                    fs::copy(e.path(), to.join(e.file_name())).unwrap();
                }
            }
        }
        copy(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/mixin_completion/project_files")
                .join(name),
            &self.0,
        )
    }
    fn install_class(&self) {
        let p = self
            .0
            .join("classpath/com/example/target/SimpleTarget.class");
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(
            p,
            include_bytes!("fixtures/mixin_completion/project_files/SimpleTarget.class.bin"),
        )
        .unwrap();
    }
    fn context(&self) -> ProjectContext {
        build_file_project_context(&self.0, None).unwrap()
    }
    fn architectury(&self) {
        for name in ["common", "fabric", "forge", "neoforge", "testmod"] {
            self.dir(&format!("src/{name}/java/com/example/{name}"));
            self.dir(&format!("src/{name}/resources"));
            let cap = name[..1].to_uppercase() + &name[1..];
            self.write(
                &format!("src/{name}/java/com/example/{name}/{cap}Mod.java"),
                &format!("package com.example.{name}; class {cap}Mod {{}}"),
            );
        }
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let p = self.0.canonicalize().unwrap();
        assert_eq!(
            p.parent(),
            Some(std::env::temp_dir().canonicalize().unwrap().as_path())
        );
        assert!(p
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("mcdev-project-stateful-"));
        fs::remove_dir_all(p).unwrap();
    }
}

#[test]
fn project_model_test_classpath_snapshot_aggregates_all_entry_types() {
    let t = Temp::new();
    let x = ClasspathSnapshot {
        project_outputs: vec![t.0.join("out")],
        dependency_jars: vec![t.0.join("lib/a.jar")],
        minecraft_jars: vec![t.0.join("minecraft.jar")],
        generated_outputs: vec![t.0.join("gen")],
        ..Default::default()
    };
    assert_eq!(x.entry_count(), 4);
    assert_eq!(
        x.all_entries(),
        vec![
            t.0.join("out"),
            t.0.join("lib/a.jar"),
            t.0.join("minecraft.jar"),
            t.0.join("gen")
        ]
    );
}
#[test]
fn project_model_test_classpath_snapshot_empty_defaults() {
    let x = ClasspathSnapshot::default();
    assert!(x.project_outputs.is_empty());
    assert!(x.dependency_jars.is_empty());
    assert_eq!(x.entry_count(), 0);
}
#[test]
fn project_model_test_source_set_context_stores_directories() {
    let t = Temp::new();
    let source = t.0.join("src/main/java");
    let resources = t.0.join("src/main/resources");
    let output = t.0.join("build/classes");
    let x = SourceSetContext {
        name: "main".into(),
        source_directories: vec![source.clone()],
        resource_directories: vec![resources],
        output_directory: Some(output.clone()),
    };
    assert_eq!(x.name, "main");
    assert_eq!(x.source_directories, vec![source]);
    assert_eq!(x.output_directory, Some(output));
}
#[test]
fn project_model_test_mixin_config_ref_defaults_to_empty_lists() {
    let t = Temp::new();
    let x = MixinConfigRef {
        path: t.0.join("mixins.json"),
        ..Default::default()
    };
    assert!(x.mixins.is_empty());
    assert!(x.client.is_empty());
    assert!(x.server.is_empty());
    assert!(x.common.is_empty());
}
#[test]
fn project_model_test_project_index_state_has_all_lifecycle_values() {
    let all = [
        ProjectIndexState::NotReady,
        ProjectIndexState::Building,
        ProjectIndexState::Ready,
        ProjectIndexState::Stale,
        ProjectIndexState::Failed,
    ];
    assert_eq!(
        all.map(|v| serde_json::to_value(v)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned())
            .into_iter()
            .collect::<BTreeSet<_>>(),
        ["NOT_READY", "BUILDING", "READY", "STALE", "FAILED"]
            .map(String::from)
            .into_iter()
            .collect()
    );
}
#[test]
fn project_model_test_mod_platform_has_supported_values() {
    let all = [
        ModPlatform::Fabric,
        ModPlatform::Forge,
        ModPlatform::NeoForge,
        ModPlatform::Unknown,
    ];
    assert_eq!(
        all.map(|v| serde_json::to_value(v)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned())
            .into_iter()
            .collect::<BTreeSet<_>>(),
        ["FABRIC", "FORGE", "NEOFORGE", "UNKNOWN"]
            .map(String::from)
            .into_iter()
            .collect()
    );
}

#[test]
fn file_based_project_context_service_test_discovers_fabric_platform_from_fixture_gradle() {
    let t = Temp::new();
    t.fixture("fabric-basic");
    assert_eq!(t.context().platform, ModPlatform::Fabric);
}
#[test]
fn file_based_project_context_service_test_discovers_mixin_config_from_fixture() {
    let t = Temp::new();
    t.fixture("fabric-basic");
    assert_eq!(t.context().mixin_configs.len(), 1);
}
#[test]
fn file_based_project_context_service_test_discovers_mappings_from_fixture() {
    let t = Temp::new();
    t.fixture("fabric-basic");
    assert!(!t.context().mappings.available_namespaces.is_empty());
}
#[test]
fn file_based_project_context_service_test_discovers_classpath_directory() {
    let t = Temp::new();
    t.fixture("fabric-basic");
    t.install_class();
    assert!(t.context().classpath.entry_count() >= 1);
}
#[test]
fn file_based_project_context_service_test_discovers_kotlin_only_source_sets_and_outputs_without_jdt(
) {
    let t = Temp::new();
    let source = t.dir("src/client/kotlin");
    let output = t.dir("build/classes/kotlin/client");
    let x = t.context();
    let c = x.source_sets.iter().find(|s| s.name == "client").unwrap();
    assert_eq!(c.source_directories, vec![source]);
    assert_eq!(c.output_directory, Some(output.clone()));
    assert!(x.classpath.project_outputs.contains(&output));
}
#[test]
fn file_based_project_context_service_test_mixed_source_set_retains_both_java_and_kotlin_classpath_outputs(
) {
    let t = Temp::new();
    let js = t.dir("src/main/java");
    let ks = t.dir("src/main/kotlin");
    let jo = t.dir("build/classes/java/main");
    let ko = t.dir("build/classes/kotlin/main");
    let x = t.context();
    assert_eq!(x.source_sets.len(), 1);
    assert_eq!(
        x.source_sets[0]
            .source_directories
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>(),
        [js, ks].into_iter().collect()
    );
    assert_eq!(x.source_sets[0].output_directory, Some(jo.clone()));
    assert!([jo, ko]
        .iter()
        .all(|p| x.classpath.project_outputs.contains(p)));
}
#[test]
fn file_based_project_context_service_test_reindex_refreshes_session() {
    let t = Temp::new();
    t.fixture("fabric-basic");
    t.install_class();
    let service = FileBasedProjectContextService::default();
    let uri = path_to_uri(&t.0);
    service.load_session(&uri, None).unwrap();
    assert_eq!(
        service.reindex(&uri, None).unwrap().context.index_state,
        ProjectIndexState::Ready
    );
}
#[test]
fn file_based_project_context_service_test_cached_session_reports_hit_and_reindex_bumps_version() {
    let t = Temp::new();
    t.fixture("fabric-basic");
    t.install_class();
    let service = FileBasedProjectContextService::default();
    let uri = path_to_uri(&t.0);
    let first = service.load_cached_session(&uri, None).unwrap();
    let second = service.load_cached_session(&uri, None).unwrap();
    service.reindex(&uri, None).unwrap();
    let after = service.load_cached_session(&uri, None).unwrap();
    assert!(!first.cache_hit);
    assert!(second.cache_hit);
    assert_eq!(first.version, second.version);
    assert!(after.version > second.version);
}
#[test]
fn file_based_project_context_service_test_empty_workspace_has_no_classpath_entries() {
    let t = Temp::new();
    let x = t.context();
    assert_eq!(x.classpath.entry_count(), 0);
    assert_eq!(x.index_state, ProjectIndexState::NotReady);
}
#[test]
fn file_based_project_context_service_test_discover_classpath_includes_minecraft_jars_from_jar_roots(
) {
    let t = Temp::new();
    let jar = t.write("libs/minecraft-client.jar", "fake");
    assert_eq!(t.context().classpath.minecraft_jars, vec![jar]);
}
#[test]
fn file_based_project_context_service_test_discovers_loom_remapped_jars_in_enhanced_classpath() {
    let t = Temp::new();
    t.write(
        ".gradle/loom-cache/remapped_working/minecraft-client-mapped.jar",
        "fake",
    );
    let x = t.context();
    assert!(!x.classpath.minecraft_jars.is_empty());
    assert!(x.classpath.minecraft_jars.iter().any(|p| p
        .file_name()
        .unwrap()
        .to_string_lossy()
        .contains("minecraft")));
}
#[test]
fn file_based_project_context_service_test_discovers_mapped_sources_for_loom_fixture() {
    let t = Temp::new();
    t.fixture("fabric-loom-e2e");
    let x = t.context();
    let main = x.source_sets.iter().find(|s| s.name == "main").unwrap();
    assert!(main
        .source_directories
        .iter()
        .any(|p| p.ends_with("mapped-sources")));
    assert!(main
        .source_directories
        .iter()
        .all(|p| !p.ends_with("mapped-sources/com/example/target")));
}
#[test]
fn file_based_project_context_service_test_discovers_main_and_client_source_sets_from_fixture() {
    let t = Temp::new();
    t.fixture("multi-source-set");
    let x = t.context();
    assert_eq!(x.source_sets.len(), 2);
    assert_eq!(
        x.source_sets
            .iter()
            .map(|s| s.name.as_str())
            .collect::<BTreeSet<_>>(),
        ["main", "client"].into_iter().collect()
    );
    for name in ["main", "client"] {
        assert!(x
            .source_sets
            .iter()
            .find(|s| s.name == name)
            .unwrap()
            .source_directories
            .iter()
            .any(|p| p.ends_with(format!("src/{name}/java"))));
    }
}
#[test]
fn file_based_project_context_service_test_discovers_architectury_style_source_sets_in_sorted_order(
) {
    let t = Temp::new();
    t.architectury();
    let x = t.context();
    assert_eq!(
        x.source_sets
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        vec!["common", "fabric", "forge", "neoforge", "testmod"]
    );
    for s in &x.source_sets {
        assert!(s
            .source_directories
            .iter()
            .any(|p| p.ends_with(format!("src/{}/java", s.name))));
        assert!(s
            .resource_directories
            .iter()
            .any(|p| p.ends_with(format!("src/{}/resources", s.name))));
        assert_eq!(
            s.output_directory,
            Some(t.0.join(format!("build/classes/java/{}", s.name)))
        );
    }
}
#[test]
fn file_based_project_context_service_test_preserves_mapped_sources_on_main_source_set_only() {
    let t = Temp::new();
    t.architectury();
    t.dir("mapped-sources");
    let x = t.context();
    assert!(x
        .source_sets
        .iter()
        .find(|s| s.name == "common")
        .unwrap()
        .source_directories
        .iter()
        .all(|p| !p.ends_with("mapped-sources")));
    t.dir("src/main/java");
    let x = t.context();
    assert!(x
        .source_sets
        .iter()
        .find(|s| s.name == "main")
        .unwrap()
        .source_directories
        .iter()
        .any(|p| p.ends_with("mapped-sources")));
    assert!(x.source_sets.iter().filter(|s| s.name != "main").all(|s| s
        .source_directories
        .iter()
        .all(|p| !p.ends_with("mapped-sources"))));
}
#[test]
fn file_based_project_context_service_test_discovers_generated_outputs_and_source_set_project_outputs(
) {
    let t = Temp::new();
    t.architectury();
    let common = t.dir("build/classes/java/common");
    let fabric = t.dir("build/classes/java/fabric");
    let sources = t.dir("build/generated/sources/annotationProcessor/java/main");
    let resources = t.dir("build/generated/resources/common");
    fs::write(sources.join("Example.java"), "class Example {}").unwrap();
    fs::write(resources.join("pack.mcmeta"), "{}").unwrap();
    let x = t.context();
    assert_eq!(
        x.classpath
            .project_outputs
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>(),
        [common, fabric].into_iter().collect()
    );
    assert_eq!(
        x.classpath
            .generated_outputs
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>(),
        [sources, resources].into_iter().collect()
    );
    assert!(x.classpath.entry_count() >= 4);
}
