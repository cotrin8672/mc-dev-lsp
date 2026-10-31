use super::*;
use mcdev_core::{
    extras_model::*,
    host_source::{project_host_handler_parameter, HostFact},
};
mod selectors;
use selectors::project_sites;

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

impl HostSemanticProvider {
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
                    collect_class_refs(argument, &mut references)?;
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
                HostFact::Known(parameter) => projected.push(parameter),
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
            sites.extend(project_sites(kind, &normalized, &handler, source)?);
        }
        Ok(sites)
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
