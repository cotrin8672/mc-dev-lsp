//! Mixin diagnostics over host-supplied source, class facts and bytecode facts.
use crate::{
    bytecode::BytecodeIndex,
    descriptor::MemberTargetParser,
    mixin_completion::AtTargetCompletionService,
    mixin_edits::MixinConfigEditor,
    mixin_source::*,
    mixin_validation::{issue, OverwriteValidationService},
    model::*,
};

pub struct MixinDiagnosticCodes;
impl MixinDiagnosticCodes {
    pub const UNRESOLVED_MIXIN_TARGET: &'static str = "MIXIN_UNRESOLVED_TARGET";
    pub const MIXIN_CLASS_NOT_LISTED_IN_CONFIG: &'static str = "MIXIN_CLASS_NOT_LISTED_IN_CONFIG";
    pub const DUPLICATE_MIXIN_CONFIG_ENTRY: &'static str = "MIXIN_DUPLICATE_CONFIG_ENTRY";
    pub const DUPLICATE_MIXIN_TARGET: &'static str = "MIXIN_DUPLICATE_TARGET";
    pub const UNSUPPORTED_ANNOTATION: &'static str = "MIXIN_UNSUPPORTED_ANNOTATION";
    pub const INVALID_MIXIN_TARGET_STRING: &'static str = "MIXIN_INVALID_TARGET_STRING";
    pub const UNRESOLVED_INJECT_METHOD: &'static str = "MIXIN_UNRESOLVED_INJECT_METHOD";
    pub const AMBIGUOUS_INJECT_METHOD: &'static str = "MIXIN_AMBIGUOUS_INJECT_METHOD";
    pub const DESCRIPTOR_MISMATCH: &'static str = "MIXIN_DESCRIPTOR_MISMATCH";
    pub const UNRESOLVED_AT_TARGET: &'static str = "MIXIN_UNRESOLVED_AT_TARGET";
    pub const INVALID_AT_TARGET_DESCRIPTOR: &'static str = "MIXIN_INVALID_AT_TARGET_DESCRIPTOR";
    pub const AT_TARGET_NOT_PRESENT: &'static str = "MIXIN_AT_TARGET_NOT_PRESENT";
    pub const ORDINAL_OUT_OF_RANGE: &'static str = "MIXIN_ORDINAL_OUT_OF_RANGE";
    pub const SHADOW_TARGET_NOT_FOUND: &'static str = "MIXIN_SHADOW_TARGET_NOT_FOUND";
    pub const SHADOW_DESCRIPTOR_MISMATCH: &'static str = "MIXIN_SHADOW_DESCRIPTOR_MISMATCH";
    pub const SHADOW_STATIC_MISMATCH: &'static str = "MIXIN_SHADOW_STATIC_MISMATCH";
    pub const SHADOW_KIND_MISMATCH: &'static str = "MIXIN_SHADOW_KIND_MISMATCH";
    pub const ACCESSOR_FIELD_NOT_FOUND: &'static str = "MIXIN_ACCESSOR_FIELD_NOT_FOUND";
    pub const ACCESSOR_SIGNATURE_MISMATCH: &'static str = "MIXIN_ACCESSOR_SIGNATURE_MISMATCH";
    pub const INVOKER_METHOD_NOT_FOUND: &'static str = "MIXIN_INVOKER_METHOD_NOT_FOUND";
    pub const INVOKER_DESCRIPTOR_MISMATCH: &'static str = "MIXIN_INVOKER_DESCRIPTOR_MISMATCH";
    pub const OVERWRITE_TARGET_NOT_FOUND: &'static str = "MIXIN_OVERWRITE_TARGET_NOT_FOUND";
    pub const OVERWRITE_DESCRIPTOR_MISMATCH: &'static str = "MIXIN_OVERWRITE_DESCRIPTOR_MISMATCH";
    pub const OVERWRITE_STATIC_MISMATCH: &'static str = "MIXIN_OVERWRITE_STATIC_MISMATCH";
    pub const UNRESOLVED_JAVA_TYPE: &'static str = "MIXIN_UNRESOLVED_JAVA_TYPE";
    pub const UNRESOLVED_HANDLER_DESCRIPTOR: &'static str = "MIXIN_UNRESOLVED_HANDLER_DESCRIPTOR";
    pub const LOW_CONFIDENCE_MEMBER_PARSE: &'static str = "MIXIN_LOW_CONFIDENCE_MEMBER_PARSE";
}
pub struct MixinDiagnosticRequest<'a> {
    pub source: &'a str,
    pub document_uri: &'a str,
    pub mixin_class_name: Option<&'a str>,
    pub mixin_package: Option<&'a str>,
    pub mixin_config_content: Option<&'a str>,
    pub mixin_config_path: Option<&'a str>,
    pub semantic_model: Option<&'a MixinClassModel>,
}
impl<'a> MixinDiagnosticRequest<'a> {
    pub fn new(source: &'a str, document_uri: &'a str) -> Self {
        Self {
            source,
            document_uri,
            mixin_class_name: None,
            mixin_package: None,
            mixin_config_content: None,
            mixin_config_path: None,
            semantic_model: None,
        }
    }
}
pub struct MixinDiagnosticsService<'a> {
    pub class_index: &'a dyn ClassIndex,
    pub bytecode_index: &'a dyn BytecodeIndex,
}
impl<'a> MixinDiagnosticsService<'a> {
    pub fn new(class_index: &'a dyn ClassIndex, bytecode_index: &'a dyn BytecodeIndex) -> Self {
        Self {
            class_index,
            bytecode_index,
        }
    }
    pub fn analyze(&self, r: &MixinDiagnosticRequest<'_>) -> Vec<McDiagnostic> {
        let parsed;
        let model = if let Some(model) = r.semantic_model {
            model
        } else {
            parsed = MixinSemanticModelParser::parse(r.source, r.document_uri);
            &parsed
        };
        let mut out = self.analyze_targets(r, model);
        out.extend(self.analyze_config(r));
        for injector in &model.injectors {
            let targets = self.targets_at(r.source, injector.range.start);
            for s in &injector.method_selectors {
                if targets.is_empty() {
                    continue;
                }
                let methods = targets
                    .iter()
                    .flat_map(|o| self.class_index.get_methods(o))
                    .filter(|m| m.name == s.name)
                    .collect::<Vec<_>>();
                let d = if methods.is_empty() {
                    Some(issue(
                        MixinDiagnosticCodes::UNRESOLVED_INJECT_METHOD,
                        format!("Unresolved injector method: {}", s.value),
                        s.range,
                        &[("method", s.value.clone())],
                    ))
                } else if let Some(desc) = &s.descriptor {
                    if methods.iter().any(|m| &m.descriptor == desc) {
                        None
                    } else {
                        Some(issue(
                            MixinDiagnosticCodes::DESCRIPTOR_MISMATCH,
                            format!("Injector method descriptor mismatch: {}", s.value),
                            s.range,
                            &[],
                        ))
                    }
                } else if methods.len() > 1 {
                    let mut d = issue(
                        MixinDiagnosticCodes::AMBIGUOUS_INJECT_METHOD,
                        format!("Ambiguous injector method '{}'; add descriptor", s.name),
                        s.range,
                        &[
                            ("method", s.name.clone()),
                            ("count", methods.len().to_string()),
                        ],
                    );
                    d.severity = McSeverity::Warning;
                    Some(d)
                } else {
                    None
                };
                if let Some(d) = d {
                    out.push(d);
                }
            }
        }
        for injector in &model.injectors {
            let targets = self.targets_at(r.source, injector.range.start);
            for at in &injector.at_selectors {
                if let (Some(ordinal), Some(owner)) = (at.ordinal, targets.first()) {
                    if at.value == "RETURN" {
                        for s in &injector.method_selectors {
                            let count = self.bytecode_index.get_return_ordinal_count(
                                owner,
                                &s.name,
                                s.descriptor.as_deref(),
                            );
                            if ordinal >= count as i32 {
                                out.push(issue(
                                    MixinDiagnosticCodes::ORDINAL_OUT_OF_RANGE,
                                    format!(
                                        "Ordinal {ordinal} out of range for RETURN (max {})",
                                        count as i64 - 1
                                    ),
                                    at.ordinal_range.unwrap_or(at.range),
                                    &[],
                                ));
                            }
                        }
                    }
                }
                let Some(target) = at.target.as_deref().filter(|t| !t.is_empty()) else {
                    continue;
                };
                let range = at.target_range.unwrap_or(at.range);
                if MemberTargetParser::parse(target).is_err() {
                    out.push(issue(
                        MixinDiagnosticCodes::INVALID_AT_TARGET_DESCRIPTOR,
                        format!("Invalid @At target descriptor: {target}"),
                        range,
                        &[],
                    ));
                    continue;
                }
                let Some(owner) = targets.first() else {
                    continue;
                };
                for s in &injector.method_selectors {
                    let candidates = self.bytecode_index.get_at_target_candidates(
                        owner,
                        &s.name,
                        s.descriptor.as_deref(),
                        &at.value,
                    );
                    if !candidates
                        .iter()
                        .any(|c| AtTargetCompletionService::format_target(c) == target)
                    {
                        out.push(issue(
                            MixinDiagnosticCodes::UNRESOLVED_AT_TARGET,
                            format!("Unresolved @At target: {target}"),
                            range,
                            &[("target", target.into()), ("atValue", at.value.clone())],
                        ));
                    }
                }
            }
        }
        out.extend(MixinMemberDeclarationParser::parse_declaration_diagnostics(
            r.source,
            Some(self.class_index),
        ));
        for d in MixinMemberDeclarationParser::parse_overwrite_declarations(
            r.source,
            Some(self.class_index),
        ) {
            out.extend(
                OverwriteValidationService::new(self.class_index)
                    .validate(&self.targets_at(r.source, d.range.start), &d),
            );
        }
        out
    }
    fn targets_at(&self, source: &str, position: McTextPosition) -> Vec<String> {
        let imports = JavaTypeDescriptorResolver::imports_for(source);
        if !imports.is_kotlin {
            return MixinTargetResolver::resolve_targets_from_source(source, self.class_index);
        }
        position_to_offset(source, position).map_or_else(Vec::new, |at| {
            MixinTargetResolver::resolve_targets(
                &AnnotationContextExtractor::resolve_raw_mixin_targets(source, at),
                self.class_index,
                Some(&imports),
            )
        })
    }
    fn analyze_targets(
        &self,
        r: &MixinDiagnosticRequest<'_>,
        model: &MixinClassModel,
    ) -> Vec<McDiagnostic> {
        let imports = JavaTypeDescriptorResolver::imports_for(r.source);
        let mut out = vec![];
        for a in annotations(r.source)
            .into_iter()
            .filter(|a| a.annotation == MixinAnnotation::Mixin)
        {
            let raw = AnnotationContextExtractor::parse_mixin_target_values(r.source, a.start);
            let mut counts = std::collections::BTreeMap::<String, usize>::new();
            for s in raw {
                let key = s
                    .trim()
                    .replace('.', "/")
                    .rsplit('/')
                    .next()
                    .unwrap_or("")
                    .to_string();
                *counts.entry(key).or_default() += 1;
            }
            for (target, count) in counts {
                if count > 1 {
                    let mut d = issue(
                        MixinDiagnosticCodes::DUPLICATE_MIXIN_TARGET,
                        format!("Duplicate @Mixin target entry: {target}"),
                        source_range(r.source, a.start, a.end),
                        &[("target", target)],
                    );
                    d.severity = McSeverity::Warning;
                    out.push(d);
                }
            }
        }
        for target in &model.targets {
            if MixinTargetResolver::resolve_target(
                &target.internal_name,
                self.class_index,
                Some(&imports),
            )
            .is_none()
            {
                out.push(issue(
                    MixinDiagnosticCodes::UNRESOLVED_MIXIN_TARGET,
                    format!("Unresolved @Mixin target: {}", target.internal_name),
                    target.range,
                    &[("target", target.internal_name.clone())],
                ));
            }
        }
        out
    }
    fn analyze_config(&self, r: &MixinDiagnosticRequest<'_>) -> Vec<McDiagnostic> {
        if JavaTypeDescriptorResolver::imports_for_uri(r.source, r.document_uri).is_kotlin {
            return vec![];
        }
        let (Some(name), Some(content)) = (r.mixin_class_name, r.mixin_config_content) else {
            return vec![];
        };
        let Ok(config) = MixinConfigEditor::parse(content, r.mixin_config_path.unwrap_or(""))
        else {
            return vec![];
        };
        let package = config
            .package_name
            .as_deref()
            .filter(|p| !p.trim().is_empty());
        let relative = match (package, r.mixin_package) {
            (None, _) => name.to_string(),
            (Some(p), Some(s)) if p == s => name.into(),
            (Some(p), Some(s)) => {
                let Some(rel) = s.strip_prefix(&format!("{p}.")).filter(|s| !s.is_empty()) else {
                    return vec![];
                };
                format!("{rel}.{name}")
            }
            _ => return vec![],
        };
        let listed = config
            .mixins
            .iter()
            .chain(&config.client)
            .chain(&config.server)
            .chain(&config.common)
            .collect::<Vec<_>>();
        let config_ref = crate::project::MixinConfigRef {
            path: ".".into(),
            package_name: config.package_name.clone(),
            mixins: config.mixins.clone(),
            client: config.client.clone(),
            server: config.server.clone(),
            common: config.common.clone(),
        };
        let found = crate::project::MixinConfigDiscoveryService::is_mixin_listed(
            &config_ref,
            name,
            r.mixin_package,
        );
        if !found {
            let tokens = source_tokens(r.source, false);
            let range = tokens
                .windows(2)
                .find(|ts| ts[0].text == "class" && ts[1].kind == SourceTokenKind::Identifier)
                .map_or_else(McTextRange::default, |ts| {
                    source_range(r.source, ts[0].start, ts[1].end)
                });
            let mut d = issue(
                MixinDiagnosticCodes::MIXIN_CLASS_NOT_LISTED_IN_CONFIG,
                format!("Mixin class '{name}' is not listed in mixin config"),
                range,
                &[
                    ("mixinClass", relative),
                    ("configPath", r.mixin_config_path.unwrap_or("").into()),
                ],
            );
            d.severity = McSeverity::Warning;
            return vec![d];
        }
        let mut counts = std::collections::BTreeMap::<String, usize>::new();
        for name in listed {
            *counts.entry(name.clone()).or_default() += 1;
        }
        counts
            .into_iter()
            .filter(|(_, c)| *c > 1)
            .map(|(name, _)| {
                let quoted = format!("\"{name}\"");
                let range = content
                    .find(&quoted)
                    .map_or_else(McTextRange::default, |s| {
                        source_range(content, s + 1, s + quoted.len() - 1)
                    });
                let mut d = issue(
                    MixinDiagnosticCodes::DUPLICATE_MIXIN_CONFIG_ENTRY,
                    format!("Duplicate mixin config entry: {name}"),
                    range,
                    &[("mixinClass", name)],
                );
                d.severity = McSeverity::Warning;
                d
            })
            .collect()
    }
}
