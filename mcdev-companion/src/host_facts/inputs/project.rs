//! The owning Gradle project's captured inputs; scope tokens grant inventory reads only.
use super::*;

pub const PROJECT_INPUTS_DOMAIN: &str =
    "java-source-sets-and-kotlin-logical-source-set-declarations";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectScope {
    pub module: String,
    pub source_set: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

impl ProjectScope {
    pub(super) fn validate(&self) -> Result<(), FactsError> {
        if !valid_token(&self.module)
            || !valid_token(&self.source_set)
            || self.id.as_deref().is_some_and(|id| !valid_token(id))
        {
            return Err(protocol("invalid project scope descriptor"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInputCoverage {
    pub domain: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRecord {
    pub id: String,
    pub owning_source_set: Fact<String>,
    pub project_path: String,
    pub project_directory: String,
    pub build_identity: Fact<String>,
    pub capture: Fact<ProjectCapture>,
    pub index: Fact<ProjectIndex>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCapture {
    pub identity: String,
    pub captured_at_unix_ms: u64,
    pub gradle_version: String,
    pub domain: String,
    pub java_source_sets_complete: bool,
    pub kotlin_present: bool,
    pub kotlin_source_sets_captured: bool,
    pub kotlin_targets: Option<Vec<KotlinTarget>>,
    pub plugin_classes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KotlinTarget {
    pub name: String,
    pub platform_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectIndex {
    pub analysis_generation: u64,
    pub completed_analysis_generation: u64,
    pub index_revision: u64,
    pub project_refresh_pending: bool,
    pub project_scan_complete: bool,
    pub library_index_complete: bool,
    pub jdk_index_complete: bool,
    pub counts: ProjectIndexCounts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectIndexCounts {
    pub domain: String,
    pub project_source_files: usize,
    pub completed_dependency_origins: usize,
    pub live_dependency_origins: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSetRecord {
    pub id: String,
    pub project: String,
    pub name: String,
    pub configurations: Fact<SourceSetConfigurations>,
    pub compile_classpath_coverage: Fact<()>,
    pub runtime_coverage: Fact<()>,
    pub compiler_metadata_complete: bool,
    pub unresolved: Vec<String>,
    pub counts: SourceSetCounts,
}

// These two nested records retain the Gradle serializer's snake_case fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSetConfigurations {
    pub compile_classpath: String,
    pub runtime_classpath: String,
    pub annotation_processor: String,
    pub api_elements: String,
    pub runtime_elements: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSetCounts {
    pub domain: String,
    pub compile_classpath: usize,
    pub runtime_classpath: Option<usize>,
    pub class_outputs: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RootRole {
    Source,
    Output,
    ProvidedArtifact,
    Resource,
    ResourceOutput,
    Configuration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootRecord {
    pub scope: String,
    pub role: RootRole,
    pub order: usize,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClasspathRole {
    Compile,
    Runtime,
    ClassOutput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComponentIdentity {
    Module {
        group: String,
        name: String,
        version: String,
    },
    Project {
        build: String,
        project: String,
    },
    File,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClasspathInputRecord {
    pub scope: String,
    pub role: ClasspathRole,
    pub order: usize,
    pub path: String,
    pub component: Fact<ComponentIdentity>,
    pub identity: Fact<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompilerTaskRecord {
    pub scope: String,
    pub task: String,
    pub task_type: String,
    pub language: String,
    pub complete: bool,
    pub problem: Option<String>,
    pub primary_kotlin: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceMembershipRecord {
    pub scope: String,
    pub task: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KotlinLogicalSourceSetRecord {
    pub project: String,
    pub name: String,
    pub depends_on: Vec<String>,
    pub compiler_scope: Fact<()>,
    pub runtime: Fact<()>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalRootRecord {
    pub project: String,
    pub source_set: String,
    pub role: RootRole,
    pub order: usize,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalMembershipRecord {
    pub project: String,
    pub source_set: String,
    pub role: RootRole,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProjectInputRecord {
    Project(ProjectRecord),
    SourceSet(SourceSetRecord),
    Root(RootRecord),
    ClasspathEntry(ClasspathInputRecord),
    CompilerTask(CompilerTaskRecord),
    SourceMembership(SourceMembershipRecord),
    KotlinLogicalSourceSet(KotlinLogicalSourceSetRecord),
    LogicalRoot(LogicalRootRecord),
    LogicalMembership(LogicalMembershipRecord),
}

#[derive(Debug, Clone)]
/// Inventory authority comes from the issued token, not Gradle's project path.
pub struct ProjectSourceSet {
    pub descriptor: SourceSetRecord,
    pub(super) scope: u64,
}

#[derive(Debug, Clone)]
pub struct ProjectInputs {
    pub records: Vec<ProjectInputRecord>,
    pub source_sets: Vec<ProjectSourceSet>,
    pub coverage: Fact<ProjectInputCoverage>,
}

#[derive(Deserialize)]
struct ProjectPage {
    records: Vec<ProjectInputRecord>,
    next: Option<String>,
    coverage: Fact<ProjectInputCoverage>,
}

impl<R: FactReader> ScopedInputs<R> {
    pub fn project_inputs(&mut self) -> Result<ProjectInputs, FactsError> {
        let mut records = Vec::new();
        let mut after = None;
        let mut seen = BTreeSet::new();
        let mut bytes = 0usize;
        let coverage = loop {
            let page: ProjectPage = self.one(
                InputOp::ProjectInputs,
                json!({"op":"projectInputs","after":after}),
            )?;
            validate_fact(&page.coverage)?;
            if page.next.is_some()
                && (!matches!(page.coverage, Fact::Unknown { .. }) || page.records.is_empty())
                || matches!(&page.coverage, Fact::Found { value } if value.domain != PROJECT_INPUTS_DOMAIN)
                || matches!(page.coverage, Fact::DefinitelyAbsent)
            {
                return Err(protocol(
                    "invalid project input coverage or nonterminal page",
                ));
            }
            bytes = bytes
                .checked_add(
                    serde_json::to_vec(&page.records)
                        .map_err(|error| protocol(error.to_string()))?
                        .len(),
                )
                .ok_or_else(|| protocol("project input inventory limit"))?;
            if bytes > MAX_RESOURCE_BYTES || records.len() + page.records.len() > 131_072 {
                return Err(protocol("project input inventory limit"));
            }
            records.extend(page.records);
            after = next_cursor(page.next, &mut seen)?;
            if after.is_none() {
                break page.coverage;
            }
        };
        let projects: Vec<_> = records
            .iter()
            .filter_map(|record| match record {
                ProjectInputRecord::Project(project) => Some(project),
                _ => None,
            })
            .collect();
        if projects.len() > 1
            || (projects.is_empty() && (!records.is_empty() || coverage.found().is_some()))
        {
            return Err(protocol(
                "owning-project input identity is missing or ambiguous",
            ));
        }
        let mut source_sets = Vec::new();
        if let Some(project) = projects.first() {
            if !valid_token(&project.id) || !valid_token(&project.project_path) {
                return Err(protocol("invalid owning-project identity"));
            }
            validate_fact(&project.owning_source_set)?;
            validate_fact(&project.build_identity)?;
            validate_fact(&project.capture)?;
            validate_fact(&project.index)?;
            if project.capture.found().is_some_and(|capture| {
                !valid_token(&capture.identity) || capture.domain != PROJECT_INPUTS_DOMAIN
            }) || project
                .index
                .found()
                .is_some_and(|index| index.counts.domain != "workspace-accepted-unique-inputs")
            {
                return Err(protocol("invalid project capture or index count domain"));
            }
            let mut issued = BTreeSet::new();
            for record in &records {
                if let ProjectInputRecord::SourceSet(source) = record {
                    if !valid_token(&source.id)
                        || !valid_token(&source.name)
                        || source.project != project.id
                        || !issued.insert(source.id.clone())
                        || source.counts.domain != "this-source-set-ordered-entries"
                    {
                        return Err(protocol("invalid or duplicate project source-set identity"));
                    }
                    validate_fact(&source.configurations)?;
                    validate_fact(&source.compile_classpath_coverage)?;
                    validate_fact(&source.runtime_coverage)?;
                    insert_consistent(&mut self.project_scopes, &source.id, source)?;
                    source_sets.push(ProjectSourceSet {
                        descriptor: source.clone(),
                        scope: self.scope,
                    });
                }
            }
            if project
                .owning_source_set
                .found()
                .is_some_and(|id| !issued.contains(id))
            {
                return Err(protocol(
                    "owning source set was not issued by projectInputs",
                ));
            }
            for record in &records {
                let scope = match record {
                    ProjectInputRecord::Root(record) => Some(&record.scope),
                    ProjectInputRecord::ClasspathEntry(record) => {
                        validate_fact(&record.component)?;
                        validate_fact(&record.identity)?;
                        Some(&record.scope)
                    }
                    ProjectInputRecord::CompilerTask(record) => Some(&record.scope),
                    ProjectInputRecord::SourceMembership(record) => Some(&record.scope),
                    ProjectInputRecord::KotlinLogicalSourceSet(record) => {
                        validate_fact(&record.compiler_scope)?;
                        validate_fact(&record.runtime)?;
                        if record.project != project.id {
                            return Err(protocol("foreign logical source-set project"));
                        }
                        None
                    }
                    ProjectInputRecord::LogicalRoot(record) => {
                        if record.project != project.id {
                            return Err(protocol("foreign logical root project"));
                        }
                        None
                    }
                    ProjectInputRecord::LogicalMembership(record) => {
                        if record.project != project.id {
                            return Err(protocol("foreign logical membership project"));
                        }
                        None
                    }
                    _ => None,
                };
                if scope.is_some_and(|id| !issued.contains(id)) {
                    return Err(protocol(
                        "project input refers to an unissued source-set scope",
                    ));
                }
            }
        }
        Ok(ProjectInputs {
            records,
            source_sets,
            coverage,
        })
    }

    pub(super) fn checked_project_scope(&self, scope: &ProjectSourceSet) -> Result<(), FactsError> {
        check_entry(
            scope.scope,
            self.scope,
            &scope.descriptor.id,
            &scope.descriptor,
            &self.project_scopes,
        )?;
        Ok(())
    }
}
