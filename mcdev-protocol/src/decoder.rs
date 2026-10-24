use crate::dto::*;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolDecodeError {
    pub message: String,
}
impl std::fmt::Display for ProtocolDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ProtocolDecodeError {}
impl ProtocolDecodeError {
    pub fn as_protocol_error(&self) -> McdevError {
        McdevError {
            code: McdevErrorCode::ParseError,
            message: self.message.clone(),
            details: Default::default(),
        }
    }
}
fn error(message: impl Into<String>) -> ProtocolDecodeError {
    ProtocolDecodeError {
        message: message.into(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodedCommand {
    Completion(McdevCompletionRequest),
    Definition(McdevDefinitionRequest),
    References(McdevReferencesRequest),
    Hover(McdevHoverRequest),
    CodeAction(McdevCodeActionRequest),
    Diagnostics(McdevDiagnosticsRequest),
    Reindex(McdevReindexRequest),
    ReloadProjectContext(McdevReloadProjectContextRequest),
    DumpContext(McdevDumpContextRequest),
    Context(McdevDiagnosticsRequest),
    Info(McdevInfoRequest),
}
impl DecodedCommand {
    pub fn command(&self) -> Command {
        match self {
            Self::Completion(_) => Command::Completion,
            Self::Definition(_) => Command::Definition,
            Self::References(_) => Command::References,
            Self::Hover(_) => Command::Hover,
            Self::CodeAction(_) => Command::CodeAction,
            Self::Diagnostics(_) => Command::Diagnostics,
            Self::Reindex(_) => Command::Reindex,
            Self::ReloadProjectContext(_) => Command::ReloadProjectContext,
            Self::DumpContext(_) => Command::DumpContext,
            Self::Context(_) => Command::Context,
            Self::Info(_) => Command::Info,
        }
    }
    pub fn context(&self) -> &McdevRequestContext {
        match self {
            Self::Completion(r) => &r.context,
            Self::Definition(r) => &r.context,
            Self::References(r) => &r.context,
            Self::Hover(r) => &r.context,
            Self::CodeAction(r) => &r.context,
            Self::Diagnostics(r) | Self::Context(r) => &r.context,
            Self::Reindex(r) => &r.context,
            Self::ReloadProjectContext(r) => &r.context,
            Self::DumpContext(r) => &r.context,
            Self::Info(r) => &r.context,
        }
    }
}
#[derive(Debug, Default, Clone, Copy)]
pub struct ProtocolPayloadDecoder;
macro_rules! context_decoder {
    ($method:ident,$ty:ident) => {
        pub fn $method(&self, arguments: &[Value]) -> Result<$ty, ProtocolDecodeError> {
            Ok($ty {
                context: self.decode_context_request(arguments)?,
            })
        }
    };
}
impl ProtocolPayloadDecoder {
    pub fn decode_command(
        &self,
        command: &str,
        arguments: &[Value],
    ) -> Result<DecodedCommand, ProtocolDecodeError> {
        match Command::parse(command)
            .ok_or_else(|| error(format!("unknown mcdev command: {command}")))?
        {
            Command::Completion => self
                .decode_completion_request(arguments)
                .map(DecodedCommand::Completion),
            Command::Definition => self
                .decode_definition_request(arguments)
                .map(DecodedCommand::Definition),
            Command::References => self
                .decode_references_request(arguments)
                .map(DecodedCommand::References),
            Command::Hover => self
                .decode_hover_request(arguments)
                .map(DecodedCommand::Hover),
            Command::CodeAction => self
                .decode_code_action_request(arguments)
                .map(DecodedCommand::CodeAction),
            Command::Diagnostics => self
                .decode_diagnostics_request(arguments)
                .map(DecodedCommand::Diagnostics),
            Command::Reindex => self
                .decode_reindex_request(arguments)
                .map(DecodedCommand::Reindex),
            Command::ReloadProjectContext => self
                .decode_reload_project_context_request(arguments)
                .map(DecodedCommand::ReloadProjectContext),
            Command::DumpContext => self
                .decode_dump_context_request(arguments)
                .map(DecodedCommand::DumpContext),
            Command::Context => self
                .decode_diagnostics_request(arguments)
                .map(DecodedCommand::Context),
            Command::Info => self
                .decode_info_request(arguments)
                .map(DecodedCommand::Info),
        }
    }
    pub fn decode_context_request(
        &self,
        arguments: &[Value],
    ) -> Result<McdevRequestContext, ProtocolDecodeError> {
        decode_context(decode_root(arguments)?)
    }
    pub fn decode_completion_request(
        &self,
        arguments: &[Value],
    ) -> Result<McdevCompletionRequest, ProtocolDecodeError> {
        let root = decode_root(arguments)?;
        let context = decode_context(root)?;
        let trigger = root.get("trigger").and_then(Value::as_object);
        let options = root.get("options").and_then(Value::as_object);
        Ok(McdevCompletionRequest {
            context,
            trigger: McdevCompletionTrigger {
                kind: string_or(trigger, "kind", "manual"),
                character: nullable_string(trigger, "character"),
            },
            options: McdevCompletionOptions {
                preferred_at_target: string_or(options, "preferredAtTarget", "smart"),
                mixin_class_insert: string_or(options, "mixinClassInsert", "import"),
                inject_method_descriptor: string_or(options, "injectMethodDescriptor", "auto"),
            },
        })
    }
    pub fn decode_code_action_request(
        &self,
        arguments: &[Value],
    ) -> Result<McdevCodeActionRequest, ProtocolDecodeError> {
        let root = decode_root(arguments)?;
        let context = decode_context(root)?;
        let range = if let Some(range) = root
            .get("range")
            .and_then(Value::as_object)
            .filter(|r| r.contains_key("start") && r.contains_key("end"))
        {
            McdevRange {
                start: decode_position(range.get("start"), "missing range start")?,
                end: decode_position(range.get("end"), "missing range end")?,
            }
        } else {
            McdevRange::default()
        };
        let diagnostic_codes = match root.get("diagnosticCodes").and_then(Value::as_array) {
            Some(values) => values
                .iter()
                .filter(|v| !v.is_null())
                .map(|v| match v {
                    Value::String(v) => Ok(v.clone()),
                    Value::Bool(_) | Value::Number(_) => Ok(v.to_string()),
                    _ => Err(error("diagnosticCodes entries must be primitives")),
                })
                .collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        };
        Ok(McdevCodeActionRequest {
            context,
            range,
            diagnostic_codes,
        })
    }
    context_decoder!(decode_definition_request, McdevDefinitionRequest);
    context_decoder!(decode_references_request, McdevReferencesRequest);
    context_decoder!(decode_hover_request, McdevHoverRequest);
    context_decoder!(decode_info_request, McdevInfoRequest);
    context_decoder!(decode_diagnostics_request, McdevDiagnosticsRequest);
    context_decoder!(decode_reindex_request, McdevReindexRequest);
    context_decoder!(
        decode_reload_project_context_request,
        McdevReloadProjectContextRequest
    );
    context_decoder!(decode_dump_context_request, McdevDumpContextRequest);
    pub fn encode_to_map<T: Serialize>(
        &self,
        value: &T,
    ) -> Result<Map<String, Value>, ProtocolDecodeError> {
        let mut value =
            crate::dto::with_definition_resolution_adapter(|| serde_json::to_value(value))
                .map_err(|e| error(e.to_string()))?;
        omit_null_object_fields(&mut value);
        value
            .as_object()
            .cloned()
            .ok_or_else(|| error("encoded payload must be an object"))
    }
    pub fn decode_from_map<T: DeserializeOwned>(
        &self,
        map: Map<String, Value>,
    ) -> Result<T, ProtocolDecodeError> {
        serde_json::from_value(Value::Object(map)).map_err(|e| error(e.to_string()))
    }
}
fn omit_null_object_fields(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|_, v| !v.is_null());
            for v in map.values_mut() {
                omit_null_object_fields(v);
            }
        }
        Value::Array(values) => {
            for v in values {
                omit_null_object_fields(v);
            }
        }
        _ => {}
    }
}
fn decode_root(arguments: &[Value]) -> Result<&Map<String, Value>, ProtocolDecodeError> {
    let payload = arguments
        .first()
        .filter(|v| !v.is_null())
        .ok_or_else(|| error("missing command payload"))?;
    payload
        .as_object()
        .ok_or_else(|| error("command payload must be an object"))
}
fn nullable_string(object: Option<&Map<String, Value>>, name: &str) -> Option<String> {
    match object?.get(name)? {
        Value::Null => None,
        Value::String(v) => Some(v.clone()),
        v => Some(v.to_string()),
    }
}
fn string_or(object: Option<&Map<String, Value>>, name: &str, default: &str) -> String {
    nullable_string(object, name).unwrap_or_else(|| default.into())
}
fn required_string(object: &Map<String, Value>, name: &str) -> Result<String, ProtocolDecodeError> {
    nullable_string(Some(object), name).ok_or_else(|| error(format!("missing field: {name}")))
}
fn number_i32(value: Option<&Value>, default: i32, name: &str) -> Result<i32, ProtocolDecodeError> {
    match value {
        None | Some(Value::Null) => Ok(default),
        Some(Value::Number(n)) => {
            if let Some(v) = n.as_i64() {
                i32::try_from(v).map_err(|_| error(format!("out of range field: {name}")))
            } else if let Some(v) = n
                .as_f64()
                .filter(|v| v.is_finite() && *v >= i32::MIN as f64 && *v <= i32::MAX as f64)
            {
                Ok(v as i32)
            } else {
                Err(error(format!("invalid integer field: {name}")))
            }
        }
        Some(Value::String(v)) => v
            .parse::<i32>()
            .map_err(|_| error(format!("invalid integer field: {name}"))),
        _ => Err(error(format!("invalid integer field: {name}"))),
    }
}
fn long_or_null(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(n) => n.as_i64().or_else(|| {
            n.as_f64()
                .filter(|v| v.is_finite() && *v >= i64::MIN as f64 && *v < i64::MAX as f64)
                .map(|n| n as i64)
        }),
        Value::String(v) => v.parse().ok(),
        _ => None,
    }
}
fn decode_position(
    value: Option<&Value>,
    message: &str,
) -> Result<McdevPosition, ProtocolDecodeError> {
    let position = value
        .and_then(Value::as_object)
        .ok_or_else(|| error(message))?;
    let line = number_i32(position.get("line"), 0, "line")?;
    let character = number_i32(position.get("character"), 0, "character")?;
    if line < 0 || character < 0 {
        return Err(error("position must be nonnegative"));
    }
    Ok(McdevPosition { line, character })
}
fn decode_context(root: &Map<String, Value>) -> Result<McdevRequestContext, ProtocolDecodeError> {
    let context = if let Some(value) = root.get("context") {
        value
            .as_object()
            .ok_or_else(|| error("request context must be an object"))?
    } else if root.contains_key("workspaceRoot") {
        root
    } else {
        return Err(error("missing request context"));
    };
    let protocol_version = number_i32(
        context.get("protocolVersion"),
        PROTOCOL_VERSION,
        "protocolVersion",
    )?;
    let workspace_root = required_string(context, "workspaceRoot")?;
    let document_uri = required_string(context, "documentUri")?;
    let direct = nullable_string(Some(context), "bufferText");
    let fallback = nullable_string(Some(context), "bufferTextFallback");
    let position = decode_position(context.get("position"), "missing cursor position")?;
    let client = context.get("client").and_then(Value::as_object);
    Ok(McdevRequestContext {
        protocol_version,
        workspace_root,
        document_uri,
        language_id: string_or(Some(context), "languageId", "java"),
        position,
        buffer_text_fallback_used: direct.is_none() && fallback.is_some(),
        buffer_text: direct.or(fallback).unwrap_or_default(),
        document_version: long_or_null(context.get("documentVersion"))
            .or_else(|| long_or_null(context.get("changedtick"))),
        client: McdevClientInfo {
            name: string_or(client, "name", "unknown"),
            version: string_or(client, "version", "0.0.0"),
        },
    })
}
