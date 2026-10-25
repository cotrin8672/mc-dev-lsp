//! Host-owned project and binary facts. Callbacks enter a fresh host evaluation;
//! neither this module nor its caches retain project/type handles or scan paths.
use crate::{
    bytecode::*,
    descriptor,
    host_source::{
        HostAnswer, HostFact, HostGenerations, HostReadSet, HostReferenceTypeFact, HostSourceError,
        ProjectSourceQuery, SourceAuthority,
    },
    model::*,
    project::*,
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
/// All eight counters must be authoritative; zero means a known generation zero.
/// If semantic generations are unavailable, the host must reject validation.
pub struct HostSessionIdentity {
    pub workspace_root_uri: String,
    pub project_session_version: u64,
    pub facts: HostProjectFacts,
    pub generations: HostGenerations,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostClasspathEntry {
    pub id: String,
    pub path: PathBuf,
    pub timestamp: Option<i64>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostBytesPurpose {
    MemberQuery,
    Fingerprint,
}
pub type HostProjectResult<T> = Result<T, HostSourceError>;
/// Known/Missing distinguish complete answers from unavailable capabilities. Host
/// implementations validate all generations and positive AND negative read stamps,
/// registering those reads in the current evaluation even on a cache hit.
pub trait HostProjectQuery: Send + Sync {
    fn is_open(&self, identity: &HostSessionIdentity) -> bool;
    fn validate(
        &self,
        identity: &HostSessionIdentity,
        reads: &HostReadSet,
    ) -> HostProjectResult<bool>;
    fn catalog(
        &self,
        identity: &HostSessionIdentity,
        entry: &HostClasspathEntry,
    ) -> HostProjectResult<HostAnswer<Vec<String>>>;
    fn class_bytes(
        &self,
        identity: &HostSessionIdentity,
        entry: &HostClasspathEntry,
        owner: &str,
        purpose: HostBytesPurpose,
    ) -> HostProjectResult<HostAnswer<Arc<[u8]>>>;
    fn reference_type(
        &self,
        _identity: &HostSessionIdentity,
        _owner: &str,
    ) -> HostProjectResult<HostAnswer<HostReferenceTypeFact>> {
        Ok(HostAnswer::unavailable(
            "reference type facts are unavailable",
        ))
    }
    fn is_assignable(
        &self,
        _identity: &HostSessionIdentity,
        _from: &str,
        _to: &str,
    ) -> HostProjectResult<HostAnswer<bool>> {
        Ok(HostAnswer::unavailable(
            "reference assignability is unavailable",
        ))
    }
    fn definition(
        &self,
        _identity: &HostSessionIdentity,
        _target: &McDefinitionTarget,
    ) -> HostProjectResult<HostAnswer<crate::mixin_navigation::ResolvedDefinition>> {
        Ok(HostAnswer::unavailable(
            "definition locations are unavailable",
        ))
    }
}

struct Lru<T> {
    values: BTreeMap<String, T>,
    order: VecDeque<String>,
    capacity: usize,
}
impl<T: Clone> Lru<T> {
    fn new(capacity: usize) -> Self {
        Self {
            values: BTreeMap::new(),
            order: VecDeque::new(),
            capacity,
        }
    }
    fn get(&mut self, key: &str) -> Option<T> {
        let value = self.values.get(key)?.clone();
        self.order.retain(|v| v != key);
        self.order.push_back(key.into());
        Some(value)
    }
    fn put(&mut self, key: String, value: T) {
        self.values.insert(key.clone(), value);
        self.order.retain(|v| v != &key);
        self.order.push_back(key);
        while self.values.len() > self.capacity {
            if let Some(old) = self.order.pop_front() {
                self.values.remove(&old);
            }
        }
    }
    fn clear(&mut self) {
        self.values.clear();
        self.order.clear();
    }
}
#[derive(Clone)]
struct OwnedBytes {
    bytes: Arc<[u8]>,
    reads: HostReadSet,
}
pub struct HostClassBytesProvider {
    pub identity: HostSessionIdentity,
    pub entries: Vec<HostClasspathEntry>,
    host: Arc<dyn HostProjectQuery>,
    catalog: Mutex<Option<Vec<String>>>,
    bytes: Mutex<Lru<OwnedBytes>>,
    stripes: Vec<Mutex<()>>,
    reads: Mutex<HostReadSet>,
    counter: Arc<AtomicUsize>,
    revision: AtomicU64,
    closed: AtomicBool,
}
impl HostClassBytesProvider {
    pub fn new(
        identity: HostSessionIdentity,
        entries: Vec<HostClasspathEntry>,
        host: Arc<dyn HostProjectQuery>,
    ) -> Self {
        Self::with_capacity(identity, entries, host, 256, Arc::new(AtomicUsize::new(0)))
    }
    pub fn with_capacity(
        identity: HostSessionIdentity,
        entries: Vec<HostClasspathEntry>,
        host: Arc<dyn HostProjectQuery>,
        capacity: usize,
        counter: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            identity,
            entries,
            host,
            catalog: Mutex::new(None),
            bytes: Mutex::new(Lru::new(capacity)),
            stripes: (0..64).map(|_| Mutex::new(())).collect(),
            reads: Mutex::new(Vec::new()),
            counter,
            revision: AtomicU64::new(0),
            closed: AtomicBool::new(false),
        }
    }
    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.invalidate();
    }
    fn invalidate(&self) {
        *self.catalog.lock().unwrap() = None;
        self.bytes.lock().unwrap().clear();
        self.reads.lock().unwrap().clear();
        self.revision.fetch_add(1, Ordering::AcqRel);
    }
    pub fn current_revision(&self) -> HostProjectResult<u64> {
        if self.closed.load(Ordering::Acquire) || !self.host.is_open(&self.identity) {
            return Err(HostSourceError::Failure("project session is closed".into()));
        }
        if !self.host.validate(&self.identity, &Vec::new())? {
            return Err(HostSourceError::Failure(
                "project session generations are stale".into(),
            ));
        }
        let reads = self.reads.lock().unwrap().clone();
        if !reads.is_empty() && !self.host.validate(&self.identity, &reads)? {
            self.invalidate();
        }
        Ok(self.revision.load(Ordering::Acquire))
    }
    fn accept<T>(&self, answer: HostAnswer<T>) -> HostProjectResult<HostFact<T>> {
        if !self.host.validate(&self.identity, &answer.reads)? {
            return Err(HostSourceError::Failure(
                "host answer changed during evaluation".into(),
            ));
        }
        let mut reads = self.reads.lock().unwrap();
        for read in answer.reads {
            if !reads.contains(&read) {
                reads.push(read);
            }
        }
        Ok(answer.fact)
    }
    fn ordered_entries(&self) -> Vec<&HostClasspathEntry> {
        let mut entries = self.entries.iter().collect::<Vec<_>>();
        entries.sort_by(|a, b| {
            a.path
                .to_string_lossy()
                .encode_utf16()
                .cmp(b.path.to_string_lossy().encode_utf16())
        });
        entries
    }
    pub fn catalog_result(&self) -> HostProjectResult<Vec<String>> {
        let revision = self.current_revision()?;
        if let Some(catalog) = self.catalog.lock().unwrap().clone() {
            return Ok(catalog);
        }
        let mut names = BTreeSet::new();
        for entry in self.ordered_entries() {
            let answer = self.host.catalog(&self.identity, entry)?;
            match self.accept(answer)? {
                HostFact::Known(values) => names.extend(values),
                HostFact::Missing => {}
                HostFact::Unavailable { reason } => return Err(HostSourceError::Failure(reason)),
            }
        }
        let mut names = names.into_iter().collect::<Vec<_>>();
        names.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
        if self.current_revision()? != revision {
            return Err(HostSourceError::Failure(
                "catalog changed during evaluation".into(),
            ));
        }
        *self.catalog.lock().unwrap() = Some(names.clone());
        Ok(names)
    }
    pub fn class_bytes_result(&self, owner: &str) -> HostProjectResult<Option<Arc<[u8]>>> {
        let revision = self.current_revision()?;
        if let Some(value) = self.bytes.lock().unwrap().get(owner) {
            if self.host.validate(&self.identity, &value.reads)? {
                return Ok(Some(value.bytes));
            }
        }
        let stripe = owner.bytes().fold(0usize, |hash, v| {
            hash.wrapping_mul(31).wrapping_add(v as usize)
        }) % self.stripes.len();
        let _load = self.stripes[stripe].lock().unwrap();
        self.current_revision()?;
        if let Some(value) = self.bytes.lock().unwrap().get(owner) {
            return Ok(Some(value.bytes));
        }
        let mut observed = Vec::new();
        for entry in self.ordered_entries() {
            let answer = self.host.class_bytes(
                &self.identity,
                entry,
                owner,
                HostBytesPurpose::MemberQuery,
            )?;
            observed.extend(answer.reads.clone());
            match self.accept(answer)? {
                HostFact::Known(bytes) => {
                    if self.current_revision()? != revision {
                        return Err(HostSourceError::Failure(
                            "class changed during evaluation".into(),
                        ));
                    }
                    self.counter.fetch_add(1, Ordering::Relaxed);
                    self.bytes.lock().unwrap().put(
                        owner.into(),
                        OwnedBytes {
                            bytes: bytes.clone(),
                            reads: observed,
                        },
                    );
                    return Ok(Some(bytes));
                }
                HostFact::Missing => {}
                HostFact::Unavailable { reason } => return Err(HostSourceError::Failure(reason)),
            }
        }
        Ok(None)
    }
    pub fn class_count(&self) -> usize {
        self.catalog_result().map_or(0, |v| v.len())
    }
    pub fn catalog_initialized(&self) -> bool {
        self.catalog.lock().unwrap().is_some()
    }
    pub fn byte_read_count(&self) -> usize {
        self.counter.load(Ordering::Relaxed)
    }
    pub fn cached_owner_count(&self) -> usize {
        self.bytes.lock().unwrap().values.len()
    }
    pub fn find_internal_names(&self, prefix: &str, limit: usize) -> Vec<String> {
        let prefix = prefix.to_lowercase();
        self.catalog_result()
            .unwrap_or_default()
            .into_iter()
            .filter(|v| {
                v.to_lowercase().starts_with(&prefix)
                    || v.replace('/', ".").to_lowercase().starts_with(&prefix)
                    || v.rsplit('/')
                        .next()
                        .unwrap_or(v)
                        .to_lowercase()
                        .starts_with(&prefix)
            })
            .take(limit)
            .collect()
    }
    pub fn find_internal_name_by_fqn(&self, fqn: &str) -> Option<String> {
        self.catalog_result()
            .ok()?
            .into_iter()
            .find(|v| v.replace('/', ".").to_lowercase() == fqn.to_lowercase())
    }
    pub fn contains_internal_name(&self, owner: &str) -> bool {
        self.catalog_result().is_ok_and(|v| {
            v.binary_search_by(|v| v.encode_utf16().cmp(owner.encode_utf16()))
                .is_ok()
        })
    }
    pub fn fingerprint_result(&self) -> HostProjectResult<String> {
        self.current_revision()?;
        let mut hash = Sha256::new();
        for name in self.catalog_result()? {
            hash.update(name.as_bytes());
            for entry in self.ordered_entries() {
                let answer = self.host.class_bytes(
                    &self.identity,
                    entry,
                    &name,
                    HostBytesPurpose::Fingerprint,
                )?;
                match self.accept(answer)? {
                    HostFact::Known(bytes) => {
                        hash.update(&bytes);
                        break;
                    }
                    HostFact::Missing => {}
                    HostFact::Unavailable { reason } => {
                        return Err(HostSourceError::Failure(reason))
                    }
                }
            }
        }
        for entry in self.ordered_entries() {
            hash.update(entry.path.to_string_lossy().as_bytes());
            hash.update(entry.timestamp.unwrap_or(0).to_string().as_bytes());
        }
        Ok(format!("{:x}", hash.finalize()))
    }
}
impl ClassBytesProvider for HostClassBytesProvider {
    fn get_class_bytes(&self, owner: &str) -> Option<Vec<u8>> {
        self.class_bytes_result(owner)
            .ok()
            .flatten()
            .map(|v| v.to_vec())
    }
    fn internal_names(&self) -> Vec<String> {
        self.catalog_result().unwrap_or_default()
    }
    fn classpath_entry_ids(&self) -> BTreeSet<String> {
        self.entries.iter().map(|e| e.id.clone()).collect()
    }
    fn classpath_entry_hash(&self, id: &str) -> Option<i64> {
        self.entries.iter().find(|e| e.id == id)?.timestamp
    }
    fn classpath_hash(&self) -> String {
        self.fingerprint_result().unwrap_or_default()
    }
}

struct OwnerIndex {
    bytes: Arc<[u8]>,
    index: Arc<BytecodeClassIndexAdapter>,
}
impl Clone for OwnerIndex {
    fn clone(&self) -> Self {
        Self {
            bytes: self.bytes.clone(),
            index: self.index.clone(),
        }
    }
}
pub struct LazyHostClassIndex {
    pub provider: Arc<HostClassBytesProvider>,
    owners: Mutex<Lru<OwnerIndex>>,
    parse_counter: AtomicUsize,
}
impl LazyHostClassIndex {
    pub fn new(provider: Arc<HostClassBytesProvider>) -> Self {
        Self::with_capacity(provider, 256)
    }
    pub fn with_capacity(provider: Arc<HostClassBytesProvider>, capacity: usize) -> Self {
        Self {
            provider,
            owners: Mutex::new(Lru::new(capacity)),
            parse_counter: AtomicUsize::new(0),
        }
    }
    fn owner(&self, owner: &str) -> HostProjectResult<Option<Arc<BytecodeClassIndexAdapter>>> {
        let Some(bytes) = self.provider.class_bytes_result(owner)? else {
            return Ok(None);
        };
        let mut owners = self.owners.lock().unwrap();
        if let Some(cached) = owners.get(owner) {
            if Arc::ptr_eq(&cached.bytes, &bytes) {
                return Ok(Some(cached.index));
            }
        }
        self.parse_counter.fetch_add(1, Ordering::Relaxed);
        let index = Arc::new(BytecodeClassIndexAdapter {
            member_index: ClassMemberIndex::index_class(&bytes, Some(owner), None)
                .map_err(|e| HostSourceError::Failure(e.to_string()))?,
        });
        owners.put(
            owner.into(),
            OwnerIndex {
                bytes,
                index: index.clone(),
            },
        );
        Ok(Some(index))
    }
    pub fn methods_result(
        &self,
        owner: &str,
    ) -> HostProjectResult<HostFact<Vec<MethodIndexEntry>>> {
        Ok(match self.owner(owner)? {
            Some(index) => HostFact::Known(index.get_methods(owner)),
            None => HostFact::Missing,
        })
    }
    pub fn fields_result(&self, owner: &str) -> HostProjectResult<HostFact<Vec<FieldIndexEntry>>> {
        Ok(match self.owner(owner)? {
            Some(index) => HostFact::Known(index.get_fields(owner)),
            None => HostFact::Missing,
        })
    }
    pub fn asm_parse_count(&self) -> usize {
        self.parse_counter.load(Ordering::Relaxed)
    }
    pub fn cached_owner_count(&self) -> usize {
        self.owners.lock().unwrap().values.len()
    }
}
fn class_entry(owner: &str) -> ClassIndexEntry {
    ClassIndexEntry {
        simple_name: owner.rsplit('/').next().unwrap_or(owner).into(),
        package_name: owner
            .rsplit_once('/')
            .map_or(String::new(), |(p, _)| p.replace('/', ".")),
        internal_name: owner.into(),
    }
}
impl ClassIndex for LazyHostClassIndex {
    fn find_classes(&self, prefix: &str, limit: usize) -> Vec<ClassIndexEntry> {
        self.provider
            .find_internal_names(prefix, limit)
            .iter()
            .map(|v| class_entry(v))
            .collect()
    }
    fn find_class(&self, owner: &str) -> Option<ClassIndexEntry> {
        self.provider
            .contains_internal_name(owner)
            .then(|| class_entry(owner))
    }
    fn find_class_by_fqn(&self, fqn: &str) -> Option<ClassIndexEntry> {
        self.provider
            .find_internal_name_by_fqn(fqn)
            .map(|v| class_entry(&v))
    }
    fn get_methods(&self, owner: &str) -> Vec<MethodIndexEntry> {
        match self.methods_result(owner) {
            Ok(HostFact::Known(values)) => values,
            _ => Vec::new(),
        }
    }
    fn get_fields(&self, owner: &str) -> Vec<FieldIndexEntry> {
        match self.fields_result(owner) {
            Ok(HostFact::Known(values)) => values,
            _ => Vec::new(),
        }
    }
}

/// Fallible class/index capability checks for command hosts.
///
/// `ClassIndex` is an intentionally compatibility-oriented trait: its methods
/// return empty values when a provider cannot answer.  Command dispatch must
/// not use those methods to decide whether a project is ready.  This adapter
/// keeps the checked path beside the same lazy binary index and optionally
/// consults the caller-owned source query when that query is authoritative.
pub struct HostCheckedClassIndex {
    binary: Arc<LazyHostClassIndex>,
    source: Option<Arc<dyn ProjectSourceQuery>>,
}
impl HostCheckedClassIndex {
    pub fn new(
        binary: Arc<LazyHostClassIndex>,
        source: Option<Arc<dyn ProjectSourceQuery>>,
    ) -> Self {
        Self { binary, source }
    }

    fn binary_find_class_result(
        &self,
        owner: &str,
    ) -> HostProjectResult<HostFact<Option<ClassIndexEntry>>> {
        let catalog = self.binary.provider.catalog_result()?;
        Ok(HostFact::Known(
            catalog
                .iter()
                .find(|candidate| candidate.as_str() == owner)
                .map(|_| class_entry(owner)),
        ))
    }

    /// Return a fallible class lookup.  A partial source answer is never
    /// promoted to a negative fact; the complete binary catalog is consulted
    /// instead, and an inconsistent source-present/binary-absent answer is an
    /// explicit failure.
    pub fn find_class_result(
        &self,
        owner: &str,
    ) -> HostProjectResult<HostFact<Option<ClassIndexEntry>>> {
        if let Some(source) = &self.source {
            let result = source.find_class(owner)?;
            if result.authority == SourceAuthority::Complete {
                return Ok(HostFact::Known(result.value));
            }
            let source_present = result.value.is_some();
            let binary = self.binary_find_class_result(owner)?;
            if source_present && matches!(binary, HostFact::Known(None)) {
                return Err(HostSourceError::Failure(format!(
                    "source class facts for {owner} are partial and binary class facts are missing"
                )));
            }
            return Ok(binary);
        }
        self.binary_find_class_result(owner)
    }

    /// Return a fallible catalog lookup.  Source values are used only when the
    /// source provider explicitly declares the result complete; otherwise the
    /// lazy provider's catalog remains the authority.
    pub fn find_classes_result(
        &self,
        prefix: &str,
        limit: usize,
    ) -> HostProjectResult<HostFact<Vec<ClassIndexEntry>>> {
        if let Some(source) = &self.source {
            let result = source.find_classes(prefix, limit)?;
            if result.authority == SourceAuthority::Complete {
                return Ok(HostFact::Known(
                    result.value.into_iter().take(limit).collect(),
                ));
            }
        }
        let prefix = prefix.to_lowercase();
        Ok(HostFact::Known(
            self.binary
                .provider
                .catalog_result()?
                .into_iter()
                .filter(|name| {
                    name.to_lowercase().starts_with(&prefix)
                        || name.replace('/', ".").to_lowercase().starts_with(&prefix)
                        || name
                            .rsplit('/')
                            .next()
                            .unwrap_or(name)
                            .to_lowercase()
                            .starts_with(&prefix)
                })
                .take(limit)
                .map(|name| class_entry(&name))
                .collect(),
        ))
    }

    /// Return a fallible fully-qualified class lookup.  The compatibility
    /// `LazyHostClassIndex` method uses `Option` and therefore cannot retain a
    /// catalog/read failure.  Command composition must keep that failure
    /// visible while still allowing an authoritative complete source answer
    /// to avoid a binary read.
    pub fn find_class_by_fqn_result(
        &self,
        fqn: &str,
    ) -> HostProjectResult<HostFact<Option<ClassIndexEntry>>> {
        if let Some(source) = &self.source {
            let result = source.find_class_by_fqn(fqn)?;
            if result.authority == SourceAuthority::Complete {
                return Ok(HostFact::Known(result.value));
            }
            let source_present = result.value.is_some();
            let catalog = self.binary.provider.catalog_result()?;
            let value = catalog
                .iter()
                .find(|candidate| candidate.replace('/', ".").eq_ignore_ascii_case(fqn))
                .map(|owner| class_entry(owner));
            if source_present && value.is_none() {
                return Err(HostSourceError::Failure(format!(
                    "source class facts for {fqn} are partial and binary class facts are missing"
                )));
            }
            return Ok(HostFact::Known(value));
        }
        let catalog = self.binary.provider.catalog_result()?;
        Ok(HostFact::Known(
            catalog
                .iter()
                .find(|candidate| candidate.replace('/', ".").eq_ignore_ascii_case(fqn))
                .map(|owner| class_entry(owner)),
        ))
    }

    fn binary_member_result<T>(
        &self,
        owner: &str,
        member: impl FnOnce(&LazyHostClassIndex) -> HostProjectResult<HostFact<Vec<T>>>,
    ) -> HostProjectResult<HostFact<Vec<T>>> {
        match self.binary_find_class_result(owner)? {
            HostFact::Known(Some(_)) => {}
            HostFact::Known(None) | HostFact::Missing => return Ok(HostFact::Missing),
            HostFact::Unavailable { reason } => {
                return Ok(HostFact::Unavailable { reason });
            }
        }
        if self.binary.provider.class_bytes_result(owner)?.is_none() {
            return Err(HostSourceError::Failure(format!(
                "class bytes for catalog owner {owner} are unavailable"
            )));
        }
        match member(&self.binary)? {
            HostFact::Known(values) => Ok(HostFact::Known(values)),
            HostFact::Missing => Err(HostSourceError::Failure(format!(
                "binary member facts for {owner} are unavailable"
            ))),
            HostFact::Unavailable { reason } => Ok(HostFact::Unavailable { reason }),
        }
    }

    /// Return fallible method facts.  A source member result is used only when
    /// its authority is complete; otherwise binary member facts are checked
    /// for both catalog presence and readable class bytes.
    pub fn get_methods_result(
        &self,
        owner: &str,
    ) -> HostProjectResult<HostFact<Vec<MethodIndexEntry>>> {
        let mut partial_source = false;
        if let Some(source) = &self.source {
            let class = source.find_class(owner)?;
            match class.authority {
                SourceAuthority::Complete => {
                    let Some(_) = class.value else {
                        return Ok(HostFact::Missing);
                    };
                    let methods = source.get_methods(owner)?;
                    if methods.authority == SourceAuthority::Complete {
                        return Ok(HostFact::Known(methods.value));
                    }
                    partial_source = true;
                }
                SourceAuthority::Partial if class.value.is_some() => partial_source = true,
                SourceAuthority::Partial | SourceAuthority::Unavailable => {}
            }
        }
        match self.binary_member_result(owner, |binary| binary.methods_result(owner))? {
            HostFact::Missing if partial_source => Err(HostSourceError::Failure(format!(
                "source method facts for {owner} are incomplete and binary class facts are missing"
            ))),
            result => Ok(result),
        }
    }

    /// Return fallible field facts with the same source-first/binary-checked
    /// policy as `get_methods_result`.
    pub fn get_fields_result(
        &self,
        owner: &str,
    ) -> HostProjectResult<HostFact<Vec<FieldIndexEntry>>> {
        let mut partial_source = false;
        if let Some(source) = &self.source {
            let class = source.find_class(owner)?;
            match class.authority {
                SourceAuthority::Complete => {
                    let Some(_) = class.value else {
                        return Ok(HostFact::Missing);
                    };
                    let fields = source.get_fields(owner)?;
                    if fields.authority == SourceAuthority::Complete {
                        return Ok(HostFact::Known(fields.value));
                    }
                    partial_source = true;
                }
                SourceAuthority::Partial if class.value.is_some() => partial_source = true,
                SourceAuthority::Partial | SourceAuthority::Unavailable => {}
            }
        }
        match self.binary_member_result(owner, |binary| binary.fields_result(owner))? {
            HostFact::Missing if partial_source => Err(HostSourceError::Failure(format!(
                "source field facts for {owner} are incomplete and binary class facts are missing"
            ))),
            result => Ok(result),
        }
    }

    /// Verify that the class and its member index are actually available.
    /// Catalog presence alone is deliberately insufficient: a lazy binary
    /// index can expose `find_class(Some)` before class bytes are readable.
    pub fn owner_result(&self, owner: &str) -> HostProjectResult<HostFact<ClassIndexEntry>> {
        let mut partial_source_class = false;
        if let Some(source) = &self.source {
            let class = source.find_class(owner)?;
            match class.authority {
                SourceAuthority::Complete => {
                    let Some(entry) = class.value else {
                        return Ok(HostFact::Missing);
                    };
                    let methods = self.get_methods_result(owner)?;
                    let fields = self.get_fields_result(owner)?;
                    if matches!(methods, HostFact::Known(_)) && matches!(fields, HostFact::Known(_))
                    {
                        return Ok(HostFact::Known(entry));
                    }
                    partial_source_class = true;
                }
                SourceAuthority::Partial if class.value.is_some() => {
                    partial_source_class = true;
                }
                SourceAuthority::Partial | SourceAuthority::Unavailable => {}
            }
        }

        let class = self.binary_find_class_result(owner)?;
        let entry = match class {
            HostFact::Known(Some(entry)) => entry,
            HostFact::Known(None) | HostFact::Missing => {
                if partial_source_class {
                    return Err(HostSourceError::Failure(format!(
                        "source class facts for {owner} are incomplete and binary class facts are missing"
                    )));
                }
                return Ok(HostFact::Missing);
            }
            HostFact::Unavailable { reason } => {
                return Ok(HostFact::Unavailable { reason });
            }
        };

        match self.binary.provider.class_bytes_result(owner)? {
            Some(_) => {
                let methods = self.binary.methods_result(owner)?;
                let fields = self.binary.fields_result(owner)?;
                if matches!(methods, HostFact::Known(_)) && matches!(fields, HostFact::Known(_)) {
                    Ok(HostFact::Known(entry))
                } else {
                    Err(HostSourceError::Failure(format!(
                        "binary member facts for {owner} are unavailable"
                    )))
                }
            }
            None => Err(HostSourceError::Failure(format!(
                "class bytes for catalog owner {owner} are unavailable"
            ))),
        }
    }
}

pub struct HostBytecodeAdapter {
    pub provider: Arc<HostClassBytesProvider>,
    pub class_index: Arc<dyn ClassIndex>,
    candidates: Mutex<Lru<Vec<AtTargetCandidate>>>,
    returns: Mutex<Lru<usize>>,
    hierarchy: Mutex<(u64, Arc<BytecodeCommonSuperClassResolver>)>,
}
impl HostBytecodeAdapter {
    pub fn new(provider: Arc<HostClassBytesProvider>, class_index: Arc<dyn ClassIndex>) -> Self {
        let hierarchy = Arc::new(BytecodeCommonSuperClassResolver::new(provider.clone()));
        Self {
            provider,
            class_index,
            candidates: Mutex::new(Lru::new(256)),
            returns: Mutex::new(Lru::new(256)),
            hierarchy: Mutex::new((0, hierarchy)),
        }
    }
    pub fn get_class_bytes_arc(&self, owner: &str) -> HostProjectResult<Option<Arc<[u8]>>> {
        self.provider.class_bytes_result(owner)
    }
    pub fn candidate_cache_size(&self) -> usize {
        self.candidates.lock().unwrap().values.len()
    }
    pub fn return_count_cache_size(&self) -> usize {
        self.returns.lock().unwrap().values.len()
    }
    pub fn common_super_result(
        &self,
        left: &str,
        right: &str,
    ) -> HostProjectResult<Option<String>> {
        let revision = self.provider.current_revision()?;
        let resolver = {
            let mut guard = self.hierarchy.lock().unwrap();
            if guard.0 != revision {
                *guard = (
                    revision,
                    Arc::new(BytecodeCommonSuperClassResolver::new(self.provider.clone())),
                );
            }
            guard.1.clone()
        };
        let result = resolver.resolve(left, right);
        if self.provider.current_revision()? != revision {
            return Err(HostSourceError::Failure(
                "hierarchy changed during evaluation".into(),
            ));
        }
        Ok(result)
    }
    pub fn clear_cache(&self) {
        self.candidates.lock().unwrap().clear();
        self.returns.lock().unwrap().clear();
    }
    fn extract(
        &self,
        owner: &str,
        name: &str,
        desc: &str,
    ) -> HostProjectResult<Vec<AtTargetCandidate>> {
        match self.provider.class_bytes_result(owner)? {
            Some(bytes) => extract_instructions(&bytes, name, desc)
                .map_err(|e| HostSourceError::Failure(e.to_string())),
            None => Ok(Vec::new()),
        }
    }
    fn convert(&self, mut c: AtTargetCandidate) -> AtTargetCandidate {
        let entry = self.class_index.find_class(&c.owner);
        c.detail = entry
            .as_ref()
            .map(|v| v.package_name.clone())
            .filter(|v| !v.is_empty())
            .unwrap_or_default();
        if entry.as_ref().is_none_or(|v| v.package_name.is_empty()) {
            let fqn = c.owner.replace('/', ".").replace('$', ".");
            c.detail = fqn.rsplit_once('.').map_or(fqn.clone(), |(v, _)| v.into());
        }
        let signature = descriptor::parse_method_descriptor(&c.descriptor)
            .map(|v| format!("{}{}", c.name, descriptor::render_method(&v)))
            .unwrap_or_else(|_| format!("{}{}", c.name, c.descriptor));
        c.display_label = match c.kind {
            AtTargetKind::Field => format!(
                "{}: {}",
                c.name,
                descriptor::parse_field_descriptor(&c.descriptor)
                    .map(|v| descriptor::render(&v))
                    .unwrap_or_else(|_| c.descriptor.clone())
            ),
            AtTargetKind::New => {
                if c.name == "<init>" {
                    format!(
                        "new {}",
                        entry.as_ref().map_or_else(
                            || c.owner.rsplit('/').next().unwrap_or("").to_owned(),
                            |v| v.simple_name.clone()
                        )
                    )
                } else {
                    c.name.clone()
                }
            }
            _ => {
                if c.name == "<init>" {
                    format!(
                        "new {}{signature}",
                        entry.as_ref().map_or("", |v| v.simple_name.as_str())
                    )
                } else {
                    signature
                }
            }
        };
        c.namespace = MappingNamespace::Named;
        // The legacy project adapter materializes the public candidate rather
        // than forwarding the extractor's conditional-constant opcode.
        c.condition_opcode = None;
        c
    }
}
fn at_matches(kind: AtTargetKind, value: &str) -> bool {
    match value.to_uppercase().as_str() {
        "INVOKE" | "INVOKE_ASSIGN" => kind == AtTargetKind::Invoke,
        "FIELD" => kind == AtTargetKind::Field,
        "NEW" => kind == AtTargetKind::New,
        "RETURN" => kind == AtTargetKind::Return,
        "CONSTANT" => kind == AtTargetKind::Constant,
        _ => false,
    }
}
impl HostBytecodeAdapter {
    pub fn candidates_result(
        &self,
        owner: &str,
        name: &str,
        desc: Option<&str>,
        value: &str,
    ) -> HostProjectResult<Vec<AtTargetCandidate>> {
        let rev = self.provider.current_revision()?;
        let key = format!("{rev}#{owner}#{name}#{}#{value}", desc.unwrap_or(""));
        if let Some(result) = self.candidates.lock().unwrap().get(&key) {
            return Ok(result);
        }
        let descriptors = if let Some(desc) = desc.filter(|v| !v.is_empty()) {
            vec![desc.into()]
        } else {
            let mut d = self
                .class_index
                .get_methods(owner)
                .into_iter()
                .filter(|m| m.name == name)
                .map(|m| m.descriptor)
                .collect::<Vec<_>>();
            d.sort();
            d
        };
        // ClassIndex's compatibility API cannot distinguish an absent method
        // from unavailable source facts. Do not persist that as a negative fact.
        if descriptors.is_empty() {
            return Err(HostSourceError::Failure(
                "method facts unavailable or no matching overload".into(),
            ));
        }
        let mut out = Vec::new();
        for desc in descriptors {
            for candidate in self.extract(owner, name, &desc)? {
                if at_matches(candidate.kind, value) {
                    let c = self.convert(candidate);
                    if !out.iter().any(|old: &AtTargetCandidate| {
                        old.owner == c.owner
                            && old.name == c.name
                            && old.descriptor == c.descriptor
                            && old.kind == c.kind
                            && old.operation_kind == c.operation_kind
                            && old.ordinal == c.ordinal
                            && old.constant_value == c.constant_value
                            && old.instruction_occurrence_index == c.instruction_occurrence_index
                            && old.occurrence_result_classification
                                == c.occurrence_result_classification
                    }) {
                        out.push(c)
                    }
                }
            }
        }
        out.sort_by(|a, b| {
            (
                &a.owner,
                &a.name,
                &a.descriptor,
                a.kind as u8,
                a.operation_kind.map(|v| v as u8),
                a.ordinal,
                a.instruction_occurrence_index,
            )
                .cmp(&(
                    &b.owner,
                    &b.name,
                    &b.descriptor,
                    b.kind as u8,
                    b.operation_kind.map(|v| v as u8),
                    b.ordinal,
                    b.instruction_occurrence_index,
                ))
        });
        if self.provider.current_revision()? != rev {
            return Err(HostSourceError::Failure(
                "project changed during candidate computation".into(),
            ));
        }
        let mut cache = self.candidates.lock().unwrap();
        if let Some(existing) = cache.get(&key) {
            return Ok(existing);
        }
        cache.put(key, out.clone());
        Ok(out)
    }
    pub fn return_count_result(
        &self,
        owner: &str,
        name: &str,
        desc: Option<&str>,
    ) -> HostProjectResult<usize> {
        let rev = self.provider.current_revision()?;
        let key = format!("{rev}#{owner}#{name}#{}", desc.unwrap_or(""));
        if let Some(count) = self.returns.lock().unwrap().get(&key) {
            return Ok(count);
        }
        let descriptor = desc
            .map(str::to_owned)
            .or_else(|| {
                self.class_index
                    .get_methods(owner)
                    .into_iter()
                    .find(|m| m.name == name)
                    .map(|m| m.descriptor)
            })
            .unwrap_or_else(|| "()V".into());
        let count = self
            .extract(owner, name, &descriptor)?
            .iter()
            .filter(|c| c.kind == AtTargetKind::Return)
            .count()
            .max(1);
        if self.provider.current_revision()? != rev {
            return Err(HostSourceError::Failure(
                "project changed during return count computation".into(),
            ));
        }
        self.returns.lock().unwrap().put(key, count);
        Ok(count)
    }
}
impl BytecodeIndex for HostBytecodeAdapter {
    fn get_at_target_candidates(
        &self,
        owner: &str,
        name: &str,
        desc: Option<&str>,
        value: &str,
    ) -> Vec<AtTargetCandidate> {
        self.candidates_result(owner, name, desc, value)
            .unwrap_or_default()
    }
    fn get_return_ordinal_count(&self, owner: &str, name: &str, desc: Option<&str>) -> usize {
        self.return_count_result(owner, name, desc).unwrap_or(1)
    }
    fn get_class_bytes(&self, owner: &str) -> Option<Vec<u8>> {
        self.provider.get_class_bytes(owner)
    }
    fn resolve_common_super_class(&self, left: &str, right: &str) -> Option<String> {
        self.common_super_result(left, right).ok().flatten()
    }
}

pub struct HostProjectSession {
    pub context: Arc<ProjectContext>,
    pub identity: HostSessionIdentity,
    pub class_bytes_provider: Arc<HostClassBytesProvider>,
    pub binary_index: Arc<LazyHostClassIndex>,
    pub checked_class_index: Arc<HostCheckedClassIndex>,
    pub class_index: Arc<dyn ClassIndex>,
    pub bytecode_index: Arc<HostBytecodeAdapter>,
    overlay: Arc<HostClassIndexOverlay>,
    checked_source: Option<Arc<dyn ProjectSourceQuery>>,
}
pub type HostClassIndexOverlay = dyn Fn(Arc<dyn ClassIndex>) -> Arc<dyn ClassIndex> + Send + Sync;
struct SessionClassIndex {
    provider: Arc<HostClassBytesProvider>,
    index: Arc<dyn ClassIndex>,
}
impl ClassIndex for SessionClassIndex {
    fn find_classes(&self, prefix: &str, limit: usize) -> Vec<ClassIndexEntry> {
        if self.provider.current_revision().is_err() {
            Vec::new()
        } else {
            self.index.find_classes(prefix, limit)
        }
    }
    fn find_class(&self, owner: &str) -> Option<ClassIndexEntry> {
        self.provider.current_revision().ok()?;
        self.index.find_class(owner)
    }
    fn find_class_by_fqn(&self, fqn: &str) -> Option<ClassIndexEntry> {
        self.provider.current_revision().ok()?;
        self.index.find_class_by_fqn(fqn)
    }
    fn get_methods(&self, owner: &str) -> Vec<MethodIndexEntry> {
        if self.provider.current_revision().is_err() {
            Vec::new()
        } else {
            self.index.get_methods(owner)
        }
    }
    fn get_fields(&self, owner: &str) -> Vec<FieldIndexEntry> {
        if self.provider.current_revision().is_err() {
            Vec::new()
        } else {
            self.index.get_fields(owner)
        }
    }
}
impl HostProjectSession {
    pub fn create(
        context: Arc<ProjectContext>,
        identity: HostSessionIdentity,
        host: Arc<dyn HostProjectQuery>,
    ) -> Self {
        Self::with_class_index(context, identity, host, Arc::new(|binary| binary))
    }
    pub fn with_class_index(
        context: Arc<ProjectContext>,
        identity: HostSessionIdentity,
        host: Arc<dyn HostProjectQuery>,
        overlay: Arc<HostClassIndexOverlay>,
    ) -> Self {
        Self::with_class_index_and_source(context, identity, host, overlay, None)
    }

    /// Build a session whose fallible index checks use the same caller-owned
    /// source query as the source overlay.  Existing `with_class_index` and
    /// `create` callers retain their compatibility index behavior and receive
    /// checked binary fallback automatically.
    pub fn with_checked_source_index(
        context: Arc<ProjectContext>,
        identity: HostSessionIdentity,
        host: Arc<dyn HostProjectQuery>,
        overlay: Arc<HostClassIndexOverlay>,
        source: Arc<dyn ProjectSourceQuery>,
    ) -> Self {
        Self::with_class_index_and_source(context, identity, host, overlay, Some(source))
    }

    fn with_class_index_and_source(
        context: Arc<ProjectContext>,
        identity: HostSessionIdentity,
        host: Arc<dyn HostProjectQuery>,
        overlay: Arc<HostClassIndexOverlay>,
        checked_source: Option<Arc<dyn ProjectSourceQuery>>,
    ) -> Self {
        let entries = identity
            .facts
            .classpath
            .all_entries()
            .into_iter()
            .map(|path| HostClasspathEntry {
                id: path.to_string_lossy().into_owned(),
                timestamp: identity
                    .facts
                    .classpath
                    .entry_timestamps
                    .get(&path)
                    .map(|v| *v as i64),
                path,
            })
            .collect();
        let mut context = (*context).clone();
        context.classpath = identity.facts.classpath.clone();
        context.source_sets = identity.facts.source_sets.clone();
        context.index_state = if context.classpath.entry_count() > 0 {
            ProjectIndexState::Ready
        } else {
            ProjectIndexState::NotReady
        };
        let provider = Arc::new(HostClassBytesProvider::new(identity.clone(), entries, host));
        let binary = Arc::new(LazyHostClassIndex::new(provider.clone()));
        let class_index: Arc<dyn ClassIndex> = Arc::new(SessionClassIndex {
            provider: provider.clone(),
            index: overlay(binary.clone()),
        });
        let checked_class_index = Arc::new(HostCheckedClassIndex::new(
            binary.clone(),
            checked_source.clone(),
        ));
        let bytecode = Arc::new(HostBytecodeAdapter::new(
            provider.clone(),
            class_index.clone(),
        ));
        Self {
            context: Arc::new(context),
            identity,
            class_bytes_provider: provider,
            binary_index: binary,
            checked_class_index,
            class_index,
            bytecode_index: bytecode,
            overlay,
            checked_source,
        }
    }
    pub fn reindex(&self) -> Self {
        Self::with_class_index_and_source(
            self.context.clone(),
            self.identity.clone(),
            self.class_bytes_provider.host.clone(),
            self.overlay.clone(),
            self.checked_source.clone(),
        )
    }
    pub fn close(&self) {
        self.class_bytes_provider.close();
    }
    pub fn definition_result(
        &self,
        target: &McDefinitionTarget,
    ) -> HostProjectResult<HostFact<crate::mixin_navigation::ResolvedDefinition>> {
        self.class_bytes_provider.current_revision()?;
        let result = self
            .class_bytes_provider
            .host
            .definition(&self.identity, target)?;
        self.class_bytes_provider.accept(result)
    }
    pub fn reference_types(&self) -> HostReferenceTypes {
        HostReferenceTypes {
            provider: self.class_bytes_provider.clone(),
        }
    }

    pub fn checked_reference_types(&self) -> HostCheckedReferenceTypes {
        HostCheckedReferenceTypes {
            provider: self.class_bytes_provider.clone(),
        }
    }
}

/// The host, rather than a local SDK-name table or reflection, decides reference
/// existence and assignability. Unknown remains None at the handler boundary.
pub struct HostReferenceTypes {
    pub provider: Arc<HostClassBytesProvider>,
}
impl HostReferenceTypes {
    pub fn reference_type_result(
        &self,
        owner: &str,
    ) -> HostProjectResult<HostFact<HostReferenceTypeFact>> {
        self.provider.current_revision()?;
        self.provider.accept(
            self.provider
                .host
                .reference_type(&self.provider.identity, owner)?,
        )
    }
    pub fn assignability_result(&self, from: &str, to: &str) -> HostProjectResult<HostFact<bool>> {
        self.provider.current_revision()?;
        self.provider.accept(
            self.provider
                .host
                .is_assignable(&self.provider.identity, from, to)?,
        )
    }
}

/// Strict facade adapter for reference facts.  The compatibility trait uses
/// `Option<bool>` by design, so an unavailable host answer must leave that
/// boundary as an error rather than becoming `None` and later a false/empty
/// core result.  The facade's existing panic boundary converts the preserved
/// `HostSourceError` back into a typed command failure.
pub struct HostCheckedReferenceTypes {
    pub provider: Arc<HostClassBytesProvider>,
}
impl HostCheckedReferenceTypes {
    fn reference_types(&self) -> HostReferenceTypes {
        HostReferenceTypes {
            provider: self.provider.clone(),
        }
    }
}
impl crate::mixinextras_handler::HandlerReferenceTypeResolver for HostCheckedReferenceTypes {
    fn is_known_reference(&self, owner: &str) -> Option<bool> {
        match self.reference_types().reference_type_result(owner) {
            Ok(HostFact::Known(_)) => Some(true),
            Ok(HostFact::Missing) => Some(false),
            Ok(HostFact::Unavailable { reason }) => {
                std::panic::panic_any(HostSourceError::Failure(reason))
            }
            Err(error) => std::panic::panic_any(error),
        }
    }

    fn is_assignable(&self, from: &str, to: &str) -> Option<bool> {
        match self.reference_types().assignability_result(from, to) {
            Ok(HostFact::Known(value)) => Some(value),
            Ok(HostFact::Missing) => Some(false),
            Ok(HostFact::Unavailable { reason }) => {
                std::panic::panic_any(HostSourceError::Failure(reason))
            }
            Err(error) => std::panic::panic_any(error),
        }
    }
}
impl crate::mixinextras_handler::HandlerReferenceTypeResolver for HostReferenceTypes {
    fn is_known_reference(&self, owner: &str) -> Option<bool> {
        match self.reference_type_result(owner).ok()? {
            HostFact::Known(_) => Some(true),
            HostFact::Missing => Some(false),
            HostFact::Unavailable { .. } => None,
        }
    }
    fn is_assignable(&self, from: &str, to: &str) -> Option<bool> {
        match self.assignability_result(from, to).ok()? {
            HostFact::Known(value) => Some(value),
            _ => None,
        }
    }
}
impl crate::mixin_navigation::DefinitionBackend for HostProjectSession {
    fn resolve(
        &self,
        target: &McDefinitionTarget,
        context: &ProjectContext,
        workspace_root_uri: &str,
    ) -> Option<crate::mixin_navigation::ResolvedDefinition> {
        if context.project_id != self.context.project_id
            || normalize_path(&context.root) != normalize_path(&self.context.root)
            || uri_to_path(workspace_root_uri)
                .ok()
                .map(|p| normalize_path(&p))
                != uri_to_path(&self.identity.workspace_root_uri)
                    .ok()
                    .map(|p| normalize_path(&p))
        {
            return None;
        }
        match self.definition_result(target).ok()? {
            HostFact::Known(value) => Some(value),
            _ => None,
        }
    }
}

/// Resource contexts and binary/source adapters share the same host session
/// version. Replacing a session closes all its old lazy adapters together.
#[derive(Default)]
pub struct HostProjectSessions {
    sessions: Mutex<BTreeMap<PathBuf, Arc<HostProjectSession>>>,
}
impl HostProjectSessions {
    pub fn load(
        &self,
        cached: &CachedProjectSession,
        identity: HostSessionIdentity,
        host: Arc<dyn HostProjectQuery>,
        overlay: Arc<HostClassIndexOverlay>,
    ) -> HostProjectResult<(Arc<HostProjectSession>, bool)> {
        let root = normalize_path(
            &uri_to_path(&identity.workspace_root_uri).map_err(HostSourceError::Failure)?,
        );
        let facts = &identity.facts;
        let generations = identity.generations;
        if cached.version != identity.project_session_version
            || normalize_path(&cached.context.root) != root
            || generations.project != facts.project_generation
            || generations.dependency != facts.dependency_generation
            || generations.generated_source != facts.generated_source_generation
            || generations.config != facts.config_generation
            || generations.provider != facts.provider_generation
        {
            return Err(HostSourceError::Failure(
                "context and host session identity do not match".into(),
            ));
        }
        if !host.is_open(&identity) || !host.validate(&identity, &Vec::new())? {
            return Err(HostSourceError::Failure(
                "host session is closed or stale".into(),
            ));
        }
        let existing = self.sessions.lock().unwrap().get(&root).cloned();
        if let Some(session) = existing {
            if session.identity == identity
                && !session.class_bytes_provider.closed.load(Ordering::Acquire)
            {
                session.class_bytes_provider.current_revision()?;
                return Ok((session, true));
            }
        }
        let session = Arc::new(HostProjectSession::with_class_index(
            cached.context.clone(),
            identity,
            host,
            overlay,
        ));
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(existing) = sessions.get(&root) {
            if existing.identity == session.identity
                && !existing.class_bytes_provider.closed.load(Ordering::Acquire)
            {
                return Ok((existing.clone(), true));
            }
        }
        let old = sessions.insert(root, session.clone());
        drop(sessions);
        if let Some(old) = old {
            old.close();
        }
        Ok((session, false))
    }
    pub fn clear(&self, workspace_root_uri: &str) -> HostProjectResult<()> {
        let root =
            normalize_path(&uri_to_path(workspace_root_uri).map_err(HostSourceError::Failure)?);
        let old = self.sessions.lock().unwrap().remove(&root);
        if let Some(old) = old {
            old.close();
        }
        Ok(())
    }
    pub fn clear_all(&self) {
        let sessions = std::mem::take(&mut *self.sessions.lock().unwrap());
        for session in sessions.values() {
            session.close();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostClasspathKind {
    ProjectOutput,
    DependencyJar,
    MinecraftJar,
    GeneratedOutput,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostClasspathPath {
    pub path: PathBuf,
    pub exists: bool,
    pub modified_at: u64,
    pub kind: HostClasspathKind,
}
pub struct HostProjectBridge;
impl HostProjectBridge {
    pub fn merge_classpath(
        base: &ClasspathSnapshot,
        extras: &[HostClasspathPath],
        captured_at: u64,
    ) -> ClasspathSnapshot {
        if extras.is_empty() {
            return base.clone();
        }
        let mut result = base.clone();
        for entry in extras.iter().filter(|e| e.exists) {
            let list = match entry.kind {
                HostClasspathKind::ProjectOutput => &mut result.project_outputs,
                HostClasspathKind::DependencyJar => &mut result.dependency_jars,
                HostClasspathKind::MinecraftJar => &mut result.minecraft_jars,
                HostClasspathKind::GeneratedOutput => &mut result.generated_outputs,
            };
            if !list.contains(&entry.path) {
                list.push(entry.path.clone());
            }
            result
                .entry_timestamps
                .entry(entry.path.clone())
                .or_insert(entry.modified_at);
        }
        result.captured_at = captured_at;
        result
    }
    pub fn selected_loom_jars(root: &Path, selected: &[HostClasspathPath]) -> Vec<PathBuf> {
        let roots = [
            root.join(".gradle/loom-cache/remapped_working"),
            root.join("build/loom-cache/remapped_working"),
        ];
        let mut out = Vec::new();
        for root in roots {
            for entry in selected {
                if entry.exists
                    && matches!(
                        entry.kind,
                        HostClasspathKind::MinecraftJar | HostClasspathKind::DependencyJar
                    )
                    && entry
                        .path
                        .extension()
                        .is_some_and(|v| v.eq_ignore_ascii_case("jar"))
                    && entry.path.starts_with(&root)
                    && !out.contains(&entry.path)
                {
                    out.push(entry.path.clone());
                }
            }
        }
        out
    }
}
/// Signature strings are host facts, never reflective method/type handles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostMethodSignature {
    pub name: String,
    pub parameter_types: Option<Vec<String>>,
}
pub fn select_host_method<'a>(
    methods: &'a [HostMethodSignature],
    name: &str,
    descriptor: Option<&str>,
    signature_available: bool,
) -> Option<&'a HostMethodSignature> {
    let matches = methods
        .iter()
        .filter(|m| m.name == name)
        .collect::<Vec<_>>();
    let Some(descriptor) = descriptor.filter(|v| !v.trim().is_empty()) else {
        return matches.first().copied();
    };
    if !signature_available {
        return None;
    }
    let expected = descriptor::parse_method_descriptor(descriptor)
        .ok()?
        .parameters
        .iter()
        .map(descriptor::to_descriptor)
        .map(|v| normalize_host_signature(&v))
        .collect::<Option<Vec<_>>>()?;
    let mut matches = matches.into_iter().filter(|m| {
        m.parameter_types
            .as_ref()
            .and_then(|v| {
                v.iter()
                    .map(|v| normalize_host_signature(v))
                    .collect::<Option<Vec<_>>>()
            })
            .is_some_and(|v| v == expected)
    });
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}
pub fn normalize_host_signature(raw: &str) -> Option<String> {
    let mut erased = raw.to_owned();
    if let Some(start) = erased.find('<') {
        let end = erased.rfind('>')?;
        if end <= start {
            return None;
        }
        erased.replace_range(start..=end, "");
    }
    let arrays = erased.bytes().take_while(|b| *b == b'[').count();
    let base = &erased[arrays..];
    let normalized = if base.starts_with('L') && base.ends_with(';') {
        base.to_owned()
    } else if base.starts_with('Q') && base.ends_with(';') {
        let name = &base[1..base.len() - 1];
        if !name.contains(['.', '/']) {
            return None;
        }
        format!("L{name};")
    } else if base.len() == 1 && "BCDFIJSZV".contains(base) {
        base.into()
    } else {
        return None;
    };
    Some(format!(
        "{}{}",
        "[".repeat(arrays),
        normalized.replace('/', ".")
    ))
}
