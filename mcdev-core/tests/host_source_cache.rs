use mcdev_core::host_source::*;
use mcdev_core::mixin_source::{MixinSemanticModelParser, ParseSource};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc, Arc, Condvar, Mutex,
};
use std::time::Duration;
fn project(id: &str, version: u64) -> HostProjectKey {
    HostProjectKey {
        project_id: id.into(),
        generations: HostGenerations {
            project: version,
            ..Default::default()
        },
    }
}
#[derive(Default)]
struct Validator {
    valid: AtomicBool,
    registrations: AtomicUsize,
    reads: Mutex<Vec<(HostGenerations, HostReadSet)>>,
}
impl Validator {
    fn good() -> Self {
        Self {
            valid: AtomicBool::new(true),
            ..Default::default()
        }
    }
}
impl HostReadValidator for Validator {
    fn validate_current_reads(
        &self,
        generations: HostGenerations,
        reads: &HostReadSet,
    ) -> Result<bool, HostSourceError> {
        self.registrations.fetch_add(1, Ordering::SeqCst);
        self.reads
            .lock()
            .unwrap()
            .push((generations, reads.clone()));
        Ok(self.valid.load(Ordering::SeqCst))
    }
}
fn stamp(missing: bool) -> HostReadSet {
    vec![HostReadStamp {
        kind: "type".into(),
        key: "example/Target".into(),
        generation: 1,
        missing,
    }]
}
#[test]
fn cancelled_constant_resolution_propagates_and_can_recover() {
    let cache = HostFactCache::<u8, String>::new(2, None);
    let validator = Validator::good();
    let mut calls = 0;
    let mut resolve = || {
        calls += 1;
        if calls == 1 {
            Err(HostSourceError::Cancelled(
                "constant resolution cancelled".into(),
            ))
        } else {
            Ok(HostAnswer::known("resolved".into(), stamp(false)))
        }
    };
    let error = cache
        .get_or_compute(0, HostGenerations::default(), &validator, &mut resolve)
        .unwrap_err();
    assert_eq!(
        error,
        HostSourceError::Cancelled("constant resolution cancelled".into())
    );
    assert_eq!(
        cache
            .get_or_compute(0, HostGenerations::default(), &validator, &mut resolve)
            .unwrap()
            .answer
            .fact,
        HostFact::Known("resolved".into())
    );
    assert_eq!(calls, 2);
}
#[test]
fn positive_and_negative_reuse_registers_current_reads_and_all_generation_dimensions_invalidate() {
    let cache = HostFactCache::<u8, bool>::new(8, None);
    let validator = Validator::good();
    let mut calls = 0;
    let build = |calls: &mut usize| {
        *calls += 1;
        Ok(HostAnswer::<bool>::missing(stamp(true)))
    };
    assert!(
        !cache
            .get_or_compute(0, HostGenerations::default(), &validator, || build(
                &mut calls
            ))
            .unwrap()
            .cache_hit
    );
    assert!(
        cache
            .get_or_compute(0, HostGenerations::default(), &validator, || panic!(
                "negative hit must not rebuild"
            ))
            .unwrap()
            .cache_hit
    );
    assert_eq!(validator.registrations.load(Ordering::SeqCst), 1);
    assert_eq!(
        *validator.reads.lock().unwrap(),
        [(HostGenerations::default(), stamp(true))]
    );
    let generations = [
        HostGenerations {
            project: 1,
            ..Default::default()
        },
        HostGenerations {
            model: 1,
            ..Default::default()
        },
        HostGenerations {
            namespace: 1,
            ..Default::default()
        },
        HostGenerations {
            classpath: 1,
            ..Default::default()
        },
        HostGenerations {
            dependency: 1,
            ..Default::default()
        },
        HostGenerations {
            generated_source: 1,
            ..Default::default()
        },
        HostGenerations {
            config: 1,
            ..Default::default()
        },
        HostGenerations {
            provider: 1,
            ..Default::default()
        },
    ];
    for generation in generations {
        assert!(
            !cache
                .get_or_compute(0, generation, &validator, || build(&mut calls))
                .unwrap()
                .cache_hit
        );
    }
    assert_eq!(calls, 9);
    validator.valid.store(false, Ordering::SeqCst);
    assert!(
        !cache
            .get_or_compute(0, generations[7], &validator, || build(&mut calls))
            .unwrap()
            .cache_hit
    );
    assert_eq!(calls, 10);
}
struct SemanticProvider {
    calls: AtomicUsize,
    first: Option<HostSourceError>,
    transient: bool,
}
impl HostReadValidator for SemanticProvider {
    fn validate_current_reads(
        &self,
        _: HostGenerations,
        _: &HostReadSet,
    ) -> Result<bool, HostSourceError> {
        Ok(true)
    }
}
impl HostSemanticModelProvider for SemanticProvider {
    fn parse(&self, r: &HostSemanticRequest<'_>) -> HostSourceResult<HostSemanticModel> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        if n == 0 {
            if let Some(error) = &self.first {
                return Err(error.clone());
            }
        }
        let transient = self.transient && n == 0;
        let mut model = MixinSemanticModelParser::parse(r.source, r.source_uri);
        model.parse_source = if transient {
            ParseSource::HandWrittenFallback
        } else {
            ParseSource::HostNormalized
        };
        Ok(HostAnswer::known(
            HostSemanticModel {
                model,
                availability: if transient {
                    SemanticAvailability::TransientFallback
                } else {
                    SemanticAvailability::Normalized
                },
                parse_ms: 7,
            },
            stamp(false),
        ))
    }
}
#[test]
fn transient_fallback_is_recomputed_so_recovery_can_use_host_normalized_model() {
    let provider = SemanticProvider {
        calls: AtomicUsize::new(0),
        first: None,
        transient: true,
    };
    let cache = SemanticModelCache::default();
    let environment = HostSemanticEnvironment::default();
    let project = project("p", 1);
    let request = HostSemanticRequest {
        source: "class ExampleMixin {}",
        source_uri: "file:///ExampleMixin.java",
        document_version: Some(1),
        environment: &environment,
        project: Some(&project),
    };
    let first = cache.get(&request, &provider).unwrap();
    let second = cache.get(&request, &provider).unwrap();
    let third = cache.get(&request, &provider).unwrap();
    assert_eq!(
        first.answer.fact.known().unwrap().model.parse_source,
        ParseSource::HandWrittenFallback
    );
    assert!(!first.cache_hit);
    assert_eq!(
        second.answer.fact.known().unwrap().model.parse_source,
        ParseSource::HostNormalized
    );
    assert!(!second.cache_hit);
    assert!(third.cache_hit);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
}
#[test]
fn canceled_semantic_computation_leaves_no_entry_for_next_request() {
    let error = HostSourceError::Cancelled("cancelled".into());
    let provider = SemanticProvider {
        calls: AtomicUsize::new(0),
        first: Some(error.clone()),
        transient: false,
    };
    let cache = SemanticModelCache::default();
    let environment = HostSemanticEnvironment::default();
    let project = project("p", 1);
    let request = HostSemanticRequest {
        source: "class ExampleMixin {}",
        source_uri: "file:///ExampleMixin.java",
        document_version: Some(2),
        environment: &environment,
        project: Some(&project),
    };
    assert_eq!(cache.get(&request, &provider).unwrap_err(), error);
    let next = cache.get(&request, &provider).unwrap();
    let cached = cache.get(&request, &provider).unwrap();
    assert_eq!(
        next.answer.fact.known().unwrap().model.parse_source,
        ParseSource::HostNormalized
    );
    assert!(!next.cache_hit);
    assert!(cached.cache_hit);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
}
#[test]
fn semantic_hash_collision_missing_generation_and_environment_changes_never_reuse_models() {
    let provider = SemanticProvider {
        calls: AtomicUsize::new(0),
        first: None,
        transient: false,
    };
    let cache = SemanticModelCache::default();
    let environment = HostSemanticEnvironment::default();
    let key = project("p", 1);
    for source in ["Aa", "BB"] {
        let request = HostSemanticRequest {
            source,
            source_uri: "file:///Example.java",
            document_version: Some(1),
            environment: &environment,
            project: Some(&key),
        };
        assert!(!cache.get(&request, &provider).unwrap().cache_hit);
    }
    for _ in 0..2 {
        let request = HostSemanticRequest {
            source: "Aa",
            source_uri: "file:///Example.java",
            document_version: Some(1),
            environment: &environment,
            project: None,
        };
        assert!(!cache.get(&request, &provider).unwrap().cache_hit);
    }
    assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
    cache.invalidate("file:///Example.java");
    assert!(cache.is_empty());
}

type FeatureFn = Box<dyn Fn(&HostProjectKey) -> HostSourceResult<bool> + Send + Sync>;
type SubtypesFn =
    Box<dyn Fn(&HostProjectKey) -> HostSourceResult<Vec<HostInjectionPointSubtype>> + Send + Sync>;
struct FeatureProvider {
    feature: FeatureFn,
    subtypes: SubtypesFn,
    validations: AtomicUsize,
}
impl HostReadValidator for FeatureProvider {
    fn validate_current_reads(
        &self,
        _: HostGenerations,
        _: &HostReadSet,
    ) -> Result<bool, HostSourceError> {
        self.validations.fetch_add(1, Ordering::SeqCst);
        Ok(true)
    }
}
impl HostInjectionPointProvider for FeatureProvider {
    fn specifier_exists(&self, p: &HostProjectKey) -> HostSourceResult<bool> {
        (self.feature)(p)
    }
    fn injection_point_subtypes(
        &self,
        p: &HostProjectKey,
    ) -> HostSourceResult<Vec<HostInjectionPointSubtype>> {
        (self.subtypes)(p)
    }
}
fn provider(
    feature: impl Fn(&HostProjectKey) -> HostSourceResult<bool> + Send + Sync + 'static,
) -> FeatureProvider {
    FeatureProvider {
        feature: Box::new(feature),
        subtypes: Box::new(|_| Ok(HostAnswer::known(vec![], vec![]))),
        validations: AtomicUsize::new(0),
    }
}
#[test]
fn null_project_returns_unavailable_without_lookup() {
    let host = provider(|_| panic!("must not look up"));
    assert!(matches!(
        InjectionPointFeatureProbe::default()
            .supports_specifier(None, &host)
            .unwrap()
            .answer
            .fact,
        HostFact::Unavailable { .. }
    ));
}
#[test]
fn present_type_returns_true() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let host = provider(move |_| {
        c.fetch_add(1, Ordering::SeqCst);
        Ok(HostAnswer::known(true, vec![]))
    });
    assert_eq!(
        InjectionPointFeatureProbe::default()
            .supports_specifier(Some(&project("p", 0)), &host)
            .unwrap()
            .answer
            .fact,
        HostFact::Known(true)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
#[test]
fn missing_type_returns_missing() {
    let host = provider(|_| Ok(HostAnswer::missing(stamp(true))));
    assert_eq!(
        InjectionPointFeatureProbe::default()
            .supports_specifier(Some(&project("p", 0)), &host)
            .unwrap()
            .answer
            .fact,
        HostFact::Missing
    );
}
#[test]
fn binary_type_counts_as_present() {
    let host = provider(|_| Ok(HostAnswer::known(true, vec![])));
    assert_eq!(
        InjectionPointFeatureProbe::default()
            .supports_specifier(Some(&project("binary", 0)), &host)
            .unwrap()
            .answer
            .fact,
        HostFact::Known(true)
    );
}
#[test]
fn lookup_exception_preserves_failure_instead_of_claiming_absence() {
    let host = provider(|_| Err(HostSourceError::Failure("findType failed".into())));
    assert_eq!(
        InjectionPointFeatureProbe::default()
            .supports_specifier(Some(&project("p", 0)), &host)
            .unwrap_err(),
        HostSourceError::Failure("findType failed".into())
    );
}
#[test]
fn caches_exactly_once_for_same_project_identity_and_version() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let host = provider(move |_| {
        c.fetch_add(1, Ordering::SeqCst);
        Ok(HostAnswer::known(true, stamp(false)))
    });
    let probe = InjectionPointFeatureProbe::default();
    let key = project("p", 7);
    for _ in 0..2 {
        assert_eq!(
            probe
                .supports_specifier(Some(&key), &host)
                .unwrap()
                .answer
                .fact,
            HostFact::Known(true)
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(host.validations.load(Ordering::SeqCst), 1);
}
#[test]
fn recomputes_when_project_session_version_changes() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let host = provider(move |_| {
        c.fetch_add(1, Ordering::SeqCst);
        Ok(HostAnswer::known(true, stamp(false)))
    });
    let probe = InjectionPointFeatureProbe::default();
    for version in [1, 2] {
        assert_eq!(
            probe
                .supports_specifier(Some(&project("p", version)), &host)
                .unwrap()
                .answer
                .fact,
            HostFact::Known(true)
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}
#[test]
fn equal_names_with_distinct_host_project_identity_do_not_share_cache() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let host = provider(move |_| {
        c.fetch_add(1, Ordering::SeqCst);
        Ok(HostAnswer::known(true, vec![]))
    });
    let probe = InjectionPointFeatureProbe::default();
    for id in ["project-a#1", "project-a#2"] {
        assert_eq!(
            probe
                .supports_specifier(Some(&project(id, 1)), &host)
                .unwrap()
                .answer
                .fact,
            HostFact::Known(true)
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}
#[test]
fn lookup_exception_is_not_cached_and_next_request_retries() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let host = provider(move |_| {
        if c.fetch_add(1, Ordering::SeqCst) == 0 {
            Err(HostSourceError::Failure(
                "transient findType failure".into(),
            ))
        } else {
            Ok(HostAnswer::known(true, vec![]))
        }
    });
    let probe = InjectionPointFeatureProbe::default();
    let key = project("p", 0);
    assert!(probe.supports_specifier(Some(&key), &host).is_err());
    assert_eq!(
        probe
            .supports_specifier(Some(&key), &host)
            .unwrap()
            .answer
            .fact,
        HostFact::Known(true)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}
#[test]
fn lookup_uses_injection_point_specifier_fqn() {
    assert_eq!(
        INJECTION_POINT_SPECIFIER_FQN,
        "org.spongepowered.asm.mixin.injection.InjectionPoint$Specifier"
    );
}

fn concurrent_features(distinct: bool) {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let g = gate.clone();
    let (entered_tx, entered) = mpsc::channel();
    let host = Arc::new(provider(move |_| {
        c.fetch_add(1, Ordering::SeqCst);
        entered_tx.send(()).unwrap();
        let (lock, wake) = &*g;
        let done = lock.lock().unwrap();
        let (done, timeout) = wake
            .wait_timeout_while(done, Duration::from_secs(5), |done| !*done)
            .unwrap();
        assert!(*done && !timeout.timed_out(), "lookups not released");
        Ok(HostAnswer::known(true, stamp(false)))
    }));
    let probe = Arc::new(InjectionPointFeatureProbe::default());
    let n = if distinct { 2 } else { 32 };
    let (ready_tx, ready) = mpsc::channel();
    let (result_tx, results) = mpsc::channel();
    let mut threads = vec![];
    for i in 0..n {
        let p = probe.clone();
        let h = host.clone();
        let r = ready_tx.clone();
        let tx = result_tx.clone();
        threads.push(std::thread::spawn(move || {
            r.send(()).unwrap();
            let result = p
                .supports_specifier(
                    Some(&project("p", if distinct { i as u64 + 1 } else { 42 })),
                    h.as_ref(),
                )
                .unwrap();
            tx.send(result.answer.fact == HostFact::Known(true))
                .unwrap();
        }));
    }
    for _ in 0..n {
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
    }
    for _ in 0..if distinct { 2 } else { 1 } {
        entered.recv_timeout(Duration::from_secs(5)).unwrap();
    }
    assert_eq!(calls.load(Ordering::SeqCst), if distinct { 2 } else { 1 });
    let (lock, wake) = &*gate;
    *lock.lock().unwrap() = true;
    wake.notify_all();
    for _ in 0..n {
        assert!(results.recv_timeout(Duration::from_secs(5)).unwrap());
    }
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(calls.load(Ordering::SeqCst), if distinct { 2 } else { 1 });
}
#[test]
fn concurrent_identical_probes_perform_single_lookup() {
    concurrent_features(false);
}
#[test]
fn distinct_project_session_versions_perform_concurrent_lookups() {
    concurrent_features(true);
}

fn string_node(value: &str) -> HostExpressionFact {
    HostExpressionFact {
        validity: Default::default(),
        kind: HostExpressionKind::Constant(HostConstant::String(value.encode_utf16().collect())),
        source_text: format!("\"{value}\""),
        source_range: None,
    }
}
fn at_annotation(name: &str, namespace: Option<&str>, value: &str) -> HostAnnotationFact {
    let mut members = vec![("value".into(), string_node(value))];
    if let Some(namespace) = namespace {
        members.push(("namespace".into(), string_node(namespace)));
    }
    HostAnnotationFact {
        source_name: name.into(),
        qualified_name: Some(name.into()),
        validity: Default::default(),
        exists: true,
        members,
        members_available: true,
        source_range: None,
        body_range: None,
    }
}
fn subtype(fqn: &str, namespace: Option<&str>, value: Option<&str>) -> HostInjectionPointSubtype {
    HostInjectionPointSubtype {
        qualified_name: fqn.into(),
        annotations: value
            .map(|v| vec![at_annotation("AtCode", namespace, v)])
            .unwrap_or_default(),
    }
}
fn at_provider(
    build: impl Fn(&HostProjectKey) -> HostSourceResult<Vec<HostInjectionPointSubtype>>
        + Send
        + Sync
        + 'static,
) -> FeatureProvider {
    FeatureProvider {
        feature: Box::new(|_| panic!("specifier must not be probed")),
        subtypes: Box::new(build),
        validations: AtomicUsize::new(0),
    }
}
#[test]
fn annotated_default_namespace_returns_bare_value() {
    assert_eq!(
        injection_point_at_codes(&[subtype(
            "com.example.AnnotatedDefault",
            None,
            Some("MYPOINT")
        )]),
        ["MYPOINT"]
    );
}
#[test]
fn annotated_custom_namespace_preserves_declared_text() {
    assert_eq!(
        injection_point_at_codes(&[subtype(
            "com.example.AnnotatedCustom",
            Some("MyMod"),
            Some("Hook")
        )]),
        ["MyMod:Hook"]
    );
}
#[test]
fn unannotated_custom_fqn_is_included_as_runtime_fallback() {
    assert_eq!(
        injection_point_at_codes(&[subtype("com.example.UnannotatedCustomPoint", None, None)]),
        ["com.example.UnannotatedCustomPoint"]
    );
}
#[test]
fn unannotated_sponge_built_in_subclass_is_excluded() {
    assert!(injection_point_at_codes(&[subtype(
        "org.spongepowered.asm.mixin.injection.ModifyConstantInjectionPoint",
        None,
        None
    )])
    .is_empty());
}
#[test]
fn missing_injection_point_is_known_empty() {
    let host = at_provider(|_| Ok(HostAnswer::missing(stamp(true))));
    assert_eq!(
        InjectionPointAtCodeIndex::default()
            .values(Some(&project("missing", 0)), &host)
            .unwrap()
            .answer
            .fact,
        HostFact::Known(vec![])
    );
}
#[test]
fn null_at_project_preserves_unavailable() {
    let host = at_provider(|_| panic!("must not query"));
    assert!(matches!(
        InjectionPointAtCodeIndex::default()
            .values(None, &host)
            .unwrap()
            .answer
            .fact,
        HostFact::Unavailable { .. }
    ));
}
#[test]
fn transient_hierarchy_failure_does_not_cache_empty_and_retries_on_next_request() {
    let builds = Arc::new(AtomicUsize::new(0));
    let b = builds.clone();
    let host = at_provider(move |_| {
        if b.fetch_add(1, Ordering::SeqCst) == 0 {
            Err(HostSourceError::Failure(
                "transient hierarchy failure".into(),
            ))
        } else {
            Ok(HostAnswer::known(
                vec![subtype(
                    "com.example.AnnotatedDefault",
                    None,
                    Some("MYPOINT"),
                )],
                stamp(false),
            ))
        }
    });
    let index = InjectionPointAtCodeIndex::default();
    let key = project("transient-failure", 99);
    assert!(index.values(Some(&key), &host).is_err());
    assert_eq!(
        index.values(Some(&key), &host).unwrap().answer.fact,
        HostFact::Known(vec!["MYPOINT".into()])
    );
    assert_eq!(builds.load(Ordering::SeqCst), 2);
    assert_eq!(
        index.values(Some(&key), &host).unwrap().answer.fact,
        HostFact::Known(vec!["MYPOINT".into()])
    );
    assert_eq!(builds.load(Ordering::SeqCst), 2);
}
#[test]
fn at_codes_cache_exactly_once_for_same_project_identity_and_version() {
    let builds = Arc::new(AtomicUsize::new(0));
    let b = builds.clone();
    let host = at_provider(move |_| {
        b.fetch_add(1, Ordering::SeqCst);
        Ok(HostAnswer::known(
            vec![subtype(
                "com.example.AnnotatedDefault",
                None,
                Some("MYPOINT"),
            )],
            stamp(false),
        ))
    });
    let index = InjectionPointAtCodeIndex::default();
    let key = project("cache-hit", 7);
    for _ in 0..2 {
        assert_eq!(
            index.values(Some(&key), &host).unwrap().answer.fact,
            HostFact::Known(vec!["MYPOINT".into()])
        );
    }
    assert_eq!(builds.load(Ordering::SeqCst), 1);
}
#[test]
fn at_codes_recompute_when_project_session_version_changes() {
    let builds = Arc::new(AtomicUsize::new(0));
    let b = builds.clone();
    let host = at_provider(move |_| {
        b.fetch_add(1, Ordering::SeqCst);
        Ok(HostAnswer::known(
            vec![subtype(
                "com.example.AnnotatedDefault",
                None,
                Some("MYPOINT"),
            )],
            stamp(false),
        ))
    });
    let index = InjectionPointAtCodeIndex::default();
    for version in [1, 2] {
        assert_eq!(
            index
                .values(Some(&project("version-change", version)), &host)
                .unwrap()
                .answer
                .fact,
            HostFact::Known(vec!["MYPOINT".into()])
        );
    }
    assert_eq!(builds.load(Ordering::SeqCst), 2);
}
fn concurrent_at_codes(distinct: bool) {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let g = gate.clone();
    let (entered_tx, entered) = mpsc::channel();
    let host = Arc::new(at_provider(move |key| {
        c.fetch_add(1, Ordering::SeqCst);
        entered_tx.send(()).unwrap();
        let (lock, wake) = &*g;
        let done = lock.lock().unwrap();
        let (done, timeout) = wake
            .wait_timeout_while(done, Duration::from_secs(5), |done| !*done)
            .unwrap();
        assert!(*done && !timeout.timed_out());
        let value = if distinct {
            key.project_id.as_str()
        } else {
            "MYPOINT"
        };
        Ok(HostAnswer::known(
            vec![subtype("com.example.AnnotatedDefault", None, Some(value))],
            stamp(false),
        ))
    }));
    let index = Arc::new(InjectionPointAtCodeIndex::default());
    let n = if distinct { 2 } else { 32 };
    let (ready_tx, ready) = mpsc::channel();
    let (result_tx, results) = mpsc::channel();
    let mut threads = vec![];
    for i in 0..n {
        let ix = index.clone();
        let h = host.clone();
        let ready = ready_tx.clone();
        let result = result_tx.clone();
        threads.push(std::thread::spawn(move || {
            let id = if distinct {
                if i == 0 {
                    "project-a"
                } else {
                    "project-b"
                }
            } else {
                "same-key"
            };
            ready.send(()).unwrap();
            let answer = ix
                .values(
                    Some(&project(id, if distinct { 1 } else { 42 })),
                    h.as_ref(),
                )
                .unwrap();
            result
                .send(
                    answer.answer.fact
                        == HostFact::Known(vec![if distinct {
                            id.into()
                        } else {
                            "MYPOINT".into()
                        }]),
                )
                .unwrap();
        }));
    }
    for _ in 0..n {
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
    }
    for _ in 0..if distinct { 2 } else { 1 } {
        entered.recv_timeout(Duration::from_secs(5)).unwrap();
    }
    assert_eq!(calls.load(Ordering::SeqCst), if distinct { 2 } else { 1 });
    let (lock, wake) = &*gate;
    *lock.lock().unwrap() = true;
    wake.notify_all();
    for _ in 0..n {
        assert!(results.recv_timeout(Duration::from_secs(5)).unwrap());
    }
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(calls.load(Ordering::SeqCst), if distinct { 2 } else { 1 });
}
#[test]
fn concurrent_identical_requests_perform_single_hierarchy_build() {
    concurrent_at_codes(false);
}
#[test]
fn distinct_projects_do_not_globally_serialize_hierarchy_build() {
    concurrent_at_codes(true);
}
#[test]
fn format_at_code_value_preserves_declared_namespace_and_value() {
    for (namespace, value, expected) in [
        (Some("mixin"), "HEAD", "mixin:HEAD"),
        (Some("MyMod"), "Hook", "MyMod:Hook"),
        (None, "VALUE", "VALUE"),
        (Some(""), "VALUE", "VALUE"),
        (Some("   "), "VALUE", "VALUE"),
    ] {
        assert_eq!(format_at_code_value(namespace, value), expected);
    }
}
#[test]
fn normalized_hierarchy_retains_selected_injection_point_descendants() {
    let host = at_provider(|p| {
        assert_eq!(p.project_id, "root-InjectionPoint");
        Ok(HostAnswer::known(
            vec![subtype("com.example.AnnotatedPoint", None, Some("MYPOINT"))],
            stamp(false),
        ))
    });
    assert_eq!(
        InjectionPointAtCodeIndex::default()
            .values(Some(&project("root-InjectionPoint", 0)), &host)
            .unwrap()
            .answer
            .fact,
        HostFact::Known(vec!["MYPOINT".into()])
    );
}
#[test]
fn normalized_hierarchy_preserves_dollar_names_and_explicit_dot_fallback() {
    for name in ["com.example.Outer$Point", "com.example.FallbackOnly"] {
        let selected = subtype(name, None, None);
        assert_eq!(selected.qualified_name, name);
        assert_eq!(injection_point_at_codes(&[selected]), [name]);
    }
}
#[test]
fn at_code_reader_matches_element_name_variants_via_member_value_pairs() {
    for name in [
        "org.spongepowered.asm.mixin.injection.InjectionPoint$AtCode",
        "org.spongepowered.asm.mixin.injection.InjectionPoint.AtCode",
        "AtCode",
    ] {
        let hash = name
            .encode_utf16()
            .fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32));
        let value = format!("POINT_{hash}");
        assert_eq!(
            declared_at_code(&at_annotation(name, None, &value)),
            Some(value)
        );
    }
}
#[test]
fn at_code_reader_ignores_nonexistent_annotations() {
    let mut annotation = at_annotation(
        "org.spongepowered.asm.mixin.injection.InjectionPoint$AtCode",
        None,
        "SHOULD_NOT_READ",
    );
    annotation.exists = false;
    assert_eq!(declared_at_code(&annotation), None);
}
#[test]
fn at_code_reader_formats_namespace_from_member_value_pairs() {
    for (namespace, value, expected) in [
        (None, "HEAD", "HEAD"),
        (Some("mixin"), "HEAD", "mixin:HEAD"),
        (Some("MyMod"), "Hook", "MyMod:Hook"),
    ] {
        assert_eq!(
            declared_at_code(&at_annotation(
                "org.spongepowered.asm.mixin.injection.InjectionPoint$AtCode",
                namespace,
                value
            )),
            Some(expected.into())
        );
    }
}
#[test]
fn end_to_end_index_uses_normalized_host_hierarchy_and_annotation_facts() {
    let host = at_provider(|_| {
        Ok(HostAnswer::known(
            vec![
                subtype("com.example.EndToEnd", Some("mixin"), Some("TAIL")),
                subtype("com.example.CustomFallback", None, None),
                subtype(
                    "org.spongepowered.asm.mixin.injection.ModifyConstantInjectionPoint",
                    None,
                    None,
                ),
            ],
            stamp(false),
        ))
    });
    assert_eq!(
        InjectionPointAtCodeIndex::default()
            .values(Some(&project("p", 0)), &host)
            .unwrap()
            .answer
            .fact,
        HostFact::Known(vec![
            "com.example.CustomFallback".into(),
            "mixin:TAIL".into()
        ])
    );
}
