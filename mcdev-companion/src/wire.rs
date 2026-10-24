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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    pub type_ref: String,
    pub use_site_target: Option<String>,
    pub range: ByteRange,
    pub arguments: Vec<Argument>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Argument {
    pub name: Option<String>,
    pub range: ByteRange,
    pub value: Fact<AnnotationValue>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    Array {
        values: Vec<Argument>,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Declaration {
    pub id: String,
    pub owner: Option<String>,
    pub range: ByteRange,
    pub name: Option<String>,
    pub annotations: Vec<Annotation>,
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
        let mut ids = BTreeSet::new();
        for d in &self.declarations {
            if !ids.insert(&d.id) {
                return Err("duplicate declaration handle".into());
            }
            registry.register(&d.id, HandleKind::Declaration)?;
            if d.name.as_deref() != Some(d.range.text(&self.source)?) {
                return Err("declaration spelling/range mismatch".into());
            }
        }
        for d in &self.declarations {
            if let Some(owner) = &d.owner {
                registry.require(owner, HandleKind::Declaration)?;
                if owner == &d.id {
                    return Err("self-owned declaration".into());
                }
            }
            for annotation in &d.annotations {
                annotation.range.text(&self.source)?;
                registry.register(&annotation.type_ref, HandleKind::TypeRef)?;
                for argument in &annotation.arguments {
                    validate_argument(argument, &self.source, registry, 0)?;
                }
            }
        }
        Ok(())
    }
}
fn validate_argument(
    a: &Argument,
    source: &str,
    registry: &mut Registry,
    depth: usize,
) -> Result<(), String> {
    if depth > 32 {
        return Err("annotation nesting limit".into());
    }
    a.range.text(source)?;
    a.value.validate()?;
    match a.value.found() {
        Some(AnnotationValue::Class { type_ref }) => {
            registry.register(type_ref, HandleKind::TypeRef)?
        }
        Some(AnnotationValue::String { content_range, .. }) => {
            content_range.text(source)?;
        }
        Some(AnnotationValue::Array { values }) => {
            if values.len() > MAX_HANDLES {
                return Err("annotation array limit".into());
            }
            for a in values {
                validate_argument(a, source, registry, depth + 1)?;
            }
        }
        _ => (),
    }
    Ok(())
}
