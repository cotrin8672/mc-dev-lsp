#![allow(non_snake_case)]
use mcdev_core::{bytecode::*, host_project::*, host_source::*, project::*, *};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread,
};

const MINIMAL: &[u8] = &[0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 0];
fn fixture_generations(facts: &HostProjectFacts) -> HostGenerations {
    // This fake authority supplies a complete eight-counter snapshot. The three
    // zeros are known initial fixture generations, not missing live host facts.
    HostGenerations {
        project: facts.project_generation,
        model: 0,
        namespace: 0,
        classpath: 0,
        dependency: facts.dependency_generation,
        generated_source: facts.generated_source_generation,
        config: facts.config_generation,
        provider: facts.provider_generation,
    }
}
fn tagged(tag: u8) -> Vec<u8> {
    [MINIMAL, &[tag]].concat()
}
fn fixture(owner: &str) -> Vec<u8> {
    match owner {
        "demo/Alpha" => {
            include_bytes!("fixtures/host_project/classes/demo/Alpha.class.bin").to_vec()
        }
        "demo/Beta" => include_bytes!("fixtures/host_project/classes/demo/Beta.class.bin").to_vec(),
        "exact/demo/Alpha" => {
            include_bytes!("fixtures/host_project/classes/exact/demo/Alpha.class.bin").to_vec()
        }
        "demo/Cache0" => {
            include_bytes!("fixtures/host_project/classes/demo/Cache0.class.bin").to_vec()
        }
        "demo/Cache1" => {
            include_bytes!("fixtures/host_project/classes/demo/Cache1.class.bin").to_vec()
        }
        "demo/Cache2" => {
            include_bytes!("fixtures/host_project/classes/demo/Cache2.class.bin").to_vec()
        }
        "demo/Cache3" => {
            include_bytes!("fixtures/host_project/classes/demo/Cache3.class.bin").to_vec()
        }
        "demo/Cache4" => {
            include_bytes!("fixtures/host_project/classes/demo/Cache4.class.bin").to_vec()
        }
        "test/ClassBytesSamples" => {
            include_bytes!("fixtures/host_project/classes/test/ClassBytesSamples.class.bin")
                .to_vec()
        }
        "test/ConcurrentSamples" => {
            include_bytes!("fixtures/host_project/classes/test/ConcurrentSamples.class.bin")
                .to_vec()
        }
        "test/FieldAccessSamples" => {
            include_bytes!("fixtures/host_project/classes/test/FieldAccessSamples.class.bin")
                .to_vec()
        }
        "test/ConstantSamples" => {
            include_bytes!("fixtures/host_project/classes/test/ConstantSamples.class.bin").to_vec()
        }
        "test/InvokeClassificationSamples" => include_bytes!(
            "fixtures/host_project/classes/test/InvokeClassificationSamples.class.bin"
        )
        .to_vec(),
        "test/OverloadInvokeSamples" => {
            include_bytes!("fixtures/host_project/classes/test/OverloadInvokeSamples.class.bin")
                .to_vec()
        }
        "hierarchy/Base" => {
            include_bytes!("fixtures/host_project/classes/hierarchy/Base.class.bin").to_vec()
        }
        "hierarchy/ChildOne" => {
            include_bytes!("fixtures/host_project/classes/hierarchy/ChildOne.class.bin").to_vec()
        }
        "hierarchy/ChildTwo" => {
            include_bytes!("fixtures/host_project/classes/hierarchy/ChildTwo.class.bin").to_vec()
        }
        "hierarchy/Peer" => {
            include_bytes!("fixtures/host_project/classes/hierarchy/Peer.class.bin").to_vec()
        }
        _ => panic!("unknown saved compiler fixture {owner}"),
    }
}
type EntryClasses = BTreeMap<String, BTreeMap<String, Vec<u8>>>;
#[derive(Default)]
struct Host {
    classes: Mutex<EntryClasses>,
    versions: Mutex<BTreeMap<String, u64>>,
    catalogs: AtomicUsize,
    bytes_calls: AtomicUsize,
    scopes: AtomicUsize,
    closed: AtomicBool,
    cancelled: AtomicBool,
    unavailable: AtomicBool,
    expected: Mutex<Option<(String, HostGenerations)>>,
    reference_facts: Mutex<Option<BTreeMap<String, HostReferenceTypeFact>>>,
    assignability: Mutex<BTreeMap<(String, String), bool>>,
    locations: Mutex<BTreeMap<String, mcdev_core::mixin_navigation::ResolvedDefinition>>,
}
impl Host {
    fn new(entries: Vec<(&str, Vec<(&str, Vec<u8>)>)>) -> Arc<Self> {
        let host = Arc::new(Self::default());
        for (entry, classes) in entries {
            host.classes.lock().unwrap().insert(
                entry.into(),
                classes.into_iter().map(|(n, b)| (n.into(), b)).collect(),
            );
        }
        host
    }
    fn key(entry: &str, owner: &str) -> String {
        format!("{entry}::{owner}")
    }
    fn stamp(&self, entry: &str, owner: &str, missing: bool) -> HostReadStamp {
        let key = Self::key(entry, owner);
        HostReadStamp {
            kind: if owner == "*" { "catalog" } else { "class" }.into(),
            generation: *self.versions.lock().unwrap().get(&key).unwrap_or(&0),
            key,
            missing,
        }
    }
    fn mutate(&self, entry: &str, owner: &str, bytes: Option<Vec<u8>>) {
        let mut classes = self.classes.lock().unwrap();
        let map = classes.entry(entry.into()).or_default();
        if let Some(bytes) = bytes {
            map.insert(owner.into(), bytes);
        } else {
            map.remove(owner);
        }
        drop(classes);
        let mut versions = self.versions.lock().unwrap();
        for name in [owner, "*"] {
            *versions.entry(Self::key(entry, name)).or_default() += 1;
        }
    }
    fn scope(&self) -> Result<(), HostSourceError> {
        self.scopes.fetch_add(1, Ordering::Relaxed);
        if self.cancelled.load(Ordering::Relaxed) {
            Err(HostSourceError::Cancelled("owned cancellation".into()))
        } else {
            Ok(())
        }
    }
}
impl HostProjectQuery for Host {
    fn is_open(&self, _: &HostSessionIdentity) -> bool {
        !self.closed.load(Ordering::Relaxed)
    }
    fn validate(
        &self,
        id: &HostSessionIdentity,
        reads: &HostReadSet,
    ) -> Result<bool, HostSourceError> {
        self.scope()?;
        if let Some((root, generations)) = self.expected.lock().unwrap().as_ref() {
            if root != &id.workspace_root_uri || generations != &id.generations {
                return Ok(false);
            }
        }
        let versions = self.versions.lock().unwrap();
        Ok(reads
            .iter()
            .all(|r| r.generation == *versions.get(&r.key).unwrap_or(&0)))
    }
    fn catalog(
        &self,
        _: &HostSessionIdentity,
        entry: &HostClasspathEntry,
    ) -> Result<HostAnswer<Vec<String>>, HostSourceError> {
        self.scope()?;
        self.catalogs.fetch_add(1, Ordering::Relaxed);
        if self.unavailable.load(Ordering::Relaxed) {
            return Ok(HostAnswer::unavailable("catalog capability absent"));
        }
        let names = self
            .classes
            .lock()
            .unwrap()
            .get(&entry.id)
            .map(|v| v.keys().cloned().collect::<Vec<_>>());
        let reads = vec![self.stamp(&entry.id, "*", names.is_none())];
        Ok(match names {
            Some(names) => HostAnswer::known(names, reads),
            None => HostAnswer::missing(reads),
        })
    }
    fn class_bytes(
        &self,
        _: &HostSessionIdentity,
        entry: &HostClasspathEntry,
        owner: &str,
        _: HostBytesPurpose,
    ) -> Result<HostAnswer<Arc<[u8]>>, HostSourceError> {
        self.scope()?;
        self.bytes_calls.fetch_add(1, Ordering::Relaxed);
        if self.unavailable.load(Ordering::Relaxed) {
            return Ok(HostAnswer::unavailable("bytes capability absent"));
        }
        let bytes = self
            .classes
            .lock()
            .unwrap()
            .get(&entry.id)
            .and_then(|v| v.get(owner))
            .cloned();
        let reads = vec![self.stamp(&entry.id, owner, bytes.is_none())];
        Ok(match bytes {
            Some(v) => HostAnswer::known(Arc::from(v), reads),
            None => HostAnswer::missing(reads),
        })
    }
    fn reference_type(
        &self,
        _: &HostSessionIdentity,
        owner: &str,
    ) -> Result<HostAnswer<HostReferenceTypeFact>, HostSourceError> {
        self.scope()?;
        let facts = self.reference_facts.lock().unwrap();
        Ok(match facts.as_ref() {
            None => HostAnswer::unavailable("type facts absent"),
            Some(facts) => match facts.get(owner) {
                Some(value) => HostAnswer::known(value.clone(), vec![]),
                None => HostAnswer::missing(vec![]),
            },
        })
    }
    fn is_assignable(
        &self,
        _: &HostSessionIdentity,
        from: &str,
        to: &str,
    ) -> Result<HostAnswer<bool>, HostSourceError> {
        self.scope()?;
        Ok(
            match self
                .assignability
                .lock()
                .unwrap()
                .get(&(from.into(), to.into()))
            {
                Some(value) => HostAnswer::known(*value, vec![]),
                None => HostAnswer::unavailable("assignability fact absent"),
            },
        )
    }
    fn definition(
        &self,
        _: &HostSessionIdentity,
        target: &McDefinitionTarget,
    ) -> Result<HostAnswer<mcdev_core::mixin_navigation::ResolvedDefinition>, HostSourceError> {
        self.scope()?;
        Ok(
            match self
                .locations
                .lock()
                .unwrap()
                .get(&target.owner_internal_name)
            {
                Some(value) => HostAnswer::known(value.clone(), vec![]),
                None => HostAnswer::missing(vec![]),
            },
        )
    }
}
fn identity(entries: &[&str]) -> HostSessionIdentity {
    let facts = HostProjectFacts {
        classpath: ClasspathSnapshot {
            project_outputs: entries.iter().map(PathBuf::from).collect(),
            ..Default::default()
        },
        ..Default::default()
    };
    HostSessionIdentity {
        workspace_root_uri: "file:///workspace".into(),
        project_session_version: 7,
        generations: fixture_generations(&facts),
        facts,
    }
}
fn provider(
    entries: Vec<(&str, Vec<(&str, Vec<u8>)>)>,
) -> (Arc<Host>, Arc<HostClassBytesProvider>) {
    let paths = entries.iter().map(|(p, _)| *p).collect::<Vec<_>>();
    let id = identity(&paths);
    let facts = paths
        .iter()
        .map(|p| HostClasspathEntry {
            id: (*p).into(),
            path: (*p).into(),
            timestamp: Some(123),
        })
        .collect();
    let host = Host::new(entries);
    let provider = Arc::new(HostClassBytesProvider::new(id, facts, host.clone()));
    (host, provider)
}
#[derive(Default)]
struct Index {
    methods: BTreeMap<String, Vec<MethodIndexEntry>>,
    known: BTreeSet<String>,
    hook: Option<Arc<dyn Fn() + Send + Sync>>,
}
impl ClassIndex for Index {
    fn find_classes(&self, _: &str, _: usize) -> Vec<ClassIndexEntry> {
        Vec::new()
    }
    fn find_class(&self, owner: &str) -> Option<ClassIndexEntry> {
        if let Some(hook) = &self.hook {
            hook();
        }
        self.known.contains(owner).then(|| ClassIndexEntry {
            simple_name: owner.rsplit('/').next().unwrap().into(),
            package_name: owner
                .rsplit_once('/')
                .map_or("", |(p, _)| p)
                .replace('/', "."),
            internal_name: owner.into(),
        })
    }
    fn find_class_by_fqn(&self, _: &str) -> Option<ClassIndexEntry> {
        None
    }
    fn get_methods(&self, owner: &str) -> Vec<MethodIndexEntry> {
        self.methods.get(owner).cloned().unwrap_or_default()
    }
    fn get_fields(&self, _: &str) -> Vec<FieldIndexEntry> {
        Vec::new()
    }
}
fn index(owner: &str, methods: &[(&str, &str)]) -> Arc<Index> {
    Arc::new(Index {
        known: [owner.into()].into_iter().collect(),
        methods: [(
            owner.into(),
            methods
                .iter()
                .map(|(name, descriptor)| MethodIndexEntry {
                    name: (*name).into(),
                    descriptor: (*descriptor).into(),
                    is_static: false,
                    readable_signature: format!("{name}{descriptor}"),
                })
                .collect(),
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    })
}
fn adapter(
    owner: &str,
    methods: &[(&str, &str)],
) -> (Arc<HostClassBytesProvider>, HostBytecodeAdapter) {
    let (_, provider) = provider(vec![("classes", vec![(owner, fixture(owner))])]);
    let adapter = HostBytecodeAdapter::new(provider.clone(), index(owner, methods));
    (provider, adapter)
}

#[test]
fn classCountDiscoversNamesWithoutReadingClassContents() {
    let (_, p) = provider(vec![
        (
            "zzz-classes",
            vec![
                ("com/example/Alpha", MINIMAL.to_vec()),
                ("com/example/Beta", MINIMAL.to_vec()),
            ],
        ),
        (
            "aaa-lib.jar",
            vec![
                ("com/example/Alpha", MINIMAL.to_vec()),
                ("com/example/Beta", MINIMAL.to_vec()),
                ("com/example/Gamma", MINIMAL.to_vec()),
            ],
        ),
    ]);
    assert_eq!(p.byte_read_count(), 0);
    assert!(!p.catalog_initialized());
    assert_eq!(p.class_count(), 3);
    assert_eq!(p.byte_read_count(), 0);
    assert!(p.catalog_initialized());
    assert!(p.class_bytes_result("com/example/Beta").unwrap().is_some());
    assert_eq!(p.byte_read_count(), 1);
}
#[test]
fn getClassBytesReadsLazilyAndCachesPerOwner() {
    let (_, p) = provider(vec![
        (
            "aaa-lib.jar",
            vec![
                ("com/example/Alpha", tagged(b'A')),
                ("com/example/Beta", tagged(b'B')),
                ("com/example/Gamma", tagged(b'G')),
            ],
        ),
        ("zzz-classes", vec![("com/example/Delta", tagged(b'D'))]),
    ]);
    assert_eq!(p.byte_read_count(), 0);
    assert_eq!(p.get_class_bytes("com/example/Alpha"), Some(tagged(b'A')));
    assert_eq!(p.byte_read_count(), 1);
    assert_eq!(p.get_class_bytes("com/example/Alpha"), Some(tagged(b'A')));
    assert_eq!(p.byte_read_count(), 1);
    assert_eq!(p.get_class_bytes("com/example/Beta"), Some(tagged(b'B')));
    assert_eq!(p.byte_read_count(), 2);
}
#[test]
fn metadataQueriesDoNotReadClassBytes() {
    let (_, p) = provider(vec![
        (
            "aaa-lib.jar",
            vec![
                ("com/example/Alpha", tagged(b'A')),
                ("com/example/Beta", tagged(b'B')),
            ],
        ),
        ("zzz-classes", vec![("com/example/Gamma", tagged(b'G'))]),
    ]);
    assert_eq!(p.class_count(), 3);
    assert_eq!(
        p.find_internal_names("com/example", 2),
        ["com/example/Alpha", "com/example/Beta"]
    );
    assert_eq!(p.find_internal_names("bet", 5), ["com/example/Beta"]);
    assert_eq!(p.byte_read_count(), 0);
}
fn duplicates() -> Arc<HostClassBytesProvider> {
    provider(vec![
        (
            "zzz-classes",
            vec![
                ("dup/Shared", tagged(b'2')),
                ("dup/SecondOnly", MINIMAL.to_vec()),
            ],
        ),
        (
            "aaa-classes",
            vec![
                ("dup/Shared", tagged(b'1')),
                ("dup/FirstOnly", MINIMAL.to_vec()),
            ],
        ),
    ])
    .1
}
#[test]
fn getClassBytesAppliesDuplicateNameSemanticsAcrossEntries() {
    let p = duplicates();
    assert_eq!(p.class_count(), 3);
    assert_eq!(p.get_class_bytes("dup/Shared"), Some(tagged(b'1')));
    assert_eq!(p.byte_read_count(), 1);
    assert_eq!(p.get_class_bytes("dup/FirstOnly"), Some(MINIMAL.to_vec()));
    assert_eq!(p.byte_read_count(), 2);
}
#[test]
fn findInternalNamesAppliesDuplicateNameSemanticsAcrossEntries() {
    let p = duplicates();
    assert_eq!(
        p.find_internal_names("dup/", 10),
        ["dup/FirstOnly", "dup/SecondOnly", "dup/Shared"]
    );
    assert_eq!(p.class_count(), 3);
    assert_eq!(p.byte_read_count(), 0);
}
#[test]
fn concurrentGetClassBytesForSameOwnerReadsClassBytesOnce() {
    let (_, p) = provider(vec![(
        "classes",
        vec![("com/example/Shared", tagged(b'S'))],
    )]);
    assert_eq!(p.byte_read_count(), 0);
    let barrier = Arc::new(std::sync::Barrier::new(32));
    let tasks = (0..32)
        .map(|_| {
            let (p, barrier) = (p.clone(), barrier.clone());
            thread::spawn(move || {
                barrier.wait();
                p.get_class_bytes("com/example/Shared")
            })
        })
        .collect::<Vec<_>>();
    for task in tasks {
        assert_eq!(task.join().unwrap(), Some(tagged(b'S')));
    }
    assert_eq!(p.byte_read_count(), 1);
}
#[test]
fn acceptsInjectedBytesReadCounter() {
    let host = Host::new(vec![("classes", vec![("demo/Sample", MINIMAL.to_vec())])]);
    let counter = Arc::new(AtomicUsize::new(0));
    let p = HostClassBytesProvider::with_capacity(
        identity(&["classes"]),
        vec![HostClasspathEntry {
            id: "classes".into(),
            path: "classes".into(),
            timestamp: None,
        }],
        host,
        256,
        counter.clone(),
    );
    assert_eq!(counter.load(Ordering::Relaxed), 0);
    p.get_class_bytes("demo/Sample");
    assert_eq!(counter.load(Ordering::Relaxed), 1);
    assert_eq!(p.byte_read_count(), 1);
}

#[test]
fn classSearchReadsNoBytesAndMemberQueriesIndexLazilyPerOwner() {
    let (_, p) = provider(vec![(
        "classes",
        vec![
            ("demo/Alpha", fixture("demo/Alpha")),
            ("demo/Beta", fixture("demo/Beta")),
        ],
    )]);
    let i = LazyHostClassIndex::new(p.clone());
    i.find_classes("demo", 10);
    i.find_class("demo/Alpha");
    i.find_class_by_fqn("demo.Alpha");
    assert_eq!(p.byte_read_count(), 0);
    assert_eq!(i.asm_parse_count(), 0);
    i.get_methods("demo/Alpha");
    assert_eq!(p.byte_read_count(), 1);
    assert_eq!(i.asm_parse_count(), 1);
    assert!(i.get_methods("demo/Alpha").iter().any(|m| m.name == "run"));
    i.get_methods("demo/Alpha");
    i.get_fields("demo/Alpha");
    assert_eq!(p.byte_read_count(), 1);
    assert_eq!(i.asm_parse_count(), 1);
    assert!(i.get_fields("demo/Alpha").iter().any(|f| f.name == "value"));
    i.get_methods("demo/Beta");
    assert_eq!(p.byte_read_count(), 2);
    assert_eq!(i.asm_parse_count(), 2);
    i.get_methods("demo/Missing");
    i.get_fields("demo/Missing");
    assert_eq!(p.byte_read_count(), 2);
    assert_eq!(i.asm_parse_count(), 2);
}
#[test]
fn exactOwnerMemberLookupBypassesProviderCatalog() {
    let (_, p) = provider(vec![(
        "classes",
        vec![("demo/Alpha", fixture("exact/demo/Alpha"))],
    )]);
    let i = LazyHostClassIndex::new(p.clone());
    assert!(!p.catalog_initialized());
    i.get_methods("demo/Alpha");
    assert!(!p.catalog_initialized());
    assert_eq!(p.byte_read_count(), 1);
    assert_eq!(i.asm_parse_count(), 1);
    i.get_methods("demo/Missing");
    i.get_fields("demo/Missing");
    assert!(!p.catalog_initialized());
    assert_eq!(p.byte_read_count(), 1);
    assert_eq!(i.asm_parse_count(), 1);
}
#[test]
fn ownerIndexCacheStaysBounded() {
    let classes = (0..5).map(|i| format!("demo/Cache{i}")).collect::<Vec<_>>();
    let (_, p) = provider(vec![(
        "classes",
        classes.iter().map(|o| (o.as_str(), fixture(o))).collect(),
    )]);
    let i = LazyHostClassIndex::with_capacity(p, 3);
    for owner in classes {
        i.get_methods(&owner);
    }
    assert_eq!(i.cached_owner_count(), 3);
    assert_eq!(i.asm_parse_count(), 5);
}
fn session(empty: bool) -> HostProjectSession {
    let classes = if empty {
        Vec::new()
    } else {
        vec![(
            "classes",
            vec![
                ("demo/Alpha", fixture("demo/Alpha")),
                ("demo/Beta", fixture("demo/Beta")),
            ],
        )]
    };
    let host = Host::new(classes);
    HostProjectSession::create(
        Arc::new(ProjectContextBuilder::empty("session-test", "/workspace")),
        identity(if empty { &[] } else { &["classes"] }),
        host,
    )
}
fn startup(s: &HostProjectSession) {
    assert_eq!(s.class_bytes_provider.byte_read_count(), 0);
    assert!(!s.class_bytes_provider.catalog_initialized());
    assert_eq!(s.binary_index.asm_parse_count(), 0);
}
#[test]
fn createAndReindexStayLazyUntilMemberQuery() {
    let s = session(false);
    assert_eq!(s.context.index_state, ProjectIndexState::Ready);
    startup(&s);
    let r = s.reindex();
    assert_eq!(r.context.index_state, ProjectIndexState::Ready);
    startup(&r);
    s.class_index.find_classes("demo", 10);
    s.class_index.find_class("demo/Alpha");
    s.class_index.find_class_by_fqn("demo.Beta");
    assert_eq!(s.class_bytes_provider.byte_read_count(), 0);
    assert_eq!(s.binary_index.asm_parse_count(), 0);
    s.class_index.get_methods("demo/Alpha");
    assert_eq!(s.class_bytes_provider.byte_read_count(), 1);
    assert_eq!(s.binary_index.asm_parse_count(), 1);
    assert!(s
        .class_index
        .get_methods("demo/Alpha")
        .iter()
        .any(|m| m.name == "run"));
    r.class_index.find_classes("Alpha", 10);
    r.class_index.find_class_by_fqn("demo.Beta");
    assert_eq!(r.class_bytes_provider.byte_read_count(), 0);
    assert_eq!(r.binary_index.asm_parse_count(), 0);
    r.class_index.get_methods("demo/Beta");
    assert_eq!(r.class_bytes_provider.byte_read_count(), 1);
    assert_eq!(r.binary_index.asm_parse_count(), 1);
    assert!(r
        .class_index
        .get_methods("demo/Beta")
        .iter()
        .any(|m| m.name == "go"));
}
#[test]
fn createAndReindexWithEmptyClasspathStayNotReadyWithoutCatalogScan() {
    let s = session(true);
    assert_eq!(s.context.index_state, ProjectIndexState::NotReady);
    startup(&s);
    let r = s.reindex();
    assert_eq!(r.context.index_state, ProjectIndexState::NotReady);
    startup(&r);
}
#[test]
fn sessionIndexesClasspathClasses() {
    let temp = Temp::new();
    copy_inputs(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/mixin_completion/project_files/fabric-basic"),
        &temp.0,
    );
    let bytes =
        include_bytes!("fixtures/mixin_completion/project_files/SimpleTarget.class.bin").to_vec();
    let entry = temp.0.join("classpath");
    let path = entry.join("com/example/target/SimpleTarget.class");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, &bytes).unwrap();
    let entry = entry.to_string_lossy().into_owned();
    let host = Host::new(vec![(
        &entry,
        vec![("com/example/target/SimpleTarget", bytes.clone())],
    )]);
    let uri = path_to_uri(&temp.0);
    let mut id = identity(&[&entry]);
    id.workspace_root_uri = uri.clone();
    let service = FileBasedProjectContextService::default();
    let cached = service.load_cached_session(&uri, Some(&id.facts)).unwrap();
    id.project_session_version = cached.version;
    let sessions = HostProjectSessions::default();
    let (s, hit) = sessions
        .load(&cached, id, host, Arc::new(|binary| binary))
        .unwrap();
    assert!(!hit);
    assert!(s.class_bytes_provider.class_count() >= 1);
    assert_eq!(s.context.index_state, ProjectIndexState::Ready);
    assert_eq!(s.context.platform, ModPlatform::Fabric);
    assert_eq!(
        s.bytecode_index
            .get_class_bytes("com/example/target/SimpleTarget"),
        Some(bytes)
    );
}

#[test]
fn getClassBytesReturnsExactProviderBytesForExistingClass() {
    let (p, a) = adapter("test/ClassBytesSamples", &[]);
    let provider_bytes = p
        .class_bytes_result("test/ClassBytesSamples")
        .unwrap()
        .unwrap();
    let adapter_bytes = a
        .get_class_bytes_arc("test/ClassBytesSamples")
        .unwrap()
        .unwrap();
    assert!(Arc::ptr_eq(&provider_bytes, &adapter_bytes));
    assert_eq!(&*adapter_bytes, fixture("test/ClassBytesSamples"));
}
#[test]
fn getClassBytesReturnsNullForMissingClass() {
    let (_, p) = provider(vec![("classes", vec![])]);
    let a = HostBytecodeAdapter::new(p, Arc::new(Index::default()));
    assert_eq!(a.get_class_bytes("missing/Class"), None);
}
#[test]
fn getClassBytesDoesNotPopulateAdapterCachesOrEnumerateClasspath() {
    let (_, p) = provider(vec![(
        "lib.jar",
        vec![
            ("com/example/Alpha", tagged(b'A')),
            ("com/example/Beta", tagged(b'B')),
        ],
    )]);
    assert!(!p.catalog_initialized());
    assert_eq!(p.byte_read_count(), 0);
    let a = HostBytecodeAdapter::new(p.clone(), Arc::new(Index::default()));
    assert_eq!(a.candidate_cache_size(), 0);
    assert_eq!(a.return_count_cache_size(), 0);
    a.get_class_bytes("com/example/Alpha");
    assert_eq!(a.candidate_cache_size(), 0);
    assert_eq!(a.return_count_cache_size(), 0);
    assert!(!p.catalog_initialized());
    assert_eq!(p.byte_read_count(), 1);
}
fn hierarchy() -> (Arc<HostClassBytesProvider>, HostBytecodeAdapter) {
    let (_, p) = provider(vec![(
        "hierarchy.jar",
        ["hierarchy/Base", "hierarchy/ChildOne", "hierarchy/ChildTwo"]
            .into_iter()
            .map(|n| (n, fixture(n)))
            .collect(),
    )]);
    let a = HostBytecodeAdapter::new(p.clone(), Arc::new(Index::default()));
    (p, a)
}
#[test]
fn resolveCommonSuperClassResolvesSiblingSuperclassAndArrayCases() {
    let (_, a) = hierarchy();
    for (l, r, expected) in [
        (
            "Lhierarchy/ChildOne;",
            "Lhierarchy/ChildTwo;",
            "Lhierarchy/Base;",
        ),
        (
            "Lhierarchy/ChildOne;",
            "Lhierarchy/Base;",
            "Lhierarchy/Base;",
        ),
        (
            "[Lhierarchy/ChildOne;",
            "[Lhierarchy/Base;",
            "[Lhierarchy/Base;",
        ),
    ] {
        assert_eq!(
            a.resolve_common_super_class(l, r).as_deref(),
            Some(expected)
        );
    }
}
#[test]
fn resolveCommonSuperClassReverseOrderReusesByteReads() {
    let (p, a) = hierarchy();
    let first = a.resolve_common_super_class("Lhierarchy/ChildOne;", "Lhierarchy/ChildTwo;");
    let reads = p.byte_read_count();
    let second = a.resolve_common_super_class("Lhierarchy/ChildTwo;", "Lhierarchy/ChildOne;");
    assert_eq!(first.as_deref(), Some("Lhierarchy/Base;"));
    assert_eq!(first, second);
    assert_eq!(reads, p.byte_read_count());
}
#[test]
fn resolveCommonSuperClassMissingOrCorruptReturnsNull() {
    let (_, p) = provider(vec![]);
    let a = HostBytecodeAdapter::new(p, Arc::new(Index::default()));
    assert_eq!(
        a.resolve_common_super_class("Lcom/missing/Alpha;", "Lcom/missing/Beta;"),
        None
    );
    let (_, p) = provider(vec![(
        "hierarchy-corrupt.jar",
        vec![
            ("hierarchy/Corrupt", MINIMAL.to_vec()),
            ("hierarchy/Peer", fixture("hierarchy/Peer")),
        ],
    )]);
    let a = HostBytecodeAdapter::new(p, Arc::new(Index::default()));
    assert_eq!(
        a.resolve_common_super_class("Lhierarchy/Corrupt;", "Lhierarchy/Peer;"),
        None
    );
}
#[test]
fn resolveCommonSuperClassDoesNotPopulateAdapterCachesOrEnumerateClasspath() {
    let (p, a) = hierarchy();
    assert!(!p.catalog_initialized());
    assert_eq!(p.byte_read_count(), 0);
    assert_eq!(a.candidate_cache_size(), 0);
    assert_eq!(a.return_count_cache_size(), 0);
    a.resolve_common_super_class("Lhierarchy/ChildOne;", "Lhierarchy/ChildTwo;");
    assert_eq!(a.candidate_cache_size(), 0);
    assert_eq!(a.return_count_cache_size(), 0);
    assert!(!p.catalog_initialized());
    assert!(p.byte_read_count() > 0);
}
#[test]
fn overloadedMethodWithoutDescriptorUnionsDistinctInvokeCandidates() {
    let (_, a) = adapter(
        "test/OverloadInvokeSamples",
        &[("run", "()V"), ("run", "(I)V")],
    );
    let all = a.get_at_target_candidates("test/OverloadInvokeSamples", "run", None, "INVOKE");
    assert_eq!(all.len(), 2);
    assert!(all.iter().any(|c| c.name == "println"));
    assert!(all.iter().any(|c| c.name == "abs"));
    assert!(all.iter().all(|c| c.kind == AtTargetKind::Invoke));
    let void =
        a.get_at_target_candidates("test/OverloadInvokeSamples", "run", Some("()V"), "INVOKE");
    assert_eq!(void.len(), 1);
    assert_eq!(void[0].name, "println");
    let int =
        a.get_at_target_candidates("test/OverloadInvokeSamples", "run", Some("(I)V"), "INVOKE");
    assert_eq!(int.len(), 1);
    assert_eq!(int[0].name, "abs");
}
#[test]
fn returnCountCacheIsBoundedAcrossDistinctKeys() {
    let (_, p) = provider(vec![]);
    let a = HostBytecodeAdapter::new(p, Arc::new(Index::default()));
    for i in 0..600 {
        a.get_return_ordinal_count("example/Target", &format!("method{i}"), Some("()V"));
    }
    assert!(a.return_count_cache_size() <= 256);
}
#[test]
fn atTargetCandidateCacheIsBoundedAcrossDistinctKeys() {
    let (_, p) = provider(vec![]);
    let a = HostBytecodeAdapter::new(p, Arc::new(Index::default()));
    for i in 0..600 {
        a.get_at_target_candidates(
            "example/Target",
            &format!("method{i}"),
            Some("()V"),
            "INVOKE",
        );
    }
    assert!(a.candidate_cache_size() <= 256);
}
#[test]
fn distinctCandidateCacheKeysComputeConcurrentlyOutsideGlobalCacheLock() {
    let (_, p) = provider(vec![(
        "classes",
        vec![("test/ConcurrentSamples", fixture("test/ConcurrentSamples"))],
    )]);
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let hook = {
        let (active, maximum) = (active.clone(), maximum.clone());
        Arc::new(move || {
            let n = active.fetch_add(1, Ordering::SeqCst) + 1;
            maximum.fetch_max(n, Ordering::SeqCst);
            barrier.wait();
            active.fetch_sub(1, Ordering::SeqCst);
        }) as Arc<dyn Fn() + Send + Sync>
    };
    let a = Arc::new(HostBytecodeAdapter::new(
        p,
        Arc::new(Index {
            hook: Some(hook),
            ..Default::default()
        }),
    ));
    let start = Arc::new(std::sync::Barrier::new(2));
    let tasks = ["alpha", "beta"]
        .into_iter()
        .map(|name| {
            let (a, start) = (a.clone(), start.clone());
            thread::spawn(move || {
                start.wait();
                a.get_at_target_candidates("test/ConcurrentSamples", name, Some("()V"), "INVOKE")
            })
        })
        .collect::<Vec<_>>();
    for task in tasks {
        task.join().unwrap();
    }
    assert_eq!(maximum.load(Ordering::SeqCst), 2);
}
#[test]
fn fieldGetAndPutAtSameOwnerNameDescriptorRemainDistinct() {
    let (_, a) = adapter("test/FieldAccessSamples", &[("accessFields", "()V")]);
    let c = a.get_at_target_candidates(
        "test/FieldAccessSamples",
        "accessFields",
        Some("()V"),
        "FIELD",
    );
    assert_eq!(c.len(), 4);
    let static_fields = c
        .iter()
        .filter(|c| c.name == "STATIC_FIELD" && c.descriptor == "I")
        .collect::<Vec<_>>();
    assert_eq!(static_fields.len(), 2);
    assert!(static_fields
        .iter()
        .any(|c| c.operation_kind == Some(AtTargetOperationKind::FieldGetStatic)));
    assert!(static_fields
        .iter()
        .any(|c| c.operation_kind == Some(AtTargetOperationKind::FieldPutStatic)));
    let fields = c
        .iter()
        .filter(|c| c.name == "instanceField" && c.descriptor == "I")
        .collect::<Vec<_>>();
    assert_eq!(fields.len(), 2);
    assert!(fields
        .iter()
        .any(|c| c.operation_kind == Some(AtTargetOperationKind::FieldGetInstance)));
    assert!(fields
        .iter()
        .any(|c| c.operation_kind == Some(AtTargetOperationKind::FieldPutInstance)));
}
#[test]
fn operationKindMapsBytecodeKindsCorrectly() {
    let (_, a) = adapter(
        "test/OverloadInvokeSamples",
        &[("run", "()V"), ("run", "(I)V")],
    );
    let c = a.get_at_target_candidates("test/OverloadInvokeSamples", "run", None, "INVOKE");
    assert_eq!(
        c.iter()
            .find(|c| c.name == "println")
            .unwrap()
            .operation_kind,
        Some(AtTargetOperationKind::InvokeVirtual)
    );
    assert_eq!(
        c.iter().find(|c| c.name == "abs").unwrap().operation_kind,
        Some(AtTargetOperationKind::InvokeStatic)
    );
    let (_, a) = adapter("test/FieldAccessSamples", &[("accessFields", "()V")]);
    let c = a.get_at_target_candidates(
        "test/FieldAccessSamples",
        "accessFields",
        Some("()V"),
        "FIELD",
    );
    let kinds = c
        .iter()
        .filter_map(|c| c.operation_kind)
        .map(|k| k as u8)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        kinds,
        [
            AtTargetOperationKind::FieldGetStatic,
            AtTargetOperationKind::FieldPutStatic,
            AtTargetOperationKind::FieldGetInstance,
            AtTargetOperationKind::FieldPutInstance
        ]
        .into_iter()
        .map(|k| k as u8)
        .collect()
    );
}
#[test]
fn distinctConstantValuesAtSameOrdinalAreBothRetained() {
    let (_, a) = adapter("test/ConstantSamples", &[("smallInts", "()V")]);
    let c =
        a.get_at_target_candidates("test/ConstantSamples", "smallInts", Some("()V"), "CONSTANT");
    assert_eq!(c.len(), 2);
    assert!(c.iter().all(|c| c.ordinal == Some(0)));
    assert!(c.iter().all(|c| c.kind == AtTargetKind::Constant));
    assert_eq!(
        c.iter()
            .filter_map(|c| match c.constant_value {
                Some(ConstantValue::IntValue(v)) => Some(v),
                _ => None,
            })
            .collect::<BTreeSet<_>>(),
        [-1, 5].into_iter().collect()
    );
}
#[test]
fn invokeOccurrenceClassificationAndIndexPropagateWithoutCollapsing() {
    let (_, a) = adapter(
        "test/InvokeClassificationSamples",
        &[("mixedAbsCalls", "()V")],
    );
    let mut c = a
        .get_at_target_candidates(
            "test/InvokeClassificationSamples",
            "mixedAbsCalls",
            Some("()V"),
            "INVOKE",
        )
        .into_iter()
        .filter(|c| c.owner == "java/lang/Math" && c.name == "abs" && c.descriptor == "(I)I")
        .collect::<Vec<_>>();
    c.sort_by_key(|c| c.instruction_occurrence_index);
    assert_eq!(c.len(), 2);
    assert_eq!(
        c[0].occurrence_result_classification,
        OccurrenceResultClassification::ImmediatelyPopped
    );
    assert_eq!(
        c[1].occurrence_result_classification,
        OccurrenceResultClassification::Retained
    );
    assert!(c[0].instruction_occurrence_index >= 0);
    assert!(c[1].instruction_occurrence_index > c[0].instruction_occurrence_index);
}
#[test]
fn unknownMethodWithoutDescriptorReturnsNoCandidates() {
    let (_, a) = adapter(
        "test/OverloadInvokeSamples",
        &[("run", "()V"), ("run", "(I)V")],
    );
    assert!(a
        .get_at_target_candidates("test/OverloadInvokeSamples", "missing", None, "INVOKE")
        .is_empty());
}

fn method(params: &[&str]) -> HostMethodSignature {
    HostMethodSignature {
        name: "process".into(),
        parameter_types: Some(params.iter().map(|v| (*v).into()).collect()),
    }
}
#[test]
fn absentDescriptorReturnsFirstSameNameOverload() {
    let methods = [method(&["I"]), method(&["Ljava/lang/String;"])];
    assert!(std::ptr::eq(
        select_host_method(&methods, "process", None, true).unwrap(),
        &methods[0]
    ));
}
#[test]
fn blankDescriptorReturnsFirstSameNameOverload() {
    let methods = [method(&["I"]), method(&["Ljava/lang/String;"])];
    assert!(std::ptr::eq(
        select_host_method(&methods, "process", Some("   "), true).unwrap(),
        &methods[0]
    ));
}
#[test]
fn explicitDescriptorExactMatchSelectsMatchingOverload() {
    let methods = [method(&["I"]), method(&["Ljava.lang.String;"])];
    assert!(std::ptr::eq(
        select_host_method(&methods, "process", Some("(Ljava/lang/String;)V"), true).unwrap(),
        &methods[1]
    ));
}
#[test]
fn explicitDescriptorMatchesPrimitiveArrayAndErasedGenericOverloads() {
    let methods = [
        method(&["Ljava.util.List<Ljava.lang.String;>;"]),
        method(&["I"]),
        method(&["[Lnet.minecraft.world.item.ItemStack;"]),
    ];
    for (desc, i) in [
        ("([Lnet/minecraft/world/item/ItemStack;)V", 2),
        ("(I)V", 1),
        ("(Ljava/util/List;)V", 0),
    ] {
        assert!(std::ptr::eq(
            select_host_method(&methods, "process", Some(desc), true).unwrap(),
            &methods[i]
        ));
    }
}
#[test]
fn explicitDescriptorWithAmbiguousNormalizedMatchesReturnsNull() {
    let methods = [
        method(&["Ljava.util.List<Ljava.lang.String;>;"]),
        method(&["Ljava.util.List;"]),
    ];
    assert!(select_host_method(&methods, "process", Some("(Ljava/util/List;)V"), true).is_none());
}
#[test]
fn onlyQualifiedUnresolvedTypesAreNormalized() {
    let qualified = [method(&["Qnet.minecraft.ItemStack;"])];
    let simple = [method(&["QString;"])];
    assert!(std::ptr::eq(
        select_host_method(
            &qualified,
            "process",
            Some("(Lnet/minecraft/ItemStack;)V"),
            true
        )
        .unwrap(),
        &qualified[0]
    ));
    assert!(select_host_method(&simple, "process", Some("(Ljava/lang/String;)V"), true).is_none());
}
#[test]
fn explicitDescriptorMismatchReturnsNull() {
    assert!(select_host_method(
        &[method(&["I"]), method(&["Ljava/lang/String;"])],
        "process",
        Some("(D)V"),
        true
    )
    .is_none());
}
#[test]
fn explicitDescriptorWhenParameterExtractionFailsReturnsNull() {
    assert!(select_host_method(&[method(&["I"])], "process", Some("(I)V"), false).is_none());
}
#[test]
fn explicitDescriptorWhenSignatureUnavailableReturnsNull() {
    assert!(select_host_method(&[method(&["I"])], "process", Some("(I)V"), false).is_none());
}
#[test]
fn explicitDescriptorWhenMethodParameterExtractionFailsReturnsNull() {
    assert!(select_host_method(
        &[HostMethodSignature {
            name: "process".into(),
            parameter_types: None
        }],
        "process",
        Some("(I)V"),
        true
    )
    .is_none());
}

#[test]
fn discoversLoomRemappedJars() {
    let root = PathBuf::from("/workspace");
    let selected = [HostClasspathPath {
        path: root.join(".gradle/loom-cache/remapped_working/minecraft-client-mapped.jar"),
        exists: true,
        modified_at: 0,
        kind: HostClasspathKind::MinecraftJar,
    }];
    let discovered = HostProjectBridge::selected_loom_jars(&root, &selected);
    assert_eq!(discovered.len(), 1);
    assert!(discovered[0]
        .file_name()
        .unwrap()
        .to_string_lossy()
        .contains("minecraft"));
}
#[test]
fn mergeClasspathAddsMinecraftJarsFromLoomNames() {
    let merged = HostProjectBridge::merge_classpath(
        &ClasspathSnapshot::default(),
        &[HostClasspathPath {
            path: "/workspace/minecraft-client-mapped.jar".into(),
            exists: true,
            modified_at: 0,
            kind: HostClasspathKind::MinecraftJar,
        }],
        1,
    );
    assert_eq!(merged.minecraft_jars.len(), 1);
}
#[test]
fn mergeWithJdtClasspathReturnsBaseWhenJdtUnavailable() {
    let base = ClasspathSnapshot {
        project_outputs: vec!["/workspace/classes".into()],
        ..Default::default()
    };
    assert_eq!(HostProjectBridge::merge_classpath(&base, &[], 1), base);
}

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "mcdev-host-project-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root.canonicalize().unwrap())
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let base = std::env::temp_dir().canonicalize().unwrap();
        let root = self.0.canonicalize().unwrap();
        assert!(
            root.starts_with(&base)
                && root.parent() == Some(base.as_path())
                && root
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("mcdev-host-project-")
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
fn copy_inputs(source: &std::path::Path, target: &std::path::Path) {
    std::fs::create_dir_all(target).unwrap();
    for item in std::fs::read_dir(source).unwrap() {
        let item = item.unwrap();
        let to = target.join(item.file_name());
        let kind = item.file_type().unwrap();
        if kind.is_dir() {
            copy_inputs(&item.path(), &to)
        } else {
            assert!(kind.is_file());
            std::fs::copy(item.path(), to).unwrap();
        }
    }
}

#[test]
fn owned_positive_and_negative_reads_invalidate_precedence_and_catalog() {
    let (host, p) = provider(vec![
        ("aaa-classes", vec![]),
        ("zzz-classes", vec![("demo/Shared", tagged(b'2'))]),
    ]);
    assert_eq!(p.get_class_bytes("demo/Shared"), Some(tagged(b'2')));
    assert_eq!(p.class_count(), 1);
    assert!(p.get_class_bytes("demo/Later").is_none());
    let scopes = host.scopes.load(Ordering::Relaxed);
    host.mutate("aaa-classes", "demo/Shared", Some(tagged(b'1')));
    host.mutate("aaa-classes", "demo/Later", Some(tagged(b'N')));
    assert_eq!(p.get_class_bytes("demo/Shared"), Some(tagged(b'1')));
    assert_eq!(p.get_class_bytes("demo/Later"), Some(tagged(b'N')));
    assert_eq!(p.class_count(), 2);
    assert!(host.scopes.load(Ordering::Relaxed) > scopes);
    assert!(p.current_revision().unwrap() > 0);
}
#[test]
fn source_and_bytecode_caches_drop_changed_owner_bytes() {
    let (host, p) = provider(vec![(
        "classes",
        vec![("demo/Alpha", fixture("demo/Alpha"))],
    )]);
    let i = Arc::new(LazyHostClassIndex::new(p.clone()));
    assert!(i.get_methods("demo/Alpha").iter().any(|m| m.name == "run"));
    let a = HostBytecodeAdapter::new(p.clone(), i.clone());
    assert_eq!(
        a.get_at_target_candidates("demo/Alpha", "run", Some("()V"), "RETURN")
            .len(),
        1
    );
    host.mutate("classes", "demo/Alpha", Some(fixture("demo/Beta")));
    assert!(!i.get_methods("demo/Alpha").iter().any(|m| m.name == "run"));
    assert!(i.get_methods("demo/Alpha").iter().any(|m| m.name == "go"));
    assert_eq!(i.asm_parse_count(), 2);
    assert!(a
        .get_at_target_candidates("demo/Alpha", "run", Some("()V"), "RETURN")
        .is_empty());
    assert_eq!(p.byte_read_count(), 2);
}
#[test]
fn missing_hierarchy_reads_are_revalidated_after_host_mutation() {
    let (host, p) = provider(vec![(
        "classes",
        vec![
            ("hierarchy/ChildOne", fixture("hierarchy/ChildOne")),
            ("hierarchy/Base", fixture("hierarchy/Base")),
        ],
    )]);
    let a = HostBytecodeAdapter::new(p.clone(), Arc::new(Index::default()));
    assert_eq!(
        a.resolve_common_super_class("Lhierarchy/ChildOne;", "Lhierarchy/ChildTwo;"),
        None
    );
    host.mutate(
        "classes",
        "hierarchy/ChildTwo",
        Some(fixture("hierarchy/ChildTwo")),
    );
    assert_eq!(
        a.resolve_common_super_class("Lhierarchy/ChildOne;", "Lhierarchy/ChildTwo;")
            .as_deref(),
        Some("Lhierarchy/Base;")
    );
}
#[test]
fn all_eight_generations_and_workspace_identity_reject_stale_reuse() {
    let (host, p) = provider(vec![(
        "classes",
        vec![("demo/Alpha", fixture("demo/Alpha"))],
    )]);
    p.get_class_bytes("demo/Alpha");
    let original = p.identity.generations;
    let mut changed = Vec::new();
    macro_rules! dimension {
        ($field:ident) => {{
            let mut value = original;
            value.$field += 1;
            changed.push(value);
        }};
    }
    dimension!(project);
    dimension!(model);
    dimension!(namespace);
    dimension!(classpath);
    dimension!(dependency);
    dimension!(generated_source);
    dimension!(config);
    dimension!(provider);
    for generations in changed {
        *host.expected.lock().unwrap() = Some((p.identity.workspace_root_uri.clone(), generations));
        assert!(p.class_bytes_result("demo/Alpha").is_err());
    }
    *host.expected.lock().unwrap() = Some(("file:///other-workspace".into(), original));
    assert!(p.catalog_result().is_err());
    *host.expected.lock().unwrap() = Some((p.identity.workspace_root_uri.clone(), original));
    assert!(p.class_bytes_result("demo/Alpha").unwrap().is_some());
    assert_eq!(p.byte_read_count(), 1);
}
#[test]
fn cancellation_unavailable_and_closed_are_not_cached_as_absence() {
    let (host, p) = provider(vec![(
        "classes",
        vec![("test/ClassBytesSamples", fixture("test/ClassBytesSamples"))],
    )]);
    let a = HostBytecodeAdapter::new(p.clone(), Arc::new(Index::default()));
    host.cancelled.store(true, Ordering::Relaxed);
    assert_eq!(
        p.class_bytes_result("test/ClassBytesSamples").unwrap_err(),
        HostSourceError::Cancelled("owned cancellation".into())
    );
    assert!(matches!(
        a.candidates_result("test/ClassBytesSamples", "<init>", Some("()V"), "INVOKE"),
        Err(HostSourceError::Cancelled(_))
    ));
    assert_eq!(p.byte_read_count(), 0);
    host.cancelled.store(false, Ordering::Relaxed);
    host.unavailable.store(true, Ordering::Relaxed);
    assert!(p.catalog_result().is_err());
    assert!(!p.catalog_initialized());
    assert!(a
        .candidates_result("test/ClassBytesSamples", "<init>", Some("()V"), "INVOKE")
        .is_err());
    assert_eq!(a.candidate_cache_size(), 0);
    host.unavailable.store(false, Ordering::Relaxed);
    assert_eq!(
        a.candidates_result("test/ClassBytesSamples", "<init>", Some("()V"), "INVOKE")
            .unwrap()
            .len(),
        1
    );
    p.close();
    assert!(p.class_bytes_result("test/ClassBytesSamples").is_err());
    assert!(a
        .get_at_target_candidates("test/ClassBytesSamples", "<init>", Some("()V"), "INVOKE")
        .is_empty());
    assert_eq!(p.class_count(), 0);
}
#[test]
fn provider_lru_touches_hits_and_reloads_only_evicted_owner() {
    let host = Host::new(vec![(
        "classes",
        vec![
            ("A", tagged(b'A')),
            ("B", tagged(b'B')),
            ("C", tagged(b'C')),
        ],
    )]);
    let p = HostClassBytesProvider::with_capacity(
        identity(&["classes"]),
        vec![HostClasspathEntry {
            id: "classes".into(),
            path: "classes".into(),
            timestamp: None,
        }],
        host,
        2,
        Arc::new(AtomicUsize::new(0)),
    );
    for owner in ["A", "B", "A", "C"] {
        p.get_class_bytes(owner);
    }
    assert_eq!(p.byte_read_count(), 3);
    assert_eq!(p.cached_owner_count(), 2);
    p.get_class_bytes("A");
    assert_eq!(p.byte_read_count(), 3);
    p.get_class_bytes("B");
    assert_eq!(p.byte_read_count(), 4);
    assert_eq!(p.cached_owner_count(), 2);
}
#[test]
fn fingerprints_metadata_and_unicode_follow_selected_entry_facts() {
    use sha2::{Digest, Sha256};
    let (_, p) = provider(vec![
        (
            "zzz-classes",
            vec![("例/Ångström", tagged(b'U')), ("dup/Shared", tagged(b'2'))],
        ),
        ("aaa-lib.jar", vec![("dup/Shared", tagged(b'1'))]),
    ]);
    assert_eq!(
        p.classpath_entry_ids(),
        ["aaa-lib.jar".into(), "zzz-classes".into()]
            .into_iter()
            .collect()
    );
    assert_eq!(p.classpath_entry_hash("aaa-lib.jar"), Some(123));
    assert_eq!(p.classpath_entry_hash("missing"), None);
    assert_eq!(p.find_internal_names("ång", 3), ["例/Ångström"]);
    assert_eq!(
        p.find_internal_name_by_fqn("例.ångström").as_deref(),
        Some("例/Ångström")
    );
    let mut hash = Sha256::new();
    for (owner, bytes) in [("dup/Shared", tagged(b'1')), ("例/Ångström", tagged(b'U'))] {
        hash.update(owner.as_bytes());
        hash.update(bytes);
    }
    for entry in ["aaa-lib.jar", "zzz-classes"] {
        hash.update(entry.as_bytes());
        hash.update(b"123");
    }
    assert_eq!(
        p.fingerprint_result().unwrap(),
        format!("{:x}", hash.finalize())
    );
    assert_eq!(p.byte_read_count(), 0);
    assert_eq!(p.cached_owner_count(), 0);
}
#[test]
fn unicode_catalog_uses_java_utf16_string_order_not_scalar_order() {
    let (_, p) = provider(vec![(
        "classes",
        vec![
            ("\u{e000}/Type", MINIMAL.to_vec()),
            ("\u{10000}/Type", MINIMAL.to_vec()),
        ],
    )]);
    assert_eq!(p.internal_names(), ["\u{10000}/Type", "\u{e000}/Type"]);
    assert!(p.contains_internal_name("\u{e000}/Type"));
    assert!(p.contains_internal_name("\u{10000}/Type"));
}

fn enum_name(value: impl std::fmt::Debug) -> String {
    let text = format!("{value:?}");
    let mut out = String::new();
    for (i, c) in text.chars().enumerate() {
        if i > 0 && c.is_uppercase() {
            out.push('_');
        }
        out.push(c.to_ascii_uppercase());
    }
    out
}
fn candidate_json(c: &AtTargetCandidate) -> serde_json::Value {
    use serde_json::json;
    let constant = match &c.constant_value {
        None => serde_json::Value::Null,
        Some(value) => {
            let raw = serde_json::to_value(value).unwrap();
            if let Some(map) = raw.as_object() {
                let (kind, value) = map.iter().next().unwrap();
                json!({"kind":kind,"value":value})
            } else {
                json!({"kind":"NullValue","value":null})
            }
        }
    };
    json!({"owner":c.owner,"name":c.name,"descriptor":c.descriptor,"displayLabel":c.display_label,"detail":c.detail,"kind":enum_name(c.kind),"ordinal":c.ordinal,"namespace":enum_name(c.namespace),"operationKind":c.operation_kind.map(enum_name),"instructionOccurrenceIndex":c.instruction_occurrence_index,"occurrenceResultClassification":enum_name(c.occurrence_result_classification),"conditionOpcode":c.condition_opcode,"constantValue":constant})
}
#[test]
fn original_public_candidate_outputs_preserve_every_metadata_field() {
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/host_project/candidate_outputs.json")).unwrap();
    assert_eq!(rows.len(), 10);
    for row in rows {
        let owner = row["owner"].as_str().unwrap();
        let methods = if owner == "test/OverloadInvokeSamples" {
            vec![("run", "()V"), ("run", "(I)V")]
        } else if owner == "test/FieldAccessSamples" {
            vec![("accessFields", "()V")]
        } else if owner == "test/ConstantSamples" {
            vec![("smallInts", "()V")]
        } else if owner == "test/InvokeClassificationSamples" {
            vec![("mixedAbsCalls", "()V")]
        } else {
            Vec::new()
        };
        let (_, p) = provider(vec![("classes", vec![(owner, fixture(owner))])]);
        let idx: Arc<dyn ClassIndex> = if owner == "test/ClassBytesSamples" {
            Arc::new(Index::default())
        } else {
            index(owner, &methods)
        };
        let a = HostBytecodeAdapter::new(p, idx);
        let output = a
            .candidates_result(
                owner,
                row["method"].as_str().unwrap(),
                row["descriptor"].as_str(),
                row["atValue"].as_str().unwrap(),
            )
            .unwrap();
        assert_eq!(
            serde_json::Value::Array(output.iter().map(candidate_json).collect()),
            row["output"],
            "request {row}"
        );
    }
}

#[test]
fn normalized_reference_facts_remain_unknown_without_host_capability() {
    use mcdev_core::mixinextras_handler::HandlerReferenceTypeResolver;
    let s = session(false);
    let types = s.reference_types();
    assert_eq!(types.is_known_reference("java/lang/String"), None);
    assert_eq!(
        types.is_assignable("arbitrary/Type", "java/lang/Object"),
        None
    );
    assert!(matches!(
        types.reference_type_result("arbitrary/Type").unwrap(),
        HostFact::Unavailable { .. }
    ));
    assert_eq!(s.class_bytes_provider.byte_read_count(), 0);
    assert!(!s.class_bytes_provider.catalog_initialized());
}

#[test]
fn session_store_reuses_identity_and_atomically_replaces_all_lazy_adapters() {
    let temp = Temp::new();
    let uri = path_to_uri(&temp.0);
    let mut id = identity(&["classes"]);
    id.workspace_root_uri = uri.clone();
    let host = Host::new(vec![(
        "classes",
        vec![("demo/Alpha", fixture("demo/Alpha"))],
    )]);
    let resources = FileBasedProjectContextService::default();
    let cached = resources
        .load_cached_session(&uri, Some(&id.facts))
        .unwrap();
    id.project_session_version = cached.version;
    let sessions = HostProjectSessions::default();
    let (s, hit) = sessions
        .load(&cached, id.clone(), host.clone(), Arc::new(|binary| binary))
        .unwrap();
    assert!(!hit);
    startup(&s);
    let (again, hit) = sessions
        .load(&cached, id.clone(), host.clone(), Arc::new(|binary| binary))
        .unwrap();
    assert!(hit);
    assert!(Arc::ptr_eq(&s, &again));
    assert!(s
        .class_index
        .get_methods("demo/Alpha")
        .iter()
        .any(|m| m.name == "run"));
    id.facts.provider_generation += 1;
    id.generations = fixture_generations(&id.facts);
    let reload = resources.reload(&uri, Some(&id.facts)).unwrap();
    id.project_session_version = reload.version;
    let (new, hit) = sessions
        .load(&reload, id.clone(), host.clone(), Arc::new(|binary| binary))
        .unwrap();
    assert!(!hit);
    assert!(!Arc::ptr_eq(&s, &new));
    assert!(s
        .class_bytes_provider
        .class_bytes_result("demo/Alpha")
        .is_err());
    startup(&new);
    assert!(new
        .class_index
        .get_methods("demo/Alpha")
        .iter()
        .any(|m| m.name == "run"));
    let mut mismatch = id.clone();
    mismatch.project_session_version += 1;
    assert!(sessions
        .load(&reload, mismatch, host.clone(), Arc::new(|binary| binary))
        .is_err());
    let mut mismatch = id.clone();
    mismatch.generations.config += 1;
    assert!(sessions
        .load(&reload, mismatch, host, Arc::new(|binary| binary))
        .is_err());
    sessions.clear(&uri).unwrap();
    assert!(new
        .class_bytes_provider
        .class_bytes_result("demo/Alpha")
        .is_err());
}
#[test]
fn session_store_keeps_identical_owner_names_isolated_between_workspaces() {
    let a = Temp::new();
    let b = Temp::new();
    let sessions = HostProjectSessions::default();
    let resources = FileBasedProjectContextService::default();
    let mut loaded = Vec::new();
    for (temp, bytes, method) in [
        (&a, fixture("demo/Alpha"), "run"),
        (&b, fixture("demo/Beta"), "go"),
    ] {
        let uri = path_to_uri(&temp.0);
        let mut id = identity(&["classes"]);
        id.workspace_root_uri = uri.clone();
        let cached = resources
            .load_cached_session(&uri, Some(&id.facts))
            .unwrap();
        id.project_session_version = cached.version;
        let host = Host::new(vec![("classes", vec![("demo/SameOwner", bytes)])]);
        let (s, _) = sessions
            .load(&cached, id, host, Arc::new(|binary| binary))
            .unwrap();
        assert!(s
            .class_index
            .get_methods("demo/SameOwner")
            .iter()
            .any(|m| m.name == method));
        loaded.push(s);
    }
    assert!(!Arc::ptr_eq(
        &loaded[0].class_bytes_provider,
        &loaded[1].class_bytes_provider
    ));
    sessions.clear_all();
    for s in loaded {
        assert!(s
            .class_bytes_provider
            .class_bytes_result("demo/SameOwner")
            .is_err());
    }
}
#[test]
fn reindex_recreates_the_source_overlay_using_the_new_binary_index() {
    let count = Arc::new(AtomicUsize::new(0));
    let factory = {
        let count = count.clone();
        Arc::new(move |_binary: Arc<dyn ClassIndex>| {
            count.fetch_add(1, Ordering::Relaxed);
            index("source/Marker", &[("sourceOnly", "()V")]) as Arc<dyn ClassIndex>
        }) as Arc<HostClassIndexOverlay>
    };
    let host = Host::new(vec![(
        "classes",
        vec![("demo/Alpha", fixture("demo/Alpha"))],
    )]);
    let s = HostProjectSession::with_class_index(
        Arc::new(ProjectContextBuilder::empty("source-overlay", "/workspace")),
        identity(&["classes"]),
        host,
        factory,
    );
    assert_eq!(
        s.class_index.get_methods("source/Marker")[0].name,
        "sourceOnly"
    );
    let new = s.reindex();
    assert_eq!(count.load(Ordering::Relaxed), 2);
    assert_eq!(
        new.class_index.get_methods("source/Marker")[0].name,
        "sourceOnly"
    );
    assert!(!Arc::ptr_eq(&s.binary_index, &new.binary_index));
    startup(&s);
    startup(&new);
    s.close();
    assert!(s.class_index.get_methods("source/Marker").is_empty());
    assert_eq!(
        new.class_index.get_methods("source/Marker")[0].name,
        "sourceOnly"
    );
}
#[test]
fn normalized_host_hierarchy_flags_assignability_and_locations_preserve_status() {
    use mcdev_core::{mixin_navigation::*, mixinextras_handler::HandlerReferenceTypeResolver};
    let host = Host::new(vec![]);
    let fact = HostReferenceTypeFact {
        internal_name: "host/Only".into(),
        super_internal_name: Some("sdk/Parent".into()),
        interface_internal_names: vec!["sdk/Contract".into()],
        access_flags: 0x11,
    };
    *host.reference_facts.lock().unwrap() =
        Some([("host/Only".into(), fact.clone())].into_iter().collect());
    host.assignability
        .lock()
        .unwrap()
        .insert(("host/Only".into(), "sdk/Parent".into()), true);
    host.assignability
        .lock()
        .unwrap()
        .insert(("host/Only".into(), "sdk/Other".into()), false);
    let s = HostProjectSession::create(
        Arc::new(ProjectContextBuilder::empty("facts", "/workspace")),
        identity(&[]),
        host.clone(),
    );
    let refs = s.reference_types();
    assert_eq!(
        refs.reference_type_result("host/Only").unwrap(),
        HostFact::Known(fact)
    );
    assert_eq!(refs.is_known_reference("host/Absent"), Some(false));
    assert_eq!(refs.is_assignable("host/Only", "sdk/Parent"), Some(true));
    assert_eq!(refs.is_assignable("host/Only", "sdk/Other"), Some(false));
    assert_eq!(refs.is_assignable("host/Only", "sdk/Unknown"), None);
    let target = McDefinitionTarget {
        kind: MemberKind::Class,
        owner_internal_name: "host/Only".into(),
        owner_fqn: None,
        name: None,
        descriptor: None,
        namespace: MappingNamespace::Named,
        source_range: None,
        direct_source_document_uri: None,
    };
    let location = ResolvedDefinition {
        target: target.clone(),
        document_uri: "file:///workspace/Only.kt".into(),
        range: McTextRange {
            start: McTextPosition {
                line: 2,
                character: 9,
            },
            end: McTextPosition {
                line: 2,
                character: 13,
            },
        },
        resolution: DefinitionResolution::Source,
        resolution_message: Some("host supplied snapshot".into()),
    };
    host.locations
        .lock()
        .unwrap()
        .insert("host/Only".into(), location.clone());
    assert_eq!(
        s.definition_result(&target).unwrap(),
        HostFact::Known(location.clone())
    );
    assert_eq!(
        DefinitionBackend::resolve(&s, &target, &s.context, &s.identity.workspace_root_uri),
        Some(location)
    );
    assert!(DefinitionBackend::resolve(&s, &target, &s.context, "file:///another").is_none());
    assert_eq!(s.class_bytes_provider.byte_read_count(), 0);
    assert!(!s.class_bytes_provider.catalog_initialized());
}
#[test]
fn selected_classpath_merge_preserves_order_duplicates_timestamps_and_generated_roots() {
    let root = PathBuf::from("/workspace");
    let base = ClasspathSnapshot {
        project_outputs: vec![root.join("classes")],
        generated_outputs: vec![root.join("generated")],
        entry_timestamps: [(root.join("classes"), 1)].into_iter().collect(),
        captured_at: 3,
        ..Default::default()
    };
    let extras = [
        HostClasspathPath {
            path: root.join("classes"),
            exists: true,
            modified_at: 99,
            kind: HostClasspathKind::ProjectOutput,
        },
        HostClasspathPath {
            path: root.join("dependency.jar"),
            exists: true,
            modified_at: 4,
            kind: HostClasspathKind::DependencyJar,
        },
        HostClasspathPath {
            path: root.join("absent.jar"),
            exists: false,
            modified_at: 4,
            kind: HostClasspathKind::DependencyJar,
        },
        HostClasspathPath {
            path: root.join("classes"),
            exists: true,
            modified_at: 5,
            kind: HostClasspathKind::MinecraftJar,
        },
    ];
    let merged = HostProjectBridge::merge_classpath(&base, &extras, 8);
    assert_eq!(merged.project_outputs, base.project_outputs);
    assert_eq!(merged.generated_outputs, base.generated_outputs);
    assert_eq!(merged.dependency_jars, [root.join("dependency.jar")]);
    assert_eq!(merged.minecraft_jars, [root.join("classes")]);
    assert_eq!(merged.entry_timestamps[&root.join("classes")], 1);
    assert_eq!(merged.captured_at, 8);
    let mk = |path: &str, kind| HostClasspathPath {
        path: root.join(path),
        exists: true,
        modified_at: 0,
        kind,
    };
    let selected = [
        mk(
            "build/loom-cache/remapped_working/z.JAR",
            HostClasspathKind::DependencyJar,
        ),
        mk(
            ".gradle/loom-cache/remapped_working/a.jar",
            HostClasspathKind::MinecraftJar,
        ),
        mk(
            ".gradle/loom-cache/remapped_working/directory.jar",
            HostClasspathKind::ProjectOutput,
        ),
        mk("outside.jar", HostClasspathKind::DependencyJar),
    ];
    assert_eq!(
        HostProjectBridge::selected_loom_jars(&root, &selected),
        [
            root.join(".gradle/loom-cache/remapped_working/a.jar"),
            root.join("build/loom-cache/remapped_working/z.JAR")
        ]
    );
}
#[test]
fn exact_original_compiler_fixtures_have_preserved_hashes() {
    use sha2::{Digest, Sha256};
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/host_project/class_manifest.json")).unwrap();
    assert_eq!(rows.len(), 18);
    for row in rows {
        let key = row["path"]
            .as_str()
            .unwrap()
            .strip_prefix("classes/")
            .unwrap()
            .strip_suffix(".class.bin")
            .unwrap();
        let bytes = fixture(key);
        assert_eq!(bytes.len() as u64, row["bytes"].as_u64().unwrap());
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            row["sha256"].as_str().unwrap(),
            "{key}"
        );
    }
}
#[test]
fn typed_member_queries_distinguish_missing_corrupt_and_unavailable() {
    let (host, p) = provider(vec![(
        "classes",
        vec![
            ("demo/Corrupt", MINIMAL.to_vec()),
            ("demo/Alpha", fixture("demo/Alpha")),
        ],
    )]);
    let index = LazyHostClassIndex::new(p);
    assert_eq!(
        index.methods_result("demo/Absent").unwrap(),
        HostFact::Missing
    );
    assert!(matches!(
        index.methods_result("demo/Corrupt"),
        Err(HostSourceError::Failure(_))
    ));
    assert_eq!(index.cached_owner_count(), 0);
    host.unavailable.store(true, Ordering::Relaxed);
    assert!(index.fields_result("demo/Alpha").is_err());
    assert_eq!(index.cached_owner_count(), 0);
    host.unavailable.store(false, Ordering::Relaxed);
    assert!(
        matches!(index.fields_result("demo/Alpha").unwrap(),HostFact::Known(fields) if fields.iter().any(|f|f.name=="value"))
    );
}
#[test]
fn zero_capacity_retains_legacy_disabled_cache_semantics() {
    let host = Host::new(vec![(
        "classes",
        vec![("demo/Alpha", fixture("demo/Alpha"))],
    )]);
    let p = Arc::new(HostClassBytesProvider::with_capacity(
        identity(&["classes"]),
        vec![HostClasspathEntry {
            id: "classes".into(),
            path: "classes".into(),
            timestamp: None,
        }],
        host,
        0,
        Arc::new(AtomicUsize::new(0)),
    ));
    let first = p.class_bytes_result("demo/Alpha").unwrap().unwrap();
    let second = p.class_bytes_result("demo/Alpha").unwrap().unwrap();
    assert!(!Arc::ptr_eq(&first, &second));
    assert_eq!(p.cached_owner_count(), 0);
    assert_eq!(p.byte_read_count(), 2);
    let index = LazyHostClassIndex::with_capacity(p.clone(), 0);
    assert!(index
        .get_methods("demo/Alpha")
        .iter()
        .any(|m| m.name == "run"));
    assert!(index
        .get_methods("demo/Alpha")
        .iter()
        .any(|m| m.name == "run"));
    assert_eq!(index.asm_parse_count(), 2);
    assert_eq!(index.cached_owner_count(), 0);
    assert_eq!(p.byte_read_count(), 4);
}
