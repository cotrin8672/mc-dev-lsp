//! The negotiated binary declaration-annotation representation, without runtime authority.
use mcdev_core::descriptor::{parse_field_descriptor, JvmType};
use serde::{de::Error, Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BinaryAnnotation {
    pub runtime_visible: bool,
    pub descriptor: String,
    pub elements: Vec<BinaryAnnotationElement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BinaryNestedAnnotation {
    pub descriptor: String,
    pub elements: Vec<BinaryAnnotationElement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BinaryAnnotationElement {
    pub name: String,
    pub value: BinaryAnnotationValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum BinaryAnnotationValue {
    Byte {
        value: i8,
    },
    Char {
        value: u16,
    },
    Double {
        bits: String,
    },
    Float {
        bits: String,
    },
    Int {
        value: i32,
    },
    Long {
        value: String,
    },
    Short {
        value: i16,
    },
    Boolean {
        value: bool,
    },
    String {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        utf16: Option<Vec<u16>>,
    },
    Enum {
        descriptor: String,
        name: String,
    },
    Class {
        descriptor: String,
    },
    Annotation {
        value: BinaryNestedAnnotation,
    },
    Array {
        values: Vec<BinaryAnnotationValue>,
    },
}

fn object_descriptor(descriptor: &str) -> bool {
    matches!(
        parse_field_descriptor(descriptor),
        Ok(JvmType::ObjectType(_))
    )
}

fn annotation(
    descriptor: &str,
    elements: &[BinaryAnnotationElement],
    depth: usize,
) -> Result<(), String> {
    if depth > 128 || !object_descriptor(descriptor) {
        return Err("invalid binary annotation descriptor or nesting".into());
    }
    let mut names = std::collections::BTreeSet::new();
    for element in elements {
        if element.name.is_empty() || !names.insert(&element.name) {
            return Err("empty or duplicate binary annotation element name".into());
        }
        element.value.validate(depth + 1)?;
    }
    Ok(())
}

impl BinaryAnnotationValue {
    fn validate(&self, depth: usize) -> Result<(), String> {
        if depth > 128 {
            return Err("binary annotation nesting limit".into());
        }
        let hex = |value: &str, length| {
            value.len() == length
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        };
        match self {
            Self::Double { bits } if !hex(bits, 16) => {
                return Err("invalid double annotation bits".into())
            }
            Self::Float { bits } if !hex(bits, 8) => {
                return Err("invalid float annotation bits".into())
            }
            Self::Long { value }
                if value
                    .parse::<i64>()
                    .ok()
                    .is_none_or(|parsed| parsed.to_string() != *value) =>
            {
                return Err("invalid long annotation decimal string".into())
            }
            Self::String { value, utf16 } if value.is_some() == utf16.is_some() => {
                return Err(
                    "binary string needs exactly one Unicode or UTF-16 representation".into(),
                )
            }
            Self::Enum { descriptor, name }
                if !object_descriptor(descriptor) || name.is_empty() =>
            {
                return Err("invalid binary enum annotation value".into())
            }
            Self::Class { descriptor }
                if descriptor != "V" && parse_field_descriptor(descriptor).is_err() =>
            {
                return Err("invalid binary class annotation descriptor".into())
            }
            Self::Annotation { value } => {
                annotation(&value.descriptor, &value.elements, depth + 1)?
            }
            Self::Array { values } => {
                for value in values {
                    value.validate(depth + 1)?;
                }
            }
            _ => (),
        }
        Ok(())
    }
}

pub(super) fn deserialize_annotations<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Vec<BinaryAnnotation>>, D::Error> {
    let result = Vec::<BinaryAnnotation>::deserialize(deserializer)?;
    for value in &result {
        annotation(&value.descriptor, &value.elements, 0).map_err(D::Error::custom)?;
    }
    Ok(Some(result))
}
