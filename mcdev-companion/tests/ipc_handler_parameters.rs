use mcdev_companion::{
    commands::ipc::semantic::{HostSemanticProvider, SemanticFactClient},
    host_facts::FactsError,
    wire::Evaluation,
};
use mcdev_core::{
    extras_model::*,
    host_source::{
        HostFact, HostSemanticEnvironment, HostSemanticModelProvider, HostSemanticRequest,
    },
    mixin_source::{AnnotationContext, AnnotationSlot, MixinAnnotation},
    mixinextras_handler::HandlerSignatureService,
    mixinextras_locals::{LocalCaptureTypeResolver, ShareCompletionService},
    model::*,
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

const CI: &str = "Lorg/spongepowered/asm/mixin/injection/callback/CallbackInfo;";
const INT_REF: &str = "Lcom/llamalad7/mixinextras/sugar/ref/LocalIntRef;";
#[path = "support/ipc_handler_selectors.rs"]
mod selectors;
#[path = "support/ipc_handler_share.rs"]
mod share;
fn found(value: Value) -> Value {
    json!({"state":"found","value":value})
}
fn unknown() -> Value {
    json!({"state":"unknown","reasons":[{"code":"UnsupportedType"}]})
}
fn span(source: &str, text: &str) -> Value {
    let start = source.find(text).unwrap();
    json!({"start":start,"end":start+text.len()})
}
fn annotation(source: &str, text: &str, name: &str, reference: &str, args: Value) -> Value {
    let start = source.find(text).unwrap() + text.find(name).unwrap();
    json!({"typeRef":reference,"range":span(source,text),"nameRange":{"start":start,"end":start+name.len()},"arguments":args})
}
fn scalar(source: &str, name: &str, text: &str, value: Value) -> Value {
    json!({"name":name,"range":span(source,text),"value":found(value)})
}
fn string_value(source: &str, value: &str) -> Value {
    json!({"kind":"string","value":value,"contentRange":span(source,value)})
}
fn modifiers(final_known: bool) -> Value {
    json!({"visibility":found(json!("Private")),"static":found(json!(false)),
    "final":if final_known {found(json!(false))} else {unknown()},"abstract":found(json!(false))})
}

#[derive(Clone)]
struct Host {
    source: String,
    declarations: Value,
    signature: String,
    active: Arc<AtomicBool>,
    calls: Arc<Mutex<Vec<Value>>>,
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
        assert_eq!(evaluation, "eval");
        self.calls.lock().unwrap().extend_from_slice(queries);
        let facts=queries.iter().map(|query|match query["op"].as_str().unwrap() {
            "sourceFacts"=>json!({"declarations":self.declarations}),
            "resolveTypeRef"=>{
                let reference=query["reference"].as_str().unwrap();
                if reference.starts_with("parameter:"){
                    if reference=="parameter:2"&&self.signature.contains("/LocalRef;") {
                        return found(json!({"id":"nominal:localref","qualifiedName":"com.llamalad7.mixinextras.sugar.ref.LocalRef","package":"com.llamalad7.mixinextras.sugar.ref"}));
                    }
                    return unknown();
                }
                let fqn=match reference {
                    "mixin"=>"org.spongepowered.asm.mixin.Mixin","inject"=>"org.spongepowered.asm.mixin.injection.Inject",
                    "at"=>"org.spongepowered.asm.mixin.injection.At","target"=>"fixture.Target",
                    "local"=>"com.llamalad7.mixinextras.sugar.Local","share"=>"com.llamalad7.mixinextras.sugar.Share",
                    "cancellable"=>"com.llamalad7.mixinextras.sugar.Cancellable","coerce"=>"org.spongepowered.asm.mixin.injection.Coerce",
                    "modifyconstant"=>"org.spongepowered.asm.mixin.injection.ModifyConstant",
                    "wrapoperation"=>"com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation",
                    "constant"=>"org.spongepowered.asm.mixin.injection.Constant",
                    "slice"=>"org.spongepowered.asm.mixin.injection.Slice",
                    "condition"=>"org.spongepowered.asm.mixin.injection.Constant.Condition",
                    other=>panic!("unexpected reference {other}"),
                };
                found(json!({"id":format!("type:{reference}"),"qualifiedName":fqn,"package":"fixture"}))
            }
            "declarationType"=>found(json!({"id":"type:owner","qualifiedName":"fixture.雪Mixin","package":"fixture"})),
            "declarationMember"=>found(json!({"id":"member:hook","name":"hook","kind":"method",
                "signature":found(json!({"id":"signature:hook","kind":"method","descriptor":self.signature})),
                "dispatch":found(json!("instance")),"visibility":found(json!("Private")),"synthetic":found(json!(false)),"mismatches":[]})),
            "typeJvm"=>{
                let internal=match query["target"].as_str().unwrap(){"type:target"=>"fixture/Target","type:condition"=>"org/spongepowered/asm/mixin/injection/Constant$Condition",other=>panic!("unexpected JVM type {other}")};
                json!({"binaryName":found(json!({"internalName":internal,"descriptor":format!("L{internal};")})),
                    "classFlags":unknown(),"declaredMembers":{"members":[],"coverage":unknown()},"abi":unknown(),"jvmAbiCoverage":unknown()})
            }
            other=>panic!("unexpected query {other}"),
        }).collect::<Vec<_>>();
        Ok(json!({"facts":facts}))
    }
}
fn fixture(kotlin: bool) -> Host {
    let coerce = if kotlin { "@Force" } else { "@Coerce" };
    let local = if kotlin {
        "@Keep(argsOnly=true, index=1, ordinal=0, name=[\"value\"], print=false)"
    } else {
        "@Local(argsOnly=true, index=1, ordinal=0, name={\"value\"}, print=false)"
    };
    let fragments = if kotlin {
        vec![
            format!("{coerce} callback: CallbackInfo"),
            format!("{local} `when`: Int"),
            "@Share(\"state\") state: LocalIntRef".into(),
            "@Cancellable ci: CallbackInfo".into(),
        ]
    } else {
        vec![
            format!("{coerce} CallbackInfo callback"),
            format!("{local} int value"),
            "@Share(\"state\") LocalIntRef state".into(),
            "@Cancellable CallbackInfo ci".into(),
        ]
    };
    let method = if kotlin {
        "@Inject(method=\"tick()V\", at=At(\"HEAD\"))"
    } else {
        "@Inject(method=\"tick()V\", at=@At(\"HEAD\"))"
    };
    let mixin = if kotlin {
        "@Mixin(Target::class)"
    } else {
        "@Mixin(Target.class)"
    };
    let source = if kotlin {
        format!("//😀\nimport com.llamalad7.mixinextras.sugar.Local as Keep\nimport org.spongepowered.asm.mixin.injection.Coerce as Force\n{mixin} class 雪Mixin {{\n {method}\n private fun hook({}) {{}}\n}}",fragments.join(", "))
    } else {
        format!(
            "//😀\n{mixin} class 雪Mixin {{\n {method}\n private void hook({}) {{}}\n}}",
            fragments.join(", ")
        )
    };
    let at_text = if kotlin {
        "At(\"HEAD\")"
    } else {
        "@At(\"HEAD\")"
    };
    let at = annotation(
        &source,
        at_text,
        "At",
        "at",
        json!([scalar(
            &source,
            "value",
            "\"HEAD\"",
            string_value(&source, "HEAD")
        )]),
    );
    let inject = annotation(
        &source,
        method,
        "Inject",
        "inject",
        json!([
            scalar(
                &source,
                "method",
                "\"tick()V\"",
                string_value(&source, "tick()V")
            ),
            scalar(
                &source,
                "at",
                at_text,
                json!({"kind":"annotation","annotation":at})
            )
        ]),
    );
    let mixin = annotation(
        &source,
        mixin,
        "Mixin",
        "mixin",
        json!([scalar(
            &source,
            "value",
            if kotlin {
                "Target::class"
            } else {
                "Target.class"
            },
            json!({"kind":"class","typeRef":"target"})
        )]),
    );
    let mut values = Vec::new();
    let names = if kotlin {
        vec!["callback", "when", "state", "ci"]
    } else {
        vec!["callback", "value", "state", "ci"]
    };
    for (index, fragment) in fragments.iter().enumerate() {
        let name = names[index];
        let spelling = if kotlin && index == 1 { "`when`" } else { name };
        let start = source.find(fragment).unwrap() + fragment.rfind(spelling).unwrap();
        let annotations = match index {
            0 => json!([annotation(
                &source,
                coerce,
                coerce.trim_start_matches('@'),
                "coerce",
                json!([])
            )]),
            1 => {
                let array = if kotlin { "[\"value\"]" } else { "{\"value\"}" };
                json!([annotation(
                    &source,
                    local,
                    if kotlin { "Keep" } else { "Local" },
                    "local",
                    json!([
                        scalar(
                            &source,
                            "argsOnly",
                            "true",
                            json!({"kind":"boolean","value":true})
                        ),
                        scalar(
                            &source,
                            "index",
                            "1",
                            json!({"kind":"integer","type":"int","value":1})
                        ),
                        scalar(
                            &source,
                            "ordinal",
                            "0",
                            json!({"kind":"integer","type":"int","value":0})
                        ),
                        scalar(
                            &source,
                            "name",
                            array,
                            json!({"kind":"array","values":[{"name":null,"range":span(&source,"\"value\""),"value":found(string_value(&source,"value"))}]})
                        ),
                        scalar(
                            &source,
                            "print",
                            "false",
                            json!({"kind":"boolean","value":false})
                        )
                    ])
                )])
            }
            2 => json!([annotation(
                &source,
                "@Share(\"state\")",
                "Share",
                "share",
                json!([scalar(
                    &source,
                    "value",
                    "\"state\"",
                    string_value(&source, "state")
                )])
            )]),
            _ => json!([annotation(
                &source,
                "@Cancellable",
                "Cancellable",
                "cancellable",
                json!([])
            )]),
        };
        values.push(json!({"name":name,"range":span(&source,fragment),"nameRange":{"start":start,"end":start+spelling.len()},
            "typeRef":format!("parameter:{index}"),"annotations":annotations}));
    }
    let class_start = source.find("@Mixin").unwrap();
    let class_body = source.find('{').unwrap();
    let method_start = source.find("@Inject").unwrap();
    let method_body = source.rfind("{}").unwrap();
    let declarations = json!([
        {"id":"class","owner":null,"name":"雪Mixin","range":span(&source,"雪Mixin"),"annotations":[mixin],"sourceKind":"Class",
            "fullRange":{"start":class_start,"end":source.len()},"headerRange":{"start":class_start,"end":class_body},
            "nameRange":span(&source,"雪Mixin"),"bodyRange":{"start":class_body,"end":source.len()},"modifiers":modifiers(true)},
        {"id":"method","owner":"class","name":"hook","range":span(&source,"hook"),"annotations":[inject],"sourceKind":"Function",
            "fullRange":{"start":method_start,"end":method_body+2},"headerRange":{"start":method_start,"end":method_body},
            "nameRange":span(&source,"hook"),"bodyRange":{"start":method_body,"end":method_body+2},"modifiers":modifiers(!kotlin),
            "parameters":found(json!(values))}
    ]);
    Host {
        source,
        declarations,
        signature: format!("({CI}I{INT_REF}{CI})V"),
        active: Arc::new(AtomicBool::new(true)),
        calls: Arc::new(Mutex::new(Vec::new())),
    }
}
fn answer(host: &Host, kotlin: bool) -> HostFact<mcdev_core::host_source::HostSemanticModel> {
    let uri = if kotlin {
        "file:///project/雪Mixin.kt"
    } else {
        "file:///project/雪Mixin.java"
    };
    let evaluation:Evaluation=serde_json::from_value(json!({"evaluation":"eval","owner":"owner","source":host.source,"configuration":{},"declarations":[]})).unwrap();
    let provider =
        HostSemanticProvider::with_client(evaluation, Arc::new(host.clone()), uri.into(), Some(5))
            .unwrap()
            .with_source_facts(true);
    let answer = provider
        .parse(&HostSemanticRequest {
            source: &host.source,
            source_uri: uri,
            document_version: Some(5),
            environment: &HostSemanticEnvironment::default(),
            project: None,
        })
        .unwrap();
    answer.fact
}
fn parse(host: &Host, kotlin: bool) -> mcdev_core::host_source::HostSemanticModel {
    let answer = answer(host, kotlin);
    let HostFact::Known(model) = answer else {
        panic!("expected normalized source: {answer:?}")
    };
    model
}
struct Index;
impl ClassIndex for Index {
    fn find_classes(&self, _: &str, _: usize) -> Vec<ClassIndexEntry> {
        panic!("unexpected source type search")
    }
    fn find_class(&self, name: &str) -> Option<ClassIndexEntry> {
        assert_eq!(name, "fixture/Target", "unexpected source type lookup");
        Some(ClassIndexEntry {
            simple_name: "Target".into(),
            package_name: "fixture".into(),
            internal_name: name.into(),
        })
    }
    fn find_class_by_fqn(&self, _: &str) -> Option<ClassIndexEntry> {
        panic!("unexpected source type lookup")
    }
    fn get_methods(&self, owner: &str) -> Vec<MethodIndexEntry> {
        assert_eq!(owner, "fixture/Target");
        vec![MethodIndexEntry {
            name: "tick".into(),
            descriptor: "()V".into(),
            is_static: false,
            readable_signature: "tick()".into(),
        }]
    }
    fn get_fields(&self, _: &str) -> Vec<FieldIndexEntry> {
        vec![]
    }
}

#[test]
fn java_and_kotlin_parameters_reach_existing_handler_and_local_validation_in_source_order() {
    for kotlin in [false, true] {
        let host = fixture(kotlin);
        let model = parse(&host, kotlin);
        let facts = model.model.normalized_handler_source.unwrap();
        let service = HandlerSignatureService::new(&Index, None).with_host_source(facts);
        let sites = service.annotation_sites(&host.source, true);
        assert_eq!(sites.len(), 1);
        let handler = sites[0].handler_method.as_ref().unwrap();
        assert_eq!(
            handler
                .parameters
                .iter()
                .map(|parameter| parameter.name.as_str())
                .collect::<Vec<_>>(),
            if kotlin {
                vec!["callback", "when", "state", "ci"]
            } else {
                vec!["callback", "value", "state", "ci"]
            }
        );
        assert_eq!(
            handler
                .parameters
                .iter()
                .map(|parameter| parameter.type_descriptor.as_deref().unwrap())
                .collect::<Vec<_>>(),
            [CI, "I", INT_REF, CI]
        );
        assert!(handler.parameters[0].has_coerce);
        let Some(HandlerParameterSugarSpec::Local(local)) = &handler.parameters[1].sugar_spec
        else {
            panic!("missing Local spec")
        };
        assert!(local.args_only);
        assert_eq!(local.index, Some(1));
        assert_eq!(local.ordinal, Some(0));
        assert!(local.names.contains("value"));
        assert!(
            matches!(&handler.parameters[2].sugar_spec,Some(HandlerParameterSugarSpec::Share(share)) if share.value.as_deref()==Some("state"))
        );
        assert_eq!(
            handler.parameters[3].sugar_spec,
            Some(HandlerParameterSugarSpec::Cancellable)
        );
        for (parameter, fact) in handler.parameters.iter().zip(
            host.declarations[1]["parameters"]["value"]
                .as_array()
                .unwrap(),
        ) {
            let range = parameter.range.unwrap();
            assert_eq!(
                mcdev_core::position_to_offset(&host.source, range.start),
                Some(fact["range"]["start"].as_u64().unwrap() as usize)
            );
            assert_eq!(
                mcdev_core::position_to_offset(&host.source, range.end),
                Some(fact["range"]["end"].as_u64().unwrap() as usize)
            );
        }
        assert!(service
            .validate_handler(
                &host.source,
                &sites[0],
                &["fixture/Target".into()],
                handler,
                None
            )
            .is_empty());
        assert_eq!(
            LocalCaptureTypeResolver::new(&Index)
                .resolve_parameter(&host.source, &handler.parameters[1])
                .as_deref(),
            Some("I")
        );
        assert!(host
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|query| query["reference"] == "parameter:1"));
    }
}

#[test]
fn unavailable_or_absent_parameter_metadata_is_not_a_zero_argument_handler() {
    for missing in [true, false] {
        let mut host = fixture(false);
        if missing {
            host.declarations[1]
                .as_object_mut()
                .unwrap()
                .remove("parameters");
        } else {
            host.declarations[1]["parameters"] = unknown();
        }
        assert!(matches!(answer(&host, false), HostFact::Unavailable { .. }));
    }
}

#[test]
fn source_parameter_count_does_not_invent_a_jvm_parameter_mapping() {
    let mut host = fixture(false);
    host.signature = "()V".into();
    let model = parse(&host, false);
    assert!(
        matches!(model.model.normalized_handler_source.unwrap().sites,HostFact::Unavailable{reason} if reason.contains("correspondence"))
    );
}

#[test]
fn generic_local_ref_never_becomes_object_and_cancelled_facts_never_parse() {
    let mut host = fixture(false);
    replace_type(&mut host, "LocalIntRef", "LocalRef<String>");
    host.signature = host
        .signature
        .replace(INT_REF, "Lcom/llamalad7/mixinextras/sugar/ref/LocalRef;");
    let model = parse(&host, false);
    let facts = model.model.normalized_handler_source.unwrap();
    assert!(facts
        .checked_sites(&host.source)
        .unwrap_err()
        .to_string()
        .contains("generic"));
    host.active.store(false, Ordering::SeqCst);
    let evaluation:Evaluation=serde_json::from_value(json!({"evaluation":"eval","owner":"owner","source":host.source,"configuration":{},"declarations":[]})).unwrap();
    assert!(matches!(
        HostSemanticProvider::with_client(
            evaluation,
            Arc::new(host),
            "file:///project/雪Mixin.java".into(),
            Some(5)
        ),
        Err(FactsError::Cancelled(_))
    ));
}

fn replace_type(host: &mut Host, old: &str, new: &str) {
    let start = host.source.find(old).unwrap();
    let end = start + old.len();
    host.source.replace_range(start..end, new);
    let delta = new.len() as i64 - old.len() as i64;
    fn shift(value: &mut Value, end: usize, delta: i64) {
        match value {
            Value::Object(fields) => {
                for (name, value) in fields.iter_mut() {
                    if matches!(name.as_str(), "start" | "end") {
                        if let Some(offset) = value.as_u64().filter(|offset| *offset >= end as u64)
                        {
                            *value = json!((offset as i64 + delta) as u64);
                        }
                    } else {
                        shift(value, end, delta);
                    }
                }
            }
            Value::Array(values) => {
                for value in values {
                    shift(value, end, delta);
                }
            }
            _ => {}
        }
    }
    shift(&mut host.declarations, end, delta);
}

#[test]
fn a_supported_zero_argument_handler_is_known_empty() {
    let mut host = fixture(false);
    let start = host.source.find("hook(").unwrap() + 5;
    let end = host.source.rfind(") {}").unwrap();
    host.source.replace_range(start..end, "");
    let body = host.source.rfind("{}").unwrap();
    host.declarations[0]["fullRange"]["end"] = json!(host.source.len());
    host.declarations[0]["bodyRange"]["end"] = json!(host.source.len());
    host.declarations[1]["fullRange"]["end"] = json!(body + 2);
    host.declarations[1]["headerRange"]["end"] = json!(body);
    host.declarations[1]["bodyRange"] = json!({"start":body,"end":body+2});
    host.declarations[1]["parameters"] = found(json!([]));
    host.signature = "()V".into();
    let model = parse(&host, false);
    let source = model.model.normalized_handler_source.unwrap();
    let sites = source.checked_sites(&host.source).unwrap();
    assert_eq!(sites.len(), 1);
    assert!(sites[0]
        .handler_method
        .as_ref()
        .unwrap()
        .parameters
        .is_empty());
}

#[test]
fn mapped_share_value_is_decoded_and_handler_facts_reject_changed_text() {
    let mut host = fixture(false);
    replace_type(&mut host, "@Share(\"state\")", "@Share(\"sta\" + \"te\")");
    let expression = "\"sta\" + \"te\"";
    let start = host.source.find(expression).unwrap();
    let segments=[1,2,3,9,10].into_iter().enumerate().map(|(decoded,offset)|json!({
        "decoded":{"start":decoded,"end":decoded+1},"source":{"start":start+offset,"end":start+offset+1}})).collect::<Vec<_>>();
    let argument =
        &mut host.declarations[1]["parameters"]["value"][2]["annotations"][0]["arguments"][0];
    argument["range"] = span(&host.source, expression);
    argument["value"] = found(json!({"kind":"mappedString","value":"state","segments":segments}));
    let model = parse(&host, false);
    let source = model.model.normalized_handler_source.unwrap();
    let sites = source.checked_sites(&host.source).unwrap();
    assert!(
        matches!(&sites[0].handler_method.as_ref().unwrap().parameters[2].sugar_spec,
        Some(HandlerParameterSugarSpec::Share(share)) if share.value.as_deref()==Some("state"))
    );
    assert!(source
        .checked_sites(&(host.source + " "))
        .unwrap_err()
        .to_string()
        .contains("stale"));
}

fn share_parameter(host: &mut Host, id: &str, ref_name: &str) {
    let text = format!("@Share(\"{id}\") {ref_name} ci");
    replace_type(host, "@Cancellable CallbackInfo ci", &text);
    let start = host.source.rfind("@Share").unwrap();
    let literal = start + "@Share(".len();
    let annotation_end = literal + id.len() + 3;
    let name_start = host.source.rfind("ci").unwrap();
    host.declarations[1]["parameters"]["value"][3]["nameRange"] =
        json!({"start":name_start,"end":name_start+2});
    host.declarations[1]["parameters"]["value"][3]["annotations"] = json!([{
        "typeRef":"share","range":{"start":start,"end":annotation_end},
        "nameRange":{"start":start+1,"end":start+6},
        "arguments":[{"name":"value","range":{"start":literal,"end":literal+id.len()+2},
            "value":found(json!({"kind":"string","value":id,"contentRange":{"start":literal+1,"end":literal+1+id.len()}}))}]
    }]);
    host.signature = format!("({CI}I{INT_REF}Lcom/llamalad7/mixinextras/sugar/ref/{ref_name};)V");
}

#[test]
fn normalized_share_ids_reach_completion_without_reparsing_parameter_types() {
    let mut host = fixture(false);
    share_parameter(&mut host, "", "LocalIntRef");
    let facts = parse(&host, false).model.normalized_handler_source.unwrap();
    let service = ShareCompletionService::new(&Index).with_host_source(Some(facts));
    let mut context = AnnotationContext::new(MixinAnnotation::Share, AnnotationSlot::Value, "st");
    context.annotation_start_offset = host.source.rfind("@Share").unwrap();
    let items = service.complete(&host.source, &context, std::iter::empty::<&str>());
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "state");

    let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        service.complete(&host.source, &context, ["class Other {}"])
    }))
    .unwrap_err();
    assert!(error.is::<mcdev_core::host_source::HostSourceError>());

    context.annotation_start_offset = 0;
    let extra = std::iter::from_fn(|| -> Option<&str> {
        panic!("additional source advanced without a normalized Share key")
    });
    assert!(service.complete(&host.source, &context, extra).is_empty());
}

#[test]
fn normalized_cancellable_and_share_constraints_reach_existing_handler_validation() {
    let mut host = fixture(false);
    replace_type(&mut host, "CallbackInfo ci", "int ci");
    let name_start = host.source.rfind("ci").unwrap();
    host.declarations[1]["parameters"]["value"][3]["nameRange"] =
        json!({"start":name_start,"end":name_start+2});
    host.signature = format!("({CI}I{INT_REF}I)V");
    let facts = parse(&host, false).model.normalized_handler_source.unwrap();
    let service = HandlerSignatureService::new(&Index, None).with_host_source(facts);
    let sites = service.annotation_sites(&host.source, true);
    let handler = sites[0].handler_method.as_ref().unwrap();
    let issues = service.validate_handler(
        &host.source,
        &sites[0],
        &["fixture/Target".into()],
        handler,
        None,
    );
    assert!(issues
        .iter()
        .any(|issue| issue.message.contains("Cancellable parameter type")));

    replace_type(&mut host, "LocalIntRef state", "int state");
    let name_start = host.source.rfind("state").unwrap();
    host.declarations[1]["parameters"]["value"][2]["nameRange"] =
        json!({"start":name_start,"end":name_start+5});
    host.signature = format!("({CI}III)V");
    let facts = parse(&host, false).model.normalized_handler_source.unwrap();
    let service = HandlerSignatureService::new(&Index, None).with_host_source(facts);
    let sites = service.annotation_sites(&host.source, true);
    let handler = sites[0].handler_method.as_ref().unwrap();
    assert!(service
        .validate_common_sugar_constraints(handler, None, Some(sites[0].annotation))
        .iter()
        .any(|issue| issue.code
            == mcdev_core::mixinextras_handler::diagnostic_codes::INVALID_SHARE_PARAMETER_TYPE));
}

struct SnapshotHost {
    base: Host,
    descriptor: Value,
    bad_parameter_context: bool,
}

struct SnapshotInventory(Vec<(Value, Value)>);
impl mcdev_companion::host_facts::bridge::FactReader for SnapshotInventory {
    fn read_facts(&mut self, evaluation: &str, queries: &[Value]) -> Result<Value, FactsError> {
        assert_eq!(evaluation, "eval");
        let facts = queries.iter().map(|query| match query["op"].as_str().unwrap() {
            "sourceInventory" => json!({"sources":self.0.iter().map(|(entry,_)|entry).collect::<Vec<_>>(),"next":null,"coverage":found(Value::Null)}),
            "sourceSnapshot" => found(self.0.iter().find(|(entry,_)|entry["id"]==query["source"]).unwrap().1.clone()),
            other => panic!("unexpected snapshot operation {other}"),
        }).collect::<Vec<_>>();
        Ok(json!({"facts":facts}))
    }
}
impl SemanticFactClient for SnapshotHost {
    fn check_active(&self) -> Result<(), FactsError> {
        self.base.check_active()
    }
    fn read_facts(&self, evaluation: &str, queries: &[Value]) -> Result<Value, FactsError> {
        let mut result = self.base.read_facts(evaluation, queries)?;
        for (query, fact) in queries.iter().zip(result["facts"].as_array_mut().unwrap()) {
            match query["op"].as_str().unwrap() {
                "sourceFacts" => {
                    assert_eq!(query["source"], self.descriptor["id"]);
                    fact["source"] = self.descriptor.clone();
                    fact["semanticScope"] = found(Value::Null);
                }
                "resolveTypeRef" | "declarationType" | "declarationMember" => {
                    let mut context = self.descriptor.clone();
                    if query["reference"] == "parameter:0" {
                        *fact = found(
                            json!({"id":"nominal:callback", "qualifiedName":"org.spongepowered.asm.mixin.injection.callback.CallbackInfo", "package":"org.spongepowered.asm.mixin.injection.callback"}),
                        );
                        if self.bad_parameter_context {
                            context["documentUri"] =
                                found(json!("file:///project/main/雪Mixin.java"));
                            context["identity"]["revision"] = json!("main:11");
                        }
                    }
                    if fact["state"] == "found" {
                        fact["value"]["readOnly"] = json!(true);
                        fact["value"]["contextSource"] = context;
                    }
                }
                other => panic!("foreign parameters must not require or bind identities: {other}"),
            }
        }
        Ok(result)
    }
}

#[test]
fn foreign_parameters_keep_the_selected_snapshot_when_main_and_test_have_the_same_fqn() {
    use mcdev_companion::{
        host_facts::inputs::{InputOps, ScopedInputs},
        wire::Fact,
    };
    use sha2::{Digest, Sha256};
    let mut host = fixture(false);
    replace_type(
        &mut host,
        "@Mixin(Target.class)",
        "@Mixin(targets=\"fixture.Target\")",
    );
    host.declarations[0]["annotations"] = json!([annotation(
        &host.source,
        "@Mixin(targets=\"fixture.Target\")",
        "Mixin",
        "mixin",
        json!([scalar(
            &host.source,
            "targets",
            "\"fixture.Target\"",
            string_value(&host.source, "fixture.Target")
        )]),
    )]);
    for bad_parameter_context in [false, true] {
        let entries = ["main", "test"].map(|scope| {
            let uri = format!("file:///project/{scope}/雪Mixin.java");
            let entry = json!({"id":format!("source:{scope}"),"displayPath":format!("C:/project/{scope}/雪Mixin.java"),
                "documentUri":found(json!(uri)),"languageId":"java", "identity":{"kind":"source","revision":format!("{scope}:11")}});
            let mut snapshot = entry.clone();
            snapshot["text"] = json!(host.source);
            snapshot["sha256"] = json!(format!("{:x}",Sha256::digest(host.source.as_bytes())));
            (entry, snapshot)
        });
        let client = SnapshotHost {
            base: host.clone(),
            descriptor: entries[1].0.clone(),
            bad_parameter_context,
        };
        let ops = InputOps::intersect(&json!({"capabilities":["source-snapshots-v1"],
            "supportedOps":{"source-snapshots-v1":["sourceInventory","sourceSnapshot"]}}))
        .unwrap();
        let mut inputs =
            ScopedInputs::new("eval", &ops, SnapshotInventory(entries.into())).unwrap();
        let captured = inputs.source_inventory().unwrap().entries;
        let snapshots = inputs.source_snapshots(&captured).unwrap();
        let project_sources = captured
            .into_iter()
            .zip(snapshots)
            .map(|(entry, fact)| {
                let Fact::Found { value } = fact else {
                    panic!("captured snapshot unavailable")
                };
                (entry, value)
            })
            .collect();
        let evaluation: Evaluation = serde_json::from_value(json!({"evaluation":"eval","owner":"owner","source":host.source,"configuration":{},"declarations":[]})).unwrap();
        let provider = HostSemanticProvider::with_client(
            evaluation,
            Arc::new(client),
            "file:///project/main/雪Mixin.java".into(),
            Some(5),
        )
        .unwrap()
        .with_source_facts(true)
        .with_project_sources(project_sources)
        .unwrap();
        let result = provider
            .parse(&HostSemanticRequest {
                source: &host.source,
                source_uri: "file:///project/test/雪Mixin.java",
                document_version: None,
                environment: &HostSemanticEnvironment::default(),
                project: None,
            })
            .unwrap();
        if bad_parameter_context {
            assert!(
                matches!(result.fact, HostFact::Unavailable {reason} if reason.contains("foreign source context"))
            );
        } else {
            let HostFact::Known(model) = result.fact else {
                panic!("foreign handler unavailable: {:?}", result.fact)
            };
            let facts = model.model.normalized_handler_source.unwrap();
            assert_eq!(facts.document_uri, "file:///project/test/雪Mixin.java");
            assert_eq!(
                facts.checked_sites(&host.source).unwrap()[0]
                    .handler_method
                    .as_ref()
                    .unwrap()
                    .parameters[1]
                    .name,
                "value"
            );
        }
    }
}
