use crate::model::{
    ClassIndex, ClassIndexEntry, FieldIndexEntry, MappingNamespace, MethodIndexEntry,
};
use noak::reader::{
    attributes::{Code, RawInstruction},
    cpool, Class,
};
use noak::AccessFlags;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConstantValue {
    StringValue(String),
    /// A JVM string containing an isolated UTF-16 surrogate. Rust `String` cannot
    /// represent this value; retaining its code units avoids replacing the constant.
    StringUtf16Value(Vec<u16>),
    IntValue(i32),
    LongValue(i64),
    FloatValue(f32),
    DoubleValue(f64),
    ClassLiteral(String),
    NullValue,
}
impl PartialEq for ConstantValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::StringValue(left), Self::StringValue(right))
            | (Self::ClassLiteral(left), Self::ClassLiteral(right)) => left == right,
            (Self::StringUtf16Value(left), Self::StringUtf16Value(right)) => left == right,
            (Self::StringValue(left), Self::StringUtf16Value(right))
            | (Self::StringUtf16Value(right), Self::StringValue(left)) => {
                left.encode_utf16().eq(right.iter().copied())
            }
            (Self::IntValue(left), Self::IntValue(right)) => left == right,
            (Self::LongValue(left), Self::LongValue(right)) => left == right,
            // Kotlin data classes use Float.compare/Double.compare: NaNs match and signed zeros differ.
            (Self::FloatValue(left), Self::FloatValue(right)) => {
                (left.is_nan() && right.is_nan()) || left.to_bits() == right.to_bits()
            }
            (Self::DoubleValue(left), Self::DoubleValue(right)) => {
                (left.is_nan() && right.is_nan()) || left.to_bits() == right.to_bits()
            }
            (Self::NullValue, Self::NullValue) => true,
            _ => false,
        }
    }
}
impl Eq for ConstantValue {}
impl std::hash::Hash for ConstantValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::StringValue(value) => {
                0u8.hash(state);
                value.encode_utf16().collect::<Vec<_>>().hash(state);
            }
            Self::StringUtf16Value(value) => {
                0u8.hash(state);
                value.hash(state);
            }
            Self::IntValue(value) => {
                1u8.hash(state);
                value.hash(state);
            }
            Self::LongValue(value) => {
                2u8.hash(state);
                value.hash(state);
            }
            Self::FloatValue(value) => {
                3u8.hash(state);
                if value.is_nan() {
                    f32::NAN.to_bits()
                } else {
                    value.to_bits()
                }
                .hash(state);
            }
            Self::DoubleValue(value) => {
                4u8.hash(state);
                if value.is_nan() {
                    f64::NAN.to_bits()
                } else {
                    value.to_bits()
                }
                .hash(state);
            }
            Self::ClassLiteral(value) => {
                5u8.hash(state);
                value.hash(state);
            }
            Self::NullValue => 6u8.hash(state),
        }
    }
}
impl ConstantValue {
    pub fn from_utf16(units: Vec<u16>) -> Self {
        match String::from_utf16(&units) {
            Ok(value) => Self::StringValue(value),
            Err(_) => Self::StringUtf16Value(units),
        }
    }
    pub fn string_utf16(&self) -> Option<Vec<u16>> {
        match self {
            Self::StringValue(value) => Some(value.encode_utf16().collect()),
            Self::StringUtf16Value(value) => Some(value.clone()),
            _ => None,
        }
    }
    pub fn java_string_literal(&self) -> Option<String> {
        self.string_utf16()
            .map(|units| java_string_literal_utf16(&units))
    }
}
pub fn java_string_literal_utf16(units: &[u16]) -> String {
    let mut literal = String::from("\"");
    for unit in char::decode_utf16(units.iter().copied()) {
        match unit {
            Err(unit) => literal.push_str(&format!("\\u{:04X}", unit.unpaired_surrogate())),
            Ok('"') => literal.push_str("\\\""),
            Ok('\\') => literal.push_str("\\\\"),
            Ok('\n') => literal.push_str("\\n"),
            Ok('\r') => literal.push_str("\\r"),
            Ok('\t') => literal.push_str("\\t"),
            Ok('\u{8}') => literal.push_str("\\b"),
            Ok('\u{c}') => literal.push_str("\\f"),
            Ok(ch) if ch < ' ' => literal.push_str(&format!("\\u{:04X}", ch as u32)),
            Ok(ch) => literal.push(ch),
        }
    }
    literal.push('"');
    literal
}
/// Decodes modified UTF-8 without losing isolated UTF-16 surrogate code units.
pub(crate) fn modified_utf8_units(value: &noak::MStr) -> Vec<u16> {
    value
        .chars()
        .flat_map(|value| match value {
            Ok(value) => value.encode_utf16(&mut [0; 2]).to_vec(),
            Err(unit) => vec![unit as u16],
        })
        .collect()
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AtTargetKind {
    Invoke,
    Field,
    New,
    Constant,
    Return,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AtTargetOperationKind {
    InvokeVirtual,
    InvokeStatic,
    InvokeSpecial,
    InvokeInterface,
    FieldGetInstance,
    FieldPutInstance,
    FieldGetStatic,
    FieldPutStatic,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OccurrenceResultClassification {
    Void,
    ImmediatelyPopped,
    Retained,
    #[default]
    NotApplicable,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AtTargetCandidate {
    pub owner: String,
    pub name: String,
    pub descriptor: String,
    pub display_label: String,
    pub detail: String,
    pub kind: AtTargetKind,
    pub ordinal: Option<usize>,
    pub namespace: MappingNamespace,
    pub constant_value: Option<ConstantValue>,
    pub operation_kind: Option<AtTargetOperationKind>,
    pub instruction_occurrence_index: i32,
    pub occurrence_result_classification: OccurrenceResultClassification,
    pub condition_opcode: Option<i32>,
}
impl AtTargetCandidate {
    pub fn new(
        kind: AtTargetKind,
        owner: impl Into<String>,
        name: impl Into<String>,
        descriptor: impl Into<String>,
    ) -> Self {
        Self {
            owner: owner.into(),
            name: name.into(),
            descriptor: descriptor.into(),
            display_label: String::new(),
            detail: String::new(),
            kind,
            ordinal: None,
            namespace: MappingNamespace::Named,
            constant_value: None,
            operation_kind: None,
            instruction_occurrence_index: -1,
            occurrence_result_classification: OccurrenceResultClassification::NotApplicable,
            condition_opcode: None,
        }
    }
}
pub trait BytecodeIndex: Send + Sync {
    fn get_at_target_candidates(
        &self,
        owner: &str,
        method: &str,
        descriptor: Option<&str>,
        at_value: &str,
    ) -> Vec<AtTargetCandidate>;
    fn get_return_ordinal_count(
        &self,
        owner: &str,
        method: &str,
        descriptor: Option<&str>,
    ) -> usize;
    fn get_class_bytes(&self, _owner: &str) -> Option<Vec<u8>> {
        None
    }
    fn get_class_bytes_ref(&self, _owner: &str) -> Option<&[u8]> {
        None
    }
    fn resolve_common_super_class(&self, _left: &str, _right: &str) -> Option<String> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BytecodeIndexError {
    ClassBytesMissing {
        internal_name: String,
    },
    MethodNotFound {
        owner: String,
        name: String,
        descriptor: String,
    },
    InvalidClass(String),
    InvalidProvider(String),
}
impl fmt::Display for BytecodeIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ClassBytesMissing { internal_name } => {
                write!(f, "class bytes missing for {internal_name}")
            }
            Self::MethodNotFound {
                owner,
                name,
                descriptor,
            } => write!(f, "method not found: {owner}.{name}{descriptor}"),
            Self::InvalidClass(message) | Self::InvalidProvider(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for BytecodeIndexError {}
impl From<noak::error::DecodeError> for BytecodeIndexError {
    fn from(value: noak::error::DecodeError) -> Self {
        Self::InvalidClass(value.to_string())
    }
}
pub type BytecodeIndexResult<T> = Result<T, BytecodeIndexError>;

pub trait ClassBytesProvider: Send + Sync {
    fn get_class_bytes(&self, internal_name: &str) -> Option<Vec<u8>>;
    fn get_class_bytes_ref(&self, _internal_name: &str) -> Option<&[u8]> {
        None
    }
    fn classpath_entry_ids(&self) -> BTreeSet<String>;
    fn classpath_entry_hash(&self, entry_id: &str) -> Option<i64>;
    fn classpath_hash(&self) -> String;
    fn internal_names(&self) -> Vec<String>;
}
#[derive(Debug, Clone)]
pub struct InMemoryClassBytesProvider {
    classes: BTreeMap<String, Vec<u8>>,
    entry_hashes: BTreeMap<String, i64>,
    default_entry_id: String,
}
impl InMemoryClassBytesProvider {
    pub fn new(classes: BTreeMap<String, Vec<u8>>) -> BytecodeIndexResult<Self> {
        Self::with_entries(classes, BTreeMap::new(), "in-memory")
    }
    pub fn with_entries(
        classes: BTreeMap<String, Vec<u8>>,
        entry_hashes: BTreeMap<String, i64>,
        default_entry_id: impl Into<String>,
    ) -> BytecodeIndexResult<Self> {
        if classes.keys().any(|name| name.contains('.')) {
            return Err(BytecodeIndexError::InvalidProvider(
                "class keys must use internal names with slashes, not dots".into(),
            ));
        }
        Ok(Self {
            classes,
            entry_hashes,
            default_entry_id: default_entry_id.into(),
        })
    }
    pub fn all_classes(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.classes
    }
    pub fn from_resources(directory: &std::path::Path) -> BytecodeIndexResult<Self> {
        fn visit(
            root: &std::path::Path,
            current: &std::path::Path,
            classes: &mut BTreeMap<String, Vec<u8>>,
        ) -> std::io::Result<()> {
            for entry in std::fs::read_dir(current)? {
                let entry = entry?;
                let path = entry.path();
                if entry.file_type()?.is_dir() {
                    visit(root, &path, classes)?;
                } else if path.extension().is_some_and(|ext| ext == "class") {
                    let name = path
                        .strip_prefix(root)
                        .unwrap()
                        .with_extension("")
                        .to_string_lossy()
                        .replace('\\', "/");
                    classes.insert(name, std::fs::read(path)?);
                }
            }
            Ok(())
        }
        let mut classes = BTreeMap::new();
        visit(directory, directory, &mut classes)
            .map_err(|error| BytecodeIndexError::InvalidProvider(error.to_string()))?;
        Self::new(classes)
    }
}
impl ClassBytesProvider for InMemoryClassBytesProvider {
    fn get_class_bytes(&self, name: &str) -> Option<Vec<u8>> {
        self.classes.get(name).cloned()
    }
    fn get_class_bytes_ref(&self, name: &str) -> Option<&[u8]> {
        self.classes.get(name).map(Vec::as_slice)
    }
    fn internal_names(&self) -> Vec<String> {
        self.classes.keys().cloned().collect()
    }
    fn classpath_entry_ids(&self) -> BTreeSet<String> {
        if self.entry_hashes.is_empty() {
            BTreeSet::from([self.default_entry_id.clone()])
        } else {
            self.entry_hashes.keys().cloned().collect()
        }
    }
    fn classpath_entry_hash(&self, id: &str) -> Option<i64> {
        self.entry_hashes.get(id).copied().or_else(|| {
            (id == self.default_entry_id).then(|| {
                let digest = Sha256::digest(self.classpath_hash().as_bytes());
                i64::from_be_bytes(digest[..8].try_into().unwrap())
            })
        })
    }
    fn classpath_hash(&self) -> String {
        let mut digest = Sha256::new();
        for (name, bytes) in &self.classes {
            digest.update(name.as_bytes());
            digest.update(bytes);
        }
        for (id, hash) in &self.entry_hashes {
            digest.update(id.as_bytes());
            digest.update(hash.to_string().as_bytes());
        }
        format!("{:x}", digest.finalize())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytecodeClassEntry {
    pub internal_name: String,
    pub qualified_name: String,
    pub simple_name: String,
    pub superclass_internal_name: Option<String>,
    pub interface_internal_names: Vec<String>,
    pub classpath_entry_id: Option<String>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClassMemberIndex {
    pub classes: BTreeMap<String, BytecodeClassEntry>,
    pub methods_by_owner: BTreeMap<String, Vec<MethodIndexEntry>>,
    pub fields_by_owner: BTreeMap<String, Vec<FieldIndexEntry>>,
}
impl ClassMemberIndex {
    pub fn find_class(&self, name: &str) -> Option<&BytecodeClassEntry> {
        self.classes.get(name)
    }
    pub fn find_method(
        &self,
        owner: &str,
        name: &str,
        descriptor: &str,
    ) -> Option<&MethodIndexEntry> {
        self.methods_by_owner
            .get(owner)?
            .iter()
            .find(|entry| entry.name == name && entry.descriptor == descriptor)
    }
    pub fn find_field(
        &self,
        owner: &str,
        name: &str,
        descriptor: &str,
    ) -> Option<&FieldIndexEntry> {
        self.fields_by_owner
            .get(owner)?
            .iter()
            .find(|entry| entry.name == name && entry.descriptor == descriptor)
    }
    pub fn index_class(
        bytes: &[u8],
        forced_name: Option<&str>,
        entry_id: Option<&str>,
    ) -> BytecodeIndexResult<Self> {
        let class = Class::new(bytes)?;
        let pool = class.pool();
        let name = forced_name.map(str::to_owned).unwrap_or(
            pool.retrieve(class.this_class())?
                .name
                .chars_lossy()
                .collect::<String>(),
        );
        let super_name = class
            .super_class()
            .map(|index| {
                pool.retrieve(index)
                    .map(|value| value.name.chars_lossy().collect::<String>())
            })
            .transpose()?
            .filter(|name| name != "java/lang/Object");
        let interfaces = class
            .interfaces()
            .into_iter()
            .map(|index| {
                Ok(pool
                    .retrieve(index?)?
                    .name
                    .chars_lossy()
                    .collect::<String>())
            })
            .collect::<BytecodeIndexResult<Vec<_>>>()?;
        let mut result = Self::default();
        result.classes.insert(
            name.clone(),
            BytecodeClassEntry {
                internal_name: name.clone(),
                qualified_name: name.replace('/', "."),
                simple_name: name.rsplit('/').next().unwrap().into(),
                superclass_internal_name: super_name,
                interface_internal_names: interfaces,
                classpath_entry_id: entry_id.map(str::to_owned),
            },
        );
        for method in class.methods() {
            let method = method?;
            let method_name = pool
                .retrieve(method.name())?
                .chars_lossy()
                .collect::<String>();
            let descriptor = pool
                .retrieve(method.descriptor())?
                .chars_lossy()
                .collect::<String>();
            result
                .methods_by_owner
                .entry(name.clone())
                .or_default()
                .push(MethodIndexEntry {
                    name: method_name.clone(),
                    readable_signature: method_label(&method_name, &descriptor),
                    descriptor,
                    is_static: method.access_flags().contains(AccessFlags::STATIC),
                });
        }
        for field in class.fields() {
            let field = field?;
            let descriptor = pool
                .retrieve(field.descriptor())?
                .chars_lossy()
                .collect::<String>();
            result
                .fields_by_owner
                .entry(name.clone())
                .or_default()
                .push(FieldIndexEntry {
                    name: pool
                        .retrieve(field.name())?
                        .chars_lossy()
                        .collect::<String>(),
                    readable_type: field_label(&descriptor),
                    descriptor,
                    is_static: field.access_flags().contains(AccessFlags::STATIC),
                });
        }
        Ok(result)
    }
    pub fn build(
        provider: &dyn ClassBytesProvider,
        entry_id: Option<&str>,
    ) -> BytecodeIndexResult<Self> {
        let mut result = Self::default();
        for name in provider.internal_names() {
            let bytes = provider.get_class_bytes(&name).ok_or_else(|| {
                BytecodeIndexError::ClassBytesMissing {
                    internal_name: name.clone(),
                }
            })?;
            let part = Self::index_class(&bytes, Some(&name), entry_id)?;
            result.classes.extend(part.classes);
            result.methods_by_owner.extend(part.methods_by_owner);
            result.fields_by_owner.extend(part.fields_by_owner);
        }
        Ok(result)
    }
}
#[derive(Debug, Clone)]
pub struct BytecodeClassIndexAdapter {
    pub member_index: ClassMemberIndex,
}
impl ClassIndex for BytecodeClassIndexAdapter {
    fn find_classes(&self, prefix: &str, limit: usize) -> Vec<ClassIndexEntry> {
        let prefix = prefix.to_lowercase();
        self.member_index
            .classes
            .values()
            .filter(|value| {
                value.simple_name.to_lowercase().starts_with(&prefix)
                    || value.qualified_name.to_lowercase().starts_with(&prefix)
                    || value.internal_name.to_lowercase().starts_with(&prefix)
            })
            .take(limit)
            .map(to_class_entry)
            .collect()
    }
    fn find_class(&self, name: &str) -> Option<ClassIndexEntry> {
        self.member_index.classes.get(name).map(to_class_entry)
    }
    fn find_class_by_fqn(&self, fqn: &str) -> Option<ClassIndexEntry> {
        self.member_index
            .classes
            .values()
            .find(|value| value.qualified_name == fqn)
            .map(to_class_entry)
    }
    fn get_methods(&self, owner: &str) -> Vec<MethodIndexEntry> {
        self.member_index
            .methods_by_owner
            .get(owner)
            .cloned()
            .unwrap_or_default()
    }
    fn get_fields(&self, owner: &str) -> Vec<FieldIndexEntry> {
        self.member_index
            .fields_by_owner
            .get(owner)
            .cloned()
            .unwrap_or_default()
    }
}
fn to_class_entry(value: &BytecodeClassEntry) -> ClassIndexEntry {
    ClassIndexEntry {
        simple_name: value.simple_name.clone(),
        package_name: value
            .qualified_name
            .rsplit_once('.')
            .map(|(package, _)| package)
            .unwrap_or("")
            .into(),
        internal_name: value.internal_name.clone(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytecodeCacheKey {
    pub classpath_entry_id: String,
    pub last_modified: i64,
    pub internal_name: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BytecodeClassRef {
    pub internal_name: String,
    pub classpath_entry_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MethodInstructionCacheKey {
    pub class_ref: BytecodeClassRef,
    pub method_name: String,
    pub descriptor: String,
    pub namespace: MappingNamespace,
}
#[derive(Debug, Clone)]
pub struct CachedBytecodeIndex {
    pub classpath_hash: String,
    pub member_index: ClassMemberIndex,
    pub class_cache_keys: BTreeMap<String, BytecodeCacheKey>,
}
#[derive(Debug, Default)]
pub struct BytecodeIndexCache {
    cached_index: Option<CachedBytecodeIndex>,
    instruction_cache: HashMap<MethodInstructionCacheKey, Vec<AtTargetCandidate>>,
}
impl BytecodeIndexCache {
    pub fn get_index(&self, hash: &str) -> Option<&CachedBytecodeIndex> {
        self.cached_index
            .as_ref()
            .filter(|index| index.classpath_hash == hash)
    }
    pub fn put_index(&mut self, index: CachedBytecodeIndex) {
        if self
            .cached_index
            .as_ref()
            .map(|index| &index.classpath_hash)
            != Some(&index.classpath_hash)
        {
            self.instruction_cache.clear();
        }
        self.cached_index = Some(index);
    }
    pub fn get_instructions(
        &self,
        key: &MethodInstructionCacheKey,
    ) -> Option<&Vec<AtTargetCandidate>> {
        self.instruction_cache.get(key)
    }
    pub fn put_instructions(
        &mut self,
        key: MethodInstructionCacheKey,
        value: Vec<AtTargetCandidate>,
    ) {
        self.instruction_cache.insert(key, value);
    }
    pub fn invalidate_classpath_entry(&mut self, id: &str, modified: i64) {
        if let Some(index) = self.cached_index.as_mut() {
            for key in index
                .class_cache_keys
                .values_mut()
                .filter(|key| key.classpath_entry_id == id && key.last_modified != modified)
            {
                key.last_modified = modified;
                self.instruction_cache
                    .retain(|entry, _| entry.class_ref.internal_name != key.internal_name);
            }
        }
    }
    pub fn invalidate_class(&mut self, name: &str) {
        self.instruction_cache
            .retain(|entry, _| entry.class_ref.internal_name != name);
        if let Some(index) = self.cached_index.as_mut() {
            index.class_cache_keys.remove(name);
        }
    }
    pub fn clear(&mut self) {
        self.cached_index = None;
        self.instruction_cache.clear();
    }
    pub fn cache_key_for_path(path: &std::path::Path, name: &str) -> BytecodeCacheKey {
        let modified = std::fs::metadata(path)
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|time| time.as_millis() as i64)
            .unwrap_or(0);
        BytecodeCacheKey {
            classpath_entry_id: path.to_string_lossy().into_owned(),
            last_modified: modified,
            internal_name: name.into(),
        }
    }
}
#[derive(Debug, Default)]
pub struct BytecodeIndexService {
    pub cache: BytecodeIndexCache,
}
impl BytecodeIndexService {
    pub fn build_index(
        &mut self,
        provider: &dyn ClassBytesProvider,
        entry: Option<&str>,
    ) -> BytecodeIndexResult<ClassMemberIndex> {
        let hash = provider.classpath_hash();
        if let Some(cached) = self.cache.get_index(&hash) {
            return Ok(cached.member_index.clone());
        }
        let index = ClassMemberIndex::build(provider, entry)?;
        let entry_id = provider
            .classpath_entry_ids()
            .into_iter()
            .next()
            .unwrap_or_else(|| "in-memory".into());
        let modified = provider.classpath_entry_hash(&entry_id).unwrap_or(0);
        let keys = provider
            .internal_names()
            .into_iter()
            .map(|name| {
                (
                    name.clone(),
                    BytecodeCacheKey {
                        internal_name: name,
                        classpath_entry_id: entry_id.clone(),
                        last_modified: modified,
                    },
                )
            })
            .collect();
        self.cache.put_index(CachedBytecodeIndex {
            classpath_hash: hash,
            member_index: index.clone(),
            class_cache_keys: keys,
        });
        Ok(index)
    }
    pub fn extract_instructions(
        &mut self,
        provider: &dyn ClassBytesProvider,
        owner: &str,
        name: &str,
        descriptor: &str,
        namespace: MappingNamespace,
        entry: Option<&str>,
    ) -> BytecodeIndexResult<Vec<AtTargetCandidate>> {
        let bytes = provider.get_class_bytes(owner).ok_or_else(|| {
            BytecodeIndexError::ClassBytesMissing {
                internal_name: owner.into(),
            }
        })?;
        if self
            .build_index(provider, None)?
            .find_method(owner, name, descriptor)
            .is_none()
        {
            return Err(BytecodeIndexError::MethodNotFound {
                owner: owner.into(),
                name: name.into(),
                descriptor: descriptor.into(),
            });
        }
        let entry = entry
            .map(str::to_owned)
            .or_else(|| provider.classpath_entry_ids().into_iter().next())
            .unwrap_or_else(|| "default".into());
        let key = MethodInstructionCacheKey {
            class_ref: BytecodeClassRef {
                internal_name: owner.into(),
                classpath_entry_id: entry,
            },
            method_name: name.into(),
            descriptor: descriptor.into(),
            namespace,
        };
        if let Some(cached) = self.cache.get_instructions(&key) {
            return Ok(cached.clone());
        }
        let candidates = extract_instructions(&bytes, name, descriptor)?;
        self.cache.put_instructions(key, candidates.clone());
        Ok(candidates)
    }
}

pub fn matches_at_value(kind: AtTargetKind, value: &str) -> bool {
    matches!(
        (kind, value.to_ascii_uppercase().as_str()),
        (AtTargetKind::Invoke, "INVOKE" | "INVOKE_ASSIGN")
            | (AtTargetKind::Field, "FIELD")
            | (AtTargetKind::New, "NEW")
            | (AtTargetKind::Return, "RETURN")
            | (AtTargetKind::Constant, "CONSTANT")
    )
}

pub fn assign_ordinals(candidates: &mut [AtTargetCandidate]) {
    let mut counters = HashMap::new();
    for candidate in candidates {
        let key = (
            candidate.kind,
            candidate.operation_kind,
            candidate.owner.clone(),
            candidate.name.clone(),
            candidate.descriptor.clone(),
            candidate.constant_value.clone(),
        );
        let ordinal = counters.entry(key).or_insert(0);
        candidate.ordinal = Some(*ordinal);
        *ordinal += 1;
    }
}

pub fn extract_instructions(
    bytes: &[u8],
    name: &str,
    descriptor: &str,
) -> BytecodeIndexResult<Vec<AtTargetCandidate>> {
    let class = Class::new(bytes)?;
    let pool = class.pool();
    for method in class.methods() {
        let method = method?;
        if pool
            .retrieve(method.name())?
            .chars_lossy()
            .collect::<String>()
            != name
            || pool
                .retrieve(method.descriptor())?
                .chars_lossy()
                .collect::<String>()
                != descriptor
        {
            continue;
        }
        for attribute in method.attributes() {
            let attribute = attribute?;
            if pool.retrieve(attribute.name())?.as_bytes() != b"Code" {
                continue;
            }
            let code = Code::try_from(attribute.read_content(pool)?)?;
            return extract_code(&code, attribute.content(), pool, bytes);
        }
        return Ok(Vec::new());
    }
    Ok(Vec::new())
}

fn extract_code(
    code: &Code<'_>,
    content: &[u8],
    pool: &cpool::ConstantPool<'_>,
    class_bytes: &[u8],
) -> BytecodeIndexResult<Vec<AtTargetCandidate>> {
    let instructions = code.raw_instructions().collect::<Result<Vec<_>, _>>()?;
    let mut labels = BTreeSet::new();
    for handler in code.exception_handlers() {
        labels.extend([
            handler.start().as_u32(),
            handler.end().as_u32(),
            handler.handler().as_u32(),
        ]);
    }
    for (offset, instruction) in &instructions {
        let position = offset.as_u32();
        let opcode = content[8 + position as usize];
        if (0x99..=0xa8).contains(&opcode) || opcode == 0xc6 || opcode == 0xc7 {
            let at = 8 + position as usize;
            let delta = i16::from_be_bytes(content[at + 1..at + 3].try_into().unwrap());
            labels.insert((position as i64 + delta as i64) as u32);
        } else if opcode == 0xc8 || opcode == 0xc9 {
            let at = 8 + position as usize;
            let delta = i32::from_be_bytes(content[at + 1..at + 5].try_into().unwrap());
            labels.insert((position as i64 + delta as i64) as u32);
        }
        match instruction {
            RawInstruction::TableSwitch(table) => {
                labels.insert((position as i64 + table.default_offset() as i64) as u32);
                for pair in table.pairs() {
                    labels.insert((position as i64 + pair.offset() as i64) as u32);
                }
            }
            RawInstruction::LookupSwitch(table) => {
                labels.insert((position as i64 + table.default_offset() as i64) as u32);
                for pair in table.pairs() {
                    labels.insert((position as i64 + pair.offset() as i64) as u32);
                }
            }
            _ => {}
        }
    }
    // ASM retains StackMapTable frame nodes even with SKIP_DEBUG, so a frame prevents an immediate POP match.
    for attribute in code.attributes() {
        let attribute = attribute?;
        if pool.retrieve(attribute.name())?.as_bytes() == b"StackMapTable" {
            let length = u32::from_be_bytes(content[4..8].try_into().unwrap());
            stack_map_offsets(
                classfile_tail(class_bytes, attribute.content())?,
                &mut labels,
                pool,
                Some(length),
            )?;
        }
    }
    let mut result = Vec::new();
    for (index, (offset, instruction)) in instructions.iter().enumerate() {
        let opcode = content[8 + offset.as_u32() as usize];
        let next = instructions
            .get(index + 1)
            .filter(|(offset, _)| !labels.contains(&offset.as_u32()))
            .map(|(offset, _)| content[8 + offset.as_u32() as usize]);
        let previous = index
            .checked_sub(1)
            .map(|i| content[8 + instructions[i].0.as_u32() as usize]);
        let mut candidate = match opcode {
            0xb2..=0xb9 => {
                let at = 8 + offset.as_u32() as usize;
                let cp_index = cpool::Index::<cpool::Item<'_>>::new(u16::from_be_bytes(
                    content[at + 1..at + 3].try_into().unwrap(),
                ))?;
                let (owner, name, descriptor) = member_ref(pool, cp_index)?;
                let mut candidate = AtTargetCandidate::new(
                    if opcode <= 0xb5 {
                        AtTargetKind::Field
                    } else {
                        AtTargetKind::Invoke
                    },
                    owner,
                    name,
                    descriptor,
                );
                candidate.operation_kind = Some(match opcode {
                    0xb2 => AtTargetOperationKind::FieldGetStatic,
                    0xb3 => AtTargetOperationKind::FieldPutStatic,
                    0xb4 => AtTargetOperationKind::FieldGetInstance,
                    0xb5 => AtTargetOperationKind::FieldPutInstance,
                    0xb6 => AtTargetOperationKind::InvokeVirtual,
                    0xb7 => AtTargetOperationKind::InvokeSpecial,
                    0xb8 => AtTargetOperationKind::InvokeStatic,
                    _ => AtTargetOperationKind::InvokeInterface,
                });
                candidate.occurrence_result_classification = if opcode <= 0xb5 {
                    if opcode == 0xb3 || opcode == 0xb5 {
                        OccurrenceResultClassification::Void
                    } else {
                        OccurrenceResultClassification::Retained
                    }
                } else {
                    classify_method_invoke_result(&candidate.descriptor, next)
                };
                candidate
            }
            0xbb | 0xc0 | 0xc1 => {
                let class_index = match instruction {
                    RawInstruction::New { index }
                    | RawInstruction::CheckCast { index }
                    | RawInstruction::InstanceOf { index } => *index,
                    _ => unreachable!(),
                };
                let owner = pool
                    .retrieve(class_index)?
                    .name
                    .chars_lossy()
                    .collect::<String>();
                if opcode == 0xbb {
                    AtTargetCandidate::new(AtTargetKind::New, &owner, "", format!("L{owner};"))
                } else {
                    let mut c = AtTargetCandidate::new(AtTargetKind::Constant, "", "", "");
                    c.constant_value = Some(ConstantValue::ClassLiteral(owner));
                    c
                }
            }
            0x01..=0x14 => {
                let constant = match instruction {
                    RawInstruction::AConstNull => Some(ConstantValue::NullValue),
                    RawInstruction::IConstM1 => Some(ConstantValue::IntValue(-1)),
                    RawInstruction::IConst0
                    | RawInstruction::IConst1
                    | RawInstruction::IConst2
                    | RawInstruction::IConst3
                    | RawInstruction::IConst4
                    | RawInstruction::IConst5 => Some(ConstantValue::IntValue(opcode as i32 - 3)),
                    RawInstruction::LConst0 | RawInstruction::LConst1 => {
                        Some(ConstantValue::LongValue((opcode - 9) as i64))
                    }
                    RawInstruction::FConst0 | RawInstruction::FConst1 | RawInstruction::FConst2 => {
                        Some(ConstantValue::FloatValue((opcode - 11) as f32))
                    }
                    RawInstruction::DConst0 | RawInstruction::DConst1 => {
                        Some(ConstantValue::DoubleValue((opcode - 14) as f64))
                    }
                    RawInstruction::BIPush { value } => {
                        Some(ConstantValue::IntValue(*value as i32))
                    }
                    RawInstruction::SIPush { value } => {
                        Some(ConstantValue::IntValue(*value as i32))
                    }
                    RawInstruction::LdC { index }
                    | RawInstruction::LdCW { index }
                    | RawInstruction::LdC2W { index } => pool_constant(pool, *index)?,
                    _ => None,
                };
                if constant.is_none() {
                    continue;
                }
                let mut candidate = AtTargetCandidate::new(AtTargetKind::Constant, "", "", "");
                candidate.constant_value = constant;
                candidate
            }
            0xac..=0xb1 => AtTargetCandidate::new(AtTargetKind::Return, "", "RETURN", ""),
            0x9b..=0x9e if !previous.is_some_and(|opcode| (0x94..=0x98).contains(&opcode)) => {
                let mut candidate = AtTargetCandidate::new(AtTargetKind::Constant, "", "", "");
                candidate.condition_opcode = Some(opcode as i32);
                candidate
            }
            // INVOKEDYNAMIC has no MethodInsnNode in ASM and does not contribute an INVOKE target.
            _ => continue,
        };
        candidate.instruction_occurrence_index = index as i32;
        candidate.detail = candidate.owner.rsplit('/').next().unwrap_or("").into();
        candidate.display_label = match candidate.kind {
            AtTargetKind::Invoke => method_label(&candidate.name, &candidate.descriptor),
            AtTargetKind::Field => {
                format!("{}: {}", candidate.name, field_label(&candidate.descriptor))
            }
            AtTargetKind::New => candidate.detail.clone(),
            AtTargetKind::Return => "RETURN".into(),
            AtTargetKind::Constant => constant_label(candidate.constant_value.as_ref()),
        };
        result.push(candidate);
    }
    assign_ordinals(&mut result);
    Ok(result)
}

pub fn classify_method_invoke_result(
    descriptor: &str,
    next_opcode: Option<u8>,
) -> OccurrenceResultClassification {
    let return_type = descriptor
        .rsplit_once(')')
        .map(|(_, value)| value)
        .unwrap_or("");
    if return_type == "V" {
        OccurrenceResultClassification::Void
    } else if next_opcode
        == Some(if return_type == "J" || return_type == "D" {
            0x58
        } else {
            0x57
        })
    {
        OccurrenceResultClassification::ImmediatelyPopped
    } else {
        OccurrenceResultClassification::Retained
    }
}
fn member_ref(
    pool: &cpool::ConstantPool<'_>,
    index: cpool::Index<cpool::Item<'_>>,
) -> BytecodeIndexResult<(String, String, String)> {
    let (class, name_type) = match pool.get(index)? {
        cpool::Item::FieldRef(value) => (value.class, value.name_and_type),
        cpool::Item::MethodRef(value) => (value.class, value.name_and_type),
        cpool::Item::InterfaceMethodRef(value) => (value.class, value.name_and_type),
        _ => {
            return Err(BytecodeIndexError::InvalidClass(
                "member instruction must reference a field or method".into(),
            ))
        }
    };
    let name_type = pool.retrieve(name_type)?;
    Ok((
        pool.retrieve(class)?.name.chars_lossy().collect::<String>(),
        name_type.name.chars_lossy().collect::<String>(),
        name_type.descriptor.chars_lossy().collect::<String>(),
    ))
}
fn pool_constant(
    pool: &cpool::ConstantPool<'_>,
    index: cpool::Index<cpool::Item<'_>>,
) -> BytecodeIndexResult<Option<ConstantValue>> {
    Ok(match pool.get(index)? {
        cpool::Item::String(value) => Some(ConstantValue::from_utf16(modified_utf8_units(
            pool.retrieve(value.string)?,
        ))),
        cpool::Item::Integer(value) => Some(ConstantValue::IntValue(value.value)),
        cpool::Item::Long(value) => Some(ConstantValue::LongValue(value.value)),
        cpool::Item::Float(value) => Some(ConstantValue::FloatValue(value.value)),
        cpool::Item::Double(value) => Some(ConstantValue::DoubleValue(value.value)),
        cpool::Item::Class(value) => Some(ConstantValue::ClassLiteral(
            pool.retrieve(value.name)?.chars_lossy().collect::<String>(),
        )),
        cpool::Item::MethodType(value) => Some(ConstantValue::ClassLiteral(
            pool.retrieve(value.descriptor)?
                .chars_lossy()
                .collect::<String>(),
        )),
        _ => None,
    })
}
pub(crate) fn stack_map_offsets(
    bytes: &[u8],
    labels: &mut BTreeSet<u32>,
    pool: &cpool::ConstantPool<'_>,
    code_length: Option<u32>,
) -> BytecodeIndexResult<()> {
    struct Cursor<'a> {
        bytes: &'a [u8],
        at: usize,
        classes: Vec<u16>,
        uninitialized: Vec<u16>,
    }
    impl Cursor<'_> {
        fn u8(&mut self) -> BytecodeIndexResult<u8> {
            let value = *self
                .bytes
                .get(self.at)
                .ok_or_else(|| BytecodeIndexError::InvalidClass("truncated stack map".into()))?;
            self.at += 1;
            Ok(value)
        }
        fn u16(&mut self) -> BytecodeIndexResult<u16> {
            Ok(u16::from_be_bytes([self.u8()?, self.u8()?]))
        }
        fn verification(&mut self) -> BytecodeIndexResult<()> {
            match self.u8()? {
                7 => {
                    let index = self.u16()?;
                    self.classes.push(index);
                }
                8 => {
                    let offset = self.u16()?;
                    self.uninitialized.push(offset);
                }
                0..=6 => {}
                _ => {
                    return Err(BytecodeIndexError::InvalidClass(
                        "invalid stack-map verification type".into(),
                    ))
                }
            }
            Ok(())
        }
    }
    let mut cursor = Cursor {
        bytes,
        at: 0,
        classes: Vec::new(),
        uninitialized: Vec::new(),
    };
    let frames = cursor.u16()?;
    let mut offset: i64 = -1;
    for _ in 0..frames {
        let tag = cursor.u8()?;
        let delta = match tag {
            0..=63 => tag as u16,
            64..=127 => {
                cursor.verification()?;
                (tag - 64) as u16
            }
            247 => {
                let delta = cursor.u16()?;
                cursor.verification()?;
                delta
            }
            248..=251 => cursor.u16()?,
            252..=254 => {
                let delta = cursor.u16()?;
                for _ in 0..tag - 251 {
                    cursor.verification()?;
                }
                delta
            }
            255 => {
                let delta = cursor.u16()?;
                let locals = cursor.u16()?;
                for _ in 0..locals {
                    cursor.verification()?;
                }
                let stack = cursor.u16()?;
                for _ in 0..stack {
                    cursor.verification()?;
                }
                delta
            }
            _ => {
                return Err(BytecodeIndexError::InvalidClass(
                    "invalid stack-map frame".into(),
                ))
            }
        };
        offset += delta as i64 + 1;
        if code_length.is_some_and(|length| offset > i64::from(length)) {
            return Err(BytecodeIndexError::InvalidClass(
                "stack-map offset outside method".into(),
            ));
        }
        labels.insert(offset as u32);
        // ASM preloads one frame at the end label, then stops when its code walk
        // finishes, even if number_of_entries claims more frames. Bound every read
        // by the complete class buffer while preserving this observable behavior.
        if code_length.is_some_and(|length| offset >= i64::from(length)) {
            break;
        }
    }
    for index in cursor.classes {
        pool.retrieve(cpool::Index::<cpool::Class<'_>>::new(index)?)?;
    }
    for offset in cursor.uninitialized {
        if code_length.is_some_and(|length| u32::from(offset) > length) {
            return Err(BytecodeIndexError::InvalidClass(
                "uninitialized stack-map offset outside method".into(),
            ));
        }
        labels.insert(u32::from(offset));
    }
    Ok(())
}
pub(crate) fn classfile_tail<'a>(
    class_bytes: &'a [u8],
    content: &[u8],
) -> BytecodeIndexResult<&'a [u8]> {
    let start = (content.as_ptr() as usize)
        .checked_sub(class_bytes.as_ptr() as usize)
        .filter(|&start| start <= class_bytes.len() && content.len() <= class_bytes.len() - start)
        .ok_or_else(|| {
            BytecodeIndexError::InvalidClass("attribute is outside class buffer".into())
        })?;
    Ok(&class_bytes[start..])
}

fn field_label(descriptor: &str) -> String {
    crate::descriptor::parse_field_descriptor(descriptor)
        .map(|value| crate::descriptor::render(&value))
        .unwrap_or_else(|_| descriptor.into())
}
fn method_label(name: &str, descriptor: &str) -> String {
    crate::descriptor::parse_method_descriptor(descriptor)
        .map(|value| format!("{name}{}", crate::descriptor::render_method(&value)))
        .unwrap_or_else(|_| format!("{name}{descriptor}"))
}
pub fn constant_label(value: Option<&ConstantValue>) -> String {
    match value {
        Some(ConstantValue::StringValue(value)) => format!("\"{value}\""),
        Some(ConstantValue::StringUtf16Value(value)) => format!(
            "\"{}\"",
            char::decode_utf16(value.iter().copied())
                .map(|unit| match unit {
                    Ok(ch) => ch.to_string(),
                    Err(unit) => format!("\\u{:04X}", unit.unpaired_surrogate()),
                })
                .collect::<String>()
        ),
        Some(ConstantValue::IntValue(value)) => value.to_string(),
        Some(ConstantValue::LongValue(value)) => format!("{value}L"),
        Some(ConstantValue::FloatValue(value)) => format!(
            "{}f",
            float_label(
                value.to_string(),
                format!("{value:e}"),
                (*value as f64).abs()
            )
        ),
        Some(ConstantValue::DoubleValue(value)) => {
            float_label(value.to_string(), format!("{value:e}"), value.abs())
        }
        Some(ConstantValue::ClassLiteral(value)) => {
            value.rsplit('/').next().unwrap_or(value).into()
        }
        Some(ConstantValue::NullValue) => "null".into(),
        None => "CONSTANT".into(),
    }
}
fn float_label(decimal: String, scientific: String, magnitude: f64) -> String {
    if decimal == "inf" {
        return "Infinity".into();
    }
    if decimal == "-inf" {
        return "-Infinity".into();
    }
    if magnitude.is_finite() && magnitude != 0.0 && !(0.001..10_000_000.0).contains(&magnitude) {
        let scientific = if scientific
            .split_once('e')
            .is_some_and(|(mantissa, _)| !mantissa.contains('.'))
        {
            // Java21 chooses the closest decimal with at least two significant
            // digits, including subnormals (MIN_VALUE -> 1.4E-45 / 4.9E-324).
            format!(
                "{:.1e}",
                if scientific.starts_with('-') {
                    -magnitude
                } else {
                    magnitude
                }
            )
        } else {
            scientific
        };
        let (mantissa, exponent) = scientific.split_once('e').unwrap();
        return format!(
            "{}{}E{}",
            mantissa,
            if mantissa.contains('.') { "" } else { ".0" },
            exponent
        );
    }
    if magnitude.is_finite() && !decimal.contains('.') {
        format!("{decimal}.0")
    } else {
        decimal
    }
}

pub struct BytecodeAtTargetAdapter {
    provider: Arc<dyn ClassBytesProvider>,
    service: Arc<Mutex<BytecodeIndexService>>,
    common_superclass: BytecodeCommonSuperClassResolver,
}
impl BytecodeAtTargetAdapter {
    pub fn new(provider: Arc<dyn ClassBytesProvider>) -> Self {
        Self::with_service(provider, Arc::default())
    }
    pub fn with_service(
        provider: Arc<dyn ClassBytesProvider>,
        service: Arc<Mutex<BytecodeIndexService>>,
    ) -> Self {
        Self {
            common_superclass: BytecodeCommonSuperClassResolver::new(provider.clone()),
            provider,
            service,
        }
    }
    fn candidates(
        &self,
        owner: &str,
        name: &str,
        descriptor: Option<&str>,
    ) -> Vec<AtTargetCandidate> {
        if name.is_empty() {
            return Vec::new();
        }
        let mut service = self
            .service
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let descriptor = match descriptor.filter(|value| !value.is_empty()) {
            Some(value) => value.to_owned(),
            None => {
                let Ok(index) = service.build_index(self.provider.as_ref(), None) else {
                    return Vec::new();
                };
                let methods = index
                    .methods_by_owner
                    .get(owner)
                    .into_iter()
                    .flatten()
                    .filter(|entry| entry.name == name)
                    .collect::<Vec<_>>();
                if methods.len() != 1 {
                    return Vec::new();
                }
                methods[0].descriptor.clone()
            }
        };
        service
            .extract_instructions(
                self.provider.as_ref(),
                owner,
                name,
                &descriptor,
                MappingNamespace::Named,
                None,
            )
            .unwrap_or_default()
    }
}
#[derive(Default)]
pub struct ProjectContextMixinIndex {
    service: Arc<Mutex<BytecodeIndexService>>,
}
impl ProjectContextMixinIndex {
    pub fn build_class_index(
        &self,
        context: &crate::project::ProjectContext,
        provider: &dyn ClassBytesProvider,
    ) -> BytecodeIndexResult<BytecodeClassIndexAdapter> {
        let member_index = self
            .service
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .build_index(provider, Some(&context.project_id))?;
        Ok(BytecodeClassIndexAdapter { member_index })
    }
    pub fn build_bytecode_index(
        &self,
        _context: &crate::project::ProjectContext,
        provider: Arc<dyn ClassBytesProvider>,
    ) -> BytecodeAtTargetAdapter {
        BytecodeAtTargetAdapter::with_service(provider, self.service.clone())
    }
}
impl BytecodeIndex for BytecodeAtTargetAdapter {
    fn get_class_bytes_ref(&self, owner: &str) -> Option<&[u8]> {
        self.provider.get_class_bytes_ref(owner)
    }
    fn get_at_target_candidates(
        &self,
        owner: &str,
        name: &str,
        descriptor: Option<&str>,
        value: &str,
    ) -> Vec<AtTargetCandidate> {
        if value.is_empty() {
            return Vec::new();
        }
        self.candidates(owner, name, descriptor)
            .into_iter()
            .filter(|candidate| matches_at_value(candidate.kind, value))
            .collect()
    }
    fn get_return_ordinal_count(&self, owner: &str, name: &str, descriptor: Option<&str>) -> usize {
        self.candidates(owner, name, descriptor)
            .iter()
            .filter(|candidate| candidate.kind == AtTargetKind::Return)
            .count()
    }
    fn get_class_bytes(&self, owner: &str) -> Option<Vec<u8>> {
        self.provider.get_class_bytes(owner)
    }
    fn resolve_common_super_class(&self, left: &str, right: &str) -> Option<String> {
        self.common_superclass.resolve(left, right)
    }
}

#[derive(Debug, Clone)]
struct Hierarchy {
    superclass: Option<String>,
    interfaces: Vec<String>,
}
struct Lru<K, V> {
    values: Vec<(K, V)>,
    capacity: usize,
}
impl<K: PartialEq, V: Clone> Lru<K, V> {
    fn new(capacity: usize) -> Self {
        Self {
            values: Vec::new(),
            capacity,
        }
    }
    fn get(&mut self, key: &K) -> Option<V> {
        let at = self.values.iter().position(|(stored, _)| stored == key)?;
        let value = self.values.remove(at);
        let result = value.1.clone();
        self.values.push(value);
        Some(result)
    }
    fn put(&mut self, key: K, value: V) {
        if let Some(at) = self.values.iter().position(|(stored, _)| stored == &key) {
            self.values.remove(at);
        }
        self.values.push((key, value));
        if self.values.len() > self.capacity {
            self.values.remove(0);
        }
    }
}
pub struct BytecodeCommonSuperClassResolver {
    provider: Arc<dyn ClassBytesProvider>,
    hierarchies: Mutex<Lru<String, Option<Hierarchy>>>,
    pairs: Mutex<Lru<(String, String), Option<String>>>,
}
impl BytecodeCommonSuperClassResolver {
    pub fn new(provider: Arc<dyn ClassBytesProvider>) -> Self {
        Self::with_capacities(provider, 256, 256).unwrap()
    }
    pub fn with_capacities(
        provider: Arc<dyn ClassBytesProvider>,
        hierarchies: usize,
        pairs: usize,
    ) -> BytecodeIndexResult<Self> {
        if hierarchies == 0 || pairs == 0 {
            return Err(BytecodeIndexError::InvalidProvider(
                "cache capacities must be positive".into(),
            ));
        }
        Ok(Self {
            provider,
            hierarchies: Mutex::new(Lru::new(hierarchies)),
            pairs: Mutex::new(Lru::new(pairs)),
        })
    }
    pub fn resolve(&self, left: &str, right: &str) -> Option<String> {
        let key = if left <= right {
            (left.into(), right.into())
        } else {
            (right.into(), left.into())
        };
        if let Some(value) = self
            .pairs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
        {
            return value;
        }
        let value = self.resolve_uncached(left, right);
        self.pairs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .put(key, value.clone());
        value
    }
    fn resolve_uncached(&self, left: &str, right: &str) -> Option<String> {
        crate::descriptor::parse_field_descriptor(left).ok()?;
        crate::descriptor::parse_field_descriptor(right).ok()?;
        if left == right {
            return Some(left.into());
        }
        if !(left.starts_with('L') || left.starts_with('['))
            || !(right.starts_with('L') || right.starts_with('['))
        {
            return None;
        }
        if self.assignable(left, right, &mut BTreeSet::new()) {
            return Some(right.into());
        }
        if self.assignable(right, left, &mut BTreeSet::new()) {
            return Some(left.into());
        }
        match (left.strip_prefix('['), right.strip_prefix('[')) {
            (Some(left), Some(right)) => {
                if left.len() == 1 || right.len() == 1 {
                    return Some("Ljava/lang/Object;".into());
                }
                return self
                    .resolve(left, right)
                    .map(|element| format!("[{element}"));
            }
            (Some(_), None) | (None, Some(_)) => return Some("Ljava/lang/Object;".into()),
            _ => {}
        }
        let mut ordered = Vec::new();
        self.ancestors(&left[1..left.len() - 1], &mut ordered, &mut BTreeSet::new());
        let mut other = Vec::new();
        self.ancestors(&right[1..right.len() - 1], &mut other, &mut BTreeSet::new());
        if (ordered.len() == 1 && self.hierarchy(&ordered[0]).is_none())
            || (other.len() == 1 && self.hierarchy(&other[0]).is_none())
        {
            return None;
        }
        ordered
            .into_iter()
            .find(|name| other.contains(name))
            .map(|name| format!("L{name};"))
    }
    fn assignable(&self, sub: &str, sup: &str, visiting: &mut BTreeSet<String>) -> bool {
        if sub == sup || sup == "Ljava/lang/Object;" {
            return true;
        }
        if let Some(component) = sub.strip_prefix('[') {
            if sup == "Ljava/lang/Cloneable;" || sup == "Ljava/io/Serializable;" {
                return true;
            }
            let Some(other) = sup.strip_prefix('[') else {
                return false;
            };
            return if component.len() == 1 || other.len() == 1 {
                component == other
            } else {
                self.assignable(component, other, visiting)
            };
        }
        if sup.starts_with('[') || !visiting.insert(sub.into()) {
            return false;
        }
        let Some(info) = self.hierarchy(&sub[1..sub.len() - 1]) else {
            return false;
        };
        info.superclass
            .iter()
            .chain(info.interfaces.iter())
            .any(|name| self.assignable(&format!("L{name};"), sup, visiting))
    }
    fn ancestors(&self, name: &str, ordered: &mut Vec<String>, visited: &mut BTreeSet<String>) {
        if !visited.insert(name.into()) {
            return;
        }
        ordered.push(name.into());
        if name == "java/lang/Object" {
            return;
        }
        if let Some(info) = self.hierarchy(name) {
            self.ancestors(
                info.superclass.as_deref().unwrap_or("java/lang/Object"),
                ordered,
                visited,
            );
            for interface in info.interfaces {
                self.ancestors(&interface, ordered, visited);
            }
        }
    }
    fn hierarchy(&self, name: &str) -> Option<Hierarchy> {
        let key = name.to_owned();
        if let Some(value) = self
            .hierarchies
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
        {
            return value;
        }
        let value = self
            .provider
            .get_class_bytes(name)
            .and_then(|bytes| ClassMemberIndex::index_class(&bytes, None, None).ok())
            .and_then(|index| index.classes.into_values().next())
            .map(|entry| Hierarchy {
                superclass: entry.superclass_internal_name,
                interfaces: entry.interface_internal_names,
            });
        self.hierarchies
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .put(key, value.clone());
        value
    }
}
