//! Exact injection-point occurrence selection. `None` means unknown; `Some(empty)` proves no match.
use crate::bytecode::{AtTargetCandidate, AtTargetKind, BytecodeIndex, ConstantValue};
use crate::descriptor::{self, MemberTarget, Pattern};
use crate::extras_model::{AtShiftSpec, HostMethodSlice, HostSliceAt, MixinExtrasAnnotationSite};
use crate::mixin_completion::AtTargetCompletionService;
use crate::MethodIndexEntry;
use std::collections::{BTreeMap, BTreeSet};

pub struct ConstantAtArgsParser;
impl ConstantAtArgsParser {
    pub fn parse(args: &[String]) -> Option<String> {
        let map = args_map(args);
        let mut selected = None;
        for (key, value) in map {
            let next = match key.as_str() {
                "intValue" if value.parse::<i32>().is_ok() => "I",
                "longValue" if value.parse::<i64>().is_ok() => "J",
                "floatValue" if parse_java_float(&value).is_some() => "F",
                "doubleValue" if parse_java_float(&value).is_some() => "D",
                "stringValue" => "Ljava/lang/String;",
                "classValue" if !value.trim().is_empty() => "Ljava/lang/Class;",
                "nullValue" if value.eq_ignore_ascii_case("true") => "Ljava/lang/Object;",
                _ => continue,
            };
            if selected.is_some_and(|old| old != next) {
                return None;
            }
            selected = Some(next);
        }
        selected.map(str::to_owned)
    }
}
fn args_map(args: &[String]) -> BTreeMap<String, String> {
    args.iter()
        .map(|s| {
            let (k, v) = s.split_once('=').unwrap_or((s, ""));
            (k.into(), v.into())
        })
        .collect()
}
/// Java's raw Float/Double parser accepts whitespace, type suffixes and hexadecimal floating literals.
pub fn parse_java_float(value: &str) -> Option<f64> {
    let value = value.trim_matches(|c: char| c <= ' ');
    if matches!(value, "NaN" | "+NaN" | "-NaN") {
        return Some(f64::NAN);
    }
    if matches!(value, "Infinity" | "+Infinity") {
        return Some(f64::INFINITY);
    }
    if value == "-Infinity" {
        return Some(f64::NEG_INFINITY);
    }
    let value = value.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(value);
    let (sign, rest) = if let Some(v) = value.strip_prefix('-') {
        (-1.0, v)
    } else {
        (1.0, value.strip_prefix('+').unwrap_or(value))
    };
    if let Some(hex) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
        return Some(f64::from_bits(
            hex_float_bits(hex, 53, 1023)? | if sign < 0.0 { 1 << 63 } else { 0 },
        ));
    }
    if rest.is_empty()
        || !rest
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-'))
    {
        return None;
    }
    value.parse().ok()
}
/// Float parsing rounds directly to binary32, avoiding double rounding through f64.
pub fn parse_java_f32(value: &str) -> Option<f32> {
    let value = value.trim_matches(|c: char| c <= ' ');
    if matches!(
        value,
        "NaN" | "+NaN" | "-NaN" | "Infinity" | "+Infinity" | "-Infinity"
    ) {
        return parse_java_float(value).map(|v| v as f32);
    }
    let value = value.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(value);
    let (negative, rest) = if let Some(rest) = value.strip_prefix('-') {
        (true, rest)
    } else {
        (false, value.strip_prefix('+').unwrap_or(value))
    };
    if let Some(hex) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
        return Some(f32::from_bits(
            hex_float_bits(hex, 24, 127)? as u32 | if negative { 1 << 31 } else { 0 },
        ));
    }
    if rest.is_empty()
        || !rest
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-'))
    {
        return None;
    }
    value.parse().ok()
}
// Keep the leading significand bits plus a round/sticky decision. This also
// handles arbitrarily long significands and exponents without overflow first.
fn hex_float_bits(hex: &str, precision: i64, bias: i64) -> Option<u64> {
    let at = hex.find(['p', 'P'])?;
    let mantissa = &hex[..at];
    let exponent = &hex[at + 1..];
    let digits = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let power = exponent
        .parse::<i64>()
        .unwrap_or(if exponent.starts_with('-') {
            i64::MIN / 2
        } else {
            i64::MAX / 2
        });
    let mut values = vec![];
    let mut point = None;
    for ch in mantissa.chars() {
        if ch == '.' {
            if point.is_some() {
                return None;
            }
            point = Some(values.len());
        } else {
            values.push(ch.to_digit(16)? as u8);
        }
    }
    if values.is_empty() {
        return None;
    }
    let fraction = values.len() - point.unwrap_or(values.len());
    let Some(nonzero) = values.iter().position(|v| *v != 0) else {
        return Some(0);
    };
    let values = &values[nonzero..];
    let skip = values[0].leading_zeros() as usize - 4;
    let length = values.len() * 4 - skip;
    let exponent = power
        .saturating_sub((fraction as i64).saturating_mul(4))
        .saturating_add(length as i64 - 1);
    let maximum = bias;
    let minimum = 1 - bias;
    let infinity = ((bias * 2 + 1) as u64) << (precision - 1);
    if exponent > maximum {
        return Some(infinity);
    }
    if exponent < minimum - precision {
        return Some(0);
    }
    let keep = if exponent >= minimum {
        precision
    } else {
        exponent - minimum + precision
    };
    let take = (keep.max(0) as usize).min(length);
    let bit = |i: usize| {
        let absolute = i + skip;
        (values[absolute / 4] >> (3 - absolute % 4)) & 1
    };
    let mut significand = 0u64;
    for i in 0..take {
        significand = significand * 2 + u64::from(bit(i));
    }
    if keep > length as i64 {
        significand <<= keep - length as i64;
    }
    if take < length
        && bit(take) == 1
        && ((take + 1..length).any(|i| bit(i) == 1) || significand & 1 == 1)
    {
        significand += 1;
    }
    if exponent < minimum {
        return Some(significand);
    }
    let mut exponent = exponent;
    if significand == 1u64 << precision {
        significand >>= 1;
        exponent += 1;
    }
    if exponent > maximum {
        return Some(infinity);
    }
    Some(
        ((exponent + bias) as u64) << (precision - 1)
            | (significand & ((1u64 << (precision - 1)) - 1)),
    )
}
pub fn constant_values_equal(left: &ConstantValue, right: &ConstantValue) -> bool {
    match (left, right) {
        (ConstantValue::FloatValue(a), ConstantValue::FloatValue(b)) => {
            (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits()
        }
        (ConstantValue::DoubleValue(a), ConstantValue::DoubleValue(b)) => {
            (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits()
        }
        _ => left == right,
    }
}
fn expected_constant(args: &[String]) -> Option<ConstantValue> {
    ConstantAtArgsParser::parse(args)?;
    let map = args_map(args);
    if let Some(v) = map.get("intValue").and_then(|v| v.parse().ok()) {
        return Some(ConstantValue::IntValue(v));
    }
    if let Some(v) = map.get("floatValue").and_then(|v| parse_java_f32(v)) {
        return Some(ConstantValue::FloatValue(v));
    }
    if let Some(v) = map.get("longValue").and_then(|v| v.parse().ok()) {
        return Some(ConstantValue::LongValue(v));
    }
    if let Some(v) = map.get("doubleValue").and_then(|v| parse_java_float(v)) {
        return Some(ConstantValue::DoubleValue(v));
    }
    if let Some(v) = map.get("stringValue") {
        return Some(ConstantValue::StringValue(v.clone()));
    }
    if let Some(v) = map.get("classValue").filter(|v| !v.trim().is_empty()) {
        return Some(ConstantValue::ClassLiteral(v.replace('.', "/")));
    }
    if map
        .get("nullValue")
        .is_some_and(|v| v.eq_ignore_ascii_case("true"))
    {
        return Some(ConstantValue::NullValue);
    }
    None
}
pub struct InjectionPointOccurrenceResolver<'a> {
    pub bytecode_index: &'a dyn BytecodeIndex,
}
impl<'a> InjectionPointOccurrenceResolver<'a> {
    pub fn new(bytecode_index: &'a dyn BytecodeIndex) -> Self {
        Self { bytecode_index }
    }
    pub fn resolve(
        &self,
        owner: &str,
        target_method: &MethodIndexEntry,
        site: &MixinExtrasAnnotationSite,
        expression_instruction_indices: Option<&BTreeSet<i32>>,
    ) -> Option<BTreeSet<i32>> {
        self.resolve_bounds(
            owner,
            target_method,
            site,
            expression_instruction_indices,
            None,
        )
    }
    pub fn resolve_in_slice(
        &self,
        owner: &str,
        target_method: &MethodIndexEntry,
        site: &MixinExtrasAnnotationSite,
        expression_instruction_indices: Option<&BTreeSet<i32>>,
        slice: Option<&HostMethodSlice>,
    ) -> Option<BTreeSet<i32>> {
        let Some(slice) = slice else {
            return self.resolve(owner, target_method, site, expression_instruction_indices);
        };
        let boundary = |point: &HostSliceAt, fail: i32| {
            let mut query = site.clone();
            query.at_value = Some(point.value.clone());
            query.at_target = point.target.clone();
            query.at_args = point.args.clone();
            query.at_id = point.id.clone();
            query.at_ordinal = point.ordinal;
            query.at_shift = point.shift;
            query.expand_zero_conditions.clear();
            let (_, specifier) = parse_at_value(&point.value)?;
            if specifier == Some("ALL") {
                return None;
            }
            let indices = self.resolve(owner, target_method, &query, None)?;
            Some(
                if specifier == Some("LAST") {
                    indices.last().copied()
                } else {
                    indices.first().copied()
                }
                .unwrap_or(fail),
            )
        };
        let start = slice
            .from
            .as_ref()
            .map(|point| boundary(point, 0))
            .unwrap_or(Some(0))?;
        let end = slice
            .to
            .as_ref()
            .map(|point| boundary(point, start))
            .unwrap_or(Some(i32::MAX))?;
        if start > end {
            return None;
        }
        self.resolve_bounds(
            owner,
            target_method,
            site,
            expression_instruction_indices,
            Some((start, end)),
        )
    }
    fn resolve_bounds(
        &self,
        owner: &str,
        target_method: &MethodIndexEntry,
        site: &MixinExtrasAnnotationSite,
        expression_instruction_indices: Option<&BTreeSet<i32>>,
        bounds: Option<(i32, i32)>,
    ) -> Option<BTreeSet<i32>> {
        let within = |index: i32| bounds.is_none_or(|(start, end)| start <= index && index <= end);
        let (value, specifier) = parse_at_value(site.at_value.as_deref()?)?;
        let value = value.to_ascii_uppercase();
        let selected = match value.as_str() {
            "MIXINEXTRAS:EXPRESSION" => select_indices(
                &expression_instruction_indices?
                    .iter()
                    .copied()
                    .filter(|index| within(*index))
                    .collect(),
                specifier,
            )?,
            "HEAD" => select_indices(
                &BTreeSet::from([bounds.map_or(0, |(start, _)| start)]),
                specifier,
            )?,
            "TAIL" => {
                let max = self
                    .candidates(owner, target_method, "RETURN")
                    .into_iter()
                    .map(|c| c.instruction_occurrence_index)
                    .filter(|i| *i >= 0)
                    .filter(|index| within(*index))
                    .max()?;
                select_indices(&BTreeSet::from([max]), specifier)?
            }
            "INVOKE" | "FIELD" | "NEW" | "RETURN" | "CONSTANT" => {
                let candidates = self
                    .candidates(owner, target_method, &value)
                    .into_iter()
                    .filter(|candidate| within(candidate.instruction_occurrence_index))
                    .collect();
                let filtered = if value == "CONSTANT" {
                    filter_constants(candidates, site)?
                } else {
                    filter_target(candidates, site.at_target.as_deref(), &value)?
                };
                select_occurrences(filtered, site.at_ordinal, specifier)?
            }
            _ => return None,
        };
        apply_shift(selected, site.at_shift)
            .map(|indices| indices.into_iter().filter(|index| within(*index)).collect())
    }
    fn candidates(
        &self,
        owner: &str,
        method: &MethodIndexEntry,
        value: &str,
    ) -> Vec<AtTargetCandidate> {
        self.bytecode_index.get_at_target_candidates(
            owner,
            &method.name,
            Some(&method.descriptor),
            value,
        )
    }
}
fn parse_at_value(value: &str) -> Option<(&str, Option<&str>)> {
    if value.eq_ignore_ascii_case("MIXINEXTRAS:EXPRESSION") {
        return Some((value, None));
    }
    let Some((base, suffix)) = value.rsplit_once(':') else {
        return Some((value, None));
    };
    if base.is_empty() || !matches!(suffix, "FIRST" | "LAST" | "ONE" | "ALL" | "DEFAULT") {
        None
    } else {
        Some((base, Some(suffix)))
    }
}
fn select_indices(indices: &BTreeSet<i32>, specifier: Option<&str>) -> Option<BTreeSet<i32>> {
    match specifier {
        None | Some("ALL" | "DEFAULT") => Some(indices.clone()),
        Some("FIRST") => Some(indices.first().copied().into_iter().collect()),
        Some("LAST") => Some(indices.last().copied().into_iter().collect()),
        Some("ONE") if indices.len() == 1 => Some(indices.clone()),
        _ => None,
    }
}
fn select_occurrences(
    mut candidates: Vec<AtTargetCandidate>,
    ordinal: Option<i32>,
    specifier: Option<&str>,
) -> Option<BTreeSet<i32>> {
    if let Some(ordinal) = ordinal {
        if candidates
            .iter()
            .any(|c| c.instruction_occurrence_index < 0)
        {
            return None;
        }
        candidates.sort_by_key(|c| c.instruction_occurrence_index);
        candidates = usize::try_from(ordinal)
            .ok()
            .and_then(|n| candidates.get(n).cloned())
            .into_iter()
            .collect();
    }
    if specifier == Some("ONE") && candidates.len() != 1 {
        return None;
    }
    let indices = candidates
        .into_iter()
        .map(|c| c.instruction_occurrence_index)
        .collect();
    select_indices(&indices, specifier)
}
fn apply_shift(indices: BTreeSet<i32>, shift: AtShiftSpec) -> Option<BTreeSet<i32>> {
    let by = match shift {
        AtShiftSpec::Before => 0,
        AtShiftSpec::After => 1,
        AtShiftSpec::By { offset } => offset,
        AtShiftSpec::Unresolved => return None,
    };
    indices
        .into_iter()
        .map(|index| index.checked_add(by).filter(|n| *n >= 0))
        .collect()
}
fn broad(target: &str) -> bool {
    if target == "*" {
        return true;
    }
    if let Ok(s) = descriptor::parse_method_selector(target) {
        if matches!(s.owner, Pattern::Any)
            || matches!(s.name, Pattern::Any)
            || matches!(s.descriptor, Pattern::Any)
        {
            return true;
        }
    }
    if let Ok(s) = descriptor::parse_field_selector(target) {
        if matches!(s.owner, Pattern::Any)
            || matches!(s.name, Pattern::Any)
            || matches!(s.descriptor, Pattern::Any)
        {
            return true;
        }
    }
    false
}
fn filter_target(
    candidates: Vec<AtTargetCandidate>,
    target: Option<&str>,
    value: &str,
) -> Option<Vec<AtTargetCandidate>> {
    let Some(target) = target.map(str::trim).filter(|s| !s.is_empty()) else {
        return Some(candidates);
    };
    if value == "RETURN" {
        return Some(if target == "RETURN" {
            candidates
                .into_iter()
                .filter(|c| c.kind == AtTargetKind::Return)
                .collect()
        } else {
            vec![]
        });
    }
    if broad(target) {
        return None;
    }
    let kind = match value {
        "INVOKE" => Some(AtTargetKind::Invoke),
        "FIELD" => Some(AtTargetKind::Field),
        "NEW" => Some(AtTargetKind::New),
        _ => None,
    };
    let by_format = candidates
        .iter()
        .filter(|c| {
            kind.is_none_or(|kind| c.kind == kind)
                && AtTargetCompletionService::format_target(c) == target
        })
        .cloned()
        .collect::<Vec<_>>();
    if !by_format.is_empty() {
        return Some(by_format);
    }
    match value {
        "INVOKE" | "FIELD" => Some(match descriptor::parse_member_target(target) {
            Ok(MemberTarget::Method {
                owner,
                name,
                descriptor,
            }) => candidates
                .into_iter()
                .filter(|c| {
                    c.kind == AtTargetKind::Invoke
                        && c.owner == owner
                        && c.name == name
                        && c.descriptor == descriptor::method_to_descriptor(&descriptor)
                })
                .collect(),
            Ok(MemberTarget::Field {
                owner,
                name,
                descriptor,
            }) => candidates
                .into_iter()
                .filter(|c| {
                    c.kind == AtTargetKind::Field
                        && c.owner == owner
                        && c.name == name
                        && c.descriptor == descriptor::to_descriptor(&descriptor)
                })
                .collect(),
            Err(_) => vec![],
        }),
        "NEW" => {
            let Ok(s) = descriptor::parse_method_selector(target) else {
                return Some(vec![]);
            };
            let (Pattern::Exact(owner), Pattern::Exact(name), Pattern::Exact(desc)) =
                (s.owner, s.name, s.descriptor)
            else {
                return None;
            };
            Some(
                candidates
                    .into_iter()
                    .filter(|c| {
                        c.kind == AtTargetKind::New
                            && c.owner == owner
                            && name == "<init>"
                            && c.name == "<init>"
                            && c.descriptor == descriptor::method_to_descriptor(&desc)
                    })
                    .collect(),
            )
        }
        _ => Some(by_format),
    }
}
fn filter_constants(
    candidates: Vec<AtTargetCandidate>,
    site: &MixinExtrasAnnotationSite,
) -> Option<Vec<AtTargetCandidate>> {
    let candidates = candidates
        .into_iter()
        .filter(|c| c.kind == AtTargetKind::Constant)
        .collect::<Vec<_>>();
    let mut selected = if !site.expand_zero_conditions.is_empty() {
        let conditions = candidates
            .iter()
            .filter(|c| {
                c.constant_value.is_none()
                    && c.condition_opcode
                        .is_some_and(|op| site.expand_zero_conditions.contains(&op))
            })
            .cloned()
            .collect::<Vec<_>>();
        if site.at_args.is_empty() {
            conditions
        } else {
            let expected = expected_constant(&site.at_args)?;
            if args_map(&site.at_args).keys().any(|k| k != "intValue") {
                return None;
            }
            let mut literal = if expected == ConstantValue::IntValue(0) {
                candidates
                    .into_iter()
                    .filter(|c| {
                        c.condition_opcode.is_none() && c.constant_value.as_ref() == Some(&expected)
                    })
                    .collect()
            } else {
                vec![]
            };
            literal.extend(conditions);
            literal
        }
    } else if site.at_args.is_empty() {
        let handler = site
            .handler_method
            .as_ref()?
            .return_type_descriptor
            .as_deref()?;
        candidates
            .into_iter()
            .filter(|c| {
                c.condition_opcode.is_none() && matches_return(c.constant_value.as_ref(), handler)
            })
            .collect()
    } else {
        let map = args_map(&site.at_args);
        let discriminators = [
            "intValue",
            "floatValue",
            "longValue",
            "doubleValue",
            "stringValue",
            "classValue",
            "nullValue",
        ];
        if map.get("classValue").map(String::as_str) == Some("void")
            && !map
                .keys()
                .any(|k| k != "classValue" && discriminators.contains(&k.as_str()))
        {
            return Some(vec![]);
        }
        let expected = expected_constant(&site.at_args)?;
        candidates
            .into_iter()
            .filter(|c| {
                c.condition_opcode.is_none()
                    && c.constant_value
                        .as_ref()
                        .is_some_and(|v| constant_values_equal(v, &expected))
            })
            .collect()
    };
    if let Some(target) = site
        .at_target
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if broad(target) {
            return None;
        }
        selected.retain(|c| AtTargetCompletionService::format_target(c) == target);
    }
    Some(selected)
}
fn matches_return(value: Option<&ConstantValue>, descriptor: &str) -> bool {
    match (descriptor, value) {
        ("I" | "Z" | "B" | "C" | "S", Some(ConstantValue::IntValue(_)))
        | ("J", Some(ConstantValue::LongValue(_)))
        | ("F", Some(ConstantValue::FloatValue(_)))
        | ("D", Some(ConstantValue::DoubleValue(_)))
        | (
            "Ljava/lang/String;",
            Some(ConstantValue::StringValue(_) | ConstantValue::StringUtf16Value(_)),
        )
        | ("Ljava/lang/Class;", Some(ConstantValue::ClassLiteral(_)))
        | ("Ljava/lang/Object;", Some(ConstantValue::NullValue)) => true,
        _ => false,
    }
}
