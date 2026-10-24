//! Shared Minecraft facade request/result facts, independent of a language host.
use crate::{
    mixin_source::{AnnotationContext, MixinClassModel, ParseConfidence, ParseSource},
    McCompletionItem, McCompletionReplacementRange,
};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MixinFacadeRequest {
    pub buffer_text: String,
    pub line: i32,
    pub character: i32,
    pub document_uri: String,
    pub mixin_class_name: Option<String>,
    pub mixin_package: Option<String>,
    pub mixin_config_content: Option<String>,
    pub mixin_config_path: Option<String>,
    pub semantic_model: Option<MixinClassModel>,
}
impl Default for MixinFacadeRequest {
    fn default() -> Self {
        Self {
            buffer_text: String::new(),
            line: 0,
            character: 0,
            document_uri: "file:///Mixin.java".into(),
            mixin_class_name: None,
            mixin_package: None,
            mixin_config_content: None,
            mixin_config_path: None,
            semantic_model: None,
        }
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McdevCompletionDebugInfo {
    pub command: String,
    pub document_uri: String,
    pub language_id: String,
    pub parse_source: Option<ParseSource>,
    pub parse_confidence: Option<ParseConfidence>,
    pub used_compilation_unit: bool,
    pub used_java_project: bool,
    pub binding_resolved_count: usize,
    pub binding_failed_count: usize,
    pub fallback_reason: Option<String>,
    pub semantic_context_found: bool,
    pub fallback_annotation_context_used: bool,
    pub fallback_annotation_context_reason: Option<String>,
    pub semantic_target_count: usize,
    pub semantic_member_count: usize,
    pub completion_context_kind: Option<String>,
    pub owner: Option<String>,
    pub method_name: Option<String>,
    pub method_descriptor: Option<String>,
    pub candidate_count_before_filter: usize,
    pub candidate_count_after_filter: usize,
    pub zero_item_reason: Option<String>,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MixinCompletionResult {
    pub items: Vec<McCompletionItem>,
    pub debug: McdevCompletionDebugInfo,
    pub replacement_range: Option<McCompletionReplacementRange>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BufferOnlyCompletionResult {
    pub result: MixinCompletionResult,
    pub context: Option<AnnotationContext>,
}

use crate::{
    bytecode::*,
    descriptor, expression_engine as engine,
    extras_model::*,
    mixin_actions::MixinCodeActionService,
    mixin_completion::*,
    mixin_diagnostics::{MixinDiagnosticRequest, MixinDiagnosticsService},
    mixin_source::*,
    mixin_validation::*,
    mixinextras_actions::MixinExtrasCodeActionService,
    mixinextras_diagnostics::{MixinExtrasDiagnosticRequest, MixinExtrasDiagnosticsService},
    mixinextras_expression::*,
    mixinextras_handler::{HandlerReferenceTypeResolver, HandlerSignatureService},
    mixinextras_locals::*,
    *,
};
use std::{collections::BTreeSet, sync::Arc};
pub type FacadeResult<T> = Result<T, engine::Cancellation>;
pub struct MixinServiceFacade<'a> {
    pub class_index: &'a dyn ClassIndex,
    pub bytecode_index: &'a dyn BytecodeIndex,
    pub at_value_completion: AtValueCompletionService<'a>,
    pub expression_members: ExpressionMemberCompletionService<'a>,
    pub reference_types: Option<&'a dyn HandlerReferenceTypeResolver>,
    share_sources: Box<dyn Fn() -> Box<dyn Iterator<Item = String> + 'a> + 'a>,
}
impl<'a> MixinServiceFacade<'a> {
    pub fn new(class_index: &'a dyn ClassIndex, bytecode_index: &'a dyn BytecodeIndex) -> Self {
        Self {
            class_index,
            bytecode_index,
            at_value_completion: AtValueCompletionService::default(),
            expression_members: ExpressionMemberCompletionService::new(class_index, bytecode_index),
            reference_types: None,
            share_sources: Box::new(|| Box::new(std::iter::empty())),
        }
    }
    pub fn with_at_value_completion(mut self, service: AtValueCompletionService<'a>) -> Self {
        self.at_value_completion = service;
        self
    }
    pub fn with_share_sources(
        mut self,
        provider: impl Fn() -> Box<dyn Iterator<Item = String> + 'a> + 'a,
    ) -> Self {
        self.share_sources = Box::new(provider);
        self
    }
    pub fn with_reference_types(
        mut self,
        reference_types: &'a dyn HandlerReferenceTypeResolver,
    ) -> Self {
        self.reference_types = Some(reference_types);
        self
    }
    pub fn with_expression_cache(mut self, cache: Arc<ExpressionMemberCompletionCache>) -> Self {
        self.expression_members.reachable_members_cache = cache;
        self
    }
    fn handler(&self) -> HandlerSignatureService<'a> {
        let handler = HandlerSignatureService::new(self.class_index, Some(self.bytecode_index));
        if let Some(reference_types) = self.reference_types {
            handler.with_reference_types(reference_types)
        } else {
            handler
        }
    }
    fn extras_actions(&self) -> MixinExtrasCodeActionService<'a> {
        MixinExtrasCodeActionService::new(self.class_index).with_signature_service(self.handler())
    }
    pub fn complete(
        &self,
        request: &MixinFacadeRequest,
        options: &MixinCompletionOptions,
    ) -> FacadeResult<Vec<McCompletionItem>> {
        Ok(self
            .complete_with_debug(request, options, "mcdev.completion", "java")?
            .items)
    }
    pub fn complete_with_debug(
        &self,
        request: &MixinFacadeRequest,
        options: &MixinCompletionOptions,
        command: &str,
        language_id: &str,
    ) -> FacadeResult<MixinCompletionResult> {
        core_call(|| self.complete_with_debug_inner(request, options, command, language_id))
    }
    fn complete_with_debug_inner(
        &self,
        request: &MixinFacadeRequest,
        options: &MixinCompletionOptions,
        command: &str,
        language_id: &str,
    ) -> FacadeResult<MixinCompletionResult> {
        let mut effective = request.clone();
        if effective.semantic_model.is_none() {
            effective.semantic_model = Some(MixinSemanticModelParser::parse(
                &request.buffer_text,
                &request.document_uri,
            ));
        }
        let position = position(&effective);
        let semantic = position.and_then(|p| {
            SemanticCompletionContextExtractor::extract(
                &effective.buffer_text,
                p.line,
                p.character,
                effective.semantic_model.as_ref(),
            )
        });
        let semantic_annotation = position.and_then(|p| {
            semantic.as_ref().and_then(|context| {
                SemanticCompletionContextExtractor::to_annotation_context(
                    &effective.buffer_text,
                    p.line,
                    p.character,
                    effective.semantic_model.as_ref(),
                    context,
                )
            })
        });
        let fallback = if semantic_annotation.is_none() {
            position
                .and_then(|p| position_to_offset(&effective.buffer_text, p))
                .and_then(|offset| {
                    AnnotationContextExtractor::extract_at_offset(&effective.buffer_text, offset)
                })
        } else {
            None
        };
        let context = semantic_annotation.as_ref().or(fallback.as_ref());
        let mut warnings = effective
            .semantic_model
            .as_ref()
            .map_or(vec![], |m| m.warnings.clone());
        let reason = if semantic_annotation.is_none() && fallback.is_some() {
            Some("semantic completion context was unavailable".to_string())
        } else {
            None
        };
        if let Some(reason) = &reason {
            warnings.push(format!("FALLBACK_ANNOTATION_CONTEXT_USED: {reason}"));
        }
        dedup(&mut warnings);
        let routed = if let Some(context) = context {
            self.route_completion(&effective, context, options)?
        } else {
            vec![]
        };
        let expression_value = context.is_some_and(|c| {
            matches!(
                c.annotation,
                MixinAnnotation::Expression | MixinAnnotation::Expressions
            ) && c.slot == AnnotationSlot::Value
        });
        let support =
            ExpressionSupport::new(Some(&self.expression_members), Some(self.class_index));
        let feature = if routed.is_empty() && !expression_value {
            position.map_or(FeatureAnnotationCompletion::default(), |p| {
                support.complete_feature_annotations_with_range(
                    &effective.buffer_text,
                    p.line,
                    p.character,
                )
            })
        } else {
            FeatureAnnotationCompletion::default()
        };
        let replacement_range = if routed.is_empty() {
            feature.replacement_range
        } else {
            None
        };
        let items = if routed.is_empty() {
            if expression_value {
                vec![]
            } else {
                feature.items
            }
        } else {
            routed
        };
        let debug = self.debug_info(
            &effective,
            command,
            language_id,
            context,
            semantic.as_ref(),
            fallback.is_some(),
            reason,
            &items,
            warnings,
        );
        Ok(MixinCompletionResult {
            items,
            debug,
            replacement_range,
        })
    }
    fn route_completion(
        &self,
        request: &MixinFacadeRequest,
        context: &AnnotationContext,
        options: &MixinCompletionOptions,
    ) -> FacadeResult<Vec<McCompletionItem>> {
        let source = &request.buffer_text;
        if context.slot == AnnotationSlot::Handler {
            let items = MixinDeclarationSnippetService::new(self.class_index).complete_at(
                source,
                context.annotation,
                context.annotation_start_offset,
                &self.resolve_targets(request, context),
                &context.partial_value,
            );
            if !items.is_empty() {
                return Ok(items
                    .into_iter()
                    .map(|i| normalize_snippet(source, context, i))
                    .collect());
            }
            return Ok(self.extras_actions().complete_handler(
                source,
                context.annotation_start_offset,
                &self.resolve_targets(request, context),
                resolved_contexts(request),
                Some(context.value_end_offset),
            ));
        }
        if context.slot == AnnotationSlot::Attribute {
            let mut items = MixinAttributeCompletionService::complete(context, source);
            if context.annotation == MixinAnnotation::Local {
                let sites = local_parameter_sites(source, context.value_end_offset);
                if sites
                    .iter()
                    .any(|(s, _)| s.annotation == MixinExtrasAnnotation::WrapMethod)
                {
                    return Ok(vec![]);
                }
                if !sites.is_empty() {
                    items.retain(|i| i.metadata.name.as_deref() != Some("type"));
                }
            }
            return Ok(items);
        }
        let mut resolved = context.clone();
        Ok(match context.annotation {
            MixinAnnotation::Mixin => MixinTargetCompletionService::new(self.class_index)
                .complete(context, options, source),
            a if matches!(
                a,
                MixinAnnotation::Inject
                    | MixinAnnotation::Redirect
                    | MixinAnnotation::ModifyArg
                    | MixinAnnotation::ModifyArgs
                    | MixinAnnotation::ModifyVariable
                    | MixinAnnotation::ModifyConstant
            ) =>
            {
                resolved.mixin_target_internal_names = self.resolve_targets(request, context);
                InjectMethodCompletionService::new(self.class_index).complete(&resolved, options)
            }
            a if matches!(
                a,
                MixinAnnotation::ModifyExpressionValue
                    | MixinAnnotation::ModifyReturnValue
                    | MixinAnnotation::ModifyReceiver
                    | MixinAnnotation::WrapOperation
                    | MixinAnnotation::WrapWithCondition
                    | MixinAnnotation::WrapMethod
            ) =>
            {
                resolved.mixin_target_internal_names = self.resolve_targets(request, context);
                let items =
                    MixinExtrasCompletionService::new(self.class_index, self.bytecode_index)
                        .complete(&resolved, options);
                if items.is_empty() && a == MixinAnnotation::ModifyReceiver {
                    InjectMethodCompletionService::new(self.class_index)
                        .complete(&resolved, options)
                } else {
                    items
                }
            }
            MixinAnnotation::At => match context.slot {
                AnnotationSlot::Value => {
                    let core = self.at_value_completion.complete(context);
                    let extras = ExpressionSupport::default().complete_at_value(context);
                    let mut merged = vec![];
                    for item in core {
                        let preferred = extras
                            .iter()
                            .find(|e| e.insert_text == item.insert_text)
                            .cloned()
                            .unwrap_or(item);
                        if !merged
                            .iter()
                            .any(|i: &McCompletionItem| i.insert_text == preferred.insert_text)
                        {
                            merged.push(preferred);
                        }
                    }
                    for item in extras {
                        if !merged.iter().any(|i| i.insert_text == item.insert_text) {
                            merged.push(item);
                        }
                    }
                    merged
                }
                AnnotationSlot::Target => self.complete_at_target(request, context),
                _ => vec![],
            },
            MixinAnnotation::Accessor if context.slot == AnnotationSlot::AccessorValue => {
                AccessorService::new(self.class_index).complete_fields(
                    &self.resolve_targets(request, context),
                    context.partial_value.trim_matches('"'),
                )
            }
            MixinAnnotation::Invoker if context.slot == AnnotationSlot::InvokerValue => {
                InvokerService::new(self.class_index).complete_methods(
                    &self.resolve_targets(request, context),
                    context.partial_value.trim_matches('"'),
                )
            }
            MixinAnnotation::Shadow => self.complete_shadow(request, context),
            MixinAnnotation::Overwrite => self.complete_overwrite(request, context),
            MixinAnnotation::Local if context.slot == AnnotationSlot::Value => {
                self.complete_local(request, context)?
            }
            MixinAnnotation::Share
                if context.slot == AnnotationSlot::Value
                    && context.attribute_name.as_deref() != Some("namespace") =>
            {
                ShareCompletionService::new(self.class_index).complete(
                    source,
                    context,
                    (self.share_sources)(),
                )
            }
            MixinAnnotation::Expression | MixinAnnotation::Expressions
                if context.slot == AnnotationSlot::Value =>
            {
                ExpressionSupport::new(Some(&self.expression_members), Some(self.class_index))
                    .complete_expression_value(
                        context,
                        source,
                        resolved_contexts(request),
                        &self.resolve_targets(request, context),
                        engine::current_cancellation().as_ref(),
                    )?
            }
            _ => vec![],
        })
    }
    fn resolve_targets(
        &self,
        request: &MixinFacadeRequest,
        context: &AnnotationContext,
    ) -> Vec<String> {
        let source = &request.buffer_text;
        let imports = JavaTypeDescriptorResolver::imports_for(source);
        if imports.is_kotlin {
            return MixinTargetResolver::resolve_targets(
                &AnnotationContextExtractor::resolve_raw_mixin_targets(
                    source,
                    context.value_start_offset,
                ),
                self.class_index,
                Some(&imports),
            );
        }
        if let Some(model) = &request.semantic_model {
            let targets: Vec<_> = model
                .targets
                .iter()
                .filter_map(|t| {
                    MixinTargetResolver::resolve_target(
                        &t.internal_name,
                        self.class_index,
                        Some(&imports),
                    )
                })
                .collect();
            if !targets.is_empty() {
                return targets;
            }
        }
        let raw = if context.mixin_target_internal_names.is_empty() {
            AnnotationContextExtractor::resolve_raw_mixin_targets(
                source,
                context.value_start_offset,
            )
        } else {
            context.mixin_target_internal_names.clone()
        };
        MixinTargetResolver::resolve_targets(&raw, self.class_index, Some(&imports))
    }
    fn targets_at(&self, source: &str, position: McTextPosition) -> Vec<String> {
        let imports = JavaTypeDescriptorResolver::imports_for(source);
        if !imports.is_kotlin {
            return MixinTargetResolver::resolve_targets_from_source(source, self.class_index);
        }
        position_to_offset(source, position).map_or(vec![], |offset| {
            MixinTargetResolver::resolve_targets(
                &AnnotationContextExtractor::resolve_raw_mixin_targets(source, offset),
                self.class_index,
                Some(&imports),
            )
        })
    }
    pub fn diagnose(&self, request: &MixinFacadeRequest) -> FacadeResult<Vec<McDiagnostic>> {
        core_call(|| self.diagnose_inner(request))
    }
    fn diagnose_inner(&self, request: &MixinFacadeRequest) -> FacadeResult<Vec<McDiagnostic>> {
        let source = &request.buffer_text;
        let mut diagnostics = MixinDiagnosticsService::new(self.class_index, self.bytecode_index)
            .analyze(&MixinDiagnosticRequest {
                source,
                document_uri: &request.document_uri,
                mixin_class_name: request.mixin_class_name.as_deref(),
                mixin_package: request.mixin_package.as_deref(),
                mixin_config_content: request.mixin_config_content.as_deref(),
                mixin_config_path: request.mixin_config_path.as_deref(),
                semantic_model: request.semantic_model.as_ref(),
            });
        diagnostics.extend(self.analyze_members(request));
        diagnostics.extend(
            MixinExtrasDiagnosticsService::new(self.class_index, self.bytecode_index)
                .with_signature_service(self.handler())
                .analyze(&MixinExtrasDiagnosticRequest {
                    source: source.clone(),
                    document_uri: request.document_uri.clone(),
                    resolved_contexts: resolved_contexts(request).to_vec(),
                }),
        );
        diagnostics.extend(self.diagnose_locals(request)?);
        diagnostics.extend(self.diagnose_standard_sugar(request));
        Ok(diagnostics)
    }
    pub fn code_actions(
        &self,
        request: &MixinFacadeRequest,
        diagnostic_code: Option<&str>,
    ) -> FacadeResult<Vec<McFix>> {
        core_call(|| self.code_actions_inner(request, diagnostic_code))
    }
    fn code_actions_inner(
        &self,
        request: &MixinFacadeRequest,
        diagnostic_code: Option<&str>,
    ) -> FacadeResult<Vec<McFix>> {
        if JavaTypeDescriptorResolver::is_kotlin_source(&request.buffer_text, &request.document_uri)
        {
            return Ok(vec![]);
        }
        let diagnostics: Vec<_> = self
            .diagnose(request)?
            .into_iter()
            .filter(|d| diagnostic_code.is_none_or(|code| code == d.code))
            .collect();
        let mut fixes = MixinCodeActionService::new(self.class_index).fixes_for_diagnostics(
            &diagnostics,
            &request.document_uri,
            &request.buffer_text,
            request.mixin_config_content.as_deref(),
            request.mixin_config_path.as_deref(),
            request.mixin_package.as_deref(),
            Some(self.class_index),
        );
        let service = self.extras_actions();
        fixes.extend(service.fixes_for_diagnostics(
            &diagnostics,
            &request.document_uri,
            &request.buffer_text,
            resolved_contexts(request),
        ));
        if let Some(p) = position(request) {
            fixes.extend(service.generate_handler_fixes_at_position(
                &request.document_uri,
                &request.buffer_text,
                p.line,
                p.character,
                &MixinTargetResolver::resolve_targets_from_source(
                    &request.buffer_text,
                    self.class_index,
                ),
                resolved_contexts(request),
            ));
        }
        Ok(MixinExtrasCodeActionService::deduplicate_fixes(&fixes))
    }
    fn analyze_members(&self, request: &MixinFacadeRequest) -> Vec<McDiagnostic> {
        let source = &request.buffer_text;
        let mut diagnostics = vec![];
        for declaration in self.shadow_declarations(request) {
            let targets = self.targets_at(source, declaration.range.start);
            if targets.is_empty() {
                continue;
            }
            diagnostics.extend(ShadowValidationService::new(self.class_index).validate(
                &targets,
                &declaration,
                MixinMemberDeclarationParser::find_shadow_prefix(source).as_deref(),
                MixinMemberDeclarationParser::find_shadow_remap(source),
            ));
        }
        for declaration in self.accessor_declarations(request) {
            let targets = self.targets_at(source, declaration.range.start);
            if !targets.is_empty() {
                diagnostics.extend(
                    AccessorService::new(self.class_index).validate(&targets, &declaration),
                );
            }
        }
        for declaration in self.invoker_declarations(request) {
            let targets = self.targets_at(source, declaration.range.start);
            if !targets.is_empty() {
                diagnostics
                    .extend(InvokerService::new(self.class_index).validate(&targets, &declaration));
            }
        }
        diagnostics
    }
    fn semantic_members<'b>(
        &self,
        request: &'b MixinFacadeRequest,
        kind: MixinMemberAnnotationKind,
    ) -> Vec<&'b MixinMemberModel> {
        request.semantic_model.as_ref().map_or(vec![], |m| {
            m.members
                .iter()
                .filter(|m| m.annotation_kind == kind)
                .collect()
        })
    }
    fn shadow_declarations(&self, request: &MixinFacadeRequest) -> Vec<ShadowMemberDeclaration> {
        let members = self.semantic_members(request, MixinMemberAnnotationKind::Shadow);
        if members.is_empty() {
            return MixinMemberDeclarationParser::parse_shadow_declarations(
                &request.buffer_text,
                Some(self.class_index),
            );
        }
        members
            .into_iter()
            .filter_map(|m| {
                let is_method = m.is_method.unwrap_or(m.method_descriptor.is_some());
                let descriptor = if is_method {
                    m.method_descriptor.clone()
                } else {
                    m.return_descriptor.clone()
                }?;
                Some(ShadowMemberDeclaration {
                    name: m
                        .explicit_target_name
                        .clone()
                        .unwrap_or_else(|| m.java_name.clone()),
                    is_method,
                    descriptor,
                    is_static: m.modifiers.contains(&JavaModifier::Static),
                    range: m.range,
                    parse_source: m.parse_source,
                    confidence: m.confidence,
                    warnings: m.warnings.clone(),
                })
            })
            .collect()
    }
    fn accessor_declarations(
        &self,
        request: &MixinFacadeRequest,
    ) -> Vec<AccessorMethodDeclaration> {
        let members = self.semantic_members(request, MixinMemberAnnotationKind::Accessor);
        if members.is_empty() {
            return MixinMemberDeclarationParser::parse_accessor_declarations(
                &request.buffer_text,
                Some(self.class_index),
            );
        }
        members
            .into_iter()
            .filter_map(|m| {
                Some(AccessorMethodDeclaration {
                    method_name: m.java_name.clone(),
                    return_type_descriptor: Some(m.return_descriptor.clone()?),
                    parameter_descriptors: m.parameter_descriptors.clone(),
                    explicit_field_name: m.explicit_target_name.clone(),
                    range: m.range,
                    parse_source: m.parse_source,
                    confidence: m.confidence,
                    warnings: m.warnings.clone(),
                })
            })
            .collect()
    }
    fn invoker_declarations(&self, request: &MixinFacadeRequest) -> Vec<InvokerMethodDeclaration> {
        let members = self.semantic_members(request, MixinMemberAnnotationKind::Invoker);
        if members.is_empty() {
            return MixinMemberDeclarationParser::parse_invoker_declarations(
                &request.buffer_text,
                Some(self.class_index),
            );
        }
        members
            .into_iter()
            .filter_map(|m| {
                Some(InvokerMethodDeclaration {
                    method_name: m.java_name.clone(),
                    return_type_descriptor: Some(m.return_descriptor.clone()?),
                    parameter_descriptors: m.parameter_descriptors.clone(),
                    explicit_target_name: m.explicit_target_name.clone(),
                    range: m.range,
                    parse_source: m.parse_source,
                    confidence: m.confidence,
                    warnings: m.warnings.clone(),
                })
            })
            .collect()
    }
    fn diagnose_standard_sugar(&self, request: &MixinFacadeRequest) -> Vec<McDiagnostic> {
        let source = &request.buffer_text;
        let imports = JavaTypeDescriptorResolver::imports_for(source);
        let mut diagnostics = vec![];
        for site in HandlerSignatureService::find_sugar_handler_annotation_sites(source) {
            if !matches!(
                site.annotation,
                MixinExtrasAnnotation::Inject
                    | MixinExtrasAnnotation::Redirect
                    | MixinExtrasAnnotation::ModifyArg
                    | MixinExtrasAnnotation::ModifyArgs
                    | MixinExtrasAnnotation::ModifyVariable
                    | MixinExtrasAnnotation::ModifyConstant
            ) {
                continue;
            }
            let Some(handler) = &site.handler_method else {
                continue;
            };
            let Some(offset) = position_to_offset(source, site.annotation_range.start) else {
                continue;
            };
            let Some(scope) = AnnotationContextExtractor::resolve_mixin_class_scope(source, offset)
            else {
                continue;
            };
            let targets = MixinTargetResolver::resolve_targets(
                &scope.raw_targets,
                self.class_index,
                Some(&imports),
            );
            if targets.is_empty() {
                continue;
            }
            let enriched = HandlerSignatureService::enrich_handler_types_with_reference_types(
                handler,
                self.class_index,
                Some(source),
                self.reference_types,
            );
            let target = self
                .handler()
                .resolve_target_method(&targets, &site.method_attribute);
            for issue in self.handler().validate_common_sugar_constraints(
                &enriched,
                target.as_ref(),
                Some(site.annotation),
            ) {
                if !diagnostics.iter().any(|d: &McDiagnostic| {
                    d.code == issue.code && d.message == issue.message && d.range == issue.range
                }) {
                    diagnostics.push(McDiagnostic {
                        code: issue.code,
                        message: issue.message,
                        range: issue.range,
                        severity: McSeverity::Error,
                        metadata: Default::default(),
                    });
                }
            }
        }
        diagnostics
    }
    fn diagnose_locals(&self, request: &MixinFacadeRequest) -> FacadeResult<Vec<McDiagnostic>> {
        let source = &request.buffer_text;
        let mut diagnostics = vec![];
        for site in HandlerSignatureService::find_sugar_handler_annotation_sites(source) {
            if site.annotation == MixinExtrasAnnotation::WrapMethod {
                continue;
            }
            let Some(handler) = &site.handler_method else {
                continue;
            };
            let targets = self.targets_at(source, site.annotation_range.start);
            if targets.is_empty() {
                continue;
            }
            for parameter in &handler.parameters {
                if !matches!(
                    parameter.sugar_spec,
                    Some(HandlerParameterSugarSpec::Local(_))
                ) {
                    continue;
                }
                let results = self.resolve_local(request, &site, parameter, &targets)?;
                let issue = if results
                    .iter()
                    .any(|r| matches!(r, LocalCaptureValidationResult::Ambiguous { .. }))
                {
                    Some((
                        crate::mixinextras_handler::diagnostic_codes::LOCAL_CAPTURE_AMBIGUOUS,
                        "Local capture is ambiguous; specify ordinal, index, or name.",
                    ))
                } else if results
                    .iter()
                    .any(|r| matches!(r, LocalCaptureValidationResult::NotFound { .. }))
                {
                    Some((
                        crate::mixinextras_handler::diagnostic_codes::LOCAL_CAPTURE_NOT_FOUND,
                        "Could not find a matching local variable",
                    ))
                } else {
                    None
                };
                if let Some((code, message)) = issue {
                    diagnostics.push(McDiagnostic {
                        code: code.into(),
                        message: message.into(),
                        range: parameter
                            .sugar_annotation_range
                            .or(parameter.range)
                            .unwrap_or(site.annotation_range),
                        severity: McSeverity::Error,
                        metadata: Default::default(),
                    });
                }
            }
        }
        dedup(&mut diagnostics);
        Ok(diagnostics)
    }
    fn complete_at_target(
        &self,
        request: &MixinFacadeRequest,
        context: &AnnotationContext,
    ) -> Vec<McCompletionItem> {
        let owners = self.resolve_targets(request, context);
        if owners.is_empty() {
            return vec![];
        }
        let Some(selector) = &context.inject_method_name else {
            return vec![];
        };
        let (name, descriptor) = method_target(selector);
        let Some(at) = &context.at_value else {
            return vec![];
        };
        let mut candidates = vec![];
        for owner in owners {
            for candidate in self.bytecode_index.get_at_target_candidates(
                &owner,
                &name,
                descriptor.as_deref(),
                at,
            ) {
                if !candidates.contains(&candidate) {
                    candidates.push(candidate);
                }
            }
        }
        if !context.at_inside_slice {
            candidates.retain(|c| {
                valid_at_target(context.parent_injector_annotation, descriptor.as_deref(), c)
            });
        }
        AtTargetCompletionService::complete(context, &candidates)
    }
    fn complete_shadow(
        &self,
        request: &MixinFacadeRequest,
        context: &AnnotationContext,
    ) -> Vec<McCompletionItem> {
        if context.slot != AnnotationSlot::ShadowMember {
            return vec![];
        }
        let targets = self.resolve_targets(request, context);
        let service = ShadowValidationService::new(self.class_index);
        let mut items = vec![];
        if context.shadow_member_is_method != Some(true) {
            for field in service.complete_fields(
                &targets,
                &context.partial_value,
                context.shadow_prefix.as_deref(),
            ) {
                items.push(completion_item(
                    field.name.clone(),
                    Some(field.readable_type.clone()),
                    Some(field.descriptor.clone()),
                    format!("{} {}", field.name, field.readable_type),
                    field.name.clone(),
                    McCompletionKind::Field,
                    format!("0700_{}", field.name),
                    "mixin.shadow",
                    None,
                    Some(field.name),
                    Some(field.descriptor),
                ));
            }
        }
        if context.shadow_member_is_method != Some(false) {
            for method in service.complete_methods(
                &targets,
                &context.partial_value,
                context.shadow_prefix.as_deref(),
            ) {
                items.push(completion_item(
                    method.readable_signature.clone(),
                    Some(method.descriptor.clone()),
                    Some(method.readable_signature.clone()),
                    format!("{} {}", method.name, method.readable_signature),
                    method.name.clone(),
                    McCompletionKind::Value,
                    format!("0701_{}", method.name),
                    "mixin.shadow",
                    None,
                    Some(method.name),
                    Some(method.descriptor),
                ));
            }
        }
        items
    }
    fn complete_overwrite(
        &self,
        request: &MixinFacadeRequest,
        context: &AnnotationContext,
    ) -> Vec<McCompletionItem> {
        if context.slot != AnnotationSlot::OverwriteMethod {
            return vec![];
        }
        let mut seen = BTreeSet::new();
        let mut items = vec![];
        for owner in self.resolve_targets(request, context) {
            for method in self.class_index.get_methods(&owner) {
                if matches!(method.name.as_str(), "<init>" | "<clinit>")
                    || !method.name.starts_with(&context.partial_value)
                    || !seen.insert((method.name.clone(), method.descriptor.clone()))
                {
                    continue;
                }
                items.push(completion_item(
                    method.readable_signature.clone(),
                    Some(owner.replace('/', ".")),
                    Some(method.descriptor.clone()),
                    format!("{} {}", method.name, method.readable_signature),
                    method.name.clone(),
                    McCompletionKind::Value,
                    format!("0702_{}", method.name),
                    "mixin.overwrite",
                    Some(owner.clone()),
                    Some(method.name),
                    Some(method.descriptor),
                ));
            }
        }
        items
    }
    fn complete_local(
        &self,
        request: &MixinFacadeRequest,
        context: &AnnotationContext,
    ) -> FacadeResult<Vec<McCompletionItem>> {
        let Some(attribute) = context
            .attribute_name
            .as_deref()
            .filter(|a| ["ordinal", "index", "name"].contains(a))
        else {
            return Ok(vec![]);
        };
        let matches = local_parameter_sites(&request.buffer_text, context.value_start_offset);
        if matches.is_empty()
            || matches
                .iter()
                .any(|(s, _)| s.annotation == MixinExtrasAnnotation::WrapMethod)
        {
            return Ok(vec![]);
        }
        let targets = self.resolve_targets(request, context);
        if targets.is_empty() {
            return Ok(vec![]);
        }
        let mut results = vec![];
        for (site, parameter) in &matches {
            results.extend(self.resolve_local(request, site, parameter, &targets)?);
        }
        Ok(LocalCompletionService::new(self.class_index).complete(
            &request.buffer_text,
            &matches[0].1,
            &results,
            attribute,
            context.partial_value.trim().trim_matches('"'),
        ))
    }
    fn resolve_local(
        &self,
        request: &MixinFacadeRequest,
        site: &MixinExtrasAnnotationSite,
        parameter: &HandlerParameterDeclaration,
        targets: &[String],
    ) -> FacadeResult<Vec<LocalCaptureValidationResult>> {
        let Some(method) = self
            .handler()
            .resolve_target_method(targets, &site.method_attribute)
        else {
            return Ok(vec![]);
        };
        let owners: Vec<_> = targets
            .iter()
            .filter(|o| {
                self.class_index
                    .get_methods(o)
                    .iter()
                    .any(|m| m.name == method.name && m.descriptor == method.descriptor)
            })
            .collect();
        if owners.is_empty() {
            return Ok(vec![]);
        }
        let points: Vec<_> = owners
            .into_iter()
            .map(|owner| LocalCaptureValidationPoint {
                owner: owner.clone(),
                target_method: method.clone(),
                site: site.clone(),
                expression_instruction_indices: self.expression_instruction_indices(
                    &request.buffer_text,
                    site,
                    &method,
                    owner,
                    resolved_contexts(request),
                ),
            })
            .collect();
        LocalCaptureValidationService::new(self.class_index, self.bytecode_index).validate(
            &request.buffer_text,
            parameter,
            &points,
        )
    }
    fn expression_instruction_indices(
        &self,
        source: &str,
        site: &MixinExtrasAnnotationSite,
        method: &MethodIndexEntry,
        owner: &str,
        contexts: &[ResolvedMixinExtrasContext],
    ) -> Option<BTreeSet<i32>> {
        if !ExpressionSupport::is_expression_at_value(site.at_value.as_deref()) {
            return None;
        }
        let kind = official_context_type(site.annotation)?;
        let context =
            ExpressionContextResolver::resolve_expression_context_for_site(source, site, contexts)?;
        let expressions: Vec<_> = context
            .values_for_at_id_utf16(site.at_id.as_deref())
            .into_iter()
            .map(|s| trim_expression_utf16(&s).to_vec())
            .filter(|s| !s.is_empty())
            .collect();
        if expressions.is_empty() {
            return None;
        }
        let bytes = self.bytecode_index.get_class_bytes(owner)?;
        let pool = OfficialExpressionIdentifierPoolBuilder::build(
            &context.definition_index,
            &ClassLiteralTypeNameResolver::for_source(source, self.class_index),
        )
        .pool;
        let common =
            |left: &str, right: &str| self.bytecode_index.resolve_common_super_class(left, right);
        let cancellation = engine::current_cancellation();
        let context = engine::MatchContext {
            context_type: kind,
            identifier_pool: &pool,
            allow_incomplete_list_inputs: false,
            common_super_class: &common,
            cancellation: cancellation.as_ref(),
        };
        match engine::match_expressions_utf16(
            &bytes,
            &method.name,
            &method.descriptor,
            &expressions,
            &context,
        ) {
            Ok(engine::MatchResult::Unavailable { .. }) => None,
            Ok(engine::MatchResult::Available(matches)) => {
                let indices: BTreeSet<_> = matches
                    .iter()
                    .map(|m| m.original_instruction_index as i32)
                    .collect();
                (!indices.is_empty()).then_some(indices)
            }
            Err(cancel) => std::panic::panic_any(cancel),
        }
    }
    fn debug_info(
        &self,
        request: &MixinFacadeRequest,
        command: &str,
        language_id: &str,
        context: Option<&AnnotationContext>,
        semantic: Option<&MixinCompletionContext>,
        fallback_used: bool,
        fallback_reason: Option<String>,
        items: &[McCompletionItem],
        warnings: Vec<String>,
    ) -> McdevCompletionDebugInfo {
        let model = request.semantic_model.as_ref();
        let parsed_method = if let Some(MixinCompletionContext::AtTarget {
            method_name: Some(name),
            method_descriptor,
            ..
        }) = semantic
        {
            Some((name.clone(), method_descriptor.clone()))
        } else {
            context
                .and_then(|c| c.inject_method_name.as_ref())
                .map(|n| method_target(n))
        };
        let zero = if context.is_none() {
            Some("NO_COMPLETION_CONTEXT")
        } else if fallback_used && items.is_empty() {
            Some("FALLBACK_ANNOTATION_CONTEXT_USED")
        } else if model.is_none_or(|m| m.targets.is_empty())
            && context.is_some_and(|c| c.annotation != MixinAnnotation::Mixin)
        {
            Some("NO_MIXIN_TARGET")
        } else if items.is_empty() {
            Some("NO_CANDIDATES")
        } else {
            None
        };
        McdevCompletionDebugInfo {
            command: command.into(),
            document_uri: request.document_uri.clone(),
            language_id: language_id.into(),
            parse_source: model.map(|m| m.parse_source),
            parse_confidence: model.map(|m| m.confidence),
            used_compilation_unit: model.is_some_and(|m| m.debug_info.used_compilation_unit),
            used_java_project: model.is_some_and(|m| m.debug_info.used_java_project),
            binding_resolved_count: model.map_or(0, |m| m.debug_info.binding_resolved_count),
            binding_failed_count: model.map_or(0, |m| m.debug_info.binding_failed_count),
            fallback_reason: model.and_then(|m| m.debug_info.fallback_reason.clone()),
            semantic_context_found: semantic.is_some(),
            fallback_annotation_context_used: fallback_used,
            fallback_annotation_context_reason: fallback_reason,
            semantic_target_count: model.map_or(0, |m| m.targets.len()),
            semantic_member_count: model.map_or(0, |m| m.members.len()),
            completion_context_kind: semantic
                .map(completion_kind)
                .map(str::to_string)
                .or_else(|| context.map(|c| slot_name(c.slot))),
            owner: model
                .and_then(|m| m.targets.first())
                .map(|t| t.internal_name.clone()),
            method_name: parsed_method.as_ref().map(|v| v.0.clone()),
            method_descriptor: parsed_method.and_then(|v| v.1),
            candidate_count_before_filter: items.len(),
            candidate_count_after_filter: items.len(),
            zero_item_reason: zero.map(str::to_string),
            warnings,
        }
    }
}
fn position(request: &MixinFacadeRequest) -> Option<McTextPosition> {
    Some(McTextPosition {
        line: request.line.try_into().ok()?,
        character: request.character.try_into().ok()?,
    })
}
fn core_call<T>(body: impl FnOnce() -> FacadeResult<T>) -> FacadeResult<T> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)) {
        Ok(result) => result,
        Err(error) => {
            if let Some(cancelled) = error.downcast_ref::<engine::Cancellation>() {
                Err(cancelled.clone())
            } else {
                std::panic::resume_unwind(error)
            }
        }
    }
}
fn resolved_contexts(request: &MixinFacadeRequest) -> &[ResolvedMixinExtrasContext] {
    request
        .semantic_model
        .as_ref()
        .map_or(&[], |m| m.resolved_mixin_extras_contexts.as_slice())
}
fn method_target(value: &str) -> (String, Option<String>) {
    if let Some(at) = value.find('(').filter(|at| *at > 0) {
        (value[..at].into(), Some(value[at..].into()))
    } else {
        (value.into(), None)
    }
}
fn dedup<T: PartialEq>(values: &mut Vec<T>) {
    let mut n = 0;
    while n < values.len() {
        if values[..n].contains(&values[n]) {
            values.remove(n);
        } else {
            n += 1;
        }
    }
}
fn local_parameter_sites(
    source: &str,
    offset: usize,
) -> Vec<(MixinExtrasAnnotationSite, HandlerParameterDeclaration)> {
    let mut out = vec![];
    for site in HandlerSignatureService::find_sugar_handler_annotation_sites(source) {
        let Some(handler) = &site.handler_method else {
            continue;
        };
        for parameter in &handler.parameters {
            if !matches!(
                parameter.sugar_spec,
                Some(HandlerParameterSugarSpec::Local(_))
            ) {
                continue;
            }
            let Some(range) = parameter.sugar_annotation_range else {
                continue;
            };
            let Some(start) = position_to_offset(source, range.start) else {
                continue;
            };
            let Some(end) = position_to_offset(source, range.end) else {
                continue;
            };
            if start <= offset && offset < end {
                out.push((site.clone(), parameter.clone()));
            }
        }
    }
    out
}
fn completion_kind(context: &MixinCompletionContext) -> &'static str {
    match context {
        MixinCompletionContext::MixinTarget { .. } => "MixinTarget",
        MixinCompletionContext::InjectMethod { .. } => "InjectMethod",
        MixinCompletionContext::AtValue { .. } => "AtValue",
        MixinCompletionContext::AtTarget { .. } => "AtTarget",
        MixinCompletionContext::AccessorValue { .. } => "AccessorValue",
        MixinCompletionContext::InvokerValue { .. } => "InvokerValue",
        MixinCompletionContext::MemberName { .. } => "MemberName",
    }
}
fn slot_name(slot: AnnotationSlot) -> String {
    let mut name = String::new();
    for c in format!("{slot:?}").chars() {
        if c.is_uppercase() && !name.is_empty() {
            name.push('_');
        }
        name.extend(c.to_uppercase());
    }
    name
}
fn completion_item(
    label: String,
    detail: Option<String>,
    documentation: Option<String>,
    filter_text: String,
    insert_text: String,
    kind: McCompletionKind,
    sort_key: String,
    source: &str,
    owner: Option<String>,
    name: Option<String>,
    descriptor: Option<String>,
) -> McCompletionItem {
    McCompletionItem {
        label,
        detail,
        documentation,
        filter_text,
        insert_text,
        kind,
        sort_key,
        metadata: McCompletionMetadata {
            source: source.into(),
            owner,
            name,
            descriptor,
            namespace: None,
        },
        additional_edits: vec![],
        insert_text_format: McCompletionInsertTextFormat::PlainText,
    }
}
fn normalize_snippet(
    source: &str,
    context: &AnnotationContext,
    mut item: McCompletionItem,
) -> McCompletionItem {
    let cursor = context.value_end_offset.min(source.len());
    let start = source[..cursor].rfind('\n').map_or(0, |i| i + 1);
    let same_line =
        !source[context.annotation_end_offset.min(cursor)..cursor].contains(['\n', '\r']);
    let indent_end = if same_line {
        context.annotation_start_offset.clamp(start, source.len())
    } else {
        cursor
    };
    let indent: String = source[start..indent_end]
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect();
    let continuation = item
        .insert_text
        .split('\n')
        .enumerate()
        .map(|(i, line)| {
            if i == 0 || line.trim().is_empty() {
                line.into()
            } else {
                format!("{indent}{line}")
            }
        })
        .collect::<Vec<String>>()
        .join("\n");
    item.insert_text = if same_line {
        format!("\n{indent}{continuation}")
    } else {
        continuation
    };
    item
}
fn has_non_void_return(value: Option<&str>) -> bool {
    value
        .and_then(|v| descriptor::parse_method_descriptor(v).ok())
        .is_some_and(|m| m.return_type != descriptor::JvmType::VoidType)
}
fn valid_at_target(
    injector: Option<MixinAnnotation>,
    method_descriptor: Option<&str>,
    candidate: &AtTargetCandidate,
) -> bool {
    use AtTargetOperationKind::*;
    let invoke = matches!(
        candidate.operation_kind,
        Some(InvokeVirtual | InvokeStatic | InvokeSpecial | InvokeInterface)
    );
    let field_get = matches!(
        candidate.operation_kind,
        Some(FieldGetInstance | FieldGetStatic)
    );
    let field_put = matches!(
        candidate.operation_kind,
        Some(FieldPutInstance | FieldPutStatic)
    );
    match injector {
        Some(MixinAnnotation::ModifyExpressionValue) => match candidate.kind {
            AtTargetKind::Invoke => {
                candidate.name != "<init>" && has_non_void_return(Some(&candidate.descriptor))
            }
            AtTargetKind::Field => field_get,
            AtTargetKind::New | AtTargetKind::Constant => true,
            _ => false,
        },
        Some(MixinAnnotation::ModifyReturnValue) => {
            has_non_void_return(method_descriptor) && candidate.kind == AtTargetKind::Return
        }
        Some(MixinAnnotation::ModifyReceiver) => {
            candidate.name != "<init>"
                && matches!(
                    candidate.operation_kind,
                    Some(
                        InvokeVirtual
                            | InvokeSpecial
                            | InvokeInterface
                            | FieldGetInstance
                            | FieldPutInstance
                    )
                )
        }
        Some(MixinAnnotation::WrapOperation) => match candidate.kind {
            AtTargetKind::Invoke => candidate.name != "<init>" && invoke,
            AtTargetKind::Field => field_get || field_put,
            AtTargetKind::New => true,
            _ => false,
        },
        Some(MixinAnnotation::WrapWithCondition) => {
            (candidate.name != "<init>"
                && invoke
                && matches!(
                    candidate.occurrence_result_classification,
                    OccurrenceResultClassification::Void
                        | OccurrenceResultClassification::ImmediatelyPopped
                ))
                || field_put
        }
        _ => true,
    }
}

/// Host-independent early completion. The supplied feature callbacks are evaluated
/// only by the At-value service, after its prefix and eligibility checks.
pub struct HostMixinFacade;
impl HostMixinFacade {
    pub fn try_complete_buffer_only<'a>(
        source: &str,
        line: i32,
        character: i32,
        options: &MixinCompletionOptions,
        document_uri: &str,
        language_id: &str,
        facade_overridden: bool,
        at_value_completion: AtValueCompletionService<'a>,
    ) -> FacadeResult<Option<BufferOnlyCompletionResult>> {
        if facade_overridden {
            return Ok(None);
        }
        let request = MixinFacadeRequest {
            buffer_text: source.into(),
            line,
            character,
            document_uri: document_uri.into(),
            ..Default::default()
        };
        let Some(offset) = position(&request).and_then(|p| position_to_offset(source, p)) else {
            return Ok(None);
        };
        let context = AnnotationContextExtractor::extract_at_offset(source, offset);
        if context
            .as_ref()
            .is_some_and(|c| !Self::is_buffer_only_eligible(c))
        {
            return Ok(None);
        }
        let facade = MixinServiceFacade::new(&BUFFER_ONLY_INDEX, &BUFFER_ONLY_INDEX)
            .with_at_value_completion(at_value_completion);
        let result =
            facade.complete_with_debug(&request, options, "mcdev.completion", language_id)?;
        if context.is_none() && (result.items.is_empty() || result.replacement_range.is_none()) {
            return Ok(None);
        }
        Ok(Some(BufferOnlyCompletionResult { result, context }))
    }
    pub fn is_buffer_only_eligible(context: &AnnotationContext) -> bool {
        context.slot == AnnotationSlot::Attribute
            || (context.annotation == MixinAnnotation::At && context.slot == AnnotationSlot::Value)
            || (matches!(
                context.annotation,
                MixinAnnotation::Expression | MixinAnnotation::Expressions
            ) && context.slot == AnnotationSlot::Value
                && matches!(
                    context.expression_completion_position,
                    Some(
                        ExpressionCompletionPosition::StatementStart
                            | ExpressionCompletionPosition::ValueStart
                            | ExpressionCompletionPosition::AfterMethodReference
                    )
                ))
    }
    pub fn is_kotlin_document(uri: &str) -> bool {
        let uri = uri.to_ascii_lowercase();
        uri.ends_with(".kt") || uri.ends_with(".kts")
    }
}
struct BufferOnlyIndex;
static BUFFER_ONLY_INDEX: BufferOnlyIndex = BufferOnlyIndex;
impl ClassIndex for BufferOnlyIndex {
    fn find_classes(&self, _prefix: &str, _limit: usize) -> Vec<ClassIndexEntry> {
        vec![]
    }
    fn find_class(&self, _name: &str) -> Option<ClassIndexEntry> {
        None
    }
    fn find_class_by_fqn(&self, _name: &str) -> Option<ClassIndexEntry> {
        None
    }
    fn get_methods(&self, _owner: &str) -> Vec<MethodIndexEntry> {
        vec![]
    }
    fn get_fields(&self, _owner: &str) -> Vec<FieldIndexEntry> {
        vec![]
    }
}
impl BytecodeIndex for BufferOnlyIndex {
    fn get_at_target_candidates(
        &self,
        _owner: &str,
        _method: &str,
        _descriptor: Option<&str>,
        _at_value: &str,
    ) -> Vec<AtTargetCandidate> {
        vec![]
    }
    fn get_return_ordinal_count(
        &self,
        _owner: &str,
        _method: &str,
        _descriptor: Option<&str>,
    ) -> usize {
        0
    }
}

use crate::host_source::{
    HostFact, HostProjectKey, HostSemanticEnvironment, HostSemanticModelProvider,
    HostSemanticRequest, HostSourceError, SemanticModelCache,
};
/// The language host owns semantic/project/SDK facts and their evaluation scope.
/// This adapter assembles requests; it never builds a language or classpath index.
pub struct HostMixinServiceFacade<'a> {
    pub class_index: &'a dyn ClassIndex,
    pub bytecode_index: &'a dyn BytecodeIndex,
    pub semantic_provider: &'a dyn HostSemanticModelProvider,
    pub semantic_cache: &'a SemanticModelCache,
    pub project: Option<HostProjectKey>,
    pub context: Option<&'a crate::project::ProjectContext>,
    pub document_version: Option<i64>,
    /// Java-only legacy providers leave this false; an integrated host explicitly
    /// enables its normalized Kotlin facts through the same provider boundary.
    pub host_supports_kotlin_semantics: bool,
    pub project_session_version: u64,
    pub reference_types: Option<&'a dyn HandlerReferenceTypeResolver>,
    pub expression_cache: Arc<ExpressionMemberCompletionCache>,
    pub expression_cache_project: Option<HostProjectKey>,
    pub project_resolver: Box<
        dyn Fn(&str, Option<&str>, bool) -> Result<Option<HostProjectKey>, HostSourceError> + 'a,
    >,
    /// Opens/validates the host source-query scope for an already composed index.
    pub source_query_ready: Box<dyn Fn(&HostProjectKey) -> Result<(), HostSourceError> + 'a>,
    pub specifier_supported:
        Box<dyn Fn(Option<&HostProjectKey>, u64) -> Result<bool, HostSourceError> + 'a>,
    pub additional_at_values:
        Box<dyn Fn(Option<&HostProjectKey>, u64) -> Result<Vec<String>, HostSourceError> + 'a>,
    pub share_sources: Box<
        dyn Fn(&str, &str, Option<&HostProjectKey>) -> Box<dyn Iterator<Item = String> + 'a> + 'a,
    >,
    pub completion_override: Option<
        Box<
            dyn Fn(
                    &MixinFacadeRequest,
                    &MixinCompletionOptions,
                ) -> FacadeResult<Vec<McCompletionItem>>
                + 'a,
        >,
    >,
    pub core_override: Option<
        Box<
            dyn Fn(
                    &MixinFacadeRequest,
                    &MixinCompletionOptions,
                    &dyn ClassIndex,
                    &dyn BytecodeIndex,
                    &str,
                ) -> FacadeResult<MixinCompletionResult>
                + 'a,
        >,
    >,
}
#[derive(Debug, Clone)]
pub enum HostFacadeError {
    Source(HostSourceError),
    Cancelled(engine::Cancellation),
    Unavailable(String),
}
impl From<HostSourceError> for HostFacadeError {
    fn from(value: HostSourceError) -> Self {
        Self::Source(value)
    }
}
impl From<engine::Cancellation> for HostFacadeError {
    fn from(value: engine::Cancellation) -> Self {
        Self::Cancelled(value)
    }
}
pub type HostFacadeResult<T> = Result<T, HostFacadeError>;
impl<'a> HostMixinServiceFacade<'a> {
    pub fn new(
        index: &'a dyn ClassIndex,
        bytecode: &'a dyn BytecodeIndex,
        semantic_provider: &'a dyn HostSemanticModelProvider,
        semantic_cache: &'a SemanticModelCache,
    ) -> Self {
        Self {
            class_index: index,
            bytecode_index: bytecode,
            semantic_provider,
            semantic_cache,
            project: None,
            context: None,
            document_version: None,
            host_supports_kotlin_semantics: false,
            project_session_version: 0,
            reference_types: None,
            expression_cache: Arc::new(ExpressionMemberCompletionCache::default()),
            expression_cache_project: None,
            project_resolver: Box::new(|_, _, _| Ok(None)),
            source_query_ready: Box::new(|_| Ok(())),
            specifier_supported: Box::new(|_, _| Ok(false)),
            additional_at_values: Box::new(|_, _| Ok(vec![])),
            share_sources: Box::new(|_, _, _| Box::new(std::iter::empty())),
            completion_override: None,
            core_override: None,
        }
    }
    pub fn for_session(
        session: &'a crate::host_project::HostProjectSession,
        provider: &'a dyn HostSemanticModelProvider,
        cache: &'a SemanticModelCache,
    ) -> Self {
        let mut service = Self::new(
            session.class_index.as_ref(),
            session.bytecode_index.as_ref(),
            provider,
            cache,
        );
        service.context = Some(session.context.as_ref());
        service.project = Some(HostProjectKey {
            project_id: session.context.project_id.clone(),
            generations: session.identity.generations,
        });
        service.project_session_version = session.identity.project_session_version;
        service
    }
    pub fn with_expression_cache(
        mut self,
        cache: Arc<ExpressionMemberCompletionCache>,
        project: HostProjectKey,
    ) -> Self {
        self.expression_cache = cache;
        self.expression_cache_project = Some(project);
        self
    }
    pub fn with_registered_sources(mut self, provider: &'a dyn HostMixinSourceProvider) -> Self {
        let context = self.context;
        self.share_sources = Box::new(move |source, uri, _project| match context {
            Some(context) => Box::new(
                HostMixinFacade::collect_registered_mixin_sources(context, source, uri, provider)
                    .map(|value| value.unwrap_or_else(|error| std::panic::panic_any(error))),
            ),
            None => Box::new(std::iter::empty()),
        });
        self
    }
    pub fn semantic_environment(
        context: Option<&crate::project::ProjectContext>,
        uri: &str,
    ) -> HostSemanticEnvironment {
        let Some(context) = context else {
            return HostSemanticEnvironment::default();
        };
        let path = crate::project::uri_to_path(uri).ok();
        let unit_name = path.as_ref().and_then(|path| {
            context
                .source_sets
                .iter()
                .flat_map(|s| &s.source_directories)
                .find_map(|root| {
                    path.strip_prefix(root)
                        .ok()
                        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
                })
                .or_else(|| path.file_name().map(|n| n.to_string_lossy().into_owned()))
        });
        HostSemanticEnvironment {
            classpath_entries: context
                .classpath
                .all_entries()
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
            sourcepath_entries: context
                .source_sets
                .iter()
                .flat_map(|s| &s.source_directories)
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
            unit_name,
        }
    }
    pub fn semantic_model(
        &self,
        source: &str,
        uri: &str,
        use_session: bool,
    ) -> HostFacadeResult<MixinClassModel> {
        if HostMixinFacade::is_kotlin_document(uri) && !self.host_supports_kotlin_semantics {
            return Ok(MixinSemanticModelParser::parse(source, uri));
        }
        let environment =
            Self::semantic_environment(if use_session { self.context } else { None }, uri);
        let answer = self.semantic_cache.get(
            &HostSemanticRequest {
                source,
                source_uri: uri,
                document_version: self.document_version,
                environment: &environment,
                project: self.project.as_ref(),
            },
            self.semantic_provider,
        )?;
        match answer.answer.fact {
            HostFact::Known(semantic) => Ok(semantic.model),
            HostFact::Missing => Err(HostFacadeError::Unavailable(
                "host semantic model is missing".into(),
            )),
            HostFact::Unavailable { reason } => Err(HostFacadeError::Unavailable(reason)),
        }
    }
    pub fn complete(
        &self,
        request: &MixinFacadeRequest,
        options: &MixinCompletionOptions,
    ) -> HostFacadeResult<Vec<McCompletionItem>> {
        Ok(self.complete_with_debug(request, options, "java")?.items)
    }
    pub fn complete_with_debug(
        &self,
        request: &MixinFacadeRequest,
        options: &MixinCompletionOptions,
        language_id: &str,
    ) -> HostFacadeResult<MixinCompletionResult> {
        let mut request = request.clone();
        if request.semantic_model.is_none() {
            request.semantic_model =
                Some(self.semantic_model(&request.buffer_text, &request.document_uri, true)?);
        }
        if let Some(complete) = &self.completion_override {
            let items = complete(&request, options)?;
            let core = MixinServiceFacade::new(self.class_index, self.bytecode_index);
            let mut debug = core.debug_info(
                &request,
                "mcdev.completion",
                language_id,
                None,
                None,
                false,
                None,
                &items,
                request
                    .semantic_model
                    .as_ref()
                    .map_or(vec![], |m| m.warnings.clone()),
            );
            debug.completion_context_kind = Some("OVERRIDE".into());
            debug.zero_item_reason = items.is_empty().then(|| "NO_CANDIDATES".into());
            return Ok(MixinCompletionResult {
                items,
                debug,
                replacement_range: None,
            });
        }
        let root = self.context.map(|c| crate::project::path_to_uri(&c.root));
        let project = (self.project_resolver)(
            &request.document_uri,
            root.as_deref(),
            HostMixinFacade::is_kotlin_document(&request.document_uri),
        )?;
        if let Some(project) = &project {
            (self.source_query_ready)(project)?;
        }
        if let Some(factory) = &self.core_override {
            return host_call(|| {
                factory(
                    &request,
                    options,
                    self.class_index,
                    self.bytecode_index,
                    language_id,
                )
            });
        }
        let at = AtValueCompletionService::new(
            || {
                host_callback((self.specifier_supported)(
                    project.as_ref(),
                    self.project_session_version,
                ))
            },
            || {
                host_callback((self.additional_at_values)(
                    project.as_ref(),
                    self.project_session_version,
                ))
            },
        );
        let cache = if self.project.is_some() && self.expression_cache_project == self.project {
            self.expression_cache.clone()
        } else {
            Arc::new(ExpressionMemberCompletionCache::default())
        };
        let mut core = MixinServiceFacade::new(self.class_index, self.bytecode_index)
            .with_at_value_completion(at)
            .with_expression_cache(cache)
            .with_share_sources(|| {
                (self.share_sources)(
                    &request.buffer_text,
                    &request.document_uri,
                    project.as_ref(),
                )
            });
        if let Some(types) = self.reference_types {
            core = core.with_reference_types(types);
        }
        host_call(|| core.complete_with_debug(&request, options, "mcdev.completion", language_id))
    }
    pub fn diagnose(&self, request: &MixinFacadeRequest) -> HostFacadeResult<Vec<McDiagnostic>> {
        let mut request = request.clone();
        if request.semantic_model.is_none() {
            request.semantic_model =
                Some(self.semantic_model(&request.buffer_text, &request.document_uri, true)?);
        }
        let mut facade = MixinServiceFacade::new(self.class_index, self.bytecode_index);
        if let Some(types) = self.reference_types {
            facade = facade.with_reference_types(types);
        }
        host_call(|| facade.diagnose(&request))
    }
    pub fn code_actions(
        &self,
        request: &MixinFacadeRequest,
        code: Option<&str>,
    ) -> HostFacadeResult<Vec<McFix>> {
        let mut request = request.clone();
        if request.semantic_model.is_none() {
            request.semantic_model =
                Some(self.semantic_model(&request.buffer_text, &request.document_uri, true)?);
        }
        let mut facade = MixinServiceFacade::new(self.class_index, self.bytecode_index);
        if let Some(types) = self.reference_types {
            facade = facade.with_reference_types(types);
        }
        host_call(|| facade.code_actions(&request, code))
    }
    pub fn definitions(
        &self,
        source: &str,
        line: usize,
        character: usize,
        uri: &str,
    ) -> HostFacadeResult<Vec<McDefinitionTarget>> {
        let model = self.semantic_model(source, uri, true)?;
        host_call(|| {
            Ok(crate::mixin_navigation::MixinDefinitionService::new(
                self.class_index,
                self.bytecode_index,
            )
            .definitions_at(source, line, character, Some(&model), Some(uri)))
        })
    }
    pub fn build_request(
        &self,
        source: &str,
        uri: &str,
        line: i32,
        character: i32,
        provider: &dyn HostMixinSourceProvider,
    ) -> HostFacadeResult<MixinFacadeRequest> {
        let config = if let Some(context) = self.context {
            HostMixinFacade::selected_mixin_config(context, source, Some(uri), provider)?
        } else {
            None
        };
        let content = if let Some(config) = &config {
            match provider.read_source_file(&config.path)?.fact {
                HostFact::Known(text) => Some(text),
                HostFact::Missing => None,
                HostFact::Unavailable { reason } => {
                    return Err(HostFacadeError::Unavailable(reason))
                }
            }
        } else {
            None
        };
        let package = source_package_name(source)
            .or_else(|| config.as_ref().and_then(|c| c.package_name.clone()));
        let model = self.semantic_model(source, uri, true)?;
        Ok(MixinFacadeRequest {
            buffer_text: source.into(),
            line,
            character,
            document_uri: uri.into(),
            mixin_class_name: source_class_name(source),
            mixin_package: package,
            mixin_config_content: content,
            mixin_config_path: config
                .as_ref()
                .map(|c| crate::project::path_to_uri(&c.path)),
            semantic_model: Some(model),
        })
    }
    pub fn diagnose_document(
        &self,
        source: &str,
        uri: &str,
        provider: &dyn HostMixinSourceProvider,
    ) -> HostFacadeResult<Vec<McDiagnostic>> {
        self.diagnose(&self.build_request(source, uri, 0, 0, provider)?)
    }
    pub fn code_actions_document(
        &self,
        source: &str,
        uri: &str,
        line: i32,
        character: i32,
        code: Option<&str>,
        provider: &dyn HostMixinSourceProvider,
    ) -> HostFacadeResult<Vec<McFix>> {
        self.code_actions(
            &self.build_request(source, uri, line, character, provider)?,
            code,
        )
    }
    pub fn has_java_project(
        &self,
        uri: &str,
        workspace: Option<&str>,
        mixin_api_exists: &dyn Fn(&HostProjectKey) -> Result<bool, HostSourceError>,
    ) -> HostFacadeResult<bool> {
        let project =
            (self.project_resolver)(uri, workspace, HostMixinFacade::is_kotlin_document(uri))?;
        Ok(if let Some(project) = project {
            mixin_api_exists(&project)?
        } else {
            false
        })
    }
    pub fn references(
        &self,
        target: &McDefinitionTarget,
        sources: &[crate::SourceScanEntry],
    ) -> Vec<McReferenceLocation> {
        crate::mixin_navigation::MixinReferenceService::find_references(target, sources)
    }
    pub fn to_completion_options(
        class_insert: &str,
        descriptor: &str,
        preferred_at_target: &str,
    ) -> MixinCompletionOptions {
        MixinCompletionOptions {
            class_insert_mode: if class_insert.eq_ignore_ascii_case("fqn") {
                MixinClassInsertMode::Fqn
            } else {
                MixinClassInsertMode::Import
            },
            inject_method_descriptor_mode: if descriptor.eq_ignore_ascii_case("always") {
                InjectMethodDescriptorMode::Always
            } else if descriptor.eq_ignore_ascii_case("never") {
                InjectMethodDescriptorMode::Never
            } else {
                InjectMethodDescriptorMode::Auto
            },
            preferred_at_target: preferred_at_target.into(),
        }
    }
}
fn host_callback<T>(answer: Result<T, HostSourceError>) -> T {
    answer.unwrap_or_else(|error| std::panic::panic_any(error))
}
fn host_call<T>(body: impl FnOnce() -> FacadeResult<T>) -> HostFacadeResult<T> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)) {
        Ok(result) => result.map_err(HostFacadeError::Cancelled),
        Err(error) => {
            if let Some(error) = error.downcast_ref::<HostFacadeError>() {
                Err(error.clone())
            } else if let Some(error) = error.downcast_ref::<HostSourceError>() {
                Err(HostFacadeError::Source(error.clone()))
            } else if let Some(error) = error.downcast_ref::<engine::Cancellation>() {
                Err(HostFacadeError::Cancelled(error.clone()))
            } else {
                std::panic::resume_unwind(error)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisteredMixinSource {
    Current,
    Source(String),
}
/// Host source and explicit resource reads are evaluated afresh. Missing host
/// source may use a known source-root path; unavailable authority stays unknown.
pub trait HostMixinSourceProvider: Send + Sync {
    fn config(
        &self,
        path: &std::path::Path,
    ) -> crate::host_source::HostSourceResult<crate::project::MixinConfigRef>;
    fn compilation_unit_source(
        &self,
        fqn: &str,
        current_path: Option<&std::path::Path>,
    ) -> crate::host_source::HostSourceResult<RegisteredMixinSource>;
    fn read_source_file(
        &self,
        path: &std::path::Path,
    ) -> crate::host_source::HostSourceResult<String>;
}
impl HostMixinFacade {
    pub fn collect_source_entries(
        current_uri: &str,
        current_source: &str,
        sources: impl IntoIterator<Item = SourceScanEntry>,
    ) -> Vec<SourceScanEntry> {
        let mut seen = BTreeSet::from([current_uri.to_string()]);
        let mut entries = vec![SourceScanEntry {
            document_uri: current_uri.into(),
            text: current_source.into(),
        }];
        entries.extend(
            sources
                .into_iter()
                .filter(|s| seen.insert(s.document_uri.clone())),
        );
        entries
    }
    pub fn mixin_config_refs(
        context: &crate::project::ProjectContext,
        uri: Option<&str>,
    ) -> Vec<crate::project::MixinConfigRef> {
        use crate::project::{MixinConfigDiscoveryService as Discovery, SourceSetResolver};
        let Some(set) = uri
            .and_then(|u| crate::project::uri_to_path(u).ok())
            .and_then(|p| SourceSetResolver::containing_source_set(&context.source_sets, &p))
        else {
            return context.mixin_configs.clone();
        };
        let mut configs = Discovery::configs_for_source_set(
            &context.mixin_configs,
            set,
            &context.source_sets,
            &context.root,
        );
        configs.extend(Discovery::shared_configs(
            &context.mixin_configs,
            &context.source_sets,
            &context.root,
        ));
        dedup(&mut configs);
        configs
    }
    pub fn selected_mixin_config(
        context: &crate::project::ProjectContext,
        source: &str,
        uri: Option<&str>,
        _provider: &dyn HostMixinSourceProvider,
    ) -> HostFacadeResult<Option<crate::project::MixinConfigRef>> {
        use crate::project::{MixinConfigDiscoveryService as Discovery, SourceSetResolver};
        let class = source_class_name(source);
        let package = source_package_name(source);
        let select = |configs: &[crate::project::MixinConfigRef]| {
            Discovery::select_for_mixin(configs, class.as_deref(), package.as_deref()).cloned()
        };
        // Selection observes the host's project snapshot. Reading the JSON is
        // a separate operation performed only after selecting the original ref.
        let Some(set) = uri
            .and_then(|uri| crate::project::uri_to_path(uri).ok())
            .and_then(|path| SourceSetResolver::containing_source_set(&context.source_sets, &path))
        else {
            return Ok(select(&context.mixin_configs));
        };
        Ok(select(&Discovery::configs_for_source_set(
            &context.mixin_configs,
            set,
            &context.source_sets,
            &context.root,
        ))
        .or_else(|| {
            select(&Discovery::shared_configs(
                &context.mixin_configs,
                &context.source_sets,
                &context.root,
            ))
        }))
    }
    pub fn collect_registered_mixin_sources<'a>(
        context: &'a crate::project::ProjectContext,
        source: &str,
        uri: &str,
        provider: &'a dyn HostMixinSourceProvider,
    ) -> Box<dyn Iterator<Item = Result<String, HostFacadeError>> + 'a> {
        let current_class = source_class_name(source);
        let package = source_package_name(source);
        let current_fqn =
            current_class.map(|c| package.as_ref().map_or(c.clone(), |p| format!("{p}.{c}")));
        let current_path = crate::project::uri_to_path(uri)
            .ok()
            .map(|p| crate::project::normalize_path(&p));
        let configs = Self::mixin_config_refs(context, Some(uri));
        let mut config_seen = BTreeSet::new();
        let mut fqn_seen = BTreeSet::new();
        let mut text_seen = BTreeSet::new();
        let entries = configs
            .into_iter()
            .filter(move |c| config_seen.insert(crate::project::normalize_path(&c.path)))
            .flat_map(move |config| {
                let entries = match provider.config(&config.path) {
                    Ok(crate::host_source::HostAnswer {
                        fact: HostFact::Known(config),
                        ..
                    }) => {
                        let package = config
                            .package_name
                            .as_deref()
                            .map(str::trim)
                            .filter(|p| !p.is_empty());
                        config
                            .mixins
                            .iter()
                            .chain(&config.client)
                            .chain(&config.server)
                            .chain(&config.common)
                            .map(|name| name.trim())
                            .filter(|name| !name.is_empty())
                            .map(|name| {
                                Ok(package.map_or_else(|| name.into(), |p| format!("{p}.{name}")))
                            })
                            .collect::<Vec<Result<String, HostFacadeError>>>()
                    }
                    Ok(crate::host_source::HostAnswer {
                        fact: HostFact::Missing,
                        ..
                    }) => vec![],
                    Ok(crate::host_source::HostAnswer {
                        fact: HostFact::Unavailable { reason },
                        ..
                    }) => vec![Err(HostFacadeError::Unavailable(reason))],
                    Err(e) => vec![Err(e.into())],
                };
                entries.into_iter()
            });
        Box::new(entries.filter_map(move |entry| {
            let fqn = match entry {
                Ok(fqn) => fqn,
                Err(e) => return Some(Err(e)),
            };
            if current_fqn.as_deref() == Some(&fqn) || !fqn_seen.insert(fqn.clone()) {
                return None;
            }
            let answer = match provider.compilation_unit_source(&fqn, current_path.as_deref()) {
                Ok(a) => a,
                Err(e) => return Some(Err(e.into())),
            };
            let text = match answer.fact {
                HostFact::Known(RegisteredMixinSource::Current) => return None,
                HostFact::Known(RegisteredMixinSource::Source(text)) => Some(Ok(text)),
                HostFact::Unavailable { reason } => Some(Err(HostFacadeError::Unavailable(reason))),
                HostFact::Missing => {
                    let relative = fqn.split('$').next().unwrap_or(&fqn).replace('.', "/");
                    let mut seen = BTreeSet::new();
                    let mut selected = None;
                    'roots: for root in context
                        .source_sets
                        .iter()
                        .flat_map(|s| &s.source_directories)
                    {
                        for ext in ["java", "kt", "kts"] {
                            let path = crate::project::normalize_path(
                                &root.join(format!("{relative}.{ext}")),
                            );
                            if current_path.as_ref() == Some(&path) || !seen.insert(path.clone()) {
                                continue;
                            }
                            match provider.read_source_file(&path) {
                                Ok(crate::host_source::HostAnswer {
                                    fact: HostFact::Known(text),
                                    ..
                                }) if !text.trim().is_empty() => {
                                    selected = Some(Ok(text));
                                    break 'roots;
                                }
                                Ok(crate::host_source::HostAnswer {
                                    fact: HostFact::Unavailable { reason },
                                    ..
                                }) => {
                                    selected = Some(Err(HostFacadeError::Unavailable(reason)));
                                    break 'roots;
                                }
                                Err(e) => {
                                    selected = Some(Err(e.into()));
                                    break 'roots;
                                }
                                _ => {}
                            }
                        }
                    }
                    selected
                }
            };
            match text {
                Some(Ok(text)) if !text.trim().is_empty() && text_seen.insert(text.clone()) => {
                    Some(Ok(text))
                }
                Some(Err(e)) => Some(Err(e)),
                _ => None,
            }
        }))
    }
}
fn source_class_name(source: &str) -> Option<String> {
    regex::Regex::new(r"\bclass\s+(\w+)")
        .unwrap()
        .captures(source)
        .map(|c| c[1].into())
}
fn source_package_name(source: &str) -> Option<String> {
    regex::Regex::new(r"(?m)^\s*package\s+([\w.]+)\s*;?")
        .unwrap()
        .captures(source)
        .map(|c| c[1].into())
}
