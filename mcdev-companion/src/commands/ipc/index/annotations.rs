use super::*;
use crate::host_facts::draft::{BinaryAnnotation, BinaryAnnotationElement, BinaryAnnotationValue};
use mcdev_core::host_source::{
    injection_point_at_codes, HostAnnotationFact, HostConstant, HostExpressionFact,
    HostExpressionKind, HostInjectionPointSubtype, HostNodeValidity, HostPrimitive, HostTypeFact,
};

impl<R: EvaluationFactReader> EvaluationIndex<R> {
    /// Compile-selected descendants only; this does not certify current authored source hierarchy.
    pub fn physical_injection_point_values(&self) -> Result<Vec<String>, FactsError> {
        let mut by_name = BTreeMap::<String, Vec<CatalogEntry>>::new();
        for entry in self.physical_catalog("")? {
            by_name
                .entry(entry.descriptor.internal_name.clone())
                .or_default()
                .push(entry);
        }
        let mut entries = Vec::new();
        for (name, candidates) in by_name {
            if let Some(entry) = self.select_candidate(&name, candidates)? {
                entries.push(entry);
            }
        }
        let mut classes = BTreeMap::new();
        for batch in entries.chunks(crate::wire::MAX_BATCH) {
            let facts = self.with_inputs(|inputs| inputs.jvm_classes(batch))?;
            if facts.len() != batch.len() {
                return Err(protocol("injection point ABI cardinality mismatch"));
            }
            for (entry, fact) in batch.iter().zip(facts) {
                let Fact::Found { value } = fact else {
                    return Err(unknown("injection point ABI inventory is incomplete"));
                };
                self.cache()?
                    .abis
                    .insert(entry.descriptor.entry.clone(), value.abi.clone());
                classes.insert(entry.descriptor.internal_name.clone(), value.abi);
            }
        }
        let mut subtypes = Vec::new();
        for (name, abi) in &classes {
            if physical_descendant(
                name,
                "org/spongepowered/asm/mixin/injection/InjectionPoint",
                &classes,
            )? {
                let annotations = abi
                    .annotations
                    .as_ref()
                    .ok_or_else(|| unknown("binary declaration annotations were not negotiated"))?;
                subtypes.push(HostInjectionPointSubtype {
                    qualified_name: name.replace('/', "."),
                    annotations: annotations
                        .iter()
                        .map(project_binary_annotation)
                        .collect::<Result<_, _>>()?,
                });
            }
        }
        self.facts.check_active()?;
        Ok(injection_point_at_codes(&subtypes))
    }
}

fn physical_descendant(
    name: &str,
    target: &str,
    classes: &BTreeMap<String, RawAbi>,
) -> Result<bool, FactsError> {
    if name == target {
        return Ok(false);
    }
    let mut pending = VecDeque::from([name.to_owned()]);
    let mut seen = BTreeSet::new();
    while let Some(name) = pending.pop_front() {
        if name == target {
            return Ok(true);
        }
        if name == "java/lang/Object" || !seen.insert(name.clone()) {
            continue;
        }
        let abi = classes
            .get(&name)
            .ok_or_else(|| unknown(format!("physical hierarchy for {name} is incomplete")))?;
        pending.extend(abi.super_name.iter().chain(&abi.interfaces).cloned());
    }
    Ok(false)
}

fn annotation_parts(
    descriptor: &str,
    elements: &[BinaryAnnotationElement],
) -> Result<HostAnnotationFact, FactsError> {
    let JvmType::ObjectType(owner) = descriptor::parse_field_descriptor(descriptor)
        .map_err(|error| protocol(error.to_string()))?
    else {
        return Err(protocol("invalid annotation owner"));
    };
    Ok(HostAnnotationFact {
        source_name: owner.rsplit('/').next().unwrap_or("").into(),
        qualified_name: Some(owner.replace('/', ".")),
        validity: HostNodeValidity::default(),
        exists: true,
        members: elements
            .iter()
            .map(|element| Ok((element.name.clone(), binary_value(&element.value)?)))
            .collect::<Result<_, FactsError>>()?,
        members_available: true,
        source_range: None,
        body_range: None,
    })
}
fn project_binary_annotation(
    annotation: &BinaryAnnotation,
) -> Result<HostAnnotationFact, FactsError> {
    annotation_parts(&annotation.descriptor, &annotation.elements)
}
fn binary_type(descriptor: &str) -> Result<HostTypeFact, FactsError> {
    if descriptor == "V" {
        return Ok(HostTypeFact::Primitive(HostPrimitive::Void));
    }
    Ok(super::super::semantic::host_type(
        descriptor::parse_field_descriptor(descriptor)
            .map_err(|error| protocol(error.to_string()))?,
    ))
}
fn binary_value(value: &BinaryAnnotationValue) -> Result<HostExpressionFact, FactsError> {
    let constant = |value| HostExpressionKind::Constant(value);
    let kind = match value {
        BinaryAnnotationValue::Byte { value } => constant(HostConstant::Int(i32::from(*value))),
        BinaryAnnotationValue::Short { value } => constant(HostConstant::Int(i32::from(*value))),
        BinaryAnnotationValue::Char { value } => constant(HostConstant::Char(*value)),
        BinaryAnnotationValue::Int { value } => constant(HostConstant::Int(*value)),
        BinaryAnnotationValue::Long { value } => constant(HostConstant::Long(
            value
                .parse()
                .map_err(|_| protocol("invalid long annotation value"))?,
        )),
        BinaryAnnotationValue::Float { bits } => constant(HostConstant::FloatBits(
            u32::from_str_radix(bits, 16).map_err(|_| protocol("invalid float annotation bits"))?,
        )),
        BinaryAnnotationValue::Double { bits } => constant(HostConstant::DoubleBits(
            u64::from_str_radix(bits, 16)
                .map_err(|_| protocol("invalid double annotation bits"))?,
        )),
        BinaryAnnotationValue::Boolean { value } => constant(HostConstant::Boolean(*value)),
        BinaryAnnotationValue::String { value, utf16 } => {
            constant(HostConstant::String(match (value, utf16) {
                (Some(value), None) => value.encode_utf16().collect(),
                (None, Some(value)) => value.clone(),
                _ => return Err(protocol("invalid binary annotation string representation")),
            }))
        }
        BinaryAnnotationValue::Class { descriptor } => {
            HostExpressionKind::TypeLiteral(binary_type(descriptor)?)
        }
        BinaryAnnotationValue::Enum { descriptor, name } => HostExpressionKind::Enum {
            owner: binary_type(descriptor)?,
            name: name.clone(),
        },
        BinaryAnnotationValue::Annotation { value } => HostExpressionKind::Annotation(Box::new(
            annotation_parts(&value.descriptor, &value.elements)?,
        )),
        BinaryAnnotationValue::Array { values } => HostExpressionKind::Array(
            values
                .iter()
                .map(|value| binary_value(value).map(Some))
                .collect::<Result<_, _>>()?,
        ),
    };
    Ok(HostExpressionFact {
        validity: HostNodeValidity::default(),
        kind,
        source_text: String::new(),
        source_range: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn typed_binary_at_code_uses_existing_namespace_and_utf16_projection() {
        let annotation: BinaryAnnotation = serde_json::from_value(json!({"runtimeVisible":false,
        "descriptor":"Lorg/spongepowered/asm/mixin/injection/InjectionPoint$AtCode;","elements":[
            {"name":"value","value":{"kind":"string","value":"雪"}},
            {"name":"namespace","value":{"kind":"string","utf16":[77,67]}}
        ]}))
        .unwrap();
        let annotation = project_binary_annotation(&annotation).unwrap();
        assert_eq!(
            mcdev_core::host_source::declared_at_code(&annotation).as_deref(),
            Some("MC:雪")
        );
        assert!(annotation.source_range.is_none());
    }

    #[test]
    fn binary_values_keep_exact_integer_float_enum_and_unpaired_utf16_data() {
        let values = [
            (
                BinaryAnnotationValue::Long {
                    value: i64::MIN.to_string(),
                },
                HostExpressionKind::Constant(HostConstant::Long(i64::MIN)),
            ),
            (
                BinaryAnnotationValue::Float {
                    bits: "80000000".into(),
                },
                HostExpressionKind::Constant(HostConstant::FloatBits(0x80000000)),
            ),
            (
                BinaryAnnotationValue::String {
                    value: None,
                    utf16: Some(vec![0xd800]),
                },
                HostExpressionKind::Constant(HostConstant::String(vec![0xd800])),
            ),
            (
                BinaryAnnotationValue::Enum {
                    descriptor: "Ldemo/Choice;".into(),
                    name: "ONE".into(),
                },
                HostExpressionKind::Enum {
                    owner: HostTypeFact::Class {
                        internal_name: "demo/Choice".into(),
                    },
                    name: "ONE".into(),
                },
            ),
        ];
        for (value, expected) in values {
            assert_eq!(binary_value(&value).unwrap().kind, expected);
        }
    }
}
