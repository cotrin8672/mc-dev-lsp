//! Explicit no-index JSON-lines mode. It is never auto-detected on semantic-v1 stdin.
use crate::commands::{CommandDispatcher, CommandEnvelope, CommandFailure, CommandHost};
use mcdev_core::expression_engine::NO_CANCELLATION;
use mcdev_protocol::{McdevCompletionRequest, ProtocolPayloadDecoder};
use serde::Serialize;
use serde_json::Value;
use std::io::{self, BufRead, Write};

/// The standalone helper can only answer the shared core's provisional contexts.
/// No workspace facts, binary inventory or feature support are assumed here.
pub fn complete_no_index(
    request: &McdevCompletionRequest,
) -> Result<Option<CommandEnvelope>, CommandFailure> {
    use crate::commands::{completion_from_core, completion_options, provisional_response};
    use mcdev_core::{
        awat::AwAtBufferSupport, mixin_completion::AtValueCompletionService,
        mixin_facade::HostMixinFacade,
    };
    let context = &request.context;
    if context.protocol_version != mcdev_protocol::PROTOCOL_VERSION
        || context.workspace_root.trim().is_empty()
        || AwAtBufferSupport::detect_file_type(&context.language_id, &context.document_uri)
            .is_some()
    {
        return Ok(None);
    }
    let started = std::time::Instant::now();
    let Some(completion) = HostMixinFacade::try_complete_buffer_only(
        &context.buffer_text,
        context.position.line,
        context.position.character,
        &completion_options(&request.options),
        &context.document_uri,
        &context.language_id,
        false,
        AtValueCompletionService::default(),
    )
    .map_err(CommandFailure::Cancelled)?
    else {
        return Ok(None);
    };
    if completion.result.items.is_empty() {
        return Ok(None);
    }
    let completion = completion_from_core(request, completion.result, completion.context)?;
    provisional_response(request, completion, started, false).map(Some)
}

#[derive(Debug, Serialize)]
pub struct CompletionLineResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    pub handled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<CompletionLineError>,
}
#[derive(Debug, Serialize)]
pub struct CompletionLineError {
    pub code: String,
    pub message: String,
}
fn parse_failure(id: Option<Value>) -> CompletionLineResponse {
    CompletionLineResponse {
        id,
        handled: false,
        response: None,
        error: Some(CompletionLineError {
            code: "PARSE_ERROR".into(),
            message: "invalid helper request".into(),
        }),
    }
}
pub fn process_completion_line<H: CommandHost>(
    line: &[u8],
    runtime: &CommandDispatcher<H>,
) -> io::Result<CompletionLineResponse> {
    process_completion_line_with(line, &|request| {
        runtime.handle_buffer_only(request, &NO_CANCELLATION)
    })
}
pub fn process_completion_line_with(
    line: &[u8],
    complete: &impl Fn(&McdevCompletionRequest) -> Result<Option<CommandEnvelope>, CommandFailure>,
) -> io::Result<CompletionLineResponse> {
    let root: Value = match serde_json::from_slice(line) {
        Ok(Value::Object(object)) => Value::Object(object),
        _ => return Ok(parse_failure(None)),
    };
    let id = root.get("id").filter(|id| !id.is_null()).cloned();
    let Some(payload) = root.get("payload").filter(|p| p.is_object()) else {
        return Ok(parse_failure(id));
    };
    let request = match ProtocolPayloadDecoder.decode_completion_request(&[payload.clone()]) {
        Ok(request) => request,
        Err(_) => return Ok(parse_failure(id)),
    };
    let envelope = complete(&request)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    let response = envelope
        .filter(|r| {
            r.result
                .as_ref()
                .and_then(|v| v["items"].as_array())
                .is_some_and(|items| !items.is_empty())
        })
        .map(|r| ProtocolPayloadDecoder.encode_to_map(&r).map(Value::Object))
        .transpose()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    Ok(CompletionLineResponse {
        id,
        handled: response.is_some(),
        response,
        error: None,
    })
}
pub fn run_completion_lines<R: BufRead, W: Write, H: CommandHost>(
    input: R,
    output: W,
    runtime: &CommandDispatcher<H>,
) -> io::Result<()> {
    run_completion_lines_with(input, output, &|request| {
        runtime.handle_buffer_only(request, &NO_CANCELLATION)
    })
}
pub fn run_completion_lines_with<R: BufRead, W: Write>(
    mut input: R,
    mut output: W,
    complete: &impl Fn(&McdevCompletionRequest) -> Result<Option<CommandEnvelope>, CommandFailure>,
) -> io::Result<()> {
    let mut line = Vec::new();
    loop {
        line.clear();
        if input.read_until(b'\n', &mut line)? == 0 {
            break;
        }
        let response = process_completion_line_with(&line, complete)?;
        serde_json::to_writer(&mut output, &response)?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
    Ok(())
}
