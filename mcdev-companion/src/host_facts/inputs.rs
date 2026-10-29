//! Evaluation-scoped readers for the public ktlsp host-input operations.
use super::{bridge::FactReader, draft::RawAbi, FactsError};
use crate::wire::{Fact, MAX_BATCH};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::{AtomicU64, Ordering},
};

const RESPONSE_BUDGET: &str = "extension response batch too large; split queries";
const MAX_TOKEN: usize = 4096;
const MAX_CLASS_BYTES: usize = 16 * 1024 * 1024;
const MAX_RESOURCE_BYTES: usize = 64 * 1024 * 1024;
const MAX_CHUNK: usize = 512 * 1024;
static NEXT_SCOPE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InputOp {
    SourceInventory,
    SourceSnapshot,
    ResourceInventory,
    ResourceLookup,
    ResourceSnapshot,
    RuntimeClass,
    ClassBytes,
    JvmCatalog,
    JvmClass,
}

impl InputOp {
    pub const ALL: [Self; 9] = [
        Self::SourceInventory,
        Self::SourceSnapshot,
        Self::ResourceInventory,
        Self::ResourceLookup,
        Self::ResourceSnapshot,
        Self::RuntimeClass,
        Self::ClassBytes,
        Self::JvmCatalog,
        Self::JvmClass,
    ];

    pub fn capability(self) -> &'static str {
        match self {
            Self::SourceInventory | Self::SourceSnapshot => "source-snapshots-v1",
            Self::ResourceInventory | Self::ResourceLookup | Self::ResourceSnapshot => {
                "owned-resources-v1"
            }
            Self::RuntimeClass | Self::ClassBytes => "runtime-class-bytes-v1",
            Self::JvmCatalog | Self::JvmClass => "jvm-catalog-v1",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::SourceInventory => "sourceInventory",
            Self::SourceSnapshot => "sourceSnapshot",
            Self::ResourceInventory => "resourceInventory",
            Self::ResourceLookup => "resourceLookup",
            Self::ResourceSnapshot => "resourceSnapshot",
            Self::RuntimeClass => "runtimeClass",
            Self::ClassBytes => "classBytes",
            Self::JvmCatalog => "jvmCatalog",
            Self::JvmClass => "jvmClass",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputOps(BTreeSet<InputOp>);

impl InputOps {
    pub fn intersect(host: &Value) -> Result<Self, FactsError> {
        let Some(capabilities) = host.get("capabilities") else {
            return Ok(Self::default());
        };
        let capabilities = capabilities
            .as_array()
            .ok_or_else(|| protocol("capabilities must be an array"))?;
        if capabilities
            .iter()
            .any(|capability| !capability.is_string())
        {
            return Err(protocol("capability names must be strings"));
        }
        let Some(map) = host.get("supportedOps") else {
            return Ok(Self::default());
        };
        let map = map
            .as_object()
            .ok_or_else(|| protocol("supportedOps must be an object"))?;
        let mut accepted = BTreeSet::new();
        for op in InputOp::ALL {
            if !capabilities.iter().any(|cap| cap == op.capability()) {
                continue;
            }
            let Some(names) = map.get(op.capability()) else {
                continue;
            };
            let names = names
                .as_array()
                .ok_or_else(|| protocol("optional operation set must be an array"))?;
            if names.iter().any(|name| !name.is_string()) {
                return Err(protocol("optional operation names must be strings"));
            }
            if names.iter().any(|name| name == op.name()) {
                accepted.insert(op);
            }
        }
        Ok(Self(accepted))
    }

    pub fn supports(&self, operation: InputOp) -> bool {
        self.0.contains(&operation)
    }

    pub fn accepted(&self) -> BTreeMap<&'static str, Vec<&'static str>> {
        let mut result = BTreeMap::<_, Vec<_>>::new();
        for op in &self.0 {
            result.entry(op.capability()).or_default().push(op.name());
        }
        result
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputIdentity {
    pub kind: String,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceDescriptor {
    pub id: String,
    pub display_path: String,
    pub document_uri: Fact<String>,
    pub language_id: String,
    pub identity: InputIdentity,
    #[serde(default)]
    pub document_version: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct SourceEntry {
    pub descriptor: SourceDescriptor,
    scope: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSnapshot {
    #[serde(flatten)]
    pub descriptor: SourceDescriptor,
    pub text: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResourceAssociation {
    Source,
    Output,
    CompileClasspath,
    RuntimeClasspath,
    Configuration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceDescriptor {
    pub id: String,
    pub relative_path: String,
    pub association: ResourceAssociation,
    pub classpath_order: usize,
    pub editable: bool,
    pub document_uri: Fact<String>,
    #[serde(default)]
    pub document_version: Option<i64>,
    pub identity: InputIdentity,
}

#[derive(Debug, Clone)]
pub struct ResourceEntry {
    pub descriptor: ResourceDescriptor,
    scope: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceContentIdentity {
    pub kind: String,
    pub revision: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceContent {
    pub identity: ResourceContentIdentity,
    pub document_version: Option<i64>,
    pub bytes: Vec<u8>,
    pub utf8: bool,
    pub text: Fact<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeAuthority {
    RuntimeBytecode,
    SdkApiHeader,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hierarchy {
    pub super_name: Option<String>,
    pub interfaces: Vec<String>,
    pub access_flags: u16,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDescriptor {
    pub id: String,
    pub internal_name: String,
    pub origin_id: String,
    pub content_sha256: String,
    pub byte_length: usize,
    pub authority: RuntimeAuthority,
    pub code_available: bool,
    pub runtime_only: bool,
    pub class_header: Fact<RawAbi>,
    pub hierarchy: Fact<Hierarchy>,
    pub runtime_abi_coverage: Fact<Value>,
    pub source_current: Fact<Value>,
}

#[derive(Debug, Clone)]
pub struct RuntimeClass {
    pub descriptor: RuntimeDescriptor,
    scope: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CatalogAssociation {
    Output,
    CompileClasspath,
    Sdk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CatalogAuthority {
    CompiledInput,
    SdkRuntimeInput,
    SdkApiHeader,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDescriptor {
    pub entry: String,
    pub internal_name: String,
    pub origin_id: String,
    pub association: CatalogAssociation,
    pub classpath_order: usize,
    pub authority: CatalogAuthority,
}

#[derive(Debug, Clone)]
pub struct CatalogEntry {
    pub descriptor: CatalogDescriptor,
    scope: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogClass {
    pub entry: String,
    pub authority: CatalogAuthority,
    pub abi: RawAbi,
    pub source_current: Fact<Value>,
}

#[derive(Debug, Clone)]
pub struct Inventory<T> {
    pub entries: Vec<T>,
    /// The host's terminal coverage fact, including its exact Found value.
    pub coverage: Fact<Value>,
}

#[derive(Debug, Clone)]
pub struct JvmInventory {
    pub entries: Vec<CatalogEntry>,
    pub coverage: Fact<Value>,
    pub problems: Vec<String>,
    pub source_current: Fact<Value>,
}

#[derive(Deserialize)]
struct SourcePage {
    sources: Vec<SourceDescriptor>,
    next: Option<String>,
    coverage: Fact<Value>,
}

#[derive(Deserialize)]
struct ResourcePage {
    resources: Vec<ResourceDescriptor>,
    next: Option<String>,
    coverage: Fact<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogPage {
    view: String,
    classes: Vec<CatalogDescriptor>,
    next: Option<String>,
    coverage: Fact<Value>,
    problems: Vec<String>,
    source_current: Fact<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResourceChunk {
    identity: ResourceContentIdentity,
    document_version: Option<i64>,
    size: usize,
    offset: usize,
    base64: String,
    utf8: bool,
    text: Fact<String>,
    next: Option<String>,
}

#[derive(Default)]
struct ResourceAssembly {
    after: Option<String>,
    seen: BTreeSet<String>,
    bytes: Vec<u8>,
    first: Option<ResourceChunk>,
    result: Option<Fact<ResourceContent>>,
}

impl ResourceAssembly {
    fn push(&mut self, entry: &ResourceEntry, fact: Fact<ResourceChunk>) -> Result<(), FactsError> {
        validate_fact(&fact)?;
        let chunk = match fact {
            Fact::Found { value } => value,
            Fact::Unknown { reasons } => {
                self.result = Some(Fact::Unknown { reasons });
                return Ok(());
            }
            Fact::DefinitelyAbsent if self.first.is_none() => {
                self.result = Some(Fact::DefinitelyAbsent);
                return Ok(());
            }
            Fact::DefinitelyAbsent => return Err(protocol("resource vanished during byte stream")),
        };
        validate_fact(&chunk.text)?;
        if chunk.identity.kind != "resource"
            || chunk.identity.revision != entry.descriptor.identity.revision
            || !valid_digest(&chunk.identity.sha256)
            || chunk.document_version != entry.descriptor.document_version
            || chunk.size > MAX_RESOURCE_BYTES
            || chunk.offset != self.bytes.len()
        {
            return Err(protocol(
                "resource stream identity, length or version mismatch",
            ));
        }
        if let Some(previous) = &self.first {
            if chunk.identity != previous.identity
                || chunk.document_version != previous.document_version
                || chunk.size != previous.size
                || chunk.utf8 != previous.utf8
                || chunk.text != previous.text
            {
                return Err(protocol("resource stream changed between chunks"));
            }
        }
        let decoded = decode_base64(&chunk.base64)?;
        if decoded.len() > MAX_CHUNK
            || self
                .bytes
                .len()
                .checked_add(decoded.len())
                .is_none_or(|end| end > chunk.size)
            || (chunk.next.is_some()
                && (decoded.is_empty() || self.bytes.len() + decoded.len() >= chunk.size))
        {
            return Err(protocol("invalid resource stream chunk"));
        }
        self.bytes.extend_from_slice(&decoded);
        self.after = next_cursor(chunk.next.clone(), &mut self.seen)?;
        if self.first.is_none() {
            self.first = Some(chunk);
        }
        if self.after.is_some() {
            return Ok(());
        }
        let first = self
            .first
            .take()
            .expect("a completed stream has a first chunk");
        if self.bytes.len() != first.size || !digest_matches(&self.bytes, &first.identity.sha256) {
            return Err(protocol("resource content length or digest mismatch"));
        }
        let actual_text = std::str::from_utf8(&self.bytes).ok();
        if first.utf8 != actual_text.is_some() {
            return Err(protocol("resource UTF-8 authority disagrees with bytes"));
        }
        match (&first.text, actual_text) {
            (Fact::Found { value }, Some(actual)) if value == actual => (),
            (Fact::Unknown { .. }, Some(_)) | (Fact::DefinitelyAbsent, None) => (),
            _ => return Err(protocol("resource text fact disagrees with bytes")),
        }
        self.result = Some(Fact::Found {
            value: ResourceContent {
                identity: first.identity,
                document_version: first.document_version,
                bytes: std::mem::take(&mut self.bytes),
                utf8: first.utf8,
                text: first.text,
            },
        });
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClassChunk {
    class_identity: String,
    offset: usize,
    byte_length: usize,
    bytes_base64: String,
    next: Option<String>,
}

fn protocol(message: impl Into<String>) -> FactsError {
    FactsError::Protocol(message.into())
}

fn decode<T: DeserializeOwned>(value: Value) -> Result<T, FactsError> {
    serde_json::from_value(value)
        .map_err(|error| protocol(format!("malformed host input: {error}")))
}

fn validate_fact<T>(fact: &Fact<T>) -> Result<(), FactsError> {
    fact.validate().map_err(protocol)
}

fn digest_matches(bytes: &[u8], expected: &str) -> bool {
    valid_digest(expected) && format!("{:x}", Sha256::digest(bytes)) == expected
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_token(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_TOKEN && !value.chars().any(char::is_control)
}

fn valid_identity(identity: &InputIdentity, kind: &str) -> bool {
    identity.kind == kind && valid_token(&identity.revision)
}

fn valid_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':'])
        && !path.chars().any(char::is_control)
        && path.split('/').all(|part| !matches!(part, "" | "." | ".."))
}

fn valid_jvm_name(name: &str, prefix: bool) -> bool {
    name.len() <= MAX_TOKEN
        && (prefix || !name.is_empty())
        && !name.starts_with('/')
        && !name.contains("//")
        && (prefix || !name.ends_with('/'))
        && !name.contains(['\\', '.', ':', ';', '[', ']'])
        && !name.chars().any(char::is_control)
}

fn check_coverage(
    fact: &Fact<Value>,
    next: &Option<String>,
    expected: Value,
) -> Result<(), FactsError> {
    validate_fact(fact)?;
    if next.is_some() && !matches!(fact, Fact::Unknown { .. }) {
        return Err(protocol("nonterminal inventory claims complete coverage"));
    }
    match fact {
        Fact::Found { value } if *value == expected => Ok(()),
        Fact::Unknown { .. } => Ok(()),
        _ => Err(protocol("invalid inventory coverage fact")),
    }
}

fn next_cursor(
    next: Option<String>,
    seen: &mut BTreeSet<String>,
) -> Result<Option<String>, FactsError> {
    if let Some(value) = &next {
        if !valid_token(value) || !seen.insert(value.clone()) {
            return Err(protocol("invalid or repeated host continuation"));
        }
    }
    Ok(next)
}

pub(crate) fn read_split<R: FactReader>(
    reader: &mut R,
    evaluation: &str,
    queries: &[Value],
) -> Result<Vec<Value>, FactsError> {
    if queries.is_empty() {
        return Ok(Vec::new());
    }
    if queries.len() > MAX_BATCH {
        let mut values = Vec::with_capacity(queries.len());
        for batch in queries.chunks(MAX_BATCH) {
            values.extend(read_split(reader, evaluation, batch)?);
        }
        return Ok(values);
    }
    let result = match reader.read_facts(evaluation, queries) {
        Ok(result) => result,
        Err(FactsError::Protocol(message))
            if queries.len() > 1 && message.contains(RESPONSE_BUDGET) =>
        {
            let middle = queries.len() / 2;
            let mut first = read_split(reader, evaluation, &queries[..middle])?;
            first.extend(read_split(reader, evaluation, &queries[middle..])?);
            return Ok(first);
        }
        Err(error) => return Err(error),
    };
    let facts = result["facts"]
        .as_array()
        .filter(|facts| facts.len() == queries.len())
        .ok_or_else(|| protocol("host fact response cardinality mismatch"))?;
    Ok(facts.clone())
}

/// Owns the local association between host-issued handles and one live evaluation.
pub struct ScopedInputs<R: FactReader> {
    evaluation: String,
    operations: InputOps,
    reader: R,
    scope: u64,
    sources: BTreeMap<String, SourceDescriptor>,
    resources: BTreeMap<String, ResourceDescriptor>,
    runtime_classes: BTreeMap<String, RuntimeDescriptor>,
    catalog_entries: BTreeMap<String, CatalogDescriptor>,
}

impl<T: FactReader + ?Sized> FactReader for &mut T {
    fn read_facts(&mut self, evaluation: &str, queries: &[Value]) -> Result<Value, FactsError> {
        (**self).read_facts(evaluation, queries)
    }
}

impl<R: FactReader> ScopedInputs<R> {
    pub fn new(evaluation: &str, operations: &InputOps, reader: R) -> Result<Self, FactsError> {
        if !valid_token(evaluation) || evaluation.len() > 256 {
            return Err(protocol("invalid evaluation token"));
        }
        let scope = NEXT_SCOPE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| protocol("local scope sequence exhausted"))?;
        Ok(Self {
            evaluation: evaluation.to_owned(),
            operations: operations.clone(),
            reader,
            scope,
            sources: BTreeMap::new(),
            resources: BTreeMap::new(),
            runtime_classes: BTreeMap::new(),
            catalog_entries: BTreeMap::new(),
        })
    }

    fn read(&mut self, op: InputOp, queries: &[Value]) -> Result<Vec<Value>, FactsError> {
        if !self.operations.supports(op) {
            return Err(FactsError::Unsupported(format!(
                "unnegotiated optional operation: {}",
                op.name()
            )));
        }
        read_split(&mut self.reader, &self.evaluation, queries)
    }

    fn one<T: DeserializeOwned>(&mut self, op: InputOp, query: Value) -> Result<T, FactsError> {
        let mut result = self.read(op, &[query])?;
        decode(result.remove(0))
    }

    pub fn source_inventory(&mut self) -> Result<Inventory<SourceEntry>, FactsError> {
        let mut entries = Vec::new();
        let mut after = None;
        let mut seen = BTreeSet::new();
        let mut issued = BTreeSet::new();
        loop {
            let page: SourcePage = self.one(
                InputOp::SourceInventory,
                json!({"op":"sourceInventory","after":after}),
            )?;
            check_coverage(&page.coverage, &page.next, Value::Null)?;
            if page.next.is_some() && page.sources.is_empty() {
                return Err(protocol("nonterminal source page made no progress"));
            }
            for source in page.sources {
                validate_source(&source)?;
                if !issued.insert(source.id.clone()) {
                    return Err(protocol("duplicate source identity in inventory"));
                }
                insert_consistent(&mut self.sources, &source.id, &source)?;
                entries.push(SourceEntry {
                    descriptor: source,
                    scope: self.scope,
                });
            }
            after = next_cursor(page.next, &mut seen)?;
            if after.is_none() {
                return Ok(Inventory {
                    entries,
                    coverage: page.coverage,
                });
            }
        }
    }

    pub fn source_snapshots(
        &mut self,
        entries: &[SourceEntry],
    ) -> Result<Vec<Fact<SourceSnapshot>>, FactsError> {
        for entry in entries {
            check_entry(
                entry.scope,
                self.scope,
                &entry.descriptor.id,
                &entry.descriptor,
                &self.sources,
            )?;
        }
        let queries: Vec<_> = entries
            .iter()
            .map(|source| json!({"op":"sourceSnapshot","source":source.descriptor.id}))
            .collect();
        let values = self.read(InputOp::SourceSnapshot, &queries)?;
        values
            .into_iter()
            .zip(entries)
            .map(|(value, entry)| {
                let fact: Fact<SourceSnapshot> = decode(value)?;
                validate_fact(&fact)?;
                if let Fact::Found { value } = &fact {
                    if value.descriptor != entry.descriptor
                        || !digest_matches(value.text.as_bytes(), &value.sha256)
                    {
                        return Err(protocol("source snapshot identity or digest mismatch"));
                    }
                }
                Ok(fact)
            })
            .collect()
    }

    pub fn resource_inventory(&mut self) -> Result<Inventory<ResourceEntry>, FactsError> {
        let mut entries = Vec::new();
        let mut after = None;
        let mut seen = BTreeSet::new();
        let mut issued = BTreeSet::new();
        loop {
            let page: ResourcePage = self.one(
                InputOp::ResourceInventory,
                json!({"op":"resourceInventory","after":after}),
            )?;
            check_coverage(&page.coverage, &page.next, json!(true))?;
            if page.next.is_some() && page.resources.is_empty() {
                return Err(protocol("nonterminal resource page made no progress"));
            }
            for descriptor in page.resources {
                if !issued.insert(descriptor.id.clone()) {
                    return Err(protocol("duplicate resource identity in inventory"));
                }
                entries.push(self.resource_entry(descriptor)?);
            }
            after = next_cursor(page.next, &mut seen)?;
            if after.is_none() {
                return Ok(Inventory {
                    entries,
                    coverage: page.coverage,
                });
            }
        }
    }

    pub fn resource_lookup(
        &mut self,
        relative_path: &str,
        association: Option<ResourceAssociation>,
    ) -> Result<Fact<Vec<ResourceEntry>>, FactsError> {
        if !valid_relative_path(relative_path) {
            return Err(protocol("invalid resource relative path"));
        }
        let fact: Fact<Vec<ResourceDescriptor>> = self.one(
            InputOp::ResourceLookup,
            json!({"op":"resourceLookup","relativePath":relative_path,"association":association}),
        )?;
        validate_fact(&fact)?;
        match fact {
            Fact::Found { value } => {
                if value.is_empty()
                    || value.iter().any(|entry| {
                        entry.relative_path != relative_path
                            || association.is_some_and(|kind| kind != entry.association)
                    })
                {
                    return Err(protocol(
                        "resource lookup returned different or empty matches",
                    ));
                }
                Ok(Fact::Found {
                    value: value
                        .into_iter()
                        .map(|entry| self.resource_entry(entry))
                        .collect::<Result<_, _>>()?,
                })
            }
            Fact::Unknown { reasons } => Ok(Fact::Unknown { reasons }),
            Fact::DefinitelyAbsent => Ok(Fact::DefinitelyAbsent),
        }
    }

    fn resource_entry(
        &mut self,
        descriptor: ResourceDescriptor,
    ) -> Result<ResourceEntry, FactsError> {
        validate_resource(&descriptor)?;
        insert_consistent(&mut self.resources, &descriptor.id, &descriptor)?;
        Ok(ResourceEntry {
            descriptor,
            scope: self.scope,
        })
    }

    pub fn resource_snapshot(
        &mut self,
        entry: &ResourceEntry,
    ) -> Result<Fact<ResourceContent>, FactsError> {
        Ok(self
            .resource_snapshots(std::slice::from_ref(entry))?
            .remove(0))
    }

    pub fn resource_snapshots(
        &mut self,
        entries: &[ResourceEntry],
    ) -> Result<Vec<Fact<ResourceContent>>, FactsError> {
        for entry in entries {
            check_entry(
                entry.scope,
                self.scope,
                &entry.descriptor.id,
                &entry.descriptor,
                &self.resources,
            )?;
        }
        if entries.is_empty() {
            self.read(InputOp::ResourceSnapshot, &[])?;
            return Ok(Vec::new());
        }
        let mut states: Vec<_> = entries
            .iter()
            .map(|_| ResourceAssembly::default())
            .collect();
        loop {
            let pending: Vec<_> = states
                .iter()
                .enumerate()
                .filter_map(|(index, state)| state.result.is_none().then_some(index))
                .collect();
            if pending.is_empty() {
                return Ok(states
                    .into_iter()
                    .map(|state| state.result.expect("completed resource"))
                    .collect());
            }
            let queries: Vec<_> = pending.iter().map(|&index| json!({
                "op":"resourceSnapshot", "resource":entries[index].descriptor.id, "after":states[index].after
            })).collect();
            let values = self.read(InputOp::ResourceSnapshot, &queries)?;
            for (index, value) in pending.into_iter().zip(values) {
                states[index].push(&entries[index], decode(value)?)?;
            }
        }
    }

    pub fn runtime_class(&mut self, internal_name: &str) -> Result<Fact<RuntimeClass>, FactsError> {
        if !valid_jvm_name(internal_name, false) {
            return Err(protocol("invalid JVM internal name"));
        }
        let fact: Fact<RuntimeDescriptor> = self.one(
            InputOp::RuntimeClass,
            json!({"op":"runtimeClass","internalName":internal_name}),
        )?;
        validate_fact(&fact)?;
        match fact {
            Fact::Found { value } => {
                validate_runtime(&value, internal_name)?;
                insert_consistent(&mut self.runtime_classes, &value.id, &value)?;
                Ok(Fact::Found {
                    value: RuntimeClass {
                        descriptor: value,
                        scope: self.scope,
                    },
                })
            }
            Fact::Unknown { reasons } => Ok(Fact::Unknown { reasons }),
            Fact::DefinitelyAbsent => Err(protocol("runtime lookup cannot certify absence")),
        }
    }

    pub fn class_bytes(&mut self, class: &RuntimeClass) -> Result<Fact<Vec<u8>>, FactsError> {
        check_entry(
            class.scope,
            self.scope,
            &class.descriptor.id,
            &class.descriptor,
            &self.runtime_classes,
        )?;
        let mut after = None;
        let mut seen = BTreeSet::new();
        let mut bytes = Vec::new();
        loop {
            let fact: Fact<ClassChunk> = self.one(
                InputOp::ClassBytes,
                json!({"op":"classBytes","classIdentity":class.descriptor.id,"after":after}),
            )?;
            validate_fact(&fact)?;
            let chunk = match fact {
                Fact::Found { value } => value,
                Fact::Unknown { reasons } => return Ok(Fact::Unknown { reasons }),
                Fact::DefinitelyAbsent => {
                    return Err(protocol("class bytes cannot certify absence"))
                }
            };
            if class.descriptor.authority != RuntimeAuthority::RuntimeBytecode
                || !class.descriptor.code_available
                || chunk.class_identity != class.descriptor.id
                || chunk.byte_length != class.descriptor.byte_length
                || chunk.byte_length > MAX_CLASS_BYTES
                || chunk.offset != bytes.len()
            {
                return Err(protocol(
                    "class stream identity, authority or length mismatch",
                ));
            }
            let decoded = decode_base64(&chunk.bytes_base64)?;
            if decoded.is_empty()
                || decoded.len() > 256 * 1024
                || bytes
                    .len()
                    .checked_add(decoded.len())
                    .is_none_or(|end| end > chunk.byte_length)
                || (chunk.next.is_some() && bytes.len() + decoded.len() >= chunk.byte_length)
            {
                return Err(protocol("invalid class stream chunk"));
            }
            bytes.extend_from_slice(&decoded);
            after = next_cursor(chunk.next, &mut seen)?;
            if after.is_none() {
                break;
            }
        }
        if bytes.len() != class.descriptor.byte_length
            || !digest_matches(&bytes, &class.descriptor.content_sha256)
        {
            return Err(protocol("class content length or digest mismatch"));
        }
        Ok(Fact::Found { value: bytes })
    }

    pub fn jvm_catalog(&mut self, prefix: &str) -> Result<JvmInventory, FactsError> {
        if !valid_jvm_name(prefix, true) {
            return Err(protocol("invalid JVM catalog prefix"));
        }
        let mut entries = Vec::new();
        let mut after = None;
        let mut seen = BTreeSet::new();
        let mut issued = BTreeSet::new();
        let mut first: Option<(Vec<String>, Fact<Value>)> = None;
        loop {
            let page: CatalogPage = self.one(
                InputOp::JvmCatalog,
                json!({"op":"jvmCatalog","prefix":prefix,"after":after}),
            )?;
            check_coverage(&page.coverage, &page.next, Value::Null)?;
            validate_fact(&page.source_current)?;
            if page.view != "selectedPhysicalJvmInputs"
                || !matches!(page.source_current, Fact::Unknown { .. })
            {
                return Err(protocol("invalid JVM catalog view or source authority"));
            }
            if page.next.is_some() && page.classes.is_empty() {
                return Err(protocol("nonterminal JVM catalog page made no progress"));
            }
            if let Some((problems, source_current)) = &first {
                if problems != &page.problems || source_current != &page.source_current {
                    return Err(protocol("JVM catalog changed between pages"));
                }
            } else {
                first = Some((page.problems.clone(), page.source_current.clone()));
            }
            for descriptor in page.classes {
                if !valid_token(&descriptor.entry)
                    || !valid_token(&descriptor.origin_id)
                    || !valid_jvm_name(&descriptor.internal_name, false)
                    || !descriptor.internal_name.starts_with(prefix)
                {
                    return Err(protocol("invalid JVM catalog entry"));
                }
                if !issued.insert(descriptor.entry.clone()) {
                    return Err(protocol("duplicate JVM catalog entry identity"));
                }
                insert_consistent(&mut self.catalog_entries, &descriptor.entry, &descriptor)?;
                entries.push(CatalogEntry {
                    descriptor,
                    scope: self.scope,
                });
            }
            after = next_cursor(page.next, &mut seen)?;
            if after.is_none() {
                let (problems, source_current) = first.expect("catalog has a first page");
                return Ok(JvmInventory {
                    entries,
                    coverage: page.coverage,
                    problems,
                    source_current,
                });
            }
        }
    }

    pub fn jvm_classes(
        &mut self,
        entries: &[CatalogEntry],
    ) -> Result<Vec<Fact<CatalogClass>>, FactsError> {
        for entry in entries {
            check_entry(
                entry.scope,
                self.scope,
                &entry.descriptor.entry,
                &entry.descriptor,
                &self.catalog_entries,
            )?;
        }
        let queries: Vec<_> = entries
            .iter()
            .map(|entry| json!({"op":"jvmClass","entry":entry.descriptor.entry}))
            .collect();
        let values = self.read(InputOp::JvmClass, &queries)?;
        values
            .into_iter()
            .zip(entries)
            .map(|(value, entry)| {
                let fact: Fact<CatalogClass> = decode(value)?;
                validate_fact(&fact)?;
                if let Fact::Found { value } = &fact {
                    validate_fact(&value.source_current)?;
                    if value.entry != entry.descriptor.entry
                        || value.authority != entry.descriptor.authority
                        || value.abi.identity.origin_id != entry.descriptor.origin_id
                        || value.abi.identity.internal_name != entry.descriptor.internal_name
                        || !valid_digest(&value.abi.identity.content_sha256)
                        || !matches!(value.source_current, Fact::Unknown { .. })
                        || (value.authority == CatalogAuthority::SdkApiHeader
                            && has_code(&value.abi))
                    {
                        return Err(protocol("catalog class identity or authority mismatch"));
                    }
                }
                Ok(fact)
            })
            .collect()
    }
}

fn validate_source(source: &SourceDescriptor) -> Result<(), FactsError> {
    validate_fact(&source.document_uri)?;
    if !valid_token(&source.id)
        || !valid_identity(&source.identity, "source")
        || !matches!(source.language_id.as_str(), "java" | "kotlin")
    {
        return Err(protocol("invalid source descriptor"));
    }
    Ok(())
}

fn validate_resource(resource: &ResourceDescriptor) -> Result<(), FactsError> {
    validate_fact(&resource.document_uri)?;
    if !valid_token(&resource.id)
        || !valid_identity(&resource.identity, "resource")
        || !valid_relative_path(&resource.relative_path)
    {
        return Err(protocol("invalid resource descriptor"));
    }
    Ok(())
}

fn validate_runtime(class: &RuntimeDescriptor, requested: &str) -> Result<(), FactsError> {
    validate_fact(&class.class_header)?;
    validate_fact(&class.hierarchy)?;
    validate_fact(&class.runtime_abi_coverage)?;
    validate_fact(&class.source_current)?;
    if !valid_token(&class.id)
        || !valid_token(&class.origin_id)
        || class.internal_name != requested
        || !valid_digest(&class.content_sha256)
        || class.byte_length > MAX_CLASS_BYTES
        || !matches!(class.source_current, Fact::Unknown { .. })
    {
        return Err(protocol("invalid runtime class identity"));
    }
    match class.authority {
        RuntimeAuthority::RuntimeBytecode
            if class.code_available
                && class.runtime_only
                && class.byte_length > 0
                && class.runtime_abi_coverage == (Fact::Found { value: Value::Null }) =>
        {
            ()
        }
        RuntimeAuthority::SdkApiHeader
            if !class.code_available
                && !class.runtime_only
                && class.byte_length == 0
                && matches!(class.runtime_abi_coverage, Fact::Unknown { .. }) =>
        {
            ()
        }
        _ => return Err(protocol("runtime class authority mismatch")),
    }
    if let Fact::Found { value: header } = &class.class_header {
        if class.authority != RuntimeAuthority::SdkApiHeader
            || header.identity.internal_name != class.internal_name
            || header.identity.origin_id != class.origin_id
            || header.identity.content_sha256 != class.content_sha256
            || has_code(header)
        {
            return Err(protocol("runtime SDK header identity mismatch"));
        }
    }
    Ok(())
}

fn has_code(abi: &RawAbi) -> bool {
    abi.methods
        .iter()
        .any(|method| method.attribute_names.iter().any(|name| name == "Code"))
}

fn insert_consistent<T: Clone + PartialEq>(
    map: &mut BTreeMap<String, T>,
    key: &str,
    value: &T,
) -> Result<(), FactsError> {
    if map.get(key).is_some_and(|existing| existing != value) {
        return Err(protocol("host reused an identity for different inputs"));
    }
    map.insert(key.to_owned(), value.clone());
    Ok(())
}

fn check_entry<T: PartialEq>(
    issued_scope: u64,
    current_scope: u64,
    id: &str,
    value: &T,
    map: &BTreeMap<String, T>,
) -> Result<(), FactsError> {
    if issued_scope != current_scope || map.get(id) != Some(value) {
        return Err(protocol("foreign or modified host input handle"));
    }
    Ok(())
}

fn decode_base64(value: &str) -> Result<Vec<u8>, FactsError> {
    fn sextet(byte: u8) -> Option<u8> {
        match byte {
            b'A'..=b'Z' => Some(byte - b'A'),
            b'a'..=b'z' => Some(byte - b'a' + 26),
            b'0'..=b'9' => Some(byte - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let input = value.as_bytes();
    if input.len() % 4 != 0 {
        return Err(protocol("invalid base64 length"));
    }
    let mut output = Vec::with_capacity(input.len() / 4 * 3);
    for (index, chunk) in input.chunks_exact(4).enumerate() {
        let final_chunk = index + 1 == input.len() / 4;
        let a = sextet(chunk[0]).ok_or_else(|| protocol("invalid base64 character"))?;
        let b = sextet(chunk[1]).ok_or_else(|| protocol("invalid base64 character"))?;
        let c = if chunk[2] == b'=' {
            None
        } else {
            Some(sextet(chunk[2]).ok_or_else(|| protocol("invalid base64 character"))?)
        };
        let d = if chunk[3] == b'=' {
            None
        } else {
            Some(sextet(chunk[3]).ok_or_else(|| protocol("invalid base64 character"))?)
        };
        match (c, d) {
            (None, None) if final_chunk && b & 15 == 0 => output.push((a << 2) | (b >> 4)),
            (Some(c), None) if final_chunk && c & 3 == 0 => {
                output.extend_from_slice(&[(a << 2) | (b >> 4), (b << 4) | (c >> 2)]);
            }
            (Some(c), Some(d)) => {
                output.extend_from_slice(&[(a << 2) | (b >> 4), (b << 4) | (c >> 2), (c << 6) | d])
            }
            _ => return Err(protocol("invalid base64 padding")),
        }
    }
    Ok(output)
}
