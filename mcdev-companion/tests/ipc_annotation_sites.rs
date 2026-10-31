//! Unit proofs for local selector completion; these are not original CEM/live-host credit.
use mcdev_companion::{
    commands::ipc::semantic::{HostSemanticProvider, SemanticFactClient},
    host_facts::FactsError,
    wire::Evaluation,
};
use mcdev_core::{
    bytecode::{AtTargetCandidate, BytecodeIndex},
    host_source::*,
    mixin_completion::MixinCompletionOptions,
    mixin_facade::{MixinFacadeRequest, MixinServiceFacade},
    mixin_source::*,
    *,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};

const SOURCE: &str =
    include_str!("fixtures/host_inputs/annotation_sites/d7-orphan-inject-crlf.java");
const URI: &str = "file:///project/ItemMixin.java";
fn found(value: Value) -> Value {
    json!({"state":"found","value":value})
}
fn unknown() -> Value {
    json!({"state":"unknown","reasons":[{"code":"IncompleteSource"}]})
}
fn span(text: &str) -> Value {
    let start = SOURCE.find(text).unwrap();
    json!({"start":start,"end":start+text.len()})
}
fn annotation(text: &str, name: &str, reference: &str, args: Value) -> Value {
    let start = SOURCE.find(text).unwrap() + text.find(name).unwrap();
    json!({"typeRef":reference,"useSiteTarget":null,"range":span(text),
        "nameRange":{"start":start,"end":start+name.len()},"arguments":args})
}
struct Host {
    declarations: Value,
    sites: Value,
    queries: Mutex<Vec<Value>>,
    custom_inject: bool,
    unknown_inject: bool,
    foreign_ref: bool,
    failure: Option<FactsError>,
}
impl Host {
    fn new() -> Self {
        let mixin = annotation(
            "@Mixin(Item.class)",
            "Mixin",
            "ref-mixin",
            json!([
                {"name":null,"range":span("Item.class"),"value":found(json!({"kind":"class","typeRef":"ref-item"}))}
            ]),
        );
        let inject = annotation(
            "@Inject(method = \"is\")",
            "Inject",
            "ref-inject",
            json!([
                {"name":"method","range":span("\"is\""),"value":found(json!({"kind":"string","value":"is","contentRange":span("is")}))}
            ]),
        );
        let full = SOURCE.find("@Mixin").unwrap();
        let body = SOURCE.find('{').unwrap();
        let end = SOURCE.rfind('}').unwrap() + 1;
        let declarations = json!([{"id":"class","owner":null,"name":"ItemMixin","range":span("ItemMixin"),"annotations":[mixin],
            "sourceKind":"Class","fullRange":{"start":full,"end":end},"headerRange":{"start":full,"end":body},
            "bodyRange":{"start":body,"end":end},"nameRange":span("ItemMixin"),
            "modifiers":{"visibility":found(json!("Public")),"static":found(json!(false)),"final":found(json!(false)),"abstract":found(json!(false))}}]);
        let mut mixin = mixin;
        mixin["owner"] = json!("class");
        let mut inject = inject;
        inject["owner"] = Value::Null;
        let mut sites = json!({"source":{"documentUri":found(json!(URI)),"identity":{"kind":"source","revision":"7"},
            "sha256":format!("{:x}",Sha256::digest(SOURCE.as_bytes()))},"annotations":[mixin,inject],"coverage":unknown()});
        sites["annotations"][0]["typeRef"] = json!("ref-mixin-alias");
        sites["annotations"][0]["arguments"][0]["value"]["value"]["typeRef"] =
            json!("ref-item-alias");
        sites["annotations"][1]["typeRef"] = json!("ref-inject-alias");
        Self {
            declarations,
            sites,
            queries: Mutex::new(Vec::new()),
            custom_inject: false,
            unknown_inject: false,
            foreign_ref: false,
            failure: None,
        }
    }
}
impl SemanticFactClient for Host {
    fn check_active(&self) -> Result<(), FactsError> {
        Ok(())
    }
    fn read_facts(&self, evaluation: &str, queries: &[Value]) -> Result<Value, FactsError> {
        assert_eq!(evaluation, "same-view");
        self.queries.lock().unwrap().extend_from_slice(queries);
        let facts=queries.iter().map(|query|->Result<Value,FactsError> { Ok(match query["op"].as_str().unwrap() {
            "sourceFacts"=>json!({"declarations":self.declarations}),
            "annotationSites"=> { assert!(query.get("source").is_none()); if let Some(error)=&self.failure{return Err(error.clone());} self.sites.clone() },
            "resolveTypeRef"=> {
                let reference=query["reference"].as_str().unwrap();
                let original=reference.strip_suffix("-alias").unwrap_or(reference);
                if original=="ref-inject" && self.unknown_inject {return Ok(unknown());}
                let name=match original {
                    "ref-mixin"=>"org.spongepowered.asm.mixin.Mixin",
                    "ref-item"=>"net.minecraft.world.item.Item",
                    "ref-inject" if self.custom_inject=>"custom.Inject",
                    "ref-inject"=>"org.spongepowered.asm.mixin.injection.Inject",
                    _=>panic!("unexpected reference {reference}"),
                };
                let mut ty=json!({"id":format!("type-{reference}"),"qualifiedName":name,"package":"fixture"});
                if original=="ref-inject" && self.foreign_ref {
                    ty["readOnly"]=json!(true);
                    ty["contextSource"]=json!({"id":"foreign-source","displayPath":"/project/Foreign.java","documentUri":found(json!("file:///project/Foreign.java")),
                        "languageId":"java","identity":{"kind":"source","revision":"7"}});
                }
                found(ty)
            }
            "declarationType"=> {assert_eq!(query["declaration"],"class"); found(json!({"id":"owner-type","qualifiedName":"io.github.cotrin8672.cem.mixin.ItemMixin","package":"io.github.cotrin8672.cem.mixin"}))},
            "typeJvm"=> {
                assert_eq!(query["target"],"type-ref-item");
                json!({"binaryName":found(json!({"internalName":"net/minecraft/world/item/Item","descriptor":"Lnet/minecraft/world/item/Item;"})),
                    "classFlags":unknown(),"declaredMembers":{"members":[],"coverage":unknown()},"abi":unknown(),"jvmAbiCoverage":unknown()})
            }
            "declarationMember"=>panic!("an orphan annotation must never acquire a fictitious member or JVM signature"),
            op=>panic!("unexpected operation {op}"),
        }) }).collect::<Result<Vec<_>,_>>()?;
        Ok(json!({"facts":facts}))
    }
}
fn position() -> McTextPosition {
    offset_to_position(SOURCE, SOURCE.find("\"is\"").unwrap() + 3).unwrap()
}
fn provider(
    host: Arc<Host>,
    enabled: bool,
    completion_position: Option<McTextPosition>,
) -> HostSemanticProvider {
    HostSemanticProvider::with_client(
        serde_json::from_value::<Evaluation>(
            json!({"evaluation":"same-view","owner":"/project/ItemMixin.java",
        "source":SOURCE,"configuration":{},"declarations":[]}),
        )
        .unwrap(),
        host,
        URI.into(),
        Some(3),
    )
    .unwrap()
    .with_source_facts(true)
    .with_annotation_sites(enabled, completion_position)
}
fn parse(provider: &HostSemanticProvider) -> HostSourceResult<HostSemanticModel> {
    provider.parse(&HostSemanticRequest {
        source: SOURCE,
        source_uri: URI,
        document_version: Some(3),
        environment: &HostSemanticEnvironment::default(),
        project: None,
    })
}
struct Index;
impl ClassIndex for Index {
    fn find_classes(&self, _: &str, _: usize) -> Vec<ClassIndexEntry> {
        vec![]
    }
    fn find_class(&self, owner: &str) -> Option<ClassIndexEntry> {
        (owner == "net/minecraft/world/item/Item").then(|| ClassIndexEntry {
            internal_name: owner.into(),
            simple_name: "Item".into(),
            package_name: "net.minecraft.world.item".into(),
        })
    }
    fn find_class_by_fqn(&self, name: &str) -> Option<ClassIndexEntry> {
        self.find_class(&name.replace('.', "/"))
    }
    fn get_methods(&self, owner: &str) -> Vec<MethodIndexEntry> {
        assert_eq!(owner, "net/minecraft/world/item/Item");
        vec![MethodIndexEntry {
            name: "isEdible".into(),
            descriptor: "()Z".into(),
            is_static: false,
            readable_signature: "isEdible(): boolean".into(),
        }]
    }
    fn get_fields(&self, _: &str) -> Vec<FieldIndexEntry> {
        vec![]
    }
}
impl BytecodeIndex for Index {
    fn get_at_target_candidates(
        &self,
        _: &str,
        _: &str,
        _: Option<&str>,
        _: &str,
    ) -> Vec<AtTargetCandidate> {
        panic!("selector completion must not read Code")
    }
    fn get_return_ordinal_count(&self, _: &str, _: &str, _: Option<&str>) -> usize {
        panic!("selector completion must not read Code")
    }
}

#[test]
fn exact_d7_positive_site_completes_under_unknown_coverage_without_a_handler_or_member() {
    assert_eq!(SOURCE.len(), 254);
    assert_eq!(
        format!("{:x}", Sha256::digest(SOURCE.as_bytes())),
        "7bd1004fdea0383b51e973c48f3a2e5162e359b9abc08eab559920e98e8df5a8"
    );
    let host = Arc::new(Host::new());
    let result = parse(&provider(host.clone(), true, Some(position()))).unwrap();
    let HostFact::Known(result) = result.fact else {
        panic!("local positive site was lost: {:?}", result.fact)
    };
    assert_eq!(result.model.parse_source, ParseSource::HostNormalized);
    assert_eq!(result.model.confidence, ParseConfidence::Medium);
    assert!(result.model.members.is_empty() && result.model.injectors.is_empty());
    let sites = result.model.host_selector_completions.as_ref().unwrap();
    assert!(matches!(sites.coverage, HostFact::Unavailable { .. }));
    assert_eq!(sites.injectors.len(), 1);
    assert_eq!(sites.injectors[0].handler_method_descriptor, None);
    assert_eq!(sites.injectors[0].method_selectors[0].value, "is");
    assert!(matches!(
        result
            .model
            .normalized_handler_source
            .as_ref()
            .unwrap()
            .sites,
        HostFact::Unavailable { .. }
    ));
    let p = position();
    let response = MixinServiceFacade::new(&Index, &Index)
        .complete(
            &MixinFacadeRequest {
                buffer_text: SOURCE.into(),
                document_uri: URI.into(),
                line: p.line as i32,
                character: p.character as i32,
                semantic_model: Some(result.model),
                ..Default::default()
            },
            &MixinCompletionOptions::default(),
        )
        .unwrap();
    assert_eq!(response.len(), 1);
    assert_eq!(response[0].insert_text, "isEdible");
    assert!(!host
        .queries
        .lock()
        .unwrap()
        .iter()
        .any(|query| query["op"] == "declarationMember"));
}

#[test]
fn complete_coverage_can_supply_the_same_selector_without_declaration_ownership() {
    let mut host = Host::new();
    host.sites["coverage"] = found(Value::Null);
    let result = parse(&provider(Arc::new(host), true, Some(position()))).unwrap();
    let HostFact::Known(result) = result.fact else {
        panic!("complete local sites unavailable")
    };
    assert!(matches!(
        result.model.host_selector_completions.unwrap().coverage,
        HostFact::Known(())
    ));
    assert!(result.model.members.is_empty() && result.model.injectors.is_empty());
}

#[test]
fn unnegotiated_or_missing_sites_still_leave_the_orphan_incomplete() {
    let host = Arc::new(Host::new());
    assert!(matches!(
        parse(&provider(host.clone(), false, Some(position())))
            .unwrap()
            .fact,
        HostFact::Unavailable { .. }
    ));
    assert!(!host
        .queries
        .lock()
        .unwrap()
        .iter()
        .any(|query| query["op"] == "annotationSites"));
    let mut host = Host::new();
    host.sites["annotations"].as_array_mut().unwrap().pop();
    assert!(matches!(
        parse(&provider(Arc::new(host), true, Some(position())))
            .unwrap()
            .fact,
        HostFact::Unavailable { .. }
    ));
}

#[test]
fn unknown_coverage_is_not_a_successful_empty_model_for_diagnostics_or_other_positions() {
    for completion_position in [
        None,
        Some(McTextPosition {
            line: 0,
            character: 0,
        }),
    ] {
        let result = parse(&provider(Arc::new(Host::new()), true, completion_position)).unwrap();
        assert!(matches!(result.fact, HostFact::Unavailable { .. }));
    }
    let mut host = Host::new();
    host.sites["annotations"] = json!([]);
    assert!(matches!(
        parse(&provider(Arc::new(host), true, Some(position())))
            .unwrap()
            .fact,
        HostFact::Unavailable { .. }
    ));
}

#[test]
fn same_spelling_unknown_identity_and_foreign_type_refs_never_gain_official_authority() {
    for mode in 0..3 {
        let mut host = Host::new();
        host.sites["coverage"] = found(Value::Null);
        host.custom_inject = mode == 0;
        host.unknown_inject = mode == 1;
        host.foreign_ref = mode == 2;
        let result = parse(&provider(Arc::new(host), true, Some(position()))).unwrap();
        assert!(matches!(result.fact, HostFact::Unavailable { .. }));
    }
}

#[test]
fn footprint_stale_unsupported_and_cancelled_remain_failures() {
    let mut host = Host::new();
    host.sites["source"]["sha256"] = json!("0".repeat(64));
    assert!(matches!(
        parse(&provider(Arc::new(host), true, Some(position())))
            .unwrap()
            .fact,
        HostFact::Unavailable { .. }
    ));
    for failure in [
        FactsError::Stale("retired view".into()),
        FactsError::Unsupported("UnsupportedSource".into()),
    ] {
        let mut host = Host::new();
        host.failure = Some(failure);
        assert!(matches!(
            parse(&provider(Arc::new(host), true, Some(position())))
                .unwrap()
                .fact,
            HostFact::Unavailable { .. }
        ));
    }
    let mut host = Host::new();
    host.failure = Some(FactsError::Cancelled("cancelled".into()));
    assert!(matches!(
        parse(&provider(Arc::new(host), true, Some(position()))),
        Err(HostSourceError::Cancelled(_))
    ));
}

struct ForeignHost {
    inner: Host,
    descriptor: Value,
    queries: Mutex<Vec<Value>>,
    wrong_revision: bool,
}
impl SemanticFactClient for ForeignHost {
    fn check_active(&self) -> Result<(), FactsError> {
        Ok(())
    }
    fn read_facts(&self, evaluation: &str, queries: &[Value]) -> Result<Value, FactsError> {
        self.queries.lock().unwrap().extend_from_slice(queries);
        let mut result = Vec::new();
        for query in queries {
            let op = query["op"].as_str().unwrap();
            if op == "typeJvm" {
                return Err(FactsError::Unsupported(
                    "UnsupportedSource: foreign typeJvm".into(),
                ));
            }
            let mut own = query.clone();
            if matches!(op, "sourceFacts" | "annotationSites") {
                assert_eq!(
                    own.as_object_mut().unwrap().remove("source"),
                    Some(json!("source:foreign"))
                );
            }
            let response = self.inner.read_facts(evaluation, &[own])?;
            let mut value = response["facts"][0].clone();
            if op == "sourceFacts" {
                value["source"] = self.descriptor.clone();
                value["semanticScope"] = found(Value::Null);
            }
            if op == "annotationSites" && self.wrong_revision {
                value["source"]["identity"]["revision"] = json!("other-generation");
            }
            if matches!(op, "resolveTypeRef" | "declarationType") && value["state"] == "found" {
                value["value"]["readOnly"] = json!(true);
                let mut context = self.descriptor.clone();
                context["id"] = json!("source:reissued-context-alias");
                value["value"]["contextSource"] = context;
            }
            result.push(value);
        }
        Ok(json!({"facts":result}))
    }
}
fn foreign_provider(wrong_revision: bool) -> (HostSemanticProvider, Arc<ForeignHost>) {
    use mcdev_companion::host_facts::{bridge::FactReader, inputs::*};
    struct Entry(Value);
    impl FactReader for Entry {
        fn read_facts(&mut self, evaluation: &str, queries: &[Value]) -> Result<Value, FactsError> {
            assert_eq!(evaluation, "same-view");
            assert_eq!(queries[0]["op"], "sourceInventory");
            Ok(json!({"facts":[{"sources":[self.0],"next":null,"coverage":found(Value::Null)}]}))
        }
    }
    let descriptor = json!({"id":"source:foreign","displayPath":"/project/ItemMixin.java","documentUri":found(json!(URI)),
        "languageId":"java","identity":{"kind":"source","revision":"7"},"documentVersion":3});
    let ops=InputOps::intersect(&json!({"capabilities":["source-snapshots-v1"],"supportedOps":{"source-snapshots-v1":["sourceInventory"]}})).unwrap();
    let mut inputs = ScopedInputs::new("same-view", &ops, Entry(descriptor.clone())).unwrap();
    let entry = inputs.source_inventory().unwrap().entries.remove(0);
    let mut snapshot = descriptor.clone();
    snapshot["text"] = json!(SOURCE);
    snapshot["sha256"] = json!(format!("{:x}", Sha256::digest(SOURCE.as_bytes())));
    let mut inner = Host::new();
    inner.sites["coverage"] = found(Value::Null);
    let host = Arc::new(ForeignHost {
        inner,
        descriptor,
        queries: Mutex::new(Vec::new()),
        wrong_revision,
    });
    let evaluation=serde_json::from_value(json!({"evaluation":"same-view","owner":"/project/Owner.java","source":"class Owner {}","configuration":{},"declarations":[]})).unwrap();
    let provider = HostSemanticProvider::with_client(
        evaluation,
        host.clone(),
        "file:///project/Owner.java".into(),
        Some(99),
    )
    .unwrap()
    .with_source_facts(true)
    .with_annotation_sites(true, None)
    .with_project_sources(vec![(entry, serde_json::from_value(snapshot).unwrap())])
    .unwrap();
    (provider, host)
}

#[test]
fn foreign_annotation_sites_use_the_issued_source_and_readonly_references_without_granting_type_jvm(
) {
    let (provider, host) = foreign_provider(false);
    let answer = parse(&provider).unwrap();
    assert!(
        matches!(answer.fact,HostFact::Unavailable {reason} if reason.contains("UnsupportedSource"))
    );
    let queries = host.queries.lock().unwrap();
    assert!(queries
        .iter()
        .any(|query| query["op"] == "annotationSites" && query["source"] == "source:foreign"));
    assert!(queries
        .iter()
        .any(|query| query["op"] == "resolveTypeRef" && query["reference"] == "ref-inject-alias"));
    assert!(queries.iter().any(|query| query["op"] == "typeJvm"));
    assert!(!queries
        .iter()
        .any(|query| query["op"] == "declarationMember"));
}

#[test]
fn foreign_annotation_site_footprint_cannot_replace_the_captured_source_generation() {
    let (provider, host) = foreign_provider(true);
    assert!(
        matches!(parse(&provider).unwrap().fact,HostFact::Unavailable {reason} if reason.contains("snapshot identity mismatch"))
    );
    assert!(!host
        .queries
        .lock()
        .unwrap()
        .iter()
        .any(|query| query["op"] == "resolveTypeRef"));
}

#[test]
fn equivalent_current_owner_uri_spelling_keeps_only_the_authorized_local_completion() {
    let provider = provider(Arc::new(Host::new()), true, Some(position()));
    let result = provider
        .parse(&HostSemanticRequest {
            source: SOURCE,
            source_uri: "file:///project/./ItemMixin.java",
            document_version: Some(3),
            environment: &HostSemanticEnvironment::default(),
            project: None,
        })
        .unwrap();
    let HostFact::Known(model) = result.fact else {
        panic!(
            "equivalent file URI lost current-owner authority: {:?}",
            result.fact
        )
    };
    assert_eq!(
        model
            .model
            .host_selector_completions
            .unwrap()
            .injectors
            .len(),
        1
    );
    assert!(model.model.members.is_empty() && model.model.injectors.is_empty());
}
