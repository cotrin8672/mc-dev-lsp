use super::{EngineError, EngineResult, Name, SourceSpan, TypeName};
use crate::descriptor::{
    parse_field_descriptor, parse_field_selector, parse_method_descriptor, parse_method_selector,
    FieldSelector, MethodSelector, Pattern,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LocalSpec {
    pub args_only: bool,
    pub index: Option<i32>,
    pub ordinal: Option<i32>,
    pub names: BTreeSet<String>,
    pub print: bool,
    pub type_class_name: Option<String>,
}
impl std::fmt::Display for LocalSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Local(argsOnly={}, index={}, ordinal={}, names=[{}], print={}, typeClassName={})",
            self.args_only,
            self.index
                .map(|v| v.to_string())
                .unwrap_or_else(|| "null".into()),
            self.ordinal
                .map(|v| v.to_string())
                .unwrap_or_else(|| "null".into()),
            self.names.iter().cloned().collect::<Vec<_>>().join(", "),
            self.print,
            self.type_class_name.as_deref().unwrap_or("null")
        )
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalDefinition {
    pub id: String,
    pub spec: LocalSpec,
    pub descriptor: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberDefinition {
    Field(FieldSelector),
    Method(MethodSelector),
    LocalIndex(i32),
    ArrayLength,
}
impl MemberDefinition {
    /// Tests an instruction definition without requiring a containing method.
    pub fn matches_instruction(&self, instruction: &super::Instruction) -> bool {
        use super::InstructionOperand;
        match self {
            Self::ArrayLength => instruction.opcode == 0xbe,
            Self::LocalIndex(slot) => {
                matches!(&instruction.operand,InstructionOperand::Local(value) if *value as i32==*slot)
            }
            Self::Field(selector) => match &instruction.operand {
                InstructionOperand::Member {
                    owner,
                    name,
                    descriptor,
                    ..
                } if matches!(instruction.opcode, 0xb2..=0xb5) => {
                    matches_field(selector, owner, name, descriptor)
                }
                _ => false,
            },
            Self::Method(selector) => match &instruction.operand {
                InstructionOperand::Member {
                    owner,
                    name,
                    descriptor,
                    ..
                } if matches!(instruction.opcode, 0xb6..=0xb9) => {
                    matches_method(selector, owner, name, descriptor)
                }
                _ => false,
            },
        }
    }
    pub fn matches_handle(&self, handle: &super::JvmConstant) -> bool {
        match (self, handle) {
            (
                Self::Method(selector),
                super::JvmConstant::MethodHandle {
                    tag: 5 | 6 | 7 | 9,
                    owner,
                    name,
                    descriptor,
                    ..
                },
            ) => matches_method(selector, owner, name, descriptor),
            _ => false,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentifierPool {
    pub members: BTreeMap<String, Vec<MemberDefinition>>,
    pub types: BTreeMap<String, Vec<String>>,
    pub locals: Vec<LocalDefinition>,
}
impl Default for IdentifierPool {
    fn default() -> Self {
        let mut result = Self {
            members: BTreeMap::new(),
            types: BTreeMap::new(),
            locals: Vec::new(),
        };
        for (name, descriptor) in [
            ("byte", "B"),
            ("char", "C"),
            ("double", "D"),
            ("float", "F"),
            ("int", "I"),
            ("long", "J"),
            ("short", "S"),
        ] {
            result.add_type(name, descriptor);
        }
        result.add_member("length", MemberDefinition::ArrayLength);
        result
    }
}
impl IdentifierPool {
    pub fn add_type(&mut self, id: impl Into<String>, descriptor: impl Into<String>) {
        self.types
            .entry(id.into())
            .or_default()
            .push(descriptor.into());
    }
    pub fn add_member(&mut self, id: impl Into<String>, definition: MemberDefinition) {
        self.members.entry(id.into()).or_default().push(definition);
    }
    pub fn identifier_exists(&self, id: &str) -> bool {
        self.members.contains_key(id) || self.types.contains_key(id)
    }
    pub fn member_exists(&self, id: &str) -> bool {
        self.members.contains_key(id)
    }
    pub fn type_exists(&self, id: &str) -> bool {
        self.types.contains_key(id)
    }
    pub(crate) fn matches_type(&self, name: &TypeName, descriptor: &str) -> EngineResult<bool> {
        let mut component = descriptor;
        for _ in 0..name.dimensions {
            let Some(inner) = component.strip_prefix('[') else {
                return Ok(false);
            };
            component = inner;
        }
        match &name.name {
            Name::Wildcard => Ok(true),
            Name::Defined(id) => {
                let entries = self.types.get(id).ok_or_else(|| {
                    EngineError::Unavailable(format!("Use of undeclared identifier '{id}'"))
                })?;
                Ok(entries.iter().any(|value| value == component))
            }
        }
    }
}
pub(crate) fn string_pattern(pattern: &Pattern<String>, value: &str) -> bool {
    match pattern {
        Pattern::Any => true,
        Pattern::Exact(expected) => expected == value,
    }
}
pub(crate) fn matches_method(
    selector: &MethodSelector,
    owner: &str,
    name: &str,
    descriptor: &str,
) -> bool {
    string_pattern(&selector.owner, owner)
        && string_pattern(&selector.name, name)
        && match &selector.descriptor {
            Pattern::Any => true,
            Pattern::Exact(expected) => {
                parse_method_descriptor(descriptor).is_ok_and(|value| &value == expected)
            }
        }
}
pub(crate) fn matches_field(
    selector: &FieldSelector,
    owner: &str,
    name: &str,
    descriptor: &str,
) -> bool {
    string_pattern(&selector.owner, owner)
        && string_pattern(&selector.name, name)
        && match &selector.descriptor {
            Pattern::Any => true,
            Pattern::Exact(expected) => {
                parse_field_descriptor(descriptor).is_ok_and(|value| &value == expected)
            }
        }
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DefinitionInput {
    pub id: Option<String>,
    pub raw_method_references: Vec<String>,
    pub raw_field_references: Vec<String>,
    pub class_literal_type_names: Vec<String>,
    pub local_specs: Vec<LocalSpec>,
    pub parse_issues: Vec<DefinitionParseIssue>,
    pub source_range: Option<SourceSpan>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinitionParseIssue {
    pub attribute: String,
    pub raw_value: String,
    pub message: String,
    pub body_range: SourceSpan,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentifierPoolBuildIssue {
    pub definition_id: Option<String>,
    pub attribute: String,
    pub raw_value: String,
    pub message: String,
    pub source_range: Option<SourceSpan>,
}
#[derive(Debug, Clone)]
pub struct IdentifierPoolBuildResult {
    pub pool: IdentifierPool,
    pub issues: Vec<IdentifierPoolBuildIssue>,
}
pub fn build_identifier_pool(
    definitions: &[DefinitionInput],
    resolve_type: &dyn Fn(&str) -> Option<String>,
) -> IdentifierPoolBuildResult {
    let mut pool = IdentifierPool::default();
    let mut issues = Vec::new();
    for definition in definitions {
        for issue in &definition.parse_issues {
            issues.push(IdentifierPoolBuildIssue {
                definition_id: definition.id.clone(),
                attribute: issue.attribute.clone(),
                raw_value: issue.raw_value.clone(),
                message: issue.message.clone(),
                source_range: definition.source_range.map(|body| SourceSpan {
                    start: body.start + issue.body_range.start,
                    end: body.start + issue.body_range.end,
                }),
            });
        }
        let mut issue = |attribute: &str, raw: &str, message: &str| {
            issues.push(IdentifierPoolBuildIssue {
                definition_id: definition.id.clone(),
                attribute: attribute.into(),
                raw_value: raw.into(),
                message: message.into(),
                source_range: definition.source_range,
            })
        };
        if definition.id.is_none()
            && definition
                .parse_issues
                .iter()
                .all(|issue| issue.attribute != "id")
            && definition.raw_method_references.is_empty()
            && definition.raw_field_references.is_empty()
            && definition.class_literal_type_names.is_empty()
            && definition.local_specs.is_empty()
        {
            issue("id", "", "definition requires a string id");
        }
        for raw in &definition.raw_method_references {
            let Some(id) = definition.id.as_ref() else {
                issue("method", raw, "method definition requires id");
                continue;
            };
            match parse_method_selector(raw) {
                Ok(selector) => pool.add_member(id, MemberDefinition::Method(selector)),
                Err(error) => issue("method", raw, &error.message),
            }
        }
        for raw in &definition.raw_field_references {
            let Some(id) = definition.id.as_ref() else {
                issue("field", raw, "field definition requires id");
                continue;
            };
            match parse_field_selector(raw) {
                Ok(selector) => pool.add_member(id, MemberDefinition::Field(selector)),
                Err(error) => issue("field", raw, &error.message),
            }
        }
        for raw in &definition.class_literal_type_names {
            let Some(id) = definition.id.as_ref() else {
                issue("type", raw, "type definition requires id");
                continue;
            };
            match resolve_type(raw) {
                None => issue("type", raw, type_resolution_failure(raw)),
                Some(descriptor) => {
                    if descriptor == "V" || parse_field_descriptor(&descriptor).is_ok() {
                        pool.add_type(id, descriptor);
                    } else {
                        issue("type", raw, "invalid resolved type");
                    }
                }
            }
        }
        for spec in &definition.local_specs {
            let raw = spec.to_string();
            let Some(id) = definition.id.as_ref() else {
                issue("local", &raw, "local definition requires id");
                continue;
            };
            let descriptor = spec
                .type_class_name
                .as_deref()
                .and_then(resolve_type)
                .filter(|value| parse_field_descriptor(value).is_ok());
            match descriptor {
                Some(descriptor) => {
                    pool.locals.push(LocalDefinition {
                        id: id.clone(),
                        spec: spec.clone(),
                        descriptor,
                    });
                    pool.add_member(id, MemberDefinition::LocalIndex(-1));
                }
                None => issue(
                    "local",
                    &raw,
                    "local definition requires a resolvable non-void type",
                ),
            }
        }
    }
    IdentifierPoolBuildResult { pool, issues }
}
fn type_resolution_failure(name: &str) -> &'static str {
    if name.starts_with('(')
        || name
            .split_once(';')
            .is_some_and(|(_, rest)| rest.contains('('))
    {
        return "method descriptor is not a type";
    }
    let base = name.split('[').next().unwrap_or(name);
    if base.starts_with('L') || (base.len() == 1 && "BCDFIJSVL".contains(base)) {
        "invalid type descriptor"
    } else {
        "unresolved type name"
    }
}
