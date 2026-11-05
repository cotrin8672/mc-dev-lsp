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
    unknown_item_alias: bool,
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
            unknown_item_alias: false,
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
                if reference=="ref-item-alias" && self.unknown_item_alias {return Ok(unknown());}
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

const OWNED_SOURCE: &str = "package io.github.cotrin8672.cem.mixin;\nimport net.minecraft.world.item.Item;\nimport org.spongepowered.asm.mixin.Mixin;\nimport org.spongepowered.asm.mixin.injection.Inject;\n@Mixin(Item.class)\npublic class ItemMixin {\n    @Inject(method = \"\")\n    private void mcdev$handler() {}\n}\n";
const OWNED_URI: &str = "file:///project/OwnedItemMixin.java";

struct OwnedHost {
    declarations: Value,
    sites: Value,
    queries: Mutex<Vec<Value>>,
}

impl OwnedHost {
    fn new() -> Self {
        let span = |text: &str| {
            let start = OWNED_SOURCE.find(text).unwrap();
            json!({"start":start,"end":start+text.len()})
        };
        let annotation = |text: &str, name: &str, reference: &str, args: Value| {
            let start = OWNED_SOURCE.find(text).unwrap() + text.find(name).unwrap();
            json!({"typeRef":reference,"useSiteTarget":null,"range":span(text),
                "nameRange":{"start":start,"end":start+name.len()},"arguments":args})
        };
        let mixin = annotation(
            "@Mixin(Item.class)",
            "Mixin",
            "ref-mixin",
            json!([{"name":null,"range":span("Item.class"),
                "value":found(json!({"kind":"class","typeRef":"ref-item"}))}]),
        );
        let empty_selector = OWNED_SOURCE.find("method = \"\"").unwrap() + "method = \"".len();
        let inject = annotation(
            "@Inject(method = \"\")",
            "Inject",
            "ref-inject",
            json!([{"name":"method","range":span("\"\""),
            "value":found(json!({"kind":"string","value":"","contentRange":{
                "start":empty_selector,"end":empty_selector,
            }}))}]),
        );
        let class_start = OWNED_SOURCE.find("@Mixin").unwrap();
        let class_body = OWNED_SOURCE.find('{').unwrap();
        let class_end = OWNED_SOURCE.rfind('}').unwrap() + 1;
        let class_declaration = json!({"id":"class","owner":null,"name":"ItemMixin",
            "range":span("ItemMixin"),"annotations":[mixin.clone()],"sourceKind":"Class",
            "fullRange":{"start":class_start,"end":class_end},
            "headerRange":{"start":class_start,"end":class_body},
            "bodyRange":{"start":class_body,"end":class_end},"nameRange":span("ItemMixin"),
            "modifiers":{"visibility":found(json!("Public")),"static":found(json!(false)),
                "final":found(json!(false)),"abstract":found(json!(false))}});
        let handler_start = OWNED_SOURCE.find("@Inject").unwrap();
        let handler_name = OWNED_SOURCE.find("mcdev$handler").unwrap();
        let handler_body = OWNED_SOURCE[handler_name..].find('{').unwrap() + handler_name;
        let handler_end = OWNED_SOURCE[handler_body..].find('}').unwrap() + handler_body + 1;
        let mut handler = inject.clone();
        handler["owner"] = Value::Null;
        let handler_declaration = json!({"id":"handler","owner":"class","name":"mcdev$handler",
            "range":{"start":handler_name,"end":handler_name+"mcdev$handler".len()},
            "annotations":[handler],"sourceKind":"Function",
            "fullRange":{"start":handler_start,"end":handler_end},
            "headerRange":{"start":handler_start,"end":handler_body},
            "bodyRange":{"start":handler_body,"end":handler_end},
            "nameRange":{"start":handler_name,"end":handler_name+"mcdev$handler".len()},
            "modifiers":{"visibility":found(json!("Private")),"static":found(json!(false)),
                "final":found(json!(false)),"abstract":found(json!(false))}});
        let mut site_mixin = mixin;
        site_mixin["owner"] = json!("class");
        let mut site_inject = inject;
        site_inject["owner"] = json!("handler");
        let sites = json!({"source":{"documentUri":found(json!(OWNED_URI)),
                "identity":{"kind":"source","revision":"owned-1"},
                "sha256":format!("{:x}",Sha256::digest(OWNED_SOURCE.as_bytes()))},
            "annotations":[site_mixin,site_inject],"coverage":unknown()});
        Self {
            declarations: json!([class_declaration, handler_declaration]),
            sites,
            queries: Mutex::new(Vec::new()),
        }
    }
}

impl SemanticFactClient for OwnedHost {
    fn check_active(&self) -> Result<(), FactsError> {
        Ok(())
    }

    fn read_facts(&self, evaluation: &str, queries: &[Value]) -> Result<Value, FactsError> {
        assert_eq!(evaluation, "owned-view");
        self.queries.lock().unwrap().extend_from_slice(queries);
        let facts = queries.iter().map(|query| -> Result<Value, FactsError> {
            Ok(match query["op"].as_str().unwrap() {
                "sourceFacts" => json!({"declarations":self.declarations}),
                "annotationSites" => self.sites.clone(),
                "resolveTypeRef" => {
                    let reference = query["reference"].as_str().unwrap();
                    let qualified = match reference {
                        "ref-mixin" => "org.spongepowered.asm.mixin.Mixin",
                        "ref-item" => "net.minecraft.world.item.Item",
                        "ref-inject" => "org.spongepowered.asm.mixin.injection.Inject",
                        other => panic!("unexpected reference {other}"),
                    };
                    found(json!({"id":format!("type-{reference}"),"qualifiedName":qualified,"package":"fixture"}))
                }
                "declarationType" => found(json!({"id":"owner-type",
                    "qualifiedName":"io.github.cotrin8672.cem.mixin.ItemMixin",
                    "package":"io.github.cotrin8672.cem.mixin"})),
                "declarationMember" => {
                    assert_eq!(query["declaration"], "handler");
                    found(json!({"id":"member-handler","name":"mcdev$handler","kind":"method",
                        "signature":found(json!({"id":"signature-handler","kind":"method","descriptor":"()V"})),
                        "dispatch":found(json!("instance")),"visibility":found(json!("Private")),
                        "synthetic":found(json!(false)),"mismatches":[]}))
                }
                "typeJvm" => {
                    assert_eq!(query["target"], "type-ref-item");
                    let unknown = unknown();
                    json!({"binaryName":found(json!({"internalName":"net/minecraft/world/item/Item",
                        "descriptor":"Lnet/minecraft/world/item/Item;"})),
                        "classFlags":unknown,"declaredMembers":{"members":[],"coverage":unknown},
                        "abi":unknown,"jvmAbiCoverage":unknown})
                }
                op => panic!("unexpected operation {op}"),
            })
        }).collect::<Result<Vec<_>,_>>()?;
        Ok(json!({"facts":facts}))
    }
}

fn owned_position() -> mcdev_core::McTextPosition {
    offset_to_position(
        OWNED_SOURCE,
        OWNED_SOURCE.find("method = \"\"").unwrap() + "method = \"".len(),
    )
    .unwrap()
}

fn owned_provider(
    host: Arc<OwnedHost>,
    completion_position: Option<mcdev_core::McTextPosition>,
) -> HostSemanticProvider {
    HostSemanticProvider::with_client(
        serde_json::from_value::<Evaluation>(json!({"evaluation":"owned-view",
            "owner":OWNED_URI,"source":OWNED_SOURCE,"configuration":{},"declarations":[]}))
        .unwrap(),
        host,
        OWNED_URI.into(),
        Some(3),
    )
    .unwrap()
    .with_source_facts(true)
    .with_annotation_sites(true, completion_position)
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
fn owned_injector_keeps_local_selector_completion_under_unknown_coverage() {
    let host = Arc::new(OwnedHost::new());
    let provider = owned_provider(host.clone(), Some(owned_position()));
    let p = owned_position();
    let answer = provider
        .parse(&HostSemanticRequest {
            source: OWNED_SOURCE,
            source_uri: OWNED_URI,
            document_version: Some(3),
            environment: &HostSemanticEnvironment::default(),
            project: None,
        })
        .unwrap();
    let HostFact::Known(result) = answer.fact else {
        panic!("owned local site was lost: {:?}", answer.fact)
    };
    assert_eq!(result.model.injectors.len(), 1);
    assert_eq!(result.model.injectors[0].method_selectors[0].value, "");
    assert_eq!(result.model.confidence, ParseConfidence::Medium);
    assert!(matches!(
        result
            .model
            .host_selector_completions
            .as_ref()
            .unwrap()
            .coverage,
        HostFact::Unavailable { .. }
    ));
    let response = MixinServiceFacade::new(&Index, &Index)
        .complete(
            &MixinFacadeRequest {
                buffer_text: OWNED_SOURCE.into(),
                document_uri: OWNED_URI.into(),
                line: p.line as i32,
                character: p.character as i32,
                semantic_model: Some(result.model),
                ..Default::default()
            },
            &MixinCompletionOptions::default(),
        )
        .unwrap();
    assert!(response.iter().any(|item| item.insert_text == "isEdible"));
    assert!(host
        .queries
        .lock()
        .unwrap()
        .iter()
        .any(|query| query["op"] == "declarationMember"));

    for completion_position in [
        None,
        Some(McTextPosition {
            line: 0,
            character: 0,
        }),
    ] {
        let answer = owned_provider(host.clone(), completion_position)
            .parse(&HostSemanticRequest {
                source: OWNED_SOURCE,
                source_uri: OWNED_URI,
                document_version: Some(3),
                environment: &HostSemanticEnvironment::default(),
                project: None,
            })
            .unwrap();
        assert!(matches!(answer.fact, HostFact::Unavailable { .. }));
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
fn recovered_class_annotation_uses_one_scoped_mixin_site_as_current_owner() {
    let mut host = Host::new();
    host.declarations[0]["annotations"] = json!([]);
    host.sites["annotations"][0]["owner"] = Value::Null;
    host.sites["annotations"][0]["arguments"][0]["value"]["value"]["typeRef"] = json!("ref-item");
    let result = parse(&provider(Arc::new(host), true, Some(position()))).unwrap();
    let HostFact::Known(result) = result.fact else {
        panic!("positive current Mixin site did not recover the class owner")
    };
    let selectors = result.model.host_selector_completions.unwrap();
    assert_eq!(selectors.injectors.len(), 1);
    assert_eq!(selectors.injectors[0].method_selectors[0].value, "is");
    assert!(matches!(selectors.coverage, HostFact::Unavailable { .. }));
}

#[test]
fn ownerless_mixin_recovery_requires_one_matching_class_header() {
    for ambiguous in [false, true] {
        let mut host = Host::new();
        host.declarations[0]["annotations"] = json!([]);
        host.declarations[0]["headerRange"] = if ambiguous {
            host.declarations[0]["headerRange"].clone()
        } else {
            span("ItemMixin")
        };
        host.sites["annotations"][0]["owner"] = Value::Null;
        host.sites["annotations"][0]["arguments"][0]["value"]["value"]["typeRef"] =
            json!("ref-item");
        if ambiguous {
            let mut duplicate = host.declarations[0].clone();
            duplicate["id"] = json!("class-duplicate");
            host.declarations.as_array_mut().unwrap().push(duplicate);
        }
        let host = Arc::new(host);
        let answer = parse(&provider(host.clone(), true, Some(position()))).unwrap();
        assert!(
            matches!(answer.fact, HostFact::Unavailable { ref reason } if reason.contains("current Mixin owner")),
            "ambiguous={ambiguous}: {answer:?}"
        );
        let queries = host.queries.lock().unwrap();
        assert!(!queries
            .iter()
            .any(|query| matches!(query["op"].as_str(), Some("declarationType" | "typeJvm"))));
    }
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

#[test]
fn nonpositive_class_literal_alias_reports_the_collected_pair_states_and_ranges() {
    let mut host = Host::new();
    host.unknown_item_alias = true;
    let result = parse(&provider(Arc::new(host), true, Some(position()))).unwrap();
    let HostFact::Unavailable { reason } = result.fact else {
        panic!("nonpositive alias must remain unavailable")
    };
    let diagnostic: Value =
        serde_json::from_str(reason.split_once("; annotationAlias=").unwrap().1).unwrap();
    assert_eq!(diagnostic["collectedReferenceCount"], 5);
    assert_eq!(diagnostic["known"]["reference"], "ref-item");
    assert_eq!(diagnostic["known"]["state"], "found");
    assert_eq!(
        diagnostic["known"]["qualifiedName"],
        "net.minecraft.world.item.Item"
    );
    assert_eq!(diagnostic["site"]["reference"], "ref-item-alias");
    assert_eq!(diagnostic["site"]["collected"], true);
    assert_eq!(diagnostic["site"]["state"], "unknown");
    assert_eq!(diagnostic["site"]["reasonCodes"][0], "IncompleteSource");
    assert_eq!(diagnostic["field"]["kind"], "classLiteral");
    assert_eq!(diagnostic["field"]["knownRange"], span("Item.class"));
    assert_eq!(diagnostic["field"]["siteRange"], span("Item.class"));
    assert_eq!(diagnostic["annotation"]["knownOwner"], "class");
    assert_eq!(diagnostic["annotation"]["siteOwner"], "class");
    assert_eq!(
        diagnostic["annotation"]["sourceSha256"],
        "7bd1004fdea0383b51e973c48f3a2e5162e359b9abc08eab559920e98e8df5a8"
    );
}
