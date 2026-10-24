//! Handler fixes and completion composed from shared source/signature/import services.
use crate::{
    extras_model::*, mixin_edits::MixinImportEditBuilder, mixin_source::*, mixinextras_handler::*,
    mixinextras_operation::*, *,
};
use std::collections::{BTreeMap, BTreeSet};
const OPERATION: &str = "com/llamalad7/mixinextras/injector/wrapoperation/Operation";
const MARKER: &str = "__MCDEV_HANDLER_NAME__";
const CANCELLABLE_MESSAGE: &str = "Cancellable parameter type should be ";
pub struct MixinExtrasCodeActionService<'a> {
    pub class_index: &'a dyn ClassIndex,
    pub signature_service: HandlerSignatureService<'a>,
}
#[derive(Clone)]
struct Plan {
    text: String,
    imports: BTreeSet<String>,
}
struct Rendered {
    text: String,
    imports: BTreeSet<String>,
}
struct RenderedSignature {
    return_type: String,
    parameters: Vec<String>,
    imports: BTreeSet<String>,
}
impl<'a> MixinExtrasCodeActionService<'a> {
    pub fn new(class_index: &'a dyn ClassIndex) -> Self {
        Self {
            class_index,
            signature_service: HandlerSignatureService::new(class_index, None),
        }
    }
    pub fn with_signature_service(mut self, service: HandlerSignatureService<'a>) -> Self {
        self.signature_service = service;
        self
    }
    fn enrich(&self, handler: &HandlerMethodDeclaration, source: &str) -> HandlerMethodDeclaration {
        HandlerSignatureService::enrich_handler_types_with_reference_types(
            handler,
            self.class_index,
            Some(source),
            self.signature_service.reference_types,
        )
    }
    pub fn fixes_for_diagnostics(
        &self,
        diagnostics: &[McDiagnostic],
        uri: &str,
        source: &str,
        contexts: &[ResolvedMixinExtrasContext],
    ) -> Vec<McFix> {
        let targets = MixinTargetResolver::resolve_targets_from_source(source, self.class_index);
        let sites = HandlerSignatureService::find_annotation_sites(source);
        let sugar = HandlerSignatureService::find_sugar_handler_annotation_sites(source);
        let mut fixes = vec![];
        for diagnostic in diagnostics {
            if diagnostic.code == diagnostic_codes::HANDLER_SIGNATURE_MISMATCH
                && diagnostic.message.starts_with(CANCELLABLE_MESSAGE)
            {
                if let Some(fix) = self.cancellable_fix(uri, source, diagnostic, &sugar) {
                    fixes.push(fix);
                }
                continue;
            }
            if diagnostic.code == diagnostic_codes::WRONG_OPERATION_CALL_ARGUMENTS {
                if let Some(fix) = self.operation_fix(uri, source, diagnostic, &targets, contexts) {
                    fixes.push(fix);
                }
                continue;
            }
            let Some(site) = site_for_diagnostic(&sites, diagnostic) else {
                continue;
            };
            if [
                diagnostic_codes::HANDLER_SIGNATURE_MISMATCH,
                diagnostic_codes::MISSING_OPERATION_PARAMETER,
                diagnostic_codes::WRONG_OPERATION_GENERIC,
                diagnostic_codes::WRONG_ORIGINAL_VALUE_TYPE,
                diagnostic_codes::WRONG_RETURN_TYPE,
            ]
            .contains(&diagnostic.code.as_str())
            {
                if let Some(fix) = self.signature_fix(uri, source, site, &sites, &targets, contexts)
                {
                    fixes.push(fix);
                }
            }
            if site.handler_method.is_none() {
                if let Some(fix) = self.generate_fix(uri, source, site, &sites, &targets, contexts)
                {
                    fixes.push(fix);
                }
            }
        }
        Self::deduplicate_fixes(&fixes)
    }
    pub fn generate_handler_fixes(
        &self,
        uri: &str,
        source: &str,
        site: &MixinExtrasAnnotationSite,
        targets: &[String],
        contexts: &[ResolvedMixinExtrasContext],
    ) -> Vec<McFix> {
        self.generate_fix(
            uri,
            source,
            site,
            &HandlerSignatureService::find_annotation_sites(source),
            targets,
            contexts,
        )
        .into_iter()
        .collect()
    }
    pub fn generate_handler_fixes_at_position(
        &self,
        uri: &str,
        source: &str,
        line: usize,
        character: usize,
        targets: &[String],
        contexts: &[ResolvedMixinExtrasContext],
    ) -> Vec<McFix> {
        let Some(offset) = position_to_offset(source, McTextPosition { line, character }) else {
            return vec![];
        };
        let mut fixes = vec![];
        for site in HandlerSignatureService::find_annotation_sites(source) {
            if site.handler_method.is_some() {
                continue;
            }
            let Some((start, end)) = range_offsets(source, site.annotation_range) else {
                continue;
            };
            if start <= offset && offset <= end {
                fixes.extend(self.generate_handler_fixes(uri, source, &site, targets, contexts));
            }
        }
        Self::deduplicate_fixes(&fixes)
    }
    pub fn complete_handler(
        &self,
        source: &str,
        annotation_start: usize,
        targets: &[String],
        contexts: &[ResolvedMixinExtrasContext],
        cursor: Option<usize>,
    ) -> Vec<McCompletionItem> {
        let sites = HandlerSignatureService::find_sugar_handler_annotation_sites(source);
        let selected: Vec<_> = sites
            .iter()
            .filter(|site| {
                site.handler_method.is_none()
                    && position_to_offset(source, site.annotation_range.start)
                        == Some(annotation_start)
            })
            .collect();
        if selected.is_empty() {
            return vec![];
        }
        let mut plans = vec![];
        for site in &selected {
            if let Some(plan) =
                self.agreed_plan(source, site, &sites, targets, contexts, Some(MARKER))
            {
                if !plans.iter().any(|p: &Plan| p.text == plan.text) {
                    plans.push(plan);
                }
            }
        }
        if plans.len() != 1 {
            return vec![];
        }
        let plan = plans.pop().unwrap();
        let name = selected[0].annotation.simple_name();
        let end = annotation_end(source, annotation_start);
        vec![McCompletionItem {
            label: format!("Generate {name} handler"),
            detail: Some("Insert the inferred handler signature".into()),
            documentation: Some(format!(
                "Generate a {name} handler and edit its method name."
            )),
            filter_text: format!("{name} handler"),
            insert_text: handler_snippet(&plan.text, source, end, cursor.unwrap_or(end)),
            kind: McCompletionKind::Method,
            sort_key: format!("0100_handler_{name}"),
            metadata: McCompletionMetadata {
                source: "mixin.handler".into(),
                name: Some(name.into()),
                ..Default::default()
            },
            additional_edits: build_import_edits(source, &plan.imports),
            insert_text_format: McCompletionInsertTextFormat::Snippet,
        }]
    }
    /// Legacy materialization is a pass-through; application must use checked edits.
    pub fn apply_handler_fix(&self, fix: &McFix, _current_source: &str) -> Option<McFix> {
        matches!(fix, McFix::WorkspaceEditFix { .. }).then(|| fix.clone())
    }
    pub fn deduplicate_fixes(fixes: &[McFix]) -> Vec<McFix> {
        let mut result: Vec<McFix> = vec![];
        for fix in fixes {
            if !result.iter().any(|other| match (fix, other) {
                (
                    McFix::WorkspaceEditFix {
                        document_uri: a,
                        edits: x,
                        ..
                    },
                    McFix::WorkspaceEditFix {
                        document_uri: b,
                        edits: y,
                        ..
                    },
                ) => a == b && x == y,
                (McFix::WorkspaceEditFix { .. }, _) | (_, McFix::WorkspaceEditFix { .. }) => false,
                _ => fix.title() == other.title() && fix.kind() == other.kind(),
            }) {
                result.push(fix.clone());
            }
        }
        result
    }
    fn agreed_plan(
        &self,
        source: &str,
        site: &MixinExtrasAnnotationSite,
        sites: &[MixinExtrasAnnotationSite],
        targets: &[String],
        contexts: &[ResolvedMixinExtrasContext],
        handler_name: Option<&str>,
    ) -> Option<Plan> {
        let related = related_sites(site, sites);
        let name = handler_name
            .or_else(|| site.handler_method.as_ref().map(|h| h.method_name.as_str()))
            .unwrap_or("mcdevHandler");
        let mut result: Option<Plan> = None;
        for site in related {
            let context = selected_context(site, contexts);
            let mut expected = self
                .signature_service
                .expected_signature(source, site, targets, context)?;
            let rendered = render_signature(source, &expected);
            expected.readable_return_type = rendered.return_type;
            for (p, text) in expected.parameters.iter_mut().zip(rendered.parameters) {
                p.readable_type = text;
            }
            let text = self.signature_service.generate_handler_stub(
                source,
                site,
                targets,
                name,
                "    ",
                context,
                Some(&expected),
            )?;
            if let Some(previous) = result.as_mut() {
                if previous.text != text {
                    return None;
                }
                previous.imports.extend(rendered.imports);
            } else {
                result = Some(Plan {
                    text,
                    imports: rendered.imports,
                });
            }
        }
        result
    }
    fn generate_fix(
        &self,
        uri: &str,
        source: &str,
        site: &MixinExtrasAnnotationSite,
        sites: &[MixinExtrasAnnotationSite],
        targets: &[String],
        contexts: &[ResolvedMixinExtrasContext],
    ) -> Option<McFix> {
        let plan = self.agreed_plan(source, site, sites, targets, contexts, None)?;
        let offset = position_to_offset(source, site.annotation_range.end)?;
        let title = if site.annotation.is_injector() {
            format!("Generate {} handler", site.annotation.simple_name())
        } else {
            "Generate MixinExtras handler".into()
        };
        let mut edits = vec![McTextEdit {
            start_offset: offset,
            end_offset: offset,
            new_text: format!("\n{}", plan.text),
        }];
        edits.extend(build_import_edits(source, &plan.imports));
        Some(workspace(
            &title,
            "quickfix.mixinextras.generateHandler",
            uri,
            edits,
            BTreeMap::from([("annotation".into(), site.annotation.simple_name().into())]),
        ))
    }
    fn signature_fix(
        &self,
        uri: &str,
        source: &str,
        site: &MixinExtrasAnnotationSite,
        sites: &[MixinExtrasAnnotationSite],
        targets: &[String],
        contexts: &[ResolvedMixinExtrasContext],
    ) -> Option<McFix> {
        let handler = site.handler_method.as_ref()?;
        self.agreed_plan(
            source,
            site,
            sites,
            targets,
            contexts,
            Some(&handler.method_name),
        )?;
        let (start, end) = range_offsets(source, handler.range)?;
        let return_start = start + source.get(start..)?.find(&handler.return_type_name)?;
        if return_start >= end {
            return None;
        }
        let mut result: Option<Plan> = None;
        for related in related_sites(site, sites) {
            let handler = related.handler_method.as_ref()?;
            let expected = self.signature_service.expected_signature(
                source,
                related,
                targets,
                selected_context(related, contexts),
            )?;
            let current = self.signature_with_existing(source, handler, &expected)?;
            if let Some(previous) = result.as_mut() {
                if previous.text != current.text {
                    return None;
                }
                previous.imports.extend(current.imports);
            } else {
                result = Some(current);
            }
        }
        let plan = result?;
        let title = if site.annotation.is_injector() {
            format!("Fix {} handler signature", site.annotation.simple_name())
        } else {
            "Fix MixinExtras handler signature".into()
        };
        let mut edits = vec![McTextEdit {
            start_offset: return_start,
            end_offset: end,
            new_text: plan.text,
        }];
        edits.extend(build_import_edits(source, &plan.imports));
        Some(workspace(
            &title,
            "quickfix.mixinextras.fixHandlerSignature",
            uri,
            edits,
            BTreeMap::from([("annotation".into(), site.annotation.simple_name().into())]),
        ))
    }
    fn signature_with_existing(
        &self,
        source: &str,
        handler: &HandlerMethodDeclaration,
        expected: &HandlerSignatureSpec,
    ) -> Option<Plan> {
        let actual = self.enrich(handler, source);
        let expected = self
            .signature_service
            .specialize_int_like_signature(expected, &actual);
        let rendered = render_signature(source, &expected);
        let params = &actual.parameters;
        if let Some(first) = params.iter().position(|p| p.is_sugar) {
            if params[first..].iter().any(|p| !p.is_sugar) {
                return None;
            }
        }
        let base: Vec<_> = params.iter().filter(|p| !p.is_sugar).collect();
        let required = &expected.parameters;
        let required_actual: Vec<_> = base.iter().take(required.len()).copied().collect();
        if required_actual
            .iter()
            .zip(required)
            .any(|(a, e)| a.is_operation != e.is_operation)
        {
            return None;
        }
        let captured = base.get(required.len()..).unwrap_or(&[]);
        let sugar: Vec<_> = params.iter().filter(|p| p.is_sugar).collect();
        if captured.len() > expected.optional_captured_target_parameters.len()
            || captured
                .iter()
                .zip(&expected.optional_captured_target_parameters)
                .any(|(a, e)| !self.signature_service.parameter_type_matches(e, a))
        {
            return None;
        }
        let required_names: Vec<_> = required
            .iter()
            .enumerate()
            .map(|(i, e)| {
                required_actual
                    .get(i)
                    .map_or_else(|| e.name.clone(), |a| a.name.clone())
            })
            .collect();
        let names: Vec<_> = required_names
            .iter()
            .map(String::as_str)
            .chain(captured.iter().map(|p| p.name.as_str()))
            .chain(sugar.iter().map(|p| p.name.as_str()))
            .collect();
        if names.len() != names.iter().copied().collect::<BTreeSet<_>>().len() {
            return None;
        }
        let mut declarations = vec![];
        for (i, expected) in required.iter().enumerate() {
            let preserved = if let Some(actual) = required_actual
                .get(i)
                .filter(|actual| !expected.is_operation && actual.has_coerce)
            {
                if self
                    .signature_service
                    .parameter_match_knowledge(expected, actual)?
                {
                    parameter_source(source, actual)
                } else {
                    None
                }
            } else {
                None
            };
            declarations.push(
                preserved
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("{} {}", rendered.parameters[i], required_names[i])),
            );
        }
        for p in captured.iter().copied().chain(sugar) {
            declarations.push(parameter_source(source, p)?.into());
        }
        let preserve_return = if actual.return_type_descriptor.is_some()
            && actual.return_type_descriptor.as_deref() != Some(&expected.return_type_descriptor)
        {
            self.signature_service.type_match_knowledge(
                actual.return_type_descriptor.as_deref(),
                &expected.return_type_descriptor,
                actual.has_method_level_coerce,
                &expected.accepted_return_type_descriptors,
            )?
        } else {
            false
        };
        let return_type = if preserve_return {
            handler.return_type_name.as_str()
        } else {
            rendered.return_type.as_str()
        };
        Some(Plan {
            text: format!(
                "{return_type} {}({})",
                handler.method_name,
                declarations.join(", ")
            ),
            imports: rendered.imports,
        })
    }
    fn operation_fix(
        &self,
        uri: &str,
        source: &str,
        diagnostic: &McDiagnostic,
        targets: &[String],
        contexts: &[ResolvedMixinExtrasContext],
    ) -> Option<McFix> {
        let sites = HandlerSignatureService::find_annotation_sites(source);
        let mut matching_sites = vec![];
        let mut issues = vec![];
        for site in &sites {
            if !matches!(
                site.annotation,
                MixinExtrasAnnotation::WrapOperation | MixinExtrasAnnotation::WrapMethod
            ) {
                continue;
            }
            let Some(handler) = site.handler_method.as_ref() else {
                continue;
            };
            let enriched = self.enrich(handler, source);
            let expected = self.signature_service.expected_signature(
                source,
                site,
                targets,
                selected_context(site, contexts),
            );
            let layout = OperationCallValidator::resolve_operation_call_layout(
                site,
                &enriched,
                expected.as_ref(),
            );
            let matches: Vec<_> =
                OperationCallValidator::validate(source, site, layout.as_ref(), None)
                    .into_iter()
                    .filter(|i| i.argument_range == diagnostic.range)
                    .collect();
            if !matches.is_empty() {
                matching_sites.push(site);
            }
            for issue in matches {
                if !issues.contains(&issue) {
                    issues.push(issue);
                }
            }
        }
        if issues.len() != 1 {
            return None;
        }
        let site = matching_sites.first()?;
        self.agreed_plan(source, site, &sites, targets, contexts, None)?;
        let (start, end) = range_offsets(source, issues[0].argument_range)?;
        Some(workspace(
            "Fix Operation.call arguments",
            "quickfix.mixinextras.fixOperationCallArguments",
            uri,
            vec![McTextEdit {
                start_offset: start,
                end_offset: end,
                new_text: issues[0].expected_names.join(", "),
            }],
            BTreeMap::new(),
        ))
    }
    fn cancellable_fix(
        &self,
        uri: &str,
        source: &str,
        diagnostic: &McDiagnostic,
        sites: &[MixinExtrasAnnotationSite],
    ) -> Option<McFix> {
        let mut matches = vec![];
        for site in sites {
            let Some(handler) = site.handler_method.as_ref() else {
                continue;
            };
            let indices: Vec<_> = handler
                .parameters
                .iter()
                .enumerate()
                .filter(|(_, p)| {
                    p.sugar_spec == Some(HandlerParameterSugarSpec::Cancellable)
                        && p.sugar_annotation_range == Some(diagnostic.range)
                })
                .map(|(i, _)| i)
                .collect();
            if !indices.is_empty() {
                matches.push((site, handler, indices));
            }
        }
        if matches.is_empty() {
            return None;
        }
        let mut expected_types = BTreeSet::new();
        let mut resolved = vec![];
        for (site, handler, indices) in matches {
            let targets =
                crate::mixinextras_diagnostics::site_targets(source, site, self.class_index);
            let target = self
                .signature_service
                .resolve_target_method(&targets, &site.method_attribute)?;
            let descriptor = if target.descriptor.rsplit(')').next() == Some("V") {
                "Lorg/spongepowered/asm/mixin/injection/callback/CallbackInfo;"
            } else {
                "Lorg/spongepowered/asm/mixin/injection/callback/CallbackInfoReturnable;"
            };
            let fqn = &descriptor[1..descriptor.len() - 1].replace('/', ".");
            expected_types.insert((descriptor.to_string(), fqn.clone()));
            resolved.push((site, handler, indices, target, descriptor, fqn.clone()));
        }
        if expected_types.len() != 1 {
            return None;
        }
        let mut edits = vec![];
        for (site, handler, indices, target, descriptor, fqn) in resolved {
            let enriched = self.enrich(handler, source);
            for index in indices {
                if !self
                    .signature_service
                    .validate_common_sugar_constraints(
                        &enriched,
                        Some(&target),
                        Some(site.annotation),
                    )
                    .iter()
                    .any(|i| {
                        i.code == diagnostic_codes::HANDLER_SIGNATURE_MISMATCH
                            && i.range == diagnostic.range
                            && i.message == diagnostic.message
                    })
                {
                    continue;
                }
                let Some(actual) = enriched.parameters.get(index) else {
                    continue;
                };
                if actual.type_descriptor.as_deref() == Some(descriptor) {
                    continue;
                }
                let Some((start, end)) = parameter_type_range(source, &handler.parameters[index])
                else {
                    continue;
                };
                let edit = McTextEdit {
                    start_offset: start,
                    end_offset: end,
                    new_text: fqn.clone(),
                };
                if !edits.contains(&edit) {
                    edits.push(edit);
                }
            }
        }
        if edits.len() != 1 {
            return None;
        }
        Some(workspace(
            "Fix @Cancellable parameter type",
            "quickfix.mixinextras.fixCancellableParameterType",
            uri,
            edits,
            BTreeMap::from([("annotation".into(), "Cancellable".into())]),
        ))
    }
}
fn selected_context<'a>(
    site: &MixinExtrasAnnotationSite,
    contexts: &'a [ResolvedMixinExtrasContext],
) -> Option<&'a ExpressionContext> {
    select_resolved_mixin_extras_context(site, contexts).map(|r| &r.context)
}
fn related_sites<'a>(
    site: &MixinExtrasAnnotationSite,
    sites: &'a [MixinExtrasAnnotationSite],
) -> Vec<&'a MixinExtrasAnnotationSite> {
    sites
        .iter()
        .filter(|other| {
            if let Some(handler) = &site.handler_method {
                other
                    .handler_method
                    .as_ref()
                    .is_some_and(|h| h.range == handler.range)
            } else {
                other.handler_method.is_none() && other.annotation_range == site.annotation_range
            }
        })
        .collect()
}
fn site_for_diagnostic<'a>(
    sites: &'a [MixinExtrasAnnotationSite],
    diagnostic: &McDiagnostic,
) -> Option<&'a MixinExtrasAnnotationSite> {
    let select = |matches: Vec<&'a MixinExtrasAnnotationSite>| {
        let first = *matches.first()?;
        let key = |site: &MixinExtrasAnnotationSite| {
            site.handler_method
                .as_ref()
                .map_or((false, site.annotation_range), |h| (true, h.range))
        };
        matches
            .iter()
            .all(|site| key(site) == key(first))
            .then_some(first)
    };
    let exact: Vec<_> = sites
        .iter()
        .filter(|s| {
            s.annotation_range == diagnostic.range
                || s.handler_method
                    .as_ref()
                    .is_some_and(|h| h.range == diagnostic.range)
        })
        .collect();
    if !exact.is_empty() {
        return select(exact);
    }
    if let Some(method) = diagnostic.metadata.get("method") {
        let matches: Vec<_> = sites
            .iter()
            .filter(|s| s.method_attribute == *method)
            .collect();
        if !matches.is_empty() {
            return select(matches);
        }
    }
    let name = diagnostic.metadata.get("annotation")?;
    select(
        sites
            .iter()
            .filter(|s| s.annotation.simple_name() == name)
            .collect(),
    )
}
fn workspace(
    title: &str,
    kind: &str,
    uri: &str,
    edits: Vec<McTextEdit>,
    metadata: BTreeMap<String, String>,
) -> McFix {
    McFix::WorkspaceEditFix {
        title: title.into(),
        kind: kind.into(),
        document_uri: uri.into(),
        edits,
        metadata,
    }
}
fn range_offsets(source: &str, range: McTextRange) -> Option<(usize, usize)> {
    let start = position_to_offset(source, range.start)?;
    let end = position_to_offset(source, range.end)?;
    (start <= end).then_some((start, end))
}
fn parameter_source<'a>(source: &'a str, p: &HandlerParameterDeclaration) -> Option<&'a str> {
    let (start, end) = range_offsets(source, p.range?)?;
    source.get(start..end)
}
fn parameter_type_range(source: &str, p: &HandlerParameterDeclaration) -> Option<(usize, usize)> {
    let (start, end) = range_offsets(source, p.range?)?;
    let search = p
        .sugar_annotation_range
        .and_then(|r| position_to_offset(source, r.end))
        .unwrap_or(start);
    let masked = AnnotationContextExtractor::mask_non_code(source);
    let bound = end.checked_sub(p.name.len())?;
    let name = masked.get(..bound + p.name.len())?.rfind(&p.name)?;
    if name < search {
        return None;
    }
    let type_start = masked.get(..name)?.rfind(&p.type_name)?;
    if type_start < search {
        return None;
    }
    let type_end = type_start + p.type_name.len();
    if type_end > name {
        return None;
    }
    let leading = if let Some(rest) = p.type_name.strip_prefix("final") {
        if rest.chars().next().is_some_and(char::is_whitespace) {
            p.type_name.len() - rest.trim_start().len()
        } else {
            0
        }
    } else {
        0
    };
    Some((type_start + leading, type_end))
}
fn annotation_end(source: &str, start: usize) -> usize {
    annotations(source)
        .into_iter()
        .find(|a| a.start == start)
        .map_or(start, |a| a.end)
}
fn internal_name(descriptor: Option<&str>) -> Option<&str> {
    let descriptor = descriptor?.trim_start_matches('[');
    descriptor.strip_prefix('L')?.strip_suffix(';')
}
fn descriptor_reference(
    source: &str,
    descriptor: Option<&str>,
    fallback: &str,
    candidates: &BTreeSet<String>,
    generic: Option<&str>,
) -> Rendered {
    let Some(internal) = internal_name(descriptor) else {
        return Rendered {
            text: fallback.into(),
            imports: BTreeSet::new(),
        };
    };
    let reference =
        MixinImportEditBuilder::reference_for_internal_name(source, internal, candidates);
    let mut imports = reference.import_fqn.into_iter().collect::<BTreeSet<_>>();
    let generic = generic
        .map(|d| descriptor_reference(source, Some(d), &boxed_readable(d), candidates, None));
    let text = format!(
        "{}{}{}",
        reference.text,
        generic
            .as_ref()
            .map(|g| format!("<{}>", g.text))
            .unwrap_or_default(),
        "[]".repeat(
            descriptor
                .unwrap()
                .bytes()
                .take_while(|b| *b == b'[')
                .count()
        )
    );
    if let Some(generic) = generic {
        imports.extend(generic.imports);
    }
    Rendered { text, imports }
}
fn boxed_readable(descriptor: &str) -> String {
    match descriptor {
        "Z" => "Boolean".into(),
        "B" => "Byte".into(),
        "C" => "Character".into(),
        "S" => "Short".into(),
        "I" => "Integer".into(),
        "J" => "Long".into(),
        "F" => "Float".into(),
        "D" => "Double".into(),
        _ => OperationSignatureRenderer::readable_type(descriptor),
    }
}
fn render_signature(source: &str, expected: &HandlerSignatureSpec) -> RenderedSignature {
    let mut candidates = BTreeSet::new();
    if let Some(name) = internal_name(Some(&expected.return_type_descriptor)) {
        candidates.insert(name.into());
    }
    for p in &expected.parameters {
        if p.is_operation {
            candidates.insert(OPERATION.into());
        }
        for descriptor in [
            Some(p.type_descriptor.as_str()),
            p.generic_type_descriptor.as_deref(),
            if p.is_operation {
                p.operation_generic_descriptor.as_deref()
            } else {
                None
            },
        ] {
            if let Some(name) = internal_name(descriptor) {
                candidates.insert(name.into());
            }
        }
    }
    let mut parameters = vec![];
    let mut imports = BTreeSet::new();
    for p in &expected.parameters {
        let rendered = if p.is_operation {
            let operation =
                MixinImportEditBuilder::reference_for_internal_name(source, OPERATION, &candidates);
            let fallback = p
                .readable_type
                .split_once('<')
                .map_or(p.readable_type.as_str(), |(_, s)| s)
                .rsplit_once('>')
                .map_or_else(
                    || {
                        p.readable_type
                            .split_once('<')
                            .map_or(p.readable_type.as_str(), |(_, s)| s)
                    },
                    |(s, _)| s,
                );
            let generic = p
                .operation_generic_descriptor
                .as_deref()
                .map(|d| descriptor_reference(source, Some(d), fallback, &candidates, None));
            let mut imports = operation.import_fqn.into_iter().collect::<BTreeSet<_>>();
            if let Some(g) = &generic {
                imports.extend(g.imports.clone());
            }
            Rendered {
                text: format!(
                    "{}<{}>",
                    operation.text,
                    generic.map_or_else(|| fallback.into(), |g| g.text)
                ),
                imports,
            }
        } else {
            descriptor_reference(
                source,
                Some(&p.type_descriptor),
                &p.readable_type,
                &candidates,
                p.generic_type_descriptor.as_deref(),
            )
        };
        parameters.push(rendered.text);
        imports.extend(rendered.imports);
    }
    let ret = descriptor_reference(
        source,
        Some(&expected.return_type_descriptor),
        &expected.readable_return_type,
        &candidates,
        None,
    );
    imports.extend(ret.imports);
    RenderedSignature {
        return_type: ret.text,
        parameters,
        imports,
    }
}
fn build_import_edits(source: &str, fqns: &BTreeSet<String>) -> Vec<McTextEdit> {
    let edits: Vec<_> = fqns
        .iter()
        .filter_map(|f| MixinImportEditBuilder::build_import_edit(source, f))
        .collect();
    let Some(first) = edits.first() else {
        return vec![];
    };
    let leading = if first.new_text.starts_with('\n') {
        "\n"
    } else {
        ""
    };
    vec![McTextEdit {
        start_offset: first.start_offset,
        end_offset: first.end_offset,
        new_text: format!(
            "{leading}{}",
            edits
                .iter()
                .map(|e| e.new_text.trim_start_matches('\n'))
                .collect::<String>()
        ),
    }]
}
fn handler_snippet(text: &str, source: &str, annotation_end: usize, cursor: usize) -> String {
    let escaped = text.replace('$', "\\$").replace(MARKER, "${1}");
    let indent = escaped.len() - escaped.trim_start().len();
    let with_access = if escaped[indent..].starts_with("private ") {
        escaped
    } else {
        format!("{}private {}", &escaped[..indent], &escaped[indent..])
    };
    let with_stop = if let Some(closing) = with_access.rfind("\n    }\n") {
        let method_indent =
            &with_access[..with_access.len() - with_access.trim_start_matches([' ', '\t']).len()];
        let body_start = with_access.find('\n').map_or(closing, |n| n + 1);
        let body = &with_access[body_start..closing];
        let body = body
            .split('\n')
            .map(|line| {
                if line.trim().is_empty() {
                    line.into()
                } else {
                    format!("{method_indent}{line}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let content = if body.trim().is_empty() {
            format!("{method_indent}    $0")
        } else {
            format!("{method_indent}    $0\n{body}")
        };
        format!(
            "{}{content}{}",
            &with_access[..body_start],
            &with_access[closing..]
        )
    } else {
        format!("{with_access}$0")
    };
    let cursor = cursor.min(source.len());
    let end = annotation_end.min(cursor);
    let Some(after) = source.get(end..cursor) else {
        return with_stop;
    };
    if !after.contains(['\n', '\r']) {
        return format!("\n{with_stop}");
    }
    let start = source[..cursor].rfind('\n').map_or(0, |n| n + 1);
    let existing = &source[start..cursor];
    if !existing.is_empty() && existing.chars().all(|c| c == ' ' || c == '\t') {
        let generated =
            &with_stop[..with_stop.len() - with_stop.trim_start_matches([' ', '\t']).len()];
        if generated.starts_with(existing) {
            return with_stop[existing.len()..].into();
        }
    }
    with_stop
}
