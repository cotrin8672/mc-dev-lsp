use super::{cache::*, host::*, runtime::*};
use mcdev_core::{
    at::AtContextExtractor,
    aw::AwContextExtractor,
    awat::{AwAtBufferSupport, AwAtFileType},
    expression_engine::CancellationChecker,
    mixin_completion::{InjectMethodDescriptorMode, MixinClassInsertMode, MixinCompletionOptions},
    McCompletionReplacementRange,
};
use mcdev_protocol::{
    convert::{CompletionConvertContext, CompletionItemConverter},
    *,
};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

pub fn completion_options(options: &McdevCompletionOptions) -> MixinCompletionOptions {
    MixinCompletionOptions {
        class_insert_mode: if options.mixin_class_insert.eq_ignore_ascii_case("fqn") {
            MixinClassInsertMode::Fqn
        } else {
            MixinClassInsertMode::Import
        },
        inject_method_descriptor_mode: match options
            .inject_method_descriptor
            .to_ascii_lowercase()
            .as_str()
        {
            "always" => InjectMethodDescriptorMode::Always,
            "never" => InjectMethodDescriptorMode::Never,
            _ => InjectMethodDescriptorMode::Auto,
        },
        preferred_at_target: options.preferred_at_target.clone(),
    }
}
/// The wire owns timing/cache fields; the core owns semantic/debug facts.
pub fn completion_from_core(
    request: &McdevCompletionRequest,
    result: mcdev_core::mixin_facade::MixinCompletionResult,
    annotation_context: Option<mcdev_core::mixin_source::AnnotationContext>,
) -> Result<RuntimeCompletion, CommandFailure> {
    let d = result.debug;
    let mut debug = blank_debug(&request.context);
    debug.command = d.command;
    debug.document_uri = d.document_uri;
    debug.language_id = d.language_id;
    debug.parse_source = d.parse_source.map(|p| screaming(&format!("{p:?}")));
    debug.parse_confidence = d.parse_confidence.map(|p| screaming(&format!("{p:?}")));
    debug.used_compilation_unit = d.used_compilation_unit;
    debug.used_java_project = d.used_java_project;
    debug.binding_resolved_count =
        i32::try_from(d.binding_resolved_count).map_err(runtime_error)?;
    debug.binding_failed_count = i32::try_from(d.binding_failed_count).map_err(runtime_error)?;
    debug.fallback_reason = d.fallback_reason;
    debug.semantic_context_found = d.semantic_context_found;
    debug.fallback_annotation_context_used = d.fallback_annotation_context_used;
    debug.fallback_annotation_context_reason = d.fallback_annotation_context_reason;
    debug.semantic_target_count = i32::try_from(d.semantic_target_count).map_err(runtime_error)?;
    debug.semantic_member_count = i32::try_from(d.semantic_member_count).map_err(runtime_error)?;
    debug.completion_context_kind = d.completion_context_kind;
    debug.owner = d.owner;
    debug.method_name = d.method_name;
    debug.method_descriptor = d.method_descriptor;
    debug.candidate_count_before_filter =
        i32::try_from(d.candidate_count_before_filter).map_err(runtime_error)?;
    debug.candidate_count_after_filter =
        i32::try_from(d.candidate_count_after_filter).map_err(runtime_error)?;
    debug.zero_item_reason = d.zero_item_reason;
    debug.warnings = d.warnings;
    Ok(RuntimeCompletion {
        items: result.items,
        replacement_range: result.replacement_range,
        annotation_context,
        debug,
    })
}
fn elapsed(start: Instant) -> i64 {
    start.elapsed().as_millis().min(i64::MAX as u128) as i64
}
fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}
fn payload_bytes(request: &McdevCompletionRequest) -> Result<i32, CommandFailure> {
    i32::try_from(
        request.context.buffer_text.len()
            + utf16_len(&request.context.workspace_root)
            + utf16_len(&request.context.document_uri)
            + utf16_len(&request.context.language_id)
            + utf16_len(&request.options.preferred_at_target)
            + utf16_len(&request.options.mixin_class_insert)
            + utf16_len(&request.options.inject_method_descriptor),
    )
    .map_err(runtime_error)
}
fn key(request: &McdevCompletionRequest) -> String {
    let c = &request.context;
    let hash = java_text_hash(&c.buffer_text);
    [
        c.workspace_root.clone(),
        c.document_uri.clone(),
        c.document_version.unwrap_or(i64::from(hash)).to_string(),
        c.position.line.to_string(),
        c.position.character.to_string(),
        hash.to_string(),
        request.options.preferred_at_target.clone(),
        request.options.mixin_class_insert.clone(),
        request.options.inject_method_descriptor.clone(),
    ]
    .join("|")
}
fn blank_debug(context: &McdevRequestContext) -> McdevCompletionDebugInfo {
    McdevCompletionDebugInfo {
        command: McdevCommands::COMPLETION.into(),
        document_uri: context.document_uri.clone(),
        language_id: context.language_id.clone(),
        total_ms: None,
        payload_bytes: None,
        buffer_text_bytes: None,
        buffer_text_fallback_used: None,
        project_session_cache_hit: None,
        project_session_version: None,
        load_session_ms: None,
        document_version: context.document_version,
        document_cache_hit: None,
        document_snapshot_ms: None,
        semantic_cache_hit: None,
        ast_parse_ms: None,
        parse_source: None,
        parse_confidence: None,
        used_compilation_unit: false,
        used_java_project: false,
        binding_resolved_count: 0,
        binding_failed_count: 0,
        fallback_reason: None,
        semantic_context_found: false,
        fallback_annotation_context_used: false,
        fallback_annotation_context_reason: None,
        semantic_target_count: 0,
        semantic_member_count: 0,
        completion_context_kind: None,
        owner: None,
        method_name: None,
        method_descriptor: None,
        candidate_cache_hit: None,
        candidate_build_ms: None,
        candidate_count_before_filter: 0,
        candidate_count_after_filter: 0,
        dto_convert_ms: None,
        zero_item_reason: None,
        negative_cache_hit: None,
        in_flight_dedup_hit: None,
        warnings: vec![],
    }
}
fn common_debug(
    debug: &mut McdevCompletionDebugInfo,
    request: &McdevCompletionRequest,
    start: Instant,
    dedup_hit: bool,
) -> Result<(), CommandFailure> {
    debug.total_ms = Some(elapsed(start));
    debug.payload_bytes = Some(payload_bytes(request)?);
    debug.buffer_text_bytes =
        Some(i32::try_from(request.context.buffer_text.len()).map_err(runtime_error)?);
    debug.buffer_text_fallback_used = Some(request.context.buffer_text_fallback_used);
    debug.negative_cache_hit = Some(false);
    debug.in_flight_dedup_hit = Some(dedup_hit);
    Ok(())
}
impl<H: CommandHost> CommandDispatcher<H> {
    pub fn negative_cache_size(&self) -> Result<usize, CommandFailure> {
        Ok(self.negative.lock().map_err(runtime_error)?.len())
    }
    pub fn handle_buffer_only(
        &self,
        request: &McdevCompletionRequest,
        cancellation: &dyn CancellationChecker,
    ) -> Result<Option<CommandEnvelope>, CommandFailure> {
        if request.context.protocol_version != PROTOCOL_VERSION
            || request.context.workspace_root.trim().is_empty()
        {
            return Ok(None);
        }
        self.buffer_response(request, Instant::now(), false, true, cancellation)
    }
    pub(super) fn handle_completion(
        &self,
        request: &McdevCompletionRequest,
        project_aware: bool,
        cancellation: &dyn CancellationChecker,
    ) -> Result<CommandEnvelope, CommandFailure> {
        let started = Instant::now();
        if let Some(response) = self.validate(&request.context, false)? {
            return Ok(response);
        }
        let awat = AwAtBufferSupport::detect_file_type(
            &request.context.language_id,
            &request.context.document_uri,
        );
        if project_aware && awat.is_none() && !self.host.has_project_facts(&request.context)? {
            return Ok(failure(
                McdevErrorCode::IncompleteProjectContext,
                "JDT project dependencies are not available yet",
            ));
        }
        let request_key = key(request);
        let (lock, dedup_hit) = {
            let mut locks = self.in_flight.lock().map_err(runtime_error)?;
            let hit = locks.contains_key(&request_key);
            let lock = locks
                .entry(request_key.clone())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone();
            (lock, hit)
        };
        let result = {
            let _guard = lock.lock().map_err(runtime_error)?;
            self.compute_completion(
                request,
                started,
                &request_key,
                dedup_hit,
                project_aware,
                cancellation,
            )
        };
        let mut locks = self.in_flight.lock().map_err(runtime_error)?;
        if locks
            .get(&request_key)
            .is_some_and(|current| Arc::ptr_eq(current, &lock))
        {
            locks.remove(&request_key);
        }
        result
    }
    fn compute_completion(
        &self,
        request: &McdevCompletionRequest,
        started: Instant,
        request_key: &str,
        dedup_hit: bool,
        project_aware: bool,
        cancellation: &dyn CancellationChecker,
    ) -> Result<CommandEnvelope, CommandFailure> {
        if !project_aware {
            let reason = self.negative.lock().map_err(runtime_error)?.get(
                request_key,
                &request.context.buffer_text,
                self.host.now_millis(),
            );
            if let Some(reason) = reason {
                return self.empty_completion(request, &reason, started, dedup_hit);
            }
            if let Some(response) =
                self.buffer_response(request, started, dedup_hit, false, cancellation)?
            {
                return Ok(response);
            }
        }
        let file_type = AwAtBufferSupport::detect_file_type(
            &request.context.language_id,
            &request.context.document_uri,
        );
        if !project_aware
            && is_kotlin(&request.context)
            && !self.host.has_project_facts(&request.context)?
        {
            return Ok(failure(
                McdevErrorCode::IncompleteProjectContext,
                "JDT project dependencies are not available yet",
            ));
        }
        let load_started = Instant::now();
        let cached = self
            .host
            .load_cached_session(&request.context.workspace_root)?;
        let load_ms = elapsed(load_started);
        if let Some(file_type) = file_type {
            let items = self
                .host
                .awat_complete(&cached.context, &request.context, cancellation)?;
            let replacement = usize::try_from(request.context.position.line)
                .ok()
                .zip(usize::try_from(request.context.position.character).ok())
                .and_then(|(line, character)| match file_type {
                    AwAtFileType::AccessWidener => {
                        AwContextExtractor::extract(&request.context.buffer_text, line, character)
                            .map(|c| McCompletionReplacementRange {
                                start_offset: c.value_start_offset,
                                end_offset: c.value_end_offset,
                            })
                    }
                    AwAtFileType::AccessTransformer => {
                        AtContextExtractor::extract(&request.context.buffer_text, line, character)
                            .map(|c| McCompletionReplacementRange {
                                start_offset: c.value_start_offset,
                                end_offset: c.value_end_offset,
                            })
                    }
                });
            let mut convert = CompletionConvertContext::new(&request.context.buffer_text, None);
            convert.mapping_resolver = Some(cached.context.mappings.resolver.as_ref());
            convert.source_namespace = cached.context.mappings.source_namespace;
            convert.runtime_namespace = cached.context.mappings.runtime_namespace;
            convert.replacement_range = replacement;
            return success(
                McdevCompletionResponse {
                    items: CompletionItemConverter::to_dtos(&items, &convert)
                        .map_err(runtime_error)?,
                    is_incomplete: CompletionItemConverter::is_incomplete(&items),
                    warnings: vec![],
                    debug: None,
                },
                &["completion"],
            );
        }
        let snapshot_started = Instant::now();
        let snapshot = self.documents.lock().map_err(runtime_error)?.get(
            &request.context.document_uri,
            request.context.document_version,
            &request.context.buffer_text,
            self.host.now_millis(),
        );
        let snapshot_ms = elapsed(snapshot_started);
        let mut context = request.context.clone();
        context.buffer_text = snapshot.snapshot.text.clone();
        context.document_version = Some(snapshot.snapshot.version);
        let semantic = self.host.semantic_model(
            &context,
            snapshot.snapshot.version,
            &cached.context,
            cancellation,
        )?;
        let options = completion_options(&request.options);
        let mut completion = self.host.complete(
            &cached.context,
            &context,
            &options,
            &semantic.model,
            cached.version,
            cancellation,
        )?;
        self.cache_negative(request_key, request, &completion)?;
        let (candidate_hit, candidate_ms) = self.host.candidate_cache_metrics()?;
        let annotation = usize::try_from(context.position.line)
            .ok()
            .zip(usize::try_from(context.position.character).ok())
            .and_then(|(line, character)| {
                CompletionItemConverter::extract_annotation_context(
                    &context.buffer_text,
                    line,
                    character,
                )
            });
        let dto_started = Instant::now();
        let mut convert = CompletionConvertContext::new(&context.buffer_text, annotation.as_ref());
        convert.class_insert_mode = options.class_insert_mode;
        convert.preferred_at_target = &options.preferred_at_target;
        convert.mapping_resolver = Some(cached.context.mappings.resolver.as_ref());
        convert.source_namespace = cached.context.mappings.source_namespace;
        convert.runtime_namespace = cached.context.mappings.runtime_namespace;
        convert.replacement_range = completion.replacement_range;
        let items =
            CompletionItemConverter::to_dtos(&completion.items, &convert).map_err(runtime_error)?;
        let dto_ms = elapsed(dto_started);
        common_debug(&mut completion.debug, request, started, dedup_hit)?;
        let debug = &mut completion.debug;
        debug.load_session_ms = Some(load_ms);
        debug.project_session_cache_hit = Some(cached.cache_hit);
        debug.project_session_version = Some(i64::try_from(cached.version).map_err(runtime_error)?);
        debug.document_cache_hit = Some(snapshot.cache_hit);
        debug.document_snapshot_ms = Some(snapshot_ms);
        debug.document_version = Some(snapshot.snapshot.version);
        debug.semantic_cache_hit = Some(semantic.cache_hit);
        debug.ast_parse_ms = Some(semantic.parse_ms);
        debug.candidate_cache_hit = candidate_hit;
        debug.candidate_build_ms = candidate_ms;
        debug.dto_convert_ms = Some(dto_ms);
        let mut warnings = semantic
            .model
            .warnings
            .iter()
            .map(|message| McdevWarning {
                code: "MIXIN_PARSE_FALLBACK".into(),
                message: message.clone(),
            })
            .collect::<Vec<_>>();
        warnings.push(McdevWarning {
            code: "MIXIN_PARSE_SOURCE".into(),
            message: screaming(&format!("{:?}", semantic.model.parse_source)),
        });
        success(
            McdevCompletionResponse {
                items,
                is_incomplete: CompletionItemConverter::is_incomplete(&completion.items),
                warnings,
                debug: Some(completion.debug),
            },
            &["completion"],
        )
    }
    fn cache_negative(
        &self,
        key: &str,
        request: &McdevCompletionRequest,
        completion: &RuntimeCompletion,
    ) -> Result<(), CommandFailure> {
        if completion.items.is_empty() {
            if let Some(reason) = &completion.debug.zero_item_reason {
                self.negative.lock().map_err(runtime_error)?.put(
                    key.into(),
                    request.context.buffer_text.clone(),
                    reason.clone(),
                    self.host.now_millis().saturating_add(negative_ttl(reason)),
                );
            }
        }
        Ok(())
    }
    fn buffer_response(
        &self,
        request: &McdevCompletionRequest,
        started: Instant,
        dedup_hit: bool,
        require_items: bool,
        cancellation: &dyn CancellationChecker,
    ) -> Result<Option<CommandEnvelope>, CommandFailure> {
        if AwAtBufferSupport::detect_file_type(
            &request.context.language_id,
            &request.context.document_uri,
        )
        .is_some()
        {
            return Ok(None);
        }
        let options = completion_options(&request.options);
        let Some(completion) =
            self.host
                .buffer_only_completion(&request.context, &options, cancellation)?
        else {
            return Ok(None);
        };
        if require_items && completion.items.is_empty() {
            return Ok(None);
        }
        self.cache_negative(&key(request), request, &completion)?;
        provisional_response(request, completion, started, dedup_hit).map(Some)
    }
    fn empty_completion(
        &self,
        request: &McdevCompletionRequest,
        reason: &str,
        started: Instant,
        dedup_hit: bool,
    ) -> Result<CommandEnvelope, CommandFailure> {
        let mut debug = blank_debug(&request.context);
        common_debug(&mut debug, request, started, dedup_hit)?;
        debug.project_session_cache_hit = Some(false);
        debug.project_session_version = Some(0);
        debug.load_session_ms = Some(0);
        debug.dto_convert_ms = Some(0);
        debug.zero_item_reason = Some(reason.into());
        debug.negative_cache_hit = Some(true);
        success(
            McdevCompletionResponse {
                items: vec![],
                is_incomplete: false,
                warnings: vec![],
                debug: Some(debug),
            },
            &["completion"],
        )
    }
}

/// Shared provisional DTO conversion, also used by the explicit buffer-only process.
pub(crate) fn provisional_response(
    request: &McdevCompletionRequest,
    mut completion: RuntimeCompletion,
    started: Instant,
    dedup_hit: bool,
) -> Result<CommandEnvelope, CommandFailure> {
    let options = completion_options(&request.options);
    let dto_started = Instant::now();
    let mut convert = CompletionConvertContext::new(
        &request.context.buffer_text,
        completion.annotation_context.as_ref(),
    );
    convert.class_insert_mode = options.class_insert_mode;
    convert.preferred_at_target = &options.preferred_at_target;
    convert.replacement_range = completion.replacement_range;
    let items =
        CompletionItemConverter::to_dtos(&completion.items, &convert).map_err(runtime_error)?;
    let dto_ms = elapsed(dto_started);
    common_debug(&mut completion.debug, request, started, dedup_hit)?;
    let debug = &mut completion.debug;
    debug.load_session_ms = Some(0);
    debug.project_session_cache_hit = Some(false);
    debug.project_session_version = Some(0);
    debug.document_version = request.context.document_version;
    debug.document_cache_hit = None;
    debug.document_snapshot_ms = None;
    debug.semantic_cache_hit = None;
    debug.ast_parse_ms = None;
    debug.candidate_cache_hit = None;
    debug.candidate_build_ms = None;
    debug.dto_convert_ms = Some(dto_ms);
    success(
        McdevCompletionResponse {
            items,
            is_incomplete: CompletionItemConverter::is_incomplete(&completion.items),
            warnings: vec![],
            debug: Some(completion.debug),
        },
        &["completion"],
    )
}
fn screaming(value: &str) -> String {
    let mut result = String::new();
    for (index, c) in value.chars().enumerate() {
        if index > 0 && c.is_ascii_uppercase() {
            result.push('_');
        }
        result.push(c.to_ascii_uppercase());
    }
    result
}
