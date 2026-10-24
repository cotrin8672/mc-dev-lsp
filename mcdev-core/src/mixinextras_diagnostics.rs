//! Diagnostics composed from the shared MixinExtras source, signature and engine services.
use crate::{
    bytecode::*, extras_model::*, mixin_source::*, mixinextras_expression::*,
    mixinextras_handler::*, mixinextras_operation::*, *,
};
use std::collections::BTreeMap;
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MixinExtrasDiagnosticRequest {
    pub source: String,
    pub document_uri: String,
    #[serde(default)]
    pub resolved_contexts: Vec<ResolvedMixinExtrasContext>,
}
pub struct MixinExtrasDiagnosticsService<'a> {
    pub class_index: &'a dyn ClassIndex,
    pub bytecode_index: &'a dyn BytecodeIndex,
    pub signature_service: HandlerSignatureService<'a>,
}
impl<'a> MixinExtrasDiagnosticsService<'a> {
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
    pub fn analyze(&self, request: &MixinExtrasDiagnosticRequest) -> Vec<McDiagnostic> {
        let source = &request.source;
        let sites = HandlerSignatureService::find_annotation_sites(source);
        let mut diagnostics = self.share_conflicts(
            source,
            &HandlerSignatureService::find_sugar_handler_annotation_sites(source),
        );
        diagnostics.extend(self.definition_diagnostics(request));
        for site in sites {
            let targets = site_targets(source, &site, self.class_index);
            let selected = select_resolved_mixin_extras_context(&site, &request.resolved_contexts);
            let context = selected.map(|r| &r.context);
            if site
                .at_value
                .as_deref()
                .is_some_and(|v| v.eq_ignore_ascii_case("MIXINEXTRAS:EXPRESSION"))
            {
                let owned;
                let expression = if let Some(context) = context {
                    context
                } else {
                    owned = ExpressionContextResolver::resolve_expression_context(source, &site);
                    &owned
                };
                let values = expression.values_for_at_id_utf16(site.at_id.as_deref());
                if values.is_empty() {
                    diagnostics.push(diagnostic(
                        diagnostic_codes::MISSING_EXPRESSION_ANNOTATION,
                        McSeverity::Error,
                        "MixinExtras expression handler requires @Expression annotation".into(),
                        site.handler_method
                            .as_ref()
                            .map_or(site.annotation_range, |h| h.range),
                        BTreeMap::new(),
                    ));
                } else if site.annotation == MixinExtrasAnnotation::ModifyExpressionValue {
                    if let Some(target) = self
                        .signature_service
                        .resolve_target_method(&targets, &site.method_attribute)
                    {
                        if self
                            .signature_service
                            .infer_expression_value_type(source, &site, &target, &targets, context)
                            .is_none()
                        {
                            diagnostics.push(diagnostic(
                                diagnostic_codes::UNSUPPORTED_EXPRESSION_CONTEXT,
                                McSeverity::Warning,
                                format!(
                                    "Could not infer expression type for: {}",
                                    values
                                        .iter()
                                        .map(|units| String::from_utf16(units).unwrap_or_else(
                                            |_| crate::bytecode::ConstantValue::from_utf16(
                                                units.clone()
                                            )
                                            .java_string_literal()
                                            .unwrap()
                                        ))
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                ),
                                site.handler_method
                                    .as_ref()
                                    .map_or(site.annotation_range, |h| h.range),
                                BTreeMap::new(),
                            ));
                        }
                    }
                }
            }
            if site.annotation == MixinExtrasAnnotation::WrapWithCondition {
                let status = self.signature_service.wrap_with_condition_target_status(
                    &site,
                    &targets,
                    Some(source),
                    context,
                );
                if let Some(d) = condition_diagnostic(&site, status) {
                    diagnostics.push(d);
                }
            }
            let Some(handler) = site.handler_method.as_ref() else {
                continue;
            };
            let enriched = HandlerSignatureService::enrich_handler_types_with_reference_types(
                handler,
                self.class_index,
                Some(source),
                self.signature_service.reference_types,
            );
            if matches!(
                site.annotation,
                MixinExtrasAnnotation::WrapOperation | MixinExtrasAnnotation::WrapMethod
            ) {
                let expected = self
                    .signature_service
                    .expected_signature(source, &site, &targets, context);
                let layout = OperationCallValidator::resolve_operation_call_layout(
                    &site,
                    &enriched,
                    expected.as_ref(),
                );
                for issue in OperationCallValidator::validate(source, &site, layout.as_ref(), None)
                {
                    let mut metadata = site_metadata(&site);
                    metadata.insert("expectedArguments".into(), issue.expected_names.join(", "));
                    diagnostics.push(diagnostic(
                        diagnostic_codes::WRONG_OPERATION_CALL_ARGUMENTS,
                        McSeverity::Error,
                        "Operation.call called with the wrong number of arguments".into(),
                        issue.argument_range,
                        metadata,
                    ));
                }
            }
            for issue in self
                .signature_service
                .validate_handler(source, &site, &targets, &enriched, context)
            {
                diagnostics.push(diagnostic(
                    &issue.code,
                    McSeverity::Error,
                    issue.message,
                    issue.range,
                    site_metadata(&site),
                ));
            }
        }
        let mut result = vec![];
        for d in diagnostics {
            if !result.contains(&d) {
                result.push(d);
            }
        }
        result
    }
    fn definition_diagnostics(&self, request: &MixinExtrasDiagnosticRequest) -> Vec<McDiagnostic> {
        let resolver = ClassLiteralTypeNameResolver::for_source(&request.source, self.class_index);
        let mut result = vec![];
        for site in HandlerSignatureService::find_sugar_handler_annotation_sites(&request.source) {
            let owned;
            let context = if let Some(resolved) =
                select_resolved_mixin_extras_context(&site, &request.resolved_contexts)
            {
                &resolved.context
            } else {
                owned =
                    ExpressionContextResolver::resolve_expression_context(&request.source, &site);
                &owned
            };
            for definition in &context.definition_index.definitions {
                let range = definition
                    .id_source_range
                    .as_ref()
                    .or(definition.source_range.as_ref())
                    .map(|r| source_range(&request.source, r.start, r.end))
                    .unwrap_or(site.annotation_range);
                let index = MixinExtrasDefinitionIndex {
                    definitions: vec![definition.clone()],
                };
                let inputs = definition_inputs(&index);
                let built = crate::expression_engine::build_identifier_pool(&inputs, &|name| {
                    resolver.resolve(name)
                });
                for issue in built.issues {
                    result.push(diagnostic(
                        diagnostic_codes::INVALID_DEFINITION,
                        McSeverity::Warning,
                        format!(
                            "Cannot resolve @Definition {}: {} ({})",
                            issue.attribute, issue.message, issue.raw_value
                        ),
                        issue
                            .source_range
                            .map(|r| source_range(&request.source, r.start, r.end))
                            .unwrap_or(range),
                        BTreeMap::new(),
                    ));
                }
            }
        }
        result
    }
    fn share_conflicts(
        &self,
        source: &str,
        sites: &[MixinExtrasAnnotationSite],
    ) -> Vec<McDiagnostic> {
        let imports = JavaTypeDescriptorResolver::imports_for(source);
        let mut claims: Vec<(String, Vec<(McTextRange, String)>)> = vec![];
        for site in sites {
            let Some(scope) =
                position_to_offset(source, site.annotation_range.start).and_then(|offset| {
                    AnnotationContextExtractor::resolve_mixin_class_scope(source, offset)
                })
            else {
                continue;
            };
            let targets = MixinTargetResolver::resolve_targets(
                &scope.raw_targets,
                self.class_index,
                Some(&imports),
            );
            let Some(target) = self
                .signature_service
                .resolve_target_method(&targets, &site.method_attribute)
            else {
                continue;
            };
            let Some(handler) = site.handler_method.as_ref() else {
                continue;
            };
            let enriched = HandlerSignatureService::enrich_handler_types_with_reference_types(
                handler,
                self.class_index,
                Some(source),
                self.signature_service.reference_types,
            );
            for owner in targets {
                if self
                    .class_index
                    .get_methods(&owner)
                    .iter()
                    .filter(|m| m.name == target.name && m.descriptor == target.descriptor)
                    .count()
                    != 1
                {
                    continue;
                }
                for parameter in &enriched.parameters {
                    let Some(HandlerParameterSugarSpec::Share(share)) = &parameter.sugar_spec
                    else {
                        continue;
                    };
                    let Some(value) = &share.value else {
                        continue;
                    };
                    let namespace = if share.namespace_specified {
                        let Some(namespace) = share.namespace.as_ref() else {
                            continue;
                        };
                        namespace
                    } else {
                        if scope.class_name.trim().is_empty() {
                            continue;
                        }
                        &scope.class_name
                    };
                    let Some(descriptor) = parameter
                        .type_descriptor
                        .as_deref()
                        .and_then(HandlerSignatureService::share_ref_value_descriptor)
                    else {
                        continue;
                    };
                    let Some(range) = parameter.sugar_annotation_range else {
                        continue;
                    };
                    let key = format!(
                        "{owner}\0{}\0{}\0{namespace}\0{value}",
                        target.name, target.descriptor
                    );
                    let at = claims
                        .iter()
                        .position(|(k, _)| k == &key)
                        .unwrap_or_else(|| {
                            claims.push((key, vec![]));
                            claims.len() - 1
                        });
                    let entries = &mut claims[at].1;
                    if let Some((_, d)) = entries.iter_mut().find(|(r, _)| r == &range) {
                        *d = descriptor.into();
                    } else {
                        entries.push((range, descriptor.into()));
                    }
                }
            }
        }
        claims
            .into_iter()
            .filter_map(|(_, claims)| {
                if claims
                    .iter()
                    .map(|(_, d)| d)
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    < 2
                {
                    return None;
                }
                let range = claims
                    .iter()
                    .map(|(r, _)| *r)
                    .max_by_key(|r| (r.start.line, r.start.character))?;
                Some(diagnostic(
                    diagnostic_codes::SHARE_TYPE_CONFLICT,
                    McSeverity::Error,
                    "Conflicting @Share reference types for the same target".into(),
                    range,
                    BTreeMap::new(),
                ))
            })
            .collect()
    }
}
pub(crate) fn site_targets(
    source: &str,
    site: &MixinExtrasAnnotationSite,
    index: &dyn ClassIndex,
) -> Vec<String> {
    position_to_offset(source, site.annotation_range.start)
        .and_then(|offset| AnnotationContextExtractor::resolve_mixin_class_scope(source, offset))
        .map(|scope| {
            MixinTargetResolver::resolve_targets(
                &scope.raw_targets,
                index,
                Some(&JavaTypeDescriptorResolver::imports_for(source)),
            )
        })
        .unwrap_or_default()
}
fn diagnostic(
    code: &str,
    severity: McSeverity,
    message: String,
    range: McTextRange,
    metadata: BTreeMap<String, String>,
) -> McDiagnostic {
    McDiagnostic {
        code: code.into(),
        severity,
        message,
        range,
        metadata,
    }
}
pub(crate) fn site_metadata(site: &MixinExtrasAnnotationSite) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("annotation".into(), site.annotation.simple_name().into()),
        ("method".into(), site.method_attribute.clone()),
    ])
}
fn condition_diagnostic(
    site: &MixinExtrasAnnotationSite,
    status: WrapWithConditionTargetStatus,
) -> Option<McDiagnostic> {
    use WrapWithConditionTargetStatus::*;
    let(code,severity,message)=match status{ValidVoid|Unresolved=>return None,ValidPoppedNonVoid=>(diagnostic_codes::WRAP_WITH_CONDITION_POPPED_NON_VOID,McSeverity::Warning,"WrapWithCondition is targeting a non-void instruction"),InvalidRetainedNonVoid=>(diagnostic_codes::WRAP_WITH_CONDITION_INVALID_TARGET,McSeverity::Error,"WrapWithCondition cannot target a retained non-void invocation"),InvalidInstruction=>(diagnostic_codes::WRAP_WITH_CONDITION_INVALID_TARGET,McSeverity::Error,"WrapWithCondition only supports void method invocations, immediately popped invocations, and field writes")};
    Some(diagnostic(
        code,
        severity,
        message.into(),
        site.annotation_range,
        site_metadata(site),
    ))
}
