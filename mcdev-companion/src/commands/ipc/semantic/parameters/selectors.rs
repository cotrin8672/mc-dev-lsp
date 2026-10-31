use super::*;
use mcdev_core::mixinextras_occurrence::ConstantAtArgsParser;

fn range(
    annotation: &HostAnnotationFact,
    source: &str,
) -> Result<mcdev_core::McTextRange, FactsError> {
    let range = annotation
        .source_range
        .as_ref()
        .ok_or_else(|| unknown("selector source range is unavailable"))?;
    text_range(
        source,
        &ByteRange {
            start: range.start,
            end: range.end,
        },
    )
}
fn expression_range(
    value: Option<&HostExpressionFact>,
    source: &str,
) -> Result<Option<mcdev_core::McTextRange>, FactsError> {
    value
        .map(|value| {
            let range = value
                .source_range
                .as_ref()
                .ok_or_else(|| unknown("selector value range is unavailable"))?;
            text_range(
                source,
                &ByteRange {
                    start: range.start,
                    end: range.end,
                },
            )
        })
        .transpose()
}
fn nested(
    annotation: &HostAnnotationFact,
    member: &str,
) -> Result<Vec<HostAnnotationFact>, FactsError> {
    annotation
        .member(member)
        .map(|value| {
            value
                .nested_annotations()
                .ok_or_else(|| unknown("nested selector annotations are unavailable"))
        })
        .transpose()
        .map(Option::unwrap_or_default)
}
fn ordinal(annotation: &HostAnnotationFact) -> Result<Option<i32>, FactsError> {
    match integer(annotation, "ordinal")? {
        Some(-1) | None => Ok(None),
        Some(value) if value >= 0 => Ok(Some(value)),
        _ => Err(unknown("selector ordinal is invalid")),
    }
}
fn boolean(annotation: &HostAnnotationFact, name: &str) -> Result<Option<bool>, FactsError> {
    annotation
        .member(name)
        .map(|value| {
            value
                .constant_boolean()
                .ok_or_else(|| unknown("selector boolean value is unavailable"))
        })
        .transpose()
}
fn at(annotation: &HostAnnotationFact, source: &str) -> Result<HostSliceAt, FactsError> {
    if annotation.qualified_name.as_deref() != Some("org.spongepowered.asm.mixin.injection.At") {
        return Err(unknown("At selector identity is unavailable"));
    }
    if annotation.member("opcode").is_some() {
        return Err(unknown("opcode-restricted At selection is unavailable"));
    }
    let mut shift = AtShiftSpec::Before;
    if let Some(value) = annotation.member("shift") {
        let HostExpressionKind::Enum { owner, name } = &value.kind else {
            return Err(unknown("At shift enum is unavailable"));
        };
        if owner.descriptor().as_deref() != Some("Lorg/spongepowered/asm/mixin/injection/At$Shift;")
        {
            return Err(unknown("At shift enum identity is unavailable"));
        }
        shift = match name.as_str() {
            "NONE" | "BEFORE" => AtShiftSpec::Before,
            "AFTER" => AtShiftSpec::After,
            "BY" => AtShiftSpec::By {
                offset: integer(annotation, "by")?.unwrap_or(0),
            },
            _ => return Err(unknown("At shift enum entry is unavailable")),
        };
    }
    Ok(HostSliceAt {
        value: string(annotation, "value")?.ok_or_else(|| unknown("At value is unavailable"))?,
        target: string(annotation, "target")?,
        args: strings(annotation, "args")?,
        id: string(annotation, "id")?,
        ordinal: ordinal(annotation)?,
        shift,
        range: range(annotation, source)?,
        target_range: expression_range(annotation.member("target"), source)?,
        ordinal_range: expression_range(annotation.member("ordinal"), source)?,
    })
}
fn slices(
    annotation: &HostAnnotationFact,
    source: &str,
) -> Result<Vec<HostMethodSlice>, FactsError> {
    let mut ids = BTreeSet::new();
    nested(annotation, "slice")?
        .into_iter()
        .map(|slice| {
            if slice.qualified_name.as_deref()
                != Some("org.spongepowered.asm.mixin.injection.Slice")
            {
                return Err(unknown("Slice identity is unavailable"));
            }
            let id = string(&slice, "id")?.unwrap_or_default();
            if !ids.insert(id.clone()) {
                return Err(unknown("duplicate Slice identifiers"));
            }
            let boundary = |name| {
                let values = nested(&slice, name)?;
                match values.as_slice() {
                    [] => Ok(None),
                    [value] => {
                        if string(value, "slice")?.is_some_and(|id| !id.is_empty()) {
                            return Err(unknown("nested slice boundaries are unavailable"));
                        }
                        at(value, source).map(Some)
                    }
                    _ => Err(unknown("Slice requires a single boundary point")),
                }
            };
            Ok(HostMethodSlice {
                id,
                from: boundary("from")?,
                to: boundary("to")?,
                range: range(&slice, source)?,
            })
        })
        .collect()
}
fn conditions(value: &HostExpressionFact) -> Result<BTreeSet<i32>, FactsError> {
    let values = match &value.kind {
        HostExpressionKind::Array(values) => values
            .iter()
            .map(|value| {
                value
                    .as_ref()
                    .ok_or_else(|| unknown("constant zero condition is unavailable"))
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => vec![value],
    };
    let mut result = BTreeSet::new();
    for value in values {
        let HostExpressionKind::Enum { owner, name } = &value.kind else {
            return Err(unknown("constant condition enum is unavailable"));
        };
        if owner.descriptor().as_deref()
            != Some("Lorg/spongepowered/asm/mixin/injection/Constant$Condition;")
        {
            return Err(unknown("constant condition enum identity is unavailable"));
        }
        result.extend(match name.as_str() {
            "LESS_THAN_ZERO" => [155, 156],
            "LESS_THAN_OR_EQUAL_TO_ZERO" => [158, 157],
            "GREATER_THAN_ZERO" => [157, 158],
            "GREATER_THAN_OR_EQUAL_TO_ZERO" => [156, 155],
            _ => return Err(unknown("constant condition enum entry is unavailable")),
        });
    }
    Ok(result)
}
#[derive(Default)]
struct Selector {
    point: Option<HostSliceAt>,
    args: Vec<String>,
    ordinal: Option<i32>,
    zero: BTreeSet<i32>,
    slice: String,
}
fn constant(annotation: &HostAnnotationFact) -> Result<Selector, FactsError> {
    if annotation.qualified_name.as_deref()
        != Some("org.spongepowered.asm.mixin.injection.Constant")
    {
        return Err(unknown("Constant selector identity is unavailable"));
    }
    let mut result = Selector {
        ordinal: ordinal(annotation)?,
        slice: string(annotation, "slice")?.unwrap_or_default(),
        ..Default::default()
    };
    for (name, value) in &annotation.members {
        let normalized = match name.as_str() {
            "ordinal" | "slice" => continue,
            "log" => {
                boolean(annotation, "log")?;
                continue;
            }
            "intValue" => value.constant_int().map(|value| value.to_string()),
            "longValue" => match value.kind {
                HostExpressionKind::Constant(HostConstant::Long(value)) => Some(value.to_string()),
                HostExpressionKind::Constant(HostConstant::Int(value)) => {
                    Some(i64::from(value).to_string())
                }
                _ => None,
            },
            "stringValue" => value.constant_string(),
            "nullValue" => value
                .constant_boolean()
                .filter(|value| *value)
                .map(|_| "true".into()),
            "classValue" => match &value.kind {
                HostExpressionKind::TypeLiteral(ty) => ty.jvm_type().and_then(|ty| match ty {
                    JvmType::ObjectType(internal) => Some(internal),
                    JvmType::VoidType => None,
                    other => Some(descriptor::to_descriptor(&other)),
                }),
                _ => None,
            },
            "floatValue" => floating(value, true),
            "doubleValue" => floating(value, false),
            "expandZeroConditions" => {
                result.zero = conditions(value)?;
                continue;
            }
            _ => return Err(unknown("Constant selector member is unavailable")),
        }
        .ok_or_else(|| unknown("typed Constant selector value is unavailable"))?;
        result.args.push(format!("{name}={normalized}"));
    }
    if !result.args.is_empty() && ConstantAtArgsParser::parse(&result.args).is_none() {
        return Err(unknown("Constant selectors have incompatible types"));
    }
    Ok(result)
}
fn floating(value: &HostExpressionFact, single: bool) -> Option<String> {
    use mcdev_core::bytecode::ConstantValue;
    let constant = if single {
        ConstantValue::FloatValue(match value.kind {
            HostExpressionKind::Constant(HostConstant::Int(value)) => value as f32,
            HostExpressionKind::Constant(HostConstant::Long(value)) => value as f32,
            HostExpressionKind::Constant(HostConstant::FloatBits(bits)) => f32::from_bits(bits),
            _ => return None,
        })
    } else {
        ConstantValue::DoubleValue(match value.kind {
            HostExpressionKind::Constant(HostConstant::Int(value)) => f64::from(value),
            HostExpressionKind::Constant(HostConstant::Long(value)) => value as f64,
            HostExpressionKind::Constant(HostConstant::FloatBits(bits)) => {
                f64::from(f32::from_bits(bits))
            }
            HostExpressionKind::Constant(HostConstant::DoubleBits(bits)) => f64::from_bits(bits),
            _ => return None,
        })
    };
    let label = mcdev_core::bytecode::constant_label(Some(&constant));
    Some(if single {
        label.strip_suffix('f').unwrap_or(&label).into()
    } else {
        label
    })
}
pub(super) fn project_sites(
    kind: MixinExtrasAnnotation,
    annotation: &HostAnnotationFact,
    handler: &HandlerMethodDeclaration,
    source: &str,
) -> Result<Vec<(MixinExtrasAnnotationSite, Option<HostMethodSlice>)>, FactsError> {
    let range = range(annotation, source)?;
    let methods = strings(annotation, "method")?;
    if methods.is_empty() {
        return Err(unknown("handler target method selector is unavailable"));
    }
    let slices = slices(annotation, source)?;
    let constants = nested(annotation, "constant")?;
    let has_constant = kind == MixinExtrasAnnotation::WrapOperation && !constants.is_empty();
    let mut infos = if kind == MixinExtrasAnnotation::ModifyConstant || has_constant {
        constants
            .iter()
            .map(constant)
            .collect::<Result<Vec<_>, _>>()?
    } else {
        nested(annotation, "at")?
            .iter()
            .map(|value| {
                Ok(Selector {
                    point: Some(at(value, source)?),
                    slice: string(value, "slice")?.unwrap_or_default(),
                    ..Default::default()
                })
            })
            .collect::<Result<Vec<_>, FactsError>>()?
    };
    if has_constant && !nested(annotation, "at")?.is_empty() {
        return Err(unknown("WrapOperation has both At and Constant selectors"));
    }
    if infos.is_empty() {
        infos.push(Selector::default())
    }
    let mut result = Vec::new();
    for method in methods {
        for info in &infos {
            let slice = slices.iter().find(|slice| slice.id == info.slice).cloned();
            if !info.slice.is_empty() && slice.is_none() {
                return Err(unknown("selected Slice is unavailable"));
            }
            let at = info.point.as_ref();
            let is_constant = kind == MixinExtrasAnnotation::ModifyConstant;
            result.push((
                MixinExtrasAnnotationSite {
                    annotation: kind,
                    method_attribute: method.clone(),
                    at_value: if is_constant {
                        Some("CONSTANT".into())
                    } else {
                        at.map(|at| at.value.clone())
                    },
                    at_target: at.and_then(|at| at.target.clone()),
                    at_args: if is_constant {
                        info.args.clone()
                    } else {
                        at.map(|at| at.args.clone()).unwrap_or_default()
                    },
                    at_id: at.and_then(|at| at.id.clone()),
                    at_ordinal: if is_constant {
                        info.ordinal
                    } else {
                        at.and_then(|at| at.ordinal)
                    },
                    at_shift: at.map(|at| at.shift).unwrap_or_default(),
                    has_constant_selector: has_constant,
                    annotation_range: range,
                    handler_method: Some(handler.clone()),
                    expand_zero_conditions: info.zero.clone(),
                },
                slice,
            ));
        }
    }
    Ok(result)
}
