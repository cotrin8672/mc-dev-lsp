use super::*;

fn namespace(host: &mut Host, index: usize, name: &str) {
    let annotation = &host.declarations[1]["parameters"]["value"][index]["annotations"][0];
    let id = annotation["arguments"][0]["value"]["value"]["value"]
        .as_str()
        .unwrap()
        .to_owned();
    let range = &annotation["range"];
    let old = host.source
        [range["start"].as_u64().unwrap() as usize..range["end"].as_u64().unwrap() as usize]
        .to_owned();
    let text = format!("@Share(value=\"{id}\", namespace=\"{name}\")");
    replace_type(host, &old, &text);
    let start = host.source.find(&text).unwrap();
    let value_start = start + "@Share(value=".len();
    let ns_start = start + text.find("namespace=").unwrap() + "namespace=".len();
    let value = |name: &str, start: usize, text: &str| json!({"name":name,"range":{"start":start,"end":start+text.len()+2},"value":found(json!({"kind":"string","value":text,"contentRange":{"start":start+1,"end":start+1+text.len()}}))});
    host.declarations[1]["parameters"]["value"][index]["annotations"] = json!([{
        "typeRef":"share","range":{"start":start,"end":start+text.len()},"nameRange":{"start":start+1,"end":start+6},
        "arguments":[value("value",value_start,&id),value("namespace",ns_start,name)]
    }]);
}

#[test]
fn normalized_cross_file_share_keeps_uri_namespace_and_exact_foreign_ranges() {
    let mut current = fixture(false);
    replace_type(&mut current, "//😀", "package fixture;\n//😀");
    share_parameter(&mut current, "", "LocalIntRef");
    let current_facts = parse(&current, false)
        .model
        .normalized_handler_source
        .unwrap();
    let mut foreign = fixture(false);
    replace_type(&mut foreign, "//😀", "package fixture;\n//😀");
    replace_type(&mut foreign, "@Share(\"state\")", "@Share(\"remote\")");
    foreign.declarations[1]["parameters"]["value"][2]["annotations"] = json!([annotation(
        &foreign.source,
        "@Share(\"remote\")",
        "Share",
        "share",
        json!([scalar(
            &foreign.source,
            "value",
            "\"remote\"",
            string_value(&foreign.source, "remote")
        )])
    )]);
    let mut foreign_facts = parse(&foreign, false)
        .model
        .normalized_handler_source
        .unwrap();
    foreign_facts.document_uri = "file:///project/test/雪Mixin.java".into();
    let mut context = AnnotationContext::new(MixinAnnotation::Share, AnnotationSlot::Value, "rem");
    context.annotation_start_offset = current.source.rfind("@Share").unwrap();
    let service = ShareCompletionService::new(&Index).with_host_source(Some(current_facts));
    let additional = HostShareSource {
        text: foreign.source.clone(),
        handlers: foreign_facts.clone(),
    };
    let items =
        service.complete_with_normalized_sources(&current.source, &context, [additional.clone()]);
    assert_eq!(
        items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        ["remote"]
    );
    let mut other_namespace = additional.clone();
    other_namespace.handlers.qualified_name = "fixture.OtherMixin".into();
    assert!(service
        .complete_with_normalized_sources(&current.source, &context, [other_namespace])
        .is_empty());
    let mut named_current = current.clone();
    let mut named_foreign = foreign.clone();
    namespace(&mut named_current, 3, "shared");
    namespace(&mut named_foreign, 2, "shared");
    let current_facts = parse(&named_current, false)
        .model
        .normalized_handler_source
        .unwrap();
    let mut foreign_facts = parse(&named_foreign, false)
        .model
        .normalized_handler_source
        .unwrap();
    foreign_facts.document_uri = "file:///project/test/雪Mixin.java".into();
    foreign_facts.qualified_name = "fixture.OtherMixin".into();
    let named_service = ShareCompletionService::new(&Index).with_host_source(Some(current_facts));
    context.annotation_start_offset = named_current.source.rfind("@Share").unwrap();
    let items = named_service.complete_with_normalized_sources(
        &named_current.source,
        &context,
        [HostShareSource {
            text: named_foreign.source,
            handlers: foreign_facts,
        }],
    );
    assert_eq!(
        items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        ["remote"]
    );
    let mut stale = additional;
    stale.text.push(' ');
    let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        service.complete_with_normalized_sources(&current.source, &context, [stale])
    }))
    .unwrap_err();
    assert!(error.is::<mcdev_core::host_source::HostSourceError>());
    context.annotation_start_offset = 0;
    assert!(service
        .complete_with_normalized_sources(
            &current.source,
            &context,
            std::iter::from_fn(|| -> Option<HostShareSource> {
                panic!("unneeded normalized source read")
            })
        )
        .is_empty());
}

struct Config {
    value: mcdev_core::project::MixinConfigRef,
    reads: Arc<std::sync::atomic::AtomicUsize>,
}
struct NoCode;
impl mcdev_core::bytecode::BytecodeIndex for NoCode {
    fn get_at_target_candidates(
        &self,
        _: &str,
        _: &str,
        _: Option<&str>,
        _: &str,
    ) -> Vec<mcdev_core::bytecode::AtTargetCandidate> {
        panic!("Share does not require executable code")
    }
    fn get_return_ordinal_count(&self, _: &str, _: &str, _: Option<&str>) -> usize {
        panic!()
    }
    fn get_class_bytes(&self, _: &str) -> Option<Vec<u8>> {
        panic!()
    }
}
impl mcdev_core::mixin_facade::HostMixinSourceProvider for Config {
    fn config(
        &self,
        path: &std::path::Path,
    ) -> mcdev_core::host_source::HostSourceResult<mcdev_core::project::MixinConfigRef> {
        assert_eq!(path, self.value.path);
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(mcdev_core::host_source::HostAnswer::known(
            self.value.clone(),
            vec![],
        ))
    }
    fn compilation_unit_source(
        &self,
        _: &str,
        _: Option<&std::path::Path>,
    ) -> mcdev_core::host_source::HostSourceResult<mcdev_core::mixin_facade::RegisteredMixinSource>
    {
        panic!("normalized sources must not use the String seam")
    }
    fn read_source_file(
        &self,
        _: &std::path::Path,
    ) -> mcdev_core::host_source::HostSourceResult<String> {
        panic!("no guessed source-root fallback")
    }
}
struct RegisteredClient {
    snapshot: SnapshotHost,
    bad_hash: Arc<AtomicBool>,
}
impl SemanticFactClient for RegisteredClient {
    fn check_active(&self) -> Result<(), FactsError> {
        self.snapshot.check_active()
    }
    fn read_facts(&self, evaluation: &str, queries: &[Value]) -> Result<Value, FactsError> {
        use sha2::{Digest, Sha256};
        self.check_active()?;
        let mut facts = Vec::new();
        for query in queries {
            self.snapshot.base.calls.lock().unwrap().push(query.clone());
            let fact = match query["op"].as_str().unwrap() {
                "resolveType" => {
                    assert_eq!(query["name"], "fixture.雪Mixin");
                    found(
                        json!({"id":"registered:selected","qualifiedName":"fixture.雪Mixin","package":"fixture"}),
                    )
                }
                "sourceLocation" => {
                    assert_eq!(query["target"], "registered:selected");
                    let source = &self.snapshot.base.source;
                    let start = source.find("雪Mixin").unwrap();
                    found(
                        json!({"target":query["target"],"documentUri":self.snapshot.descriptor["documentUri"]["value"],
                        "range":{"start":mcdev_core::offset_to_position(source,start).unwrap(),"end":mcdev_core::offset_to_position(source,start+"雪Mixin".len()).unwrap()},
                        "source":{"identity":self.snapshot.descriptor["identity"],"sha256":if self.bad_hash.load(Ordering::SeqCst){"0".repeat(64)}else{format!("{:x}",Sha256::digest(source.as_bytes()))}}}),
                    )
                }
                _ => self
                    .snapshot
                    .read_facts(evaluation, std::slice::from_ref(query))?["facts"][0]
                    .clone(),
            };
            facts.push(fact);
        }
        Ok(json!({"facts":facts}))
    }
}

#[test]
fn registered_foreign_share_iterator_uses_the_host_selected_uri_even_for_the_current_fqn() {
    use mcdev_companion::{
        commands::ipc::semantic::RegisteredSourceProvider,
        host_facts::inputs::{InputOps, ScopedInputs},
        wire::Fact,
    };
    use mcdev_core::{
        host_source::{HostAnswer, HostSourceError},
        mixin_facade::{HostMixinFacade, HostMixinSourceProvider, RegisteredHandlerSource},
        project::{MixinConfigRef, ProjectContextBuilder},
    };
    use sha2::{Digest, Sha256};
    let mut current = fixture(false);
    replace_type(&mut current, "//😀", "package fixture;\n//😀");
    share_parameter(&mut current, "", "LocalIntRef");
    let main_uri_value =
        mcdev_core::project::path_to_uri(&std::path::PathBuf::from("C:/project/main/雪Mixin.java"));
    let test_uri_value =
        mcdev_core::project::path_to_uri(&std::path::PathBuf::from("C:/project/test/雪Mixin.java"));
    let main_uri = main_uri_value.as_str();
    let test_uri = test_uri_value.as_str();
    let mut current_facts = parse(&current, false)
        .model
        .normalized_handler_source
        .unwrap();
    current_facts.document_uri = main_uri.into();
    let mut foreign = fixture(false);
    replace_type(&mut foreign, "//😀", "package fixture;\n//😀");
    replace_type(
        &mut foreign,
        "@Mixin(Target.class)",
        "@Mixin(targets=\"fixture.Target\")",
    );
    foreign.declarations[0]["annotations"] = json!([annotation(
        &foreign.source,
        "@Mixin(targets=\"fixture.Target\")",
        "Mixin",
        "mixin",
        json!([scalar(
            &foreign.source,
            "targets",
            "\"fixture.Target\"",
            string_value(&foreign.source, "fixture.Target")
        )])
    )]);
    replace_type(&mut foreign, "@Share(\"state\")", "@Share(\"remote\")");
    foreign.declarations[1]["parameters"]["value"][2]["annotations"] = json!([annotation(
        &foreign.source,
        "@Share(\"remote\")",
        "Share",
        "share",
        json!([scalar(
            &foreign.source,
            "value",
            "\"remote\"",
            string_value(&foreign.source, "remote")
        )])
    )]);
    let entries=[("main",main_uri,&current.source),("test",test_uri,&foreign.source)].map(|(scope,uri,text)|{
        let entry=json!({"id":format!("source:{scope}"),"displayPath":format!("C:/project/{scope}/雪Mixin.java"),"documentUri":found(json!(uri)),"languageId":"java","identity":{"kind":"source","revision":format!("{scope}:11")}});
        let mut snapshot=entry.clone();snapshot["text"]=json!(text);snapshot["sha256"]=json!(format!("{:x}",Sha256::digest(text.as_bytes())));
        (entry,snapshot)
    });
    let bad_hash = Arc::new(AtomicBool::new(false));
    let client = RegisteredClient {
        snapshot: SnapshotHost {
            base: foreign.clone(),
            descriptor: entries[1].0.clone(),
            bad_parameter_context: false,
        },
        bad_hash: bad_hash.clone(),
    };
    let ops=InputOps::intersect(&json!({"capabilities":["source-snapshots-v1"],"supportedOps":{"source-snapshots-v1":["sourceInventory","sourceSnapshot"]}})).unwrap();
    let mut inputs = ScopedInputs::new("eval", &ops, SnapshotInventory(entries.into())).unwrap();
    let captured = inputs.source_inventory().unwrap().entries;
    let snapshots = inputs.source_snapshots(&captured).unwrap();
    let project_sources = captured
        .into_iter()
        .zip(snapshots)
        .map(|(entry, fact)| {
            let Fact::Found { value } = fact else {
                panic!()
            };
            (entry, value)
        })
        .collect();
    let evaluation=serde_json::from_value(json!({"evaluation":"eval","owner":"owner","source":current.source,"configuration":{},"declarations":[]})).unwrap();
    let semantic =
        HostSemanticProvider::with_client(evaluation, Arc::new(client), main_uri.into(), Some(5))
            .unwrap()
            .with_source_facts(true)
            .with_project_sources(project_sources)
            .unwrap();
    let semantic = Arc::new(semantic);
    let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let config = MixinConfigRef {
        path: "C:/project/mixins.json".into(),
        package_name: Some("fixture".into()),
        mixins: vec!["雪Mixin".into()],
        ..Default::default()
    };
    let registered = RegisteredSourceProvider::new(
        Arc::new(Config {
            value: config.clone(),
            reads: reads.clone(),
        }),
        semantic.clone(),
        true,
    );
    let mut project = ProjectContextBuilder::empty("project", "C:/project");
    project.mixin_configs = vec![config];
    let service = ShareCompletionService::new(&Index).with_host_source(Some(current_facts));
    let mut context = AnnotationContext::new(MixinAnnotation::Share, AnnotationSlot::Value, "rem");
    let sources = || {
        HostMixinFacade::collect_registered_handler_sources(
            &project,
            &current.source,
            main_uri,
            &registered,
        )
        .map(|value| value.unwrap_or_else(|error| panic!("registered Share source: {error:?}")))
    };
    assert!(service
        .complete_with_normalized_sources(&current.source, &context, sources())
        .is_empty());
    assert_eq!(reads.load(Ordering::SeqCst), 0);
    context.annotation_start_offset = current.source.rfind("@Share").unwrap();
    let items = service.complete_with_normalized_sources(&current.source, &context, sources());
    assert_eq!(
        items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        ["remote"]
    );
    assert_eq!(reads.load(Ordering::SeqCst), 1);
    let cache = mcdev_core::host_source::SemanticModelCache::default();
    let mut facade = mcdev_core::mixin_facade::HostMixinServiceFacade::new(
        &Index,
        &NoCode,
        semantic.as_ref(),
        &cache,
    );
    facade.context = Some(&project);
    facade = facade
        .with_registered_sources(&registered)
        .with_registered_handler_sources(&registered);
    let mut model = parse(&current, false).model;
    model.source_uri = main_uri.into();
    model
        .normalized_handler_source
        .as_mut()
        .unwrap()
        .document_uri = main_uri.into();
    let position = mcdev_core::offset_to_position(
        &current.source,
        current.source.rfind("@Share").unwrap() + "@Share(\"".len(),
    )
    .unwrap();
    let request = mcdev_core::mixin_facade::MixinFacadeRequest {
        buffer_text: current.source.clone(),
        document_uri: main_uri.into(),
        line: position.line as i32,
        character: position.character as i32,
        semantic_model: Some(model),
        ..Default::default()
    };
    let items = facade
        .complete(
            &request,
            &mcdev_core::mixin_completion::MixinCompletionOptions::default(),
        )
        .unwrap();
    assert!(items.iter().any(|item| item.label == "remote"));
    let HostAnswer {
        fact: HostFact::Known(RegisteredHandlerSource::Source(source)),
        ..
    } = registered
        .registered_handler_source("fixture.雪Mixin", None)
        .unwrap()
    else {
        panic!("normalized registered source unavailable")
    };
    assert_eq!(source.handlers.document_uri, test_uri);
    bad_hash.store(true, Ordering::SeqCst);
    assert!(
        matches!(registered.registered_handler_source("fixture.雪Mixin",None).unwrap().fact,HostFact::Unavailable{reason} if reason.contains("identity"))
    );
    bad_hash.store(false, Ordering::SeqCst);
    foreign.active.store(false, Ordering::SeqCst);
    assert!(matches!(
        registered.registered_handler_source("fixture.雪Mixin", None),
        Err(HostSourceError::Cancelled(_))
    ));
}
