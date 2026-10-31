use crate::descriptor::{to_descriptor, JvmType};
use serde::{Deserialize, Serialize};

/// A resolved JVM type supplied by the semantic authority. Type-variable bounds,
/// captures and projections must already have their JVM erasure chosen by it.
/// This is deliberately not a parser for a backend's private signature syntax.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum HostTypeFact {
    Primitive(HostPrimitive),
    Class {
        internal_name: String,
    },
    Array {
        element: Box<HostTypeFact>,
        dimensions: usize,
    },
    Erasure(Box<HostTypeFact>),
    Candidates(Vec<HostTypeFact>),
    Recovered,
    Unresolved,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HostPrimitive {
    Void,
    Boolean,
    Byte,
    Char,
    Short,
    Int,
    Long,
    Float,
    Double,
}
impl HostTypeFact {
    pub fn jvm_type(&self) -> Option<JvmType> {
        Some(match self {
            Self::Primitive(p) => match p {
                HostPrimitive::Void => JvmType::VoidType,
                HostPrimitive::Boolean => JvmType::BooleanType,
                HostPrimitive::Byte => JvmType::ByteType,
                HostPrimitive::Char => JvmType::CharType,
                HostPrimitive::Short => JvmType::ShortType,
                HostPrimitive::Int => JvmType::IntType,
                HostPrimitive::Long => JvmType::LongType,
                HostPrimitive::Float => JvmType::FloatType,
                HostPrimitive::Double => JvmType::DoubleType,
            },
            Self::Class { internal_name } => {
                let descriptor = format!("L{internal_name};");
                crate::descriptor::parse_field_descriptor(&descriptor).ok()?
            }
            Self::Array {
                element,
                dimensions,
            } => {
                if *dimensions == 0 || *dimensions > 255 {
                    return None;
                }
                let mut ty = element.jvm_type()?;
                if ty == JvmType::VoidType {
                    return None;
                }
                for _ in 0..*dimensions {
                    ty = JvmType::ArrayType(Box::new(ty));
                }
                // Also checks total dimensions when facts contain nested arrays.
                let mut component = &ty;
                let mut total = 0;
                while let JvmType::ArrayType(inner) = component {
                    total += 1;
                    component = inner;
                }
                if total > 255 {
                    return None;
                }
                crate::descriptor::parse_field_descriptor(&to_descriptor(&ty)).ok()?
            }
            Self::Erasure(erasure) => erasure.jvm_type()?,
            Self::Candidates(candidates) => {
                let mut types = vec![];
                for candidate in candidates {
                    let ty = candidate.jvm_type()?;
                    if !types.contains(&ty) {
                        types.push(ty);
                    }
                }
                if types.len() != 1 {
                    return None;
                }
                types.pop()?
            }
            Self::Recovered | Self::Unresolved => return None,
        })
    }
    pub fn descriptor(&self) -> Option<String> {
        self.jvm_type().map(|t| to_descriptor(&t))
    }
}

/// The host folds constants. Width tags and UTF16 code units are retained;
/// folding language expressions or replacing malformed strings is not MC's job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum HostConstant {
    String(Vec<u16>),
    Boolean(bool),
    Int(i32),
    Long(i64),
    Char(u16),
    FloatBits(u32),
    DoubleBits(u64),
    Null,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostNodeValidity {
    pub recovered: bool,
    pub malformed: bool,
}
impl HostNodeValidity {
    pub fn is_usable(&self) -> bool {
        !self.recovered && !self.malformed
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum HostExpressionKind {
    Constant(HostConstant),
    TypeLiteral(HostTypeFact),
    Enum { owner: HostTypeFact, name: String },
    Array(Vec<Option<HostExpressionFact>>),
    Annotation(Box<HostAnnotationFact>),
    Unresolved,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostExpressionFact {
    pub validity: HostNodeValidity,
    pub kind: HostExpressionKind,
    pub source_text: String,
    /// UTF8 bytes in the exact source snapshot, never backend scalar offsets.
    pub source_range: Option<std::ops::Range<usize>>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostAnnotationFact {
    pub source_name: String,
    pub qualified_name: Option<String>,
    pub validity: HostNodeValidity,
    pub exists: bool,
    /// Ordered pairs preserve malformed duplicate attributes for validation.
    pub members: Vec<(String, HostExpressionFact)>,
    pub members_available: bool,
    pub source_range: Option<std::ops::Range<usize>>,
    pub body_range: Option<std::ops::Range<usize>>,
}
impl HostAnnotationFact {
    pub fn member(&self, name: &str) -> Option<&HostExpressionFact> {
        if !self.validity.is_usable() || !self.members_available {
            return None;
        }
        let mut matches = self.members.iter().filter(|(key, _)| key == name);
        let first = &matches.next()?.1;
        if matches.next().is_some() {
            None
        } else {
            Some(first)
        }
    }
    pub fn has_unique_members(&self) -> bool {
        let mut names = std::collections::BTreeSet::new();
        self.members_available && self.members.iter().all(|(name, _)| names.insert(name))
    }
}
impl HostExpressionFact {
    pub fn constant_utf16(&self) -> Option<&[u16]> {
        if !self.validity.is_usable() {
            return None;
        }
        match &self.kind {
            HostExpressionKind::Constant(HostConstant::String(s)) => Some(s),
            _ => None,
        }
    }
    /// Checked convenience only; raw UTF16 remains available for wire/engine APIs.
    pub fn constant_string(&self) -> Option<String> {
        String::from_utf16(self.constant_utf16()?).ok()
    }
    pub fn constant_boolean(&self) -> Option<bool> {
        if !self.validity.is_usable() {
            return None;
        }
        match self.kind {
            HostExpressionKind::Constant(HostConstant::Boolean(v)) => Some(v),
            _ => None,
        }
    }
    pub fn constant_int(&self) -> Option<i32> {
        if !self.validity.is_usable() {
            return None;
        }
        match self.kind {
            HostExpressionKind::Constant(HostConstant::Int(v)) => Some(v),
            _ => None,
        }
    }
    pub fn type_literal_descriptor(&self) -> Option<String> {
        if !self.validity.is_usable() {
            return None;
        }
        match &self.kind {
            HostExpressionKind::TypeLiteral(ty) => ty.descriptor(),
            _ => None,
        }
    }
    pub fn type_literal_source_name(&self) -> Option<String> {
        let descriptor = self.type_literal_descriptor()?;
        Some(
            match descriptor.as_str() {
                "V" => "void",
                "Z" => "boolean",
                "B" => "byte",
                "C" => "char",
                "S" => "short",
                "I" => "int",
                "J" => "long",
                "F" => "float",
                "D" => "double",
                _ => return Some(descriptor),
            }
            .into(),
        )
    }
    fn map_elements<T>(&self, map: impl Fn(&Self) -> Option<T>) -> Option<Vec<T>> {
        if !self.validity.is_usable() {
            return None;
        }
        match &self.kind {
            HostExpressionKind::Array(elements) => {
                elements.iter().map(|e| map(e.as_ref()?)).collect()
            }
            _ => Some(vec![map(self)?]),
        }
    }
    pub fn string_array(&self) -> Option<Vec<String>> {
        self.map_elements(Self::constant_string)
    }
    pub fn string_array_utf16(&self) -> Option<Vec<Vec<u16>>> {
        self.map_elements(|e| Some(e.constant_utf16()?.to_vec()))
    }
    pub fn type_array(&self) -> Option<Vec<String>> {
        self.map_elements(Self::type_literal_descriptor)
    }
    pub fn type_array_source_names(&self) -> Option<Vec<String>> {
        self.map_elements(Self::type_literal_source_name)
    }
    pub fn nested_annotations(&self) -> Option<Vec<HostAnnotationFact>> {
        self.map_elements(|e| match &e.kind {
            HostExpressionKind::Annotation(annotation)
                if e.validity.is_usable() && annotation.validity.is_usable() =>
            {
                Some((**annotation).clone())
            }
            _ => None,
        })
    }
}
