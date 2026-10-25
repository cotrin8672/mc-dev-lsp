//! Command capability boundary. Implementations supply authoritative host facts;
//! this library never discovers a workspace, classpath or source tree itself.
use mcdev_core::{
    expression_engine::{Cancellation, CancellationChecker},
    mixin_completion::MixinCompletionOptions,
    mixin_source::{AnnotationContext, MixinClassModel},
    project::{CachedProjectSession, ProjectContext},
    McCompletionItem, McCompletionReplacementRange, McDefinitionTarget, McDiagnostic, McFix,
    McReferenceLocation, SourceScanEntry,
};
use mcdev_protocol::{
    convert::ResolvedDefinition, McdevCompletionDebugInfo, McdevError, McdevRange,
    McdevRequestContext,
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum CommandFailure {
    Cancelled(Cancellation),
    Runtime(String),
    Unavailable(McdevError),
}
impl From<Cancellation> for CommandFailure {
    fn from(error: Cancellation) -> Self {
        Self::Cancelled(error)
    }
}
impl std::fmt::Display for CommandFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled(e) => e.fmt(f),
            Self::Runtime(message) => f.write_str(message),
            Self::Unavailable(error) => f.write_str(&error.message),
        }
    }
}
impl std::error::Error for CommandFailure {}

#[derive(Debug, Clone)]
pub struct RuntimeCompletion {
    pub items: Vec<McCompletionItem>,
    pub replacement_range: Option<McCompletionReplacementRange>,
    pub annotation_context: Option<AnnotationContext>,
    pub debug: McdevCompletionDebugInfo,
}
#[derive(Debug, Clone)]
pub struct SemanticSnapshot {
    pub model: MixinClassModel,
    pub cache_hit: bool,
    pub parse_ms: i64,
}
#[derive(Debug, Clone)]
pub struct BuildIdentity {
    pub commit: String,
    pub build_time: String,
    pub version: String,
    pub artifact_location: String,
}

impl BuildIdentity {
    pub fn current() -> Self {
        Self {
            commit: option_env!("MCDEV_BUILD_COMMIT")
                .filter(|value| !value.is_empty())
                .unwrap_or("unknown")
                .to_owned(),
            build_time: option_env!("MCDEV_BUILD_TIME")
                .filter(|value| !value.is_empty())
                .unwrap_or("unknown")
                .to_owned(),
            version: option_env!("MCDEV_BUILD_VERSION")
                .filter(|value| !value.is_empty())
                .unwrap_or(env!("CARGO_PKG_VERSION"))
                .to_owned(),
            artifact_location: std::env::current_exe()
                .ok()
                .map(|path| path.to_string_lossy().into_owned())
                .filter(|path| !path.is_empty())
                .unwrap_or_else(|| "unknown".to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BuildIdentity;
    use std::path::Path;

    #[test]
    fn current_reports_executable_and_compile_time_metadata() {
        let identity = BuildIdentity::current();

        assert!(!identity.version.is_empty());
        assert!(Path::new(&identity.artifact_location).is_file());
        assert_eq!(
            identity.commit,
            option_env!("MCDEV_BUILD_COMMIT")
                .filter(|value| !value.is_empty())
                .unwrap_or("unknown")
        );
        assert_eq!(
            identity.build_time,
            option_env!("MCDEV_BUILD_TIME")
                .filter(|value| !value.is_empty())
                .unwrap_or("unknown")
        );
        assert_eq!(
            identity.version,
            option_env!("MCDEV_BUILD_VERSION")
                .filter(|value| !value.is_empty())
                .unwrap_or(env!("CARGO_PKG_VERSION"))
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFamily {
    JavaKotlin,
    AwAt,
}

/// Library-only operations. These are not semantic-v1 wire capabilities.
/// Required methods intentionally have no success-producing default adapter.
pub trait CommandHost: Send + Sync {
    fn now_millis(&self) -> i64;
    fn build_identity(&self) -> Result<BuildIdentity, CommandFailure>;
    fn has_project_facts(&self, context: &McdevRequestContext) -> Result<bool, CommandFailure>;
    fn load_session(&self, workspace_root: &str) -> Result<Arc<ProjectContext>, CommandFailure>;
    fn load_cached_session(
        &self,
        workspace_root: &str,
    ) -> Result<CachedProjectSession, CommandFailure>;
    fn reindex(&self, workspace_root: &str) -> Result<Arc<ProjectContext>, CommandFailure>;
    fn clear_project_cache(&self) -> Result<(), CommandFailure>;
    fn semantic_model(
        &self,
        context: &McdevRequestContext,
        snapshot_version: i64,
        project: &ProjectContext,
        cancellation: &dyn CancellationChecker,
    ) -> Result<SemanticSnapshot, CommandFailure>;
    fn buffer_only_completion(
        &self,
        context: &McdevRequestContext,
        options: &MixinCompletionOptions,
        cancellation: &dyn CancellationChecker,
    ) -> Result<Option<RuntimeCompletion>, CommandFailure>;
    fn complete(
        &self,
        project: &ProjectContext,
        context: &McdevRequestContext,
        options: &MixinCompletionOptions,
        semantic: &MixinClassModel,
        project_version: u64,
        cancellation: &dyn CancellationChecker,
    ) -> Result<RuntimeCompletion, CommandFailure>;
    fn candidate_cache_metrics(&self) -> Result<(Option<bool>, Option<i64>), CommandFailure>;
    fn awat_complete(
        &self,
        project: &ProjectContext,
        context: &McdevRequestContext,
        cancellation: &dyn CancellationChecker,
    ) -> Result<Vec<McCompletionItem>, CommandFailure>;
    fn diagnostics(
        &self,
        project: &ProjectContext,
        context: &McdevRequestContext,
        family: SourceFamily,
        cancellation: &dyn CancellationChecker,
    ) -> Result<Vec<McDiagnostic>, CommandFailure>;
    fn definitions(
        &self,
        project: &ProjectContext,
        context: &McdevRequestContext,
        family: SourceFamily,
        cancellation: &dyn CancellationChecker,
    ) -> Result<Vec<McDefinitionTarget>, CommandFailure>;
    fn resolve_definitions(
        &self,
        project: &ProjectContext,
        workspace_root: &str,
        targets: &[McDefinitionTarget],
    ) -> Result<Vec<ResolvedDefinition>, CommandFailure>;
    fn source_entries(
        &self,
        project: &ProjectContext,
        context: &McdevRequestContext,
        family: SourceFamily,
    ) -> Result<Vec<SourceScanEntry>, CommandFailure>;
    fn references(
        &self,
        project: &ProjectContext,
        target: &McDefinitionTarget,
        sources: &[SourceScanEntry],
        family: SourceFamily,
        cancellation: &dyn CancellationChecker,
    ) -> Result<Vec<McReferenceLocation>, CommandFailure>;
    fn code_actions(
        &self,
        project: &ProjectContext,
        context: &McdevRequestContext,
        range: &McdevRange,
        diagnostic_code: Option<&str>,
        family: SourceFamily,
        cancellation: &dyn CancellationChecker,
    ) -> Result<Vec<McFix>, CommandFailure>;
    fn selected_mixin_config_content(
        &self,
        project: &ProjectContext,
        context: &McdevRequestContext,
    ) -> Result<Option<String>, CommandFailure>;
    fn resolve_fix(
        &self,
        project: &ProjectContext,
        context: &McdevRequestContext,
        fix: &McFix,
        config_content: Option<&str>,
    ) -> Result<Option<(String, McFix)>, CommandFailure>;
}
