//! Host-owned command evaluations, with no cross-evaluation session caching.
mod definition;
pub mod index;
mod project;
pub mod semantic;

use crate::{
    host_facts::{
        bridge::ScopedFacts,
        draft::SupportedOps,
        inputs::{InputOps, ScopedInputs},
        FactsError,
    },
    provider_commands::CommandFacts,
    wire::{Evaluation, Registry},
};
use mcdev_core::{
    expression_engine::{Cancellation, CancellationChecker},
    host_source::HostSourceError,
    model::ClassIndex,
    project::{normalize_path, uri_to_path},
};
use mcdev_protocol::{
    Command, DecodedCommand, McdevError, McdevResponseEnvelope, ProtocolPayloadDecoder,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};

use super::{
    BuildIdentity, CommandDispatcher, CommandFailure, CoreCommandHost, EvaluationCommandInputs,
};

/// Advertise only commands with an implemented evaluation route.
pub fn supports_command(name: &str) -> bool {
    matches!(
        Command::parse(name),
        Some(
            Command::Completion
                | Command::Diagnostics
                | Command::Context
                | Command::Definition
                | Command::Hover
        )
    )
}

#[derive(Debug)]
struct ValidatedCommand {
    decoded: DecodedCommand,
    evaluation: Evaluation,
    workspace_root: String,
    document_uri: String,
    document_version: Option<i64>,
    refresh_generation: Option<String>,
}

fn same_path(left: &str, right: &str) -> Result<bool, FactsError> {
    let left = uri_to_path(left).map_err(FactsError::Protocol)?;
    let right = uri_to_path(right).map_err(FactsError::Protocol)?;
    if !left.is_absolute() || !right.is_absolute() {
        return Err(FactsError::Protocol(
            "command paths must be absolute".into(),
        ));
    }
    let left = normalize_path(&left);
    let right = normalize_path(&right);
    Ok(if cfg!(windows) {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    } else {
        left == right
    })
}

fn validated_command(
    params: &Value,
    initialization: &Value,
) -> Result<Result<ValidatedCommand, McdevError>, FactsError> {
    let command = params["command"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| FactsError::Protocol("missing command".into()))?;
    if !initialization["commands"]
        .as_array()
        .is_some_and(|commands| commands.iter().any(|id| id == command))
    {
        return Err(FactsError::Unsupported(
            "command was not acknowledged".into(),
        ));
    }
    let arguments = params["arguments"]
        .as_array()
        .ok_or_else(|| FactsError::Protocol("command arguments must be an array".into()))?;
    let decoded = match ProtocolPayloadDecoder.decode_command(command, arguments) {
        Ok(decoded) => decoded,
        Err(error) => return Ok(Err(error.as_protocol_error())),
    };
    let evaluation: Evaluation = serde_json::from_value(params.clone())
        .map_err(|error| FactsError::Protocol(format!("invalid command evaluation: {error}")))?;
    let mut registry = Registry::default();
    evaluation
        .validate(&mut registry)
        .map_err(FactsError::Protocol)?;
    let host = params["context"]
        .as_object()
        .ok_or_else(|| FactsError::Protocol("missing validated host context".into()))?;
    let field = |name: &str| {
        host.get(name)
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| FactsError::Protocol(format!("missing host context {name}")))
    };
    let workspace_root = field("workspaceRoot")?;
    let document_uri = field("documentUri")?;
    let source_sha = field("sourceSha256")?;
    if source_sha != format!("{:x}", Sha256::digest(evaluation.source.as_bytes())) {
        return Err(FactsError::Stale(
            "host source hash differs from evaluation text".into(),
        ));
    }
    let version = match host.get("documentVersion") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_i64()
                .ok_or_else(|| FactsError::Protocol("invalid host document version".into()))?,
        ),
    };
    let refresh_generation = match host.get("refreshGeneration") {
        None | Some(Value::Null) => None,
        Some(Value::String(value))
            if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) =>
        {
            Some(value.clone())
        }
        Some(_) => {
            return Err(FactsError::Protocol(
                "invalid host refresh generation".into(),
            ))
        }
    };
    let inner = decoded.context();
    let language = match host.get("languageId") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) if !value.is_empty() => Some(value.as_str()),
        Some(_) => return Err(FactsError::Protocol("invalid host language id".into())),
    };
    if let Some(language) = language {
        use mcdev_core::awat::AwAtBufferSupport;
        let host_family = AwAtBufferSupport::detect_file_type(language, document_uri);
        let inner_family = AwAtBufferSupport::detect_file_type(&inner.language_id, document_uri);
        if !language.eq_ignore_ascii_case(&inner.language_id)
            && !(host_family.is_some() && host_family == inner_family)
        {
            return Err(FactsError::Stale(
                "command language differs from the host snapshot".into(),
            ));
        }
    }
    if !same_path(&inner.workspace_root, workspace_root)?
        || !same_path(&inner.document_uri, document_uri)?
        || inner.buffer_text != evaluation.source
        || inner
            .document_version
            .is_some_and(|value| Some(value) != version)
    {
        return Err(FactsError::Stale(
            "command context differs from the host snapshot".into(),
        ));
    }
    Ok(Ok(ValidatedCommand {
        decoded,
        evaluation,
        workspace_root: workspace_root.to_owned(),
        document_uri: document_uri.to_owned(),
        document_version: version,
        refresh_generation,
    }))
}

fn encode_envelope(value: &McdevResponseEnvelope<Value>) -> Result<Value, FactsError> {
    ProtocolPayloadDecoder
        .encode_to_map(value)
        .map(Value::Object)
        .map_err(|error| FactsError::Protocol(format!("invalid command response: {error}")))
}

fn domain_failure(error: McdevError) -> Result<Value, FactsError> {
    encode_envelope(&McdevResponseEnvelope::failure(error))
}

fn source_error(error: FactsError) -> HostSourceError {
    error.core()
}

struct CommandCancellation(CommandFacts);
impl CancellationChecker for CommandCancellation {
    fn check_cancelled(&self) -> Result<(), Cancellation> {
        self.0
            .check_active()
            .map_err(|error| Cancellation(Arc::new(error)))
    }
}

fn execute_inner(
    params: Value,
    initialization: Value,
    facts: CommandFacts,
) -> Result<Value, FactsError> {
    facts.check_active()?;
    let validated = match validated_command(&params, &initialization)? {
        Ok(value) => value,
        Err(error) => return domain_failure(error),
    };
    if let DecodedCommand::Completion(request) = &validated.decoded {
        if let Some(response) =
            crate::completion_lines::complete_no_index(request).map_err(|error| match error {
                CommandFailure::Cancelled(error) => FactsError::Cancelled(error.to_string()),
                CommandFailure::Unavailable(error) => FactsError::Unknown(error.message),
                CommandFailure::Runtime(error) => FactsError::Protocol(error),
            })?
        {
            facts.check_active()?;
            return encode_envelope(&response);
        }
    }
    if let DecodedCommand::Definition(request) = &validated.decoded {
        if request.context.protocol_version != mcdev_protocol::PROTOCOL_VERSION {
            return encode_envelope(&mcdev_protocol::protocol_mismatch(
                request.context.protocol_version,
            ));
        }
        if let (Ok(line), Ok(character)) = (
            usize::try_from(request.context.position.line),
            usize::try_from(request.context.position.character),
        ) {
            if let Some(response) = definition::expression_definition(
                &validated.evaluation,
                &validated.document_uri,
                line,
                character,
                facts.clone(),
            )? {
                let result = serde_json::to_value(response).map_err(|e| {
                    FactsError::Protocol(format!("invalid definition response: {e}"))
                })?;
                facts.check_active()?;
                return encode_envelope(&McdevResponseEnvelope::success(
                    result,
                    ["definition".to_owned()],
                ));
            }
        }
    }

    let ops = InputOps::intersect(&initialization)?;
    let mut inputs = ScopedInputs::new(&validated.evaluation.evaluation, &ops, facts.clone())?;
    let project_ops = SupportedOps::intersect(&initialization)?;
    let mut registry = Registry::default();
    let mut project_reader = facts.clone();
    let project_facts = ScopedFacts::new(
        &validated.evaluation.evaluation,
        &project_ops,
        &mut registry,
        &mut project_reader,
    )?
    .project_context()?;
    let project = project::selected_project(
        &mut inputs,
        project_facts,
        &validated.workspace_root,
        &validated.evaluation.evaluation,
    )?;
    if !project.project_complete
        && matches!(
            validated.decoded.command(),
            Command::Info
                | Command::DumpContext
                | Command::References
                | Command::CodeAction
                | Command::Reindex
                | Command::ReloadProjectContext
        )
    {
        return Err(FactsError::Unknown(
            "complete project source-set and resource inventory is unavailable".into(),
        ));
    }
    let inputs = Arc::new(Mutex::new(inputs));
    let index = Arc::new(index::EvaluationIndex::new(inputs, facts.clone()));
    let runtime_index = Arc::new(index.physical_view());
    let active_facts = facts.clone();
    let check_active = Arc::new(move || active_facts.check_active().map_err(source_error));
    let specifier_index = index.clone();
    let specifier_supported = Arc::new(move || {
        Ok(specifier_index
            .find_class_by_fqn(mcdev_core::host_source::INJECTION_POINT_SPECIFIER_FQN)
            .is_some())
    });
    let additional_at_values = Arc::new(|| {
        Err(HostSourceError::Failure(
            "complete injection point subtype annotation facts are unavailable".into(),
        ))
    });
    let definition_evaluation = serde_json::from_value(params)
        .map_err(|error| FactsError::Protocol(format!("invalid definition evaluation: {error}")))?;
    let definitions = Arc::new(definition::CurrentDefinitions::new(
        definition_evaluation,
        validated.document_uri.clone(),
        facts.clone(),
    )?);
    let semantic = Arc::new(
        semantic::HostSemanticProvider::new(
            validated.evaluation,
            facts.clone(),
            validated.document_uri,
            validated.document_version,
        )?
        .with_index(index.clone()),
    );
    let host = CoreCommandHost::with_evaluation(
        EvaluationCommandInputs {
            workspace_root_uri: validated.workspace_root,
            cached_session: project.cached_session,
            class_index: index.clone(),
            bytecode_index: runtime_index,
            reference_types: index,
            definitions,
            check_active,
            specifier_supported,
            additional_at_values,
            refresh_generation: validated.refresh_generation,
        },
        semantic,
        project.provider,
        project.sources,
        BuildIdentity::current(),
        true,
    )
    .map_err(|error| match error {
        CommandFailure::Cancelled(error) => FactsError::Cancelled(error.to_string()),
        CommandFailure::Unavailable(error) => FactsError::Unknown(error.message),
        CommandFailure::Runtime(error) => FactsError::Protocol(error),
    })?;
    let dispatcher = CommandDispatcher::new(Arc::new(host));
    let envelope = dispatcher
        .execute_decoded(
            validated.decoded,
            false,
            &CommandCancellation(facts.clone()),
        )
        .map_err(|error| match error {
            CommandFailure::Cancelled(error) => FactsError::Cancelled(error.to_string()),
            CommandFailure::Unavailable(error) => FactsError::Unknown(error.message),
            CommandFailure::Runtime(error) => FactsError::Protocol(error),
        })?;
    facts.check_active()?;
    encode_envelope(&envelope)
}

/// Preserve the domain error envelope while propagating transport cancellation/staleness.
pub fn execute(
    params: Value,
    initialization: Value,
    facts: CommandFacts,
) -> Result<Value, FactsError> {
    match execute_inner(params, initialization, facts) {
        Err(
            error @ (FactsError::Cancelled(_) | FactsError::Stale(_) | FactsError::Protocol(_)),
        ) => Err(error),
        Err(error) => domain_failure(error.protocol()),
        success => success,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcdev_core::project::path_to_uri;
    use serde_json::json;
    use std::time::{Duration, Instant};

    fn request(command: &str, source: &str, position: i32) -> (Value, Value) {
        let root = std::env::temp_dir().join("mcdev-ipc-command-test");
        let uri = path_to_uri(&root.join("Owner.java"));
        let root_uri = path_to_uri(&root);
        let initialization = json!({"commands":[command], "capabilities":["semantic-v1"]});
        let params = json!({
            "evaluation":"42", "owner":root.join("Owner.java").to_string_lossy(),
            "source":source, "configuration":{}, "declarations":[], "command":command,
            "context":{"workspaceRoot":root.to_string_lossy(),"documentUri":uri,
                "sourceSha256":format!("{:x}", Sha256::digest(source.as_bytes())),"documentVersion":7,"languageId":"java"},
            "arguments":[{"context":{"workspaceRoot":root_uri,"documentUri":uri,
                "languageId":"java","position":{"line":0,"character":position},
                "bufferText":source,"documentVersion":7}}]
        });
        (params, initialization)
    }

    #[test]
    fn command_context_must_match_the_host_owned_text_uri_and_version() {
        let (params, initialization) = request("mcdev.diagnostics", "class Owner {}", 0);
        let valid = validated_command(&params, &initialization)
            .unwrap()
            .unwrap();
        assert_eq!(valid.document_version, Some(7));
        for (pointer, value) in [
            ("/arguments/0/context/bufferText", json!("class Changed {}")),
            ("/arguments/0/context/documentVersion", json!(6)),
            ("/arguments/0/context/languageId", json!("kotlin")),
            ("/context/sourceSha256", json!("0".repeat(64))),
            (
                "/arguments/0/context/documentUri",
                json!(path_to_uri(&std::env::temp_dir().join("other.java"))),
            ),
        ] {
            let mut changed = params.clone();
            *changed.pointer_mut(pointer).unwrap() = value;
            assert!(
                matches!(
                    validated_command(&changed, &initialization),
                    Err(FactsError::Stale(_))
                ),
                "{pointer}"
            );
        }
    }

    #[test]
    fn malformed_domain_payload_keeps_the_legacy_parse_error() {
        let (mut params, initialization) = request("mcdev.diagnostics", "class Owner {}", 0);
        params["arguments"] = json!([null]);
        let error = validated_command(&params, &initialization)
            .unwrap()
            .unwrap_err();
        assert_eq!(error.code, mcdev_protocol::McdevErrorCode::ParseError);
        let mut not_acknowledged = initialization;
        not_acknowledged["commands"] = json!([]);
        assert!(matches!(
            validated_command(&params, &not_acknowledged),
            Err(FactsError::Unsupported(_))
        ));
    }

    #[test]
    fn buffer_only_completion_uses_the_real_core_without_host_reads() {
        let (params, initialization) = request("mcdev.completion", "@Inject(meth", 12);
        let (facts, requests) = CommandFacts::new(
            json!("42"),
            initialization.clone(),
            Instant::now() + Duration::from_secs(3),
        );
        let response = execute(params, initialization, facts).unwrap();
        assert!(response["result"]["items"]
            .as_array()
            .is_some_and(|items| !items.is_empty()));
        assert!(matches!(
            requests.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty | std::sync::mpsc::TryRecvError::Disconnected)
        ));
    }
}
