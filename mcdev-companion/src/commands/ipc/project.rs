//! Project inputs derived only from the selected host evaluation.
use crate::{
    host_facts::{
        bridge::FactReader,
        draft,
        inputs::{ResourceAssociation, ResourceEntry, ScopedInputs},
        FactsError,
    },
    wire::Fact,
};
use mcdev_core::{
    host_source::{HostAnswer, HostFact, HostSourceResult},
    mixin_facade::{HostMixinSourceProvider, RegisteredMixinSource},
    model::SourceScanEntry,
    project::{
        is_excluded_from_mapping_discovery, is_under_excluded_directory, normalize_path,
        uri_to_path, AccessTransformerRef, AccessWidenerRef, AwAtDiscoveryService,
        CachedProjectSession, ClasspathSnapshot, MappingDiscoveryService,
        MixinConfigDiscoveryService, MixinConfigRef, PlatformDetector, ProjectContext,
        ProjectIndexState, ResourceIssue, SourceSetContext,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

/// `source_entries` is unavailable until the host proves all project source sets.
pub(crate) struct SelectedProject {
    pub cached_session: CachedProjectSession,
    pub sources: HostFact<Vec<SourceScanEntry>>,
    pub provider: Arc<dyn HostMixinSourceProvider>,
    pub project_complete: bool,
}

fn required<T>(fact: Fact<T>, name: &str) -> Result<T, FactsError> {
    match fact {
        Fact::Found { value } => Ok(value),
        Fact::Unknown { .. } | Fact::DefinitelyAbsent => {
            Err(FactsError::Unknown(format!("{name} is unavailable")))
        }
    }
}

fn coverage(
    fact: Fact<serde_json::Value>,
    expected: serde_json::Value,
    name: &str,
) -> Result<(), FactsError> {
    let value = required(fact, name)?;
    if value == expected {
        Ok(())
    } else {
        Err(FactsError::Protocol(format!("invalid {name}")))
    }
}

fn path_from_uri(fact: &Fact<String>) -> Result<Option<PathBuf>, FactsError> {
    let Fact::Found { value } = fact else {
        return Ok(None);
    };
    if !value.starts_with("file:") {
        return Err(FactsError::Protocol(
            "host document URI is not a file URI".into(),
        ));
    }
    let path = uri_to_path(value).map_err(FactsError::Protocol)?;
    if !path.is_absolute() {
        return Err(FactsError::Protocol(
            "host document URI is not absolute".into(),
        ));
    }
    Ok(Some(normalize_path(&path)))
}

fn relative_name(entry: &ResourceEntry) -> PathBuf {
    PathBuf::from(entry.descriptor.relative_path.replace('\\', "/"))
}

fn required_resource(entry: &ResourceEntry, root: &Path) -> Result<bool, FactsError> {
    let path = relative_name(entry);
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let ext = path.extension().unwrap_or_default().to_string_lossy();
    let ordinary = MixinConfigDiscoveryService::is_mixin_config_file(&path)
        || AwAtDiscoveryService::is_access_widener_file(&path)
        || AwAtDiscoveryService::is_access_transformer_file(&path);
    let mapping = matches!(ext.to_ascii_lowercase().as_str(), "tiny" | "srg" | "tsrg");
    let gradle = matches!(
        name.as_ref(),
        "build.gradle" | "build.gradle.kts" | "settings.gradle" | "settings.gradle.kts"
    );
    if !ordinary && !mapping && !gradle {
        return Ok(false);
    }
    let Some(absolute) = path_from_uri(&entry.descriptor.document_uri)? else {
        return match entry.descriptor.association {
            ResourceAssociation::Source | ResourceAssociation::Configuration => {
                Err(FactsError::Unknown(format!(
                    "resource {} has no authoritative URI",
                    entry.descriptor.relative_path
                )))
            }
            _ => Ok(false),
        };
    };
    let Some(relative) = workspace_relative(root, &absolute) else {
        return Ok(false);
    };
    if gradle && relative == Path::new(name.as_ref()) {
        return Ok(true);
    }
    if mapping && !is_excluded_from_mapping_discovery(&relative) {
        return Ok(true);
    }
    Ok(ordinary && !is_under_excluded_directory(relative.parent().unwrap_or(Path::new(""))))
}

fn workspace_relative(root: &Path, path: &Path) -> Option<PathBuf> {
    if let Ok(relative) = path.strip_prefix(root) {
        return Some(relative.to_owned());
    }
    if !cfg!(windows) {
        return None;
    }
    let mut path_parts = path.components();
    for root_part in root.components() {
        let path_part = path_parts.next()?;
        if !root_part
            .as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(&path_part.as_os_str().to_string_lossy())
        {
            return None;
        }
    }
    Some(path_parts.collect())
}

fn source_root(path: &Path, relative: &Path) -> Option<PathBuf> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return None;
    }
    let mut root = path.to_path_buf();
    for _ in relative.components() {
        if !root.pop() {
            return None;
        }
    }
    (root.join(relative) == path).then_some(root)
}

#[derive(Default)]
struct SnapshotProvider {
    text_by_path: BTreeMap<PathBuf, String>,
    configs: BTreeMap<PathBuf, MixinConfigRef>,
}

impl HostMixinSourceProvider for SnapshotProvider {
    fn config(&self, path: &Path) -> HostSourceResult<MixinConfigRef> {
        let path = normalize_path(path);
        Ok(match self.configs.get(&path) {
            Some(config) => HostAnswer::known(config.clone(), vec![]),
            None => HostAnswer::unavailable("configuration outside the captured resource snapshot"),
        })
    }

    fn compilation_unit_source(
        &self,
        _fqn: &str,
        _current_path: Option<&Path>,
    ) -> HostSourceResult<RegisteredMixinSource> {
        Ok(HostAnswer::unavailable(
            "source-to-type association is unavailable",
        ))
    }

    fn read_source_file(&self, path: &Path) -> HostSourceResult<String> {
        let path = normalize_path(path);
        Ok(match self.text_by_path.get(&path) {
            Some(text) => HostAnswer::known(text.clone(), vec![]),
            None => HostAnswer::unavailable("source is outside the captured host inventory"),
        })
    }
}

pub(crate) fn selected_project<R: FactReader>(
    inputs: &mut ScopedInputs<R>,
    facts: draft::ProjectContext,
    workspace_root: &str,
    evaluation: &str,
) -> Result<SelectedProject, FactsError> {
    let root = uri_to_path(workspace_root).map_err(FactsError::Protocol)?;
    if !root.is_absolute() {
        return Err(FactsError::Protocol(
            "workspace root is not absolute".into(),
        ));
    }
    let root = normalize_path(&root);
    let scope = required(facts.scope, "selected project scope")?;
    let source_roots = required(facts.source_roots, "selected source roots")?;
    let output_roots = required(facts.output_roots, "selected output roots")?;
    if scope.module.is_empty() || scope.source_set.is_empty() {
        return Err(FactsError::Protocol(
            "invalid selected project scope".into(),
        ));
    }
    if source_roots
        .iter()
        .chain(&output_roots)
        .any(|path| !Path::new(path).is_absolute())
    {
        return Err(FactsError::Protocol(
            "host project root is not absolute".into(),
        ));
    }
    let selected_classpath = required(facts.classpath, "selected classpath")?;
    required(selected_classpath.coverage, "selected classpath coverage")?;
    // SDK authority is checked by class and bytecode queries when used.
    facts.sdk.validate().map_err(FactsError::Protocol)?;

    let source_inventory = inputs.source_inventory()?;
    // A partial selected source inventory cannot prove absence; individual
    // successfully returned snapshots can still serve their exact URI.
    source_inventory
        .coverage
        .validate()
        .map_err(FactsError::Protocol)?;
    if !matches!(
        &source_inventory.coverage,
        Fact::Found {
            value: serde_json::Value::Null
        } | Fact::Unknown { .. }
    ) {
        return Err(FactsError::Protocol(
            "invalid source inventory coverage".into(),
        ));
    }
    let snapshots = inputs.source_snapshots(&source_inventory.entries)?;
    if snapshots.len() != source_inventory.entries.len() {
        return Err(FactsError::Protocol(
            "source snapshot cardinality mismatch".into(),
        ));
    }
    let mut provider = SnapshotProvider::default();
    for snapshot in snapshots {
        match snapshot {
            Fact::Found { value: snapshot } => {
                if let Some(path) = path_from_uri(&snapshot.descriptor.document_uri)? {
                    provider.text_by_path.insert(path, snapshot.text);
                }
            }
            Fact::Unknown { .. } | Fact::DefinitelyAbsent => {}
        }
    }

    let resources = inputs.resource_inventory()?;
    coverage(
        resources.coverage,
        serde_json::json!(true),
        "selected resource inventory coverage",
    )?;
    let mut entries = Vec::new();
    for entry in resources.entries {
        if required_resource(&entry, &root)? {
            entries.push(entry);
        }
    }
    let contents = inputs.resource_snapshots(&entries)?;
    if contents.len() != entries.len() {
        return Err(FactsError::Protocol(
            "resource snapshot cardinality mismatch".into(),
        ));
    }
    let mut gradle = vec![];
    let mut mapping_inputs = vec![];
    let mut configs = vec![];
    let mut wideners = vec![];
    let mut transformers = vec![];
    let mut issues = vec![];
    let mut resource_roots = BTreeSet::new();
    for (entry, content) in entries.into_iter().zip(contents) {
        let content = required(content, "selected resource snapshot")?;
        if !content.utf8 {
            return Err(FactsError::Unknown(format!(
                "resource {} is not UTF-8",
                entry.descriptor.relative_path
            )));
        }
        let text = String::from_utf8(content.bytes)
            .map_err(|_| FactsError::Protocol("host marked non-UTF-8 resource as UTF-8".into()))?;
        let relative = relative_name(&entry);
        let path = path_from_uri(&entry.descriptor.document_uri)?;
        if let Some(path) = &path {
            if provider
                .text_by_path
                .get(path)
                .is_some_and(|previous| previous != &text)
            {
                return Err(FactsError::Stale(
                    "host returned conflicting snapshots for one URI".into(),
                ));
            }
            provider.text_by_path.insert(path.clone(), text.clone());
            if entry.descriptor.association == ResourceAssociation::Source {
                if let Some(dir) = source_root(path, &relative) {
                    resource_roots.insert(dir);
                }
            }
        }
        let name = relative.file_name().unwrap_or_default().to_string_lossy();
        if matches!(
            name.as_ref(),
            "build.gradle" | "build.gradle.kts" | "settings.gradle" | "settings.gradle.kts"
        ) {
            gradle.push(text.clone());
        }
        let ext = relative
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase();
        if matches!(ext.as_str(), "tiny" | "srg" | "tsrg") {
            mapping_inputs.push((
                path.clone().unwrap_or_else(|| relative.clone()),
                text.clone(),
            ));
        }
        // A known file URI is required before using a resource as a config/edit target.
        let path = path.ok_or_else(|| {
            FactsError::Unknown(format!(
                "editable resource {} has no authoritative URI",
                entry.descriptor.relative_path
            ))
        })?;
        if MixinConfigDiscoveryService::is_mixin_config_file(&relative) {
            match MixinConfigDiscoveryService::parse_content(&path, &text) {
                Some(config) => {
                    provider.configs.insert(path.clone(), config.clone());
                    configs.push(config);
                }
                None => issues.push(ResourceIssue {
                    path: path.clone(),
                    message: "Invalid or unreadable Mixin config".into(),
                }),
            }
        }
        if AwAtDiscoveryService::is_access_widener_file(&relative) {
            let parsed = mcdev_core::aw::AccessWidenerParser::parse(&text).ok();
            wideners.push(AccessWidenerRef {
                path: path.clone(),
                namespace: parsed.as_ref().map(|p| p.namespace),
                parsed,
            });
        }
        if AwAtDiscoveryService::is_access_transformer_file(&relative) {
            transformers.push(AccessTransformerRef {
                path,
                parsed: mcdev_core::at::AccessTransformerParser::parse(&text).ok(),
            });
        }
    }
    let sort_path = |path: &Path| {
        let value = path.to_string_lossy().into_owned();
        if cfg!(windows) {
            value.replace('/', "\\").to_uppercase()
        } else {
            value
        }
    };
    mapping_inputs.sort_by_key(|(path, _)| sort_path(path));
    configs.sort_by_key(|config| sort_path(&config.path));
    wideners.sort_by_key(|aw| sort_path(&aw.path));
    transformers.sort_by_key(|at| sort_path(&at.path));
    let platform = PlatformDetector::detect(&gradle);
    let mappings =
        MappingDiscoveryService::build_mapping_context_from_contents(&mapping_inputs, platform)
            .map_err(|error| FactsError::Unknown(format!("mapping snapshots: {error}")))?;
    let output_paths: Vec<PathBuf> = output_roots.into_iter().map(PathBuf::from).collect();
    let output_set: BTreeSet<_> = output_paths.iter().cloned().collect();
    let classpath = ClasspathSnapshot {
        project_outputs: output_paths.clone(),
        dependency_jars: selected_classpath
            .entries
            .into_iter()
            .map(|e| PathBuf::from(e.display_path))
            .filter(|path| !output_set.contains(path))
            .collect(),
        ..Default::default()
    };
    let source_set = SourceSetContext {
        name: scope.source_set,
        source_directories: source_roots.into_iter().map(PathBuf::from).collect(),
        resource_directories: resource_roots.into_iter().collect(),
        output_directory: output_paths.first().cloned(),
    };
    let context = Arc::new(ProjectContext {
        project_id: scope.module,
        root,
        platform,
        classpath,
        mappings,
        mixin_configs: configs,
        access_wideners: wideners,
        access_transformers: transformers,
        minecraft_jars: vec![],
        source_sets: vec![source_set],
        index_state: ProjectIndexState::NotReady,
        mapping_files: mapping_inputs.into_iter().map(|(path, _)| path).collect(),
        resource_issues: issues,
    });
    static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);
    let version = NEXT_SESSION
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
        .map_err(|_| FactsError::Protocol("session sequence exhausted".into()))?;
    let cached_session = CachedProjectSession {
        context,
        version,
        cache_hit: false,
        fingerprint: evaluation.to_owned(),
    };
    Ok(SelectedProject {
        cached_session,
        sources: HostFact::Unavailable {
            reason: "host inventory covers the selected source set, not all project source sets"
                .into(),
        },
        provider: Arc::new(provider),
        // shortcut: this API enumerates one source set, switch to complete project coverage when host publishes it.
        project_complete: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_facts::inputs::InputOps;
    use mcdev_core::{
        mapping::{ClassRef, MappingLookupResult},
        project::{path_to_uri, ModPlatform},
        MappingNamespace,
    };
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    use std::{
        collections::VecDeque,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct Reader(VecDeque<Value>);
    impl FactReader for Reader {
        fn read_facts(
            &mut self,
            evaluation: &str,
            _queries: &[Value],
        ) -> Result<Value, FactsError> {
            assert_eq!(evaluation, "eval-1");
            self.0
                .pop_front()
                .ok_or_else(|| FactsError::Protocol("unexpected host read".into()))
        }
    }
    fn found(value: Value) -> Value {
        json!({"state":"found","value":value})
    }
    fn sha(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }
    fn base64(bytes: &[u8]) -> String {
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for part in bytes.chunks(3) {
            let a = part[0];
            let b = *part.get(1).unwrap_or(&0);
            let c = *part.get(2).unwrap_or(&0);
            out.push(alphabet[(a >> 2) as usize] as char);
            out.push(alphabet[(((a & 3) << 4) | (b >> 4)) as usize] as char);
            out.push(if part.len() > 1 {
                alphabet[(((b & 15) << 2) | (c >> 6)) as usize] as char
            } else {
                '='
            });
            out.push(if part.len() > 2 {
                alphabet[(c & 63) as usize] as char
            } else {
                '='
            });
        }
        out
    }
    fn descriptor(id: &str, relative: &str, association: &str, absolute: &Path) -> Value {
        json!({"id":id,"relativePath":relative,"association":association,"classpathOrder":0,
            "editable":true,"documentUri":found(json!(path_to_uri(absolute))),"documentVersion":7,
            "identity":{"kind":"resource","revision":"1"}})
    }
    fn chunk(text: &str) -> Value {
        found(
            json!({"identity":{"kind":"resource","revision":"1","sha256":sha(text.as_bytes())},
            "documentVersion":7,"size":text.len(),"offset":0,"base64":base64(text.as_bytes()),
            "utf8":true,"text":found(json!(text)),"next":null}),
        )
    }
    fn operations() -> InputOps {
        InputOps::intersect(&json!({
            "capabilities":["source-snapshots-v1","owned-resources-v1"],
            "supportedOps":{"source-snapshots-v1":["sourceInventory","sourceSnapshot"],
                "owned-resources-v1":["resourceInventory","resourceLookup","resourceSnapshot"]}
        }))
        .unwrap()
    }
    fn metadata(root: &Path) -> draft::ProjectContext {
        serde_json::from_value(json!({
            "workspaceUri":{"state":"unknown","reasons":[{"code":"MissingFacts"}]},
            "scope":found(json!({"module":":demo","sourceSet":"main"})),
            "sourceRoots":found(json!([root.join("src/main/java")])),
            "outputRoots":found(json!([root.join("build/classes")])),
            "classpath":found(json!({"entries":[],"coverage":found(Value::Null)})),
            "sdk":found(json!({"identity":"selected-sdk","api":{"kind":"full"}}))
        }))
        .unwrap()
    }
    fn unique_root() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("mcdev-ipc-snapshot-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn host_snapshots_build_selected_project_without_scanning_or_claiming_other_sets() {
        let root = unique_root();
        assert!(!root.exists());
        let java = root.join("src/main/java/Owner.java");
        let config = root.join("src/main/resources/demo.mixins.json");
        let shared_config = root.join("mixins.shared.json");
        let mappings = root.join("src/main/resources/demo.tiny");
        let build = root.join("build.gradle");
        let source = "class Owner {}";
        let config_text = r#"{"package":"demo","mixins":["Patch"]}"#;
        let shared_config_text = r#"{"package":"demo","mixins":["Shared"]}"#;
        let mapping_text = "tiny\t2\t0\tnamed\tintermediary\nc\tdemo/Target\tx/T\n";
        let source_descriptor = json!({"id":"src:1","displayPath":java.to_string_lossy(),
            "documentUri":found(json!(path_to_uri(&java))),"languageId":"java",
            "identity":{"kind":"source","revision":"1"}});
        let source_snapshot = json!({"id":"src:1","displayPath":java.to_string_lossy(),
            "documentUri":found(json!(path_to_uri(&java))),"languageId":"java",
            "identity":{"kind":"source","revision":"1"},"text":source,"sha256":sha(source.as_bytes())});
        let replies = VecDeque::from(vec![
            json!({"facts":[{"sources":[source_descriptor],"next":null,"coverage":found(Value::Null)}]}),
            json!({"facts":[found(source_snapshot)]}),
            json!({"facts":[{"resources":[
                descriptor("r1","build.gradle","configuration",&build),
                descriptor("r2","demo.mixins.json","source",&config),
                descriptor("r3","demo.tiny","source",&mappings),
                descriptor("r4","mixins.shared.json","configuration",&shared_config),
                {"id":"jar1","relativePath":"build.gradle","association":"runtimeClasspath","classpathOrder":1,
                    "editable":false,"documentUri":{"state":"unknown","reasons":[{"code":"MissingFacts"}]},
                    "identity":{"kind":"resource","revision":"1"}},
                {"id":"jar2","relativePath":"mappings.tiny","association":"compileClasspath","classpathOrder":1,
                    "editable":false,"documentUri":{"state":"unknown","reasons":[{"code":"MissingFacts"}]},
                    "identity":{"kind":"resource","revision":"1"}}],
                "next":null,"coverage":found(json!(true))}]}),
            json!({"facts":[chunk("fabric-loom"),chunk(config_text),chunk(mapping_text),chunk(shared_config_text)]}),
        ]);
        let mut inputs = ScopedInputs::new("eval-1", &operations(), Reader(replies)).unwrap();
        let selected = selected_project(
            &mut inputs,
            metadata(&root),
            &root.to_string_lossy(),
            "eval-1",
        )
        .unwrap();
        let context = &selected.cached_session.context;
        assert_eq!(context.platform, ModPlatform::Fabric);
        assert_eq!(
            context.source_sets[0].resource_directories,
            vec![root.join("src/main/resources")]
        );
        assert!(context
            .mixin_configs
            .iter()
            .any(|config| config.mixins == ["Patch"]));
        assert!(context
            .mixin_configs
            .iter()
            .any(|config| config.mixins == ["Shared"]));
        assert!(matches!(
            context.mappings.resolver.remap_class(
                &ClassRef {
                    internal_name: "demo/Target".into(),
                    namespace: MappingNamespace::Named
                },
                MappingNamespace::Intermediary
            ),
            MappingLookupResult::Found(_)
        ));
        assert!(matches!(selected.sources, HostFact::Unavailable { .. }));
        assert!(!selected.project_complete);
        assert!(
            matches!(selected.provider.read_source_file(&config).unwrap().fact, HostFact::Known(value) if value == config_text)
        );
        assert!(matches!(
            selected
                .provider
                .read_source_file(&root.join("other"))
                .unwrap()
                .fact,
            HostFact::Unavailable { .. }
        ));
        assert!(!root.exists());
    }

    #[test]
    fn missing_resource_coverage_never_becomes_a_successful_empty_project() {
        let root = unique_root();
        let replies = VecDeque::from(vec![
            json!({"facts":[{"sources":[],"next":null,"coverage":found(Value::Null)}]}),
            json!({"facts":[{"resources":[],"next":null,"coverage":{"state":"unknown","reasons":[{"code":"MissingFacts"}]}}]}),
        ]);
        let mut inputs = ScopedInputs::new("eval-1", &operations(), Reader(replies)).unwrap();
        assert!(matches!(
            selected_project(
                &mut inputs,
                metadata(&root),
                &root.to_string_lossy(),
                "eval-1"
            ),
            Err(FactsError::Unknown(_))
        ));
    }

    #[test]
    fn relative_host_source_roots_are_rejected_before_any_host_read() {
        let root = unique_root();
        let mut facts = metadata(&root);
        facts.source_roots = Fact::Found {
            value: vec!["relative/source".into()],
        };
        let mut inputs =
            ScopedInputs::new("eval-1", &operations(), Reader(VecDeque::new())).unwrap();
        assert!(matches!(
            selected_project(&mut inputs, facts, &root.to_string_lossy(), "eval-1"),
            Err(FactsError::Protocol(_))
        ));
    }

    #[test]
    fn unknown_sdk_does_not_block_owned_project_snapshots() {
        let root = unique_root();
        let replies = VecDeque::from(vec![
            json!({"facts":[{"sources":[],"next":null,"coverage":found(Value::Null)}]}),
            json!({"facts":[{"resources":[],"next":null,"coverage":found(json!(true))}]}),
        ]);
        let mut facts = metadata(&root);
        facts.sdk = Fact::Unknown {
            reasons: vec![json!({"code":"MissingFacts"})],
        };
        let mut inputs = ScopedInputs::new("eval-1", &operations(), Reader(replies)).unwrap();
        let selected =
            selected_project(&mut inputs, facts, &root.to_string_lossy(), "eval-1").unwrap();
        assert_eq!(selected.cached_session.context.source_sets[0].name, "main");
        assert!(matches!(selected.sources, HostFact::Unavailable { .. }));
        assert!(!selected.project_complete);

        let mut malformed = metadata(&root);
        malformed.sdk = Fact::Unknown { reasons: vec![] };
        let mut inputs =
            ScopedInputs::new("eval-1", &operations(), Reader(VecDeque::new())).unwrap();
        assert!(matches!(
            selected_project(&mut inputs, malformed, &root.to_string_lossy(), "eval-1"),
            Err(FactsError::Protocol(_))
        ));
    }

    #[test]
    fn runtime_bytecode_stays_unavailable_when_selected_sdk_is_unknown() {
        use crate::commands::{
            ipc::index::{EvaluationFactReader, EvaluationIndex},
            CommandFailure,
        };
        use mcdev_core::bytecode::BytecodeIndex;
        use std::sync::atomic::{AtomicUsize, Ordering};

        #[derive(Clone)]
        struct MissingSdk(Arc<AtomicUsize>);
        impl FactReader for MissingSdk {
            fn read_facts(
                &mut self,
                evaluation: &str,
                queries: &[Value],
            ) -> Result<Value, FactsError> {
                assert_eq!(evaluation, "eval-1");
                assert_eq!(
                    queries,
                    &[json!({"op":"runtimeClass","internalName":"java/lang/Object"})]
                );
                self.0.fetch_add(1, Ordering::Relaxed);
                Ok(
                    json!({"facts":[{"state":"unknown","reasons":[{"code":"BYTECODE_UNAVAILABLE","detail":"selected SDK unavailable"}]}]}),
                )
            }
        }
        impl EvaluationFactReader for MissingSdk {
            fn check_active(&self) -> Result<(), FactsError> {
                Ok(())
            }
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let reader = MissingSdk(calls.clone());
        let ops = InputOps::intersect(&json!({"capabilities":["runtime-class-bytes-v1"],
            "supportedOps":{"runtime-class-bytes-v1":["runtimeClass","classBytes"]}}))
        .unwrap();
        let inputs = ScopedInputs::new("eval-1", &ops, reader.clone()).unwrap();
        let index = Arc::new(EvaluationIndex::new(
            Arc::new(std::sync::Mutex::new(inputs)),
            reader,
        ));
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            index.physical_view().get_class_bytes("java/lang/Object")
        }))
        .expect_err("unknown SDK must not yield missing bytes or a successful empty result");
        assert!(matches!(failure.downcast_ref::<CommandFailure>(),
            Some(CommandFailure::Unavailable(error)) if error.code == mcdev_protocol::McdevErrorCode::BytecodeUnavailable));
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }
}
