//! Project inputs derived only from the selected host evaluation.
#[cfg(test)]
mod root_tests;
mod roots;
use crate::{
    host_facts::{
        bridge::FactReader,
        draft,
        inputs::{
            ClasspathRole, Inventory, ProjectInputRecord, ProjectInputs, ProjectScope,
            ProjectSourceSet, ResourceAssociation, ResourceEntry, RootRole, ScopedInputs,
            SourceEntry, SourceSnapshot,
        },
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
pub(crate) use roots::DiscoveryNeeds;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

pub(crate) struct SelectedProject {
    pub cached_session: CachedProjectSession,
    pub sources: HostFact<Vec<SourceScanEntry>>,
    pub provider: Arc<dyn HostMixinSourceProvider>,
    /// Complete legacy context DTO discovery; reference coverage is separate.
    pub project_complete: bool,
    pub config_selection_complete: bool,
    pub mapping_complete: bool,
    pub references_complete: bool,
    /// Positive accepted snapshots retain their host-issued descriptor for sourceFacts.
    pub source_snapshots: Vec<(SourceEntry, SourceSnapshot)>,
    pub owner_sources: CapturedSources,
}

pub(crate) struct CapturedSources {
    pub inventory: Inventory<SourceEntry>,
    pub snapshots: Vec<Fact<SourceSnapshot>>,
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

fn path_key(path: &Path) -> PathBuf {
    let path = normalize_path(path);
    if cfg!(windows) {
        PathBuf::from(path.to_string_lossy().to_ascii_lowercase())
    } else {
        path
    }
}

#[derive(Clone)]
struct CapturedText {
    path: PathBuf,
    text: String,
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
    versions: BTreeMap<PathBuf, Option<i64>>,
}

impl SnapshotProvider {
    fn insert_text(
        &mut self,
        path: &Path,
        text: &str,
        version: Option<i64>,
    ) -> Result<(), FactsError> {
        let key = path_key(path);
        if self
            .text_by_path
            .get(&key)
            .is_some_and(|previous| previous != text)
            || self
                .versions
                .get(&key)
                .copied()
                .flatten()
                .zip(version)
                .is_some_and(|(a, b)| a != b)
        {
            return Err(FactsError::Stale(
                "host returned conflicting snapshots for one URI".into(),
            ));
        }
        self.text_by_path.insert(key.clone(), text.into());
        let previous = self.versions.get(&key).copied().flatten();
        self.versions.insert(key, previous.or(version));
        Ok(())
    }
}

struct OwnedModel {
    root: PathBuf,
    project_id: String,
    scopes: Vec<ProjectSourceSet>,
    source_sets: Vec<SourceSetContext>,
    classpath: ClasspathSnapshot,
    index_state: ProjectIndexState,
    complete: bool,
    inventory_complete: bool,
    plugin_classes: Vec<String>,
}

fn bind_compiler_scope(
    semantic_module: &mut Option<String>,
    membership: &ProjectScope,
    issued: &ProjectSourceSet,
    kind: &str,
) -> Result<(), FactsError> {
    if membership.source_set != issued.descriptor.name
        || semantic_module
            .as_ref()
            .is_some_and(|module| module != &membership.module)
    {
        return Err(FactsError::Protocol(format!(
            "{kind} differs from authoritative project scope: expectedToken={:?} actualToken={:?} expectedSourceSet={:?} actualSourceSet={:?} expectedSemanticModule={:?} actualModule={:?}",
            issued.descriptor.id, membership.id, issued.descriptor.name, membership.source_set,
            semantic_module, membership.module,
        )));
    }
    semantic_module.get_or_insert_with(|| membership.module.clone());
    Ok(())
}

fn absolute_path(path: &str) -> Result<PathBuf, FactsError> {
    let path = Path::new(path);
    if !path.is_absolute() {
        return Err(FactsError::Protocol(
            "host project input path is not absolute".into(),
        ));
    }
    Ok(normalize_path(path))
}

fn observed_index(index: &crate::host_facts::inputs::ProjectIndex) -> ProjectIndexState {
    if index.project_refresh_pending {
        ProjectIndexState::Stale
    } else if index.completed_analysis_generation == index.analysis_generation
        && index.project_scan_complete
        && index.library_index_complete
        && index.jdk_index_complete
    {
        ProjectIndexState::Ready
    } else {
        ProjectIndexState::Building
    }
}

impl OwnedModel {
    fn from_inputs(
        inputs: ProjectInputs,
        selected: &draft::SelectedScope,
    ) -> Result<Self, FactsError> {
        let project = inputs
            .records
            .iter()
            .find_map(|record| match record {
                ProjectInputRecord::Project(project) => Some(project),
                _ => None,
            })
            .ok_or_else(|| {
                FactsError::Unknown("owning project input model is unavailable".into())
            })?;
        let owner = required(project.owning_source_set.clone(), "owning source set")?;
        if !inputs.source_sets.iter().any(|scope| {
            scope.descriptor.id == owner && scope.descriptor.name == selected.source_set
        }) {
            return Err(FactsError::Protocol(
                "owning source-set identity disagrees with selected scope".into(),
            ));
        }
        let root = absolute_path(&project.project_directory)?;
        let capture = project.capture.found();
        let mut complete = inputs.coverage.found().is_some()
            && capture.is_some()
            && project.build_identity.found().is_some()
            && project.index.found().is_some();
        let mut by_scope = BTreeMap::new();
        let mut names = BTreeSet::new();
        for scope in &inputs.source_sets {
            if !names.insert(scope.descriptor.name.clone()) {
                return Err(FactsError::Protocol(
                    "duplicate project source-set name".into(),
                ));
            }
            complete &= scope
                .descriptor
                .compile_classpath_coverage
                .found()
                .is_some()
                && scope.descriptor.runtime_coverage.found().is_some();
            by_scope.insert(
                scope.descriptor.id.clone(),
                SourceSetContext {
                    name: scope.descriptor.name.clone(),
                    ..Default::default()
                },
            );
        }
        let mut roots = BTreeMap::new();
        let mut compile_entries = BTreeMap::new();
        let mut runtime_entries = BTreeMap::new();
        let mut class_outputs = BTreeMap::new();
        for record in &inputs.records {
            match record {
                ProjectInputRecord::Root(record) => {
                    let path = absolute_path(&record.path)?;
                    if roots
                        .insert(
                            (record.scope.clone(), record.role as u8, record.order),
                            path,
                        )
                        .is_some()
                    {
                        return Err(FactsError::Protocol(
                            "duplicate ordered project root".into(),
                        ));
                    }
                }
                ProjectInputRecord::ClasspathEntry(record) => {
                    let entries = match record.role {
                        ClasspathRole::Compile => &mut compile_entries,
                        ClasspathRole::Runtime => &mut runtime_entries,
                        ClasspathRole::ClassOutput => &mut class_outputs,
                    };
                    if entries
                        .insert(
                            (record.scope.clone(), record.order),
                            absolute_path(&record.path)?,
                        )
                        .is_some()
                    {
                        return Err(FactsError::Protocol(
                            "duplicate ordered classpath entry".into(),
                        ));
                    }
                }
                _ => {}
            }
        }
        for scope in &inputs.source_sets {
            for (entries, expected) in [
                (
                    &compile_entries,
                    Some(scope.descriptor.counts.compile_classpath),
                ),
                (&runtime_entries, scope.descriptor.counts.runtime_classpath),
                (&class_outputs, scope.descriptor.counts.class_outputs),
            ] {
                let orders = entries
                    .keys()
                    .filter_map(|(id, order)| (id == &scope.descriptor.id).then_some(*order))
                    .collect::<Vec<_>>();
                if expected.is_some_and(|count| orders != (0..count).collect::<Vec<_>>()) {
                    return Err(FactsError::Protocol(
                        "project classpath count or order disagrees with records".into(),
                    ));
                }
            }
        }
        let mut output_paths = BTreeSet::new();
        for ((scope, role, _), path) in &roots {
            let set = by_scope
                .get_mut(scope)
                .ok_or_else(|| FactsError::Protocol("foreign project root scope".into()))?;
            if *role == RootRole::Source as u8 {
                set.source_directories.push(path.clone());
            } else if *role == RootRole::Resource as u8 {
                set.resource_directories.push(path.clone());
            } else if *role == RootRole::Output as u8 {
                set.output_directory.get_or_insert_with(|| path.clone());
                output_paths.insert(path.clone());
            }
        }
        let mut source_sets = by_scope
            .into_values()
            .map(|set| (set.name.clone(), set))
            .collect::<BTreeMap<_, _>>();
        for record in &inputs.records {
            if let ProjectInputRecord::LogicalRoot(record) = record {
                let set = source_sets
                    .entry(record.source_set.clone())
                    .or_insert_with(|| SourceSetContext {
                        name: record.source_set.clone(),
                        ..Default::default()
                    });
                let directories = match record.role {
                    RootRole::Source => &mut set.source_directories,
                    RootRole::Resource => &mut set.resource_directories,
                    _ => {
                        return Err(FactsError::Protocol(
                            "invalid logical source-set root role".into(),
                        ))
                    }
                };
                let path = absolute_path(&record.path)?;
                if !directories.contains(&path) {
                    directories.push(path);
                }
            }
        }
        output_paths.extend(class_outputs.values().cloned());
        let mut dependencies = BTreeSet::new();
        let mut minecraft = BTreeSet::new();
        for path in compile_entries.values().chain(runtime_entries.values()) {
            if output_paths.contains(path) {
                continue;
            }
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase();
            if name.contains("minecraft") || (name.contains("client") && name.contains("mapped")) {
                minecraft.insert(path.clone());
            } else {
                dependencies.insert(path.clone());
            }
        }
        let classpath = ClasspathSnapshot {
            project_outputs: output_paths.into_iter().collect(),
            dependency_jars: dependencies.into_iter().collect(),
            minecraft_jars: minecraft.into_iter().collect(),
            captured_at: capture.map_or(0, |capture| capture.captured_at_unix_ms),
            ..Default::default()
        };
        let index_state = project
            .index
            .found()
            .map(observed_index)
            .unwrap_or_default();
        Ok(Self {
            root,
            project_id: selected.module.clone(),
            scopes: inputs.source_sets,
            source_sets: source_sets.into_values().collect(),
            classpath,
            index_state,
            complete,
            inventory_complete: inputs.coverage.found().is_some(),
            plugin_classes: capture.map_or_else(Vec::new, |capture| capture.plugin_classes.clone()),
        })
    }
}

impl HostMixinSourceProvider for SnapshotProvider {
    fn config(&self, path: &Path) -> HostSourceResult<MixinConfigRef> {
        let path = path_key(path);
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
        let path = path_key(path);
        Ok(match self.text_by_path.get(&path) {
            Some(text) => HostAnswer::known(text.clone(), vec![]),
            None => HostAnswer::unavailable("source is outside the captured host inventory"),
        })
    }
}

#[cfg(test)]
pub(crate) fn selected_project<R: FactReader>(
    inputs: &mut ScopedInputs<R>,
    facts: draft::ProjectContext,
    workspace_root: &str,
    evaluation: &str,
) -> Result<SelectedProject, FactsError> {
    selected_project_inner(
        inputs,
        facts,
        workspace_root,
        evaluation,
        DiscoveryNeeds::ALL,
    )
}

pub(crate) fn selected_project_for_command<R: FactReader>(
    inputs: &mut ScopedInputs<R>,
    facts: draft::ProjectContext,
    workspace_root: &str,
    evaluation: &str,
    command: &mcdev_protocol::DecodedCommand,
) -> Result<SelectedProject, FactsError> {
    selected_project_inner(
        inputs,
        facts,
        workspace_root,
        evaluation,
        DiscoveryNeeds::for_command(command),
    )
}

fn selected_project_inner<R: FactReader>(
    inputs: &mut ScopedInputs<R>,
    facts: draft::ProjectContext,
    workspace_root: &str,
    evaluation: &str,
    needs: DiscoveryNeeds,
) -> Result<SelectedProject, FactsError> {
    let root = uri_to_path(workspace_root).map_err(FactsError::Protocol)?;
    if !root.is_absolute() {
        return Err(FactsError::Protocol(
            "workspace root is not absolute".into(),
        ));
    }
    let mut root = normalize_path(&root);
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
    let owned_model = match inputs.project_inputs() {
        Ok(model) if !model.records.is_empty() => Some(OwnedModel::from_inputs(model, &scope)?),
        Ok(_) | Err(FactsError::Unsupported(_)) => None,
        Err(error) => return Err(error),
    };
    if let Some(model) = &owned_model {
        root = model.root.clone();
    }
    let scopes: Vec<_> = owned_model.as_ref().map_or_else(
        || vec![None],
        |model| model.scopes.iter().map(Some).collect(),
    );
    let mut provider = SnapshotProvider::default();
    let mut source_snapshots = vec![];
    let mut owner_sources = if needs.compiler_sources {
        None
    } else {
        Some(CapturedSources {
            inventory: Inventory {
                entries: vec![],
                coverage: Fact::Unknown {
                    reasons: vec![serde_json::json!({"code":"NotRequested",
                        "detail":"metadata command does not request compiler source capture"})],
                },
            },
            snapshots: vec![],
        })
    };
    let mut source_paths = BTreeSet::new();
    // projectContext.module is a Gradle path; issued memberships use semantic module identities.
    let mut compiler_module = None;
    for project_scope in scopes.iter().filter(|_| needs.compiler_sources) {
        let inventory = match project_scope {
            Some(scope) => inputs.source_inventory_in(scope)?,
            None => inputs.source_inventory()?,
        };
        inventory
            .coverage
            .validate()
            .map_err(FactsError::Protocol)?;
        if !matches!(
            &inventory.coverage,
            Fact::Found {
                value: serde_json::Value::Null
            } | Fact::Unknown { .. }
        ) {
            return Err(FactsError::Protocol(
                "invalid source inventory coverage".into(),
            ));
        }
        if let Some(model) = &owned_model {
            for source in &inventory.entries {
                for member in source
                    .descriptor
                    .owner_scope
                    .as_ref()
                    .and_then(Fact::found)
                    .into_iter()
                    .chain(
                        source
                            .descriptor
                            .memberships
                            .as_ref()
                            .and_then(Fact::found)
                            .into_iter()
                            .flatten(),
                    )
                {
                    if let Some(issued) = model
                        .scopes
                        .iter()
                        .find(|issued| member.id.as_deref() == Some(issued.descriptor.id.as_str()))
                    {
                        bind_compiler_scope(
                            &mut compiler_module,
                            member,
                            issued,
                            "source membership",
                        )?;
                    }
                }
            }
        }
        let snapshots = inputs.source_snapshots(&inventory.entries)?;
        if snapshots.len() != inventory.entries.len() {
            return Err(FactsError::Protocol(
                "source snapshot cardinality mismatch".into(),
            ));
        }
        if project_scope
            .is_none_or(|project_scope| project_scope.descriptor.name == scope.source_set)
        {
            owner_sources = Some(CapturedSources {
                inventory: inventory.clone(),
                snapshots: snapshots.clone(),
            });
        }
        for (entry, snapshot) in inventory.entries.iter().zip(snapshots) {
            match snapshot {
                Fact::Found { value: snapshot } => {
                    if let Some(path) = path_from_uri(&snapshot.descriptor.document_uri)? {
                        provider.insert_text(
                            &path,
                            &snapshot.text,
                            snapshot.descriptor.document_version,
                        )?;
                        let key = path_key(&path);
                        if source_paths.insert(key) {
                            source_snapshots.push((entry.clone(), snapshot.clone()));
                        }
                    }
                }
                Fact::Unknown { .. } | Fact::DefinitelyAbsent => {}
            }
        }
    }
    let mut resources_complete = true;
    let mut entries = Vec::new();
    for project_scope in &scopes {
        let resources = match project_scope {
            Some(scope) => inputs.resource_inventory_in(scope)?,
            None => inputs.resource_inventory()?,
        };
        if owned_model.is_none() {
            coverage(
                resources.coverage,
                serde_json::json!(true),
                "selected resource inventory coverage",
            )?;
        } else {
            resources
                .coverage
                .validate()
                .map_err(FactsError::Protocol)?;
            match resources.coverage {
                Fact::Found { value } if value == serde_json::json!(true) => {}
                Fact::Unknown { .. } => resources_complete = false,
                _ => {
                    return Err(FactsError::Protocol(
                        "invalid resource inventory coverage".into(),
                    ))
                }
            }
        }
        for entry in resources.entries {
            if let (Some(issued), Some(membership)) =
                (project_scope, entry.descriptor.membership_scope.as_ref())
            {
                bind_compiler_scope(
                    &mut compiler_module,
                    membership,
                    issued,
                    "resource membership",
                )?;
            }
            if required_resource(&entry, &root)? {
                entries.push(entry);
            }
        }
    }
    let contents = inputs.resource_snapshots(&entries)?;
    if contents.len() != entries.len() {
        return Err(FactsError::Protocol(
            "resource snapshot cardinality mismatch".into(),
        ));
    }
    let mut selected_texts = BTreeMap::new();
    let mut resource_roots = BTreeSet::new();
    for (entry, content) in entries.into_iter().zip(contents) {
        let Some(path) = path_from_uri(&entry.descriptor.document_uri)? else {
            continue;
        };
        let relative = relative_name(&entry);
        if entry.descriptor.association == ResourceAssociation::Source {
            if let Some(dir) = source_root(&path, &relative) {
                resource_roots.insert(dir);
            }
        }
        let Fact::Found { value: content } = content else {
            continue;
        };
        if !content.utf8 {
            continue;
        }
        let text = String::from_utf8(content.bytes)
            .map_err(|_| FactsError::Protocol("host marked non-UTF-8 resource as UTF-8".into()))?;
        provider.insert_text(&path, &text, content.document_version)?;
        selected_texts.insert(path_key(&path), CapturedText { path, text });
    }
    let output_paths: Vec<PathBuf> = output_roots.into_iter().map(PathBuf::from).collect();
    let output_set: BTreeSet<_> = output_paths.iter().cloned().collect();
    let selected_classpath = ClasspathSnapshot {
        project_outputs: output_paths.clone(),
        dependency_jars: selected_classpath
            .entries
            .into_iter()
            .map(|e| PathBuf::from(e.display_path))
            .filter(|path| !output_set.contains(path))
            .collect(),
        ..Default::default()
    };
    let selected_source_set = SourceSetContext {
        name: scope.source_set,
        source_directories: source_roots.into_iter().map(PathBuf::from).collect(),
        resource_directories: resource_roots.into_iter().collect(),
        output_directory: output_paths.first().cloned(),
    };
    let source_sets = owned_model.as_ref().map_or_else(
        || vec![selected_source_set],
        |model| model.source_sets.clone(),
    );
    let root_inputs = roots::discover(inputs, workspace_root, &source_sets, needs)?;
    if let Some(workspace) = &root_inputs.workspace {
        root = workspace.clone();
    }
    for domain in [
        &root_inputs.resources,
        &root_inputs.mappings,
        &root_inputs.references,
    ] {
        for body in domain.bodies.values() {
            let path = uri_to_path(&body.uri).map_err(FactsError::Protocol)?;
            provider.insert_text(&path, &body.text, body.document_version.map(i64::from))?;
        }
    }
    // Positive selected snapshots cannot coexist with an attested absence in the same root domain.
    for body in selected_texts.values() {
        if let Some(relative) = workspace_relative(&root, &body.path) {
            for (domain, required) in [
                (&root_inputs.resources, roots::is_resource(&relative)),
                (&root_inputs.mappings, roots::is_mapping(&relative)),
                (
                    &root_inputs.references,
                    roots::is_reference(&body.path, &source_sets),
                ),
            ] {
                if domain.inventory_complete
                    && required
                    && !domain.listed.contains(&path_key(&body.path))
                {
                    return Err(FactsError::Stale(
                        "root inventory contradicts a current selected resource".into(),
                    ));
                }
            }
        }
    }
    for (_, snapshot) in &source_snapshots {
        if let Some(path) = path_from_uri(&snapshot.descriptor.document_uri)? {
            if root_inputs.references.inventory_complete
                && roots::is_reference(&path, &source_sets)
                && workspace_relative(&root, &path).is_some()
                && !root_inputs.references.listed.contains(&path_key(&path))
            {
                return Err(FactsError::Stale(
                    "root inventory contradicts a current compiler snapshot".into(),
                ));
            }
        }
    }
    let mut resource_texts = BTreeMap::new();
    let mut mapping_texts = BTreeMap::new();
    for (key, body) in &selected_texts {
        if let Some(relative) = workspace_relative(&root, &body.path) {
            if !root_inputs.resources.complete && roots::is_resource(&relative) {
                resource_texts.insert(key.clone(), body.clone());
            }
            if !root_inputs.mappings.complete && roots::is_mapping(&relative) {
                mapping_texts.insert(key.clone(), body.clone());
            }
        }
    }
    for (domain, texts) in [
        (&root_inputs.resources, &mut resource_texts),
        (&root_inputs.mappings, &mut mapping_texts),
    ] {
        for (key, body) in &domain.bodies {
            texts.insert(
                key.clone(),
                CapturedText {
                    path: uri_to_path(&body.uri).map_err(FactsError::Protocol)?,
                    text: body.text.clone(),
                },
            );
        }
    }
    let mut gradle = vec![];
    let mut configs = vec![];
    let mut wideners = vec![];
    let mut transformers = vec![];
    let mut issues = vec![];
    for body in resource_texts.values() {
        let Some(relative) = workspace_relative(&root, &body.path) else {
            continue;
        };
        if roots::is_gradle(&relative) {
            gradle.push(body.text.clone());
        }
        if MixinConfigDiscoveryService::is_mixin_config_file(&body.path) {
            match MixinConfigDiscoveryService::parse_content(&body.path, &body.text) {
                Some(config) => {
                    provider
                        .configs
                        .insert(path_key(&body.path), config.clone());
                    configs.push(config);
                }
                None => issues.push(ResourceIssue {
                    path: body.path.clone(),
                    message: "Invalid or unreadable Mixin config".into(),
                }),
            }
        }
        if AwAtDiscoveryService::is_access_widener_file(&body.path) {
            let parsed = mcdev_core::aw::AccessWidenerParser::parse(&body.text).ok();
            wideners.push(AccessWidenerRef {
                path: body.path.clone(),
                namespace: parsed.as_ref().map(|p| p.namespace),
                parsed,
            });
        }
        if AwAtDiscoveryService::is_access_transformer_file(&body.path) {
            transformers.push(AccessTransformerRef {
                path: body.path.clone(),
                parsed: mcdev_core::at::AccessTransformerParser::parse(&body.text).ok(),
            });
        }
    }
    let sort_path = |path: &Path| path_key(path).to_string_lossy().into_owned();
    let mut mapping_inputs = mapping_texts
        .into_values()
        .map(|body| (body.path, body.text))
        .collect::<Vec<_>>();
    mapping_inputs.sort_by_key(|(path, _)| sort_path(path));
    configs.sort_by_key(|config| sort_path(&config.path));
    wideners.sort_by_key(|aw| sort_path(&aw.path));
    transformers.sort_by_key(|at| sort_path(&at.path));
    if !root_inputs.platform_complete() {
        if let Some(model) = &owned_model {
            for plugin in &model.plugin_classes {
                let plugin = plugin.to_ascii_lowercase();
                if plugin.contains("fabricmc.loom") {
                    gradle.push("fabric-loom".into());
                } else if plugin.contains("neoforged") {
                    gradle.push("org.neoforged.moddev".into());
                } else if plugin.contains("forgegradle") || plugin.contains("minecraftforge.gradle")
                {
                    gradle.push("net.minecraftforge.gradle".into());
                }
            }
        }
    }
    let platform = PlatformDetector::detect(&gradle);
    let mappings =
        MappingDiscoveryService::build_mapping_context_from_contents(&mapping_inputs, platform)
            .map_err(|error| FactsError::Unknown(format!("mapping snapshots: {error}")))?;
    let mapping_complete = root_inputs.mappings.complete && root_inputs.platform_complete();
    let config_selection_complete = owned_model
        .as_ref()
        .is_some_and(|model| model.inventory_complete)
        && resources_complete
        && root_inputs.resources.complete;
    let project_complete = owned_model.as_ref().is_some_and(|model| model.complete)
        && resources_complete
        && root_inputs.resources.complete
        && mapping_complete;
    let references_complete = owned_model
        .as_ref()
        .is_some_and(|model| model.inventory_complete)
        && resources_complete
        && root_inputs.references.complete;
    let (project_id, classpath, index_state) = match owned_model {
        Some(model) => (model.project_id, model.classpath, model.index_state),
        None => (
            scope.module,
            selected_classpath,
            ProjectIndexState::NotReady,
        ),
    };
    let minecraft_jars = classpath.minecraft_jars.clone();
    let context = Arc::new(ProjectContext {
        project_id,
        root,
        platform,
        classpath,
        mappings,
        mixin_configs: configs,
        access_wideners: wideners,
        access_transformers: transformers,
        minecraft_jars,
        source_sets,
        index_state,
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
        sources: if references_complete {
            HostFact::Known(
                root_inputs
                    .references
                    .bodies
                    .into_values()
                    .map(|body| SourceScanEntry {
                        document_uri: body.uri,
                        text: body.text,
                    })
                    .collect(),
            )
        } else {
            HostFact::Unavailable {
                reason: "complete legacy source/resource reference discovery is unavailable".into(),
            }
        },
        provider: Arc::new(provider),
        project_complete,
        config_selection_complete,
        mapping_complete,
        references_complete,
        source_snapshots,
        owner_sources: owner_sources.ok_or_else(|| {
            FactsError::Protocol("evaluation owner source inventory is unavailable".into())
        })?,
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

    pub(super) struct Reader(pub(super) VecDeque<Value>);
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
    pub(super) fn found(value: Value) -> Value {
        json!({"state":"found","value":value})
    }
    pub(super) fn sha(bytes: &[u8]) -> String {
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
    pub(super) fn unique_root() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("mcdev-ipc-snapshot-{}-{nonce}", std::process::id()))
    }

    fn project_operations() -> InputOps {
        InputOps::intersect(&json!({
            "capabilities":["project-inputs-v1","source-snapshots-v1","owned-resources-v1"],
            "supportedOps":{"project-inputs-v1":["projectInputs"],
                "source-snapshots-v1":["sourceInventory","sourceSnapshot"],
                "owned-resources-v1":["resourceInventory","resourceLookup","resourceSnapshot"]}
        }))
        .unwrap()
    }

    fn project_records(root: &Path) -> Vec<Value> {
        let mut records = vec![json!({"kind":"project","id":"project-demo",
            "owningSourceSet":found(json!("scope-main")),"projectPath":":demo","projectDirectory":root,
            "buildIdentity":found(json!("captured-build")),"capture":found(json!({
                "identity":"original-capture","capturedAtUnixMs":1700000123456u64,"gradleVersion":"9.6.1",
                "domain":crate::host_facts::inputs::PROJECT_INPUTS_DOMAIN,"javaSourceSetsComplete":true,
                "kotlinPresent":false,"kotlinSourceSetsCaptured":false,"kotlinTargets":null,
                "pluginClasses":["net.fabricmc.loom.LoomGradlePlugin"]})),
            "index":found(json!({"analysisGeneration":9,"completedAnalysisGeneration":9,"indexRevision":81,
                "projectRefreshPending":false,"projectScanComplete":true,"libraryIndexComplete":true,"jdkIndexComplete":true,
                "counts":{"domain":"workspace-accepted-unique-inputs","projectSourceFiles":2,
                    "completedDependencyOrigins":1,"liveDependencyOrigins":1}}))})];
        for name in ["main", "client"] {
            let scope = format!("scope-{name}");
            records.push(json!({"kind":"sourceSet","id":scope,"project":"project-demo","name":name,
                "configurations":found(json!({"compile_classpath":format!("{name}CompileClasspath"),
                    "runtime_classpath":format!("{name}RuntimeClasspath"),"annotation_processor":"annotationProcessor",
                    "api_elements":"apiElements","runtime_elements":"runtimeElements"})),
                "compileClasspathCoverage":found(Value::Null),"runtimeCoverage":found(Value::Null),
                "compilerMetadataComplete":true,"unresolved":[],
                "counts":{"domain":"this-source-set-ordered-entries","compileClasspath":2,"runtimeClasspath":2,"classOutputs":1}}));
            for (role, path) in [
                ("source", root.join(format!("src/{name}/java"))),
                ("resource", root.join(format!("src/{name}/resources"))),
                ("output", root.join(format!("build/classes/java/{name}"))),
                ("configuration", root.join("build.gradle")),
            ] {
                records
                    .push(json!({"kind":"root","scope":scope,"role":role,"order":0,"path":path}));
            }
            records.push(json!({"kind":"root","scope":scope,"role":"configuration","order":1,"path":root.join("mixins.shared.json")}));
            for role in ["compile", "runtime"] {
                for (order, file) in ["minecraft-mapped.jar", "library.jar"]
                    .into_iter()
                    .enumerate()
                {
                    records.push(json!({"kind":"classpathEntry","scope":scope,"role":role,"order":order,
                        "path":root.join("libs").join(file),"identity":found(json!(format!("origin-{file}"))),
                        "component":found(json!("File"))}));
                }
            }
            records.push(json!({"kind":"classpathEntry","scope":scope,"role":"classOutput","order":0,
                "path":root.join(format!("build/classes/java/{name}")),"identity":found(json!(format!("output-{name}"))),
                "component":found(json!("File"))}));
        }
        records
    }

    pub(super) fn model_replies(root: &Path, local_configs: &[(&str, &str)]) -> VecDeque<Value> {
        let records = project_records(root);
        let mut replies = vec![
            json!({"facts":[{"records":records[..2],"next":"project-page-2", "coverage":{"state":"unknown","reasons":[{"code":"IncompleteModel"}]}}]}),
            json!({"facts":[{"records":records[2..],"next":null,"coverage":found(json!({"domain":crate::host_facts::inputs::PROJECT_INPUTS_DOMAIN}))}]}),
        ];
        for name in ["main", "client"] {
            let path = root.join(format!(
                "src/{name}/java/{}.java",
                if name == "main" { "Patch" } else { "Other" }
            ));
            let text = if name == "main" {
                "package fixture; class Patch {}"
            } else {
                "package fixture; class Other {}"
            };
            let scope =
                json!({"module":"gradle::|:demo","sourceSet":name,"id":format!("scope-{name}")});
            let descriptor = json!({"id":format!("source-{name}"),"displayPath":path,
                "documentUri":found(json!(path_to_uri(&path))),"languageId":"java",
                "identity":{"kind":"source","revision":"23"},"ownerScope":found(scope.clone()),
                "memberships":found(json!([scope])),"editable":found(json!(true))});
            let mut snapshot = descriptor.clone();
            snapshot["text"] = json!(text);
            snapshot["sha256"] = json!(sha(text.as_bytes()));
            replies.push(json!({"facts":[{"sources":[descriptor],"next":null,"coverage":found(Value::Null)}]}));
            replies.push(json!({"facts":[found(snapshot)]}));
        }
        let client_path = root.join("src/client/resources/client.mixins.json");
        let shared_path = root.join("mixins.shared.json");
        let aw_path = root.join("src/client/resources/demo.aw");
        let empty = r#"{"package":"fixture","mixins":[]}"#;
        let shared = r#"{"package":"fixture","mixins":["Patch"]}"#;
        let mut bodies = vec![chunk(empty), chunk(shared)];
        let resource = |id: &str,
                        relative: &str,
                        association: &str,
                        path: &Path,
                        member: &str,
                        owner: Option<&str>| {
            let mut descriptor = descriptor(id, relative, association, path);
            descriptor["membershipScope"] = json!({"module":"gradle::|:demo","sourceSet":member});
            descriptor["ownerScope"] = owner.map_or_else(
                || json!({"state":"unknown","reasons":[{"code":"MissingFacts"}]}),
                |owner| found(json!({"module":"gradle::|:demo","sourceSet":owner})),
            );
            descriptor
        };
        let mut main_resources = vec![
            resource(
                "main-client-config",
                "client.mixins.json",
                "compileClasspath",
                &client_path,
                "main",
                Some("client"),
            ),
            resource(
                "main-shared-config",
                "mixins.shared.json",
                "configuration",
                &shared_path,
                "main",
                None,
            ),
        ];
        for (index, (name, text)) in local_configs.iter().enumerate() {
            main_resources.push(resource(
                &format!("main-local-{index}"),
                name,
                "source",
                &root.join("src/main/resources").join(name),
                "main",
                Some("main"),
            ));
            bodies.push(chunk(text));
        }
        replies.push(json!({"facts":[{"resources":main_resources,"next":null,"coverage":found(json!(true))}]}));
        replies.push(json!({"facts":[{"resources":[
            resource("client-config", "client.mixins.json", "source", &client_path, "client", Some("client")),
            resource("client-aw", "demo.aw", "source", &aw_path, "client", Some("client"))],
            "next":null,"coverage":found(json!(true))}]}));
        bodies.extend([chunk(empty), chunk("accessWidener v2 named\n")]);
        replies.push(json!({"facts":bodies}));
        replies.into()
    }

    pub(super) fn owning_metadata(root: &Path) -> draft::ProjectContext {
        let mut metadata = metadata(root);
        metadata.scope =
            serde_json::from_value(found(json!({"module":"gradle::|:demo","sourceSet":"main"})))
                .unwrap();
        metadata
    }

    fn selected_model(root: &Path, local_configs: &[(&str, &str)]) -> SelectedProject {
        let mut inputs = ScopedInputs::new(
            "eval-1",
            &project_operations(),
            Reader(model_replies(root, local_configs)),
        )
        .unwrap();
        selected_project(
            &mut inputs,
            owning_metadata(root),
            &root.parent().unwrap().to_string_lossy(),
            "eval-1",
        )
        .unwrap()
    }

    #[test]
    fn owning_model_keeps_all_source_sets_and_client_configs_out_of_shared_selection() {
        let empty = r#"{"package":"fixture","mixins":[]}"#;
        let registered = r#"{"package":"fixture","mixins":["Patch"]}"#;
        for (locals, expected) in [
            (vec![], "mixins.shared.json"),
            (
                vec![("empty.mixins.json", empty)],
                "src/main/resources/empty.mixins.json",
            ),
            (
                vec![
                    ("empty.mixins.json", empty),
                    ("registered.mixins.json", registered),
                ],
                "src/main/resources/registered.mixins.json",
            ),
        ] {
            let root = unique_root().join("module");
            let selected = selected_model(&root, &locals);
            let context = &selected.cached_session.context;
            assert_eq!(context.root, root);
            assert_eq!(context.project_id, "gradle::|:demo");
            assert_eq!(
                context
                    .source_sets
                    .iter()
                    .map(|set| set.name.as_str())
                    .collect::<Vec<_>>(),
                ["client", "main"]
            );
            assert_eq!(context.mixin_configs.len(), 2 + locals.len());
            let config = mcdev_core::mixin_facade::HostMixinFacade::selected_mixin_config(
                context,
                "package fixture; class Patch {}",
                Some(&path_to_uri(&root.join("src/main/java/Patch.java"))),
                selected.provider.as_ref(),
            )
            .unwrap()
            .unwrap();
            assert_eq!(config.path, root.join(expected));
            assert_eq!(context.platform, ModPlatform::Fabric);
            assert_eq!(context.index_state, ProjectIndexState::Ready);
            assert_eq!(context.classpath.captured_at, 1700000123456);
            assert_eq!(context.classpath.entry_count(), 4);
            assert_eq!(
                context.classpath.minecraft_jars,
                [root.join("libs/minecraft-mapped.jar")]
            );
            assert_eq!(selected.source_snapshots.len(), 2);
            assert_eq!(selected.owner_sources.inventory.entries.len(), 1);
            assert_eq!(
                selected.owner_sources.inventory.entries[0].descriptor.id,
                "source-main"
            );
            assert_eq!(selected.owner_sources.snapshots.len(), 1);
            assert!(matches!(selected.sources, HostFact::Unavailable { .. }));
            assert!(!selected.project_complete);
            assert!(!selected.config_selection_complete);
            assert!(!root.exists());
        }
    }

    #[test]
    fn project_index_counts_do_not_supply_completion_and_capture_does_not_become_current_time() {
        let root = unique_root();
        let mut records = project_records(&root);
        records[0]["index"]["value"]["projectScanComplete"] = json!(false);
        records[0]["index"]["value"]["counts"]["projectSourceFiles"] = json!(0);
        let reader = Reader(VecDeque::from([
            json!({"facts":[{"records":records,"next":null,
            "coverage":found(json!({"domain":crate::host_facts::inputs::PROJECT_INPUTS_DOMAIN}))}]} ),
        ]));
        let mut inputs = ScopedInputs::new("eval-1", &project_operations(), reader).unwrap();
        let model = OwnedModel::from_inputs(
            inputs.project_inputs().unwrap(),
            &draft::SelectedScope {
                module: ":demo".into(),
                source_set: "main".into(),
            },
        )
        .unwrap();
        assert_eq!(model.index_state, ProjectIndexState::Building);
        assert_eq!(model.classpath.captured_at, 1700000123456);
    }

    #[test]
    fn project_model_coverage_and_source_coverage_remain_independent() {
        for missing_model in [false, true] {
            let root = unique_root();
            let mut replies = model_replies(&root, &[]);
            let unknown = json!({"state":"unknown","reasons":[{"code":"IncompleteModel"}]});
            if missing_model {
                replies[1]["facts"][0]["coverage"] = unknown;
            } else {
                replies[2]["facts"][0]["coverage"] = unknown;
            }
            let mut inputs =
                ScopedInputs::new("eval-1", &project_operations(), Reader(replies)).unwrap();
            let selected = selected_project(
                &mut inputs,
                owning_metadata(&root),
                &root.to_string_lossy(),
                "eval-1",
            )
            .unwrap();
            assert!(matches!(selected.sources, HostFact::Unavailable { .. }));
            assert_eq!(selected.source_snapshots.len(), 2);
            assert!(!selected.project_complete);
            assert_eq!(
                selected.owner_sources.inventory.coverage.found().is_some(),
                missing_model
            );
            assert_eq!(selected.cached_session.context.source_sets.len(), 2);
            assert_eq!(
                selected.cached_session.context.classpath.captured_at,
                1700000123456
            );
            assert!(!selected.config_selection_complete);
        }
    }

    #[test]
    fn scoped_membership_must_match_the_bound_semantic_project_scope() {
        for source_membership in [false, true] {
            let root = unique_root();
            let mut replies = model_replies(&root, &[]);
            if source_membership {
                replies[2]["facts"][0]["sources"][0]["memberships"]["value"][0]["module"] =
                    json!(":demo");
            } else {
                replies[6]["facts"][0]["resources"][0]["membershipScope"]["module"] =
                    json!(":demo");
            }
            let mut inputs =
                ScopedInputs::new("eval-1", &project_operations(), Reader(replies)).unwrap();
            assert!(
                matches!(selected_project(&mut inputs, owning_metadata(&root), &root.to_string_lossy(), "eval-1"),
                Err(FactsError::Protocol(message)) if message.contains("authoritative project scope"))
            );
        }
    }

    fn semantic_module_replies(root: &Path, shared_owner: Value) -> VecDeque<Value> {
        fn set_modules(value: &mut Value) {
            match value {
                Value::Object(values) => {
                    for (key, value) in values {
                        if key == "module" {
                            *value = json!("gradle::included|:demo");
                        } else {
                            set_modules(value);
                        }
                    }
                }
                Value::Array(values) => values.iter_mut().for_each(set_modules),
                _ => {}
            }
        }
        let mut replies = model_replies(root, &[]);
        for reply in &mut replies {
            set_modules(reply);
        }
        replies[0]["facts"][0]["records"][0]["buildIdentity"] = found(json!(":included"));
        let path = root.join("src/shared/java/Shared.java");
        let text = "package fixture; class Shared {}";
        let shared = json!({"id":"shared-source","displayPath":path,
            "documentUri":found(json!(path_to_uri(&path))),"languageId":"java",
            "identity":{"kind":"source","revision":"23"},"ownerScope":shared_owner,
            "memberships":found(json!([
                {"module":"gradle::included|:demo","sourceSet":"main","id":"scope-main"},
                {"module":"gradle::included|:demo","sourceSet":"client","id":"scope-client"}])),
            "editable":{"state":"unknown","reasons":[{"code":"MissingFacts"}]}});
        let mut snapshot = shared.clone();
        snapshot["text"] = json!(text);
        snapshot["sha256"] = json!(sha(text.as_bytes()));
        for (inventory, snapshots) in [(2, 3), (4, 5)] {
            replies[inventory]["facts"][0]["sources"]
                .as_array_mut()
                .unwrap()
                .push(shared.clone());
            replies[snapshots]["facts"]
                .as_array_mut()
                .unwrap()
                .push(found(snapshot.clone()));
        }
        replies
    }

    #[test]
    fn gradle_project_path_and_semantic_module_keep_shared_memberships_and_resource_ownership_separate(
    ) {
        for shared_owner in [
            json!({"state":"unknown","reasons":[{"code":"MissingFacts"}]}),
            found(json!({"module":"gradle::other-build|:library","sourceSet":"main","id":null})),
        ] {
            let root = unique_root();
            let mut inputs = ScopedInputs::new(
                "eval-1",
                &project_operations(),
                Reader(semantic_module_replies(&root, shared_owner.clone())),
            )
            .unwrap();
            // The published encoder emits Gradle's raw project path in projectContext.scope.module.
            let selected = selected_project(
                &mut inputs,
                metadata(&root),
                &root.to_string_lossy(),
                "eval-1",
            )
            .unwrap();
            assert_eq!(selected.cached_session.context.project_id, ":demo");
            assert_eq!(selected.cached_session.context.source_sets.len(), 2);
            assert_eq!(selected.source_snapshots.len(), 3);
            assert_eq!(selected.owner_sources.inventory.entries.len(), 2);
            let shared = selected
                .source_snapshots
                .iter()
                .find(|(entry, _)| entry.descriptor.id == "shared-source")
                .unwrap();
            assert_eq!(
                shared.0.descriptor.owner_scope.as_ref().unwrap(),
                &serde_json::from_value::<Fact<ProjectScope>>(shared_owner).unwrap()
            );
            assert_eq!(
                shared
                    .0
                    .descriptor
                    .memberships
                    .as_ref()
                    .and_then(Fact::found)
                    .unwrap()
                    .len(),
                2
            );
            assert!(matches!(
                shared.0.descriptor.editable,
                Some(Fact::Unknown { .. })
            ));
            let root_client = root.join("src/client/resources/client.mixins.json");
            assert!(selected
                .cached_session
                .context
                .mixin_configs
                .iter()
                .any(|config| config.path == root_client));
            let config = mcdev_core::mixin_facade::HostMixinFacade::selected_mixin_config(
                &selected.cached_session.context,
                "package fixture; class Patch {}",
                Some(&path_to_uri(&root.join("src/main/java/Patch.java"))),
                selected.provider.as_ref(),
            )
            .unwrap()
            .unwrap();
            assert_eq!(config.path, root.join("mixins.shared.json"));
        }
    }

    #[test]
    fn semantic_module_bindings_reject_conflicting_sibling_modules_and_scope_names() {
        for invalid in ["source-module", "source-name", "resource-module"] {
            let root = unique_root();
            let mut replies = semantic_module_replies(
                &root,
                json!({"state":"unknown","reasons":[{"code":"MissingFacts"}]}),
            );
            match invalid {
                "source-module" => {
                    replies[4]["facts"][0]["sources"][0]["ownerScope"]["value"]["module"] =
                        json!("gradle::foreign|:demo");
                }
                "source-name" => {
                    replies[2]["facts"][0]["sources"][0]["memberships"]["value"].as_array_mut().unwrap()
                        .push(json!({"module":"gradle::included|:demo","sourceSet":"main","id":"scope-client"}));
                }
                "resource-module" => {
                    replies[7]["facts"][0]["resources"][0]["membershipScope"]["module"] =
                        json!("gradle::foreign|:demo");
                }
                _ => unreachable!(),
            }
            let mut inputs =
                ScopedInputs::new("eval-1", &project_operations(), Reader(replies)).unwrap();
            let error = match selected_project(
                &mut inputs,
                metadata(&root),
                &root.to_string_lossy(),
                "eval-1",
            ) {
                Err(FactsError::Protocol(message)) => message,
                _ => panic!("{invalid} must fail as a protocol inconsistency"),
            };
            assert!(error.contains("authoritative project scope"), "{error}");
            assert!(error.contains("expectedToken=\"scope-client\""), "{error}");
            assert!(
                error.contains("expectedSemanticModule=Some(\"gradle::included|:demo\")"),
                "{error}"
            );
            assert!(
                error.contains(if invalid == "source-name" {
                    "actualSourceSet=\"main\""
                } else {
                    "actualModule=\"gradle::foreign|:demo\""
                }),
                "{error}"
            );
            assert!(!error.contains("class Patch"));
        }
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
