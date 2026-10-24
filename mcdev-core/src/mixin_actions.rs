//! Symbolic Mixin quickfix generation and lossless materialization into byte edits.
use crate::{
    mixin_diagnostics::MixinDiagnosticCodes as C,
    mixin_edits::{MixinConfigEditor, MixinSide, MixinSideResolver},
    mixin_source::*,
    mixin_validation::{AccessorService, InvokerService},
    model::*,
};
#[derive(Default)]
pub struct MixinCodeActionService<'a> {
    pub accessor_service: Option<AccessorService<'a>>,
    pub invoker_service: Option<InvokerService<'a>>,
}
impl<'a> MixinCodeActionService<'a> {
    pub fn new(index: &'a dyn ClassIndex) -> Self {
        Self {
            accessor_service: Some(AccessorService::new(index)),
            invoker_service: Some(InvokerService::new(index)),
        }
    }
    pub fn fixes_for_diagnostics(
        &self,
        diagnostics: &[McDiagnostic],
        document_uri: &str,
        source: &str,
        _config_content: Option<&str>,
        config_path: Option<&str>,
        mixin_package: Option<&str>,
        class_index: Option<&dyn ClassIndex>,
    ) -> Vec<McFix> {
        let mut out = vec![];
        for d in diagnostics {
            match d.code.as_str() {
                C::MIXIN_CLASS_NOT_LISTED_IN_CONFIG => {
                    let Some(name) = d.metadata.get("mixinClass") else {
                        continue;
                    };
                    let Some(path) =
                        config_path.or_else(|| d.metadata.get("configPath").map(String::as_str))
                    else {
                        continue;
                    };
                    let array = match MixinSideResolver::explicit_side_from_source(source) {
                        Some(MixinSide::Client) => "client",
                        Some(MixinSide::Server) => "server",
                        None => "mixins",
                    };
                    out.push(McFix::AddMixinConfigEntryFix {
                        title: format!("Add '{name}' to mixin config"),
                        config_path: path.into(),
                        mixin_class_name: name.clone(),
                        mixin_package: mixin_package.map(str::to_string),
                        array_name: array.into(),
                    });
                }
                C::AMBIGUOUS_INJECT_METHOD => {
                    let (Some(name), Some(index)) = (d.metadata.get("method"), class_index) else {
                        continue;
                    };
                    let methods = MixinTargetResolver::resolve_targets_from_source(source, index)
                        .iter()
                        .flat_map(|o| index.get_methods(o))
                        .filter(|m| &m.name == name)
                        .collect::<Vec<_>>();
                    if methods.len() != 1 {
                        continue;
                    }
                    let (Some(start), Some(end)) = (
                        position_to_offset(source, d.range.start),
                        position_to_offset(source, d.range.end),
                    ) else {
                        continue;
                    };
                    out.push(McFix::AddMethodDescriptorFix {
                        title: "Add descriptor to method target".into(),
                        document_uri: document_uri.into(),
                        start_offset: start,
                        end_offset: end,
                        method_name: name.clone(),
                        descriptor: methods[0].descriptor.clone(),
                    });
                }
                C::ACCESSOR_FIELD_NOT_FOUND => {
                    let (Some(name), Some(index), Some(_)) =
                        (d.metadata.get("field"), class_index, &self.accessor_service)
                    else {
                        continue;
                    };
                    let field = MixinTargetResolver::resolve_targets_from_source(source, index)
                        .iter()
                        .find_map(|o| index.get_fields(o).into_iter().find(|f| &f.name == name));
                    let Some(field) = field else { continue };
                    let stub = if JavaTypeDescriptorResolver::imports_for(source).is_kotlin {
                        let Some(ty) = kotlin_type(&field.descriptor) else {
                            continue;
                        };
                        format!(
                            "\n    @Accessor(\"{}\")\n    fun get{}(): {}\n",
                            field.name,
                            upper_first(&field.name),
                            ty
                        )
                    } else {
                        let Ok(ty) = crate::descriptor::parse_field_descriptor(&field.descriptor)
                        else {
                            continue;
                        };
                        format!(
                            "\n    @Accessor(\"{}\")\n    {} get{}();\n",
                            field.name,
                            java_type(&ty),
                            upper_first(&field.name)
                        )
                    };
                    out.push(McFix::GenerateAccessorMethodFix {
                        title: format!("Generate @Accessor getter for '{name}'"),
                        document_uri: document_uri.into(),
                        insert_offset: body_insert_offset(source),
                        method_source: stub,
                        field_name: name.clone(),
                        is_getter: true,
                    });
                }
                C::INVOKER_METHOD_NOT_FOUND => {
                    let (Some(name), Some(index), Some(_)) =
                        (d.metadata.get("name"), class_index, &self.invoker_service)
                    else {
                        continue;
                    };
                    let methods = MixinTargetResolver::resolve_targets_from_source(source, index)
                        .iter()
                        .flat_map(|o| index.get_methods(o))
                        .filter(|m| &m.name == name)
                        .collect::<Vec<_>>();
                    if methods.len() != 1 {
                        continue;
                    }
                    let Some(stub) = invoker_stub(
                        &methods[0],
                        JavaTypeDescriptorResolver::imports_for(source).is_kotlin,
                    ) else {
                        continue;
                    };
                    out.push(McFix::GenerateInvokerMethodFix {
                        title: format!("Generate @Invoker for '{name}'"),
                        document_uri: document_uri.into(),
                        insert_offset: body_insert_offset(source),
                        method_source: stub,
                        target_method_name: name.clone(),
                    });
                }
                _ => (),
            }
        }
        out
    }
    pub fn apply_mixin_config_fix(fix: &McFix, current_content: &str) -> Option<McFix> {
        let McFix::AddMixinConfigEntryFix {
            config_path,
            mixin_class_name,
            array_name,
            ..
        } = fix
        else {
            return None;
        };
        let result =
            MixinConfigEditor::add_entry(current_content, mixin_class_name, array_name).ok()?;
        if !result.added {
            return None;
        }
        let edits = if let Some(d) = result.delta {
            vec![McTextEdit {
                start_offset: d.start_offset,
                end_offset: d.end_offset,
                new_text: d.new_text,
            }]
        } else {
            vec![McTextEdit {
                start_offset: 0,
                end_offset: current_content.len(),
                new_text: result.content,
            }]
        };
        let mut f = crate::awat::workspace_edit(fix, config_path, edits);
        if let McFix::WorkspaceEditFix { metadata, .. } = &mut f {
            metadata.insert("arrayName".into(), result.array_name);
        }
        Some(f)
    }
    pub fn apply_method_descriptor_fix(fix: &McFix, source: &str) -> Option<McFix> {
        let McFix::AddMethodDescriptorFix {
            document_uri,
            start_offset,
            end_offset,
            method_name,
            descriptor,
            ..
        } = fix
        else {
            return None;
        };
        if !source.is_char_boundary(*start_offset)
            || !source.is_char_boundary(*end_offset)
            || start_offset > end_offset
            || crate::descriptor::parse_method_descriptor(descriptor).is_err()
        {
            return None;
        }
        let value = format!("{method_name}{descriptor}");
        let new_text = if JavaTypeDescriptorResolver::imports_for(source).is_kotlin {
            let raw = source_tokens(source, true).iter().any(|t| {
                matches!(t.kind, SourceTokenKind::String { raw: true, .. })
                    && t.start <= *start_offset
                    && t.end >= *end_offset
            });
            if raw {
                value.replace('$', "${'$'}")
            } else {
                value.replace('$', "\\$")
            }
        } else {
            value
        };
        Some(crate::awat::workspace_edit(
            fix,
            document_uri,
            vec![McTextEdit {
                start_offset: *start_offset,
                end_offset: *end_offset,
                new_text,
            }],
        ))
    }
    pub fn apply_fix(fix: &McFix, content: &str) -> Option<McFix> {
        if let Some(f) = Self::apply_mixin_config_fix(fix, content)
            .or_else(|| Self::apply_method_descriptor_fix(fix, content))
        {
            return Some(f);
        }
        let (uri, offset, stub) = match fix {
            McFix::GenerateAccessorMethodFix {
                document_uri,
                insert_offset,
                method_source,
                ..
            }
            | McFix::GenerateInvokerMethodFix {
                document_uri,
                insert_offset,
                method_source,
                ..
            } => (document_uri, *insert_offset, method_source),
            McFix::WorkspaceEditFix { edits, .. } => {
                return apply_text_edits(content, edits).ok().map(|_| fix.clone())
            }
            _ => return None,
        };
        if !content.is_char_boundary(offset) {
            return None;
        }
        let mut new_text = if content.contains("\r\n") {
            stub.replace("\r\n", "\n").replace('\n', "\r\n")
        } else {
            stub.clone()
        };
        let mut edits = vec![];
        let annotation = if matches!(fix, McFix::GenerateAccessorMethodFix { .. }) {
            "Accessor"
        } else {
            "Invoker"
        };
        if new_text.contains(&format!("@{annotation}")) {
            let reference = crate::mixin_edits::MixinImportEditBuilder::reference_for_internal_name(
                content,
                &format!("org/spongepowered/asm/mixin/gen/{annotation}"),
                &Default::default(),
            );
            new_text = new_text.replacen(
                &format!("@{annotation}"),
                &format!("@{}", reference.text),
                1,
            );
            if let Some(fqn) = reference.import_fqn {
                if let Some(edit) =
                    crate::mixin_edits::MixinImportEditBuilder::build_import_edit(content, &fqn)
                {
                    edits.push(edit);
                }
            }
        }
        edits.push(McTextEdit {
            start_offset: offset,
            end_offset: offset,
            new_text,
        });
        Some(crate::awat::workspace_edit(fix, uri, edits))
    }
}
fn upper_first(value: &str) -> String {
    let mut c = value.chars();
    c.next().map_or_else(String::new, |x| {
        x.to_uppercase().collect::<String>() + c.as_str()
    })
}
fn body_insert_offset(source: &str) -> usize {
    let ts = source_tokens(
        source,
        JavaTypeDescriptorResolver::imports_for(source).is_kotlin,
    );
    let declaration = ts
        .iter()
        .position(|t| matches!(t.text.as_str(), "class" | "interface" | "object"));
    declaration
        .and_then(|i| ts[i..].iter().find(|t| t.text == "{").map(|t| t.end))
        .unwrap_or(source.len())
}
fn kotlin_type(descriptor: &str) -> Option<String> {
    use crate::descriptor::JvmType;
    fn render(t: &JvmType) -> String {
        match t {
            JvmType::ByteType => "Byte".into(),
            JvmType::CharType => "Char".into(),
            JvmType::DoubleType => "Double".into(),
            JvmType::FloatType => "Float".into(),
            JvmType::IntType => "Int".into(),
            JvmType::LongType => "Long".into(),
            JvmType::ShortType => "Short".into(),
            JvmType::BooleanType => "Boolean".into(),
            JvmType::VoidType => "Unit".into(),
            JvmType::ObjectType(n) => n.replace(['/', '$'], "."),
            JvmType::ArrayType(c) => match c.as_ref() {
                JvmType::ObjectType(_) | JvmType::ArrayType(_) => format!("Array<{}>", render(c)),
                _ => format!("{}Array", render(c)),
            },
        }
    }
    crate::descriptor::parse_field_descriptor(descriptor)
        .ok()
        .map(|t| render(&t))
}
fn invoker_stub(method: &MethodIndexEntry, kotlin: bool) -> Option<String> {
    let d = crate::descriptor::parse_method_descriptor(&method.descriptor).ok()?;
    let method_name = if method.name == "<init>" {
        "newInstance".into()
    } else {
        format!("invoke{}", upper_first(&method.name))
    };
    let types = d
        .parameters
        .iter()
        .map(crate::descriptor::to_descriptor)
        .collect::<Vec<_>>();
    if kotlin {
        let params = types
            .iter()
            .enumerate()
            .map(|(i, d)| kotlin_type(d).map(|t| format!("arg{i}: {t}")))
            .collect::<Option<Vec<_>>>()?
            .join(", ");
        let return_type = if crate::descriptor::to_descriptor(&d.return_type) == "V" {
            "Unit".into()
        } else {
            kotlin_type(&crate::descriptor::to_descriptor(&d.return_type))?
        };
        Some(format!(
            "\n    @Invoker(\"{}\")\n    fun {method_name}({params}): {return_type}\n",
            method.name
        ))
    } else {
        let params = d
            .parameters
            .iter()
            .enumerate()
            .map(|(i, t)| format!("{} arg{i}", java_type(t)))
            .collect::<Vec<_>>()
            .join(", ");
        Some(format!(
            "\n    @Invoker(\"{}\")\n    {} {method_name}({params});\n",
            method.name,
            java_type(&d.return_type)
        ))
    }
}
fn java_type(t: &crate::descriptor::JvmType) -> String {
    use crate::descriptor::JvmType;
    match t {
        JvmType::ObjectType(n) => n.replace(['/', '$'], "."),
        JvmType::ArrayType(c) => format!("{}[]", java_type(c)),
        _ => crate::descriptor::render(t),
    }
}
