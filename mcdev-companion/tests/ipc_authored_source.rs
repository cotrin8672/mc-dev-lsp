use mcdev_companion::{
    commands::ipc::semantic::{HostSemanticProvider, SemanticFactClient},
    host_facts::FactsError,
    wire::Evaluation,
};
use mcdev_core::{
    host_source::{
        HostFact, HostSemanticEnvironment, HostSemanticModelProvider, HostSemanticRequest,
        SemanticAvailability,
    },
    mixin_source::{JavaTypeKind, ParseConfidence, ParseSource},
    offset_to_position,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

fn found(value: Value) -> Value {
    json!({"state":"found","value":value})
}
fn span(source: &str, text: &str) -> Value {
    let start = source.find(text).unwrap();
    json!({"start":start,"end":start+text.len()})
}
fn modifiers(visibility: &str) -> Value {
    json!({"visibility":found(json!(visibility)),"static":found(json!(false)),
        "final":found(json!(false)),"abstract":found(json!(false))})
}
struct Host {
    declarations: Value,
    queries: Mutex<Vec<Value>>,
}
impl SemanticFactClient for Host {
    fn check_active(&self) -> Result<(), FactsError> {
        Ok(())
    }
    fn read_facts(&self, evaluation: &str, queries: &[Value]) -> Result<Value, FactsError> {
        assert_eq!(evaluation, "evaluation");
        self.queries.lock().unwrap().extend_from_slice(queries);
        let facts = queries.iter().map(|query| match query["op"].as_str().unwrap() {
            "sourceFacts" => json!({"declarations":self.declarations}),
            "resolveTypeRef" => {
                let name = match query["reference"].as_str().unwrap() {
                    "mixin-ref" => "org.spongepowered.asm.mixin.Mixin",
                    "inject-ref" => "org.spongepowered.asm.mixin.injection.Inject",
                    "at-ref" => "org.spongepowered.asm.mixin.injection.At",
                    "target-ref" => "fixture.Target",
                    other => panic!("unexpected reference {other}"),
                };
                found(json!({"id":format!("type-{name}"),"qualifiedName":name,"package":"fixture"}))
            }
            "declarationType" => found(json!({"id":"owner-type","qualifiedName":"fixture.雪Mixin","package":"fixture"})),
            "declarationMember" => found(json!({"id":"hook-member","name":"hook","kind":"method",
                "signature":found(json!({"id":"hook-signature","kind":"method","descriptor":"(I)V"})),
                "dispatch":found(json!("instance")),"visibility":found(json!("Private")),
                "synthetic":found(json!(false)),"mismatches":[]})),
            "typeJvm" => {
                assert_eq!(query["target"], "type-fixture.Target");
                let unknown = json!({"state":"unknown","reasons":[{"code":"MissingFacts"}]});
                json!({"binaryName":found(json!({"internalName":"fixture/Target","descriptor":"Lfixture/Target;"})),
                    "classFlags":unknown,"declaredMembers":{"members":[],"coverage":unknown},
                    "abi":unknown,"jvmAbiCoverage":unknown})
            }
            other => panic!("unexpected operation {other}"),
        }).collect::<Vec<_>>();
        Ok(json!({"facts":facts}))
    }
}
fn annotation(source: &str, text: &str, name: &str, reference: &str, arguments: Value) -> Value {
    let start = source.find(text).unwrap();
    let name_start = start + text.find(name).unwrap();
    json!({"typeRef":reference,"useSiteTarget":null,"range":span(source,text),
        "nameRange":{"start":name_start,"end":name_start+name.len()},"arguments":arguments})
}
fn fixture() -> (String, Value) {
    let source = "//😀\n@Mixin(Target.class) class 雪Mixin {\n @Inject(method=\"ti\" + \"ck()V\", at=@At(value=\"HEAD\", ordinal=1))\n private void hook(int n) {}\n}".to_owned();
    let mixin = annotation(
        &source,
        "@Mixin(Target.class)",
        "Mixin",
        "mixin-ref",
        json!([
            {"name":null,"range":span(&source,"Target.class"),"value":found(json!({"kind":"class","typeRef":"target-ref"}))}
        ]),
    );
    let at = annotation(
        &source,
        "@At(value=\"HEAD\", ordinal=1)",
        "At",
        "at-ref",
        json!([
            {"name":"value","range":span(&source,"\"HEAD\""),"value":found(json!({"kind":"string","value":"HEAD","contentRange":span(&source,"HEAD")}))},
            {"name":"ordinal","range":span(&source,"1"),"value":found(json!({"kind":"integer","type":"int","value":1}))}
        ]),
    );
    let expression = "\"ti\" + \"ck()V\"";
    let expression_start = source.find(expression).unwrap();
    let segments = [1,2,8,9,10,11,12].into_iter().enumerate().map(|(decoded, offset)|
        json!({"decoded":{"start":decoded,"end":decoded+1},"source":{"start":expression_start+offset,"end":expression_start+offset+1}})).collect::<Vec<_>>();
    let inject = annotation(
        &source,
        "@Inject(method=\"ti\" + \"ck()V\", at=@At(value=\"HEAD\", ordinal=1))",
        "Inject",
        "inject-ref",
        json!([
            {"name":"method","range":span(&source,expression),"value":found(json!({"kind":"mappedString","value":"tick()V","segments":segments}))},
            {"name":"at","range":span(&source,"@At(value=\"HEAD\", ordinal=1)"),"value":found(json!({"kind":"annotation","annotation":at}))}
        ]),
    );
    let class_start = source.find("@Mixin").unwrap();
    let class_body = source.find('{').unwrap();
    let member_start = source.find("@Inject").unwrap();
    let member_body = source.rfind("{}").unwrap();
    let declarations = json!([
        {"id":"class-declaration","owner":null,"name":"雪Mixin","range":span(&source,"雪Mixin"),"annotations":[mixin],
            "sourceKind":"Class","fullRange":{"start":class_start,"end":source.len()},
            "headerRange":{"start":class_start,"end":class_body},"nameRange":span(&source,"雪Mixin"),
            "bodyRange":{"start":class_body,"end":source.len()},"modifiers":modifiers("Package")},
        {"id":"method-declaration","owner":"class-declaration","name":"hook","range":span(&source,"hook"),"annotations":[inject],
            "sourceKind":"Function","fullRange":{"start":member_start,"end":member_body+2},
            "headerRange":{"start":member_start,"end":member_body},"nameRange":span(&source,"hook"),
            "bodyRange":{"start":member_body,"end":member_body+2},"modifiers":modifiers("Private")}
    ]);
    (source, declarations)
}
fn parse(
    source: &str,
    declarations: Value,
) -> (
    HostFact<mcdev_core::host_source::HostSemanticModel>,
    Arc<Host>,
) {
    let evaluation: Evaluation =
        serde_json::from_value(json!({"evaluation":"evaluation","owner":"owner",
        "source":source,"configuration":{},"declarations":[]}))
        .unwrap();
    let host = Arc::new(Host {
        declarations,
        queries: Mutex::new(Vec::new()),
    });
    let provider = HostSemanticProvider::with_client(
        evaluation,
        host.clone(),
        "file:///project/雪Mixin.java".into(),
        Some(3),
    )
    .unwrap()
    .with_source_facts(true);
    let result = provider
        .parse(&HostSemanticRequest {
            source,
            source_uri: "file:///project/雪Mixin.java",
            document_version: Some(3),
            environment: &HostSemanticEnvironment::default(),
            project: None,
        })
        .unwrap();
    (result.fact, host)
}

#[test]
fn first_evaluation_uses_authored_ranges_and_nested_mapped_values() {
    let (source, declarations) = fixture();
    let (answer, host) = parse(&source, declarations);
    let HostFact::Known(result) = answer else {
        panic!("expected normalized source facts: {answer:?}")
    };
    assert_eq!(host.queries.lock().unwrap()[0], json!({"op":"sourceFacts"}));
    assert_eq!(result.availability, SemanticAvailability::Normalized);
    assert_eq!(result.model.parse_source, ParseSource::HostNormalized);
    assert_eq!(result.model.confidence, ParseConfidence::High);
    assert_eq!(result.model.type_kind, JavaTypeKind::Class);
    assert_eq!(result.model.targets[0].internal_name, "fixture/Target");
    assert!(result.model.debug_info.fallback_reason.is_none());
    let injector = &result.model.injectors[0];
    assert_eq!(injector.method_selectors[0].value, "tick()V");
    assert_eq!(
        injector.method_selectors[0].descriptor.as_deref(),
        Some("()V")
    );
    assert_eq!(injector.handler_method_descriptor.as_deref(), Some("(I)V"));
    assert_eq!(injector.at_selectors[0].value, "HEAD");
    assert_eq!(injector.at_selectors[0].ordinal, Some(1));
    assert_eq!(
        injector.at_selectors[0].range.start,
        offset_to_position(&source, source.find("@At").unwrap()).unwrap()
    );
}

#[test]
fn unknown_unused_handler_finality_does_not_invent_a_projected_member_flag() {
    let (source, mut declarations) = fixture();
    declarations[1]["modifiers"]["final"] =
        json!({"state":"unknown","reasons":[{"code":"UnsupportedSource"}]});
    let HostFact::Known(model) = parse(&source, declarations).0 else {
        panic!("independent injector facts should remain available")
    };
    assert!(model.model.members.is_empty());
    assert!(matches!(
        model.model.normalized_handler_source.unwrap().sites,
        HostFact::Unavailable { .. }
    ));
}

#[test]
fn malformed_authored_header_range_is_rejected() {
    let (source, mut declarations) = fixture();
    declarations[1]["headerRange"] = json!({"start":0,"end":1});
    assert!(matches!(
        parse(&source, declarations).0,
        HostFact::Unavailable { .. }
    ));
}

#[test]
fn omitted_minecraft_annotation_is_rejected_instead_of_a_successful_empty_model() {
    let (source, _) = fixture();
    assert!(matches!(
        parse(&source, json!([])).0,
        HostFact::Unavailable { .. }
    ));
}

#[test]
fn a_normalized_document_without_mixin_has_an_authoritative_empty_model() {
    let source = "class Plain {}";
    let declarations = json!([{"id":"plain","owner":null,"name":"Plain","range":span(source,"Plain"),
        "annotations":[],"sourceKind":"Class","fullRange":{"start":0,"end":source.len()},
        "headerRange":{"start":0,"end":12},"nameRange":span(source,"Plain"),
        "bodyRange":{"start":12,"end":14},"modifiers":modifiers("Package")}]);
    let (answer, host) = parse(source, declarations);
    let HostFact::Known(result) = answer else {
        panic!("expected normalized no-Mixin document")
    };
    assert_eq!(result.availability, SemanticAvailability::Normalized);
    assert!(
        result.model.members.is_empty()
            && result.model.targets.is_empty()
            && result.model.injectors.is_empty()
    );
    assert_eq!(host.queries.lock().unwrap().len(), 1);
}

#[derive(Clone)]
struct ForeignHost {
    descriptor: Value,
    source: String,
    declarations: Value,
    scope_known: bool,
    calls: Arc<Mutex<Vec<Value>>>,
    bad_context: Arc<std::sync::atomic::AtomicBool>,
}
impl SemanticFactClient for ForeignHost {
    fn check_active(&self) -> Result<(), FactsError> {
        Ok(())
    }
    fn read_facts(&self, evaluation: &str, queries: &[Value]) -> Result<Value, FactsError> {
        assert_eq!(evaluation, "evaluation");
        self.calls.lock().unwrap().extend_from_slice(queries);
        let unknown = || json!({"state":"unknown","reasons":[{"code":"UnsupportedSource"}]});
        let mut context = self.descriptor.clone();
        context["id"] = json!("source:context");
        if self.bad_context.load(std::sync::atomic::Ordering::SeqCst) {
            context["identity"]["revision"] = json!("foreign");
        }
        let facts = queries.iter().map(|query| match query["op"].as_str().unwrap() {
            "sourceInventory" => json!({"sources":[self.descriptor],"next":null,"coverage":found(Value::Null)}),
            "sourceSnapshot" => {
                use sha2::{Digest,Sha256};
                let mut snapshot = self.descriptor.clone(); snapshot["text"] = json!(self.source);
                snapshot["sha256"] = json!(format!("{:x}",Sha256::digest(self.source.as_bytes())));
                found(snapshot)
            }
            "sourceFacts" => {
                assert_eq!(query["source"],self.descriptor["id"]);
                json!({"source":self.descriptor,"semanticScope":if self.scope_known {found(Value::Null)} else {unknown()},
                    "declarations":if self.scope_known {self.declarations.clone()} else {json!([])}})
            }
            "resolveType" => {
                assert_eq!(query["name"],"fixture.OtherMixin");
                if self.scope_known {found(json!({"id":"selected:registered","qualifiedName":"fixture.OtherMixin","package":"fixture"}))}
                else {unknown()}
            }
            "sourceLocation" => {
                use sha2::{Digest,Sha256};
                assert_eq!(query["target"],"selected:registered");
                let start = self.source.find("OtherMixin").unwrap();
                found(json!({"target":query["target"],"documentUri":self.descriptor["documentUri"]["value"],
                    "range":{"start":offset_to_position(&self.source,start).unwrap(),
                        "end":offset_to_position(&self.source,start+"OtherMixin".len()).unwrap()},
                    "source":{"identity":self.descriptor["identity"],"sha256":if self.bad_context.load(std::sync::atomic::Ordering::SeqCst) {
                        "0".repeat(64)
                    } else {format!("{:x}",Sha256::digest(self.source.as_bytes()))}}}))
            }
            "resolveTypeRef" => {
                let fqn = match query["reference"].as_str().unwrap() {
                    "foreign:mixin" => "org.spongepowered.asm.mixin.Mixin",
                    "foreign:shadow" => "org.spongepowered.asm.mixin.Shadow",
                    "foreign:target" => "fixture.Target",
                    other => panic!("unexpected foreign reference {other}"),
                };
                found(json!({"id":format!("readonly:{fqn}"),"qualifiedName":fqn,"package":"fixture",
                    "readOnly":true,"contextSource":context}))
            }
            "declarationType" => found(json!({"id":"readonly:owner","qualifiedName":"fixture.OtherMixin",
                "package":"fixture","readOnly":true,"contextSource":context})),
            "declarationMember" => found(json!({"id":"readonly:value","name":"value","kind":"field",
                "signature":found(json!({"id":"readonly:signature","kind":"field","descriptor":"I"})),
                "dispatch":found(json!("instance")),"visibility":found(json!("Private")),
                "synthetic":found(json!(false)),"mismatches":[],"readOnly":true,"contextSource":context})),
            "typeJvm" => {
                assert_eq!(query["target"],"readonly:fixture.Target");
                json!({"binaryName":unknown(),"classFlags":unknown(),"declaredMembers":{"members":[],"coverage":unknown()},
                    "abi":unknown(),"jvmAbiCoverage":unknown()})
            }
            other => panic!("foreign snapshot must not use this operation: {other}"),
        }).collect::<Vec<_>>();
        Ok(json!({"facts":facts}))
    }
}
impl mcdev_companion::host_facts::bridge::FactReader for ForeignHost {
    fn read_facts(&mut self, evaluation: &str, queries: &[Value]) -> Result<Value, FactsError> {
        SemanticFactClient::read_facts(self, evaluation, queries)
    }
}
fn foreign_provider(
    class_literal: bool,
    scope_known: bool,
) -> (HostSemanticProvider, ForeignHost, String) {
    use mcdev_companion::host_facts::inputs::{InputOps, ScopedInputs};
    use mcdev_companion::wire::Fact;
    let value = if class_literal {
        "Target.class"
    } else {
        "\"fixture.Target\""
    };
    let attribute = if class_literal { "value" } else { "targets" };
    let mixin_text = format!("@Mixin({attribute}={value})");
    let source = format!("//😀\n{mixin_text} class OtherMixin {{ @Shadow private int value; }}");
    let mixin = annotation(
        &source,
        &mixin_text,
        "Mixin",
        "foreign:mixin",
        json!([
            {"name":attribute,"range":span(&source,value),"value":found(if class_literal {
                json!({"kind":"class","typeRef":"foreign:target"})
            } else {json!({"kind":"string","value":"fixture.Target","contentRange":span(&source,"fixture.Target")})})}
        ]),
    );
    let shadow = annotation(&source, "@Shadow", "Shadow", "foreign:shadow", json!([]));
    let class_start = source.find("@Mixin").unwrap();
    let class_body = source.find('{').unwrap();
    let field_start = source.find("@Shadow").unwrap();
    let field_end = source.find(';').unwrap() + 1;
    let field_name = source.rfind("value").unwrap();
    let field_range = json!({"start":field_name,"end":field_name+5});
    let declarations = json!([
        {"id":"foreign:class","owner":null,"name":"OtherMixin","range":span(&source,"OtherMixin"),"annotations":[mixin],
            "sourceKind":"Class","fullRange":{"start":class_start,"end":source.len()},
            "headerRange":{"start":class_start,"end":class_body},"nameRange":span(&source,"OtherMixin"),
            "bodyRange":{"start":class_body,"end":source.len()},"modifiers":modifiers("Package")},
        {"id":"foreign:field","owner":"foreign:class","name":"value","range":field_range,"annotations":[shadow],
            "sourceKind":"Property","fullRange":{"start":field_start,"end":field_end},
            "headerRange":{"start":field_start,"end":field_end},"nameRange":field_range,
            "bodyRange":null,"modifiers":modifiers("Private")}
    ]);
    let uri = mcdev_core::project::path_to_uri(
        &std::env::temp_dir().join("mcdev-foreign/OtherMixin.java"),
    );
    let descriptor = json!({"id":"source:foreign","displayPath":std::env::temp_dir().join("mcdev-foreign/OtherMixin.java").to_string_lossy(),
        "documentUri":found(json!(uri)),"languageId":"java","identity":{"kind":"source","revision":"22"}});
    let host = ForeignHost {
        descriptor,
        source,
        declarations,
        scope_known,
        calls: Arc::new(Mutex::new(Vec::new())),
        bad_context: Arc::new(std::sync::atomic::AtomicBool::new(false)),
    };
    let ops = InputOps::intersect(&json!({"capabilities":["source-snapshots-v1","source-facts-v2"],
        "supportedOps":{"source-snapshots-v1":["sourceInventory","sourceSnapshot"],"source-facts-v2":["sourceFacts"]}})).unwrap();
    let mut inputs = ScopedInputs::new("evaluation", &ops, host.clone()).unwrap();
    let entries = inputs.source_inventory().unwrap().entries;
    let snapshot = match inputs.source_snapshots(&entries).unwrap().remove(0) {
        Fact::Found { value } => value,
        _ => panic!(),
    };
    let evaluation = serde_json::from_value(
        json!({"evaluation":"evaluation","owner":"owner","source":"class Owner {}",
        "configuration":{},"declarations":[]}),
    )
    .unwrap();
    let provider = HostSemanticProvider::with_client(
        evaluation,
        Arc::new(host.clone()),
        mcdev_core::project::path_to_uri(&std::env::temp_dir().join("mcdev-foreign/Owner.java")),
        Some(1),
    )
    .unwrap()
    .with_source_facts(true)
    .with_project_sources(vec![(entries[0].clone(), snapshot)])
    .unwrap();
    (provider, host, uri)
}

#[test]
fn registered_foreign_source_uses_read_only_declaration_members_and_its_own_utf16_ranges() {
    let (provider, host, uri) = foreign_provider(false, true);
    let answer = provider
        .parse(&HostSemanticRequest {
            source: &host.source,
            source_uri: &uri,
            document_version: None,
            environment: &HostSemanticEnvironment::default(),
            project: None,
        })
        .unwrap();
    let HostFact::Known(result) = answer.fact else {
        panic!("expected foreign normalized facts: {:?}", answer.fact)
    };
    assert_eq!(result.availability, SemanticAvailability::Normalized);
    assert_eq!(result.model.source_uri, uri);
    assert_eq!(result.model.qualified_name, "fixture.OtherMixin");
    assert_eq!(
        result.model.members[0].return_descriptor.as_deref(),
        Some("I")
    );
    assert_eq!(
        result.model.members[0].name_range.start,
        offset_to_position(&host.source, host.source.find("value").unwrap()).unwrap()
    );
    assert_eq!(result.model.targets[0].internal_name, "fixture/Target");
}

#[test]
fn foreign_host_revision_is_never_an_editor_version_and_unknown_scope_is_not_empty_success() {
    let (provider, host, uri) = foreign_provider(false, true);
    let calls = host.calls.lock().unwrap().len();
    let answer = provider
        .parse(&HostSemanticRequest {
            source: &host.source,
            source_uri: &uri,
            document_version: Some(22),
            environment: &HostSemanticEnvironment::default(),
            project: None,
        })
        .unwrap();
    assert!(matches!(answer.fact, HostFact::Unavailable { .. }));
    assert_eq!(host.calls.lock().unwrap().len(), calls);
    let (provider, host, uri) = foreign_provider(false, false);
    let answer = provider
        .parse(&HostSemanticRequest {
            source: &host.source,
            source_uri: &uri,
            document_version: None,
            environment: &HostSemanticEnvironment::default(),
            project: None,
        })
        .unwrap();
    assert!(matches!(answer.fact, HostFact::Unavailable { .. }));
}

#[test]
fn foreign_class_literal_jvm_identity_is_not_guessed_when_snapshot_type_jvm_is_unsupported() {
    let (provider, host, uri) = foreign_provider(true, true);
    let answer = provider
        .parse(&HostSemanticRequest {
            source: &host.source,
            source_uri: &uri,
            document_version: None,
            environment: &HostSemanticEnvironment::default(),
            project: None,
        })
        .unwrap();
    assert!(
        matches!(answer.fact,HostFact::Unavailable{reason} if reason=="JVM class identity is unavailable")
    );
    assert!(host
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|query| query["op"] == "typeJvm" && query["target"] == "readonly:fixture.Target"));
}

#[test]
fn a_read_only_type_from_another_snapshot_is_rejected_before_member_projection() {
    let (provider, host, uri) = foreign_provider(false, true);
    host.bad_context
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let answer = provider
        .parse(&HostSemanticRequest {
            source: &host.source,
            source_uri: &uri,
            document_version: None,
            environment: &HostSemanticEnvironment::default(),
            project: None,
        })
        .unwrap();
    assert!(
        matches!(answer.fact,HostFact::Unavailable{reason} if reason=="semantic identity has a foreign source context")
    );
    assert!(!host
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|query| query["op"] == "declarationMember"));
}

struct BaseSources;
impl mcdev_core::mixin_facade::HostMixinSourceProvider for BaseSources {
    fn config(
        &self,
        _: &std::path::Path,
    ) -> mcdev_core::host_source::HostSourceResult<mcdev_core::project::MixinConfigRef> {
        Ok(mcdev_core::host_source::HostAnswer::unavailable(
            "base config",
        ))
    }
    fn read_source_file(
        &self,
        _: &std::path::Path,
    ) -> mcdev_core::host_source::HostSourceResult<String> {
        Ok(mcdev_core::host_source::HostAnswer::known(
            "base resource".into(),
            vec![],
        ))
    }
    fn compilation_unit_source(
        &self,
        _: &str,
        _: Option<&std::path::Path>,
    ) -> mcdev_core::host_source::HostSourceResult<mcdev_core::mixin_facade::RegisteredMixinSource>
    {
        panic!("registered source selection must use the host association")
    }
}

#[test]
fn registered_compilation_unit_source_is_wired_through_host_type_and_snapshot_identity() {
    use mcdev_companion::commands::ipc::semantic::RegisteredSourceProvider;
    use mcdev_core::mixin_facade::{HostMixinSourceProvider, RegisteredMixinSource};
    let (provider, host, uri) = foreign_provider(false, true);
    let provider = RegisteredSourceProvider::new(Arc::new(BaseSources), Arc::new(provider), true);
    let answer = provider
        .compilation_unit_source("fixture.OtherMixin", None)
        .unwrap();
    assert_eq!(
        answer.fact,
        HostFact::Known(RegisteredMixinSource::Source(host.source.clone()))
    );
    let path = mcdev_core::project::uri_to_path(&uri).unwrap();
    let answer = provider
        .compilation_unit_source("fixture.OtherMixin", Some(&path))
        .unwrap();
    assert_eq!(answer.fact, HostFact::Known(RegisteredMixinSource::Current));
    assert!(
        matches!(provider.config(&path).unwrap().fact,HostFact::Unavailable{reason} if reason=="base config")
    );
    assert_eq!(
        provider.read_source_file(&path).unwrap().fact,
        HostFact::Known("base resource".into())
    );
    assert!(host
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|query| query["op"] == "resolveType" && query["name"] == "fixture.OtherMixin"));
    assert!(host
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|query| query["op"] == "sourceLocation" && query["target"] == "selected:registered"));
}

#[test]
fn registered_source_unknown_scope_or_changed_snapshot_never_falls_back_to_a_guessed_path() {
    use mcdev_companion::commands::ipc::semantic::RegisteredSourceProvider;
    use mcdev_core::mixin_facade::HostMixinSourceProvider;
    for known in [true, false] {
        let (provider, host, _) = foreign_provider(false, known);
        if known {
            host.bad_context
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
        let provider =
            RegisteredSourceProvider::new(Arc::new(BaseSources), Arc::new(provider), true);
        assert!(matches!(
            provider
                .compilation_unit_source("fixture.OtherMixin", None)
                .unwrap()
                .fact,
            HostFact::Unavailable { .. }
        ));
    }
}

#[test]
fn same_fqn_in_another_captured_file_does_not_replace_the_host_selected_source() {
    use mcdev_companion::{
        commands::ipc::semantic::RegisteredSourceProvider,
        host_facts::inputs::{InputOps, ScopedInputs},
        wire::Fact,
    };
    use mcdev_core::mixin_facade::{HostMixinSourceProvider, RegisteredMixinSource};
    let (provider, host, _) = foreign_provider(false, true);
    let mut sibling = host.clone();
    sibling.source = sibling
        .source
        .replace("private int value", "private long value");
    sibling.descriptor["id"] = json!("source:sibling");
    sibling.descriptor["identity"]["revision"] = json!("23");
    let sibling_path = std::env::temp_dir().join("mcdev-foreign/test/OtherMixin.java");
    sibling.descriptor["displayPath"] = json!(sibling_path.to_string_lossy());
    sibling.descriptor["documentUri"] =
        found(json!(mcdev_core::project::path_to_uri(&sibling_path)));
    let ops = InputOps::intersect(&json!({"capabilities":["source-snapshots-v1"],
        "supportedOps":{"source-snapshots-v1":["sourceInventory","sourceSnapshot"]}}))
    .unwrap();
    let mut inputs = ScopedInputs::new("evaluation", &ops, sibling).unwrap();
    let entries = inputs.source_inventory().unwrap().entries;
    let snapshot = match inputs.source_snapshots(&entries).unwrap().remove(0) {
        Fact::Found { value } => value,
        _ => panic!(),
    };
    let provider = provider
        .with_project_sources(vec![(entries[0].clone(), snapshot)])
        .unwrap();
    let provider = RegisteredSourceProvider::new(Arc::new(BaseSources), Arc::new(provider), true);
    assert_eq!(
        provider
            .compilation_unit_source("fixture.OtherMixin", None)
            .unwrap()
            .fact,
        HostFact::Known(RegisteredMixinSource::Source(host.source))
    );
}
