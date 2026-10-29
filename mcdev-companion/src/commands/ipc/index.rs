//! JVM facts remain scoped to a single host-owned command evaluation.
use crate::{
    host_facts::{
        bridge::FactReader,
        draft::{RawAbi, TypeJvm},
        inputs::{CatalogEntry, ScopedInputs},
        FactsError,
    },
    provider_commands::CommandFacts,
    wire::{Fact, HandleKind, Registry},
};
use mcdev_core::{
    bytecode::{
        extract_instructions, AtTargetCandidate, BytecodeAtTargetAdapter, BytecodeIndex,
        ClassMemberIndex, InMemoryClassBytesProvider,
    },
    descriptor::{self, JvmType},
    host_source::{ProjectSourceQuery, SourceAuthority},
    mixinextras_handler::HandlerReferenceTypeResolver,
    model::{ClassIndex, ClassIndexEntry, FieldIndexEntry, MethodIndexEntry},
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{Arc, Mutex},
};

/// A reader's cached facts may only be used while its host evaluation is live.
pub trait EvaluationFactReader: FactReader + Send + Sync {
    fn check_active(&self) -> Result<(), FactsError>;
}

impl EvaluationFactReader for CommandFacts {
    fn check_active(&self) -> Result<(), FactsError> {
        CommandFacts::check_active(self)
    }
}

#[derive(Default)]
struct Cache {
    catalogs: BTreeMap<String, Vec<CatalogEntry>>,
    abis: BTreeMap<String, RawAbi>,
    bytes: BTreeMap<String, RuntimeBytes>,
    selected_origins: BTreeMap<String, String>,
    host_types: BTreeMap<String, EditingView>,
}

#[derive(Clone)]
struct RuntimeBytes {
    origin: String,
    bytes: Vec<u8>,
}

#[derive(Clone, PartialEq, Eq)]
struct EditingView {
    methods: Option<Vec<MethodIndexEntry>>,
    fields: Option<Vec<FieldIndexEntry>>,
}

#[derive(Clone, Copy)]
enum HierarchyView {
    Current,
    Physical,
}

pub struct EvaluationIndex<R: EvaluationFactReader = CommandFacts> {
    inputs: Arc<Mutex<ScopedInputs<R>>>,
    facts: R,
    cache: Mutex<Cache>,
    source: Option<Arc<dyn ProjectSourceQuery>>,
}

/// A physical-only view. Its negative results say nothing about current source.
pub struct PhysicalIndex<R: EvaluationFactReader = CommandFacts> {
    index: Arc<EvaluationIndex<R>>,
}

impl<R: EvaluationFactReader> EvaluationIndex<R> {
    pub fn new(inputs: Arc<Mutex<ScopedInputs<R>>>, facts: R) -> Self {
        Self {
            inputs,
            facts,
            cache: Mutex::new(Cache::default()),
            source: None,
        }
    }

    /// The caller must supply source facts derived from the same host evaluation.
    pub fn with_source_query(mut self, source: Arc<dyn ProjectSourceQuery>) -> Self {
        self.source = Some(source);
        self
    }

    pub fn physical_view(self: &Arc<Self>) -> PhysicalIndex<R> {
        PhysicalIndex {
            index: Arc::clone(self),
        }
    }

    fn with_inputs<T>(
        &self,
        read: impl FnOnce(&mut ScopedInputs<R>) -> Result<T, FactsError>,
    ) -> Result<T, FactsError> {
        self.facts.check_active()?;
        let result = {
            let mut inputs = self
                .inputs
                .lock()
                .map_err(|_| protocol("host input lock poisoned"))?;
            read(&mut inputs)
        };
        self.facts.check_active()?;
        result
    }

    fn cache(&self) -> Result<std::sync::MutexGuard<'_, Cache>, FactsError> {
        self.cache
            .lock()
            .map_err(|_| protocol("JVM cache lock poisoned"))
    }

    fn physical_catalog(&self, prefix: &str) -> Result<Vec<CatalogEntry>, FactsError> {
        self.facts.check_active()?;
        if prefix.len() > 4096
            || prefix.starts_with('/')
            || prefix.contains("//")
            || prefix.contains(['\\', '.', ':', ';', '[', ']'])
            || prefix.chars().any(char::is_control)
        {
            return Err(protocol("invalid JVM catalog prefix"));
        }
        let cached = {
            let cache = self.cache()?;
            cache.catalogs.get(prefix).cloned().or_else(|| {
                cache.catalogs.get("").map(|all| {
                    all.iter()
                        .filter(|entry| entry.descriptor.internal_name.starts_with(prefix))
                        .cloned()
                        .collect()
                })
            })
        };
        if let Some(entries) = cached {
            self.facts.check_active()?;
            return Ok(entries);
        }
        let catalog = self.with_inputs(|inputs| inputs.jvm_catalog(prefix))?;
        match catalog.coverage {
            Fact::Found { value: Value::Null } => (),
            Fact::Unknown { reasons } => return Err(unknown_fact("JVM catalog coverage", reasons)),
            _ => return Err(protocol("JVM catalog has invalid terminal coverage")),
        }
        self.facts.check_active()?;
        self.cache()?
            .catalogs
            .insert(prefix.to_owned(), catalog.entries.clone());
        self.facts.check_active()?;
        Ok(catalog.entries)
    }

    fn selected_entry(&self, owner: &str) -> Result<Option<CatalogEntry>, FactsError> {
        let candidates: Vec<_> = self
            .physical_catalog(owner)?
            .into_iter()
            .filter(|entry| entry.descriptor.internal_name == owner)
            .collect();
        self.select_candidate(owner, candidates)
    }

    fn select_candidate(
        &self,
        owner: &str,
        candidates: Vec<CatalogEntry>,
    ) -> Result<Option<CatalogEntry>, FactsError> {
        match candidates.as_slice() {
            [] => Ok(None),
            [only] => Ok(Some(only.clone())),
            _ => {
                self.facts.check_active()?;
                let selected = self.cache()?.selected_origins.get(owner).cloned();
                self.facts.check_active()?;
                let Some(selected) = selected else {
                    return Err(unknown(format!("multiple selected JVM inputs for {owner}; host type selection is unavailable")));
                };
                let matches: Vec<_> = candidates
                    .into_iter()
                    .filter(|entry| entry.descriptor.origin_id == selected)
                    .collect();
                match matches.as_slice() {
                    [only] => Ok(Some(only.clone())),
                    _ => Err(unknown(format!(
                        "host origin does not select one JVM input for {owner}"
                    ))),
                }
            }
        }
    }

    fn class_abi(&self, entry: &CatalogEntry) -> Result<RawAbi, FactsError> {
        self.facts.check_active()?;
        if let Some(abi) = self.cache()?.abis.get(&entry.descriptor.entry).cloned() {
            self.facts.check_active()?;
            return Ok(abi);
        }
        let result = self.with_inputs(|inputs| inputs.jvm_classes(std::slice::from_ref(entry)))?;
        let fact = result
            .into_iter()
            .next()
            .ok_or_else(|| protocol("missing JVM ABI result"))?;
        let abi = match fact {
            Fact::Found { value } => value.abi,
            Fact::Unknown { reasons } => return Err(unknown_fact("JVM ABI", reasons)),
            Fact::DefinitelyAbsent => {
                return Err(protocol("issued JVM entry cannot establish absence"))
            }
        };
        self.facts.check_active()?;
        self.cache()?
            .abis
            .insert(entry.descriptor.entry.clone(), abi.clone());
        self.facts.check_active()?;
        Ok(abi)
    }

    /// Preserve the exact physical ABI, including private, bridge and synthetic flags.
    pub fn raw_abi(&self, owner: &str) -> Result<RawAbi, FactsError> {
        let entry = self.selected_entry(owner)?.ok_or_else(|| {
            unknown(format!(
                "current source or generated class {owner} is unknown"
            ))
        })?;
        self.class_abi(&entry)
    }

    /// Bind an ambiguous physical name only to an origin issued by host type resolution.
    pub fn bind_host_type(&self, observed: &TypeJvm) -> Result<(), FactsError> {
        self.facts.check_active()?;
        observed.binary_name.validate().map_err(protocol)?;
        observed.abi.validate().map_err(protocol)?;
        let binary = observed
            .binary_name
            .found()
            .ok_or_else(|| unknown("host JVM type name is unknown"))?;
        if descriptor::parse_field_descriptor(&binary.descriptor).ok()
            != Some(JvmType::ObjectType(binary.internal_name.clone()))
        {
            return Err(protocol("host type name and ABI identity disagree"));
        }
        let editing = editing_view(observed)?;
        {
            let mut cache = self.cache()?;
            if cache
                .host_types
                .get(&binary.internal_name)
                .is_some_and(|previous| previous != &editing)
            {
                return Err(FactsError::Stale(
                    "host type members changed within an evaluation".into(),
                ));
            }
            cache
                .host_types
                .insert(binary.internal_name.clone(), editing);
        }
        self.facts.check_active()?;
        let Some(abi) = observed.abi.found() else {
            return Ok(());
        };
        if abi.identity.internal_name != binary.internal_name {
            return Err(protocol("host type name and ABI identity disagree"));
        }
        let matches: Vec<_> = self
            .physical_catalog(&binary.internal_name)?
            .into_iter()
            .filter(|entry| {
                entry.descriptor.internal_name == binary.internal_name
                    && entry.descriptor.origin_id == abi.identity.origin_id
            })
            .collect();
        let [entry] = matches.as_slice() else {
            return Err(unknown(
                "host origin does not identify one selected JVM entry",
            ));
        };
        if self.class_abi(entry)?.identity != abi.identity {
            return Err(unknown(
                "host type and selected physical class content differ",
            ));
        }
        self.facts.check_active()?;
        let mut cache = self.cache()?;
        if cache
            .selected_origins
            .get(&binary.internal_name)
            .is_some_and(|previous| previous != &abi.identity.origin_id)
        {
            return Err(FactsError::Stale(
                "host type selection changed within an evaluation".into(),
            ));
        }
        cache
            .selected_origins
            .insert(binary.internal_name.clone(), abi.identity.origin_id.clone());
        drop(cache);
        self.facts.check_active()
    }

    fn source_class(&self, owner: &str) -> Result<Option<ClassIndexEntry>, FactsError> {
        let source = self
            .source
            .as_ref()
            .ok_or_else(|| unknown(format!("current source ownership for {owner} is unknown")))?;
        let answer = source
            .find_class(owner)
            .map_err(|error| FactsError::from_core(error, false))?;
        if answer.authority != SourceAuthority::Complete {
            return Err(unknown(format!(
                "current source ownership for {owner} is incomplete"
            )));
        }
        self.facts.check_active()?;
        Ok(answer.value)
    }

    fn class_exists(&self, owner: &str) -> Result<Option<ClassIndexEntry>, FactsError> {
        self.facts.check_active()?;
        if let Some(source) = &self.source {
            if let Some(entry) = self.source_class(owner)? {
                return Ok(Some(entry));
            }
            if source.covers_project_dependencies() {
                return Ok(None);
            }
        } else {
            let selected = { self.cache()?.host_types.contains_key(owner) };
            self.facts.check_active()?;
            if selected {
                return Ok(Some(class_entry(owner)));
            }
            return Err(unknown(format!(
                "current source ownership for {owner} is unknown"
            )));
        }
        match self.selected_entry(owner)? {
            Some(_) => Ok(Some(class_entry(owner))),
            None => Ok(None),
        }
    }

    fn methods(&self, owner: &str) -> Result<Vec<MethodIndexEntry>, FactsError> {
        if self.source_class(owner)?.is_some() {
            let source = self.source.as_ref().expect("source authority was checked");
            let answer = source
                .get_methods(owner)
                .map_err(|error| FactsError::from_core(error, false))?;
            if answer.authority != SourceAuthority::Complete {
                return Err(unknown(format!(
                    "current source members for {owner} are incomplete"
                )));
            }
            self.facts.check_active()?;
            return Ok(answer.value);
        }
        if self
            .source
            .as_ref()
            .is_some_and(|source| source.covers_project_dependencies())
            || self.selected_entry(owner)?.is_none()
        {
            return Ok(Vec::new());
        }
        self.physical_methods(owner)
    }

    fn fields(&self, owner: &str) -> Result<Vec<FieldIndexEntry>, FactsError> {
        if self.source_class(owner)?.is_some() {
            let source = self.source.as_ref().expect("source authority was checked");
            let answer = source
                .get_fields(owner)
                .map_err(|error| FactsError::from_core(error, false))?;
            if answer.authority != SourceAuthority::Complete {
                return Err(unknown(format!(
                    "current source fields for {owner} are incomplete"
                )));
            }
            self.facts.check_active()?;
            return Ok(answer.value);
        }
        if self
            .source
            .as_ref()
            .is_some_and(|source| source.covers_project_dependencies())
            || self.selected_entry(owner)?.is_none()
        {
            return Ok(Vec::new());
        }
        self.physical_fields(owner)
    }

    /// Host-selected editing methods; generated and raw JVM members may differ.
    pub fn host_editing_methods(&self, owner: &str) -> Result<Vec<MethodIndexEntry>, FactsError> {
        self.facts.check_active()?;
        let editing = self
            .cache()?
            .host_types
            .get(owner)
            .cloned()
            .ok_or_else(|| unknown(format!("host editing type {owner} is unavailable")))?;
        let result = editing
            .methods
            .ok_or_else(|| unknown(format!("host editing methods for {owner} are incomplete")));
        self.facts.check_active()?;
        result
    }

    /// Host-selected editing fields; generated and raw JVM members may differ.
    pub fn host_editing_fields(&self, owner: &str) -> Result<Vec<FieldIndexEntry>, FactsError> {
        self.facts.check_active()?;
        let editing = self
            .cache()?
            .host_types
            .get(owner)
            .cloned()
            .ok_or_else(|| unknown(format!("host editing type {owner} is unavailable")))?;
        let result = editing
            .fields
            .ok_or_else(|| unknown(format!("host editing fields for {owner} are incomplete")));
        self.facts.check_active()?;
        result
    }

    /// Complete members of the selected physical ABI, with no source-current claim.
    pub fn physical_methods(&self, owner: &str) -> Result<Vec<MethodIndexEntry>, FactsError> {
        abi_methods(self.raw_abi(owner)?)
    }

    /// Complete fields of the selected physical ABI, with no source-current claim.
    pub fn physical_fields(&self, owner: &str) -> Result<Vec<FieldIndexEntry>, FactsError> {
        abi_fields(self.raw_abi(owner)?)
    }

    fn class_bytes(&self, owner: &str) -> Result<Vec<u8>, FactsError> {
        let selected = self
            .selected_entry(owner)?
            .ok_or_else(|| unknown(format!("source or generated class {owner} may exist")))?;
        let runtime = self.runtime_bytes(owner)?;
        if runtime.origin != selected.descriptor.origin_id {
            return Err(unknown(format!(
                "runtime and selected compile origins differ for {owner}"
            )));
        }
        Ok(runtime.bytes)
    }

    /// The host-selected runtime bytecode view, independent of compile-catalog order.
    pub fn physical_runtime_bytes(&self, owner: &str) -> Result<Vec<u8>, FactsError> {
        self.runtime_bytes(owner).map(|value| value.bytes)
    }

    fn runtime_bytes(&self, owner: &str) -> Result<RuntimeBytes, FactsError> {
        self.facts.check_active()?;
        let cached = { self.cache()?.bytes.get(owner).cloned() };
        if let Some(bytes) = cached {
            self.ensure_bound_runtime(owner, &bytes.origin)?;
            self.facts.check_active()?;
            return Ok(bytes);
        }
        let class = self.with_inputs(|inputs| inputs.runtime_class(owner))?;
        let class = match class {
            Fact::Found { value } => value,
            Fact::Unknown { reasons } => return Err(bytecode_unknown("runtime class", reasons)),
            Fact::DefinitelyAbsent => {
                return Err(protocol("runtime lookup cannot certify absence"))
            }
        };
        self.ensure_bound_runtime(owner, &class.descriptor.origin_id)?;
        let result = self.with_inputs(|inputs| inputs.class_bytes(&class))?;
        let bytes = match result {
            Fact::Found { value } => value,
            Fact::Unknown { reasons } => {
                return Err(bytecode_unknown("runtime class bytes", reasons))
            }
            Fact::DefinitelyAbsent => return Err(protocol("runtime bytes cannot certify absence")),
        };
        let index = ClassMemberIndex::index_class(&bytes, None, None)
            .map_err(|error| FactsError::BytecodeUnavailable(error.to_string()))?;
        if !index.classes.contains_key(owner) || index.classes.len() != 1 {
            return Err(protocol("runtime bytes contain a different JVM class"));
        }
        self.facts.check_active()?;
        let bytes = RuntimeBytes {
            origin: class.descriptor.origin_id,
            bytes,
        };
        self.cache()?.bytes.insert(owner.into(), bytes.clone());
        self.facts.check_active()?;
        Ok(bytes)
    }

    fn ensure_bound_runtime(&self, owner: &str, runtime_origin: &str) -> Result<(), FactsError> {
        self.facts.check_active()?;
        let bound = self.cache()?.selected_origins.get(owner).cloned();
        self.facts.check_active()?;
        if bound
            .as_deref()
            .is_some_and(|origin| origin != runtime_origin)
        {
            return Err(unknown(format!(
                "runtime and host-selected type origins differ for {owner}"
            )));
        }
        Ok(())
    }

    fn bytecode_adapter(
        &self,
        owner: &str,
        name: &str,
        descriptor: Option<&str>,
        physical_runtime: bool,
    ) -> Result<BytecodeAtTargetAdapter, FactsError> {
        let bytes = if physical_runtime {
            self.physical_runtime_bytes(owner)?
        } else {
            self.class_bytes(owner)?
        };
        let index = ClassMemberIndex::index_class(&bytes, None, None)
            .map_err(|error| FactsError::BytecodeUnavailable(error.to_string()))?;
        let selected = match descriptor.filter(|value| !value.is_empty()) {
            Some(value) => {
                descriptor::parse_method_descriptor(value)
                    .map_err(|error| protocol(format!("invalid method descriptor: {error}")))?;
                Some(value.to_owned())
            }
            None => {
                let methods: Vec<_> = index
                    .methods_by_owner
                    .get(owner)
                    .into_iter()
                    .flatten()
                    .filter(|method| method.name == name)
                    .collect();
                (methods.len() == 1).then(|| methods[0].descriptor.clone())
            }
        };
        if let Some(descriptor) = selected {
            extract_instructions(&bytes, name, &descriptor)
                .map_err(|error| FactsError::BytecodeUnavailable(error.to_string()))?;
        }
        let provider = InMemoryClassBytesProvider::new(BTreeMap::from([(owner.to_owned(), bytes)]))
            .map_err(|error| FactsError::BytecodeUnavailable(error.to_string()))?;
        Ok(BytecodeAtTargetAdapter::new(Arc::new(provider)))
    }

    fn known_type(&self, value: &JvmType, view: HierarchyView) -> Result<(), FactsError> {
        if self.reference_exists(value, view)? {
            Ok(())
        } else {
            Err(unknown("referenced JVM type is absent"))
        }
    }

    fn reference_exists(&self, value: &JvmType, view: HierarchyView) -> Result<bool, FactsError> {
        match value {
            JvmType::ObjectType(name) => match view {
                HierarchyView::Current => Ok(self.class_exists(name)?.is_some()),
                HierarchyView::Physical => Ok(self.selected_entry(name)?.is_some()),
            },
            JvmType::ArrayType(element) => self.known_component(element, view),
            _ => Err(protocol("reference type must be an object or array")),
        }
    }

    fn known_component(&self, value: &JvmType, view: HierarchyView) -> Result<bool, FactsError> {
        match value {
            JvmType::ObjectType(_) | JvmType::ArrayType(_) => self.reference_exists(value, view),
            JvmType::VoidType => Err(protocol("void array component")),
            _ => Ok(true),
        }
    }

    fn hierarchy_abi(&self, owner: &str, view: HierarchyView) -> Result<RawAbi, FactsError> {
        if matches!(view, HierarchyView::Current) && self.source_class(owner)?.is_some() {
            return Err(unknown(format!(
                "current source hierarchy for {owner} is unavailable"
            )));
        }
        self.raw_abi(owner)
    }

    fn assignable(
        &self,
        target: &JvmType,
        value: &JvmType,
        view: HierarchyView,
    ) -> Result<bool, FactsError> {
        self.known_type(target, view)?;
        self.known_type(value, view)?;
        self.assignable_known(target, value, view)
    }

    fn assignable_known(
        &self,
        target: &JvmType,
        value: &JvmType,
        view: HierarchyView,
    ) -> Result<bool, FactsError> {
        if target == value {
            return Ok(true);
        }
        match (target, value) {
            (JvmType::ObjectType(target), JvmType::ArrayType(_)) => Ok(matches!(
                target.as_str(),
                "java/lang/Object" | "java/lang/Cloneable" | "java/io/Serializable"
            )),
            (JvmType::ArrayType(target), JvmType::ArrayType(value)) => {
                if is_reference(target) && is_reference(value) {
                    self.assignable_known(target, value, view)
                } else {
                    Ok(target == value)
                }
            }
            (JvmType::ArrayType(_), JvmType::ObjectType(_)) => Ok(false),
            (JvmType::ObjectType(target), JvmType::ObjectType(_))
                if target == "java/lang/Object" =>
            {
                Ok(true)
            }
            (JvmType::ObjectType(target), JvmType::ObjectType(value)) => {
                self.is_ancestor(target, value, view)
            }
            _ => Err(protocol("invalid reference assignability input")),
        }
    }

    fn is_ancestor(
        &self,
        target: &str,
        value: &str,
        view: HierarchyView,
    ) -> Result<bool, FactsError> {
        let mut pending = VecDeque::from([value.to_owned()]);
        let mut seen = BTreeSet::new();
        let mut incomplete = None;
        while let Some(current) = pending.pop_front() {
            if !seen.insert(current.clone()) {
                continue;
            }
            if current == target {
                return Ok(true);
            }
            match self.hierarchy_abi(&current, view) {
                Ok(abi) => {
                    pending.extend(abi.super_name);
                    pending.extend(abi.interfaces);
                }
                Err(error @ (FactsError::Unknown(_) | FactsError::BytecodeUnavailable(_))) => {
                    if incomplete.is_none() {
                        incomplete = Some(error);
                    }
                }
                Err(error) => return Err(error),
            }
        }
        if let Some(error) = incomplete {
            Err(error)
        } else {
            Ok(false)
        }
    }

    fn common_type(
        &self,
        left: &JvmType,
        right: &JvmType,
        view: HierarchyView,
    ) -> Result<JvmType, FactsError> {
        let left_receives_right = self.assignable(left, right, view);
        if matches!(left_receives_right, Ok(true)) {
            return Ok(left.clone());
        }
        if let Err(error) = &left_receives_right {
            if !matches!(
                error,
                FactsError::Unknown(_) | FactsError::BytecodeUnavailable(_)
            ) {
                return Err(error.clone());
            }
        }
        let right_receives_left = self.assignable(right, left, view);
        if matches!(right_receives_left, Ok(true)) {
            return Ok(right.clone());
        }
        left_receives_right?;
        right_receives_left?;
        match (left, right) {
            (JvmType::ArrayType(left), JvmType::ArrayType(right))
                if is_reference(left) && is_reference(right) =>
            {
                Ok(JvmType::ArrayType(Box::new(
                    self.common_type(left, right, view)?,
                )))
            }
            (JvmType::ArrayType(_), _) | (_, JvmType::ArrayType(_)) => {
                let object = JvmType::ObjectType("java/lang/Object".into());
                self.known_type(&object, view)?;
                Ok(object)
            }
            (JvmType::ObjectType(left), JvmType::ObjectType(right)) => {
                let left = self.ancestors(left, view)?;
                let right: BTreeSet<_> = self.ancestors(right, view)?.into_iter().collect();
                left.into_iter()
                    .find(|name| right.contains(name))
                    .map(JvmType::ObjectType)
                    .ok_or_else(|| unknown("JVM hierarchies have no proven common parent"))
            }
            _ => Err(protocol("invalid common-superclass type")),
        }
    }

    fn ancestors(&self, start: &str, view: HierarchyView) -> Result<Vec<String>, FactsError> {
        let mut pending = vec![start.to_owned()];
        let mut seen = BTreeSet::new();
        let mut ordered = Vec::new();
        while let Some(name) = pending.pop() {
            if !seen.insert(name.clone()) {
                continue;
            }
            let abi = self.hierarchy_abi(&name, view)?;
            ordered.push(name);
            for parent in abi.interfaces.into_iter().rev() {
                pending.push(parent);
            }
            if let Some(parent) = abi.super_name {
                pending.push(parent);
            }
        }
        Ok(ordered)
    }

    /// This is a physical ABI view and does not certify unsaved source hierarchy.
    pub fn physical_is_known_reference(&self, internal: &str) -> Result<bool, FactsError> {
        self.facts.check_active()?;
        let value = parse_reference(internal)?;
        let result = self.reference_exists(&value, HierarchyView::Physical);
        self.facts.check_active()?;
        result
    }

    /// Tests assignability of selected physical JVM inputs, not source-current ABI.
    pub fn physical_is_assignable(&self, from: &str, expected: &str) -> Result<bool, FactsError> {
        self.facts.check_active()?;
        let target = parse_reference(from)?;
        let value = parse_reference(expected)?;
        let result = if !self.reference_exists(&target, HierarchyView::Physical)?
            || !self.reference_exists(&value, HierarchyView::Physical)?
        {
            Ok(false)
        } else {
            self.assignable(&target, &value, HierarchyView::Physical)
        };
        self.facts.check_active()?;
        result
    }

    /// Resolves descriptors against the selected physical ABI hierarchy only.
    pub fn physical_common_super_class(
        &self,
        left: &str,
        right: &str,
    ) -> Result<String, FactsError> {
        self.facts.check_active()?;
        let left = descriptor::parse_field_descriptor(left)
            .map_err(|error| protocol(format!("invalid JVM type descriptor: {error}")))?;
        let right = descriptor::parse_field_descriptor(right)
            .map_err(|error| protocol(format!("invalid JVM type descriptor: {error}")))?;
        let result = self
            .common_type(&left, &right, HierarchyView::Physical)
            .map(|value| descriptor::to_descriptor(&value));
        self.facts.check_active()?;
        result
    }

    fn query<T>(&self, operation: impl FnOnce() -> Result<T, FactsError>) -> T {
        let result = self.facts.check_active().and_then(|()| operation());
        let result = result.and_then(|value| self.facts.check_active().map(|()| value));
        result.unwrap_or_else(|error| std::panic::panic_any(error.command()))
    }

    fn physical_classes(
        &self,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<ClassIndexEntry>, FactsError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let folded = prefix.to_lowercase();
        let mut by_name: BTreeMap<String, Vec<CatalogEntry>> = BTreeMap::new();
        for entry in self.physical_catalog("")? {
            let name = &entry.descriptor.internal_name;
            let item = class_entry(name);
            if name.to_lowercase().starts_with(&folded)
                || item.fqn().to_lowercase().starts_with(&folded)
                || item.simple_name.to_lowercase().starts_with(&folded)
            {
                by_name.entry(name.clone()).or_default().push(entry);
            }
        }
        let mut values = Vec::new();
        for (name, entries) in by_name {
            if self.select_candidate(&name, entries)?.is_some() {
                values.push(class_entry(&name));
                if values.len() == limit {
                    break;
                }
            }
        }
        Ok(values)
    }

    fn physical_class_by_fqn(&self, fqn: &str) -> Result<Option<ClassIndexEntry>, FactsError> {
        let mut matches: BTreeMap<String, Vec<CatalogEntry>> = BTreeMap::new();
        for entry in self.physical_catalog("")? {
            if entry
                .descriptor
                .internal_name
                .replace('/', ".")
                .eq_ignore_ascii_case(fqn)
            {
                matches
                    .entry(entry.descriptor.internal_name.clone())
                    .or_default()
                    .push(entry);
            }
        }
        if matches.len() > 1 {
            return Err(unknown(format!(
                "case-insensitive JVM name {fqn} is ambiguous"
            )));
        }
        match matches.into_iter().next() {
            Some((name, candidates)) => Ok(self
                .select_candidate(&name, candidates)?
                .map(|_| class_entry(&name))),
            None => Ok(None),
        }
    }
}

fn editing_view(observed: &TypeJvm) -> Result<EditingView, FactsError> {
    observed
        .declared_members
        .coverage
        .validate()
        .map_err(protocol)?;
    observed.class_flags.validate().map_err(protocol)?;
    observed.jvm_abi_coverage.validate().map_err(protocol)?;
    let complete = matches!(
        observed.declared_members.coverage,
        Fact::Found { value: () }
    );
    let mut methods = complete.then(Vec::new);
    let mut fields = complete.then(Vec::new);
    let mut ids = BTreeSet::new();
    let mut signatures = BTreeMap::new();
    let mut registry = Registry::default();
    for editing in &observed.declared_members.members {
        let member = &editing.member;
        if member.id.is_empty() || member.name.is_empty() || !ids.insert(member.id.clone()) {
            return Err(protocol("invalid or duplicate host editing member"));
        }
        registry
            .register(&member.id, HandleKind::Member)
            .map_err(protocol)?;
        member.signature.validate().map_err(protocol)?;
        member.dispatch.validate().map_err(protocol)?;
        editing.visibility.validate().map_err(protocol)?;
        editing.synthetic.validate().map_err(protocol)?;
        let signature = match &member.signature {
            Fact::Found { value } => {
                if value.id.is_empty() || value.kind != member.kind {
                    return Err(protocol("invalid host editing signature"));
                }
                registry
                    .register(&value.id, HandleKind::Signature)
                    .map_err(protocol)?;
                let signature = (value.kind.clone(), value.descriptor.clone());
                if signatures
                    .get(&value.id)
                    .is_some_and(|previous| previous != &signature)
                {
                    return Err(protocol(
                        "host reused a signature handle for different facts",
                    ));
                }
                signatures.insert(value.id.clone(), signature);
                Some(value)
            }
            Fact::Unknown { .. } | Fact::DefinitelyAbsent => None,
        };
        let dispatch = match &member.dispatch {
            Fact::Found { value } if value == "static" => Some(true),
            Fact::Found { value } if value == "instance" => Some(false),
            Fact::Found { .. } => return Err(protocol("invalid host editing dispatch")),
            Fact::Unknown { .. } | Fact::DefinitelyAbsent => None,
        };
        match member.kind.as_str() {
            "method" => {
                let parsed = signature
                    .map(|signature| {
                        descriptor::parse_method_descriptor(&signature.descriptor).map_err(
                            |error| protocol(format!("invalid host editing method: {error}")),
                        )
                    })
                    .transpose()?;
                if signature.is_none() || dispatch.is_none() || !editing.mismatches.is_empty() {
                    methods = None;
                } else if let Some(values) = &mut methods {
                    let signature = signature.expect("signature was checked");
                    let parsed = parsed.expect("descriptor was checked");
                    values.push(MethodIndexEntry {
                        name: member.name.clone(),
                        descriptor: signature.descriptor.clone(),
                        is_static: dispatch.expect("dispatch was checked"),
                        readable_signature: format!(
                            "{}{}",
                            member.name,
                            descriptor::render_method(&parsed)
                        ),
                    });
                }
            }
            "field" => {
                let parsed = signature
                    .map(|signature| {
                        descriptor::parse_field_descriptor(&signature.descriptor).map_err(|error| {
                            protocol(format!("invalid host editing field: {error}"))
                        })
                    })
                    .transpose()?;
                if signature.is_none() || dispatch.is_none() || !editing.mismatches.is_empty() {
                    fields = None;
                } else if let Some(values) = &mut fields {
                    let signature = signature.expect("signature was checked");
                    let parsed = parsed.expect("descriptor was checked");
                    values.push(FieldIndexEntry {
                        name: member.name.clone(),
                        descriptor: signature.descriptor.clone(),
                        is_static: dispatch.expect("dispatch was checked"),
                        readable_type: descriptor::render(&parsed),
                    });
                }
            }
            _ => return Err(protocol("invalid host editing member kind")),
        }
    }
    Ok(EditingView { methods, fields })
}

fn abi_methods(abi: RawAbi) -> Result<Vec<MethodIndexEntry>, FactsError> {
    abi.methods
        .into_iter()
        .map(|method| {
            let descriptor = descriptor::parse_method_descriptor(&method.descriptor)
                .map_err(|error| protocol(format!("invalid JVM method descriptor: {error}")))?;
            Ok(MethodIndexEntry {
                readable_signature: format!(
                    "{}{}",
                    method.name,
                    descriptor::render_method(&descriptor)
                ),
                name: method.name,
                descriptor: method.descriptor,
                is_static: method.access_flags & 8 != 0,
            })
        })
        .collect()
}

fn abi_fields(abi: RawAbi) -> Result<Vec<FieldIndexEntry>, FactsError> {
    abi.fields
        .into_iter()
        .map(|field| {
            let descriptor = descriptor::parse_field_descriptor(&field.descriptor)
                .map_err(|error| protocol(format!("invalid JVM field descriptor: {error}")))?;
            Ok(FieldIndexEntry {
                readable_type: descriptor::render(&descriptor),
                name: field.name,
                descriptor: field.descriptor,
                is_static: field.access_flags & 8 != 0,
            })
        })
        .collect()
}

fn protocol(message: impl Into<String>) -> FactsError {
    FactsError::Protocol(message.into())
}
fn unknown(message: impl Into<String>) -> FactsError {
    FactsError::Unknown(message.into())
}
fn unknown_fact(label: &str, reasons: Vec<Value>) -> FactsError {
    unknown(format!("{label} unavailable: {reasons:?}"))
}
fn bytecode_unknown(label: &str, reasons: Vec<Value>) -> FactsError {
    FactsError::BytecodeUnavailable(format!("{label} unavailable: {reasons:?}"))
}

fn class_entry(owner: &str) -> ClassIndexEntry {
    ClassIndexEntry {
        simple_name: owner.rsplit('/').next().unwrap_or(owner).into(),
        package_name: owner
            .rsplit_once('/')
            .map_or(String::new(), |(package, _)| package.replace('/', ".")),
        internal_name: owner.into(),
    }
}

fn is_reference(value: &JvmType) -> bool {
    matches!(value, JvmType::ObjectType(_) | JvmType::ArrayType(_))
}

fn parse_reference(value: &str) -> Result<JvmType, FactsError> {
    let descriptor = if value.starts_with('[') {
        value.to_owned()
    } else {
        format!("L{value};")
    };
    let ty = descriptor::parse_field_descriptor(&descriptor)
        .map_err(|error| protocol(format!("invalid JVM reference: {error}")))?;
    if !is_reference(&ty) {
        return Err(protocol("not a reference type"));
    }
    Ok(ty)
}

impl<R: EvaluationFactReader> ClassIndex for EvaluationIndex<R> {
    fn find_classes(&self, prefix: &str, limit: usize) -> Vec<ClassIndexEntry> {
        self.query(|| {
            if limit == 0 {
                return Ok(Vec::new());
            }
            let source = self
                .source
                .as_ref()
                .ok_or_else(|| unknown("current source class inventory is incomplete"))?;
            let answer = source
                .find_classes(prefix, limit)
                .map_err(|error| FactsError::from_core(error, false))?;
            if answer.authority != SourceAuthority::Complete {
                return Err(unknown("current source class inventory is incomplete"));
            }
            if source.covers_project_dependencies() || answer.value.len() >= limit {
                return Ok(answer.value.into_iter().take(limit).collect());
            }
            let mut results = answer.value;
            let mut names: BTreeSet<_> = results
                .iter()
                .map(|entry| entry.internal_name.clone())
                .collect();
            let folded = prefix.to_lowercase();
            let mut by_name: BTreeMap<String, Vec<CatalogEntry>> = BTreeMap::new();
            for entry in self.physical_catalog("")? {
                let name = &entry.descriptor.internal_name;
                let class = class_entry(name);
                if name.to_lowercase().starts_with(&folded)
                    || class.fqn().to_lowercase().starts_with(&folded)
                    || class.simple_name.to_lowercase().starts_with(&folded)
                {
                    by_name.entry(name.clone()).or_default().push(entry);
                }
            }
            for (name, entries) in by_name {
                if names.contains(&name) {
                    continue;
                }
                if self.select_candidate(&name, entries)?.is_some() {
                    names.insert(name.clone());
                    results.push(class_entry(&name));
                }
            }
            results.truncate(limit);
            Ok(results)
        })
    }

    fn find_class(&self, internal_name: &str) -> Option<ClassIndexEntry> {
        self.query(|| self.class_exists(internal_name))
    }

    fn find_class_by_fqn(&self, fqn: &str) -> Option<ClassIndexEntry> {
        self.query(|| {
            let source = self
                .source
                .as_ref()
                .ok_or_else(|| unknown(format!("current source ownership for {fqn} is unknown")))?;
            let answer = source
                .find_class_by_fqn(fqn)
                .map_err(|error| FactsError::from_core(error, false))?;
            if answer.authority != SourceAuthority::Complete {
                return Err(unknown(format!(
                    "current source ownership for {fqn} is incomplete"
                )));
            }
            if answer.value.is_some() || source.covers_project_dependencies() {
                return Ok(answer.value);
            }
            let mut matches: BTreeMap<String, Vec<CatalogEntry>> = BTreeMap::new();
            for entry in self.physical_catalog("")? {
                if entry
                    .descriptor
                    .internal_name
                    .replace('/', ".")
                    .eq_ignore_ascii_case(fqn)
                {
                    matches
                        .entry(entry.descriptor.internal_name.clone())
                        .or_default()
                        .push(entry);
                }
            }
            if matches.len() > 1 {
                return Err(unknown(format!(
                    "case-insensitive JVM name {fqn} is ambiguous"
                )));
            }
            match matches.into_iter().next() {
                Some((name, candidates)) => Ok(self
                    .select_candidate(&name, candidates)?
                    .map(|_| class_entry(&name))),
                None => Ok(None),
            }
        })
    }

    fn get_methods(&self, owner: &str) -> Vec<MethodIndexEntry> {
        self.query(|| self.methods(owner))
    }
    fn get_fields(&self, owner: &str) -> Vec<FieldIndexEntry> {
        self.query(|| self.fields(owner))
    }
}

impl<R: EvaluationFactReader> BytecodeIndex for EvaluationIndex<R> {
    fn get_at_target_candidates(
        &self,
        owner: &str,
        method: &str,
        descriptor: Option<&str>,
        value: &str,
    ) -> Vec<AtTargetCandidate> {
        self.query(|| {
            if method.is_empty() || value.is_empty() {
                return Ok(Vec::new());
            }
            Ok(self
                .bytecode_adapter(owner, method, descriptor, false)?
                .get_at_target_candidates(owner, method, descriptor, value))
        })
    }
    fn get_return_ordinal_count(
        &self,
        owner: &str,
        method: &str,
        descriptor: Option<&str>,
    ) -> usize {
        self.query(|| {
            if method.is_empty() {
                return Ok(0);
            }
            Ok(self
                .bytecode_adapter(owner, method, descriptor, false)?
                .get_return_ordinal_count(owner, method, descriptor))
        })
    }
    fn get_class_bytes(&self, owner: &str) -> Option<Vec<u8>> {
        self.query(|| self.class_bytes(owner).map(Some))
    }
    fn resolve_common_super_class(&self, left: &str, right: &str) -> Option<String> {
        self.query(|| {
            let left = descriptor::parse_field_descriptor(left)
                .map_err(|error| protocol(format!("invalid JVM type descriptor: {error}")))?;
            let right = descriptor::parse_field_descriptor(right)
                .map_err(|error| protocol(format!("invalid JVM type descriptor: {error}")))?;
            self.common_type(&left, &right, HierarchyView::Current)
                .map(|value| Some(descriptor::to_descriptor(&value)))
        })
    }
}

impl<R: EvaluationFactReader> HandlerReferenceTypeResolver for EvaluationIndex<R> {
    fn is_known_reference(&self, internal: &str) -> Option<bool> {
        self.query(|| {
            let value = parse_reference(internal)?;
            self.reference_exists(&value, HierarchyView::Current)
                .map(Some)
        })
    }
    fn is_assignable(&self, from: &str, expected: &str) -> Option<bool> {
        self.query(|| {
            let target = parse_reference(from)?;
            let value = parse_reference(expected)?;
            if !self.reference_exists(&target, HierarchyView::Current)?
                || !self.reference_exists(&value, HierarchyView::Current)?
            {
                return Ok(Some(false));
            }
            self.assignable(&target, &value, HierarchyView::Current)
                .map(Some)
        })
    }
}

impl<R: EvaluationFactReader> ClassIndex for PhysicalIndex<R> {
    fn find_classes(&self, prefix: &str, limit: usize) -> Vec<ClassIndexEntry> {
        self.index
            .query(|| self.index.physical_classes(prefix, limit))
    }
    fn find_class(&self, internal_name: &str) -> Option<ClassIndexEntry> {
        self.index.query(|| {
            self.index
                .selected_entry(internal_name)
                .map(|entry| entry.map(|_| class_entry(internal_name)))
        })
    }
    fn find_class_by_fqn(&self, fqn: &str) -> Option<ClassIndexEntry> {
        self.index.query(|| self.index.physical_class_by_fqn(fqn))
    }
    fn get_methods(&self, owner: &str) -> Vec<MethodIndexEntry> {
        self.index
            .query(|| match self.index.selected_entry(owner)? {
                Some(_) => self.index.physical_methods(owner),
                None => Ok(Vec::new()),
            })
    }
    fn get_fields(&self, owner: &str) -> Vec<FieldIndexEntry> {
        self.index
            .query(|| match self.index.selected_entry(owner)? {
                Some(_) => self.index.physical_fields(owner),
                None => Ok(Vec::new()),
            })
    }
}

impl<R: EvaluationFactReader> BytecodeIndex for PhysicalIndex<R> {
    fn get_at_target_candidates(
        &self,
        owner: &str,
        method: &str,
        descriptor: Option<&str>,
        value: &str,
    ) -> Vec<AtTargetCandidate> {
        self.index.query(|| {
            if method.is_empty() || value.is_empty() {
                return Ok(Vec::new());
            }
            Ok(self
                .index
                .bytecode_adapter(owner, method, descriptor, true)?
                .get_at_target_candidates(owner, method, descriptor, value))
        })
    }
    fn get_return_ordinal_count(
        &self,
        owner: &str,
        method: &str,
        descriptor: Option<&str>,
    ) -> usize {
        self.index.query(|| {
            if method.is_empty() {
                return Ok(0);
            }
            Ok(self
                .index
                .bytecode_adapter(owner, method, descriptor, true)?
                .get_return_ordinal_count(owner, method, descriptor))
        })
    }
    fn get_class_bytes(&self, owner: &str) -> Option<Vec<u8>> {
        self.index
            .query(|| self.index.physical_runtime_bytes(owner).map(Some))
    }
    fn resolve_common_super_class(&self, left: &str, right: &str) -> Option<String> {
        self.index.query(|| {
            self.index
                .physical_common_super_class(left, right)
                .map(Some)
        })
    }
}

impl<R: EvaluationFactReader> HandlerReferenceTypeResolver for PhysicalIndex<R> {
    fn is_known_reference(&self, internal: &str) -> Option<bool> {
        self.index
            .query(|| self.index.physical_is_known_reference(internal).map(Some))
    }
    fn is_assignable(&self, from: &str, expected: &str) -> Option<bool> {
        self.index
            .query(|| self.index.physical_is_assignable(from, expected).map(Some))
    }
}
