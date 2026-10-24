//! Exact field model for project-type-facts143-wire.md (proposed, unimplemented).
//! No project identity, revision counter or external read-set authority is added.
use super::FactsError;
use crate::wire::{Fact, Member};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

pub const CAPABILITY: &str = "project-class-facts-v1";
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Operation {
    ProjectContext,
    TypeJvm,
}
impl Operation {
    pub fn name(self) -> &'static str {
        match self {
            Self::ProjectContext => "projectContext",
            Self::TypeJvm => "typeJvm",
        }
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SupportedOps(BTreeSet<Operation>);
impl SupportedOps {
    pub fn intersect(host: &Value) -> Result<Self, FactsError> {
        if !host["capabilities"]
            .as_array()
            .is_some_and(|caps| caps.iter().any(|c| c == CAPABILITY))
        {
            return Ok(Self::default());
        }
        let Some(map) = host.get("supportedOps") else {
            return Ok(Self::default());
        };
        let map = map
            .as_object()
            .ok_or_else(|| FactsError::Protocol("supportedOps must be an object".into()))?;
        let Some(values) = map.get(CAPABILITY) else {
            return Ok(Self::default());
        };
        let values = values.as_array().ok_or_else(|| {
            FactsError::Protocol("optional operation set must be an array".into())
        })?;
        let mut accepted = BTreeSet::new();
        for name in values {
            match name.as_str().ok_or_else(|| {
                FactsError::Protocol("optional operation names must be strings".into())
            })? {
                "projectContext" => {
                    accepted.insert(Operation::ProjectContext);
                }
                "typeJvm" => {
                    accepted.insert(Operation::TypeJvm);
                }
                _ => (),
            }
        }
        Ok(Self(accepted))
    }
    pub fn supports(&self, op: Operation) -> bool {
        self.0.contains(&op)
    }
    pub fn accepted(&self) -> Vec<&'static str> {
        self.0.iter().map(|op| op.name()).collect()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedScope {
    pub module: String,
    pub source_set: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClasspathEntry {
    pub id: String,
    pub display_path: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Classpath {
    pub entries: Vec<ClasspathEntry>,
    pub coverage: Fact<()>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SdkApi {
    Full,
    Release { release: u16 },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sdk {
    pub identity: String,
    pub api: SdkApi,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectContext {
    pub workspace_uri: Fact<String>,
    pub scope: Fact<SelectedScope>,
    pub source_roots: Fact<Vec<String>>,
    pub output_roots: Fact<Vec<String>>,
    pub classpath: Fact<Classpath>,
    pub sdk: Fact<Sdk>,
}
impl ProjectContext {
    pub fn validate(&self) -> Result<(), FactsError> {
        for result in [
            self.workspace_uri.validate(),
            self.scope.validate(),
            self.source_roots.validate(),
            self.output_roots.validate(),
            self.classpath.validate(),
            self.sdk.validate(),
        ] {
            result.map_err(FactsError::Protocol)?;
        }
        if let Some(classpath) = self.classpath.found() {
            classpath
                .coverage
                .validate()
                .map_err(FactsError::Protocol)?;
            for entry in &classpath.entries {
                if entry.id.is_empty() {
                    return Err(FactsError::Protocol(
                        "invalid selected classpath metadata".into(),
                    ));
                }
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinaryName {
    pub internal_name: String,
    pub descriptor: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditingMember {
    #[serde(flatten)]
    pub member: Member,
    pub visibility: Fact<String>,
    pub synthetic: Fact<bool>,
    pub mismatches: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeclaredMembers {
    pub members: Vec<EditingMember>,
    pub coverage: Fact<()>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbiIdentity {
    pub origin_id: String,
    pub internal_name: String,
    pub content_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RawMember {
    pub name: String,
    pub descriptor: String,
    pub access_flags: u16,
    pub signature: Option<String>,
    pub attribute_names: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InnerClass {
    pub inner: String,
    pub outer: Option<String>,
    pub name: Option<String>,
    pub access_flags: u16,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawAbi {
    pub identity: AbiIdentity,
    pub major_version: u16,
    pub minor_version: u16,
    pub access_flags: u16,
    pub super_name: Option<String>,
    pub interfaces: Vec<String>,
    pub signature: Option<String>,
    pub source_file: Option<String>,
    pub attribute_names: Vec<String>,
    pub inner_classes: Vec<InnerClass>,
    pub fields: Vec<RawMember>,
    pub methods: Vec<RawMember>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeJvm {
    pub binary_name: Fact<BinaryName>,
    pub class_flags: Fact<u16>,
    pub declared_members: DeclaredMembers,
    pub abi: Fact<RawAbi>,
    pub jvm_abi_coverage: Fact<()>,
}
