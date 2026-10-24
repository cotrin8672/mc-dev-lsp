//! Mixin definition/reference facts and host-supplied source-location resolution.
use crate::{
    bytecode::BytecodeIndex,
    descriptor::{self, Pattern},
    extras_model::MixinExtrasAnnotation,
    mixin_source::*,
    mixin_validation::{AccessorService, InvokerService},
    mixinextras_expression::{ClassLiteralTypeNameResolver, ExpressionContextResolver},
    *,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

fn range(source: &str, start: usize, end: usize) -> McTextRange {
    let start = start.min(source.len());
    let end = end.max(start).min(source.len());
    McTextRange {
        start: offset_to_position(source, start).unwrap_or_default(),
        end: offset_to_position(source, end).unwrap_or_default(),
    }
}
fn bounds(source: &str, r: McTextRange) -> Option<std::ops::Range<usize>> {
    Some(position_to_offset(source, r.start)?..position_to_offset(source, r.end)?)
}
fn definition(
    kind: MemberKind,
    owner: &str,
    fqn: Option<String>,
    name: Option<String>,
    descriptor: Option<String>,
    source_range: McTextRange,
) -> McDefinitionTarget {
    McDefinitionTarget {
        kind,
        owner_internal_name: owner.into(),
        owner_fqn: fqn,
        name,
        descriptor,
        namespace: MappingNamespace::Named,
        source_range: Some(source_range),
        direct_source_document_uri: None,
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedAtTarget {
    pub owner_internal_name: String,
    pub name: String,
    pub descriptor: String,
    pub kind: MemberKind,
}
pub struct AtTargetParser;
impl AtTargetParser {
    pub fn parse(target: &str) -> Option<ParsedAtTarget> {
        let trimmed = target.trim().strip_prefix('L')?;
        let (owner, tail) = trimmed.split_once(';')?;
        if owner.is_empty() || tail.is_empty() {
            return None;
        }
        let (name, descriptor, kind) = if !tail.contains('(') && tail.contains(':') {
            let (name, descriptor) = tail.split_once(':')?;
            (name, descriptor, MemberKind::Field)
        } else {
            let at = tail.find('(')?;
            (&tail[..at], &tail[at..], MemberKind::Method)
        };
        if name.is_empty() || descriptor.is_empty() {
            return None;
        }
        Some(ParsedAtTarget {
            owner_internal_name: owner.into(),
            name: name.into(),
            descriptor: descriptor.into(),
            kind,
        })
    }
}
pub struct MixinDefinitionService<'a> {
    pub class_index: &'a dyn ClassIndex,
    pub bytecode_index: &'a dyn BytecodeIndex,
}
impl<'a> MixinDefinitionService<'a> {
    pub fn new(class_index: &'a dyn ClassIndex, bytecode_index: &'a dyn BytecodeIndex) -> Self {
        Self {
            class_index,
            bytecode_index,
        }
    }
    pub fn definitions_at(
        &self,
        source: &str,
        line: usize,
        character: usize,
        semantic_model: Option<&MixinClassModel>,
        document_uri: Option<&str>,
    ) -> Vec<McDefinitionTarget> {
        position_to_offset(source, McTextPosition { line, character })
            .map(|offset| self.definitions_at_offset(source, offset, semantic_model, document_uri))
            .unwrap_or_default()
    }
    pub fn definitions_at_offset(
        &self,
        source: &str,
        offset: usize,
        semantic_model: Option<&MixinClassModel>,
        document_uri: Option<&str>,
    ) -> Vec<McDefinitionTarget> {
        if offset > source.len() || !source.is_char_boundary(offset) {
            return vec![];
        }
        if let Some(context) = AnnotationContextExtractor::extract_at_offset(source, offset) {
            let result = self.from_context(source, &context, semantic_model, offset, document_uri);
            if !result.is_empty() {
                return result;
            }
        }
        for captures in Regex::new(r#"target\s*=\s*"([^"]*)""#)
            .unwrap()
            .captures_iter(source)
        {
            let value = captures.get(1).unwrap();
            if value.start() <= offset && offset <= value.end() {
                if let Some(parsed) = AtTargetParser::parse(value.as_str()) {
                    return vec![
                        self.parsed_target(parsed, range(source, value.start(), value.end()))
                    ];
                }
            }
        }
        self.from_member(source, offset, semantic_model)
    }
    fn owner_fqn(&self, owner: &str) -> String {
        self.class_index
            .find_class(owner)
            .map(|c| c.fqn())
            .unwrap_or_else(|| owner.replace('/', "."))
    }
    fn parsed_target(&self, p: ParsedAtTarget, r: McTextRange) -> McDefinitionTarget {
        definition(
            p.kind,
            &p.owner_internal_name,
            Some(self.owner_fqn(&p.owner_internal_name)),
            Some(p.name),
            Some(p.descriptor),
            r,
        )
    }
    fn from_context(
        &self,
        source: &str,
        c: &AnnotationContext,
        model: Option<&MixinClassModel>,
        offset: usize,
        uri: Option<&str>,
    ) -> Vec<McDefinitionTarget> {
        if c.slot == AnnotationSlot::Method
            && MixinExtrasAnnotation::from_mixin_annotation(c.annotation).is_some()
        {
            return self.injector(source, c, model);
        }
        match (c.annotation, c.slot) {
            (MixinAnnotation::Mixin, AnnotationSlot::Class | AnnotationSlot::Targets) => {
                self.mixin_class(source, c, model, offset)
            }
            (MixinAnnotation::Shadow, AnnotationSlot::ShadowMember) => self.shadow(source, offset),
            (MixinAnnotation::Overwrite, AnnotationSlot::OverwriteMethod) => {
                self.overwrite(source, offset)
            }
            (MixinAnnotation::Accessor, AnnotationSlot::AccessorValue) => {
                self.accessor(source, c, model)
            }
            (MixinAnnotation::Invoker, AnnotationSlot::InvokerValue) => {
                self.invoker(source, c, model)
            }
            (MixinAnnotation::At, AnnotationSlot::Target) => {
                AtTargetParser::parse(c.partial_value.trim_matches('"'))
                    .map(|p| {
                        vec![self.parsed_target(
                            p,
                            range(source, c.value_start_offset, c.value_end_offset),
                        )]
                    })
                    .unwrap_or_default()
            }
            (MixinAnnotation::Definition, AnnotationSlot::Value) => {
                match c.attribute_name.as_deref() {
                    Some("method") => self.exact_selector(source, c, true),
                    Some("field") => self.exact_selector(source, c, false),
                    Some("type") => self.class_literal(source, c),
                    _ => vec![],
                }
            }
            (MixinAnnotation::Local, AnnotationSlot::Value)
                if c.attribute_name.as_deref() == Some("type") =>
            {
                let inside = AnnotationContextExtractor::find_annotation_offsets(
                    source,
                    MixinAnnotation::Definition,
                )
                .into_iter()
                .any(|start| {
                    AnnotationContextExtractor::annotation_end_offset(source, start).is_some_and(
                        |end| start < c.annotation_start_offset && c.annotation_start_offset < end,
                    )
                });
                if inside {
                    self.class_literal(source, c)
                } else {
                    vec![]
                }
            }
            (MixinAnnotation::Expression | MixinAnnotation::Expressions, AnnotationSlot::Value) => {
                self.expression_id(source, c, offset, uri)
            }
            _ => vec![],
        }
    }
    fn mixin_class(
        &self,
        source: &str,
        c: &AnnotationContext,
        model: Option<&MixinClassModel>,
        offset: usize,
    ) -> Vec<McDefinitionTarget> {
        let raw = c
            .partial_value
            .trim()
            .trim_matches('"')
            .trim_end_matches(".class");
        if raw.is_empty() {
            return vec![];
        }
        let imports = JavaTypeDescriptorResolver::imports_for(source);
        let semantic = model.and_then(|m| {
            m.targets.iter().find(|t| {
                bounds(source, t.range).is_some_and(|r| r.start <= offset && offset <= r.end)
            })
        });
        let entry = if let Some(target) = semantic {
            self.class_index
                .find_class(&target.internal_name)
                .or_else(|| {
                    MixinTargetResolver::resolve_target(
                        &target.internal_name,
                        self.class_index,
                        Some(&imports),
                    )
                    .and_then(|name| self.class_index.find_class(&name))
                })
        } else {
            let start = c.value_start_offset.min(source.len());
            let end = c.value_end_offset.max(start).min(source.len());
            let complete = if start < end && start <= offset && offset <= end {
                source[start..end]
                    .trim()
                    .strip_suffix(".class")
                    .map(str::trim)
            } else {
                None
            };
            self.class_entry(complete.unwrap_or(raw), Some(&imports), complete.is_none())
        };
        entry
            .map(|e| {
                vec![definition(
                    MemberKind::Class,
                    &e.internal_name,
                    Some(e.fqn()),
                    None,
                    None,
                    range(source, c.value_start_offset, c.value_end_offset),
                )]
            })
            .unwrap_or_default()
    }
    fn class_entry(
        &self,
        raw: &str,
        imports: Option<&JavaSourceImports>,
        prefix: bool,
    ) -> Option<ClassIndexEntry> {
        let raw = raw.trim().trim_matches('"').trim_end_matches(".class");
        if raw.is_empty() {
            return None;
        }
        if let Some(entry) = self.class_index.find_class_by_fqn(raw) {
            return Some(entry);
        }
        if let Some(entry) = MixinTargetResolver::resolve_target(raw, self.class_index, imports)
            .and_then(|name| self.class_index.find_class(&name))
        {
            return Some(entry);
        }
        if prefix {
            let mut candidates = self.class_index.find_classes(raw, 5);
            if candidates.len() == 1 {
                return candidates.pop();
            }
        }
        None
    }
    fn targets(
        &self,
        source: &str,
        c: Option<&AnnotationContext>,
        model: Option<&MixinClassModel>,
        offset: Option<usize>,
    ) -> Vec<String> {
        let imports = JavaTypeDescriptorResolver::imports_for(source);
        if imports.is_kotlin {
            return offset
                .map(|at| {
                    MixinTargetResolver::resolve_targets(
                        &AnnotationContextExtractor::resolve_raw_mixin_targets(source, at),
                        self.class_index,
                        Some(&imports),
                    )
                })
                .unwrap_or_default();
        }
        if let Some(model) = model {
            let targets = model
                .targets
                .iter()
                .filter_map(|t| {
                    MixinTargetResolver::resolve_target(
                        &t.internal_name,
                        self.class_index,
                        Some(&imports),
                    )
                })
                .collect::<Vec<_>>();
            if !targets.is_empty() {
                return targets;
            }
        }
        if c.is_none() && model.is_none() {
            return MixinTargetResolver::resolve_targets_from_source(source, self.class_index);
        }
        let raw = if let Some(c) = c {
            if c.mixin_target_internal_names.is_empty() {
                AnnotationContextExtractor::resolve_raw_mixin_targets(source, c.value_start_offset)
            } else {
                c.mixin_target_internal_names.clone()
            }
        } else {
            AnnotationContextExtractor::resolve_raw_mixin_targets(source, 0)
        };
        MixinTargetResolver::resolve_targets(&raw, self.class_index, Some(&imports))
    }
    fn field(&self, owners: &[String], name: &str, r: McTextRange) -> Vec<McDefinitionTarget> {
        for owner in owners {
            if let Some(field) = self
                .class_index
                .get_fields(owner)
                .into_iter()
                .find(|f| f.name == name)
            {
                return vec![definition(
                    MemberKind::Field,
                    owner,
                    Some(self.owner_fqn(owner)),
                    Some(field.name),
                    Some(field.descriptor),
                    r,
                )];
            }
        }
        vec![]
    }
    fn method(
        &self,
        owners: &[String],
        name: &str,
        r: McTextRange,
        descriptor: Option<&str>,
    ) -> Vec<McDefinitionTarget> {
        for owner in owners {
            let methods = self
                .class_index
                .get_methods(owner)
                .into_iter()
                .filter(|m| m.name == name)
                .collect::<Vec<_>>();
            let method = if let Some(descriptor) = descriptor {
                methods.into_iter().find(|m| m.descriptor == descriptor)
            } else if methods.len() == 1 {
                methods.into_iter().next()
            } else {
                None
            };
            if let Some(method) = method {
                return vec![definition(
                    MemberKind::Method,
                    owner,
                    Some(self.owner_fqn(owner)),
                    Some(method.name),
                    Some(method.descriptor),
                    r,
                )];
            }
        }
        vec![]
    }
    fn member(
        &self,
        owners: &[String],
        name: &str,
        is_method: bool,
        descriptor: Option<&str>,
        r: McTextRange,
    ) -> Vec<McDefinitionTarget> {
        if !is_method {
            return self.field(owners, name, r);
        }
        let Some(descriptor) = descriptor else {
            return vec![];
        };
        let owner = owners.iter().find(|owner| {
            self.class_index
                .get_methods(owner)
                .iter()
                .any(|m| m.name == name)
        });
        owner
            .map(|owner| self.method(&[owner.clone()], name, r, Some(descriptor)))
            .unwrap_or_default()
    }
    fn name_range(
        &self,
        source: &str,
        r: McTextRange,
        name: &str,
        annotation: &str,
        is_method: bool,
    ) -> Option<std::ops::Range<usize>> {
        bounds(
            source,
            MixinMemberDeclarationParser::find_member_name_text_range(
                source, r, name, annotation, is_method,
            )?,
        )
    }
    fn shadow(&self, source: &str, offset: usize) -> Vec<McDefinitionTarget> {
        for d in
            MixinMemberDeclarationParser::parse_shadow_declarations(source, Some(self.class_index))
        {
            if self
                .name_range(source, d.range, &d.name, "Shadow", d.is_method)
                .is_some_and(|r| r.start <= offset && offset <= r.end)
            {
                let owners = self.targets(source, None, None, Some(offset));
                let prefix = MixinMemberDeclarationParser::find_shadow_prefix(source);
                let name = prefix
                    .as_deref()
                    .and_then(|p| d.name.strip_prefix(p))
                    .unwrap_or(&d.name);
                return self.member(&owners, name, d.is_method, Some(&d.descriptor), d.range);
            }
        }
        vec![]
    }
    fn overwrite(&self, source: &str, offset: usize) -> Vec<McDefinitionTarget> {
        for d in MixinMemberDeclarationParser::parse_overwrite_declarations(
            source,
            Some(self.class_index),
        ) {
            if self
                .name_range(source, d.range, &d.name, "Overwrite", true)
                .is_some_and(|r| r.start <= offset && offset <= r.end)
            {
                return self.member(
                    &self.targets(source, None, None, Some(offset)),
                    &d.name,
                    true,
                    Some(&d.descriptor),
                    d.range,
                );
            }
        }
        vec![]
    }
    fn accessor(
        &self,
        source: &str,
        c: &AnnotationContext,
        model: Option<&MixinClassModel>,
    ) -> Vec<McDefinitionTarget> {
        let line = offset_to_position(source, c.annotation_start_offset)
            .unwrap_or_default()
            .line;
        let declaration = MixinMemberDeclarationParser::parse_accessor_declarations(
            source,
            Some(self.class_index),
        )
        .into_iter()
        .find(|d| d.range.start.line >= line);
        if declaration.is_none() && self.parse_failure(source, line) {
            return vec![];
        }
        let name = if c.partial_value.trim_matches('"').is_empty() {
            declaration
                .as_ref()
                .and_then(AccessorService::infer_field_name)
        } else {
            Some(c.partial_value.trim_matches('"').into())
        };
        name.map(|name| {
            self.field(
                &self.targets(source, Some(c), model, Some(c.value_start_offset)),
                &name,
                range(source, c.value_start_offset, c.value_end_offset),
            )
        })
        .unwrap_or_default()
    }
    fn invoker(
        &self,
        source: &str,
        c: &AnnotationContext,
        model: Option<&MixinClassModel>,
    ) -> Vec<McDefinitionTarget> {
        let line = offset_to_position(source, c.annotation_start_offset)
            .unwrap_or_default()
            .line;
        let declaration = MixinMemberDeclarationParser::parse_invoker_declarations(
            source,
            Some(self.class_index),
        )
        .into_iter()
        .find(|d| d.range.start.line >= line);
        if declaration.is_none() && self.parse_failure(source, line) {
            return vec![];
        }
        let name = if c.partial_value.trim_matches('"').is_empty() {
            declaration
                .as_ref()
                .and_then(InvokerService::infer_target_name)
        } else {
            Some(c.partial_value.trim_matches('"').into())
        };
        let descriptor = declaration.as_ref().and_then(invoker_descriptor);
        name.map(|name| {
            self.method(
                &self.targets(source, Some(c), model, Some(c.value_start_offset)),
                &name,
                range(source, c.value_start_offset, c.value_end_offset),
                descriptor.as_deref(),
            )
        })
        .unwrap_or_default()
    }
    fn parse_failure(&self, source: &str, line: usize) -> bool {
        MixinMemberDeclarationParser::parse_declaration_diagnostics(source, Some(self.class_index))
            .iter()
            .any(|d| d.range.start.line >= line)
    }
    fn from_member(
        &self,
        source: &str,
        offset: usize,
        model: Option<&MixinClassModel>,
    ) -> Vec<McDefinitionTarget> {
        if let Some(model) = model {
            for member in &model.members {
                let is_method = member
                    .is_method
                    .unwrap_or(member.method_descriptor.is_some());
                let annotation = match member.annotation_kind {
                    MixinMemberAnnotationKind::Shadow => "Shadow",
                    MixinMemberAnnotationKind::Overwrite => "Overwrite",
                    MixinMemberAnnotationKind::Accessor => "Accessor",
                    MixinMemberAnnotationKind::Invoker => "Invoker",
                };
                let name_range = bounds(source, member.name_range)
                    .filter(|r| {
                        r.start < r.end && source.get(r.clone()) == Some(member.java_name.as_str())
                    })
                    .map(|_| member.name_range)
                    .or_else(|| {
                        MixinMemberDeclarationParser::find_member_name_text_range(
                            source,
                            member.range,
                            &member.java_name,
                            annotation,
                            is_method,
                        )
                    });
                let Some(name_range) = name_range else {
                    continue;
                };
                if !bounds(source, name_range).is_some_and(|r| r.start <= offset && offset <= r.end)
                {
                    continue;
                }
                let owners = self.targets(source, None, Some(model), Some(offset));
                return match member.annotation_kind {
                    MixinMemberAnnotationKind::Shadow | MixinMemberAnnotationKind::Overwrite => {
                        self.member(
                            &owners,
                            member
                                .explicit_target_name
                                .as_deref()
                                .unwrap_or(&member.java_name),
                            is_method,
                            if is_method {
                                member.method_descriptor.as_deref()
                            } else {
                                member.return_descriptor.as_deref()
                            },
                            name_range,
                        )
                    }
                    MixinMemberAnnotationKind::Accessor => {
                        let declaration = AccessorMethodDeclaration {
                            method_name: member.java_name.clone(),
                            return_type_descriptor: member.return_descriptor.clone(),
                            parameter_descriptors: member.parameter_descriptors.clone(),
                            explicit_field_name: member.explicit_target_name.clone(),
                            range: member.range,
                            parse_source: member.parse_source,
                            confidence: member.confidence,
                            warnings: member.warnings.clone(),
                        };
                        AccessorService::infer_field_name(&declaration)
                            .map(|name| self.field(&owners, &name, name_range))
                            .unwrap_or_default()
                    }
                    MixinMemberAnnotationKind::Invoker => {
                        let declaration = InvokerMethodDeclaration {
                            method_name: member.java_name.clone(),
                            return_type_descriptor: member.return_descriptor.clone(),
                            parameter_descriptors: member.parameter_descriptors.clone(),
                            explicit_target_name: member.explicit_target_name.clone(),
                            range: member.range,
                            parse_source: member.parse_source,
                            confidence: member.confidence,
                            warnings: member.warnings.clone(),
                        };
                        declaration
                            .explicit_target_name
                            .clone()
                            .or_else(|| InvokerService::infer_target_name(&declaration))
                            .map(|name| {
                                self.method(
                                    &owners,
                                    &name,
                                    name_range,
                                    member.method_descriptor.as_deref(),
                                )
                            })
                            .unwrap_or_default()
                    }
                };
            }
        }
        if MixinMemberDeclarationParser::parse_shadow_declarations(source, Some(self.class_index))
            .iter()
            .any(|d| {
                self.name_range(source, d.range, &d.name, "Shadow", d.is_method)
                    .is_some_and(|r| r.start <= offset && offset <= r.end)
            })
        {
            return self.shadow(source, offset);
        }
        if MixinMemberDeclarationParser::parse_overwrite_declarations(
            source,
            Some(self.class_index),
        )
        .iter()
        .any(|d| {
            self.name_range(source, d.range, &d.name, "Overwrite", true)
                .is_some_and(|r| r.start <= offset && offset <= r.end)
        }) {
            return self.overwrite(source, offset);
        }
        for d in MixinMemberDeclarationParser::parse_accessor_declarations(
            source,
            Some(self.class_index),
        ) {
            if self
                .name_range(source, d.range, &d.method_name, "Accessor", true)
                .is_some_and(|r| {
                    r.start <= offset
                        && source[..offset].encode_utf16().count() + 1
                            < source[..r.end].encode_utf16().count()
                })
            {
                let Some(name) = AccessorService::infer_field_name(&d) else {
                    return vec![];
                };
                return self.field(
                    &self.targets(source, None, None, Some(offset)),
                    &name,
                    d.range,
                );
            }
        }
        for d in
            MixinMemberDeclarationParser::parse_invoker_declarations(source, Some(self.class_index))
        {
            if self
                .name_range(source, d.range, &d.method_name, "Invoker", true)
                .is_some_and(|r| {
                    r.start <= offset
                        && source[..offset].encode_utf16().count() + 1
                            < source[..r.end].encode_utf16().count()
                })
            {
                let Some(name) = d
                    .explicit_target_name
                    .clone()
                    .or_else(|| InvokerService::infer_target_name(&d))
                else {
                    return vec![];
                };
                return self.method(
                    &self.targets(source, None, None, Some(offset)),
                    &name,
                    d.range,
                    invoker_descriptor(&d).as_deref(),
                );
            }
        }
        vec![]
    }
    fn injector(
        &self,
        source: &str,
        c: &AnnotationContext,
        model: Option<&MixinClassModel>,
    ) -> Vec<McDefinitionTarget> {
        let Some(raw) = raw_value(source, c) else {
            return vec![];
        };
        if raw.contains('*') {
            return vec![];
        }
        let Ok(selector) = descriptor::parse_method_selector(raw) else {
            return vec![];
        };
        let Pattern::Exact(name) = selector.name else {
            return vec![];
        };
        let descriptor = match selector.descriptor {
            Pattern::Exact(value) => Some(descriptor::method_to_descriptor(&value)),
            Pattern::Any => None,
        };
        let owners = self.targets(source, Some(c), model, Some(c.value_start_offset));
        let owners = match selector.owner {
            Pattern::Exact(owner) if owners.contains(&owner) => vec![owner],
            Pattern::Exact(_) => return vec![],
            Pattern::Any => owners,
        };
        self.method(
            &owners,
            &name,
            range(source, c.value_start_offset, c.value_end_offset),
            descriptor.as_deref(),
        )
    }
    fn exact_selector(
        &self,
        source: &str,
        c: &AnnotationContext,
        is_method: bool,
    ) -> Vec<McDefinitionTarget> {
        let Some(raw) = raw_value(source, c) else {
            return vec![];
        };
        if raw.contains('*') {
            return vec![];
        }
        let (kind, owner, name, descriptor) = if is_method {
            let Ok(selector) = descriptor::parse_method_selector(raw) else {
                return vec![];
            };
            let (Pattern::Exact(owner), Pattern::Exact(name), Pattern::Exact(value)) =
                (selector.owner, selector.name, selector.descriptor)
            else {
                return vec![];
            };
            (
                MemberKind::Method,
                owner,
                name,
                descriptor::method_to_descriptor(&value),
            )
        } else {
            let Ok(selector) = descriptor::parse_field_selector(raw) else {
                return vec![];
            };
            let (Pattern::Exact(owner), Pattern::Exact(name), Pattern::Exact(value)) =
                (selector.owner, selector.name, selector.descriptor)
            else {
                return vec![];
            };
            (
                MemberKind::Field,
                owner,
                name,
                descriptor::to_descriptor(&value),
            )
        };
        vec![definition(
            kind,
            &owner,
            Some(self.owner_fqn(&owner)),
            Some(name),
            Some(descriptor),
            range(source, c.value_start_offset, c.value_end_offset),
        )]
    }
    fn class_literal(&self, source: &str, c: &AnnotationContext) -> Vec<McDefinitionTarget> {
        let Some(raw) = source.get(c.value_start_offset..c.value_end_offset) else {
            return vec![];
        };
        let literal = raw.trim();
        let suffix = if c.is_kotlin { "::class" } else { ".class" };
        let Some(at) = literal.find(suffix) else {
            return vec![];
        };
        if !literal[at + suffix.len()..].trim().is_empty() {
            return vec![];
        }
        let Some(descriptor) = ClassLiteralTypeNameResolver::for_source(source, self.class_index)
            .resolve(literal[..at].trim())
        else {
            return vec![];
        };
        let Some(owner) = descriptor
            .trim_start_matches('[')
            .strip_prefix('L')
            .and_then(|s| s.strip_suffix(';'))
        else {
            return vec![];
        };
        let Some(entry) = self.class_index.find_class(owner) else {
            return vec![];
        };
        let start = c.value_start_offset + raw.find(literal).unwrap();
        vec![definition(
            MemberKind::Class,
            &entry.internal_name,
            Some(entry.fqn()),
            None,
            None,
            range(source, start, start + at + suffix.len()),
        )]
    }
    fn expression_id(
        &self,
        source: &str,
        c: &AnnotationContext,
        offset: usize,
        uri: Option<&str>,
    ) -> Vec<McDefinitionTarget> {
        let Some(position) = c.expression_completion_position else {
            return vec![];
        };
        if !matches!(
            position,
            ExpressionCompletionPosition::StatementStart
                | ExpressionCompletionPosition::ValueStart
                | ExpressionCompletionPosition::AfterDot
                | ExpressionCompletionPosition::AfterMethodReference
        ) || offset < c.value_start_offset
            || offset >= c.value_end_offset
        {
            return vec![];
        }
        let id = c
            .decoded_partial_value
            .as_deref()
            .or_else(|| source.get(c.value_start_offset..c.value_end_offset))
            .unwrap_or("");
        if !expression_identifier(id) {
            return vec![];
        }
        let Some(site) =
            ExpressionContextResolver::find_enclosing_site(source, c.value_start_offset)
        else {
            return vec![];
        };
        let expression = ExpressionContextResolver::resolve_expression_context(source, &site);
        expression
            .definition_index
            .definitions_with_id(id)
            .into_iter()
            .filter_map(|d| {
                if position == ExpressionCompletionPosition::AfterDot
                    && d.raw_field_references.is_empty()
                    && d.raw_method_references.is_empty()
                {
                    return None;
                }
                if position == ExpressionCompletionPosition::AfterMethodReference
                    && d.raw_method_references.is_empty()
                {
                    return None;
                }
                let r = d.id_source_range.as_ref()?;
                let mut target = definition(
                    MemberKind::Class,
                    "",
                    None,
                    Some(id.into()),
                    None,
                    range(source, r.start, r.end),
                );
                target.direct_source_document_uri = uri.map(str::to_owned);
                Some(target)
            })
            .collect()
    }
}
fn raw_value<'a>(source: &'a str, c: &AnnotationContext) -> Option<&'a str> {
    source
        .get(c.value_start_offset..c.value_end_offset)
        .map(|s| s.trim_matches('"'))
        .filter(|s| !s.is_empty())
}
fn invoker_descriptor(d: &InvokerMethodDeclaration) -> Option<String> {
    Some(format!(
        "({}){}",
        d.parameter_descriptors.concat(),
        d.return_type_descriptor.as_deref()?
    ))
}
fn expression_identifier(id: &str) -> bool {
    if [
        "return", "throw", "this", "super", "true", "false", "null", "new",
    ]
    .contains(&id)
    {
        return false;
    }
    // The legacy predicate used Kotlin Char.isLetter/isLetterOrDigit, which
    // operate on UTF16 code units and use Letter/DecimalDigit categories.
    // Rust's alphabetic/alphanumeric predicates additionally admit e.g. Roman
    // numerals and superscript digits, so they would change identifier lookup.
    static IDENTIFIER: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    id.chars().all(|c| u32::from(c) <= 0xffff)
        && IDENTIFIER
            .get_or_init(|| Regex::new(r"\A[\p{L}_$][\p{L}\p{Nd}_$]*\z").unwrap())
            .is_match(id)
}

#[cfg(test)]
mod identifier_tests {
    #[test]
    fn expression_identifier_preserves_kotlin_char_categories() {
        for value in ["α1", "対象2", "_value$0"] {
            assert!(super::expression_identifier(value));
        }
        for value in ["", "return", "Ⅻ", "value²", "value\u{345}", "𐐀name"] {
            assert!(!super::expression_identifier(value));
        }
    }
}

fn java_regex(pattern: &str) -> Regex {
    Regex::new(
        &pattern
            .replace(r"\s", r"[ \t\n\r\x0B\x0C]")
            .replace(r"\S", r"[^ \t\n\r\x0B\x0C]"),
    )
    .expect("fixed navigation pattern")
}
pub struct MixinReferenceService;
impl MixinReferenceService {
    pub fn find_references(
        target: &McDefinitionTarget,
        sources: &[SourceScanEntry],
    ) -> Vec<McReferenceLocation> {
        sources
            .iter()
            .flat_map(|entry| Self::in_source(target, entry))
            .collect()
    }
    fn in_source(t: &McDefinitionTarget, e: &SourceScanEntry) -> Vec<McReferenceLocation> {
        let mut out = vec![];
        match t.kind {
            MemberKind::Class => {
                let simple = t
                    .owner_fqn
                    .as_deref()
                    .unwrap_or(&t.owner_internal_name)
                    .rsplit(['.', '/'])
                    .next()
                    .unwrap_or("");
                let owner = &t.owner_internal_name;
                let dotted = owner.replace('/', ".");
                collect_matches(e, r"@Mixin\s*\(([^)]*)\)", "mixin.class", &mut out, |c| {
                    let body = &c[1];
                    body.contains(&format!("{simple}.class"))
                        || t.owner_fqn.as_ref().is_some_and(|fqn| {
                            body.contains(fqn) || body.contains(&format!("\"{fqn}\""))
                        })
                        || body.contains(&format!("\"{owner}\""))
                        || body.contains(&dotted)
                });
                collect_matches(
                    e,
                    r#"@Mixin\s*\(\s*targets\s*=\s*("([^"]*)"|(\{[^}]*\}))"#,
                    "mixin.targets",
                    &mut out,
                    |c| {
                        c[1].contains(simple)
                            || t.owner_fqn.as_ref().is_some_and(|f| c[1].contains(f))
                            || c[1].contains(owner)
                            || c[1].contains(&dotted)
                    },
                );
                collect_matches(
                    e,
                    r"(accessible|extendable|mutable|natural)\s+class\s+(\S+)",
                    "aw.class",
                    &mut out,
                    |c| &c[2] == owner || c[2] == dotted,
                );
                collect_matches(
                    e,
                    r"(accessible|extendable|mutable|natural)\s+(method|field)\s+(\S+)\s+",
                    "aw.owner",
                    &mut out,
                    |c| &c[3] == owner || c[3] == dotted,
                );
                let fqn = t.owner_fqn.as_deref().unwrap_or(&dotted);
                collect_matches(
                    e,
                    r"(?m)(public|protected|private|default)\s+([A-Za-z0-9_.]+)(?:\s+\S+)?\s*$",
                    "at.class",
                    &mut out,
                    |c| &c[2] == fqn,
                );
                let mut seen = BTreeSet::new();
                out.retain(|location| {
                    seen.insert((
                        location.document_uri.clone(),
                        location.range.start.line,
                        location.range.start.character,
                    ))
                });
            }
            MemberKind::Field => {
                let Some(name) = t.name.as_deref() else {
                    return out;
                };
                for declaration in
                    MixinMemberDeclarationParser::parse_shadow_declarations(&e.text, None)
                        .into_iter()
                        .filter(|d| !d.is_method && d.name == name)
                {
                    out.push(reference_range(e, declaration.range, "mixin.shadow"))
                }
                collect_matches(
                    e,
                    r#"@Accessor\s*\(\s*"([^"]*)"\s*\)"#,
                    "mixin.accessor",
                    &mut out,
                    |c| &c[1] == name,
                );
                collect_matches(
                    e,
                    r#"target\s*=\s*"([^"]*)""#,
                    "mixin.atTarget",
                    &mut out,
                    |c| c[1].contains(&format!(";{name}:")),
                );
                let dotted = t.owner_internal_name.replace('/', ".");
                collect_matches(
                    e,
                    r"(accessible|extendable|mutable|natural)\s+field\s+(\S+)\s+([A-Za-z0-9_]+)\s+(\S+)",
                    "aw.field",
                    &mut out,
                    |c| {
                        &c[3] == name
                            && (c[2] == t.owner_internal_name || c[2] == dotted)
                            && t.descriptor.as_ref().is_none_or(|d| &c[4] == d)
                    },
                );
                let fqn = t.owner_fqn.as_deref().unwrap_or(&dotted);
                collect_matches(
                    e,
                    r"(?m)(public|protected|private|default)\s+([A-Za-z0-9_.]+)\s+([A-Za-z0-9_]+)\s*$",
                    "at.field",
                    &mut out,
                    |c| &c[2] == fqn && &c[3] == name,
                );
            }
            MemberKind::Method => {
                let Some(name) = t.name.as_deref() else {
                    return out;
                };
                for declaration in
                    MixinMemberDeclarationParser::parse_shadow_declarations(&e.text, None)
                        .into_iter()
                        .filter(|d| d.is_method && d.name == name)
                {
                    out.push(reference_range(e, declaration.range, "mixin.shadow"))
                }
                collect_matches(
                    e,
                    r#"@Invoker\s*\(\s*"([^"]*)"\s*\)"#,
                    "mixin.invoker",
                    &mut out,
                    |c| &c[1] == name,
                );
                collect_matches(
                    e,
                    r#"method\s*=\s*"([^"]*)""#,
                    "mixin.inject",
                    &mut out,
                    |c| {
                        let value = &c[1];
                        value == name
                            || t.descriptor
                                .as_deref()
                                .filter(|d| !d.trim().is_empty())
                                .is_some_and(|d| value == format!("{name}{d}"))
                            || t.descriptor.as_deref().is_none_or(|d| d.trim().is_empty())
                                && value.starts_with(&format!("{name}("))
                    },
                );
                for declaration in
                    MixinMemberDeclarationParser::parse_overwrite_declarations(&e.text, None)
                        .into_iter()
                        .filter(|d| d.name == name)
                {
                    out.push(reference_range(e, declaration.range, "mixin.overwrite"))
                }
                if let Some(descriptor) = &t.descriptor {
                    collect_matches(
                        e,
                        r#"target\s*=\s*"([^"]*)""#,
                        "mixin.atTarget",
                        &mut out,
                        |c| c[1].contains(&format!(";{name}{descriptor}")),
                    )
                }
                let dotted = t.owner_internal_name.replace('/', ".");
                collect_matches(
                    e,
                    r"(accessible|extendable|mutable|natural)\s+method\s+(\S+)\s+([A-Za-z0-9_]+)\s+(\S+)",
                    "aw.method",
                    &mut out,
                    |c| {
                        &c[3] == name
                            && (c[2] == t.owner_internal_name || c[2] == dotted)
                            && t.descriptor.as_ref().is_none_or(|d| &c[4] == d)
                    },
                );
                let fqn = t.owner_fqn.as_deref().unwrap_or(&dotted);
                collect_matches(
                    e,
                    r"(?m)(public|protected|private|default)\s+([A-Za-z0-9_.]+)\s+(\S+)\s*$",
                    "at.method",
                    &mut out,
                    |c| {
                        &c[2] == fqn
                            && c[3].starts_with(name)
                            && t.descriptor
                                .as_ref()
                                .is_none_or(|d| c[3] == format!("{name}{d}"))
                    },
                );
            }
            _ => {}
        }
        out
    }
}
fn reference_range(entry: &SourceScanEntry, r: McTextRange, source: &str) -> McReferenceLocation {
    McReferenceLocation {
        document_uri: entry.document_uri.clone(),
        range: r,
        metadata: BTreeMap::from([("source".into(), source.into())]),
    }
}
fn collect_matches(
    entry: &SourceScanEntry,
    pattern: &str,
    source: &str,
    output: &mut Vec<McReferenceLocation>,
    predicate: impl Fn(&regex::Captures<'_>) -> bool,
) {
    for captures in java_regex(pattern).captures_iter(&entry.text) {
        if predicate(&captures) {
            let m = captures.get(0).unwrap();
            output.push(reference_range(
                entry,
                range(&entry.text, m.start(), m.end()),
                source,
            ));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DefinitionResolution {
    Source,
    Jdt,
    BytecodeOnly,
    Unresolved,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedDefinition {
    pub target: McDefinitionTarget,
    pub document_uri: String,
    pub range: McTextRange,
    pub resolution: DefinitionResolution,
    pub resolution_message: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostSourceDeclaration {
    pub target: McDefinitionTarget,
    pub range: McTextRange,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostSourceSnapshot {
    pub entry: SourceScanEntry,
    pub generation: u64,
    pub owner_internal_names: Vec<String>,
    pub declarations: Vec<HostSourceDeclaration>,
}
/// Immutable normalized declaration facts supplied by the host. No source or classpath scan.
#[derive(Debug, Clone, Default)]
pub struct SourceIndex {
    snapshots: Vec<HostSourceSnapshot>,
}
impl SourceIndex {
    pub fn from_host_snapshots(snapshots: Vec<HostSourceSnapshot>) -> Self {
        Self { snapshots }
    }
    pub fn snapshots(&self) -> &[HostSourceSnapshot] {
        &self.snapshots
    }
    pub fn resolve(&self, target: &McDefinitionTarget) -> ResolvedDefinition {
        let fqn = target
            .owner_fqn
            .clone()
            .unwrap_or_else(|| target.owner_internal_name.replace('/', "."));
        let Some(file) = self.snapshots.iter().find(|file| {
            file.owner_internal_names
                .iter()
                .any(|name| name == &target.owner_internal_name || name.replace('/', ".") == fqn)
                || file.declarations.iter().any(|d| {
                    d.target.owner_internal_name == target.owner_internal_name
                        || d.target.owner_fqn.as_deref() == Some(&fqn)
                })
        }) else {
            return unresolved(target, format!("no project source file for {fqn}"));
        };
        let candidates = file
            .declarations
            .iter()
            .filter(|d| {
                d.target.kind == target.kind
                    && (d.target.owner_internal_name == target.owner_internal_name
                        || d.target.owner_fqn.as_deref() == Some(&fqn))
            })
            .collect::<Vec<_>>();
        let declaration = match target.kind {
            MemberKind::Class => candidates.first().copied(),
            MemberKind::Field | MemberKind::Method => {
                let Some(name) = target.name.as_deref() else {
                    return unresolved(
                        target,
                        if target.kind == MemberKind::Field {
                            "field name missing"
                        } else {
                            "method name missing"
                        },
                    );
                };
                let named = candidates
                    .into_iter()
                    .filter(|d| d.target.name.as_deref() == Some(name))
                    .collect::<Vec<_>>();
                if target.kind == MemberKind::Method {
                    target
                        .descriptor
                        .as_deref()
                        .filter(|d| !d.trim().is_empty())
                        .and_then(|descriptor| {
                            named.iter().find(|d| {
                                d.target.descriptor.as_deref().is_some_and(|value| {
                                    value.split(')').next() == descriptor.split(')').next()
                                })
                            })
                        })
                        .copied()
                        .or_else(|| named.first().copied())
                } else {
                    named.first().copied()
                }
            }
            _ => {
                return unresolved(
                    target,
                    format!(
                        "unsupported member kind {}",
                        format!("{:?}", target.kind).to_uppercase()
                    ),
                )
            }
        };
        if let Some(declaration) = declaration {
            return ResolvedDefinition {
                target: target.clone(),
                document_uri: file.entry.document_uri.clone(),
                range: declaration.range,
                resolution: DefinitionResolution::Source,
                resolution_message: None,
            };
        }
        let message = match target.kind {
            MemberKind::Class => format!("class declaration not found in {fqn}"),
            MemberKind::Field => format!(
                "field '{}' not found in {fqn}",
                target.name.as_deref().unwrap_or("")
            ),
            _ => format!(
                "method '{}' not found in {fqn}",
                target.name.as_deref().unwrap_or("")
            ),
        };
        unresolved(target, message)
    }
}
pub trait DefinitionBackend: Send + Sync {
    fn resolve(
        &self,
        target: &McDefinitionTarget,
        context: &crate::project::ProjectContext,
        workspace_root_uri: &str,
    ) -> Option<ResolvedDefinition>;
}
impl<
        F: Fn(
                &McDefinitionTarget,
                &crate::project::ProjectContext,
                &str,
            ) -> Option<ResolvedDefinition>
            + Send
            + Sync,
    > DefinitionBackend for F
{
    fn resolve(
        &self,
        target: &McDefinitionTarget,
        context: &crate::project::ProjectContext,
        uri: &str,
    ) -> Option<ResolvedDefinition> {
        self(target, context, uri)
    }
}
pub struct SourceDefinitionBackend<'a> {
    pub source_index: &'a SourceIndex,
}
impl DefinitionBackend for SourceDefinitionBackend<'_> {
    fn resolve(
        &self,
        target: &McDefinitionTarget,
        _: &crate::project::ProjectContext,
        _: &str,
    ) -> Option<ResolvedDefinition> {
        let resolved = self.source_index.resolve(target);
        (resolved.resolution == DefinitionResolution::Source).then_some(resolved)
    }
}
#[derive(Default)]
pub struct DefinitionResolutionService<'a> {
    pub backends: Vec<&'a dyn DefinitionBackend>,
}
impl<'a> DefinitionResolutionService<'a> {
    pub fn new(backends: Vec<&'a dyn DefinitionBackend>) -> Self {
        Self { backends }
    }
    pub fn resolve_all(
        &self,
        targets: &[McDefinitionTarget],
        context: &crate::project::ProjectContext,
        uri: &str,
    ) -> Vec<ResolvedDefinition> {
        targets
            .iter()
            .map(|target| {
                if let Some(direct) = &target.direct_source_document_uri {
                    return if direct.trim().is_empty() || target.source_range.is_none() {
                        unresolved(target, "direct source target missing uri or range")
                    } else {
                        ResolvedDefinition {
                            target: target.clone(),
                            document_uri: direct.clone(),
                            range: target.source_range.unwrap(),
                            resolution: DefinitionResolution::Source,
                            resolution_message: None,
                        }
                    };
                }
                self.backends
                    .iter()
                    .find_map(|backend| backend.resolve(target, context, uri))
                    .unwrap_or_else(|| unresolved(target, "no navigable definition found"))
            })
            .collect()
    }
}
fn unresolved(target: &McDefinitionTarget, message: impl Into<String>) -> ResolvedDefinition {
    ResolvedDefinition {
        target: target.clone(),
        document_uri: String::new(),
        range: target.source_range.unwrap_or_default(),
        resolution: DefinitionResolution::Unresolved,
        resolution_message: Some(message.into()),
    }
}
