//! MixinExtras local capture and Local/Share completion, using host source/index facts.
use crate::{
    bytecode::BytecodeIndex, extras_model::*, mixin_completion::*, mixin_source::*,
    mixinextras_handler::HandlerSignatureService,
    mixinextras_occurrence::InjectionPointOccurrenceResolver, *,
};
use std::collections::{BTreeMap, BTreeSet};

pub struct LocalCaptureExtractor;
impl LocalCaptureExtractor {
    pub fn extract(
        class_bytes: Option<&[u8]>,
        method_name: &str,
        method_descriptor: &str,
        instruction_occurrence_indices: &BTreeSet<i32>,
    ) -> Result<LocalCaptureResult, crate::expression_engine::Cancellation> {
        let cancellation = crate::expression_engine::current_cancellation();
        Self::extract_with_cancellation(
            class_bytes,
            method_name,
            method_descriptor,
            instruction_occurrence_indices,
            cancellation.as_ref(),
        )
    }
    pub fn extract_with_cancellation(
        class_bytes: Option<&[u8]>,
        method_name: &str,
        method_descriptor: &str,
        indices: &BTreeSet<i32>,
        cancellation: &dyn crate::expression_engine::CancellationChecker,
    ) -> Result<LocalCaptureResult, crate::expression_engine::Cancellation> {
        use crate::expression_engine::*;
        cancellation.check_cancelled()?;
        let failure = |error| Ok(LocalCaptureResult::Failure { error });
        let Some(bytes) = class_bytes else {
            return failure(LocalCaptureError::MissingClassBytes);
        };
        if bytes.is_empty() {
            return failure(LocalCaptureError::EmptyClassBytes);
        }
        let pool = IdentifierPool::default();
        let common = |left: &str, right: &str| (left == right).then(|| left.to_owned());
        let context = MatchContext {
            context_type: MatchContextType::Custom,
            identifier_pool: &pool,
            allow_incomplete_list_inputs: false,
            common_super_class: &common,
            cancellation,
        };
        let decoded = match decode_method(bytes, method_name, method_descriptor, &context)? {
            Ok(method) => method,
            Err(DecodeMethodError::CorruptClass { detail }) => {
                return failure(LocalCaptureError::CorruptClassBytes { detail })
            }
            Err(DecodeMethodError::MethodNotFound) => {
                return failure(LocalCaptureError::MethodNotFound {
                    method_name: method_name.into(),
                    method_descriptor: method_descriptor.into(),
                })
            }
        };
        cancellation.check_cancelled()?;
        let count = decoded.instructions.len() as i32;
        for &index in indices {
            cancellation.check_cancelled()?;
            if index < 0 || index >= count {
                return failure(LocalCaptureError::InvalidInstructionIndex {
                    instruction_occurrence_index: index,
                    instruction_count: count,
                });
            }
        }
        if indices.is_empty() {
            return Ok(LocalCaptureResult::Success { snapshots: vec![] });
        }
        let flow = match analyze_method_with_policy(
            bytes,
            method_name,
            method_descriptor,
            &context,
            AnalysisPolicy::ExactLocalCapture,
        )? {
            Ok(flow) => flow,
            Err(detail) => return failure(LocalCaptureError::Analysis { detail }),
        };
        let mut declared = BTreeMap::new();
        let mut slot = if flow.is_static {
            0
        } else {
            declared.insert(0, format!("L{};", flow.class_name));
            1
        };
        let mut arguments = BTreeSet::new();
        if let Ok(descriptor) = crate::descriptor::parse_method_descriptor(method_descriptor) {
            for ty in descriptor.parameters {
                let descriptor = crate::descriptor::to_descriptor(&ty);
                arguments.insert(slot);
                let width = if matches!(descriptor.as_str(), "J" | "D") {
                    2
                } else {
                    1
                };
                declared.insert(slot, descriptor);
                slot += width
            }
        }
        let mut snapshots = vec![];
        for &index in indices {
            cancellation.check_cancelled()?;
            let Some(frame) = flow.frames.get(index as usize).and_then(Option::as_ref) else {
                return failure(LocalCaptureError::Analysis {
                    detail: format!("missing frame for instruction occurrence index {index}"),
                });
            };
            let instruction = &flow.instructions[index as usize];
            let mut candidates = vec![];
            let mut ordinals = BTreeMap::<String, i32>::new();
            let mut slot = 0;
            while slot < frame.locals.len() {
                cancellation.check_cancelled()?;
                let node = frame.locals[slot];
                let Some(value) = flow.graph.nodes.get(node) else {
                    slot += 1;
                    continue;
                };
                let ty = value.ty.descriptor();
                if ty == "V" {
                    slot += 1;
                    continue;
                }
                let width = value.ty.size();
                let local = flow.local_variables.iter().find(|l| {
                    l.slot == slot
                        && l.start <= instruction.byte_offset
                        && instruction.byte_offset < l.end
                });
                let descriptor = local
                    .map(|l| l.descriptor.clone())
                    .or_else(|| declared.get(&slot).cloned())
                    .unwrap_or_else(|| {
                        if ty == "Lint-like;" {
                            "I".into()
                        } else {
                            ty.into()
                        }
                    });
                let ordinal = ordinals.entry(descriptor.clone()).or_default();
                let selected_ordinal = *ordinal;
                *ordinal += 1;
                if flow.is_static || slot != 0 {
                    candidates.push(LocalCaptureCandidate {
                        slot_index: slot as i32,
                        descriptor,
                        name: local.map(|l| l.name.clone()),
                        is_argument: arguments.contains(&slot),
                        ordinal: Some(selected_ordinal),
                    })
                }
                slot += width
            }
            snapshots.push(LocalCaptureSnapshot {
                instruction_occurrence_index: index,
                candidates,
            })
        }
        Ok(LocalCaptureResult::Success { snapshots })
    }
}

pub struct LocalDiscriminatorResolver;
impl LocalDiscriminatorResolver {
    pub fn resolve(
        spec: &LocalSugarSpec,
        target_descriptor: &str,
        candidates: &[LocalCaptureCandidate],
    ) -> LocalDiscriminatorResolution {
        let mut filtered = candidates
            .iter()
            .filter(|c| c.descriptor == target_descriptor && (!spec.args_only || c.is_argument))
            .cloned()
            .collect::<Vec<_>>();
        filtered.sort_by_key(|c| c.slot_index);
        if let Some(ordinal) = spec.ordinal.filter(|v| *v >= 0) {
            let candidate = filtered
                .iter()
                .find(|c| c.ordinal == Some(ordinal))
                .or_else(|| {
                    filtered
                        .iter()
                        .all(|c| c.ordinal.is_none())
                        .then(|| filtered.get(ordinal as usize))
                        .flatten()
                });
            return candidate.cloned().map_or(
                LocalDiscriminatorResolution::NotFound,
                LocalDiscriminatorResolution::Resolved,
            );
        }
        if let Some(index) = spec.index.filter(|v| *v >= 0) {
            filtered.retain(|c| c.slot_index == index);
        } else if !spec.names.is_empty() {
            return filtered
                .into_iter()
                .find(|c| {
                    c.name
                        .as_ref()
                        .is_some_and(|name| spec.names.contains(name))
                })
                .map_or(
                    LocalDiscriminatorResolution::NotFound,
                    LocalDiscriminatorResolution::Resolved,
                );
        }
        match filtered.len() {
            0 => LocalDiscriminatorResolution::NotFound,
            1 => LocalDiscriminatorResolution::Resolved(filtered.remove(0)),
            _ => LocalDiscriminatorResolution::Ambiguous(filtered),
        }
    }
}
const REF_PREFIX: &str = "com/llamalad7/mixinextras/sugar/ref/";
pub struct LocalCaptureTypeResolver<'a> {
    pub class_index: &'a dyn ClassIndex,
}
impl<'a> LocalCaptureTypeResolver<'a> {
    pub fn new(class_index: &'a dyn ClassIndex) -> Self {
        Self { class_index }
    }
    pub fn resolve_parameter(
        &self,
        source: &str,
        parameter: &HandlerParameterDeclaration,
    ) -> Option<String> {
        if let Some(descriptor) = parameter.type_descriptor.as_deref() {
            crate::descriptor::parse_field_descriptor(descriptor).ok()?;
            if object_internal(descriptor) == Some("com/llamalad7/mixinextras/sugar/ref/LocalRef") {
                if !parameter.type_name.contains('<') {
                    return None;
                }
            } else {
                return Some(
                    HandlerSignatureService::share_ref_value_descriptor(descriptor)
                        .unwrap_or(descriptor)
                        .into(),
                );
            }
        }
        let imports = JavaTypeDescriptorResolver::imports_for(source);
        let lookup = ClassIndexJavaTypeLookup {
            class_index: self.class_index,
        };
        let ctx = JavaTypeResolutionContext {
            imports: &imports,
            lookup: Some(&lookup),
        };
        let ty = &parameter.type_name;
        if let Some(start) = ty.find('<') {
            let base = ty[..start].trim();
            let descriptor = JavaTypeDescriptorResolver::descriptor_or_null(base, &ctx)?;
            if object_internal(&descriptor) == Some("com/llamalad7/mixinextras/sugar/ref/LocalRef")
            {
                return Self::unwrap_local_ref(ty, &ctx);
            }
            if base.rsplit('.').next()?.trim() == "LocalRef" {
                return None;
            }
        }
        let outer = JavaTypeDescriptorResolver::descriptor_or_null(ty, &ctx)?;
        let Some(name) = object_internal(&outer) else {
            return Some(outer);
        };
        if let Some(simple) = name.strip_prefix(REF_PREFIX) {
            let descriptor = match simple {
                "LocalBooleanRef" => Some("Z"),
                "LocalByteRef" => Some("B"),
                "LocalCharRef" => Some("C"),
                "LocalShortRef" => Some("S"),
                "LocalIntRef" => Some("I"),
                "LocalLongRef" => Some("J"),
                "LocalFloatRef" => Some("F"),
                "LocalDoubleRef" => Some("D"),
                _ => None,
            };
            if let Some(descriptor) = descriptor {
                return Some(descriptor.into());
            }
            if simple == "LocalRef" {
                return Self::unwrap_local_ref(ty, &ctx);
            }
        }
        Some(outer)
    }
    pub fn resolve_definition(&self, source: &str, spec: &LocalSugarSpec) -> Option<String> {
        let name = spec.type_class_name.as_deref()?;
        if name == "void" {
            return None;
        }
        let imports = JavaTypeDescriptorResolver::imports_for(source);
        let lookup = ClassIndexJavaTypeLookup {
            class_index: self.class_index,
        };
        let ctx = JavaTypeResolutionContext {
            imports: &imports,
            lookup: Some(&lookup),
        };
        JavaTypeDescriptorResolver::descriptor_or_null(name, &ctx).filter(|v| v != "V")
    }
    fn unwrap_local_ref(ty: &str, ctx: &JavaTypeResolutionContext<'_>) -> Option<String> {
        let ty = ty.trim();
        let start = ty.find('<')?;
        let base = JavaTypeDescriptorResolver::descriptor_or_null(ty[..start].trim(), ctx)?;
        if object_internal(&base) != Some("com/llamalad7/mixinextras/sugar/ref/LocalRef") {
            return None;
        }
        let mut depth = 0;
        let mut end = None;
        for (i, c) in ty.char_indices().skip_while(|(i, _)| *i < start) {
            match c {
                '<' => depth += 1,
                '>' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let end = end?;
        if !ty[end + 1..].trim().is_empty() {
            return None;
        }
        let content = ty[start + 1..end].trim();
        let mut depth = 0;
        let mut parts = vec![];
        let mut begin = 0;
        for (i, c) in content.char_indices() {
            match c {
                '<' => depth += 1,
                '>' => depth = (depth - 1).max(0),
                ',' if depth == 0 => {
                    let part = content[begin..i].trim();
                    if !part.is_empty() {
                        parts.push(part)
                    }
                    begin = i + 1
                }
                _ => {}
            }
        }
        let last = content[begin..].trim();
        if !last.is_empty() {
            parts.push(last)
        }
        if parts.len() != 1 || parts[0].contains('?') {
            return None;
        }
        let arg = parts[0].split('<').next()?.trim();
        if arg.is_empty() {
            return None;
        }
        JavaTypeDescriptorResolver::descriptor_or_null(arg, ctx)
    }
}
fn object_internal(descriptor: &str) -> Option<&str> {
    descriptor.strip_prefix('L')?.strip_suffix(';')
}

pub struct LocalCompletionService<'a> {
    pub class_index: &'a dyn ClassIndex,
}
impl<'a> LocalCompletionService<'a> {
    pub fn new(class_index: &'a dyn ClassIndex) -> Self {
        Self { class_index }
    }
    pub fn complete(
        &self,
        source: &str,
        parameter: &HandlerParameterDeclaration,
        results: &[LocalCaptureValidationResult],
        attribute_name: &str,
        partial_prefix: &str,
    ) -> Vec<McCompletionItem> {
        if results.is_empty()
            || results.iter().any(|r| {
                matches!(r, LocalCaptureValidationResult::Unavailable { .. })
                    || r.snapshots().is_empty()
            })
        {
            return vec![];
        }
        let Some(HandlerParameterSugarSpec::Local(spec)) = &parameter.sugar_spec else {
            return vec![];
        };
        let Some(target) =
            LocalCaptureTypeResolver::new(self.class_index).resolve_parameter(source, parameter)
        else {
            return vec![];
        };
        let snapshots = results
            .iter()
            .flat_map(|r| r.snapshots())
            .collect::<Vec<_>>();
        let mut filtered = snapshots[0]
            .candidates
            .iter()
            .filter(|c| c.descriptor == target && (!spec.args_only || c.is_argument))
            .collect::<Vec<_>>();
        filtered.sort_by_key(|c| c.slot_index);
        let mut values = match attribute_name {
            "ordinal" => (0..filtered.len())
                .map(|v| v.to_string())
                .collect::<Vec<_>>(),
            "index" => filtered.iter().map(|c| c.slot_index.to_string()).collect(),
            "name" => filtered.iter().filter_map(|c| c.name.clone()).collect(),
            _ => return vec![],
        };
        let mut seen = BTreeSet::new();
        values.retain(|v| seen.insert(v.clone()));
        values.retain(|value| {
            let mut probe = spec.clone();
            probe.ordinal = None;
            probe.index = None;
            probe.names.clear();
            match attribute_name {
                "ordinal" => probe.ordinal = value.parse().ok(),
                "index" => probe.index = value.parse().ok(),
                _ => {
                    probe.names.insert(value.clone());
                }
            }
            snapshots.iter().all(|s| {
                matches!(
                    LocalDiscriminatorResolver::resolve(&probe, &target, &s.candidates),
                    LocalDiscriminatorResolution::Resolved(_)
                )
            }) && if attribute_name == "name" {
                value
                    .to_lowercase()
                    .starts_with(&partial_prefix.to_lowercase())
            } else {
                value.starts_with(partial_prefix)
            }
        });
        if attribute_name == "name" {
            values.sort_by_key(|v| v.to_lowercase())
        } else {
            values.sort_by_key(|v| v.parse::<i32>().unwrap())
        }
        values
            .into_iter()
            .enumerate()
            .map(|(i, value)| {
                let mut item = item(
                    value.clone(),
                    value.clone(),
                    McCompletionKind::Value,
                    format!(
                        "{}_{i:04}_{value}",
                        if attribute_name == "name" {
                            "0100"
                        } else {
                            "0200"
                        }
                    ),
                    "mixinextras.local",
                );
                item.detail = Some(format!("@Local {attribute_name}"));
                item.metadata.name = Some(attribute_name.into());
                item
            })
            .collect()
    }
}

pub struct MixinExtrasCompletionService<'a> {
    pub class_index: &'a dyn ClassIndex,
    pub bytecode_index: &'a dyn BytecodeIndex,
}
impl<'a> MixinExtrasCompletionService<'a> {
    pub fn new(class_index: &'a dyn ClassIndex, bytecode_index: &'a dyn BytecodeIndex) -> Self {
        Self {
            class_index,
            bytecode_index,
        }
    }
    pub fn complete(
        &self,
        context: &AnnotationContext,
        options: &MixinCompletionOptions,
    ) -> Vec<McCompletionItem> {
        if matches!(
            context.annotation,
            MixinAnnotation::ModifyExpressionValue
                | MixinAnnotation::ModifyReturnValue
                | MixinAnnotation::ModifyReceiver
                | MixinAnnotation::WrapOperation
                | MixinAnnotation::WrapWithCondition
                | MixinAnnotation::WrapMethod
        ) && context.slot == AnnotationSlot::Method
        {
            return InjectMethodCompletionService::new(self.class_index)
                .complete(context, options)
                .into_iter()
                .map(|mut item| {
                    item.metadata.source = "mixinextras.injectMethod".into();
                    item
                })
                .collect();
        }
        if context.annotation == MixinAnnotation::At
            && context.slot == AnnotationSlot::Target
            && context.at_value.as_deref() == Some("INVOKE")
            && (!context.partial_value.is_empty() || context.inject_method_name.is_some())
        {
            if let (Some(owner), Some(method)) = (
                context.mixin_target_internal_names.first(),
                &context.inject_method_name,
            ) {
                let candidates = self
                    .bytecode_index
                    .get_at_target_candidates(owner, method, None, "INVOKE");
                return AtTargetCompletionService::complete(context, &candidates);
            }
        }
        vec![]
    }
    pub fn complete_at_target_for_wrap_operation(
        &self,
        mixin_targets: &[String],
        inject_method_name: &str,
        inject_method_descriptor: Option<&str>,
        context: &AnnotationContext,
    ) -> Vec<McCompletionItem> {
        let Some(owner) = mixin_targets.first() else {
            return vec![];
        };
        let candidates = self.bytecode_index.get_at_target_candidates(
            owner,
            inject_method_name,
            inject_method_descriptor,
            "INVOKE",
        );
        AtTargetCompletionService::complete(context, &candidates)
    }
}

pub struct LocalCaptureValidationService<'a> {
    pub class_index: &'a dyn ClassIndex,
    pub bytecode_index: &'a dyn BytecodeIndex,
    pub signature_service: HandlerSignatureService<'a>,
}
impl<'a> LocalCaptureValidationService<'a> {
    pub fn new(class_index: &'a dyn ClassIndex, bytecode_index: &'a dyn BytecodeIndex) -> Self {
        Self {
            class_index,
            bytecode_index,
            signature_service: HandlerSignatureService::new(class_index, Some(bytecode_index)),
        }
    }
    pub fn with_signature_service(mut self, service: HandlerSignatureService<'a>) -> Self {
        self.signature_service = service;
        self
    }
    pub fn validate(
        &self,
        source: &str,
        parameter: &HandlerParameterDeclaration,
        points: &[LocalCaptureValidationPoint],
    ) -> Result<Vec<LocalCaptureValidationResult>, crate::expression_engine::Cancellation> {
        let cancellation = crate::expression_engine::current_cancellation();
        cancellation.check_cancelled()?;
        let Some(HandlerParameterSugarSpec::Local(spec)) = &parameter.sugar_spec else {
            return Ok(points
                .iter()
                .cloned()
                .map(|point| LocalCaptureValidationResult::Unavailable {
                    point,
                    snapshots: vec![],
                })
                .collect());
        };
        let Some(descriptor) =
            LocalCaptureTypeResolver::new(self.class_index).resolve_parameter(source, parameter)
        else {
            return Ok(points
                .iter()
                .cloned()
                .map(|point| LocalCaptureValidationResult::Unavailable {
                    point,
                    snapshots: vec![],
                })
                .collect());
        };
        let mut results = vec![];
        for point in points {
            cancellation.check_cancelled()?;
            let unavailable = || LocalCaptureValidationResult::Unavailable {
                point: point.clone(),
                snapshots: vec![],
            };
            let Some(bytes) = self
                .bytecode_index
                .get_class_bytes(&point.owner)
                .filter(|b| !b.is_empty())
            else {
                results.push(unavailable());
                continue;
            };
            let mut site = point.site.clone();
            site.handler_method = site.handler_method.as_ref().map(|handler| {
                self.signature_service
                    .enrich_declared_handler(handler, source)
            });
            let Some(indices) = InjectionPointOccurrenceResolver::new(self.bytecode_index)
                .resolve_in_slice(
                    &point.owner,
                    &point.target_method,
                    &site,
                    point.expression_instruction_indices.as_ref(),
                    self.signature_service.selected_slice(source, &point.site),
                )
            else {
                results.push(unavailable());
                continue;
            };
            if indices.is_empty() {
                results.push(LocalCaptureValidationResult::NotFound {
                    point: point.clone(),
                    snapshots: vec![],
                });
                continue;
            }
            let extraction = LocalCaptureExtractor::extract_with_cancellation(
                Some(&bytes),
                &point.target_method.name,
                &point.target_method.descriptor,
                &indices,
                cancellation.as_ref(),
            )?;
            let LocalCaptureResult::Success { snapshots } = extraction else {
                results.push(unavailable());
                continue;
            };
            let resolutions = snapshots
                .iter()
                .map(|snapshot| {
                    LocalDiscriminatorResolver::resolve(spec, &descriptor, &snapshot.candidates)
                })
                .collect::<Vec<_>>();
            let result = if resolutions
                .iter()
                .any(|r| matches!(r, LocalDiscriminatorResolution::Ambiguous(_)))
            {
                LocalCaptureValidationResult::Ambiguous {
                    point: point.clone(),
                    snapshots,
                }
            } else if snapshots.is_empty()
                || resolutions
                    .iter()
                    .any(|r| matches!(r, LocalDiscriminatorResolution::NotFound))
            {
                LocalCaptureValidationResult::NotFound {
                    point: point.clone(),
                    snapshots,
                }
            } else {
                LocalCaptureValidationResult::Resolved {
                    point: point.clone(),
                    snapshots,
                    candidates: resolutions
                        .into_iter()
                        .map(|r| {
                            if let LocalDiscriminatorResolution::Resolved(c) = r {
                                c
                            } else {
                                unreachable!()
                            }
                        })
                        .collect(),
                }
            };
            results.push(result)
        }
        Ok(results)
    }
}

pub struct ShareCompletionService<'a> {
    pub class_index: &'a dyn ClassIndex,
    host_source: Option<HostHandlerSource>,
}
impl<'a> ShareCompletionService<'a> {
    pub fn new(class_index: &'a dyn ClassIndex) -> Self {
        Self {
            class_index,
            host_source: None,
        }
    }
    pub fn with_host_source(mut self, source: Option<HostHandlerSource>) -> Self {
        self.host_source = source;
        self
    }
    pub fn has_host_source(&self) -> bool {
        self.host_source.is_some()
    }
    pub fn complete_with_normalized_sources(
        &self,
        source: &str,
        context: &AnnotationContext,
        additional_sources: impl IntoIterator<Item = HostShareSource>,
    ) -> Vec<McCompletionItem> {
        if context.annotation != MixinAnnotation::Share
            || context.slot != AnnotationSlot::Value
            || context.attribute_name.as_deref() == Some("namespace")
        {
            return vec![];
        }
        let host = self.host_source.as_ref().unwrap_or_else(|| {
            std::panic::panic_any(crate::host_source::HostSourceError::Failure(
                "current normalized Share source is unavailable".into(),
            ))
        });
        let sites = host
            .checked_sites(source)
            .unwrap_or_else(|error| std::panic::panic_any(error));
        self.complete_normalized(source, context, host, sites, additional_sources)
    }
    pub fn complete<I, S>(
        &self,
        source: &str,
        context: &AnnotationContext,
        additional_sources: I,
    ) -> Vec<McCompletionItem>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        if context.annotation != MixinAnnotation::Share
            || context.slot != AnnotationSlot::Value
            || context.attribute_name.as_deref() == Some("namespace")
        {
            return vec![];
        }
        if let Some(host) = &self.host_source {
            let sites = host
                .checked_sites(source)
                .unwrap_or_else(|error| std::panic::panic_any(error));
            return self.complete_normalized(
                source,
                context,
                host,
                sites,
                additional_sources.into_iter().map(|_| -> HostShareSource {
                    std::panic::panic_any(crate::host_source::HostSourceError::Failure(
                        "normalized Share declarations for additional sources are unavailable"
                            .into(),
                    ))
                }),
            );
        }
        if AnnotationContextExtractor::resolve_mixin_class_scope(
            source,
            context.annotation_start_offset,
        )
        .is_none()
        {
            return vec![];
        }
        let sites = HandlerSignatureService::find_sugar_handler_annotation_sites(source);
        let imports = JavaTypeDescriptorResolver::imports_for(source);
        let mut current = BTreeSet::new();
        for site in &sites {
            let Some(handler) = &site.handler_method else {
                continue;
            };
            let Some(scope) = site_scope(site, source) else {
                continue;
            };
            let enriched = HandlerSignatureService::enrich_handler_types(
                handler,
                self.class_index,
                Some(source),
            );
            let Some(parameter) = enriched.parameters.iter().find(|p| {
                matches!(p.sugar_spec, Some(HandlerParameterSugarSpec::Share(_)))
                    && contains(
                        source,
                        p.sugar_annotation_range,
                        context.annotation_start_offset,
                    )
            }) else {
                continue;
            };
            let Some(HandlerParameterSugarSpec::Share(share)) = &parameter.sugar_spec else {
                continue;
            };
            let Some(namespace) = effective_namespace(share, &scope.class_name) else {
                continue;
            };
            let Some(descriptor) = parameter
                .type_descriptor
                .as_deref()
                .and_then(HandlerSignatureService::share_ref_value_descriptor)
            else {
                continue;
            };
            for target in self.targets(site, &scope, &imports) {
                current.insert((
                    target.0,
                    target.1,
                    target.2,
                    namespace.to_owned(),
                    descriptor.to_owned(),
                ));
            }
        }
        if current.is_empty() {
            return vec![];
        }
        let mut values = BTreeSet::new();
        let mut collect = |candidate_source: &str, is_current: bool| {
            let candidate_sites = if is_current {
                sites.clone()
            } else {
                HandlerSignatureService::find_sugar_handler_annotation_sites(candidate_source)
            };
            let candidate_imports = JavaTypeDescriptorResolver::imports_for(candidate_source);
            for site in candidate_sites {
                let Some(handler) = &site.handler_method else {
                    continue;
                };
                let Some(scope) = site_scope(&site, candidate_source) else {
                    continue;
                };
                let targets = self.targets(&site, &scope, &candidate_imports);
                if targets.is_empty() {
                    continue;
                }
                let enriched = HandlerSignatureService::enrich_handler_types(
                    handler,
                    self.class_index,
                    Some(candidate_source),
                );
                for parameter in enriched.parameters {
                    let Some(HandlerParameterSugarSpec::Share(share)) = &parameter.sugar_spec
                    else {
                        continue;
                    };
                    let Some(value) = share.value.as_ref().filter(|v| !v.is_empty()) else {
                        continue;
                    };
                    if is_current
                        && contains(
                            candidate_source,
                            parameter.sugar_annotation_range,
                            context.annotation_start_offset,
                        )
                    {
                        continue;
                    }
                    let Some(namespace) = effective_namespace(share, &scope.class_name) else {
                        continue;
                    };
                    let Some(descriptor) = parameter
                        .type_descriptor
                        .as_deref()
                        .and_then(HandlerSignatureService::share_ref_value_descriptor)
                    else {
                        continue;
                    };
                    if targets.iter().any(|t| {
                        current.contains(&(
                            t.0.clone(),
                            t.1.clone(),
                            t.2.clone(),
                            namespace.to_owned(),
                            descriptor.to_owned(),
                        ))
                    }) {
                        values.insert(value.clone());
                    }
                }
            }
        };
        collect(source, true);
        for additional in additional_sources {
            collect(additional.as_ref(), false)
        }
        values
            .into_iter()
            .filter_map(|value| {
                let escaped = escape_java_string(&value);
                escaped
                    .starts_with(&context.partial_value)
                    .then_some((value, escaped))
            })
            .enumerate()
            .map(|(i, (value, escaped))| {
                let mut item = item(
                    value.clone(),
                    escaped.clone(),
                    McCompletionKind::Value,
                    format!("0200_{i:04}_{value}"),
                    "mixinextras.share",
                );
                item.detail = Some("@Share".into());
                item.metadata.name = Some(value);
                item
            })
            .collect()
    }
    fn targets(
        &self,
        site: &MixinExtrasAnnotationSite,
        scope: &MixinClassScope,
        imports: &JavaSourceImports,
    ) -> BTreeSet<(String, String, String)> {
        let targets = MixinTargetResolver::resolve_targets(
            &scope.raw_targets,
            self.class_index,
            Some(imports),
        );
        self.target_keys(site, &targets)
    }
    fn target_keys(
        &self,
        site: &MixinExtrasAnnotationSite,
        targets: &[String],
    ) -> BTreeSet<(String, String, String)> {
        let Some(resolved) = HandlerSignatureService::new(self.class_index, None)
            .resolve_target_method(&targets, &site.method_attribute)
        else {
            return BTreeSet::new();
        };
        targets
            .iter()
            .cloned()
            .flat_map(|owner| {
                self.class_index
                    .get_methods(&owner)
                    .into_iter()
                    .filter(|m| m.name == resolved.name && m.descriptor == resolved.descriptor)
                    .map(move |m| (owner.clone(), m.name, m.descriptor))
            })
            .collect()
    }
    fn complete_normalized<I>(
        &self,
        source: &str,
        context: &AnnotationContext,
        host: &HostHandlerSource,
        sites: &[MixinExtrasAnnotationSite],
        additional_sources: I,
    ) -> Vec<McCompletionItem>
    where
        I: IntoIterator<Item = HostShareSource>,
    {
        let mut current = BTreeSet::new();
        for site in sites {
            let Some(handler) = &site.handler_method else {
                continue;
            };
            for parameter in &handler.parameters {
                let Some(HandlerParameterSugarSpec::Share(share)) = &parameter.sugar_spec else {
                    continue;
                };
                if !contains(
                    source,
                    parameter.sugar_annotation_range,
                    context.annotation_start_offset,
                ) {
                    continue;
                }
                let Some(namespace) = effective_namespace(share, &host.qualified_name) else {
                    continue;
                };
                let Some(descriptor) = parameter
                    .type_descriptor
                    .as_deref()
                    .and_then(HandlerSignatureService::share_ref_value_descriptor)
                else {
                    continue;
                };
                let targets = self.target_keys(site, &host.targets);
                if targets.is_empty() {
                    std::panic::panic_any(crate::host_source::HostSourceError::Failure(
                        "normalized Share target method facts are unavailable".into(),
                    ))
                }
                for target in targets {
                    current.insert((
                        target.0,
                        target.1,
                        target.2,
                        namespace.to_owned(),
                        descriptor.to_owned(),
                    ));
                }
            }
        }
        if current.is_empty() {
            return vec![];
        }
        let mut values = BTreeSet::new();
        let mut collect = |candidate: &HostHandlerSource,
                           text: &str,
                           sites: &[MixinExtrasAnnotationSite],
                           is_current: bool| {
            for site in sites {
                let Some(handler) = &site.handler_method else {
                    continue;
                };
                for parameter in &handler.parameters {
                    let Some(HandlerParameterSugarSpec::Share(share)) = &parameter.sugar_spec
                    else {
                        continue;
                    };
                    if is_current
                        && contains(
                            text,
                            parameter.sugar_annotation_range,
                            context.annotation_start_offset,
                        )
                    {
                        continue;
                    }
                    let Some(value) = share.value.as_ref().filter(|value| !value.is_empty()) else {
                        continue;
                    };
                    let Some(namespace) = effective_namespace(share, &candidate.qualified_name)
                    else {
                        continue;
                    };
                    let Some(descriptor) = parameter
                        .type_descriptor
                        .as_deref()
                        .and_then(HandlerSignatureService::share_ref_value_descriptor)
                    else {
                        continue;
                    };
                    let targets = self.target_keys(site, &candidate.targets);
                    if targets.is_empty() {
                        std::panic::panic_any(crate::host_source::HostSourceError::Failure(
                            "normalized Share target method facts are unavailable".into(),
                        ))
                    }
                    if targets.iter().any(|target| {
                        current.contains(&(
                            target.0.clone(),
                            target.1.clone(),
                            target.2.clone(),
                            namespace.into(),
                            descriptor.into(),
                        ))
                    }) {
                        values.insert(value.clone());
                    }
                }
            }
        };
        collect(host, source, sites, true);
        let mut seen =
            std::collections::BTreeMap::from([(host.document_uri.clone(), host.sha256.clone())]);
        for candidate in additional_sources {
            let sites = candidate
                .handlers
                .checked_sites(&candidate.text)
                .unwrap_or_else(|error| std::panic::panic_any(error));
            match seen.get(&candidate.handlers.document_uri) {
                Some(sha) if sha == &candidate.handlers.sha256 => continue,
                Some(_) => std::panic::panic_any(crate::host_source::HostSourceError::Failure(
                    "normalized Share source URI has a changed digest".into(),
                )),
                None => {
                    seen.insert(
                        candidate.handlers.document_uri.clone(),
                        candidate.handlers.sha256.clone(),
                    );
                }
            }
            collect(&candidate.handlers, &candidate.text, sites, false);
        }
        values
            .into_iter()
            .filter_map(|value| {
                let escaped = escape_java_string(&value);
                escaped
                    .starts_with(&context.partial_value)
                    .then_some((value, escaped))
            })
            .enumerate()
            .map(|(index, (value, escaped))| {
                let mut result = item(
                    value.clone(),
                    escaped,
                    McCompletionKind::Value,
                    format!("0200_{index:04}_{value}"),
                    "mixinextras.share",
                );
                result.detail = Some("@Share".into());
                result.metadata.name = Some(value);
                result
            })
            .collect()
    }
}
fn effective_namespace<'a>(share: &'a ShareSugarSpec, default: &'a str) -> Option<&'a str> {
    if share.namespace_specified {
        share.namespace.as_deref()
    } else {
        (!default.trim().is_empty()).then_some(default)
    }
}
fn offset(source: &str, p: McTextPosition) -> Option<usize> {
    crate::position_to_offset(source, p)
}
fn site_scope(site: &MixinExtrasAnnotationSite, source: &str) -> Option<MixinClassScope> {
    AnnotationContextExtractor::resolve_mixin_class_scope(
        source,
        offset(source, site.annotation_range.start)?,
    )
}
fn contains(source: &str, range: Option<McTextRange>, at: usize) -> bool {
    range
        .and_then(|r| Some((offset(source, r.start)?, offset(source, r.end)?)))
        .is_some_and(|(a, b)| a <= at && at <= b)
}
fn escape_java_string(value: &str) -> String {
    let mut out = String::new();
    for c in value.chars() {
        out.push_str(match c {
            '\\' => "\\\\",
            '"' => "\\\"",
            '\u{0008}' => "\\b",
            '\u{000c}' => "\\f",
            '\n' => "\\n",
            '\r' => "\\r",
            '\t' => "\\t",
            _ => {
                out.push(c);
                continue;
            }
        })
    }
    out
}
