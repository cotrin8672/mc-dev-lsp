//! Mixin policy expressed as ktlsp shared semantic requirements.
use std::ops::Range;

use ktcore::diagnostics::{Diagnostic, Severity};
use ktcore::extensions::source::{AnnotationArgument, AnnotationValue, SourceAnnotation};
use ktcore::extensions::*;
use ktcore::knowledge::Knowledge;
use ktcore::types::TypeRef;

pub const PROVIDER_ID: &str = "mcdev.mixin";

/// Register with ktlsp's analysis provider registry to extend standard LSP operations.
pub struct McdevMixinProvider;

fn annotation(read: &mut ReadSession<'_>, annotation: &SourceAnnotation, name: &str) -> bool {
    matches!(read.resolve_type_ref(&annotation.ty), Knowledge::Found(ty) if ty.qualified_name() == name)
}

fn hint(
    read: &mut ReadSession<'_>,
    range: Range<usize>,
    message: &str,
) -> Result<(), ContractError> {
    read.diagnostic(Diagnostic {
        start_byte: range.start,
        end_byte: range.end,
        severity: Severity::Hint,
        code: None,
        message: format!("[mcdev.mixin.incomplete] {message}"),
    })
}

fn target_values<'a>(
    argument: &'a AnnotationArgument,
    strings: bool,
    values: &mut Vec<&'a AnnotationValue>,
) -> bool {
    match &argument.value {
        Knowledge::Found(AnnotationValue::Array(items)) => items
            .iter()
            .all(|item| target_values(item, strings, values)),
        Knowledge::Found(value @ (AnnotationValue::Class(_) | AnnotationValue::String { .. })) => {
            if matches!(value, AnnotationValue::String { .. }) != strings {
                return false;
            }
            values.push(value);
            true
        }
        _ => false,
    }
}

impl AnalysisProvider for McdevMixinProvider {
    fn id(&self) -> &str {
        PROVIDER_ID
    }

    fn analyze(&self, read: &mut ReadSession<'_>) -> Result<(), ContractError> {
        let declarations = read.declarations().to_vec();
        for owner in &declarations {
            let Some(mixin) = owner.annotations.iter().find(|item| {
                annotation(read, item, "org.spongepowered.asm.mixin.Mixin")
                    && item.use_site_target.is_none()
            }) else {
                continue;
            };
            let mut values = Vec::new();
            let known = mixin
                .arguments
                .iter()
                .filter(|item| matches!(item.name.as_deref(), None | Some("value" | "targets")))
                .all(|item| {
                    target_values(item, item.name.as_deref() == Some("targets"), &mut values)
                });
            if !known || values.len() != 1 {
                hint(
                    read,
                    mixin.range.clone(),
                    "exactly one known target is supported",
                )?;
                continue;
            }
            let target = match values[0] {
                AnnotationValue::Class(ty) => read.resolve_type_ref(ty),
                AnnotationValue::String { value, .. } => read.resolve_type(value),
                _ => unreachable!(),
            };
            let owner_range = owner.definition.start_byte..owner.definition.end_byte;
            let spelling = read.source()[owner_range.clone()].trim_matches('`');
            // shortcut: self-name collisions stay incomplete until the host exposes declaration_type.
            let own_type = read.resolve_type_ref(&TypeRef::simple(spelling));
            let own_members = match own_type {
                Knowledge::Found(ty) if ty.definition() == &owner.definition => {
                    read.declared_members(&ty)?.0
                }
                _ => {
                    hint(read, owner_range, "authored owner identity is incomplete")?;
                    continue;
                }
            };
            for declaration in declarations
                .iter()
                .filter(|item| item.owner.as_ref() == Some(&owner.definition))
            {
                let Some(shadow) = declaration
                    .annotations
                    .iter()
                    .find(|item| annotation(read, item, "org.spongepowered.asm.mixin.Shadow"))
                else {
                    continue;
                };
                let range = declaration.definition.start_byte..declaration.definition.end_byte;
                let Some(member) = own_members
                    .iter()
                    .find(|item| item.id.definition() == &declaration.definition)
                else {
                    hint(read, range, "authored member facts are incomplete")?;
                    continue;
                };
                if !matches!(shadow.use_site_target.as_deref(), None | Some("field"))
                    || shadow.use_site_target.is_some() && member.kind != MemberKind::Field
                {
                    hint(read, range, "accessor use-site targets are not supported")?;
                    continue;
                }
                if shadow.arguments.iter().any(|item| {
                    item.name.as_deref() == Some("aliases")
                        && !matches!(&item.value, Knowledge::Found(AnnotationValue::Array(items)) if items.is_empty())
                }) {
                    // shortcut: alias fallback needs independent rename semantics; add it as a separate slice.
                    hint(read, range, "Shadow aliases are not supported")?;
                    continue;
                }
                let mut prefix = "shadow$";
                if let Some(argument) = shadow
                    .arguments
                    .iter()
                    .find(|item| item.name.as_deref() == Some("prefix"))
                {
                    let Knowledge::Found(AnnotationValue::String { value, .. }) = &argument.value
                    else {
                        hint(read, range, "Shadow prefix is not a known string")?;
                        continue;
                    };
                    prefix = value;
                }
                let spelling = &read.source()[range.clone()];
                let quoted = spelling.starts_with('`') && spelling.ends_with('`');
                let mut editable =
                    range.start + usize::from(quoted)..range.end - usize::from(quoted);
                let authored_name = &read.source()[editable.clone()];
                if member.kind == MemberKind::Field && authored_name.starts_with(prefix) {
                    read.diagnostic(Diagnostic {
                        start_byte: range.start,
                        end_byte: range.end,
                        severity: Severity::Error,
                        code: None,
                        message: "[mcdev.mixin.shadow.field-prefix] Shadow fields cannot use the method prefix".into(),
                    })?;
                    continue;
                }
                let (name, link) = if member.kind == MemberKind::Method && !prefix.is_empty() {
                    if let Some(name) = authored_name.strip_prefix(prefix) {
                        editable.start += prefix.len();
                        (name.to_owned(), NameLink::Prefixed(prefix.into()))
                    } else {
                        (authored_name.to_owned(), NameLink::Direct)
                    }
                } else {
                    (authored_name.to_owned(), NameLink::Direct)
                };
                if name.is_empty() {
                    hint(read, range, "Shadow prefix leaves an empty target name")?;
                    continue;
                }
                let authored = read.declaration(range.clone())?;
                let requirement = read.require(MemberRequirement {
                    target: target.clone(),
                    authored: Some(authored),
                    names: MemberNames::Exact(vec![name]),
                    kind: member.kind,
                    signature: member.signature.clone(),
                    dispatch: member.dispatch.clone(),
                    name_link: link,
                })?;
                read.bind_site(requirement, range.clone(), editable, RenameSubject::Target)?;
                match &read.resolution(requirement)?.binding {
                    Knowledge::Found(_) => {}
                    Knowledge::Unknown(_) => {
                        hint(read, range, "Shadow target binding is incomplete")?
                    }
                    Knowledge::DefinitelyAbsent => read.diagnostic(Diagnostic {
                        start_byte: range.start,
                        end_byte: range.end,
                        severity: Severity::Error,
                        code: None,
                        message: "[mcdev.mixin.shadow.missing] no matching Shadow target member"
                            .into(),
                    })?,
                }
            }
        }
        Ok(())
    }
}
