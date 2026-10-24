//! Scoped two-operation adapter. Host resume/finish owns lifetime/read validation.
//! This adapter has no generation counters, persistent cache or project scanner.
use super::{draft::*, FactsError};
use crate::wire::{HandleKind, Registry};
use mcdev_core::{
    descriptor::{self, JvmType},
    host_source::HostFact,
    FieldIndexEntry, MethodIndexEntry,
};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// The existing framed RPC actor must perform evaluation routing, cancellation,
/// stale reply rejection and all global frame/batch/deadline limits. It returns
/// the existing `analysis/readFacts` result object containing `facts`.
pub trait FactReader {
    fn read_facts(&mut self, evaluation: &str, queries: &[Value]) -> Result<Value, FactsError>;
}
pub struct ScopedFacts<'a, R: FactReader> {
    evaluation: &'a str,
    ops: &'a SupportedOps,
    registry: &'a mut Registry,
    reader: &'a mut R,
}
impl<'a, R: FactReader> ScopedFacts<'a, R> {
    pub fn new(
        evaluation: &'a str,
        ops: &'a SupportedOps,
        registry: &'a mut Registry,
        reader: &'a mut R,
    ) -> Result<Self, FactsError> {
        if evaluation.is_empty() || evaluation.len() > 256 {
            return Err(FactsError::Protocol("invalid evaluation token".into()));
        }
        Ok(Self {
            evaluation,
            ops,
            registry,
            reader,
        })
    }
    fn one<T: DeserializeOwned>(&mut self, op: Operation, query: Value) -> Result<T, FactsError> {
        if !self.ops.supports(op) {
            return Err(FactsError::Unsupported(format!(
                "unnegotiated optional operation: {}",
                op.name()
            )));
        }
        let result = self.reader.read_facts(self.evaluation, &[query])?;
        let facts = result["facts"]
            .as_array()
            .filter(|v| v.len() == 1)
            .ok_or_else(|| {
                FactsError::Protocol("host fact response cardinality mismatch".into())
            })?;
        serde_json::from_value(facts[0].clone())
            .map_err(|e| FactsError::Protocol(format!("malformed optional host fact: {e}")))
    }
    pub fn project_context(&mut self) -> Result<ProjectContext, FactsError> {
        let result: ProjectContext =
            self.one(Operation::ProjectContext, json!({"op":"projectContext"}))?;
        result.validate()?;
        Ok(result)
    }
    pub fn type_jvm(&mut self, target: &str) -> Result<TypeJvm, FactsError> {
        self.registry
            .require(target, HandleKind::Type)
            .map_err(FactsError::Protocol)?;
        let result: TypeJvm =
            self.one(Operation::TypeJvm, json!({"op":"typeJvm","target":target}))?;
        validate_type(&result, self.registry)?;
        Ok(result)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AbiMembers {
    pub internal_name: String,
    pub class_flags: u16,
    pub methods: Vec<MethodIndexEntry>,
    pub fields: Vec<FieldIndexEntry>,
}
fn unknown(reason: &str) -> FactsError {
    FactsError::Unknown(reason.into())
}
pub fn full_project_session() -> HostFact<mcdev_core::host_project::HostSessionIdentity> {
    HostFact::Unavailable{reason:"projectContext/typeJvm do not provide complete HostProjectSession identity or eight authoritative generations".into()}
}
pub fn executable_bytecode() -> Result<(), FactsError> {
    Err(FactsError::BytecodeUnavailable(
        "typeJvm header ABI does not provide Code, LVT or annotation payloads".into(),
    ))
}
pub fn abi_members(facts: &TypeJvm) -> Result<AbiMembers, FactsError> {
    let binary = facts
        .binary_name
        .found()
        .ok_or_else(|| unknown("authoritative JVM binary identity is unavailable"))?;
    if facts.jvm_abi_coverage.found().is_none() {
        return Err(unknown("complete JVM ABI coverage is unavailable"));
    }
    let abi = facts
        .abi
        .found()
        .ok_or_else(|| unknown("fresh raw JVM ABI is unavailable"))?;
    if abi.identity.origin_id.is_empty()
        || abi.identity.content_sha256.len() != 64
        || !abi
            .identity
            .content_sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
    {
        return Err(FactsError::Protocol(
            "raw ABI digest must be lowercase hex64 with a selected origin".into(),
        ));
    }
    if abi.identity.internal_name != binary.internal_name {
        return Err(FactsError::Protocol(
            "raw ABI owner differs from checked JVM identity".into(),
        ));
    }
    if facts
        .class_flags
        .found()
        .is_some_and(|flags| *flags != abi.access_flags)
    {
        return Err(FactsError::Protocol(
            "raw ABI and class flag facts disagree".into(),
        ));
    }
    let mut method_keys = BTreeSet::new();
    let mut methods = vec![];
    for raw in &abi.methods {
        if !method_keys.insert((&raw.name, &raw.descriptor)) {
            return Err(FactsError::Protocol(
                "duplicate exact JVM method identity".into(),
            ));
        }
        let descriptor = descriptor::parse_method_descriptor(&raw.descriptor)
            .map_err(|e| FactsError::Protocol(e.to_string()))?;
        if matches!(raw.name.as_str(), "<init>" | "<clinit>")
            && descriptor.return_type != JvmType::VoidType
        {
            return Err(FactsError::Protocol(
                "JVM initializer has non-void return".into(),
            ));
        }
        methods.push(MethodIndexEntry {
            name: raw.name.clone(),
            descriptor: raw.descriptor.clone(),
            is_static: raw.access_flags & 8 != 0,
            readable_signature: format!("{}{}", raw.name, descriptor::render_method(&descriptor)),
        });
    }
    let mut field_keys = BTreeSet::new();
    let mut fields = vec![];
    for raw in &abi.fields {
        if !field_keys.insert((&raw.name, &raw.descriptor)) {
            return Err(FactsError::Protocol(
                "duplicate exact JVM field identity".into(),
            ));
        }
        let descriptor = descriptor::parse_field_descriptor(&raw.descriptor)
            .map_err(|e| FactsError::Protocol(e.to_string()))?;
        fields.push(FieldIndexEntry {
            name: raw.name.clone(),
            descriptor: raw.descriptor.clone(),
            is_static: raw.access_flags & 8 != 0,
            readable_type: descriptor::render(&descriptor),
        });
    }
    Ok(AbiMembers {
        internal_name: binary.internal_name.clone(),
        class_flags: abi.access_flags,
        methods,
        fields,
    })
}
fn validate_type(facts: &TypeJvm, registry: &mut Registry) -> Result<(), FactsError> {
    for result in [
        facts.binary_name.validate(),
        facts.class_flags.validate(),
        facts.declared_members.coverage.validate(),
        facts.abi.validate(),
        facts.jvm_abi_coverage.validate(),
    ] {
        result.map_err(FactsError::Protocol)?;
    }
    if let Some(binary) = facts.binary_name.found() {
        let parsed = descriptor::parse_field_descriptor(&binary.descriptor)
            .map_err(|e| FactsError::Protocol(e.to_string()))?;
        if !matches!(parsed,JvmType::ObjectType(ref name)if name==&binary.internal_name) {
            return Err(FactsError::Protocol(
                "JVM binary name/descriptor mismatch".into(),
            ));
        }
    }
    let mut editing_ids = BTreeSet::new();
    for editing in &facts.declared_members.members {
        let member = &editing.member;
        if !editing_ids.insert(&member.id) {
            return Err(FactsError::Protocol(
                "duplicate editing member identity".into(),
            ));
        }
        registry
            .register(&member.id, HandleKind::Member)
            .map_err(FactsError::Protocol)?;
        for result in [
            member.signature.validate(),
            member.dispatch.validate(),
            editing.visibility.validate(),
            editing.synthetic.validate(),
        ] {
            result.map_err(FactsError::Protocol)?;
        }
        if let Some(signature) = member.signature.found() {
            registry
                .register(&signature.id, HandleKind::Signature)
                .map_err(FactsError::Protocol)?;
            if signature.kind != member.kind {
                return Err(FactsError::Protocol(
                    "editing member signature kind mismatch".into(),
                ));
            }
            match member.kind.as_str() {
                "method" => {
                    descriptor::parse_method_descriptor(&signature.descriptor)
                        .map_err(|e| FactsError::Protocol(e.to_string()))?;
                }
                "field" => {
                    descriptor::parse_field_descriptor(&signature.descriptor)
                        .map_err(|e| FactsError::Protocol(e.to_string()))?;
                }
                _ => return Err(FactsError::Protocol("unknown editing member kind".into())),
            }
        }
        if member
            .dispatch
            .found()
            .is_some_and(|v| v != "instance" && v != "static")
        {
            return Err(FactsError::Protocol("unknown dispatch value".into()));
        }
    }
    Ok(())
}
