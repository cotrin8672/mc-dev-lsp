use mcdev_core::{
    bytecode::*, host_source::*, mixin_completion::*, mixin_facade::*, mixin_source::*, *,
};
use std::sync::atomic::{AtomicU64, AtomicU8, AtomicUsize, Ordering};
struct Facts {
    parses: AtomicUsize,
    validations: AtomicUsize,
    revision: AtomicU64,
    mode: AtomicU8,
}
impl Facts {
    fn new() -> Self {
        Self {
            parses: 0.into(),
            validations: 0.into(),
            revision: 1.into(),
            mode: 0.into(),
        }
    }
}
impl HostReadValidator for Facts {
    fn validate_current_reads(
        &self,
        _generation: HostGenerations,
        reads: &HostReadSet,
    ) -> Result<bool, HostSourceError> {
        self.validations.fetch_add(1, Ordering::SeqCst);
        assert_eq!(reads.len(), 1);
        assert!(reads[0].missing);
        Ok(reads[0].generation == self.revision.load(Ordering::SeqCst))
    }
}
impl HostSemanticModelProvider for Facts {
    fn parse(&self, request: &HostSemanticRequest<'_>) -> HostSourceResult<HostSemanticModel> {
        self.parses.fetch_add(1, Ordering::SeqCst);
        let mode = self.mode.load(Ordering::SeqCst);
        if mode == 3 {
            return Ok(HostAnswer::unavailable(
                "normalized host semantic facts unavailable",
            ));
        }
        if mode == 4 {
            return Err(HostSourceError::Cancelled("host cancelled".into()));
        }
        let mut model = MixinSemanticModelParser::parse(request.source, request.source_uri);
        for target in &mut model.targets {
            if target.internal_name == "Target" {
                target.internal_name = "game/Target".into();
            }
        }
        model.parse_source = ParseSource::HostNormalized;
        model.confidence = ParseConfidence::High;
        model.debug_info.parse_source = ParseSource::HostNormalized;
        model.debug_info.used_java_project = true;
        model.warnings = vec![format!("revision {}", self.revision.load(Ordering::SeqCst))];
        Ok(HostAnswer::known(
            HostSemanticModel {
                model,
                availability: if mode == 1 {
                    SemanticAvailability::TransientFallback
                } else {
                    SemanticAvailability::Normalized
                },
                parse_ms: 0,
            },
            vec![HostReadStamp {
                kind: "sdkType".into(),
                key: "missing.Type".into(),
                generation: self.revision.load(Ordering::SeqCst),
                missing: true,
            }],
        ))
    }
}
impl ClassIndex for Facts {
    fn find_classes(&self, prefix: &str, _limit: usize) -> Vec<ClassIndexEntry> {
        self.find_class("Target")
            .into_iter()
            .filter(|c| c.simple_name.starts_with(prefix))
            .collect()
    }
    fn find_class(&self, name: &str) -> Option<ClassIndexEntry> {
        matches!(name, "Target" | "game/Target").then(|| ClassIndexEntry {
            simple_name: "Target".into(),
            package_name: "game".into(),
            internal_name: "game/Target".into(),
        })
    }
    fn find_class_by_fqn(&self, name: &str) -> Option<ClassIndexEntry> {
        self.find_class(&name.replace('.', "/"))
    }
    fn get_methods(&self, owner: &str) -> Vec<MethodIndexEntry> {
        if owner != "game/Target" {
            return vec![];
        }
        vec![MethodIndexEntry {
            name: "tick".into(),
            descriptor: "()V".into(),
            is_static: false,
            readable_signature: "tick(): void".into(),
        }]
    }
    fn get_fields(&self, _owner: &str) -> Vec<FieldIndexEntry> {
        vec![]
    }
}
impl BytecodeIndex for Facts {
    fn get_at_target_candidates(
        &self,
        _o: &str,
        _m: &str,
        _d: Option<&str>,
        _v: &str,
    ) -> Vec<AtTargetCandidate> {
        panic!("completion must not read bytecode")
    }
    fn get_return_ordinal_count(&self, _o: &str, _m: &str, _d: Option<&str>) -> usize {
        panic!("completion must not read bytecode")
    }
    fn get_class_bytes(&self, _owner: &str) -> Option<Vec<u8>> {
        panic!("completion must not read bytecode")
    }
}
fn request() -> MixinFacadeRequest {
    let source="@Mixin(Target.class) class Patch { @Inject(method = \"ti\", at = @At(\"HEAD\")) void handler() {} }";
    let cursor = source.find("\"ti\"").unwrap() + 3;
    MixinFacadeRequest {
        buffer_text: source.into(),
        character: cursor as i32,
        ..Default::default()
    }
}

fn selector_request(parse_source: ParseSource, registered: bool) -> MixinFacadeRequest {
    let mut request = request();
    let mut model = MixinSemanticModelParser::parse(&request.buffer_text, &request.document_uri);
    assert_eq!(model.injectors.len(), 1);
    model.parse_source = parse_source;
    model.targets[0].internal_name = "game/Target".into();
    if registered {
        model.injectors[0].handler_method_descriptor = Some("()V".into());
    } else {
        model.injectors.clear();
    }
    request.semantic_model = Some(model);
    request
}

#[test]
fn normalized_selector_completion_never_promotes_an_unregistered_same_spelling_injector() {
    let facts = Facts::new();
    // The host resolved this spelling to a non-official type, so its normalized
    // injector inventory is empty even though the source-only scanner guesses Inject.
    let request = selector_request(ParseSource::HostNormalized, false);
    let raw = AnnotationContextExtractor::extract_at_offset(
        &request.buffer_text,
        request.character as usize,
    )
    .unwrap();
    assert_eq!(raw.annotation, MixinAnnotation::Inject);
    assert!(SemanticCompletionContextExtractor::extract(
        &request.buffer_text,
        0,
        request.character as usize,
        request.semantic_model.as_ref(),
    )
    .is_none());
    let result = MixinServiceFacade::new(&facts, &facts)
        .complete_with_debug(
            &request,
            &MixinCompletionOptions::default(),
            "mcdev.completion",
            "java",
        )
        .unwrap();
    assert!(result.items.is_empty());
    assert!(!result.debug.semantic_context_found);
    assert!(!result.debug.fallback_annotation_context_used);
}

#[test]
fn normalized_selector_completion_keeps_the_registered_trusted_injector() {
    let facts = Facts::new();
    let request = selector_request(ParseSource::HostNormalized, true);
    let context = SemanticCompletionContextExtractor::extract(
        &request.buffer_text,
        0,
        request.character as usize,
        request.semantic_model.as_ref(),
    )
    .unwrap();
    assert!(
        matches!(context, MixinCompletionContext::InjectMethod { injector, .. }
        if injector.handler_method_descriptor.as_deref() == Some("()V"))
    );
    let result = MixinServiceFacade::new(&facts, &facts)
        .complete_with_debug(
            &request,
            &MixinCompletionOptions::default(),
            "mcdev.completion",
            "java",
        )
        .unwrap();
    assert_eq!(result.items[0].insert_text, "tick");
    assert!(result.debug.semantic_context_found);
    assert!(!result.debug.fallback_annotation_context_used);
}

#[test]
fn handwritten_selector_completion_retains_its_partial_injector_fallback() {
    let facts = Facts::new();
    let request = selector_request(ParseSource::HandWrittenFallback, false);
    let context = SemanticCompletionContextExtractor::extract(
        &request.buffer_text,
        0,
        request.character as usize,
        request.semantic_model.as_ref(),
    )
    .unwrap();
    assert!(
        matches!(context, MixinCompletionContext::InjectMethod { injector, .. }
        if injector.handler_method_descriptor.is_none())
    );
    let result = MixinServiceFacade::new(&facts, &facts)
        .complete_with_debug(
            &request,
            &MixinCompletionOptions::default(),
            "mcdev.completion",
            "java",
        )
        .unwrap();
    assert_eq!(result.items[0].insert_text, "tick");
    assert!(result.debug.semantic_context_found);
}

#[test]
fn public_host_pipeline_validates_negative_reads_and_all_generation_dimensions() {
    let facts = Facts::new();
    let cache = SemanticModelCache::default();
    let mut facade = HostMixinServiceFacade::new(&facts, &facts, &facts, &cache);
    facade.project = Some(HostProjectKey {
        project_id: "host".into(),
        generations: HostGenerations::default(),
    });
    facade.document_version = Some(1);
    let request = request();
    let options = MixinCompletionOptions::default();
    let first = facade
        .complete_with_debug(&request, &options, "java")
        .unwrap();
    let second = facade
        .complete_with_debug(&request, &options, "java")
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(first.items[0].insert_text, "tick");
    assert_eq!(first.debug.parse_source, Some(ParseSource::HostNormalized));
    assert_eq!(facts.parses.load(Ordering::SeqCst), 1);
    assert_eq!(facts.validations.load(Ordering::SeqCst), 1);
    for dimension in 0..8 {
        let generations = &mut facade.project.as_mut().unwrap().generations;
        match dimension {
            0 => generations.project += 1,
            1 => generations.model += 1,
            2 => generations.namespace += 1,
            3 => generations.classpath += 1,
            4 => generations.dependency += 1,
            5 => generations.generated_source += 1,
            6 => generations.config += 1,
            _ => generations.provider += 1,
        }
        let version = facts.revision.fetch_add(1, Ordering::SeqCst) + 1;
        let result = facade
            .complete_with_debug(&request, &options, "java")
            .unwrap();
        assert_eq!(result.debug.warnings, [format!("revision {version}")]);
        assert_eq!(facts.parses.load(Ordering::SeqCst), dimension + 2);
    }
    // An individual negative SDK read can change while the document stays identical.
    facts.revision.fetch_add(1, Ordering::SeqCst);
    facade.complete(&request, &options).unwrap();
    assert_eq!(facts.parses.load(Ordering::SeqCst), 10);
    assert!(facts.validations.load(Ordering::SeqCst) >= 2);
    facade.document_version = Some(2);
    facade.complete(&request, &options).unwrap();
    assert_eq!(facts.parses.load(Ordering::SeqCst), 11);
    let mut changed = request.clone();
    changed.buffer_text.push(' ');
    facade.complete(&changed, &options).unwrap();
    assert_eq!(facts.parses.load(Ordering::SeqCst), 12);
}
#[test]
fn host_unavailable_and_cancellation_never_become_empty_successes_or_cached_fallbacks() {
    let facts = Facts::new();
    let cache = SemanticModelCache::default();
    let mut facade = HostMixinServiceFacade::new(&facts, &facts, &facts, &cache);
    facade.project = Some(HostProjectKey {
        project_id: "host".into(),
        generations: HostGenerations::default(),
    });
    let request = request();
    let options = MixinCompletionOptions::default();
    facts.mode.store(1, Ordering::SeqCst);
    facade.complete(&request, &options).unwrap();
    facade.complete(&request, &options).unwrap();
    assert_eq!(
        facts.parses.load(Ordering::SeqCst),
        2,
        "transient answers must be recomputed"
    );
    assert_eq!(cache.len(), 0);
    facts.mode.store(3, Ordering::SeqCst);
    for _ in 0..2 {
        assert!(
            matches!(facade.complete(&request,&options),Err(HostFacadeError::Unavailable(reason))if reason=="normalized host semantic facts unavailable")
        );
    }
    assert_eq!(facts.parses.load(Ordering::SeqCst), 4);
    assert_eq!(cache.len(), 0);
    facts.mode.store(4, Ordering::SeqCst);
    assert!(
        matches!(facade.complete(&request,&options),Err(HostFacadeError::Source(HostSourceError::Cancelled(reason)))if reason=="host cancelled")
    );
    facts.mode.store(0, Ordering::SeqCst);
    assert_eq!(
        facade.complete(&request, &options).unwrap()[0].insert_text,
        "tick"
    );
}
#[test]
fn host_feature_failure_is_typed_and_pure_buffer_helpers_do_not_read_semantic_providers() {
    let facts = Facts::new();
    let cache = SemanticModelCache::default();
    let mut facade = HostMixinServiceFacade::new(&facts, &facts, &facts, &cache);
    let source = "@Inject(method = \"tick\", at = @At(\"INVOKE:\"))";
    let cursor = source.find("INVOKE:").unwrap() + 7;
    let request = MixinFacadeRequest {
        buffer_text: source.into(),
        character: cursor as i32,
        semantic_model: Some(MixinSemanticModelParser::parse(
            source,
            "file:///Mixin.java",
        )),
        ..Default::default()
    };
    facade.additional_at_values =
        Box::new(|_, _| Err(HostSourceError::Failure("host eval failed".into())));
    assert!(
        matches!(facade.complete(&request,&MixinCompletionOptions::default()),Err(HostFacadeError::Source(HostSourceError::Failure(reason)))if reason=="host eval failed")
    );
    let source = "@Inject(meth";
    let buffer = HostMixinFacade::try_complete_buffer_only(
        source,
        0,
        source.len() as i32,
        &MixinCompletionOptions::default(),
        "file:///Mixin.java",
        "java",
        false,
        AtValueCompletionService::new(
            || panic!("specifier evaluated"),
            || panic!("at provider evaluated"),
        ),
    )
    .unwrap()
    .unwrap();
    assert!(buffer
        .result
        .items
        .iter()
        .any(|i| i.filter_text == "method"));
    assert_eq!(facts.parses.load(Ordering::SeqCst), 0);
}
#[test]
fn integrated_kotlin_host_facts_are_authoritative_when_the_host_advertises_them() {
    let facts = Facts::new();
    let cache = SemanticModelCache::default();
    let mut facade = HostMixinServiceFacade::new(&facts, &facts, &facts, &cache);
    let source="import org.spongepowered.asm.mixin.Mixin\nimport org.spongepowered.asm.mixin.injection.Inject\nimport org.spongepowered.asm.mixin.injection.At\nimport game.Target\n@Mixin(Target::class)\nabstract class Patch {\n @Inject(method = [\"ti\"], at = [At(\"HEAD\")])\n private fun handler() {}\n}";
    let cursor = source.find("\"ti\"").unwrap() + 3;
    let p = offset_to_position(source, cursor).unwrap();
    let request = MixinFacadeRequest {
        buffer_text: source.into(),
        document_uri: "file:///Patch.kt".into(),
        line: p.line as i32,
        character: p.character as i32,
        ..Default::default()
    };
    let options = MixinCompletionOptions::default();
    let legacy = facade
        .complete_with_debug(&request, &options, "kotlin")
        .unwrap();
    assert_eq!(
        legacy.debug.parse_source,
        Some(ParseSource::HandWrittenFallback)
    );
    assert_eq!(facts.parses.load(Ordering::SeqCst), 0);
    facade.host_supports_kotlin_semantics = true;
    let normalized = facade
        .complete_with_debug(&request, &options, "kotlin")
        .unwrap();
    assert_eq!(
        normalized.debug.parse_source,
        Some(ParseSource::HostNormalized)
    );
    assert_eq!(normalized.items[0].insert_text, "tick");
    assert_eq!(normalized.items, legacy.items);
    assert_eq!(facts.parses.load(Ordering::SeqCst), 1);
}
