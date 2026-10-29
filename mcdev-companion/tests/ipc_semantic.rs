use mcdev_companion::{
    commands::ipc::semantic::{HostSemanticProvider, SemanticFactClient},
    host_facts::FactsError,
    wire::Evaluation,
};
use mcdev_core::{
    host_source::{
        HostFact, HostSemanticEnvironment, HostSemanticModelProvider, HostSemanticRequest,
    },
    mixin_source::{JavaTypeKind, MixinMemberAnnotationKind, ParseSource},
    offset_to_position,
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

const DEFINITION_FIXTURE_SOURCE: &str = "package fixture;\nimport org.spongepowered.asm.mixin.Mixin;\nimport org.spongepowered.asm.mixin.injection.At;\nimport com.llamalad7.mixinextras.expression.Definition;\nimport com.llamalad7.mixinextras.expression.Expression;\nimport com.llamalad7.mixinextras.injector.ModifyExpressionValue;\n@Mixin(Target.class)\nabstract class ExampleMixin {\n    @Definition(id = \"drawCall\")\n    @ModifyExpressionValue(method = \"draw(Ljava/lang/String;FF)I\", at = @At(\"MIXINEXTRAS:EXPRESSION\"))\n    @Expression(\"drawCall\")\n    private int mcdev$handler(int original) { return original; }\n}\n";

#[test]
fn definition_fixture_cursor_inside_identifier_and_at_end_are_distinct() {
    use mcdev_core::bytecode::{
        BytecodeAtTargetAdapter, BytecodeClassIndexAdapter, ClassMemberIndex,
        InMemoryClassBytesProvider,
    };
    use mcdev_core::mixin_navigation::MixinDefinitionService;

    let source = DEFINITION_FIXTURE_SOURCE;
    let index = BytecodeClassIndexAdapter {
        member_index: ClassMemberIndex::default(),
    };
    let bytes = Arc::new(InMemoryClassBytesProvider::new(Default::default()).unwrap());
    let bytecode = BytecodeAtTargetAdapter::new(bytes);
    let service = MixinDefinitionService::new(&index, &bytecode);
    let start = source.rfind("drawCall").unwrap();
    let inside =
        service.definitions_at_offset(source, start + 2, None, Some("file:///ExampleMixin.java"));
    assert_eq!(inside.len(), 1);
    assert_eq!(
        inside[0].direct_source_document_uri.as_deref(),
        Some("file:///ExampleMixin.java")
    );
    let at_end = service.definitions_at_offset(
        source,
        start + "drawCall".len(),
        None,
        Some("file:///ExampleMixin.java"),
    );
    assert!(at_end.is_empty());
}

struct Host {
    active: AtomicBool,
    reads: AtomicUsize,
    unknown_target: bool,
    custom_mixin: bool,
    close_after_first: bool,
}
impl Host {
    fn new() -> Self {
        Self {
            active: AtomicBool::new(true),
            reads: AtomicUsize::new(0),
            unknown_target: false,
            custom_mixin: false,
            close_after_first: false,
        }
    }
}
impl SemanticFactClient for Host {
    fn check_active(&self) -> Result<(), FactsError> {
        if self.active.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(FactsError::Cancelled("closed".into()))
        }
    }
    fn read_facts(&self, evaluation: &str, queries: &[Value]) -> Result<Value, FactsError> {
        assert_eq!(evaluation, "eval-1");
        self.reads.fetch_add(1, Ordering::SeqCst);
        if self.close_after_first {
            self.active.store(false, Ordering::SeqCst);
        }
        let facts = queries.iter().map(|query| match query["op"].as_str().unwrap() {
            "resolveTypeRef" => {
                let id = query["reference"].as_str().unwrap();
                let (name, qualified) = match id {
                    "ref-mixin" if self.custom_mixin => ("type-mixin", "custom.Mixin"),
                    "ref-mixin" => ("type-mixin", "org.spongepowered.asm.mixin.Mixin"),
                    "ref-shadow" => ("type-shadow", "org.spongepowered.asm.mixin.Shadow"),
                    "ref-inject" => ("type-inject", "org.spongepowered.asm.mixin.injection.Inject"),
                    "ref-other" => ("type-other", "fixture.Other"),
                    "ref-definition" => ("type-definition", "com.llamalad7.mixinextras.expression.Definition"),
                    "ref-expression" => ("type-expression", "com.llamalad7.mixinextras.expression.Expression"),
                    "ref-modify" => ("type-modify", "com.llamalad7.mixinextras.injector.ModifyExpressionValue"),
                    "ref-mapping" => ("type-mapping", "io.github.diskria.lapis.annotations.MappingName"),
                    "ref-target" => ("type-target", "example.Outer.Target"),
                    other => panic!("unexpected reference {other}"),
                };
                json!({"state":"found","value":{"id":name,"qualifiedName":qualified,"package":"example"}})
            }
            "declarationType" => json!({"state":"found","value":{"id":"type-owner","qualifiedName":"example.雪Mixin","package":"example"}}),
            "declarationMember" if query["declaration"] == "method" => json!({"state":"found","value":{
                "id":"member-hook","name":"hook","kind":"method",
                "signature":{"state":"found","value":{"id":"signature-hook","kind":"method","descriptor":"(I)V"}},
                "dispatch":{"state":"found","value":"instance"},
                "visibility":{"state":"found","value":"Private"},
                "synthetic":{"state":"found","value":false},"mismatches":[]
            }}),
            "declarationMember" => json!({"state":"found","value":{
                "id":"member-value","name":"value","kind":"field",
                "signature":{"state":"found","value":{"id":"signature-value","kind":"field","descriptor":"I"}},
                "dispatch":{"state":"found","value":"instance"},
                "visibility":{"state":"found","value":"Private"},
                "synthetic":{"state":"found","value":false},"mismatches":[]
            }}),
            "typeJvm" => {
                assert_eq!(query["target"], "type-target");
                let binary = if self.unknown_target { json!({"state":"unknown","reasons":[{"code":"MissingFacts"}]}) }
                    else { json!({"state":"found","value":{"internalName":"example/Outer$Target","descriptor":"Lexample/Outer$Target;"}}) };
                let unknown = json!({"state":"unknown","reasons":[{"code":"MissingFacts"}]});
                json!({"binaryName":binary,"classFlags":unknown,"declaredMembers":{"members":[],"coverage":unknown},"abi":unknown,"jvmAbiCoverage":unknown})
            }
            other => panic!("unexpected operation {other}"),
        }).collect::<Vec<_>>();
        Ok(json!({"facts":facts}))
    }
}

fn span(source: &str, text: &str) -> Value {
    let start = source.find(text).unwrap();
    json!({"start":start,"end":start+text.len()})
}
fn fixture() -> (String, Value) {
    let source =
        "//🙂\n@Mixin(Target.class)\nclass 雪Mixin { @Shadow private int value; }\n".to_string();
    let wire = json!({
        "evaluation":"eval-1", "owner":"owned-source", "source":source, "configuration":{},
        "declarations":[
            {"id":"class", "owner":null, "name":"雪Mixin", "range":span(&source,"雪Mixin"),
             "annotations":[{"typeRef":"ref-mixin","useSiteTarget":null,"range":span(&source,"@Mixin(Target.class)"),
                "arguments":[{"name":null,"range":span(&source,"Target.class"),"value":{"state":"found","value":{"kind":"class","typeRef":"ref-target"}}}]}]},
            {"id":"field", "owner":"class", "name":"value", "range":span(&source,"value"),
             "annotations":[{"typeRef":"ref-shadow","useSiteTarget":null,"range":span(&source,"@Shadow"),"arguments":[]}]}
        ]
    });
    (source, wire)
}
fn provider(wire: Value, host: Arc<Host>) -> HostSemanticProvider {
    HostSemanticProvider::with_client(
        serde_json::from_value::<Evaluation>(wire).unwrap(),
        host,
        "file:///project/雪Mixin.java".into(),
        Some(7),
    )
    .unwrap()
}
fn parse(
    provider: &HostSemanticProvider,
    source: &str,
    version: Option<i64>,
) -> mcdev_core::host_source::HostSourceResult<mcdev_core::host_source::HostSemanticModel> {
    provider.parse(&HostSemanticRequest {
        source,
        source_uri: "file:///project/雪Mixin.java",
        document_version: version,
        environment: &HostSemanticEnvironment::default(),
        project: None,
    })
}

#[test]
fn host_identity_descriptor_and_utf16_ranges_reach_real_core_model() {
    let (source, wire) = fixture();
    let host = Arc::new(Host::new());
    let provider = provider(wire, host.clone());
    let answer = parse(&provider, &source, Some(7)).unwrap();
    let HostFact::Known(model) = answer.fact else {
        panic!("expected normalized facts")
    };
    assert_eq!(model.model.parse_source, ParseSource::HandWrittenFallback);
    assert_eq!(
        model.availability,
        mcdev_core::host_source::SemanticAvailability::StableFallback
    );
    assert!(model.model.debug_info.fallback_reason.is_some());
    assert_eq!(model.model.type_kind, JavaTypeKind::Unknown);
    assert_eq!(model.model.targets.len(), 1);
    assert_eq!(model.model.targets[0].internal_name, "example/Outer$Target");
    assert_eq!(model.model.members.len(), 1);
    let member = &model.model.members[0];
    assert_eq!(member.annotation_kind, MixinMemberAnnotationKind::Shadow);
    assert_eq!(member.java_name, "value");
    assert_eq!(member.return_descriptor.as_deref(), Some("I"));
    assert_eq!(
        member.name_range.start,
        offset_to_position(&source, source.find("value").unwrap()).unwrap()
    );
    assert!(host.reads.load(Ordering::SeqCst) > 0);
}

#[test]
fn unknown_binary_identity_is_never_guessed_from_source_qualified_name() {
    let (source, wire) = fixture();
    let mut host = Host::new();
    host.unknown_target = true;
    let answer = parse(&provider(wire, Arc::new(host)), &source, Some(7)).unwrap();
    assert!(matches!(answer.fact, HostFact::Unavailable { .. }));
}

#[test]
fn a_foreign_same_spelling_annotation_is_not_treated_as_mixin() {
    let (source, wire) = fixture();
    let mut host = Host::new();
    host.custom_mixin = true;
    let answer = parse(&provider(wire, Arc::new(host)), &source, Some(7)).unwrap();
    assert!(matches!(answer.fact, HostFact::Unavailable { .. }));
}

#[test]
fn unknown_nested_at_stays_unavailable_instead_of_becoming_an_empty_selector() {
    let source = "@Mixin(Target.class) class 雪Mixin { @Inject(method=\"tick\", at=@At(value=\"HEAD\")) void hook() {} }".to_string();
    let wire = json!({"evaluation":"eval-1","owner":"owned-source","source":source,"configuration":{},"declarations":[
    {"id":"class","owner":null,"name":"雪Mixin","range":span(&source,"雪Mixin"),"annotations":[
        {"typeRef":"ref-mixin","useSiteTarget":null,"range":span(&source,"@Mixin(Target.class)"),"arguments":[
            {"name":null,"range":span(&source,"Target.class"),"value":{"state":"found","value":{"kind":"class","typeRef":"ref-target"}}}]}]},
    {"id":"method","owner":"class","name":"hook","range":span(&source,"hook"),"annotations":[
        {"typeRef":"ref-inject","useSiteTarget":null,"range":span(&source,"@Inject(method=\"tick\", at=@At(value=\"HEAD\"))"),"arguments":[
            {"name":"method","range":span(&source,"\"tick\""),"value":{"state":"found","value":{"kind":"string","value":"tick","contentRange":span(&source,"tick")}}},
            {"name":"at","range":span(&source,"@At(value=\"HEAD\")"),"value":{"state":"unknown","reasons":[{"code":"UnsupportedSource"}]}}
        ]}]}]});
    let answer = parse(&provider(wire, Arc::new(Host::new())), &source, Some(7)).unwrap();
    assert!(
        matches!(answer.fact, HostFact::Unavailable { reason } if reason == "annotation argument is unavailable")
    );
}

#[test]
fn nested_annotation_in_an_unrelated_annotation_is_not_a_missing_declaration() {
    let source = "@Other(value=@At(\"HEAD\")) @Mixin(Target.class) class 雪Mixin { @Shadow private int value; }".to_string();
    let value_start = source.rfind("value").unwrap();
    let wire = json!({"evaluation":"eval-1","owner":"owned-source","source":source,"configuration":{},"declarations":[
    {"id":"class","owner":null,"name":"雪Mixin","range":span(&source,"雪Mixin"),"annotations":[
        {"typeRef":"ref-other","useSiteTarget":null,"range":span(&source,"@Other(value=@At(\"HEAD\"))"),"arguments":[
            {"name":"value","range":span(&source,"@At(\"HEAD\")"),"value":{"state":"unknown","reasons":[{"code":"UnsupportedSource"}]}}]},
        {"typeRef":"ref-mixin","useSiteTarget":null,"range":span(&source,"@Mixin(Target.class)"),"arguments":[
            {"name":null,"range":span(&source,"Target.class"),"value":{"state":"found","value":{"kind":"class","typeRef":"ref-target"}}}]}]},
    {"id":"field","owner":"class","name":"value","range":{"start":value_start,"end":value_start+5},"annotations":[
        {"typeRef":"ref-shadow","useSiteTarget":null,"range":span(&source,"@Shadow"),"arguments":[]}
    ]}]});
    let answer = parse(&provider(wire, Arc::new(Host::new())), &source, Some(7)).unwrap();
    let HostFact::Known(model) = answer.fact else {
        panic!("unrelated nested annotation blocked the Mixin model")
    };
    assert_eq!(model.model.members[0].java_name, "value");
    assert_eq!(model.model.targets[0].internal_name, "example/Outer$Target");
}

#[test]
fn oversized_outer_range_does_not_hide_an_omitted_top_level_annotation() {
    let source = "@Other(\"x\") @Mixin(Target.class) class 雪Mixin { @Shadow private int value; }"
        .to_string();
    let shadow_end = source.find("@Shadow").unwrap() + "@Shadow".len();
    let wire = json!({"evaluation":"eval-1","owner":"owned-source","source":source,"configuration":{},"declarations":[
    {"id":"class","owner":null,"name":"雪Mixin","range":span(&source,"雪Mixin"),"annotations":[
        {"typeRef":"ref-other","useSiteTarget":null,"range":{"start":0,"end":shadow_end},"arguments":[
            {"name":null,"range":span(&source,"\"x\""),"value":{"state":"found","value":{"kind":"string","value":"x","contentRange":span(&source,"x")}}}]},
        {"typeRef":"ref-mixin","useSiteTarget":null,"range":span(&source,"@Mixin(Target.class)"),"arguments":[
            {"name":null,"range":span(&source,"Target.class"),"value":{"state":"found","value":{"kind":"class","typeRef":"ref-target"}}}]}
    ]}]});
    let host = Arc::new(Host::new());
    let answer = parse(&provider(wire, host.clone()), &source, Some(7)).unwrap();
    assert!(
        matches!(answer.fact, HostFact::Unavailable { reason } if reason == "host annotation inventory is incomplete")
    );
    assert_eq!(host.reads.load(Ordering::SeqCst), 0);
}

#[test]
fn definition_fixture_keeps_unknown_nested_at_unavailable_after_shape_check() {
    let source = DEFINITION_FIXTURE_SOURCE.to_string();
    let expression_start = source.rfind("\"drawCall\"").unwrap();
    let expression_range =
        json!({"start":expression_start,"end":expression_start+"\"drawCall\"".len()});
    let expression_content =
        json!({"start":expression_start+1,"end":expression_start+1+"drawCall".len()});
    let wire = json!({"evaluation":"eval-1","owner":"owned-source","source":source,"configuration":{},"declarations":[
    {"id":"class","owner":null,"name":"ExampleMixin","range":span(&source,"ExampleMixin"),"annotations":[
        {"typeRef":"ref-mixin","useSiteTarget":null,"range":span(&source,"@Mixin(Target.class)"),"arguments":[
            {"name":null,"range":span(&source,"Target.class"),"value":{"state":"found","value":{"kind":"class","typeRef":"ref-target"}}}]}]},
    {"id":"method","owner":"class","name":"mcdev$handler","range":span(&source,"mcdev$handler"),"annotations":[
        {"typeRef":"ref-definition","useSiteTarget":null,"range":span(&source,"@Definition(id = \"drawCall\")"),"arguments":[
            {"name":"id","range":span(&source,"\"drawCall\""),"value":{"state":"found","value":{"kind":"string","value":"drawCall","contentRange":span(&source,"drawCall")}}}]},
        {"typeRef":"ref-modify","useSiteTarget":null,"range":span(&source,"@ModifyExpressionValue(method = \"draw(Ljava/lang/String;FF)I\", at = @At(\"MIXINEXTRAS:EXPRESSION\"))"),"arguments":[
            {"name":"method","range":span(&source,"\"draw(Ljava/lang/String;FF)I\""),"value":{"state":"found","value":{"kind":"string","value":"draw(Ljava/lang/String;FF)I","contentRange":span(&source,"draw(Ljava/lang/String;FF)I")}}},
            {"name":"at","range":span(&source,"@At(\"MIXINEXTRAS:EXPRESSION\")"),"value":{"state":"unknown","reasons":[{"code":"UnsupportedSource"}]}}]},
        {"typeRef":"ref-expression","useSiteTarget":null,"range":span(&source,"@Expression(\"drawCall\")"),"arguments":[
            {"name":null,"range":expression_range,"value":{"state":"found","value":{"kind":"string","value":"drawCall","contentRange":expression_content}}}]}
    ]}]});
    let host = Arc::new(Host::new());
    let answer = parse(&provider(wire, host.clone()), &source, Some(7)).unwrap();
    assert!(
        matches!(answer.fact, HostFact::Unavailable { reason } if reason == "annotation argument is unavailable")
    );
    assert!(host.reads.load(Ordering::SeqCst) > 0);
}

#[test]
fn stale_source_version_and_closed_evaluation_never_query_host() {
    let (source, wire) = fixture();
    let host = Arc::new(Host::new());
    let provider = provider(wire, host.clone());
    assert!(matches!(
        parse(&provider, &source, Some(8)).unwrap().fact,
        HostFact::Unavailable { .. }
    ));
    assert!(matches!(
        parse(&provider, &(source.clone() + " "), Some(7))
            .unwrap()
            .fact,
        HostFact::Unavailable { .. }
    ));
    assert_eq!(host.reads.load(Ordering::SeqCst), 0);
    host.active.store(false, Ordering::SeqCst);
    assert!(matches!(
        parse(&provider, &source, Some(7)),
        Err(mcdev_core::host_source::HostSourceError::Cancelled(_))
    ));
    assert_eq!(host.reads.load(Ordering::SeqCst), 0);
}

#[test]
fn kotlin_field_use_site_preserves_host_binding_and_current_utf16_range() {
    let source =
        "//🙂\n@Mixin(Target::class)\nclass 雪Mixin { @field:Shadow private var value: Int = 0 }\n"
            .to_string();
    let uri = "file:///project/雪Mixin.kt";
    let wire = json!({"evaluation":"eval-1","owner":"owned-source","source":source,"configuration":{},"declarations":[
    {"id":"class","owner":null,"name":"雪Mixin","range":span(&source,"雪Mixin"),"annotations":[
        {"typeRef":"ref-mixin","useSiteTarget":null,"range":span(&source,"@Mixin(Target::class)"),"arguments":[
            {"name":null,"range":span(&source,"Target::class"),"value":{"state":"found","value":{"kind":"class","typeRef":"ref-target"}}}]}]},
    {"id":"field","owner":"class","name":"value","range":span(&source,"value"),"annotations":[
        {"typeRef":"ref-shadow","useSiteTarget":"field","range":span(&source,"@field:Shadow"),"arguments":[]}
    ]}]});
    let host = Arc::new(Host::new());
    let provider = HostSemanticProvider::with_client(
        serde_json::from_value::<Evaluation>(wire).unwrap(),
        host,
        uri.into(),
        Some(7),
    )
    .unwrap();
    let answer = provider
        .parse(&HostSemanticRequest {
            source: &source,
            source_uri: uri,
            document_version: Some(7),
            environment: &HostSemanticEnvironment::default(),
            project: None,
        })
        .unwrap();
    let model = match answer.fact {
        HostFact::Known(model) => model,
        other => panic!("expected Kotlin field binding: {other:?}"),
    };
    assert_eq!(model.model.targets[0].internal_name, "example/Outer$Target");
    assert_eq!(model.model.members[0].java_name, "value");
    assert_eq!(
        model.model.members[0].return_descriptor.as_deref(),
        Some("I")
    );
    assert_eq!(
        model.model.members[0].name_range.start,
        offset_to_position(&source, source.find("value").unwrap()).unwrap()
    );
}

#[test]
fn target_array_unicode_and_mapping_name_keep_host_values_and_ranges() {
    let source = "@Mixin(targets={\"pkg.雪\",\"pkg.B\"}, remap=false) class 雪Mixin { @MappingName(\"renamed\") @Shadow private int value; }".to_string();
    let wire = json!({"evaluation":"eval-1","owner":"owned-source","source":source,"configuration":{},"declarations":[
    {"id":"class","owner":null,"name":"雪Mixin","range":span(&source,"雪Mixin"),"annotations":[
        {"typeRef":"ref-mixin","useSiteTarget":null,"range":span(&source,"@Mixin(targets={\"pkg.雪\",\"pkg.B\"}, remap=false)"),"arguments":[
            {"name":"targets","range":span(&source,"{\"pkg.雪\",\"pkg.B\"}"),"value":{"state":"found","value":{"kind":"array","values":[
                {"name":null,"range":span(&source,"\"pkg.雪\""),"value":{"state":"found","value":{"kind":"string","value":"pkg.雪","contentRange":span(&source,"pkg.雪")}}},
                {"name":null,"range":span(&source,"\"pkg.B\""),"value":{"state":"found","value":{"kind":"string","value":"pkg.B","contentRange":span(&source,"pkg.B")}}}
            ]}}},
            {"name":"remap","range":span(&source,"false"),"value":{"state":"found","value":{"kind":"boolean","value":false}}}
        ]}]},
    {"id":"field","owner":"class","name":"value","range":span(&source,"value"),"annotations":[
        {"typeRef":"ref-mapping","useSiteTarget":null,"range":span(&source,"@MappingName(\"renamed\")"),"arguments":[
            {"name":null,"range":span(&source,"\"renamed\""),"value":{"state":"found","value":{"kind":"string","value":"renamed","contentRange":span(&source,"renamed")}}}]},
        {"typeRef":"ref-shadow","useSiteTarget":null,"range":span(&source,"@Shadow"),"arguments":[]}
    ]}]});
    let answer = parse(&provider(wire, Arc::new(Host::new())), &source, Some(7)).unwrap();
    let HostFact::Known(model) = answer.fact else {
        panic!("expected normalized target array")
    };
    assert_eq!(
        model
            .model
            .targets
            .iter()
            .map(|target| target.internal_name.as_str())
            .collect::<Vec<_>>(),
        ["pkg/雪", "pkg/B"]
    );
    assert_eq!(
        model.model.targets[0].range.start,
        offset_to_position(&source, source.find("pkg.雪").unwrap()).unwrap()
    );
    assert_eq!(
        model.model.members[0].explicit_target_name.as_deref(),
        Some("renamed")
    );
}

#[test]
fn authored_injector_descriptor_comes_from_the_host_member() {
    let source = "@Mixin(Target.class) class 雪Mixin { @Inject(method=\"tick\") private void hook(int n) {} }".to_string();
    let wire = json!({"evaluation":"eval-1","owner":"owned-source","source":source,"configuration":{},"declarations":[
        {"id":"class","owner":null,"name":"雪Mixin","range":span(&source,"雪Mixin"),"annotations":[
            {"typeRef":"ref-mixin","useSiteTarget":null,"range":span(&source,"@Mixin(Target.class)"),"arguments":[
                {"name":null,"range":span(&source,"Target.class"),"value":{"state":"found","value":{"kind":"class","typeRef":"ref-target"}}}]}]},
        {"id":"method","owner":"class","name":"hook","range":span(&source,"hook"),"annotations":[
            {"typeRef":"ref-inject","useSiteTarget":null,"range":span(&source,"@Inject(method=\"tick\")"),"arguments":[
                {"name":"method","range":span(&source,"\"tick\""),"value":{"state":"found","value":{"kind":"string","value":"tick","contentRange":span(&source,"tick")}}}]}]}]});
    let answer = parse(&provider(wire, Arc::new(Host::new())), &source, Some(7)).unwrap();
    let HostFact::Known(model) = answer.fact else {
        panic!("expected normalized injector")
    };
    assert_eq!(model.model.injectors.len(), 1);
    assert_eq!(model.model.injectors[0].method_selectors[0].value, "tick");
    assert_eq!(
        model.model.injectors[0]
            .handler_method_descriptor
            .as_deref(),
        Some("(I)V")
    );
}

#[test]
fn evaluation_cancelled_between_fact_batches_stops_projection() {
    let (source, wire) = fixture();
    let mut host = Host::new();
    host.close_after_first = true;
    let host = Arc::new(host);
    let provider = provider(wire, host.clone());
    assert!(matches!(
        parse(&provider, &source, Some(7)),
        Err(mcdev_core::host_source::HostSourceError::Cancelled(_))
    ));
    assert_eq!(host.reads.load(Ordering::SeqCst), 1);
}

#[test]
fn host_argument_range_outside_its_annotation_is_rejected() {
    let (source, mut wire) = fixture();
    wire["declarations"][0]["annotations"][0]["arguments"][0]["range"] = span(&source, "value");
    let answer = parse(&provider(wire, Arc::new(Host::new())), &source, Some(7)).unwrap();
    assert!(matches!(answer.fact, HostFact::Unavailable { .. }));
}

#[test]
fn omitted_host_annotation_is_not_turned_into_successful_empty_members() {
    let (source, mut wire) = fixture();
    wire["declarations"].as_array_mut().unwrap().pop();
    let answer = parse(&provider(wire, Arc::new(Host::new())), &source, Some(7)).unwrap();
    assert!(matches!(answer.fact, HostFact::Unavailable { .. }));
}
