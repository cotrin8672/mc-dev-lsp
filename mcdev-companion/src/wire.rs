use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_BATCH: usize = 256;
pub const MAX_HANDLES: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Fact<T> {
    Found { value: T },
    Unknown { reasons: Vec<Value> },
    DefinitelyAbsent,
}
impl<T> Fact<T> {
    pub fn validate(&self) -> Result<(), String> {
        if let Self::Unknown { reasons } = self {
            if reasons.is_empty()
                || reasons.len() > 64
                || reasons
                    .iter()
                    .any(|r| r["code"].as_str().is_none_or(str::is_empty))
            {
                return Err("invalid Unknown reasons".into());
            }
        }
        Ok(())
    }
    pub fn found(&self) -> Option<&T> {
        if let Self::Found { value } = self {
            Some(value)
        } else {
            None
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}
impl ByteRange {
    pub fn text<'a>(&self, source: &'a str) -> Result<&'a str, String> {
        source
            .get(self.start..self.end)
            .filter(|_| self.start <= self.end)
            .ok_or_else(|| "invalid UTF-8 source range".into())
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    pub type_ref: String,
    pub use_site_target: Option<String>,
    pub range: ByteRange,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_range: Option<ByteRange>,
    pub arguments: Vec<Argument>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Argument {
    pub name: Option<String>,
    pub range: ByteRange,
    pub value: Fact<AnnotationValue>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AnnotationValue {
    Class {
        type_ref: String,
    },
    String {
        value: String,
        content_range: ByteRange,
    },
    Boolean {
        value: bool,
    },
    MappedString {
        value: String,
        segments: Vec<StringSegment>,
    },
    Integer {
        #[serde(rename = "type")]
        integer_type: IntegerType,
        value: i64,
    },
    Enum {
        type_ref: String,
        name: String,
    },
    Annotation {
        annotation: Box<Annotation>,
    },
    Array {
        values: Vec<Argument>,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IntegerType {
    Int,
    Long,
}
/// Decoded offsets count UTF-16 code units; source offsets count UTF-8 bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StringSegment {
    pub decoded: ByteRange,
    pub source: ByteRange,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeclarationModifiers {
    pub visibility: Fact<String>,
    #[serde(rename = "static")]
    pub is_static: Fact<bool>,
    #[serde(rename = "final")]
    pub is_final: Fact<bool>,
    #[serde(rename = "abstract")]
    pub is_abstract: Fact<bool>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceParameter {
    pub name: String,
    pub range: ByteRange,
    pub name_range: ByteRange,
    pub type_ref: String,
    pub annotations: Vec<Annotation>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Declaration {
    pub id: String,
    pub owner: Option<String>,
    pub range: ByteRange,
    pub name: Option<String>,
    pub annotations: Vec<Annotation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_range: Option<ByteRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header_range: Option<ByteRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_range: Option<ByteRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_range: Option<ByteRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modifiers: Option<DeclarationModifiers>,
    /// Older hosts omit this fact; omission does not prove an empty parameter list.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_parameters"
    )]
    pub parameters: Option<Fact<Vec<SourceParameter>>>,
}

fn deserialize_parameters<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Fact<Vec<SourceParameter>>>, D::Error> {
    Fact::<Vec<SourceParameter>>::deserialize(deserializer).map(Some)
}
#[derive(Debug, Deserialize)]
pub struct Evaluation {
    pub evaluation: String,
    pub owner: String,
    pub source: String,
    pub configuration: Value,
    pub declarations: Vec<Declaration>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeFact {
    pub id: String,
    pub qualified_name: String,
    #[serde(default)]
    pub package: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub read_only: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_source: Option<crate::host_facts::inputs::SourceDescriptor>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signature {
    pub id: String,
    pub kind: String,
    pub descriptor: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub signature: Fact<Signature>,
    pub dispatch: Fact<String>,
    #[serde(
        default,
        rename = "readOnly",
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub read_only: bool,
    #[serde(
        default,
        rename = "contextSource",
        skip_serializing_if = "Option::is_none"
    )]
    pub context_source: Option<crate::host_facts::inputs::SourceDescriptor>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Site {
    pub requirement: String,
    pub full_range: ByteRange,
    pub editable_name: ByteRange,
    pub rename_subject: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub range: ByteRange,
    pub severity: String,
    pub code: Option<String>,
    pub message: String,
}
#[derive(Debug, Default, Serialize)]
pub struct Contributions {
    pub sites: Vec<Site>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleKind {
    Declaration,
    TypeRef,
    Type,
    Member,
    Signature,
    Requirement,
}
#[derive(Default)]
pub struct Registry(BTreeMap<String, HandleKind>);
impl Registry {
    pub fn register(&mut self, id: &str, kind: HandleKind) -> Result<(), String> {
        if id.is_empty() || id.len() > 256 || self.0.len() >= MAX_HANDLES {
            return Err("invalid or excessive opaque handles".into());
        }
        if self
            .0
            .insert(id.into(), kind)
            .is_some_and(|old| old != kind)
        {
            return Err("opaque handle type collision".into());
        }
        Ok(())
    }
    pub fn require(&self, id: &str, kind: HandleKind) -> Result<(), String> {
        if self.0.get(id) == Some(&kind) {
            Ok(())
        } else {
            Err("foreign or wrongly typed opaque handle".into())
        }
    }
}
impl Evaluation {
    pub fn validate(&self, registry: &mut Registry) -> Result<(), String> {
        if self.evaluation.is_empty()
            || self.evaluation.len() > 256
            || self.owner.is_empty()
            || self.declarations.len() > MAX_HANDLES
        {
            return Err("invalid evaluation".into());
        }
        validate_declarations(&self.declarations, &self.source, registry, false)
    }
}

pub fn validate_declarations(
    declarations: &[Declaration],
    source: &str,
    registry: &mut Registry,
    detailed: bool,
) -> Result<(), String> {
    if declarations.len() > MAX_HANDLES {
        return Err("too many source declarations".into());
    }
    let mut ids = BTreeSet::new();
    for d in declarations {
        if !ids.insert(&d.id) {
            return Err("duplicate declaration handle".into());
        }
        registry.register(&d.id, HandleKind::Declaration)?;
        if d.name.as_deref() != Some(d.range.text(source)?) {
            return Err("declaration spelling/range mismatch".into());
        }
    }
    for d in declarations {
        if let Some(owner) = &d.owner {
            registry.require(owner, HandleKind::Declaration)?;
            if owner == &d.id {
                return Err("self-owned declaration".into());
            }
        }
        if detailed {
            let full = d
                .full_range
                .as_ref()
                .ok_or("missing full declaration range")?;
            let header = d
                .header_range
                .as_ref()
                .ok_or("missing declaration header range")?;
            let name = d
                .name_range
                .as_ref()
                .ok_or("missing declaration name range")?;
            full.text(source)?;
            header.text(source)?;
            if d.source_kind.as_deref().is_none_or(str::is_empty)
                || name != &d.range
                || !contains(full, header)
                || !contains(header, name)
                || d.body_range
                    .as_ref()
                    .is_some_and(|body| !contains(full, body))
            {
                return Err("inconsistent authored declaration ranges".into());
            }
            if let Some(body) = &d.body_range {
                body.text(source)?;
            }
            let modifiers = d
                .modifiers
                .as_ref()
                .ok_or("missing declaration modifiers")?;
            modifiers.visibility.validate()?;
            modifiers.is_static.validate()?;
            modifiers.is_final.validate()?;
            modifiers.is_abstract.validate()?;
            validate_parameters(d, full, header, source, registry)?;
        }
        for annotation in &d.annotations {
            let mut nodes = 0;
            validate_annotation(annotation, source, registry, 0, detailed, &mut nodes)?;
        }
    }
    Ok(())
}

fn contains(parent: &ByteRange, child: &ByteRange) -> bool {
    parent.start <= child.start && child.start <= child.end && child.end <= parent.end
}

fn validate_parameters(
    declaration: &Declaration,
    full: &ByteRange,
    header: &ByteRange,
    source: &str,
    registry: &mut Registry,
) -> Result<(), String> {
    let Some(parameters) = &declaration.parameters else {
        return Ok(());
    };
    parameters.validate()?;
    let parameters = match parameters {
        Fact::Found { value } if declaration.source_kind.as_deref() == Some("Function") => value,
        Fact::Unknown { .. } => return Ok(()),
        _ => return Err("parameter knowledge must be Unknown or a supported callable list".into()),
    };
    if parameters.len() > 255 {
        return Err("parameter normalization limit".into());
    }
    let mut previous_end = header.start;
    let mut names = BTreeSet::new();
    for parameter in parameters {
        parameter.range.text(source)?;
        let raw_name = parameter.name_range.text(source)?;
        let source_name = raw_name
            .strip_prefix('`')
            .and_then(|name| name.strip_suffix('`'))
            .unwrap_or(raw_name);
        if parameter.name.is_empty()
            || source_name.contains('`')
            || source_name != parameter.name
            || !names.insert(&parameter.name)
            || !contains(full, &parameter.range)
            || !contains(header, &parameter.range)
            || !contains(&parameter.range, &parameter.name_range)
            || parameter.range.start < previous_end
        {
            return Err("invalid parameter name, range or source order".into());
        }
        previous_end = parameter.range.end;
        registry.register(&parameter.type_ref, HandleKind::TypeRef)?;
        for annotation in &parameter.annotations {
            if !contains(&parameter.range, &annotation.range) {
                return Err("parameter annotation is outside the parameter range".into());
            }
            let mut nodes = 0;
            validate_annotation(annotation, source, registry, 0, true, &mut nodes)?;
        }
    }
    Ok(())
}

pub(crate) fn validate_annotation(
    annotation: &Annotation,
    source: &str,
    registry: &mut Registry,
    depth: usize,
    detailed: bool,
    nodes: &mut usize,
) -> Result<(), String> {
    if depth > 128 || (detailed && annotation.range.text(source)?.len() > 64 * 1024) {
        return Err("annotation normalization limit".into());
    }
    annotation.range.text(source)?;
    registry.register(&annotation.type_ref, HandleKind::TypeRef)?;
    if detailed {
        let name = annotation
            .name_range
            .as_ref()
            .ok_or("missing annotation name range")?;
        if name.text(source)?.is_empty() || !contains(&annotation.range, name) {
            return Err("annotation name range is outside annotation".into());
        }
    }
    for argument in &annotation.arguments {
        if !contains(&annotation.range, &argument.range) {
            return Err("argument range is outside annotation".into());
        }
        validate_argument(argument, source, registry, depth, detailed, nodes)?;
    }
    Ok(())
}

fn validate_argument(
    a: &Argument,
    source: &str,
    registry: &mut Registry,
    depth: usize,
    detailed: bool,
    nodes: &mut usize,
) -> Result<(), String> {
    *nodes += 1;
    if depth > if detailed { 128 } else { 32 } || (detailed && *nodes > 4096) {
        return Err("annotation nesting limit".into());
    }
    a.range.text(source)?;
    a.value.validate()?;
    match a.value.found() {
        Some(AnnotationValue::Class { type_ref } | AnnotationValue::Enum { type_ref, .. }) => {
            registry.register(type_ref, HandleKind::TypeRef)?
        }
        Some(AnnotationValue::String {
            value,
            content_range,
        }) => {
            if !contains(&a.range, content_range) || content_range.text(source)? != value {
                return Err("plain string content does not match source".into());
            }
        }
        Some(AnnotationValue::MappedString { value, segments }) => {
            let mut decoded = 0;
            let mut previous_end = a.range.start;
            if value.chars().count() > 16_384 || segments.len() != value.chars().count() {
                return Err("mapped string scalar limit or cardinality mismatch".into());
            }
            for (scalar, segment) in value.chars().zip(segments) {
                if segment.decoded.start != decoded
                    || segment.decoded.end != decoded + scalar.len_utf16()
                    || segment.source.start < previous_end
                    || !contains(&a.range, &segment.source)
                    || segment.source.text(source)?.is_empty()
                {
                    return Err("invalid mapped string scalar or source range".into());
                }
                decoded = segment.decoded.end;
                previous_end = segment.source.end;
            }
        }
        Some(AnnotationValue::Integer {
            integer_type: IntegerType::Int,
            value,
        }) if i32::try_from(*value).is_err() => {
            return Err("int annotation value is out of range".into());
        }
        Some(AnnotationValue::Annotation { annotation }) => {
            if !contains(&a.range, &annotation.range) {
                return Err("nested annotation range is outside argument".into());
            }
            validate_annotation(annotation, source, registry, depth + 1, detailed, nodes)?;
        }
        Some(AnnotationValue::Array { values }) => {
            if values.len() > MAX_HANDLES {
                return Err("annotation array limit".into());
            }
            for child in values {
                if !contains(&a.range, &child.range) {
                    return Err("array element range is outside argument".into());
                }
                validate_argument(child, source, registry, depth + 1, detailed, nodes)?;
            }
        }
        _ => (),
    }
    Ok(())
}
