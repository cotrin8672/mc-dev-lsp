use super::*;
use mcdev_core::{
    extras_model::*,
    host_source::{project_host_handler_parameter, HostFact},
    mixin_source::{ClassIndexJavaTypeLookup, JavaTypeLookup, JavaTypeResolutionContext},
    mixinextras_handler::HandlerSignatureService,
};
mod selectors;

pub(super) fn source_facts(
    source: &str,
    uri: &str,
    qualified_name: &str,
    targets: &[String],
    sites: HostFact<Vec<MixinExtrasAnnotationSite>>,
) -> HostHandlerSource {
    use sha2::{Digest, Sha256};
    HostHandlerSource {
        document_uri: uri.into(),
        sha256: format!("{:x}", Sha256::digest(source.as_bytes())),
        qualified_name: qualified_name.into(),
        targets: targets.to_vec(),
        sites,
        slice_selections: vec![],
    }
}

pub(super) fn project_handler_sites(
    declaration: &Declaration,
    handler: &HandlerMethodDeclaration,
    identities: &BTreeMap<String, Fact<TypeFact>>,
    method_binaries: &BTreeMap<String, String>,
    source: &str,
) -> Result<Vec<(MixinExtrasAnnotationSite, Option<HostMethodSlice>)>, FactsError> {
    let mut sites = Vec::new();
    for annotation in &declaration.annotations {
        let ty = require_type(identities, &annotation.type_ref)?;
        let Some(kind) = MixinAnnotation::from_official_fqn(&ty.qualified_name)
            .and_then(MixinExtrasAnnotation::from_mixin_annotation)
            .filter(|kind| kind.is_sugar_handler_injector())
        else {
            continue;
        };
        let normalized = convert_annotation(annotation, identities, method_binaries, source)?;
        sites.extend(selectors::project_sites(
            kind,
            &normalized,
            handler,
            source,
        )?);
    }
    Ok(sites)
}

impl HostSemanticProvider {
    pub(super) fn recovered_local_handler<'a>(
        &self,
        declarations: &'a [Declaration],
        owner: &Declaration,
        identities: &BTreeMap<String, Fact<TypeFact>>,
        source: &str,
        imports: &JavaSourceImports,
    ) -> Option<(&'a Declaration, HandlerMethodDeclaration)> {
        if imports.is_kotlin {
            return None;
        }
        let position = self.annotation_completion_position?;
        let cursor = position_to_offset(source, position)?;
        let context = AnnotationContextExtractor::extract_at_offset(source, cursor)?;
        if context.annotation != MixinAnnotation::Local
            || context.slot != AnnotationSlot::Value
            || context.resolved_annotation_fqn.as_deref()
                != Some("com.llamalad7.mixinextras.sugar.Local")
            || !matches!(
                context.attribute_name.as_deref(),
                Some("ordinal" | "index" | "name")
            )
        {
            return None;
        }
        declarations
            .iter()
            .filter(|declaration| {
                declaration.owner.as_deref() == Some(owner.id.as_str())
                    && declaration.source_kind.as_deref() == Some("Function")
                    && declaration
                        .parameters
                        .as_ref()
                        .is_none_or(|parameters| matches!(parameters, Fact::Unknown { .. }))
            })
            .find_map(|declaration| {
                let header = declaration.header_range.as_ref()?;
                if !(header.start <= cursor && cursor < header.end) {
                    return None;
                }
                let has_injector = declaration.annotations.iter().any(|annotation| {
                    identities
                        .get(&annotation.type_ref)
                        .and_then(Fact::found)
                        .and_then(|ty| MixinAnnotation::from_official_fqn(&ty.qualified_name))
                        .and_then(MixinExtrasAnnotation::from_mixin_annotation)
                        .is_some_and(|kind| {
                            kind.is_sugar_handler_injector()
                                && kind != MixinExtrasAnnotation::WrapMethod
                        })
                });
                if !has_injector {
                    return None;
                }
                let handler = HandlerSignatureService::parse_handler_method(
                    source,
                    header.start,
                    Some(imports),
                )?;
                if declaration.range.text(source).ok()? != handler.method_name
                    || text_range(source, header).ok()?.end < handler.range.end
                    || context.annotation_end_offset > header.end
                {
                    return None;
                }
                let local_range = mcdev_core::mixin_source::source_range(
                    source,
                    context.annotation_start_offset,
                    context.annotation_end_offset,
                );
                handler
                    .parameters
                    .iter()
                    .any(|parameter| {
                        matches!(
                            parameter.sugar_spec,
                            Some(HandlerParameterSugarSpec::Local(_))
                        ) && parameter.sugar_annotation_range == Some(local_range)
                    })
                    .then_some((declaration, handler))
            })
    }

    pub(super) fn normalized_handler_sites(
        &self,
        declaration: &Declaration,
        parameter_types: &[HostTypeFact],
        return_type: &HostTypeFact,
        dispatch: &str,
        identities: &BTreeMap<String, Fact<TypeFact>>,
        method_binaries: &BTreeMap<String, String>,
        registry: &mut Registry,
        context: Option<&SourceDescriptor>,
        source: &str,
    ) -> Result<Vec<(MixinExtrasAnnotationSite, Option<HostMethodSlice>)>, FactsError> {
        let parameters = declaration
            .parameters
            .as_ref()
            .and_then(Fact::found)
            .ok_or_else(|| {
                FactsError::Unknown("authored handler parameter facts are unavailable".into())
            })?;
        if parameters.len() != parameter_types.len() {
            return Err(FactsError::Unknown("authored parameters and the exact declaration signature have no supported correspondence".into()));
        }
        let mut references = BTreeSet::new();
        for parameter in parameters {
            for annotation in &parameter.annotations {
                for argument in &annotation.arguments {
                    collect_class_refs(argument, source, &mut references)?;
                }
            }
        }
        let types = self.resolve_refs(&references, registry, context)?;
        let queries = references
            .iter()
            .map(|reference| {
                require_type(&types, reference).map(|ty| json!({"op":"typeJvm","target":ty.id}))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let jvm: Vec<crate::host_facts::draft::TypeJvm> = self.read(&queries)?;
        let mut binaries = BTreeMap::new();
        for (reference, facts) in references.iter().zip(jvm) {
            facts.binary_name.validate().map_err(FactsError::Protocol)?;
            let binary = facts.binary_name.found().ok_or_else(|| {
                FactsError::Unknown("parameter annotation JVM type facts are unavailable".into())
            })?;
            if descriptor::parse_field_descriptor(&binary.descriptor).ok()
                != Some(JvmType::ObjectType(binary.internal_name.clone()))
            {
                return Err(FactsError::Protocol(
                    "parameter annotation JVM identity is inconsistent".into(),
                ));
            }
            binaries.insert(reference.clone(), binary.internal_name.clone());
        }
        let mut projected = Vec::new();
        let source_operation_generics =
            self.java_operation_generics(source, declaration, parameters.len());
        // Both lists describe this checked authored declaration; no JVM index or local slot is issued.
        for (parameter, ty) in parameters.iter().zip(parameter_types) {
            let annotations = parameter
                .annotations
                .iter()
                .map(|annotation| {
                    if annotation
                        .use_site_target
                        .as_deref()
                        .is_some_and(|target| target != "param")
                    {
                        return Err(FactsError::Unknown(
                            "parameter annotation use-site is unavailable".into(),
                        ));
                    }
                    convert_annotation(annotation, identities, &binaries, source)
                })
                .collect::<Result<Vec<_>, _>>()?;
            match project_host_handler_parameter(
                source,
                &parameter.name,
                ty,
                &(parameter.range.start..parameter.range.end),
                &annotations,
            ) {
                HostFact::Known(mut projected_parameter) => {
                    if projected_parameter.is_operation {
                        if let Some((name, descriptor)) = source_operation_generics
                            .as_ref()
                            .and_then(|generics| {
                                generics.iter().find(|(parameter_name, start, end, _, _)| {
                                    parameter_name == &parameter.name
                                        && *start == parameter.range.start
                                        && *end == parameter.range.end
                                })
                            })
                            .map(|(_, _, _, name, descriptor)| (name, descriptor))
                        {
                            projected_parameter.operation_generic_name = Some(name.clone());
                            projected_parameter.operation_generic_descriptor =
                                Some(descriptor.clone());
                        }
                    }
                    projected.push(projected_parameter)
                }
                HostFact::Unavailable { reason } => return Err(FactsError::Unknown(reason)),
                HostFact::Missing => {
                    return Err(FactsError::Unknown(
                        "handler parameter is unavailable".into(),
                    ))
                }
            }
        }
        let range = declaration
            .header_range
            .as_ref()
            .ok_or_else(|| FactsError::Protocol("missing handler header range".into()))?;
        let return_type_descriptor = return_type
            .descriptor()
            .ok_or_else(|| FactsError::Unknown("handler return type is unavailable".into()))?;
        let handler =
            HandlerMethodDeclaration {
                method_name: declaration
                    .name
                    .clone()
                    .unwrap_or_default()
                    .trim_matches('`')
                    .into(),
                return_type_name: descriptor::render(&return_type.jvm_type().ok_or_else(|| {
                    FactsError::Unknown("handler return type is unavailable".into())
                })?),
                return_type_descriptor: Some(return_type_descriptor),
                parameters: projected,
                range: text_range(source, range)?,
                is_static: dispatch == "static",
                has_method_level_coerce: declaration.annotations.iter().any(|annotation| {
                    identities
                        .get(&annotation.type_ref)
                        .and_then(Fact::found)
                        .is_some_and(|ty| {
                            ty.qualified_name == mcdev_core::mixinextras_handler::COERCE_FQN
                        })
                }),
            };
        project_handler_sites(declaration, &handler, identities, method_binaries, source)
    }
}

impl HostSemanticProvider {
    fn java_operation_generics(
        &self,
        source: &str,
        declaration: &Declaration,
        parameter_count: usize,
    ) -> Option<Vec<(String, usize, usize, String, String)>> {
        let imports = JavaTypeDescriptorResolver::imports_for(source);
        if imports.is_kotlin {
            return None;
        }
        let start = declaration
            .full_range
            .as_ref()
            .or(declaration.header_range.as_ref())?
            .start;
        let handler = HandlerSignatureService::parse_handler_method(source, start, Some(&imports))?;
        if declaration
            .name
            .as_deref()
            .is_none_or(|name| handler.method_name == name.trim_matches('`'))
            && handler.parameters.len() == parameter_count
        {
            let class_lookup = self
                .class_index
                .as_deref()
                .map(|class_index| ClassIndexJavaTypeLookup { class_index });
            let context = JavaTypeResolutionContext {
                imports: &imports,
                lookup: class_lookup
                    .as_ref()
                    .map(|lookup| lookup as &dyn JavaTypeLookup),
            };
            Some(
                handler
                    .parameters
                    .into_iter()
                    .filter_map(|parameter| {
                        let generic = parameter.operation_generic_name?;
                        let range = parameter.range?;
                        let start = position_to_offset(source, range.start)?;
                        let end = position_to_offset(source, range.end)?;
                        let descriptor =
                            JavaTypeDescriptorResolver::descriptor_or_null(&generic, &context)?;
                        Some((parameter.name, start, end, generic, descriptor))
                    })
                    .collect(),
            )
        } else {
            None
        }
    }
}

fn unknown(message: &str) -> FactsError {
    FactsError::Unknown(message.into())
}
fn string(annotation: &HostAnnotationFact, name: &str) -> Result<Option<String>, FactsError> {
    annotation
        .member(name)
        .map(|value| {
            value
                .constant_string()
                .ok_or_else(|| unknown("handler annotation string value is unavailable"))
        })
        .transpose()
}
fn integer(annotation: &HostAnnotationFact, name: &str) -> Result<Option<i32>, FactsError> {
    annotation
        .member(name)
        .map(|value| {
            value
                .constant_int()
                .ok_or_else(|| unknown("handler annotation integer value is unavailable"))
        })
        .transpose()
}
fn strings(annotation: &HostAnnotationFact, name: &str) -> Result<Vec<String>, FactsError> {
    annotation
        .member(name)
        .map(|value| {
            value
                .string_array()
                .ok_or_else(|| unknown("handler annotation string array is unavailable"))
        })
        .transpose()
        .map(Option::unwrap_or_default)
}
