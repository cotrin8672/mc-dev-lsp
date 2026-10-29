use mcdev_core::{
    mapping::{ClassRef, MappingLookupResult},
    project::*,
    MappingNamespace,
};
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "mcdev-project-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn dir(&self, path: &str) -> PathBuf {
        let path = self.0.join(path);
        fs::create_dir_all(&path).unwrap();
        path
    }
    fn write(&self, path: &str, text: &str) -> PathBuf {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        path
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let base = std::env::temp_dir().canonicalize().unwrap();
        let path = self.0.canonicalize().unwrap();
        assert_eq!(path.parent(), Some(base.as_path()));
        assert!(path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("mcdev-project-"));
        fs::remove_dir_all(path).unwrap();
    }
}
const TINY:&str="tiny\t2\t0\tnamed\tintermediary\nc\tnet/minecraft/Client\tnet/minecraft/class_310\n\tm\t()V\ttick\tmethod_1\n";
fn config(path: PathBuf, package: &str, mixins: &[&str]) -> MixinConfigRef {
    MixinConfigRef {
        path,
        package_name: Some(package.into()),
        mixins: mixins.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    }
}
fn set(root: &Path, name: &str) -> SourceSetContext {
    SourceSetContext {
        name: name.into(),
        source_directories: vec![root.join(format!("src/{name}/java"))],
        resource_directories: vec![root.join(format!("src/{name}/resources"))],
        output_directory: None,
    }
}

#[test]
fn platform_markers_and_precedence() {
    for (marker, want) in [
        ("fabric-loom", ModPlatform::Fabric),
        ("net.fabricmc.fabric-loom", ModPlatform::Fabric),
        ("org.neoforged.moddev", ModPlatform::NeoForge),
        ("net.neoforged.gradle", ModPlatform::NeoForge),
        ("neoforge moddev", ModPlatform::NeoForge),
        ("ForgeGradle", ModPlatform::Forge),
        ("net.minecraftforge.gradle", ModPlatform::Forge),
        ("minecraftforge", ModPlatform::Forge),
        ("java", ModPlatform::Unknown),
        (
            "net.neoforged.gradle net.minecraftforge.gradle",
            ModPlatform::NeoForge,
        ),
    ] {
        assert_eq!(PlatformDetector::detect(&[marker.into()]), want);
    }
    assert_eq!(PlatformDetector::detect(&[]), ModPlatform::Unknown);
}
#[test]
fn detects_root_combining_gradle_files() {
    let t = Temp::new();
    t.write("build.gradle.kts", "plugins { java }");
    t.write("settings.gradle", "fabric-loom");
    assert_eq!(
        PlatformDetector::detect_from_root(&t.0).unwrap(),
        ModPlatform::Fabric
    );
}
#[test]
fn walker_filters_only_directory_segments_in_root_scope() {
    let t = Temp::new();
    for name in [
        "src/a.json",
        "build/ignored.json",
        "nested/.gradle/ignored.json",
        "nested/node_modules/ignored.json",
        "buildable/yes.json",
        "src/build/ignored.json",
    ] {
        t.write(name, "{}");
    }
    let found = walk_regular_files(&t.0).unwrap();
    assert_eq!(
        found,
        vec![t.0.join("buildable/yes.json"), t.0.join("src/a.json")]
    );
    assert!(walk_regular_files(&t.0.join("missing")).unwrap().is_empty());
    let root = t.dir("build");
    t.write("build/root-resource.json", "{}");
    assert_eq!(walk_regular_files(&root).unwrap().len(), 2);
}
#[test]
fn mapping_whitelist_all_prefixes_and_nested_paths() {
    let t = Temp::new();
    let prefixes = [
        ".gradle/loom-cache",
        ".gradle/caches/fabric-loom",
        ".gradle/caches/modules-2/files-2.1/net.fabricmc/yarn",
        ".gradle/caches/modules-2/files-2.1/net.minecraft",
        ".gradle/caches/modules-2/files-2.1/net.minecraftforge",
        ".gradle/caches/modules-2/files-2.1/net.neoforged",
        "build/loom-cache",
        "build/createSrgToMcp",
        "build/tmp",
        "module/.gradle/caches/fabric-loom",
        "src/build/loom-cache",
    ];
    for prefix in prefixes {
        t.write(&format!("{prefix}/version/mappings.tiny"), TINY);
    }
    for prefix in ["build/classes", ".gradle/caches/other", "node_modules/pkg"] {
        t.write(&format!("{prefix}/ignored.tiny"), TINY);
    }
    let found = MappingDiscoveryService::discover_mapping_files(&t.0).unwrap();
    assert_eq!(found.len(), prefixes.len());
    let sort_key = |p: &PathBuf| {
        if cfg!(windows) {
            p.to_string_lossy().replace('/', "\\").to_uppercase()
        } else {
            p.to_string_lossy().into_owned()
        }
    };
    assert!(found.windows(2).all(|w| sort_key(&w[0]) < sort_key(&w[1])));
    assert!(should_prune_mapping_discovery_directory(Path::new(
        "build/classes"
    )));
    assert!(!should_prune_mapping_discovery_directory(Path::new(
        ".gradle/caches/modules-2"
    )));
    assert!(!should_prune_mapping_discovery_directory(Path::new(
        "build"
    )));
    assert!(is_excluded_from_mapping_discovery(Path::new(
        ".gradle/caches/unrelated.tiny"
    )));
}
#[test]
fn mapping_context_parses_merges_and_selects_namespaces() {
    let t = Temp::new();
    t.write("a.tiny", TINY);
    t.write(
        "b.tiny",
        "tiny\t2\t0\tnamed\tintermediary\nc\tnet/minecraft/Screen\tnet/minecraft/class_437\n",
    );
    t.write("bad.tiny", "broken");
    let ctx = MappingDiscoveryService::discover_mapping_context(&t.0, ModPlatform::Fabric).unwrap();
    assert_eq!(ctx.source_namespace, MappingNamespace::Named);
    assert_eq!(ctx.runtime_namespace, MappingNamespace::Intermediary);
    assert_eq!(ctx.aw_namespace, Some(MappingNamespace::Named));
    for name in ["net/minecraft/Client", "net/minecraft/Screen"] {
        assert!(matches!(
            ctx.resolver.remap_class(
                &ClassRef {
                    internal_name: name.into(),
                    namespace: MappingNamespace::Named
                },
                MappingNamespace::Intermediary
            ),
            MappingLookupResult::Found(_)
        ));
    }
    assert!(MappingDiscoveryService::parse_mapping_file(&t.0.join("bad.tiny")).is_err());
}
#[test]
fn host_mapping_snapshots_use_the_same_order_and_namespace_selection() {
    let t = Temp::new();
    let first = t.write("first.tiny", TINY);
    let second_text = "tiny\t2\t0\tnamed\tintermediary\nc\tnet/minecraft/Client\tother/Client\nc\tnet/minecraft/Screen\tnet/minecraft/class_437\n";
    let second = t.write("second.tiny", second_text);
    let invalid = t.write("invalid.tiny", "broken");
    let from_files = MappingDiscoveryService::build_mapping_context(
        &[first.clone(), second.clone(), invalid.clone()],
        ModPlatform::Fabric,
    )
    .unwrap();
    let snapshots = vec![
        (first, TINY.to_owned()),
        (second, second_text.to_owned()),
        (invalid, "broken".to_owned()),
    ];
    // Snapshot paths are display metadata; the host owns all byte access.
    for (path, _) in &snapshots {
        fs::remove_file(path).unwrap();
    }
    let from_snapshots = MappingDiscoveryService::build_mapping_context_from_contents(
        &snapshots,
        ModPlatform::Fabric,
    )
    .unwrap();
    assert_eq!(
        from_snapshots.available_namespaces,
        from_files.available_namespaces
    );
    assert_eq!(from_snapshots.source_namespace, from_files.source_namespace);
    assert_eq!(
        from_snapshots.runtime_namespace,
        from_files.runtime_namespace
    );
    assert_eq!(from_snapshots.aw_namespace, from_files.aw_namespace);
    assert_eq!(from_snapshots.at_namespace, from_files.at_namespace);
    for name in [
        "net/minecraft/Client",
        "net/minecraft/Screen",
        "missing/Class",
    ] {
        let class = ClassRef {
            internal_name: name.into(),
            namespace: MappingNamespace::Named,
        };
        assert_eq!(
            from_snapshots
                .resolver
                .remap_class(&class, MappingNamespace::Intermediary),
            from_files
                .resolver
                .remap_class(&class, MappingNamespace::Intermediary),
        );
    }
}
#[test]
fn forge_mapping_namespaces_and_empty_context() {
    let t = Temp::new();
    t.write("joined.srg", "CL: a/b c/d\nMD: a/b/m ()V c/d/func_1 ()V\n");
    let ctx = MappingDiscoveryService::discover_mapping_context(&t.0, ModPlatform::Forge).unwrap();
    assert_eq!(ctx.at_namespace, Some(MappingNamespace::Srg));
    assert_eq!(ctx.source_namespace, MappingNamespace::Srg);
    let empty = MappingDiscoveryService::discover_mapping_context(
        &t.0.join("missing"),
        ModPlatform::Unknown,
    )
    .unwrap();
    assert!(empty.available_namespaces.is_empty());
    assert_eq!(empty.aw_namespace, None);
    assert_eq!(empty.at_namespace, None);
}
#[test]
fn config_names_and_permissive_optional_fields() {
    for name in [
        "mixins.json",
        "example.mixins.json5",
        "mixin-test.json",
        "mixins.json5",
    ] {
        assert!(MixinConfigDiscoveryService::is_mixin_config_file(
            Path::new(name)
        ));
    }
    assert!(!MixinConfigDiscoveryService::is_mixin_config_file(
        Path::new("fabric.mod.json")
    ));
    let cfg = MixinConfigDiscoveryService::parse_content(
        Path::new("x"),
        r#"{"package":17,"mixins":["A",12],"client":"bad"}"#,
    )
    .unwrap();
    assert_eq!(cfg.package_name, None);
    assert_eq!(cfg.mixins, vec!["A"]);
    assert!(cfg.client.is_empty());
    for text in ["{bad", "[]", "false"] {
        assert!(MixinConfigDiscoveryService::parse_content(Path::new("x"), text).is_none());
    }
}

#[test]
fn discovery_and_editor_share_json5_single_quote_and_unquoted_key_parser() {
    let content = "{package:'a',mixins:['Foo'],client:['Client'],}";
    let discovered =
        MixinConfigDiscoveryService::parse_content(Path::new("a.mixins.json5"), content).unwrap();
    let edited = mcdev_core::mixin_edits::MixinConfigEditor::parse(content, "").unwrap();
    assert_eq!(discovered.package_name.as_deref(), Some("a"));
    assert_eq!(discovered.mixins, vec!["Foo"]);
    assert_eq!(discovered.client, vec!["Client"]);
    assert_eq!(discovered.package_name, edited.package_name);
    assert_eq!(discovered.mixins, edited.mixins);
}
#[test]
fn json5_comments_trailing_commas_and_string_delimiters() {
    let text =
        "{/* x */\"package\":\"a.b\",\n// comment\n\"mixins\":[\"A\",],\"url\":\"http://a/*x*/\",}";
    let cfg = MixinConfigDiscoveryService::parse_content(Path::new("x.json5"), text).unwrap();
    assert_eq!(cfg.package_name.as_deref(), Some("a.b"));
    assert_eq!(cfg.mixins, vec!["A"]);
    let value: serde_json::Value =
        serde_json::from_str(&MixinConfigDiscoveryService::normalize_json_content(text)).unwrap();
    assert_eq!(value["url"], "http://a/*x*/");
}
#[test]
fn discovers_config_and_awat_excluding_generated_dirs() {
    let t = Temp::new();
    t.write("src/main/resources/main.mixins.json", "{}");
    t.write(
        "src/client/resources/client.mixins.json5",
        "{\"client\":[],}",
    );
    for prefix in ["build", ".gradle", "node_modules"] {
        t.write(&format!("{prefix}/bad.mixins.json"), "{}");
        t.write(&format!("{prefix}/bad.aw"), "accessWidener v2 named");
        t.write(&format!("{prefix}/bad_at.cfg"), "public a.B");
    }
    t.write(
        "src/main/resources/test.accesswidener",
        "accessWidener v2 named\naccessible class a/B\n",
    );
    t.write("src/main/resources/accessTransformer.cfg", "public a.B\n");
    assert_eq!(
        MixinConfigDiscoveryService::discover(&t.0).unwrap().len(),
        2
    );
    let aw = AwAtDiscoveryService::discover_access_wideners(&t.0).unwrap();
    assert_eq!(aw.len(), 1);
    assert!(aw[0].parsed.is_some());
    assert_eq!(aw[0].namespace, Some(MappingNamespace::Named));
    let at = AwAtDiscoveryService::discover_access_transformers(&t.0).unwrap();
    assert_eq!(at.len(), 1);
    assert!(at[0].parsed.is_some());
}
#[test]
fn awat_names_and_invalid_models() {
    for name in ["x.aw", "X.ACCESSWIDENER"] {
        assert!(AwAtDiscoveryService::is_access_widener_file(Path::new(
            name
        )));
    }
    for name in [
        "accesstransformer.cfg",
        "example_at.cfg",
        "x.at",
        "access-transform.cfg",
    ] {
        assert!(AwAtDiscoveryService::is_access_transformer_file(Path::new(
            name
        )));
    }
    assert!(!AwAtDiscoveryService::is_access_transformer_file(
        Path::new("x.cfg")
    ));
    let t = Temp::new();
    let aw = t.write("bad.aw", "invalid");
    let at = t.write("bad.at", "private-broken class");
    assert!(AwAtDiscoveryService::parse_access_widener(&aw)
        .unwrap()
        .parsed
        .is_none());
    assert!(AwAtDiscoveryService::parse_access_transformer(&at)
        .unwrap()
        .parsed
        .is_none());
}
#[test]
fn source_set_longest_prefix_ambiguity_and_mapped_sources() {
    let t = Temp::new();
    let mut sets = vec![set(&t.0, "main"), set(&t.0, "client")];
    sets[0].source_directories.push(t.0.join("mapped-sources"));
    assert_eq!(
        SourceSetResolver::containing_source_set(&sets, &t.0.join("src/client/java/A.java"))
            .unwrap()
            .name,
        "client"
    );
    assert_eq!(
        SourceSetResolver::containing_source_set(&sets, &t.0.join("mapped-sources/A.java"))
            .unwrap()
            .name,
        "main"
    );
    assert!(SourceSetResolver::containing_source_set(
        &sets,
        &t.0.join("src/main/javascript/A.java")
    )
    .is_none());
    let mut nested = set(&t.0, "nested");
    nested.source_directories = vec![t.0.join("src/main/java/nested")];
    sets.push(nested.clone());
    assert_eq!(
        SourceSetResolver::containing_source_set(&sets, &t.0.join("src/main/java/nested/A.java"))
            .unwrap()
            .name,
        "nested"
    );
    nested.name = "duplicate".into();
    sets.push(nested);
    assert!(SourceSetResolver::containing_source_set(
        &sets,
        &t.0.join("src/main/java/nested/A.java")
    )
    .is_none());
}
#[test]
fn configs_source_set_local_shared_sibling_and_root_boundary() {
    let t = Temp::new();
    let sets = vec![set(&t.0, "fabric"), set(&t.0, "forge"), set(&t.0, "client")];
    let configs = vec![
        config(t.0.join("shared.mixins.json"), "common", &[]),
        config(
            t.0.join("src/fabric/resources/fabric.mixins.json"),
            "fabric",
            &[],
        ),
        config(
            t.0.join("src/forge/resources/forge.mixins.json"),
            "forge",
            &[],
        ),
        config(
            t.0.parent().unwrap().join("external.mixins.json"),
            "outside",
            &[],
        ),
    ];
    assert_eq!(
        MixinConfigDiscoveryService::shared_configs(&configs, &sets, &t.0).len(),
        1
    );
    let local =
        MixinConfigDiscoveryService::configs_for_source_set(&configs, &sets[0], &sets, &t.0);
    assert_eq!(local.len(), 1);
    assert_eq!(local[0].package_name.as_deref(), Some("fabric"));
    assert_eq!(
        MixinConfigDiscoveryService::configs_for_source_set(&configs, &sets[2], &sets, &t.0)[0]
            .package_name
            .as_deref(),
        Some("common")
    );
    assert!(MixinConfigDiscoveryService::configs_for_source_set(
        &configs[1..],
        &sets[2],
        &sets,
        &t.0
    )
    .is_empty());
}
#[test]
fn selects_listed_config_then_longest_segment_safe_package() {
    let configs = vec![
        config("a".into(), "com.example", &[]),
        config("b".into(), "com.example.mixin", &["sub.A"]),
        config("c".into(), "com.other", &["B"]),
    ];
    assert_eq!(
        MixinConfigDiscoveryService::select_for_mixin(
            &configs,
            Some("A"),
            Some("com.example.mixin.sub")
        )
        .unwrap()
        .path,
        PathBuf::from("b")
    );
    assert_eq!(
        MixinConfigDiscoveryService::select_for_mixin(
            &configs,
            Some("Missing"),
            Some("com.example.mixin.sub")
        )
        .unwrap()
        .path,
        PathBuf::from("b")
    );
    assert!(MixinConfigDiscoveryService::select_for_mixin(
        &configs,
        Some("A"),
        Some("com.examples")
    )
    .is_none());
    assert!(MixinConfigDiscoveryService::select_for_mixin(&configs, None, None).is_none());
    assert!(MixinConfigDiscoveryService::select_for_mixin(
        &[configs[1].clone(), configs[1].clone()],
        Some("A"),
        Some("com.example.mixin.sub")
    )
    .is_none());
    assert!(MixinConfigDiscoveryService::select_for_mixin(
        &[configs[0].clone(), configs[0].clone()],
        Some("Unknown"),
        Some("com.example")
    )
    .is_none());
}
#[test]
fn config_listing_default_package_and_common_lists() {
    let mut cfg = config("a".into(), "", &["A"]);
    assert!(MixinConfigDiscoveryService::is_mixin_listed(
        &cfg,
        "A",
        Some("any.package")
    ));
    cfg.package_name = Some("a.b".into());
    cfg.mixins.clear();
    cfg.common = vec!["A".into()];
    assert!(MixinConfigDiscoveryService::is_mixin_listed(
        &cfg,
        "A",
        Some("a.b")
    ));
}
#[test]
fn builder_discovery_overrides_and_absent_inputs() {
    let t = Temp::new();
    t.write("build.gradle.kts", "fabric-loom");
    t.write("mappings.tiny", TINY);
    t.write("main.mixins.json", "{\"package\":\"a\"}");
    t.write("bad.mixins.json", "bad");
    let ctx = ProjectContextBuilder::build(ProjectContextInput::new("test", &t.0)).unwrap();
    assert_eq!(ctx.platform, ModPlatform::Fabric);
    assert_eq!(ctx.mapping_files.len(), 1);
    assert_eq!(ctx.mixin_configs.len(), 1);
    assert_eq!(ctx.resource_issues.len(), 1);
    let mut input = ProjectContextInput::new("test", &t.0);
    input.platform = Some(ModPlatform::Forge);
    input.mappings = Some(empty_mapping_context());
    input.mixin_configs = Some(vec![]);
    input.classpath.minecraft_jars = vec![t.0.join("host.jar")];
    let ctx = ProjectContextBuilder::build(input).unwrap();
    assert_eq!(ctx.platform, ModPlatform::Forge);
    assert!(ctx.mapping_files.is_empty());
    assert!(ctx.mixin_configs.is_empty());
    assert_eq!(ctx.minecraft_jars, ctx.classpath.minecraft_jars);
    assert!(ProjectContextBuilder::empty("empty", t.0.join("missing"))
        .mixin_configs
        .is_empty());
}
#[test]
fn builder_gradle_contents_work_without_root_files() {
    let t = Temp::new();
    let mut input = ProjectContextInput::new("test", &t.0);
    input.gradle_contents = vec!["org.neoforged.moddev".into()];
    assert_eq!(
        ProjectContextBuilder::build(input).unwrap().platform,
        ModPlatform::NeoForge
    );
}
#[test]
fn file_fallback_preserves_java_kotlin_and_generated_outputs() {
    let t = Temp::new();
    for name in [
        "common", "fabric", "forge", "neoforge", "testmod", "main", "client",
    ] {
        t.dir(&format!("src/{name}/resources"));
        t.dir(&format!("src/{name}/kotlin"));
    }
    t.dir("src/main/java");
    t.dir("mapped-sources/a/b");
    let java = t.dir("build/classes/java/main");
    let kotlin = t.dir("build/classes/kotlin/main");
    let client = t.dir("build/classes/kotlin/client");
    let generated = t.dir("build/generated/sources/annotationProcessor/java/main");
    let resources = t.dir("build/generated/resources/common");
    let dependency = t.write("libs/library.jar", "");
    let minecraft = t.write(
        ".gradle/loom-cache/remapped_working/minecraft-client-mapped.jar",
        "",
    );
    let ctx = build_file_project_context(&t.0, None).unwrap();
    assert_eq!(
        ctx.source_sets
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        vec!["client", "common", "fabric", "forge", "main", "neoforge", "testmod"]
    );
    let main = ctx.source_sets.iter().find(|s| s.name == "main").unwrap();
    assert_eq!(main.source_directories.len(), 3);
    assert_eq!(main.output_directory, Some(java.clone()));
    assert!(ctx
        .source_sets
        .iter()
        .filter(|s| s.name != "main")
        .all(|s| !s.source_directories.contains(&t.0.join("mapped-sources"))));
    assert_eq!(ctx.classpath.project_outputs.len(), 3);
    for path in [java, kotlin, client] {
        assert!(ctx.classpath.project_outputs.contains(&path));
    }
    assert_eq!(ctx.classpath.generated_outputs.len(), 2);
    assert!(ctx.classpath.generated_outputs.contains(&generated));
    assert!(ctx.classpath.generated_outputs.contains(&resources));
    assert_eq!(ctx.classpath.dependency_jars, vec![dependency]);
    assert_eq!(ctx.classpath.minecraft_jars, vec![minecraft]);
    assert_eq!(ctx.index_state, ProjectIndexState::Ready);
}
#[test]
fn explicit_host_empty_facts_never_replaced_by_file_fallback() {
    let t = Temp::new();
    t.dir("src/main/java");
    t.dir("build/classes/java/main");
    t.write("libs/minecraft.jar", "");
    let ctx = build_file_project_context(&t.0, Some(&HostProjectFacts::default())).unwrap();
    assert!(ctx.source_sets.is_empty());
    assert_eq!(ctx.classpath.entry_count(), 0);
    assert_eq!(ctx.index_state, ProjectIndexState::NotReady);
}
#[test]
fn host_facts_preserve_generated_dependency_and_minecraft_categories() {
    let t = Temp::new();
    let classpath = ClasspathSnapshot {
        project_outputs: vec!["output".into()],
        generated_outputs: vec!["generated".into()],
        dependency_jars: vec!["dependency".into()],
        minecraft_jars: vec!["minecraft".into()],
        ..Default::default()
    };
    let sets = vec![set(&t.0, "client")];
    let facts = HostProjectFacts {
        classpath: classpath.clone(),
        source_sets: sets.clone(),
        ..Default::default()
    };
    let ctx = build_file_project_context(&t.0, Some(&facts)).unwrap();
    assert_eq!(ctx.classpath, classpath);
    assert_eq!(ctx.classpath.all_entries().len(), 4);
    assert_eq!(ctx.source_sets, sets);
}
#[test]
fn unicode_file_uris_plus_windows_unix_unc_and_invalid_escapes() {
    assert_eq!(
        uri_to_path("file:///C:/work/%E6%97%A5%E6%9C%AC%20x+A.kt").unwrap(),
        PathBuf::from("C:/work/日本 x+A.kt")
    );
    assert_eq!(
        uri_to_path("file:///tmp/%E6%97%A5%E6%9C%AC%20x").unwrap(),
        PathBuf::from("/tmp/日本 x")
    );
    assert_eq!(
        uri_to_path("file://localhost/tmp/A").unwrap(),
        PathBuf::from("/tmp/A")
    );
    assert_eq!(
        uri_to_path("file://server/share/A%20B").unwrap(),
        PathBuf::from("//server/share/A B")
    );
    for uri in [
        "",
        "https://example.com",
        "file:///bad%GG",
        "file:///bad%",
        "file:///bad%FF",
        "file:///bad%00",
        "file:///bad#fragment",
    ] {
        assert!(uri_to_path(uri).is_err(), "{uri}");
    }
    let t = Temp::new();
    let path = t.0.join("日本 +%.kt");
    assert_eq!(uri_to_path(&path_to_uri(&path)).unwrap(), path);
}

#[cfg(windows)]
#[test]
fn windows_file_uris_normalize_verbatim_paths_without_reencoding() {
    let t = Temp::new();
    let existing = t.write("space # % 日本/file.txt", "");
    let canonical = fs::canonicalize(&existing).unwrap();
    let canonical_text = canonical.to_string_lossy();
    assert!(canonical_text.starts_with(r"\\?\"));
    let normal = PathBuf::from(canonical_text.strip_prefix(r"\\?\").unwrap());
    let canonical_uri = path_to_uri(&canonical);
    assert_eq!(canonical_uri, path_to_uri(&normal));
    assert_eq!(uri_to_path(&canonical_uri).unwrap(), normal);

    for (raw, expected_uri) in [
        (
            r"C:\space # ? %\日本\file.txt",
            "file:///C:/space%20%23%20%3F%20%25/%E6%97%A5%E6%9C%AC/file.txt",
        ),
        (
            r"\\server\share\space # ? %\日本\file.txt",
            "file://server/share/space%20%23%20%3F%20%25/%E6%97%A5%E6%9C%AC/file.txt",
        ),
        (
            r"\\?\C:\space # ? %\日本\file.txt",
            "file:///C:/space%20%23%20%3F%20%25/%E6%97%A5%E6%9C%AC/file.txt",
        ),
        (
            r"\\?\UNC\server\share\space # ? %\日本\file.txt",
            "file://server/share/space%20%23%20%3F%20%25/%E6%97%A5%E6%9C%AC/file.txt",
        ),
        (
            r"\\?\unc\server\share\space # ? %\日本\file.txt",
            "file://server/share/space%20%23%20%3F%20%25/%E6%97%A5%E6%9C%AC/file.txt",
        ),
    ] {
        let uri = path_to_uri(Path::new(raw));
        assert_eq!(uri, expected_uri);
        assert_eq!(path_to_uri(&uri_to_path(&uri).unwrap()), expected_uri);
    }
    let unknown_namespace = path_to_uri(Path::new(
        r"\\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy1\file.txt",
    ));
    assert!(unknown_namespace.starts_with("file://%3F/GLOBALROOT/"));
    assert!(!unknown_namespace.starts_with("file:///GLOBALROOT/"));
}

#[test]
fn cache_detects_resource_contents_and_host_generation_changes() {
    let t = Temp::new();
    let config = t.write("main.mixins.json", "{\"package\":\"old\"}");
    let service = FileBasedProjectContextService::default();
    let uri = path_to_uri(&t.0);
    let first = service.load_cached_session(&uri, None).unwrap();
    let second = service.load_cached_session(&uri, None).unwrap();
    assert!(!first.cache_hit);
    assert!(second.cache_hit);
    assert!(Arc::ptr_eq(&first.context, &second.context));
    fs::write(config, "{\"package\":\"new\"}").unwrap();
    let third = service.load_cached_session(&uri, None).unwrap();
    assert!(!third.cache_hit);
    assert!(third.version > second.version);
    assert_eq!(
        third.context.mixin_configs[0].package_name.as_deref(),
        Some("new")
    );
    let mut facts = HostProjectFacts::default();
    let supplied = service.load_cached_session(&uri, Some(&facts)).unwrap();
    facts.dependency_generation += 1;
    let changed = service.load_cached_session(&uri, Some(&facts)).unwrap();
    assert!(changed.version > supplied.version);
    let reloaded = service.reload(&uri, Some(&facts)).unwrap();
    assert!(reloaded.version > changed.version);
    service.clear_cache().unwrap();
    let cleared = service.load_cached_session(&uri, Some(&facts)).unwrap();
    assert!(!cleared.cache_hit);
    assert!(cleared.version > reloaded.version);
}
#[test]
fn concurrent_cold_load_builds_once() {
    let t = Temp::new();
    let count = Arc::new(AtomicUsize::new(0));
    let builds = count.clone();
    let service = Arc::new(FileBasedProjectContextService::with_builder(
        move |root, facts| {
            builds.fetch_add(1, Ordering::Relaxed);
            build_file_project_context(root, facts)
        },
    ));
    let uri = path_to_uri(&t.0);
    let gate = Arc::new(std::sync::Barrier::new(32));
    let threads = (0..32)
        .map(|_| {
            let service = service.clone();
            let uri = uri.clone();
            let gate = gate.clone();
            std::thread::spawn(move || {
                gate.wait();
                service.load_cached_session(&uri, None).unwrap()
            })
        })
        .collect::<Vec<_>>();
    let results = threads
        .into_iter()
        .map(|t| t.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(count.load(Ordering::Relaxed), 1);
    assert_eq!(results.iter().filter(|r| !r.cache_hit).count(), 1);
    assert!(results
        .iter()
        .all(|r| Arc::ptr_eq(&r.context, &results[0].context)));
    assert!(results.iter().all(|r| r.version == results[0].version));
}
#[test]
fn cold_failure_is_returned_once_and_next_load_recovers() {
    let t = Temp::new();
    let count = Arc::new(AtomicUsize::new(0));
    let builds = count.clone();
    let service = Arc::new(FileBasedProjectContextService::with_builder(
        move |root, facts| {
            if builds.fetch_add(1, Ordering::Relaxed) == 0 {
                return Err(io::Error::other("simulated build failure"));
            }
            build_file_project_context(root, facts)
        },
    ));
    let uri = path_to_uri(&t.0);
    let gate = Arc::new(std::sync::Barrier::new(8));
    let threads = (0..8)
        .map(|_| {
            let gate = gate.clone();
            let service = service.clone();
            let uri = uri.clone();
            std::thread::spawn(move || {
                gate.wait();
                service.load_cached_session(&uri, None)
            })
        })
        .collect::<Vec<_>>();
    let results = threads
        .into_iter()
        .map(|t| t.join().unwrap())
        .collect::<Vec<_>>();
    let errors = results
        .iter()
        .filter_map(|r| r.as_ref().err())
        .collect::<Vec<_>>();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].kind(), io::ErrorKind::Other);
    assert_eq!(errors[0].to_string(), "simulated build failure");
    assert!(!errors
        .iter()
        .any(|e| e.to_string().contains("Session missing")));
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 7);
    assert!(service.load_cached_session(&uri, None).unwrap().cache_hit);
    assert_eq!(count.load(Ordering::Relaxed), 2);
}
#[test]
fn warm_workspace_does_not_wait_for_other_workspaces_cold_build() {
    use std::sync::{mpsc, Mutex};
    use std::time::Duration;
    let t = Temp::new();
    let a = t.dir("a");
    let b = t.dir("b");
    let b_expected = b.clone();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    let resume_rx = Mutex::new(resume_rx);
    let service = Arc::new(FileBasedProjectContextService::with_builder(
        move |root, facts| {
            if root == b_expected {
                entered_tx.send(()).unwrap();
                resume_rx
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap();
            }
            build_file_project_context(root, facts)
        },
    ));
    assert!(
        !service
            .load_cached_session(&path_to_uri(&a), None)
            .unwrap()
            .cache_hit
    );
    let service_b = service.clone();
    let b_thread = std::thread::spawn(move || {
        service_b
            .load_cached_session(&path_to_uri(&b), None)
            .unwrap()
    });
    entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(
        service
            .load_cached_session(&path_to_uri(&a), None)
            .unwrap()
            .cache_hit
    );
    assert!(!b_thread.is_finished());
    resume_tx.send(()).unwrap();
    b_thread.join().unwrap();
}
#[test]
fn info_lines_and_serialized_dump_preserve_contract() {
    let mut ctx = ProjectContextBuilder::empty("demo", PathBuf::from("project"));
    ctx.platform = ModPlatform::Fabric;
    ctx.classpath.minecraft_jars = vec!["minecraft.jar".into()];
    ctx.index_state = ProjectIndexState::Ready;
    let lines = InfoService::format_lines(&ctx, 1, "1.2");
    assert_eq!(lines.len(), 20);
    for line in [
        "Project: demo",
        "Platform: Fabric",
        "Mappings: none",
        "Minecraft jar: found",
        "Class index: ready",
        "Bytecode index: ready",
        "Protocol: 1",
        "Extension: 1.2",
    ] {
        assert!(lines.iter().any(|s| s == line));
    }
    assert_eq!(InfoService::format(&ctx, 1, "1.2"), lines.join("\n"));
    let dump: serde_json::Value = serde_json::from_str(&InfoService::dump(&ctx).unwrap()).unwrap();
    assert_eq!(dump["projectId"], "demo");
    assert!(dump["classpath"]["minecraftJars"].is_array());
    assert_eq!(dump["platform"], "fabric");
    assert_eq!(dump["mappings"]["sourceNamespace"], "named");
    assert_eq!(dump["indexState"], "ready");
    assert_eq!(dump["classpath"]["entryCount"], 1);
    assert_eq!(dump["lines"].as_array().unwrap().len(), 20);
    let reload = InfoService::reload_value(&ctx, 1, "1.2");
    assert_eq!(reload["status"], "project context reloaded");
    assert_eq!(reload["classpathEntries"], 1);
    assert_eq!(
        serde_json::to_string(&ModPlatform::NeoForge).unwrap(),
        "\"NEOFORGE\""
    );
    for state in [
        ProjectIndexState::NotReady,
        ProjectIndexState::Building,
        ProjectIndexState::Stale,
        ProjectIndexState::Failed,
    ] {
        ctx.index_state = state;
        assert!(InfoService::format(&ctx, 1, "x").contains("Class index:"));
    }
}

#[test]
fn reindex_waits_for_cold_load_then_builds_new_version() {
    use std::sync::{mpsc, Mutex};
    use std::time::Duration;
    let t = Temp::new();
    let count = Arc::new(AtomicUsize::new(0));
    let builds = count.clone();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    let resume_rx = Mutex::new(resume_rx);
    let (reindex_entered_tx, reindex_entered_rx) = mpsc::channel();
    let (reindex_resume_tx, reindex_resume_rx) = mpsc::channel();
    let reindex_resume_rx = Mutex::new(reindex_resume_rx);
    let service = Arc::new(FileBasedProjectContextService::with_builder(
        move |root, facts| {
            if builds.fetch_add(1, Ordering::Relaxed) == 0 {
                entered_tx.send(()).unwrap();
                resume_rx
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap();
            } else {
                reindex_entered_tx.send(()).unwrap();
                reindex_resume_rx
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap();
            }
            build_file_project_context(root, facts)
        },
    ));
    let uri = path_to_uri(&t.0);
    let cold_service = service.clone();
    let cold_uri = uri.clone();
    let cold =
        std::thread::spawn(move || cold_service.load_cached_session(&cold_uri, None).unwrap());
    entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let reload_service = service.clone();
    let reload_uri = uri.clone();
    let reload = std::thread::spawn(move || reload_service.reindex(&reload_uri, None).unwrap());
    assert_eq!(count.load(Ordering::Relaxed), 1);
    resume_tx.send(()).unwrap();
    reindex_entered_rx
        .recv_timeout(Duration::from_secs(10))
        .unwrap();
    assert_eq!(count.load(Ordering::Relaxed), 2);
    reindex_resume_tx.send(()).unwrap();
    let first = cold.join().unwrap();
    let second = reload.join().unwrap();
    assert_eq!(count.load(Ordering::Relaxed), 2);
    assert!(!first.cache_hit);
    assert!(second.version > first.version);
    let current = service.load_cached_session(&uri, None).unwrap();
    assert!(current.cache_hit);
    assert!(Arc::ptr_eq(&current.context, &second.context));
}

#[test]
fn config_invalidation_tracks_add_remove_mapping_and_all_host_generations() {
    let t = Temp::new();
    let facts = HostProjectFacts::default();
    let initial = context_fingerprint(&t.0, Some(&facts)).unwrap();
    let config = t.write("x.mixins.json", "{}");
    let added = context_fingerprint(&t.0, Some(&facts)).unwrap();
    assert_ne!(initial, added);
    fs::remove_file(config).unwrap();
    assert_eq!(initial, context_fingerprint(&t.0, Some(&facts)).unwrap());
    t.write(".gradle/caches/fabric-loom/mappings.tiny", TINY);
    let mapped = context_fingerprint(&t.0, Some(&facts)).unwrap();
    assert_ne!(initial, mapped);
    t.write("node_modules/ignored.mixins.json", "{}");
    assert_eq!(mapped, context_fingerprint(&t.0, Some(&facts)).unwrap());
    for i in 0..5 {
        let mut changed = facts.clone();
        match i {
            0 => changed.project_generation += 1,
            1 => changed.dependency_generation += 1,
            2 => changed.generated_source_generation += 1,
            3 => changed.config_generation += 1,
            _ => changed.provider_generation += 1,
        };
        assert_ne!(mapped, context_fingerprint(&t.0, Some(&changed)).unwrap());
    }
}
