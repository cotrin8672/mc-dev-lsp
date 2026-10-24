//! Pure member validation. Declarations and parse confidence are owned by `mixin_source`.
pub use crate::mixin_source::{
    AccessorMethodDeclaration, InvokerMethodDeclaration, OverwriteMethodDeclaration,
    ShadowMemberDeclaration,
};
use crate::{
    descriptor::{parse_field_descriptor, parse_method_descriptor, render},
    mixin_diagnostics::MixinDiagnosticCodes as C,
    model::*,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessorKind {
    Getter,
    Setter,
    Unknown,
}
pub(crate) fn issue(
    code: &str,
    message: String,
    range: McTextRange,
    metadata: &[(&str, String)],
) -> McDiagnostic {
    McDiagnostic {
        code: code.into(),
        severity: McSeverity::Error,
        message,
        range,
        metadata: metadata
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect(),
    }
}
fn lower_first(s: &str) -> String {
    let mut chars = s.chars();
    chars.next().map_or_else(String::new, |c| {
        c.to_lowercase().collect::<String>() + chars.as_str()
    })
}
fn upper_first(s: &str) -> String {
    let mut chars = s.chars();
    chars.next().map_or_else(String::new, |c| {
        c.to_uppercase().collect::<String>() + chars.as_str()
    })
}
pub struct AccessorService<'a> {
    pub class_index: &'a dyn ClassIndex,
}
impl<'a> AccessorService<'a> {
    pub fn new(class_index: &'a dyn ClassIndex) -> Self {
        Self { class_index }
    }
    pub fn infer_field_name(d: &AccessorMethodDeclaration) -> Option<String> {
        d.explicit_field_name.clone().or_else(|| {
            ["get", "is", "set"].into_iter().find_map(|p| {
                d.method_name
                    .strip_prefix(p)
                    .filter(|s| !s.is_empty())
                    .map(lower_first)
            })
        })
    }
    pub fn infer_kind(d: &AccessorMethodDeclaration) -> AccessorKind {
        if d.method_name.starts_with("set") && d.parameter_descriptors.len() == 1 {
            AccessorKind::Setter
        } else if d.method_name.starts_with("get")
            || d.method_name.starts_with("is")
            || d.return_type_descriptor
                .as_deref()
                .is_some_and(|r| r != "V")
                && d.parameter_descriptors.is_empty()
        {
            AccessorKind::Getter
        } else if d.return_type_descriptor.as_deref() == Some("V")
            && d.parameter_descriptors.len() == 1
        {
            AccessorKind::Setter
        } else {
            AccessorKind::Unknown
        }
    }
    pub fn complete_fields(&self, targets: &[String], prefix: &str) -> Vec<McCompletionItem> {
        targets
            .iter()
            .flat_map(|o| {
                self.class_index
                    .get_fields(o)
                    .into_iter()
                    .filter(|f| f.name.starts_with(prefix))
                    .map(|f| {
                        crate::awat::completion(
                            f.name.clone(),
                            Some(f.readable_type.clone()),
                            Some(f.descriptor.clone()),
                            format!("{} {}", f.name, f.readable_type),
                            f.name.clone(),
                            McCompletionKind::Field,
                            format!("0500_{}", f.name),
                            McCompletionMetadata {
                                source: "mixin.accessor".into(),
                                owner: Some(o.clone()),
                                name: Some(f.name),
                                descriptor: Some(f.descriptor),
                                namespace: None,
                            },
                        )
                    })
            })
            .collect()
    }
    pub fn validate(&self, targets: &[String], d: &AccessorMethodDeclaration) -> Vec<McDiagnostic> {
        let Some(name) = Self::infer_field_name(d) else {
            return vec![issue(
                C::ACCESSOR_FIELD_NOT_FOUND,
                format!("Cannot infer @Accessor field name from '{}'", d.method_name),
                d.range,
                &[],
            )];
        };
        let kind = Self::infer_kind(d);
        for owner in targets {
            if let Some(f) = self
                .class_index
                .get_fields(owner)
                .into_iter()
                .find(|f| f.name == name)
            {
                if kind == AccessorKind::Unknown {
                    return vec![issue(
                        C::ACCESSOR_SIGNATURE_MISMATCH,
                        format!(
                            "Unrecognized @Accessor method shape for '{}'",
                            d.method_name
                        ),
                        d.range,
                        &[],
                    )];
                }
                let good = match kind {
                    AccessorKind::Getter => {
                        d.return_type_descriptor.as_deref() == Some(f.descriptor.as_str())
                    }
                    AccessorKind::Setter => {
                        d.return_type_descriptor.as_deref() == Some("V")
                            && d.parameter_descriptors == [f.descriptor]
                    }
                    _ => false,
                };
                return if good {
                    vec![]
                } else {
                    vec![issue(
                        C::ACCESSOR_SIGNATURE_MISMATCH,
                        format!("@Accessor signature mismatch for '{}'", d.method_name),
                        d.range,
                        &[],
                    )]
                };
            }
        }
        vec![issue(
            C::ACCESSOR_FIELD_NOT_FOUND,
            format!("@Accessor field '{name}' not found in mixin targets"),
            d.range,
            &[("field", name)],
        )]
    }
    pub fn generate_method_stub(
        field: &FieldIndexEntry,
        kind: AccessorKind,
        indent: &str,
    ) -> String {
        match kind {
            AccessorKind::Getter => format!(
                "{indent}@Accessor(\"{}\")\n{indent}{} get{}();\n",
                field.name,
                field.readable_type,
                upper_first(&field.name)
            ),
            AccessorKind::Setter => format!(
                "{indent}@Accessor(\"{}\")\n{indent}void set{}({} value);\n",
                field.name,
                upper_first(&field.name),
                field.readable_type
            ),
            AccessorKind::Unknown => String::new(),
        }
    }
    pub fn readable_field_type(descriptor: &str) -> String {
        parse_field_descriptor(descriptor).map_or_else(|_| descriptor.into(), |t| render(&t))
    }
}
pub struct InvokerService<'a> {
    pub class_index: &'a dyn ClassIndex,
}
impl<'a> InvokerService<'a> {
    pub fn new(class_index: &'a dyn ClassIndex) -> Self {
        Self { class_index }
    }
    pub fn infer_target_name(d: &InvokerMethodDeclaration) -> Option<String> {
        d.explicit_target_name.clone().or_else(|| {
            d.method_name
                .strip_prefix("invoke")
                .filter(|s| !s.is_empty())
                .map(lower_first)
        })
    }
    pub fn complete_methods(&self, targets: &[String], prefix: &str) -> Vec<McCompletionItem> {
        targets
            .iter()
            .flat_map(|o| {
                self.class_index
                    .get_methods(o)
                    .into_iter()
                    .filter(|m| m.name.starts_with(prefix))
                    .map(|m| {
                        crate::awat::completion(
                            m.readable_signature.clone(),
                            Some(o.replace('/', ".")),
                            Some(m.descriptor.clone()),
                            format!("{} {}", m.name, m.readable_signature),
                            m.name.clone(),
                            McCompletionKind::Value,
                            format!("0600_{}", m.name),
                            McCompletionMetadata {
                                source: "mixin.invoker".into(),
                                owner: Some(o.clone()),
                                name: Some(m.name),
                                descriptor: Some(m.descriptor),
                                namespace: None,
                            },
                        )
                    })
            })
            .collect()
    }
    pub fn validate(&self, targets: &[String], d: &InvokerMethodDeclaration) -> Vec<McDiagnostic> {
        let Some(name) = Self::infer_target_name(d) else {
            return vec![issue(
                C::INVOKER_METHOD_NOT_FOUND,
                format!("Cannot infer @Invoker target from '{}'", d.method_name),
                d.range,
                &[],
            )];
        };
        let desc = format!(
            "({}){}",
            d.parameter_descriptors.join(""),
            d.return_type_descriptor.as_deref().unwrap_or("V")
        );
        for o in targets {
            let candidates = self
                .class_index
                .get_methods(o)
                .into_iter()
                .filter(|m| m.name == name)
                .collect::<Vec<_>>();
            if candidates.is_empty() {
                continue;
            }
            return if candidates.iter().any(|m| m.descriptor == desc) {
                vec![]
            } else {
                vec![issue(
                    C::INVOKER_DESCRIPTOR_MISMATCH,
                    format!("@Invoker descriptor mismatch for '{name}'"),
                    d.range,
                    &[("name", name), ("descriptor", desc)],
                )]
            };
        }
        vec![issue(
            C::INVOKER_METHOD_NOT_FOUND,
            format!("@Invoker target method '{name}' not found"),
            d.range,
            &[("name", name)],
        )]
    }
    pub fn generate_method_stub(method: &MethodIndexEntry, indent: &str) -> String {
        let name = format!("invoke{}", upper_first(&method.name));
        format!(
            "{indent}@Invoker(\"{}\")\n{indent}{};\n",
            method.name,
            method.readable_signature.replace(&method.name, &name)
        )
    }
}
pub struct ShadowValidationService<'a> {
    pub class_index: &'a dyn ClassIndex,
}
impl<'a> ShadowValidationService<'a> {
    pub fn new(class_index: &'a dyn ClassIndex) -> Self {
        Self { class_index }
    }
    pub fn validate(
        &self,
        targets: &[String],
        d: &ShadowMemberDeclaration,
        prefix: Option<&str>,
        _remap: bool,
    ) -> Vec<McDiagnostic> {
        if targets.is_empty() {
            return vec![];
        }
        let name = prefix
            .filter(|p| !p.is_empty())
            .and_then(|p| d.name.strip_prefix(p))
            .unwrap_or(&d.name);
        for o in targets {
            let result = self.validate_one(o, name, d);
            if result.is_empty() {
                return vec![];
            }
            if targets.len() == 1 {
                return result;
            }
        }
        vec![issue(
            C::SHADOW_TARGET_NOT_FOUND,
            format!("@Shadow target '{name}' not found in mixin targets"),
            d.range,
            &[("name", name.into())],
        )]
    }
    fn validate_one(
        &self,
        owner: &str,
        name: &str,
        d: &ShadowMemberDeclaration,
    ) -> Vec<McDiagnostic> {
        let mismatch = || {
            vec![issue(
                C::SHADOW_DESCRIPTOR_MISMATCH,
                format!("@Shadow descriptor mismatch for '{name}'"),
                d.range,
                &[("name", name.into())],
            )]
        };
        let not_found = || {
            vec![issue(
                C::SHADOW_TARGET_NOT_FOUND,
                format!("@Shadow target not found: {name}"),
                d.range,
                &[("name", name.into())],
            )]
        };
        let (is_static, kind) = if d.is_method {
            let methods = self
                .class_index
                .get_methods(owner)
                .into_iter()
                .filter(|m| m.name == name)
                .collect::<Vec<_>>();
            if methods.is_empty() {
                return not_found();
            }
            if parse_method_descriptor(&d.descriptor).is_err() {
                return mismatch();
            }
            let Some(m) = methods.iter().find(|m| m.descriptor == d.descriptor) else {
                return mismatch();
            };
            (m.is_static, "method")
        } else {
            let Some(f) = self
                .class_index
                .get_fields(owner)
                .into_iter()
                .find(|f| f.name == name)
            else {
                return not_found();
            };
            if f.is_static != d.is_static {
                return vec![issue(
                    C::SHADOW_STATIC_MISMATCH,
                    format!("@Shadow static mismatch for field '{name}'"),
                    d.range,
                    &[],
                )];
            }
            if parse_field_descriptor(&d.descriptor).is_ok() && f.descriptor != d.descriptor {
                return mismatch();
            }
            (f.is_static, "field")
        };
        if is_static != d.is_static {
            vec![issue(
                C::SHADOW_STATIC_MISMATCH,
                format!("@Shadow static mismatch for {kind} '{name}'"),
                d.range,
                &[],
            )]
        } else {
            vec![]
        }
    }
    pub fn complete_fields(
        &self,
        targets: &[String],
        prefix: &str,
        shadow_prefix: Option<&str>,
    ) -> Vec<FieldIndexEntry> {
        let mut out = vec![];
        for o in targets {
            for f in self.class_index.get_fields(o) {
                let n = shadow_prefix
                    .and_then(|p| f.name.strip_prefix(p))
                    .unwrap_or(&f.name);
                if n.starts_with(prefix) && !out.iter().any(|v: &FieldIndexEntry| v.name == f.name)
                {
                    out.push(f);
                }
            }
        }
        out
    }
    pub fn complete_methods(
        &self,
        targets: &[String],
        prefix: &str,
        shadow_prefix: Option<&str>,
    ) -> Vec<MethodIndexEntry> {
        let mut out = vec![];
        for o in targets {
            for m in self.class_index.get_methods(o) {
                let n = shadow_prefix
                    .and_then(|p| m.name.strip_prefix(p))
                    .unwrap_or(&m.name);
                if m.name != "<init>"
                    && m.name != "<clinit>"
                    && n.starts_with(prefix)
                    && !out.iter().any(|v: &MethodIndexEntry| {
                        v.name == m.name && v.descriptor == m.descriptor
                    })
                {
                    out.push(m);
                }
            }
        }
        out
    }
}
pub struct OverwriteValidationService<'a> {
    pub class_index: &'a dyn ClassIndex,
}
impl<'a> OverwriteValidationService<'a> {
    pub fn new(class_index: &'a dyn ClassIndex) -> Self {
        Self { class_index }
    }
    pub fn validate(
        &self,
        targets: &[String],
        d: &OverwriteMethodDeclaration,
    ) -> Vec<McDiagnostic> {
        if targets.is_empty() {
            return vec![];
        }
        for o in targets {
            let methods = self
                .class_index
                .get_methods(o)
                .into_iter()
                .filter(|m| m.name == d.name)
                .collect::<Vec<_>>();
            let result = if methods.is_empty() {
                vec![issue(
                    C::OVERWRITE_TARGET_NOT_FOUND,
                    format!("@Overwrite target not found: {}", d.name),
                    d.range,
                    &[("name", d.name.clone())],
                )]
            } else {
                match methods
                    .iter()
                    .find(|m| m.descriptor == d.descriptor)
                    .filter(|_| parse_method_descriptor(&d.descriptor).is_ok())
                {
                    None => vec![issue(
                        C::OVERWRITE_DESCRIPTOR_MISMATCH,
                        format!("@Overwrite descriptor mismatch for '{}'", d.name),
                        d.range,
                        &[("name", d.name.clone())],
                    )],
                    Some(m) if m.is_static != d.is_static => vec![issue(
                        C::OVERWRITE_STATIC_MISMATCH,
                        format!("@Overwrite static mismatch for method '{}'", d.name),
                        d.range,
                        &[],
                    )],
                    Some(_) => vec![],
                }
            };
            if result.is_empty() {
                return vec![];
            }
            if targets.len() == 1 {
                return result;
            }
        }
        vec![issue(
            C::OVERWRITE_TARGET_NOT_FOUND,
            format!("@Overwrite target '{}' not found in mixin targets", d.name),
            d.range,
            &[("name", d.name.clone())],
        )]
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmbiguityPolicy {
    ExactDescriptorRequired,
    SingleCandidateIfDescriptorMissing,
    AllowAmbiguousForCompletion,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MethodResolution {
    Resolved(MethodIndexEntry),
    MissingDescriptor(Vec<MethodIndexEntry>),
    DescriptorMismatch(Vec<MethodIndexEntry>),
    Ambiguous(Vec<MethodIndexEntry>),
    UnresolvedOwner(String),
    NotFound,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldResolution {
    Resolved(FieldIndexEntry),
    MissingDescriptor(Vec<FieldIndexEntry>),
    DescriptorMismatch(Vec<FieldIndexEntry>),
    Ambiguous(Vec<FieldIndexEntry>),
    UnresolvedOwner(String),
    NotFound,
}
pub trait MemberResolver {
    fn resolve_method(
        &self,
        owner: &str,
        name: &str,
        descriptor: Option<&str>,
        policy: AmbiguityPolicy,
    ) -> MethodResolution;
    fn resolve_field(
        &self,
        owner: &str,
        name: &str,
        descriptor: Option<&str>,
        policy: AmbiguityPolicy,
    ) -> FieldResolution;
}
pub struct ClassIndexMemberResolver<'a> {
    pub class_index: &'a dyn ClassIndex,
}
impl<'a> ClassIndexMemberResolver<'a> {
    pub fn new(class_index: &'a dyn ClassIndex) -> Self {
        Self { class_index }
    }
    fn missing_owner(&self, o: &str) -> bool {
        self.class_index
            .find_classes(o.rsplit('/').next().unwrap_or(o), 1)
            .is_empty()
            && self.class_index.get_methods(o).is_empty()
            && self.class_index.get_fields(o).is_empty()
    }
}
impl MemberResolver for ClassIndexMemberResolver<'_> {
    fn resolve_method(
        &self,
        owner: &str,
        name: &str,
        descriptor: Option<&str>,
        policy: AmbiguityPolicy,
    ) -> MethodResolution {
        if self.missing_owner(owner) {
            return MethodResolution::UnresolvedOwner(owner.into());
        }
        let c = self
            .class_index
            .get_methods(owner)
            .into_iter()
            .filter(|m| m.name == name)
            .collect::<Vec<_>>();
        if c.is_empty() {
            return MethodResolution::NotFound;
        }
        if let Some(d) = descriptor {
            return c.iter().find(|m| m.descriptor == d).cloned().map_or_else(
                || MethodResolution::DescriptorMismatch(c),
                MethodResolution::Resolved,
            );
        }
        match policy {
            AmbiguityPolicy::ExactDescriptorRequired => MethodResolution::MissingDescriptor(c),
            AmbiguityPolicy::SingleCandidateIfDescriptorMissing if c.len() == 1 => {
                MethodResolution::Resolved(c.into_iter().next().unwrap())
            }
            _ => MethodResolution::Ambiguous(c),
        }
    }
    fn resolve_field(
        &self,
        owner: &str,
        name: &str,
        descriptor: Option<&str>,
        policy: AmbiguityPolicy,
    ) -> FieldResolution {
        if self.missing_owner(owner) {
            return FieldResolution::UnresolvedOwner(owner.into());
        }
        let c = self
            .class_index
            .get_fields(owner)
            .into_iter()
            .filter(|f| f.name == name)
            .collect::<Vec<_>>();
        if c.is_empty() {
            return FieldResolution::NotFound;
        }
        if let Some(d) = descriptor {
            return c.iter().find(|f| f.descriptor == d).cloned().map_or_else(
                || FieldResolution::DescriptorMismatch(c),
                FieldResolution::Resolved,
            );
        }
        match policy {
            AmbiguityPolicy::ExactDescriptorRequired => FieldResolution::MissingDescriptor(c),
            AmbiguityPolicy::SingleCandidateIfDescriptorMissing if c.len() == 1 => {
                FieldResolution::Resolved(c.into_iter().next().unwrap())
            }
            _ => FieldResolution::Ambiguous(c),
        }
    }
}
