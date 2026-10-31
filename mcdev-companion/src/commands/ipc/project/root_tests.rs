use super::tests::{found, model_replies, owning_metadata, sha, unique_root, Reader};
use super::*;
use crate::host_facts::inputs::InputOps;
use mcdev_core::{
    bytecode::{AtTargetCandidate, BytecodeIndex},
    host_source::{
        HostGenerations, HostReadSet, HostReadValidator, HostSemanticModel,
        HostSemanticModelProvider, HostSemanticRequest, HostSourceError,
    },
    mixinextras_handler::HandlerReferenceTypeResolver,
    model::{ClassIndex, ClassIndexEntry, FieldIndexEntry, MethodIndexEntry},
};
use mcdev_core::{
    mapping::{ClassRef, MappingLookupResult},
    project::{path_to_uri, ModPlatform},
    MappingNamespace,
};
use serde_json::{json, Value};
use std::sync::Mutex;

struct NoSemanticReads;
impl ClassIndex for NoSemanticReads {
    fn find_classes(&self, _: &str, _: usize) -> Vec<ClassIndexEntry> {
        panic!("context DTO queried classes")
    }
    fn find_class(&self, _: &str) -> Option<ClassIndexEntry> {
        panic!("context DTO queried a class")
    }
    fn find_class_by_fqn(&self, _: &str) -> Option<ClassIndexEntry> {
        panic!("context DTO queried a source type")
    }
    fn get_methods(&self, _: &str) -> Vec<MethodIndexEntry> {
        panic!("context DTO queried methods")
    }
    fn get_fields(&self, _: &str) -> Vec<FieldIndexEntry> {
        panic!("context DTO queried fields")
    }
}
impl BytecodeIndex for NoSemanticReads {
    fn get_at_target_candidates(
        &self,
        _: &str,
        _: &str,
        _: Option<&str>,
        _: &str,
    ) -> Vec<AtTargetCandidate> {
        panic!("context DTO queried Code")
    }
    fn get_return_ordinal_count(&self, _: &str, _: &str, _: Option<&str>) -> usize {
        panic!("context DTO queried instructions")
    }
}
impl HandlerReferenceTypeResolver for NoSemanticReads {
    fn is_known_reference(&self, _: &str) -> Option<bool> {
        panic!("context DTO queried a reference type")
    }
    fn is_assignable(&self, _: &str, _: &str) -> Option<bool> {
        panic!("context DTO queried assignability")
    }
}
impl HostReadValidator for NoSemanticReads {
    fn validate_current_reads(
        &self,
        _: HostGenerations,
        _: &HostReadSet,
    ) -> Result<bool, HostSourceError> {
        panic!("context DTO queried semantics")
    }
}
impl HostSemanticModelProvider for NoSemanticReads {
    fn parse(&self, _: &HostSemanticRequest<'_>) -> HostSourceResult<HostSemanticModel> {
        panic!("context DTO parsed source")
    }
}

struct RootReader {
    base: Reader,
    workspace: PathBuf,
    files: BTreeMap<String, String>,
    issued: BTreeMap<String, (String, Value, String)>,
    unknown_domain: Option<&'static str>,
    missing: BTreeSet<String>,
    conflict: Option<String>,
    version_conflict: bool,
    stale: bool,
    stale_domain: Option<&'static str>,
    root_override: Option<PathBuf>,
    reads: Arc<Mutex<Vec<String>>>,
}

fn operations() -> InputOps {
    InputOps::intersect(&json!({"capabilities":["project-inputs-v1","source-snapshots-v1","owned-resources-v1","root-files-v1"],
        "supportedOps":{"project-inputs-v1":["projectInputs"],"source-snapshots-v1":["sourceInventory","sourceSnapshot"],
            "owned-resources-v1":["resourceInventory","resourceLookup","resourceSnapshot"],
            "root-files-v1":["rootFileInventory","rootFileSnapshot"]}})).unwrap()
}

impl RootReader {
    fn new(workspace: &Path) -> Self {
        let files = [
            ("module/src/main/java/Patch.java","package fixture; class Patch {}"),
            ("module/src/client/java/Other.java","package fixture; class Other {}"),
            ("module/src/client/resources/client.mixins.json",r#"{"package":"fixture","mixins":[]}"#),
            ("module/mixins.shared.json",r#"{"package":"fixture","mixins":["Patch"]}"#),
            ("module/src/client/resources/demo.aw","accessWidener v2 named\n"),
            ("mixins.root.json",r#"{"package":"fixture","mixins":["Patch"]}"#),
            ("outside.aw","accessWidener v2 named\naccessible class game/Target\n"),
            ("legacy_at.cfg","public game.Target\n"),
            ("build.gradle","plugins { id 'fabric-loom' }"),
            (".gradle/loom-cache/cache.tiny","tiny\t2\t0\tnamed\tintermediary\nc\tgame/Target\tx/T\n"),
            ("build/tmp/second.srg","CL: other/Thing other/T\n"),
            (".gradle/other/ignored.tiny","not a mapping"),
            ("build/unselected/ignored.mixins.json","{}"),
            ("node_modules/ignored.mixins.json","{}"),
            ("module/src/main/java/Excluded.java","@Mixin(game.Target.class) class Excluded { @Inject(method = \"tick()V\", at = @At(\"HEAD\")) void handler() {} }"),
            ("module/src/main/java/build/Nested.java","class Nested {}"),
            ("module/src/main/java/Upper.JAVA","class Upper {}"),
            ("module/src/main/java/Other.kt","class Other"),
        ].into_iter().map(|(path,text)|(path.into(),text.into())).collect();
        Self {
            base: Reader(model_replies(&workspace.join("module"), &[])),
            workspace: workspace.into(),
            files,
            issued: BTreeMap::new(),
            unknown_domain: None,
            missing: BTreeSet::new(),
            conflict: None,
            version_conflict: false,
            stale: false,
            stale_domain: None,
            root_override: None,
            reads: Arc::new(Mutex::new(vec![])),
        }
    }
}

impl FactReader for RootReader {
    fn read_facts(&mut self, evaluation: &str, queries: &[Value]) -> Result<Value, FactsError> {
        if queries
            .first()
            .and_then(|query| query["op"].as_str())
            .is_none_or(|op| !op.starts_with("rootFile"))
        {
            return self.base.read_facts(evaluation, queries);
        }
        if self.stale {
            return Err(FactsError::Stale("root catalog expired".into()));
        }
        let mut facts = vec![];
        for query in queries {
            if query["op"] == "rootFileInventory" {
                let filter = &query["filter"];
                let tag = if filter == &serde_json::to_value(roots::resource_filter()).unwrap() {
                    "resources"
                } else if filter == &serde_json::to_value(roots::mapping_filter()).unwrap() {
                    "mappings"
                } else {
                    "references"
                };
                if self.stale_domain == Some(tag) {
                    return Err(FactsError::Stale(format!("{tag} catalog expired")));
                }
                let revision = match tag {
                    "resources" => "11",
                    "mappings" => "22",
                    _ => "33",
                };
                let root = json!({"identity":format!("catalog-{tag}"),"uri":path_to_uri(self.root_override.as_deref().unwrap_or(&self.workspace)),"rootKind":"workspace","revision":revision});
                let mut files = vec![];
                for (relative, text) in &self.files {
                    let path = Path::new(relative);
                    let name = path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .to_ascii_lowercase();
                    let extension = path
                        .extension()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_ascii_lowercase();
                    let excluded = relative.split('/').rev().skip(1).any(|part| {
                        filter["excludedDirectories"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|value| {
                                value
                                    .as_str()
                                    .is_some_and(|value| value.eq_ignore_ascii_case(part))
                            })
                    });
                    let positive = filter["basenames"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|value| value == &name)
                        || filter["extensions"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|value| value == &extension);
                    if excluded || !positive {
                        continue;
                    }
                    let file = format!("file-{tag}-{relative}");
                    self.issued
                        .insert(file.clone(), (relative.clone(), root.clone(), text.clone()));
                    files.push(json!({"file":file,"uri":path_to_uri(&self.workspace.join(relative)),"relativePath":relative}));
                }
                let unknown = self.unknown_domain == Some(tag);
                facts.push(json!({"root":root,"filter":filter,"files":files,"next":null,
                    "coverage":if unknown {json!({"state":"unknown","reasons":[{"code":"WatchUnavailable"}]})} else {found(json!(true))},
                    "problems":if unknown {vec!["watch-unavailable"]} else {vec![]}}));
            } else {
                let file = query["file"].as_str().unwrap();
                let (relative, root, mut text) = self.issued.get(file).unwrap().clone();
                self.reads.lock().unwrap().push(relative.clone());
                if self.missing.contains(&relative) {
                    facts.push(json!({"state":"unknown","reasons":[{"code":"BodyUnavailable"}]}));
                    continue;
                }
                if file.starts_with("file-references-") && self.conflict.as_ref() == Some(&relative)
                {
                    text.push_str("# changed\n");
                }
                let version = if self.version_conflict
                    && file.starts_with("file-references-")
                    && relative == "module/src/client/resources/demo.aw"
                {
                    8
                } else {
                    7
                };
                facts.push(found(json!({"file":file,"root":root,"uri":path_to_uri(&self.workspace.join(&relative)),"relativePath":relative,
                    "identity":{"kind":"rootFile","revision":"73","sha256":sha(text.as_bytes())},"documentVersion":version,
                    "size":text.len(),"offset":0,"text":text,"next":null})));
            }
        }
        Ok(json!({"facts":facts}))
    }
}

fn project(reader: RootReader) -> Result<SelectedProject, FactsError> {
    let workspace = reader.workspace.clone();
    let mut inputs = ScopedInputs::new("eval-1", &operations(), reader)?;
    selected_project(
        &mut inputs,
        owning_metadata(&workspace.join("module")),
        &path_to_uri(&workspace),
        "eval-1",
    )
}

fn project_for_command(
    reader: RootReader,
    command: &mcdev_protocol::DecodedCommand,
) -> Result<SelectedProject, FactsError> {
    let workspace = reader.workspace.clone();
    let mut inputs = ScopedInputs::new("eval-1", &operations(), reader)?;
    selected_project_for_command(
        &mut inputs,
        owning_metadata(&workspace.join("module")),
        &path_to_uri(&workspace),
        "eval-1",
        command,
    )
}

fn command(
    workspace: &Path,
    name: &str,
    marked_source: &str,
    file: &str,
    language: &str,
    codes: &[&str],
) -> mcdev_protocol::DecodedCommand {
    let offset = marked_source.find('|').unwrap_or(marked_source.len());
    let prefix = &marked_source[..offset];
    let position = json!({"line":prefix.bytes().filter(|byte|*byte==b'\n').count(),
        "character":prefix.rsplit('\n').next().unwrap().encode_utf16().count()});
    mcdev_protocol::ProtocolPayloadDecoder.decode_command(name,&[json!({"context":{
        "protocolVersion":1,"workspaceRoot":path_to_uri(workspace),"documentUri":path_to_uri(&workspace.join(file)),
        "languageId":language,"position":position,"bufferText":marked_source.replacen('|',"",1),"documentVersion":7},
        "diagnosticCodes":codes})]).unwrap()
}

#[test]
fn command_discovery_does_not_read_unrelated_root_domains() {
    let workspace = unique_root();
    for command in [
        command(
            &workspace,
            "mcdev.codeAction",
            "class Patch {}",
            "module/src/main/java/Patch.java",
            "java",
            &["MIXIN_UNRESOLVED_INJECT_METHOD"],
        ),
        command(
            &workspace,
            "mcdev.completion",
            "@Mixin(targets = \"game.T|\") class Patch {}",
            "module/src/main/java/Patch.java",
            "java",
            &[],
        ),
        command(
            &workspace,
            "mcdev.completion",
            "accessWidener v2 named\naccessible class game/T|",
            "outside.aw",
            "accesswidener",
            &[],
        ),
        command(
            &workspace,
            "mcdev.references",
            "accessWidener |",
            "outside.aw",
            "accesswidener",
            &[],
        ),
    ] {
        let mut reader = RootReader::new(&workspace);
        reader.stale = true;
        let selected = project_for_command(reader, &command).unwrap();
        assert!(
            !selected.project_complete
                && !selected.mapping_complete
                && !selected.references_complete
        );
        assert_eq!(selected.source_snapshots.len(), 2);
    }
    for name in [
        "mcdev.info",
        "mcdev.dumpContext",
        "mcdev.reindex",
        "mcdev.reloadProjectContext",
    ] {
        let command = command(
            &workspace,
            name,
            "class Patch {}",
            "module/src/main/java/Patch.java",
            "java",
            &[],
        );
        let mut reader = RootReader::new(&workspace);
        reader.stale_domain = Some("references");
        let selected = project_for_command(reader, &command).unwrap();
        assert!(
            selected.project_complete
                && selected.config_selection_complete
                && selected.mapping_complete
        );
        assert!(!selected.references_complete);
    }
    let request = command(
        &workspace,
        "mcdev.completion",
        "accessWidener v2 intermediary\naccessible class x/T|",
        "outside.aw",
        "accesswidener",
        &[],
    );
    let mut reader = RootReader::new(&workspace);
    reader.missing.insert("mixins.root.json".into());
    reader.stale_domain = Some("references");
    let reads = reader.reads.clone();
    let selected = project_for_command(reader, &request).unwrap();
    assert!(selected.mapping_complete);
    assert!(!selected.config_selection_complete && !selected.project_complete);
    assert!(!reads
        .lock()
        .unwrap()
        .iter()
        .any(|path| path.ends_with(".json") || path.ends_with(".java")));
}

#[test]
fn needed_command_domains_preserve_expired_evaluation_errors() {
    let workspace = unique_root();
    for (name, source, file, language, codes, domain) in [
        (
            "mcdev.diagnostics",
            "class Patch {}",
            "module/src/main/java/Patch.java",
            "java",
            vec![],
            "resources",
        ),
        (
            "mcdev.codeAction",
            "class Patch {}",
            "module/src/main/java/Patch.java",
            "java",
            vec![],
            "resources",
        ),
        (
            "mcdev.codeAction",
            "class Patch {}",
            "module/src/main/java/Patch.java",
            "java",
            vec![
                "MIXIN_UNRESOLVED_INJECT_METHOD",
                "MIXIN_CLASS_NOT_LISTED_IN_CONFIG",
            ],
            "resources",
        ),
        (
            "mcdev.completion",
            "@Share(\"te|\") class Patch {}",
            "module/src/main/java/Patch.java",
            "java",
            vec![],
            "resources",
        ),
        (
            "mcdev.completion",
            "accessWidener v2 intermediary\naccessible class x/T|",
            "outside.aw",
            "accesswidener",
            vec![],
            "mappings",
        ),
        (
            "mcdev.references",
            "@Mixin(targets = \"game.T|\") class Patch {}",
            "module/src/main/java/Patch.java",
            "java",
            vec![],
            "references",
        ),
    ] {
        let command = command(&workspace, name, source, file, language, &codes);
        let mut reader = RootReader::new(&workspace);
        reader.stale_domain = Some(domain);
        assert!(
            matches!(
                project_for_command(reader, &command),
                Err(FactsError::Stale(_))
            ),
            "{name}: {domain}"
        );
    }
}

#[test]
fn root_files_restore_workspace_discovery_without_broadening_compiler_inputs() {
    let workspace = unique_root();
    let reader = RootReader::new(&workspace);
    let reads = reader.reads.clone();
    let selected = project(reader).unwrap();
    let context = &selected.cached_session.context;
    assert_eq!(context.root, workspace);
    assert_eq!(
        context.source_sets[1].source_directories,
        [workspace.join("module/src/main/java")]
    );
    assert_eq!(context.platform, ModPlatform::Fabric);
    assert!(
        selected.project_complete
            && selected.config_selection_complete
            && selected.mapping_complete
            && selected.references_complete
    );
    assert!(context
        .mixin_configs
        .iter()
        .any(|config| config.path == workspace.join("mixins.root.json")));
    assert!(!context
        .mixin_configs
        .iter()
        .any(|config| config.path.to_string_lossy().contains("ignored")));
    assert!(context
        .access_wideners
        .iter()
        .any(|aw| aw.path == workspace.join("outside.aw")));
    assert_eq!(
        context.access_transformers[0].path,
        workspace.join("legacy_at.cfg")
    );
    assert!(context
        .mapping_files
        .contains(&workspace.join(".gradle/loom-cache/cache.tiny")));
    assert!(context
        .mapping_files
        .contains(&workspace.join("build/tmp/second.srg")));
    assert!(
        matches!(context.mappings.resolver.remap_class(&ClassRef {internal_name:"game/Target".into(),namespace:MappingNamespace::Named},
        MappingNamespace::Intermediary),MappingLookupResult::Found(value) if value.internal_name=="x/T")
    );
    let HostFact::Known(sources) = selected.sources else {
        panic!("legacy reference inventory unavailable")
    };
    assert!(sources
        .iter()
        .any(|source| source.document_uri.ends_with("/Excluded.java")));
    assert!(sources
        .iter()
        .any(|source| source.document_uri.ends_with("/build/Nested.java")));
    assert!(!sources
        .iter()
        .any(|source| source.document_uri.ends_with(".kt")
            || source.document_uri.ends_with(".JAVA")
            || source.document_uri.ends_with("/outside.aw")));
    assert_eq!(selected.source_snapshots.len(), 2);
    assert_eq!(selected.owner_sources.inventory.entries.len(), 1);
    assert!(!reads
        .lock()
        .unwrap()
        .iter()
        .any(|path| path.contains("ignored") || path.ends_with(".kt") || path.ends_with(".JAVA")));
    assert!(!workspace.exists());
}

#[test]
fn copy_excluded_owner_config_precedes_root_shared_and_client_configs() {
    let workspace = unique_root();
    let mut reader = RootReader::new(&workspace);
    reader.files.insert(
        "module/src/main/resources/copy-excluded.mixins.json".into(),
        r#"{"package":"fixture","mixins":[]}"#.into(),
    );
    let selected = project(reader).unwrap();
    let config = mcdev_core::mixin_facade::HostMixinFacade::selected_mixin_config(
        &selected.cached_session.context,
        "package fixture; class Patch {}",
        Some(&path_to_uri(
            &workspace.join("module/src/main/java/Patch.java"),
        )),
        selected.provider.as_ref(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        config.path,
        workspace.join("module/src/main/resources/copy-excluded.mixins.json")
    );
    assert!(selected.config_selection_complete);
}

#[test]
fn root_domains_keep_unknown_mapping_and_source_coverage_separate() {
    for domain in ["resources", "mappings", "references"] {
        let workspace = unique_root();
        let mut reader = RootReader::new(&workspace);
        reader.unknown_domain = Some(domain);
        let selected = project(reader).unwrap();
        assert_eq!(selected.config_selection_complete, domain != "resources");
        assert_eq!(
            selected.mapping_complete,
            domain != "resources" && domain != "mappings"
        );
        assert_eq!(selected.references_complete, domain != "references");
        assert_eq!(selected.project_complete, domain == "references");
        assert_eq!(
            matches!(selected.sources, HostFact::Known(_)),
            domain != "references"
        );
    }
}

#[test]
fn required_body_unknown_never_becomes_absent_but_unrelated_json_is_not_read() {
    let workspace = unique_root();
    let mut reader = RootReader::new(&workspace);
    reader.missing.insert("mixins.root.json".into());
    reader
        .files
        .insert("huge-unrelated.json".into(), "unrelated".into());
    let reads = reader.reads.clone();
    let selected = project(reader).unwrap();
    assert!(!selected.config_selection_complete);
    assert!(!selected.project_complete);
    assert!(selected.mapping_complete && selected.references_complete);
    assert!(!reads
        .lock()
        .unwrap()
        .contains(&"huge-unrelated.json".into()));
}

#[test]
fn root_and_selected_or_cross_domain_overlay_conflicts_are_stale() {
    for compiler_conflict in [false, true] {
        let workspace = unique_root();
        let mut reader = RootReader::new(&workspace);
        if compiler_conflict {
            reader.files.insert(
                "module/src/main/java/Patch.java".into(),
                "class Changed {}".into(),
            );
        } else {
            reader.conflict = Some("module/src/client/resources/demo.aw".into());
        }
        assert!(matches!(project(reader), Err(FactsError::Stale(_))));
    }
    let workspace = unique_root();
    let mut reader = RootReader::new(&workspace);
    reader.version_conflict = true;
    assert!(matches!(project(reader), Err(FactsError::Stale(_))));
}

#[test]
fn an_expired_root_read_is_not_a_partial_project_success() {
    let workspace = unique_root();
    let mut reader = RootReader::new(&workspace);
    reader.stale = true;
    assert!(matches!(project(reader), Err(FactsError::Stale(_))));
}

#[test]
fn same_prefix_source_set_ambiguity_is_not_replaced_with_the_command_owner() {
    let workspace = unique_root();
    let mut reader = RootReader::new(&workspace);
    reader.files.insert(
        "module/src/main/resources/copy-excluded.mixins.json".into(),
        r#"{"package":"fixture","mixins":[]}"#.into(),
    );
    for record in reader.base.0[1]["facts"][0]["records"]
        .as_array_mut()
        .unwrap()
    {
        if record["kind"] == "root"
            && record["scope"] == "scope-client"
            && record["role"] == "source"
        {
            record["path"] = json!(workspace.join("module/src/main/java"));
        }
    }
    let selected = project(reader).unwrap();
    let context = &selected.cached_session.context;
    let path = workspace.join("module/src/main/java/Patch.java");
    assert!(
        mcdev_core::project::SourceSetResolver::containing_source_set(&context.source_sets, &path)
            .is_none()
    );
    let config = mcdev_core::mixin_facade::HostMixinFacade::selected_mixin_config(
        context,
        "package fixture; class Patch {}",
        Some(&path_to_uri(&path)),
        selected.provider.as_ref(),
    )
    .unwrap();
    assert!(
        config.is_none(),
        "ambiguous registered configs must remain ambiguous"
    );
}

#[test]
fn outside_workspace_reference_roots_do_not_poison_independent_context_metadata() {
    let workspace = unique_root();
    let mut reader = RootReader::new(&workspace);
    for record in reader.base.0[1]["facts"][0]["records"]
        .as_array_mut()
        .unwrap()
    {
        if record["kind"] == "root"
            && record["scope"] == "scope-client"
            && record["role"] == "source"
        {
            record["path"] = json!(workspace.parent().unwrap().join("outside-java"));
        }
    }
    let selected = project(reader).unwrap();
    assert!(
        selected.project_complete
            && selected.config_selection_complete
            && selected.mapping_complete
    );
    assert!(!selected.references_complete);
    assert!(matches!(selected.sources, HostFact::Unavailable { .. }));
}

#[test]
fn a_different_attested_workspace_root_is_not_accepted_as_the_requested_root() {
    let workspace = unique_root();
    let mut reader = RootReader::new(&workspace);
    reader.files.clear();
    reader.root_override = Some(workspace.parent().unwrap().join("foreign-root"));
    assert!(
        matches!(project(reader),Err(FactsError::Protocol(message)) if message.contains("command workspace"))
    );
}

#[test]
fn full_context_commands_use_attested_root_data_without_inventing_type_or_code_authority() {
    use crate::commands::{
        BuildIdentity, CommandDispatcher, CoreCommandHost, EvaluationCommandInputs,
    };
    let workspace = unique_root();
    let selected = project(RootReader::new(&workspace)).unwrap();
    let facts = Arc::new(NoSemanticReads);
    let host = CoreCommandHost::with_evaluation(
        EvaluationCommandInputs {
            workspace_root_uri: path_to_uri(&workspace),
            cached_session: selected.cached_session,
            class_index: facts.clone(),
            bytecode_index: facts.clone(),
            reference_types: facts.clone(),
            definitions: Arc::new(
                |_: &mcdev_core::McDefinitionTarget,
                 _: &ProjectContext,
                 _: &str|
                 -> Option<mcdev_core::mixin_navigation::ResolvedDefinition> {
                    panic!("context DTO queried navigation")
                },
            ),
            check_active: Arc::new(|| Ok(())),
            specifier_supported: Arc::new(|| panic!("context DTO queried specifiers")),
            additional_at_values: Arc::new(|| panic!("context DTO queried annotations")),
            refresh_generation: Some("129".into()),
            config_selection_complete: selected.config_selection_complete,
            mapping_discovery_complete: selected.mapping_complete,
        },
        facts,
        selected.provider,
        selected.sources,
        BuildIdentity {
            commit: "fixture".into(),
            build_time: "fixture".into(),
            version: "fixture".into(),
            artifact_location: "fixture".into(),
        },
        true,
    )
    .unwrap();
    let commands = [
        "mcdev.info".into(),
        "mcdev.dumpContext".into(),
        "mcdev.reindex".into(),
        "mcdev.reloadProjectContext".into(),
    ];
    let dispatcher =
        CommandDispatcher::for_evaluation(Arc::new(host), &commands, selected.project_complete)
            .with_discovery_coverage(
                selected.config_selection_complete,
                selected.references_complete,
            );
    let payload = json!({"context":{"protocolVersion":1,"workspaceRoot":path_to_uri(&workspace),
        "documentUri":path_to_uri(&workspace.join("module/src/main/java/Patch.java")),"languageId":"java",
        "position":{"line":0,"character":0},"bufferText":"package fixture; class Patch {}","documentVersion":7,
        "bufferTextFallbackUsed":false,"client":{"name":"root-fixture","version":"1"}}});
    for command in &commands {
        let response = dispatcher.execute(command, &[payload.clone()]).unwrap();
        assert!(response.error.is_none(), "{command}: {:?}", response.error);
        let result = response.result.unwrap();
        if command == "mcdev.dumpContext" {
            assert_eq!(result["root"], workspace.to_string_lossy().as_ref());
            assert_eq!(result["projectId"], "gradle::|:demo");
            assert_eq!(result["classpath"]["entryCount"], 4);
            assert_eq!(result["classpath"]["capturedAt"], 1700000123456u64);
            assert_eq!(result["sourceSets"].as_array().unwrap().len(), 2);
            assert_eq!(result["mixinConfigs"].as_array().unwrap().len(), 3);
            assert_eq!(result["accessWideners"].as_array().unwrap().len(), 2);
            assert_eq!(result["accessTransformers"].as_array().unwrap().len(), 1);
            assert_eq!(result["mappings"]["sourceNamespace"], "named");
            assert_eq!(result["mappings"]["runtimeNamespace"], "intermediary");
        } else if command == "mcdev.info" {
            assert_eq!(result["registeredCommands"].as_array().unwrap().len(), 4);
        } else {
            assert_eq!(result["indexState"], "ready");
            assert_eq!(result["classpathEntries"], 4);
        }
    }
}
