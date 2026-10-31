use super::*;
use crate::{
    host_facts::inputs::read_split,
    wire::{Fact, TypeFact},
};
use serde::de::DeserializeOwned;
use serde_json::json;

pub(super) struct CurrentSourceInputs {
    pub inventory: Inventory<SourceEntry>,
    pub snapshots: Vec<Fact<SourceSnapshot>>,
    pub facts: Option<Vec<SourceFacts>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_facts::inputs::InputOps;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn found(value: Value) -> Value {
        json!({"state":"found","value":value})
    }
    fn unknown_fact() -> Value {
        json!({"state":"unknown","reasons":[{"code":"MissingFacts"}]})
    }
    fn abi() -> Value {
        json!({"identity":{"originId":"compile:dep","internalName":"demo/Dep","contentSha256":"a".repeat(64)},
        "majorVersion":61,"minorVersion":0,"accessFlags":33,"superName":"java/lang/Object","interfaces":[],
        "signature":null,"sourceFile":null,"attributeNames":[],"innerClasses":[],"fields":[],"methods":[
            {"name":"compiled","descriptor":"()V","accessFlags":1,"signature":null,"attributeNames":[]},
            {"name":"compiledBridge","descriptor":"()Ljava/lang/Object;","accessFlags":4161,"signature":null,"attributeNames":[]}
        ]})
    }
    #[derive(Clone)]
    struct Host {
        calls: Arc<Mutex<Vec<Value>>>,
        active: Arc<AtomicBool>,
        inventory_complete: bool,
    }
    impl FactReader for Host {
        fn read_facts(&mut self, evaluation: &str, queries: &[Value]) -> Result<Value, FactsError> {
            assert_eq!(evaluation, "evaluation");
            self.calls.lock().unwrap().extend_from_slice(queries);
            let facts = queries.iter().map(|query| match query["op"].as_str().unwrap() {
                "resolveType" => found(json!({"id":format!("type:{}",query["name"].as_str().unwrap()),
                    "qualifiedName":query["name"],"package":"demo"})),
                "typeJvm" => {
                    let live = query["target"] == "type:demo.Live";
                    let owner = if live { "demo/Live" } else { "demo/Dep" };
                    let members = if live { json!([{"id":"method:tick","name":"tick","kind":"method",
                        "signature":found(json!({"id":"signature:tick","kind":"method","descriptor":"()V"})),
                        "dispatch":found(json!("instance")),"visibility":found(json!("Private")),
                        "synthetic":found(json!(false)),"mismatches":[]}]) } else { json!([]) };
                    json!({"binaryName":found(json!({"internalName":owner,"descriptor":format!("L{owner};")})),
                        "classFlags":found(json!(33)),"declaredMembers":{"members":members,"coverage":found(Value::Null)},
                        "abi":if live {unknown_fact()} else {found(abi())},"jvmAbiCoverage":unknown_fact()})
                }
                "sourceLocation" if query["target"] == "type:demo.Live" => found(json!({"target":query["target"],
                    "documentUri":mcdev_core::project::path_to_uri(&std::env::temp_dir().join("mcdev-current/Live.java")),
                    "range":{"start":{"line":1,"character":6},"end":{"line":1,"character":10}},
                    "source":{"identity":{"kind":"source","revision":"8"},"sha256":"b".repeat(64)}})),
                "sourceLocation" => unknown_fact(),
                "sourceInventory" => json!({"sources":[],"next":null,"coverage":if self.inventory_complete {found(Value::Null)} else {unknown_fact()}}),
                "jvmCatalog" => json!({"view":"selectedPhysicalJvmInputs","classes":[{"entry":"entry:dep","internalName":"demo/Dep","originId":"compile:dep",
                    "association":"compileClasspath","classpathOrder":0,"authority":"compiled-input"}],"next":null,
                    "coverage":found(Value::Null),"problems":[],"sourceCurrent":unknown_fact()}),
                "jvmClass" => found(json!({"entry":"entry:dep","authority":"compiled-input","abi":abi(),"sourceCurrent":unknown_fact()})),
                other => panic!("unexpected query {other}"),
            }).collect::<Vec<_>>();
            Ok(json!({"facts":facts}))
        }
    }
    impl EvaluationFactReader for Host {
        fn check_active(&self) -> Result<(), FactsError> {
            if self.active.load(Ordering::SeqCst) {
                Ok(())
            } else {
                Err(FactsError::Cancelled("cancelled".into()))
            }
        }
    }
    fn test_index(inventory_complete: bool) -> (EvaluationIndex<Host>, Host) {
        let host = Host {
            calls: Arc::new(Mutex::new(Vec::new())),
            active: Arc::new(AtomicBool::new(true)),
            inventory_complete,
        };
        let ops = InputOps::intersect(&json!({"capabilities":["source-snapshots-v1","source-facts-v2","source-locations-v1","jvm-catalog-v1"],
            "supportedOps":{"source-snapshots-v1":["sourceInventory","sourceSnapshot"],"source-facts-v2":["sourceFacts"],
                "source-locations-v1":["sourceLocation"],"jvm-catalog-v1":["jvmCatalog","jvmClass"]}})).unwrap();
        let inputs = Arc::new(Mutex::new(
            ScopedInputs::new("evaluation", &ops, host.clone()).unwrap(),
        ));
        (
            EvaluationIndex::new(inputs, host.clone()).with_current_source_facts("evaluation"),
            host,
        )
    }

    #[test]
    fn current_source_members_need_no_compiled_abi_or_global_source_sweep() {
        let (index, host) = test_index(true);
        let methods = index.methods("demo/Live").unwrap();
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "tick");
        assert_eq!(methods[0].descriptor, "()V");
        assert!(index.fields("demo/Live").unwrap().is_empty());
        let calls = host.calls.lock().unwrap();
        assert_eq!(
            calls
                .iter()
                .map(|query| query["op"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["resolveType", "typeJvm", "sourceLocation"]
        );
        drop(calls);
        host.active.store(false, Ordering::SeqCst);
        assert!(matches!(
            index.methods("demo/Live"),
            Err(FactsError::Cancelled(_))
        ));
    }

    #[test]
    fn dependency_members_keep_the_selected_raw_abi_and_do_not_promote_source_current() {
        let (index, host) = test_index(true);
        let methods = index.methods("demo/Dep").unwrap();
        assert_eq!(
            methods
                .iter()
                .map(|member| member.name.as_str())
                .collect::<Vec<_>>(),
            ["compiled", "compiledBridge"]
        );
        let calls = host.calls.lock().unwrap();
        assert!(calls.iter().any(|query| query["op"] == "sourceInventory"));
        assert!(calls.iter().any(|query| query["op"] == "jvmClass"));
        assert!(!calls
            .iter()
            .any(|query| query["op"] == "runtimeClass" || query["op"] == "classBytes"));
        assert_eq!(
            index
                .cache()
                .unwrap()
                .selected_origins
                .get("demo/Dep")
                .map(String::as_str),
            Some("compile:dep")
        );
    }

    #[test]
    fn a_sibling_source_set_output_is_not_a_binary_dependency_when_source_authority_is_unknown() {
        let (index, _) = test_index(true);
        let index = index.with_owned_output_origins(BTreeSet::from(["compile:dep".to_owned()]));
        assert!(
            matches!(index.methods("demo/Dep"),Err(FactsError::Unknown(reason))
            if reason.contains("project output"))
        );
        assert!(!index.cache().unwrap().source_types.contains_key("demo/Dep"));
        assert!(matches!(
            index.current_classes("Dep", 100),
            Err(FactsError::Unknown(_))
        ));
    }

    #[test]
    fn incomplete_inventory_or_missing_location_for_an_authored_name_cannot_prove_absence() {
        let (index, host) = test_index(false);
        assert!(matches!(
            index.methods("demo/Dep"),
            Err(FactsError::Unknown(_))
        ));
        assert!(!host
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|query| query["op"] == "jvmClass"));
        let (index, host) = test_index(true);
        index.cache().unwrap().source_names = Some(BTreeSet::from(["demo.Dep".to_owned()]));
        assert!(matches!(
            index.methods("demo/Dep"),
            Err(FactsError::Unknown(_))
        ));
        assert!(!host
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|query| query["op"] == "jvmClass"));
    }
}

impl<R: EvaluationFactReader> EvaluationIndex<R> {
    fn semantic_read<T: DeserializeOwned>(&self, queries: &[Value]) -> Result<Vec<T>, FactsError> {
        let evaluation = self
            .source_evaluation
            .as_deref()
            .filter(|evaluation| !evaluation.is_empty())
            .ok_or_else(|| protocol("missing authored source evaluation"))?;
        self.facts.check_active()?;
        let values = read_split(&mut self.facts.clone(), evaluation, queries)?;
        self.facts.check_active()?;
        values
            .into_iter()
            .map(|value| {
                serde_json::from_value(value)
                    .map_err(|error| protocol(format!("invalid authored source fact: {error}")))
            })
            .collect()
    }

    fn selected_type(&self, name: &str) -> Result<Option<(TypeFact, TypeJvm)>, FactsError> {
        let fact: Fact<TypeFact> = self
            .semantic_read(&[json!({"op":"resolveType","name":name})])?
            .pop()
            .ok_or_else(|| protocol("missing selected type fact"))?;
        fact.validate().map_err(protocol)?;
        let Fact::Found { value: ty } = fact else {
            return Ok(None);
        };
        if ty.qualified_name.is_empty() || ty.read_only {
            return Err(protocol("invalid selected authored type"));
        }
        Registry::default()
            .register(&ty.id, HandleKind::Type)
            .map_err(protocol)?;
        let jvm: TypeJvm = self
            .semantic_read(&[json!({"op":"typeJvm","target":ty.id})])?
            .pop()
            .ok_or_else(|| protocol("missing selected JVM type fact"))?;
        jvm.binary_name.validate().map_err(protocol)?;
        let binary = jvm
            .binary_name
            .found()
            .ok_or_else(|| unknown("selected JVM name is unavailable"))?;
        if descriptor::parse_field_descriptor(&binary.descriptor).ok()
            != Some(JvmType::ObjectType(binary.internal_name.clone()))
        {
            return Err(protocol("selected type has inconsistent JVM identity"));
        }
        Ok(Some((ty, jvm)))
    }

    /// Complete authored names establish source absence; they do not grant ABI or runtime authority.
    fn current_source_names(&self) -> Result<BTreeSet<String>, FactsError> {
        self.facts.check_active()?;
        if let Some(names) = self.cache()?.source_names.clone() {
            return Ok(names);
        }
        let inventory = match &self.source_inputs {
            Some(inputs) => inputs.inventory.clone(),
            None => self.with_inputs(|inputs| inputs.source_inventory())?,
        };
        if !matches!(inventory.coverage, Fact::Found { value: Value::Null }) {
            return Err(unknown("current source inventory coverage is incomplete"));
        }
        let snapshots = match &self.source_inputs {
            Some(inputs) => inputs.snapshots.clone(),
            None => self.with_inputs(|inputs| inputs.source_snapshots(&inventory.entries))?,
        };
        let facts = match &self.source_inputs {
            Some(CurrentSourceInputs {
                facts: Some(facts), ..
            }) => facts.clone(),
            _ => self.with_inputs(|inputs| inputs.source_facts_many(&inventory.entries))?,
        };
        if snapshots.len() != facts.len() || facts.len() != inventory.entries.len() {
            return Err(protocol("authored source inventory cardinality mismatch"));
        }
        let mut declarations = Vec::new();
        let mut contexts = Vec::new();
        let mut registry = Registry::default();
        for ((snapshot, facts), entry) in snapshots.into_iter().zip(facts).zip(&inventory.entries) {
            let snapshot = match snapshot {
                Fact::Found { value } => value,
                _ => return Err(unknown("current source snapshot is unavailable")),
            };
            if snapshot.descriptor != entry.descriptor
                || facts.source.as_ref() != Some(&entry.descriptor)
            {
                return Err(protocol(
                    "reused authored source inputs have inconsistent identities",
                ));
            }
            {
                use sha2::{Digest, Sha256};
                if snapshot.sha256 != format!("{:x}", Sha256::digest(snapshot.text.as_bytes())) {
                    return Err(protocol("reused authored source snapshot digest mismatch"));
                }
            }
            if facts
                .semantic_scope
                .as_ref()
                .and_then(Fact::found)
                .is_none()
            {
                return Err(unknown("current source semantic scope is unavailable"));
            }
            facts.validate(&snapshot.text, &mut Registry::default())?;
            for declaration in facts.declarations {
                match declaration.source_kind.as_deref() {
                    Some("Class" | "Interface" | "Object" | "EnumClass" | "TypeAlias") => {
                        declarations
                            .push(json!({"op":"declarationType","declaration":declaration.id}));
                        contexts.push(entry.descriptor.clone());
                    }
                    Some("Function" | "Property") => {}
                    _ => return Err(unknown("authored source declaration kind is unsupported")),
                }
            }
        }
        let types: Vec<Fact<TypeFact>> = self.semantic_read(&declarations)?;
        let mut names = BTreeSet::new();
        for (fact, expected) in types.into_iter().zip(contexts) {
            fact.validate().map_err(protocol)?;
            let Fact::Found { value: ty } = fact else {
                return Err(unknown("authored source type identity is unavailable"));
            };
            let context = ty
                .context_source
                .as_ref()
                .ok_or_else(|| protocol("snapshot type lacks its source context"))?;
            if !ty.read_only
                || ty.qualified_name.is_empty()
                || context.identity != expected.identity
                || context.document_uri != expected.document_uri
                || context.language_id != expected.language_id
            {
                return Err(protocol("snapshot type has a foreign source context"));
            }
            registry
                .register(&ty.id, HandleKind::Type)
                .map_err(protocol)?;
            names.insert(ty.qualified_name);
        }
        self.facts.check_active()?;
        self.cache()?.source_names = Some(names.clone());
        Ok(names)
    }

    pub(super) fn current_source_class(
        &self,
        owner: &str,
    ) -> Result<Option<ClassIndexEntry>, FactsError> {
        self.facts.check_active()?;
        if let Some(current) = self.cache()?.source_types.get(owner) {
            return Ok(current.as_ref().map(|_| class_entry(owner)));
        }
        let name = owner.replace('/', ".");
        let mut selected = self.selected_type(&name)?;
        if selected.is_none() && name.contains('$') {
            selected = self.selected_type(&name.replace('$', "."))?;
        }
        let Some((ty, jvm)) = selected else {
            let names = self.current_source_names()?;
            if names.contains(&name) || names.contains(&name.replace('$', ".")) {
                return Err(unknown(format!(
                    "authored type {owner} is not resolved in the selected scope"
                )));
            }
            self.check_dependency_origin(owner)?;
            self.cache()?.source_types.insert(owner.into(), None);
            return Ok(None);
        };
        if !jvm
            .binary_name
            .found()
            .is_some_and(|binary| binary.internal_name == owner)
        {
            return Err(unknown(
                "resolved type does not match the requested JVM owner",
            ));
        }
        let location = self.with_inputs(|inputs| inputs.source_location(&ty.id))?;
        match location {
            Fact::Found { value } => {
                super::super::definition::checked_location_range(&value, &ty.id)?;
                let editing = editing_view(&jvm)?;
                self.facts.check_active()?;
                self.cache()?
                    .source_types
                    .insert(owner.into(), Some(editing));
                Ok(Some(class_entry(owner)))
            }
            Fact::Unknown { .. } => {
                if self.current_source_names()?.contains(&ty.qualified_name) {
                    return Err(unknown(format!(
                        "current source location for {owner} is unavailable"
                    )));
                }
                self.bind_host_type(&jvm)?;
                self.check_dependency_origin(owner)?;
                self.facts.check_active()?;
                self.cache()?.source_types.insert(owner.into(), None);
                Ok(None)
            }
            Fact::DefinitelyAbsent => Err(protocol("source location cannot prove source absence")),
        }
    }

    fn check_dependency_origin(&self, owner: &str) -> Result<(), FactsError> {
        if self.selected_entry(owner)?.is_some_and(|entry| {
            entry.descriptor.association == crate::host_facts::inputs::CatalogAssociation::Output
                || self
                    .owned_output_origins
                    .contains(&entry.descriptor.origin_id)
        }) {
            return Err(unknown(format!(
                "current source authority for project output {owner} is unavailable"
            )));
        }
        Ok(())
    }

    pub(super) fn current_source_members<T>(
        &self,
        owner: &str,
        project: impl FnOnce(&EditingView) -> Option<Vec<T>>,
        label: &str,
    ) -> Result<Vec<T>, FactsError> {
        self.facts.check_active()?;
        let value = self
            .cache()?
            .source_types
            .get(owner)
            .and_then(Option::as_ref)
            .and_then(project)
            .ok_or_else(|| unknown(format!("current source {label} for {owner} are incomplete")))?;
        self.facts.check_active()?;
        Ok(value)
    }

    pub(super) fn current_class_by_fqn(
        &self,
        fqn: &str,
    ) -> Result<Option<ClassIndexEntry>, FactsError> {
        if let Some((_, jvm)) = self.selected_type(fqn)? {
            let owner = jvm
                .binary_name
                .found()
                .expect("selected JVM name was checked")
                .internal_name
                .clone();
            return self.class_exists(&owner);
        }
        if self.current_source_names()?.contains(fqn) {
            return Err(unknown(format!(
                "authored type {fqn} is not resolved in the selected scope"
            )));
        }
        let entry = self.physical_class_by_fqn(fqn)?;
        if let Some(entry) = &entry {
            self.check_dependency_origin(&entry.internal_name)?;
        }
        Ok(entry)
    }

    pub(super) fn current_classes(
        &self,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<ClassIndexEntry>, FactsError> {
        let folded = prefix.to_lowercase();
        let mut results = Vec::new();
        let mut names = BTreeSet::new();
        for fqn in self.current_source_names()? {
            if fqn.to_lowercase().starts_with(&folded)
                || fqn
                    .rsplit('.')
                    .next()
                    .unwrap_or("")
                    .to_lowercase()
                    .starts_with(&folded)
            {
                let entry = self
                    .current_class_by_fqn(&fqn)?
                    .ok_or_else(|| unknown("authored source type selection is unavailable"))?;
                if names.insert(entry.internal_name.clone()) {
                    results.push(entry);
                }
            }
        }
        if results.len() < limit {
            let mut by_name = BTreeMap::<String, Vec<CatalogEntry>>::new();
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
            for (name, candidates) in by_name {
                if names.contains(&name) {
                    continue;
                }
                let Some(entry) = self.select_candidate(&name, candidates)? else {
                    continue;
                };
                let class = if entry.descriptor.association
                    == crate::host_facts::inputs::CatalogAssociation::Output
                    || self
                        .owned_output_origins
                        .contains(&entry.descriptor.origin_id)
                {
                    self.current_source_class(&name)?
                        .ok_or_else(|| unknown("current project output source is unavailable"))?
                } else {
                    class_entry(&name)
                };
                names.insert(name);
                results.push(class);
                if results.len() == limit {
                    break;
                }
            }
        }
        results.truncate(limit);
        self.facts.check_active()?;
        Ok(results)
    }
}
