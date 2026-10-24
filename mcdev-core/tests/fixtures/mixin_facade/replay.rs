//! Whole facade/source/helper outputs, real edits, and strict external callback replay.
use mcdev_core::{
    bytecode::*, mixin_completion::*, mixin_facade::*, mixinextras_handler::*,
    mixinextras_operation::*, *,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};

fn snake(name: &str) -> String {
    let mut result = String::new();
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            result.push('_');
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }
    result
}
fn pascal(name: &str) -> String {
    name.split('_')
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|c| c.to_ascii_uppercase().to_string() + &chars.as_str().to_ascii_lowercase())
                .unwrap_or_default()
        })
        .collect()
}
fn model(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    let output = if key == "handlerParameterDescriptors" {
                        value.clone()
                    } else if [
                        "annotation",
                        "kind",
                        "operationKind",
                        "occurrenceResultClassification",
                    ]
                    .contains(&key.as_str())
                        && value.is_string()
                    {
                        json!(pascal(value.as_str().unwrap()))
                    } else {
                        model(value)
                    };
                    let key = if key.chars().next().is_some_and(char::is_uppercase) {
                        key.clone()
                    } else {
                        snake(key)
                    };
                    (key, output)
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(model).collect()),
        other => other.clone(),
    }
}
fn decode<T: DeserializeOwned>(value: &Value) -> T {
    serde_json::from_value(model(value)).unwrap_or_else(|e| panic!("typed fixture {e}: {value}"))
}
fn output<T: Serialize>(value: T) -> Value {
    serde_json::to_value(value).unwrap()
}
struct Reads<'a> {
    rows: &'a [Value],
    calls: std::sync::Mutex<Vec<Value>>,
}
impl HandlerReferenceTypeResolver for Reads<'_> {
    fn is_known_reference(&self, name: &str) -> Option<bool> {
        reference_facts()["known"][name].as_bool()
    }
    fn is_assignable(&self, from: &str, expected: &str) -> Option<bool> {
        reference_facts()["assignable"][format!("{from}|{expected}")].as_bool()
    }
}
fn reference_facts() -> &'static Value {
    static FACTS: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    FACTS.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../mixinextras_services/host-reference-facts.json"
        ))
        .unwrap()
    })
}
fn utf16_offset(source: &str, units: usize) -> Option<usize> {
    let mut current = 0;
    for (byte, ch) in source.char_indices() {
        if current == units {
            return Some(byte);
        }
        current += ch.len_utf16();
    }
    if current == units {
        Some(source.len())
    } else {
        None
    }
}
impl Reads<'_> {
    fn get(&self, api: &str, args: Value) -> &Value {
        let mut calls = self.calls.lock().unwrap();
        let occurrence = calls
            .iter()
            .filter(|r| r["api"] == api && r["args"] == args)
            .count();
        calls.push(json!({"api":api,"args":args}));
        drop(calls);
        &self
            .rows
            .iter()
            .filter(|r| r["api"] == api && r["args"] == args)
            .nth(occurrence)
            .unwrap_or_else(|| {
                panic!(
                    "uncaptured read {api} {args}: {}",
                    std::backtrace::Backtrace::force_capture()
                )
            })["output"]
    }
    fn assert_trace(&self) {
        let actual = self.calls.lock().unwrap();
        let expected: Vec<_> = self
            .rows
            .iter()
            .map(|r| json!({"api":r["api"],"args":r["args"]}))
            .collect();
        assert!(
            *actual == expected,
            "callback trace mismatch actual={} expected={}",
            serde_json::to_string(&*actual).unwrap(),
            serde_json::to_string(&expected).unwrap()
        );
    }
}
use mcdev_core::host_source::*;
struct HostProvider<'a> {
    reads: &'a Reads<'a>,
}
impl HostReadValidator for HostProvider<'_> {
    fn validate_current_reads(
        &self,
        _generation: HostGenerations,
        _reads: &HostReadSet,
    ) -> Result<bool, HostSourceError> {
        Ok(true)
    }
}
impl HostSemanticModelProvider for HostProvider<'_> {
    fn parse(&self, request: &HostSemanticRequest<'_>) -> HostSourceResult<HostSemanticModel> {
        let environment = json!({"classpathEntries":request.environment.classpath_entries,"sourcepathEntries":request.environment.sourcepath_entries,"unitName":request.environment.unit_name});
        let value = self.reads.get(
            "Host.semanticProvider",
            json!([request.source, request.source_uri, environment]),
        );
        let model = source_replay::deserialize(value);
        Ok(HostAnswer::known(
            HostSemanticModel {
                model,
                availability: SemanticAvailability::StableFallback,
                parse_ms: 0,
            },
            vec![],
        ))
    }
}
struct ResourceProvider<'a> {
    reads: &'a Reads<'a>,
    files: &'a Value,
    project: bool,
}
impl HostMixinSourceProvider for ResourceProvider<'_> {
    fn config(
        &self,
        path: &std::path::Path,
    ) -> HostSourceResult<mcdev_core::project::MixinConfigRef> {
        let Some(text) = self
            .files
            .get(path.to_string_lossy().as_ref())
            .and_then(Value::as_str)
        else {
            return Ok(HostAnswer::missing(vec![]));
        };
        let value =
            mcdev_core::mixin_edits::MixinConfigEditor::parse(text, &path.to_string_lossy())
                .unwrap();
        Ok(HostAnswer::known(
            mcdev_core::project::MixinConfigRef {
                path: path.into(),
                package_name: value.package_name,
                mixins: value.mixins,
                client: value.client,
                server: value.server,
                common: value.common,
            },
            vec![],
        ))
    }
    fn read_source_file(&self, path: &std::path::Path) -> HostSourceResult<String> {
        Ok(self
            .files
            .get(path.to_string_lossy().as_ref())
            .and_then(Value::as_str)
            .map_or_else(
                || HostAnswer::missing(vec![]),
                |text| HostAnswer::known(text.into(), vec![]),
            ))
    }
    fn compilation_unit_source(
        &self,
        fqn: &str,
        current: Option<&std::path::Path>,
    ) -> HostSourceResult<RegisteredMixinSource> {
        if !self.project {
            return Ok(HostAnswer::missing(vec![]));
        }
        let value = self.reads.get(
            "Host.unitSource",
            json!([fqn, current.map(|p| p.to_string_lossy().into_owned())]),
        );
        Ok(match value["status"].as_str().unwrap() {
            "missing" => HostAnswer::missing(vec![]),
            "current" => HostAnswer::known(RegisteredMixinSource::Current, vec![]),
            "source" => HostAnswer::known(
                RegisteredMixinSource::Source(value["text"].as_str().unwrap().into()),
                vec![],
            ),
            status => panic!("unit source status {status}"),
        })
    }
}
fn host_context(value: &Value) -> mcdev_core::project::ProjectContext {
    use mcdev_core::project::*;
    let mut context = ProjectContextBuilder::empty(
        value["projectId"].as_str().unwrap(),
        value["root"].as_str().unwrap(),
    );
    context.classpath = serde_json::from_value(value["classpath"].clone()).unwrap();
    context.source_sets = serde_json::from_value(value["sourceSets"].clone()).unwrap();
    context.mixin_configs = serde_json::from_value(value["mixinConfigs"].clone()).unwrap();
    context
}
fn host_project(
    reads: &Reads<'_>,
    uri: &str,
    workspace: Option<&str>,
    kotlin: bool,
) -> Option<HostProjectKey> {
    let value = if kotlin {
        let root = workspace?;
        reads.get("Host.workspaceProject", json!([root]))
    } else {
        reads.get("Host.documentProject", json!([uri]))
    };
    (!value.is_null()).then(|| HostProjectKey {
        project_id: "oracleHost".into(),
        generations: HostGenerations::default(),
    })
}
fn recorded_at<'a>(reads: &'a Reads<'a>) -> AtValueCompletionService<'a> {
    let callback = move |api: &str, host: &str| {
        if let Some(resolver) = reads.rows.iter().find(|r| {
            matches!(
                r["api"].as_str(),
                Some("Host.documentProject" | "Host.workspaceProject")
            )
        }) {
            let done = reads
                .calls
                .lock()
                .unwrap()
                .iter()
                .any(|r| r["api"] == resolver["api"]);
            if !done {
                reads.get(resolver["api"].as_str().unwrap(), resolver["args"].clone());
            }
        }
        if let Some(input) = reads.rows.iter().find(|r| r["api"] == host) {
            reads.get(host, input["args"].clone());
        }
        reads.get(api, json!([])).clone()
    };
    AtValueCompletionService::new(
        move || {
            callback("AtValue.injectionPointSpecifierSupported", "Host.specifier")
                .as_bool()
                .unwrap()
        },
        move || {
            serde_json::from_value(callback("AtValue.additionalAtValues", "Host.atCodes")).unwrap()
        },
    )
}
fn replay_host(row: &Value) -> (Value, Value) {
    let args = &row["args"];
    let source = args["source"].as_str().unwrap();
    let uri = args["documentUri"].as_str().unwrap();
    let reads = Reads {
        rows: row["reads"].as_array().unwrap(),
        calls: Default::default(),
    };
    let provider = HostProvider { reads: &reads };
    let cache = SemanticModelCache::default();
    let context = args
        .get("session")
        .map(|s| host_context(&s["context"]))
        .or_else(|| args.get("projectContext").map(host_context));
    let mut facade = HostMixinServiceFacade::new(&reads, &reads, &provider, &cache);
    facade.context = context.as_ref();
    facade.reference_types = Some(&reads);
    facade.project_session_version = args["projectSessionVersion"].as_u64().unwrap_or(0);
    facade.project_resolver =
        Box::new(|uri, workspace, kotlin| Ok(host_project(&reads, uri, workspace, kotlin)));
    facade.source_query_ready = Box::new(|_| {
        reads.get("Host.sourceQueryFactory", json!([{"present":true}]));
        Ok(())
    });
    facade.specifier_supported = Box::new(|project, version| {
        let v = reads
            .get(
                "Host.specifier",
                json!([project.map(|_| json!({"present":true})), version]),
            )
            .as_bool()
            .unwrap();
        assert_eq!(
            reads.get("AtValue.injectionPointSpecifierSupported", json!([])),
            &json!(v)
        );
        Ok(v)
    });
    facade.additional_at_values = Box::new(|project, version| {
        let v = reads.get(
            "Host.atCodes",
            json!([project.map(|_| json!({"present":true})), version]),
        );
        assert_eq!(reads.get("AtValue.additionalAtValues", json!([])), v);
        Ok(serde_json::from_value(v.clone()).unwrap())
    });
    if args["__hasOverride"] == true {
        facade.completion_override = Some(Box::new(|request, options| {
            Ok(serde_json::from_value(
                reads
                    .get(
                        "Host.completeOverride",
                        json!([
                            request.buffer_text,
                            request.line,
                            request.character,
                            options
                        ]),
                    )
                    .clone(),
            )
            .unwrap())
        }));
    }
    if args["__hasFactory"] == true {
        facade.core_override = Some(Box::new(|request, options, index, bytes, language| {
            reads.get("Host.coreFactory", json!([]));
            let owner = if row["legacyCase"]
                .as_str()
                .unwrap()
                .contains("customFacadeFactoryRemainsCompatibleWithProjectSourceQueryFactory")
            {
                "demo/TargetTarget"
            } else {
                "ignored"
            };
            let found = index.find_class(owner);
            assert_eq!(found.is_some(), owner != "ignored");
            assert!(bytes
                .get_at_target_candidates("ignored", "run", Some("()V"), "INVOKE")
                .is_empty());
            MixinServiceFacade::new(&reads, &reads)
                .with_reference_types(&reads)
                .with_at_value_completion(recorded_at(&reads))
                .complete_with_debug(request, options, "mcdev.completion", language)
        }));
    }
    let request = MixinFacadeRequest {
        buffer_text: source.into(),
        line: args["line"].as_i64().unwrap_or(0) as i32,
        character: args["character"].as_i64().unwrap_or(0) as i32,
        document_uri: uri.into(),
        semantic_model: args
            .get("semanticModel")
            .filter(|m| !m.is_null())
            .map(source_replay::deserialize),
        ..Default::default()
    };
    let actual = match row["api"].as_str().unwrap() {
        "HandlerSignatureService.findAnnotationSites" => {
            output(HandlerSignatureService::find_annotation_sites(source))
        }
        "HandlerSignatureService.findSugarHandlerAnnotationSites" => {
            output(HandlerSignatureService::find_sugar_handler_annotation_sites(source))
        }
        "HostMixinServiceFacade.semanticModel" => output(
            facade
                .semantic_model(source, uri, args.get("session").is_some())
                .unwrap(),
        ),
        "HostMixinServiceFacade.complete" => output(
            facade
                .complete(&request, &options(&args["options"]))
                .unwrap(),
        ),
        "HostMixinServiceFacade.completeWithDebug" => output(
            facade
                .complete_with_debug(
                    &request,
                    &options(&args["options"]),
                    args["languageId"].as_str().unwrap(),
                )
                .unwrap(),
        ),
        "HostMixinServiceFacade.tryCompleteBufferOnly" => {
            let project = std::cell::RefCell::new(None::<Option<HostProjectKey>>);
            let lazy = || {
                if project.borrow().is_none() {
                    *project.borrow_mut() = Some(host_project(
                        &reads,
                        uri,
                        args["workspaceRootUri"].as_str(),
                        HostMixinFacade::is_kotlin_document(uri),
                    ));
                }
                project.borrow().as_ref().unwrap().clone()
            };
            let at = AtValueCompletionService::new(
                || {
                    let project = lazy();
                    (facade.specifier_supported)(project.as_ref(), 0).unwrap()
                },
                || {
                    let project = lazy();
                    (facade.additional_at_values)(project.as_ref(), 0).unwrap()
                },
            );
            output(
                HostMixinFacade::try_complete_buffer_only(
                    source,
                    request.line,
                    request.character,
                    &options(&args["options"]),
                    uri,
                    args["languageId"].as_str().unwrap(),
                    args["__hasFactory"].as_bool().unwrap(),
                    at,
                )
                .unwrap(),
            )
        }
        "HostMixinServiceFacade.definitions" => output(
            facade
                .definitions(
                    source,
                    request.line as usize,
                    request.character as usize,
                    uri,
                )
                .unwrap(),
        ),
        "HostMixinServiceFacade.analyzeDiagnostics" | "HostMixinServiceFacade.codeActions" => {
            let resources = ResourceProvider {
                reads: &reads,
                files: &args["session"]["files"],
                project: false,
            };
            if row["api"] == "HostMixinServiceFacade.analyzeDiagnostics" {
                output(facade.diagnose_document(source, uri, &resources).unwrap())
            } else {
                output(
                    facade
                        .code_actions_document(
                            source,
                            uri,
                            request.line,
                            request.character,
                            args["diagnosticCode"].as_str(),
                            &resources,
                        )
                        .unwrap(),
                )
            }
        }
        "HostMixinServiceFacade.collectRegisteredMixinSources" => {
            let resources = ResourceProvider {
                reads: &reads,
                files: &args["files"],
                project: !args["javaProject"].is_null(),
            };
            output(
                HostMixinFacade::collect_registered_mixin_sources(
                    context.as_ref().unwrap(),
                    source,
                    uri,
                    &resources,
                )
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            )
        }
        api => panic!("unknown host facade API {api}"),
    };
    if row["api"] == "HostMixinServiceFacade.tryCompleteBufferOnly" {
        let id = row["legacyCase"].as_str().unwrap();
        let permitted = [
            (
                "bufferOnlyWrapCompletionOmitsUnknownWrapWithConditionVariants",
                2,
            ),
            (
                "definitionIdCompletionDoesNotLeakAdjacentHandlerDefinitions",
                8,
            ),
            ("definitionIdCompletionWorksWithoutJavaProjectResolver", 8),
        ];
        let mut internal = 0;
        let external:Vec<_>=reads.rows.iter().filter(|r|{if r["api"].as_str().unwrap().starts_with("ClassIndex."){assert!(permitted.iter().any(|(case,_)|id.ends_with(case)),"only the three reviewed private singleton cases may normalize internal empty reads");assert!(r["output"].is_null()||r["output"]==json!([]),"the legacy buffer-only singleton must always be empty");internal+=1;false}else{true}}).map(|r|json!({"api":r["api"],"args":r["args"]})).collect();
        assert_eq!(
            internal,
            permitted
                .iter()
                .find(|(case, _)| id.ends_with(case))
                .map_or(0, |(_, count)| *count)
        );
        assert_eq!(
            *reads.calls.lock().unwrap(),
            external,
            "buffer-only external provider callbacks"
        );
    } else {
        reads.assert_trace();
    }
    let mut expected = row["output"].clone();
    source_replay::normalize_expected_offsets(&mut expected, source, "");
    (canonical(actual, ""), canonical(expected, ""))
}
impl ClassIndex for Reads<'_> {
    fn find_classes(&self, prefix: &str, limit: usize) -> Vec<ClassIndexEntry> {
        serde_json::from_value(
            self.get("ClassIndex.findClasses", json!([prefix, limit]))
                .clone(),
        )
        .unwrap()
    }
    fn find_class(&self, name: &str) -> Option<ClassIndexEntry> {
        serde_json::from_value(self.get("ClassIndex.findClass", json!([name])).clone()).unwrap()
    }
    fn find_class_by_fqn(&self, fqn: &str) -> Option<ClassIndexEntry> {
        serde_json::from_value(self.get("ClassIndex.findClassByFqn", json!([fqn])).clone()).unwrap()
    }
    fn get_methods(&self, owner: &str) -> Vec<MethodIndexEntry> {
        serde_json::from_value(self.get("ClassIndex.getMethods", json!([owner])).clone()).unwrap()
    }
    fn get_fields(&self, owner: &str) -> Vec<FieldIndexEntry> {
        serde_json::from_value(self.get("ClassIndex.getFields", json!([owner])).clone()).unwrap()
    }
}
impl BytecodeIndex for Reads<'_> {
    fn get_at_target_candidates(
        &self,
        owner: &str,
        name: &str,
        descriptor: Option<&str>,
        at_value: &str,
    ) -> Vec<AtTargetCandidate> {
        decode(self.get(
            "BytecodeIndex.getAtTargetCandidates",
            json!([owner, name, descriptor, at_value]),
        ))
    }
    fn get_return_ordinal_count(&self, owner: &str, name: &str, descriptor: Option<&str>) -> usize {
        self.get(
            "BytecodeIndex.getReturnOrdinalCount",
            json!([owner, name, descriptor]),
        )
        .as_u64()
        .unwrap() as usize
    }
    fn get_class_bytes(&self, owner: &str) -> Option<Vec<u8>> {
        let value = self.get("BytecodeIndex.getClassBytes", json!([owner]));
        (!value.is_null()).then(|| {
            value
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap() as u8)
                .collect()
        })
    }
    fn resolve_common_super_class(&self, left: &str, right: &str) -> Option<String> {
        self.get(
            "BytecodeIndex.resolveCommonSuperClass",
            json!([left, right]),
        )
        .as_str()
        .map(str::to_owned)
    }
}
impl OperationCallArgumentTypeResolver for Reads<'_> {
    fn resolve_type(&self, expression: &str) -> Option<String> {
        self.get(
            "OperationCallArgumentTypeResolver.resolveType",
            json!([expression]),
        )
        .as_str()
        .map(str::to_owned)
    }
}
#[path = "../mixin_source/replay.rs"]
mod source_replay;
fn request(value: &Value) -> MixinFacadeRequest {
    MixinFacadeRequest {
        buffer_text: value["bufferText"].as_str().unwrap().into(),
        line: value["line"].as_i64().unwrap() as i32,
        character: value["character"].as_i64().unwrap() as i32,
        document_uri: value["documentUri"].as_str().unwrap().into(),
        mixin_class_name: value["mixinClassName"].as_str().map(str::to_string),
        mixin_package: value["mixinPackage"].as_str().map(str::to_string),
        mixin_config_content: value["mixinConfigContent"].as_str().map(str::to_string),
        mixin_config_path: value["mixinConfigPath"].as_str().map(str::to_string),
        semantic_model: (!value["semanticModel"].is_null())
            .then(|| source_replay::deserialize(&value["semanticModel"])),
    }
}
fn options(value: &Value) -> MixinCompletionOptions {
    MixinCompletionOptions {
        class_insert_mode: match value["classInsertMode"].as_str().unwrap() {
            "IMPORT" => MixinClassInsertMode::Import,
            "FQN" => MixinClassInsertMode::Fqn,
            v => panic!("mode {v}"),
        },
        inject_method_descriptor_mode: match value["injectMethodDescriptorMode"].as_str().unwrap() {
            "AUTO" => InjectMethodDescriptorMode::Auto,
            "ALWAYS" => InjectMethodDescriptorMode::Always,
            "NEVER" => InjectMethodDescriptorMode::Never,
            v => panic!("mode {v}"),
        },
        ..Default::default()
    }
}
fn replay_facade(row: &Value) -> (Value, Value) {
    let args = &row["args"];
    let request = request(&args["request"]);
    let reads = Reads {
        rows: row["reads"].as_array().unwrap(),
        calls: Default::default(),
    };
    let at = recorded_at(&reads);
    let facade = MixinServiceFacade::new(&reads, &reads)
        .with_reference_types(&reads)
        .with_at_value_completion(at)
        .with_share_sources(|| {
            reads.get("Share.sources", json!([]));
            let sources = reads
                .rows
                .iter()
                .filter(|r| r["api"] == "Share.source")
                .map(|r| r["output"].as_str().unwrap().to_string());
            Box::new(sources.map(|_| {
                reads
                    .get("Share.source", json!([]))
                    .as_str()
                    .unwrap()
                    .to_string()
            }))
        });
    if row.get("throwable").is_some() {
        use mcdev_core::expression_engine as engine;
        let cancellation = engine::Cancellation(std::sync::Arc::new(std::io::Error::other(
            "facade local capture cancelled",
        )));
        let expected = cancellation.clone();
        let checker = std::sync::Arc::new(move || Err(cancellation.clone()));
        engine::with_cancellation(checker, || {
            let error = match row["api"].as_str().unwrap() {
                "CoreMixinServiceFacade.complete" => facade
                    .complete(&request, &options(&args["options"]))
                    .unwrap_err(),
                "CoreMixinServiceFacade.completeWithDebug" => facade
                    .complete_with_debug(
                        &request,
                        &options(&args["options"]),
                        args["command"].as_str().unwrap(),
                        args["languageId"].as_str().unwrap(),
                    )
                    .unwrap_err(),
                api => panic!("unexpected throwable {api}"),
            };
            assert!(
                std::sync::Arc::ptr_eq(&error.0, &expected.0),
                "the exact cancellation identity must propagate"
            );
        });
        reads.assert_trace();
        return (
            json!("propagatedCancellation"),
            json!("propagatedCancellation"),
        );
    }
    let actual = match row["api"].as_str().unwrap() {
        "CoreMixinServiceFacade.complete" => output(
            facade
                .complete(&request, &options(&args["options"]))
                .unwrap(),
        ),
        "CoreMixinServiceFacade.completeWithDebug" => output(
            facade
                .complete_with_debug(
                    &request,
                    &options(&args["options"]),
                    args["command"].as_str().unwrap(),
                    args["languageId"].as_str().unwrap(),
                )
                .unwrap(),
        ),
        "CoreMixinServiceFacade.diagnose" => output(facade.diagnose(&request).unwrap()),
        "CoreMixinServiceFacade.codeActions" => output(
            facade
                .code_actions(&request, args["diagnosticCode"].as_str())
                .unwrap(),
        ),
        other => panic!("unknown facade {other}"),
    };
    reads.assert_trace();
    let mut expected = row["output"].clone();
    source_replay::normalize_expected_offsets(&mut expected, &request.buffer_text, "");
    (canonical(actual, ""), canonical(expected, ""))
}
fn replay_tail(row: &Value) -> (Value, Value) {
    let args = &row["args"];
    let reads = Reads {
        rows: row["reads"].as_array().unwrap(),
        calls: Default::default(),
    };
    let source = args["source"]
        .as_str()
        .or_else(|| args["currentSource"].as_str())
        .or_else(|| args["currentContent"].as_str())
        .unwrap_or("");
    let actual = match row["api"].as_str().unwrap() {
        "HandlerSignatureService.findAnnotationSites" => {
            output(HandlerSignatureService::find_annotation_sites(source))
        }
        "HandlerSignatureService.findSugarHandlerAnnotationSites" => {
            output(HandlerSignatureService::find_sugar_handler_annotation_sites(source))
        }
        "MixinDefinitionService.definitionsAtOffset" => {
            let model = args
                .get("semanticModel")
                .filter(|m| !m.is_null())
                .map(source_replay::deserialize);
            output(
                mcdev_core::mixin_navigation::MixinDefinitionService::new(&reads, &reads)
                    .definitions_at_offset(
                        source,
                        utf16_offset(source, args["offset"].as_u64().unwrap() as usize).unwrap(),
                        model.as_ref(),
                        args["documentUri"].as_str(),
                    ),
            )
        }
        "MixinDefinitionService.definitionsAt" => {
            let model = args
                .get("semanticModel")
                .filter(|m| !m.is_null())
                .map(source_replay::deserialize);
            output(
                mcdev_core::mixin_navigation::MixinDefinitionService::new(&reads, &reads)
                    .definitions_at(
                        source,
                        args["line"].as_u64().unwrap() as usize,
                        args["character"].as_u64().unwrap() as usize,
                        model.as_ref(),
                        args["documentUri"].as_str(),
                    ),
            )
        }
        "MixinCodeActionService.applyMixinConfigFix"
        | "MixinCodeActionService.applyMethodDescriptorFix" => {
            let mut fix = args["fix"].clone();
            source_replay::normalize_expected_offsets(&mut fix, source, "");
            let fix: McFix = serde_json::from_value(fix).unwrap();
            let actual = if row["api"] == "MixinCodeActionService.applyMixinConfigFix" {
                mcdev_core::mixin_actions::MixinCodeActionService::apply_mixin_config_fix(
                    &fix, source,
                )
            } else {
                mcdev_core::mixin_actions::MixinCodeActionService::apply_method_descriptor_fix(
                    &fix, source,
                )
            };
            if let Some(McFix::WorkspaceEditFix { edits, .. }) = &actual {
                let mut expected = row["output"].clone();
                source_replay::normalize_expected_offsets(&mut expected, source, "");
                let expected: McFix = serde_json::from_value(expected).unwrap();
                let McFix::WorkspaceEditFix { edits: e, .. } = expected else {
                    panic!("expected applied fix")
                };
                assert_eq!(
                    apply_text_edits(source, edits),
                    apply_text_edits(source, &e),
                    "actual materialized document"
                );
            }
            output(actual)
        }
        api => panic!("tail API {api}"),
    };
    reads.assert_trace();
    let mut expected = row["output"].clone();
    source_replay::normalize_expected_offsets(&mut expected, source, "");
    (canonical(actual, ""), canonical(expected, ""))
}
fn canonical(mut value: Value, key: &str) -> Value {
    if key == "parseConfidence" {
        return value
            .as_str()
            .map(|s| json!(s.to_ascii_lowercase()))
            .unwrap_or(value);
    }
    if key == "metadata" {
        return value;
    }
    match &mut value {
        Value::Object(map) => {
            if map.contains_key("type") && !map.contains_key("kind") {
                if let Ok(fix) = serde_json::from_value::<McFix>(Value::Object(map.clone())) {
                    map.insert("kind".into(), json!(fix.kind()));
                }
            }
            let taken = std::mem::take(map);
            for (k, v) in taken {
                map.insert(k.clone(), canonical(v, &k));
            }
        }
        Value::Array(a) => {
            for v in a {
                *v = canonical(v.take(), key);
            }
        }
        _ => {}
    }
    source_replay::canonical(value, key)
}
pub fn run_core() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("core-cases.json")).unwrap();
    assert_eq!(cases.len(), 159);
    assert!(cases.iter().all(|c| c["status"] == "passed"));
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    let mut failures = vec![];
    let mut facade_count = 0;
    for (line, text) in include_str!("core-calls.jsonl").lines().enumerate() {
        let row: Value = serde_json::from_str(text).unwrap();
        let id = row["legacyCase"].as_str().unwrap();
        if std::env::var("MCDEV_FACADE_CASE")
            .ok()
            .is_some_and(|filter| !id.ends_with(&filter))
        {
            continue;
        }
        if row.get("throwable").is_none() {
            *counts.entry(id.into()).or_default() += 1;
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let api = row["api"].as_str().unwrap();
            if api.starts_with("CoreMixinServiceFacade.") {
                facade_count += 1;
                replay_facade(&row)
            } else if api.starts_with("MixinDefinitionService.")
                || api.starts_with("MixinCodeActionService.")
                || api.starts_with("HandlerSignatureService.")
            {
                replay_tail(&row)
            } else {
                let actual = source_replay::replay(&row);
                assert!(
                    source_replay::kinds_match(&row, &actual),
                    "business variant"
                );
                (
                    source_replay::canonical(actual, ""),
                    source_replay::expected_output(&row),
                )
            }
        }));
        match result {Ok((actual,expected))if actual==expected=>{},Ok((actual,expected))=>failures.push(json!({"line":line+1,"case":id,"api":row["api"],"actual":actual,"expected":expected})),Err(error)=>{let message=error.downcast_ref::<String>().cloned().or_else(||error.downcast_ref::<&str>().map(|s|s.to_string())).unwrap_or_else(||"non-string panic".into());failures.push(json!({"line":line+1,"case":id,"api":row["api"],"panic":message}));}}
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../build/rust-test-logs/facade-failures.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, serde_json::to_string_pretty(&failures).unwrap()).unwrap();
    for case in &cases {
        let id = case["legacyCase"].as_str().unwrap();
        if std::env::var("MCDEV_FACADE_CASE")
            .ok()
            .is_some_and(|filter| !id.ends_with(&filter))
        {
            continue;
        }
        assert_eq!(
            counts.get(id).copied().unwrap_or(0),
            case["calls"].as_u64().unwrap() as usize,
            "all original calls {id}"
        );
    }
    assert!(facade_count > 0);
    assert!(failures.is_empty(),"{} failures / {facade_count} final facade + {} total outputs; see build/rust-test-logs/facade-failures.json",failures.len(),counts.values().sum::<usize>());
}
pub fn run_host() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("host-cases.json")).unwrap();
    assert_eq!(cases.len(), 30);
    assert!(cases.iter().all(|c| c["status"] == "passed"));
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    let mut failures = vec![];
    for (line, text) in include_str!("host-calls.jsonl").lines().enumerate() {
        let row: Value = serde_json::from_str(text).unwrap();
        let id = row["legacyCase"].as_str().unwrap();
        *counts.entry(id.into()).or_default() += 1;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let api = row["api"].as_str().unwrap();
            if api.starts_with("HostMixinServiceFacade.") {
                replay_host(&row)
            } else if api.starts_with("CoreMixinServiceFacade.") {
                replay_facade(&row)
            } else if api.starts_with("MixinDefinitionService.")
                || api.starts_with("MixinCodeActionService.")
                || api.starts_with("HandlerSignatureService.")
            {
                replay_tail(&row)
            } else {
                let actual = source_replay::replay(&row);
                assert!(
                    source_replay::kinds_match(&row, &actual),
                    "business variant"
                );
                (
                    source_replay::canonical(actual, ""),
                    source_replay::expected_output(&row),
                )
            }
        }));
        match result{Ok((a,e))if a==e=>{},Ok((actual,expected))=>failures.push(json!({"line":line+1,"case":id,"api":row["api"],"actual":actual,"expected":expected})),Err(e)=>{let message=e.downcast_ref::<String>().cloned().or_else(||e.downcast_ref::<&str>().map(|s|s.to_string())).unwrap_or_else(||"non-string panic".into());failures.push(json!({"line":line+1,"case":id,"api":row["api"],"panic":message}));}}
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../build/rust-test-logs/host-facade-failures.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, serde_json::to_string_pretty(&failures).unwrap()).unwrap();
    for case in &cases {
        let id = case["legacyCase"].as_str().unwrap();
        assert_eq!(
            counts.get(id).copied().unwrap_or(0),
            case["calls"].as_u64().unwrap() as usize,
            "all original host calls {id}"
        );
    }
    assert!(failures.is_empty(),"{} host failures across30cases/{}events; see build/rust-test-logs/host-facade-failures.json",failures.len(),counts.values().sum::<usize>());
}
pub fn cancel_recovery() {
    use mcdev_core::expression_engine as engine;
    let rows: Vec<Value> = include_str!("core-calls.jsonl")
        .lines()
        .map(|t| serde_json::from_str::<Value>(t).unwrap())
        .filter(|r| {
            r["legacyCase"]
                .as_str()
                .unwrap()
                .ends_with("cancelledLocalCaptureCompletionPropagatesAndNextRequestRecovers")
                && r["api"] == "CoreMixinServiceFacade.complete"
        })
        .collect();
    assert_eq!(rows.len(), 2);
    let read_rows: Vec<_> = rows
        .iter()
        .flat_map(|r| r["reads"].as_array().unwrap().clone())
        .collect();
    let reads = Reads {
        rows: &read_rows,
        calls: Default::default(),
    };
    let facade = MixinServiceFacade::new(&reads, &reads).with_reference_types(&reads);
    let request = request(&rows[0]["args"]["request"]);
    let options = options(&rows[0]["args"]["options"]);
    let cancelled = engine::Cancellation(std::sync::Arc::new(std::io::Error::other(
        "facade local capture cancelled",
    )));
    let expected = cancelled.clone();
    engine::with_cancellation(std::sync::Arc::new(move || Err(cancelled.clone())), || {
        let error = facade.complete(&request, &options).unwrap_err();
        assert!(std::sync::Arc::ptr_eq(&error.0, &expected.0));
    });
    let recovered = facade.complete(&request, &options).unwrap();
    assert_eq!(
        recovered
            .iter()
            .map(|i| i.insert_text.as_str())
            .collect::<Vec<_>>(),
        ["0", "1"]
    );
    assert_eq!(
        canonical(output(recovered), ""),
        canonical(rows[1]["output"].clone(), "")
    );
    reads.assert_trace();
}
struct LiveFiles {
    units: std::sync::Mutex<std::collections::BTreeMap<String, Value>>,
    file_reads: std::sync::Mutex<Vec<std::path::PathBuf>>,
    config_reads: std::sync::Mutex<Vec<std::path::PathBuf>>,
    unit_reads: std::sync::Mutex<Vec<String>>,
}
impl HostMixinSourceProvider for LiveFiles {
    fn config(
        &self,
        path: &std::path::Path,
    ) -> HostSourceResult<mcdev_core::project::MixinConfigRef> {
        self.config_reads.lock().unwrap().push(path.into());
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(HostAnswer::missing(vec![]))
            }
            Err(e) => return Err(HostSourceError::Failure(e.to_string())),
        };
        let value =
            mcdev_core::mixin_edits::MixinConfigEditor::parse(&text, &path.to_string_lossy())
                .unwrap();
        Ok(HostAnswer::known(
            mcdev_core::project::MixinConfigRef {
                path: path.into(),
                package_name: value.package_name,
                mixins: value.mixins,
                client: value.client,
                server: value.server,
                common: value.common,
            },
            vec![],
        ))
    }
    fn compilation_unit_source(
        &self,
        fqn: &str,
        _current: Option<&std::path::Path>,
    ) -> HostSourceResult<RegisteredMixinSource> {
        self.unit_reads.lock().unwrap().push(fqn.into());
        let units = self.units.lock().unwrap();
        let Some(value) = units.get(fqn) else {
            return Ok(HostAnswer::missing(vec![]));
        };
        Ok(match value["status"].as_str().unwrap() {
            "missing" => HostAnswer::missing(vec![]),
            "current" => HostAnswer::known(RegisteredMixinSource::Current, vec![]),
            "source" => HostAnswer::known(
                RegisteredMixinSource::Source(value["text"].as_str().unwrap().into()),
                vec![],
            ),
            s => panic!("unit status {s}"),
        })
    }
    fn read_source_file(&self, path: &std::path::Path) -> HostSourceResult<String> {
        self.file_reads.lock().unwrap().push(path.into());
        match std::fs::read_to_string(path) {
            Ok(text) => Ok(HostAnswer::known(text, vec![])),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(HostAnswer::missing(vec![])),
            Err(e) => Err(HostSourceError::Failure(e.to_string())),
        }
    }
}
pub fn registered_files() {
    let rows: Vec<Value> = include_str!("host-calls.jsonl")
        .lines()
        .map(|t| serde_json::from_str::<Value>(t).unwrap())
        .filter(|r| r["api"] == "HostMixinServiceFacade.collectRegisteredMixinSources")
        .collect();
    assert_eq!(rows.len(), 6);
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../build/mixin-facade-native-files")
        .join(format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    let base = mcdev_core::project::normalize_path(&base);
    std::fs::create_dir_all(&base).unwrap();
    let provider = LiveFiles {
        units: Default::default(),
        file_reads: Default::default(),
        config_reads: Default::default(),
        unit_reads: Default::default(),
    };
    let mut prior =
        std::collections::BTreeMap::<String, std::collections::BTreeSet<std::path::PathBuf>>::new();
    for row in rows {
        let args = &row["args"];
        let old = std::path::Path::new(args["projectContext"]["root"].as_str().unwrap());
        let key = old.to_string_lossy().into_owned();
        let root = base.join(format!(
            "root-{}",
            prior.keys().position(|k| k == &key).unwrap_or(prior.len())
        ));
        let remap = |p: &std::path::Path| root.join(p.strip_prefix(old).unwrap());
        let mut context = host_context(&args["projectContext"]);
        context.root = root.clone();
        for set in &mut context.source_sets {
            for p in set
                .source_directories
                .iter_mut()
                .chain(&mut set.resource_directories)
            {
                *p = remap(p);
            }
        }
        for config in &mut context.mixin_configs {
            config.path = remap(&config.path);
        }
        let mut paths = std::collections::BTreeSet::new();
        for (path, text) in args["files"].as_object().unwrap() {
            let path = remap(std::path::Path::new(path));
            assert!(path.starts_with(&base));
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, text.as_str().unwrap()).unwrap();
            paths.insert(path);
        }
        if let Some(previous) = prior.get(&key) {
            for path in previous.difference(&paths) {
                assert!(path.starts_with(&base));
                std::fs::remove_file(path).unwrap();
            }
        }
        prior.insert(key, paths);
        let units = row["reads"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["api"] == "Host.unitSource")
            .map(|r| {
                (
                    r["args"][0].as_str().unwrap().to_string(),
                    r["output"].clone(),
                )
            })
            .collect();
        *provider.units.lock().unwrap() = units;
        provider.file_reads.lock().unwrap().clear();
        let uri = mcdev_core::project::uri_to_path(args["documentUri"].as_str().unwrap())
            .ok()
            .map(|p| mcdev_core::project::path_to_uri(&remap(&p)))
            .unwrap();
        if row["legacyCase"]
            .as_str()
            .unwrap()
            .ends_with("kotlinSourcesUseSemicolonlessPackagesForConfigAndRegisteredShareLookup")
        {
            let source = args["source"].as_str().unwrap();
            provider.config_reads.lock().unwrap().clear();
            provider.unit_reads.lock().unwrap().clear();
            let selected = HostMixinFacade::selected_mixin_config(
                &context,
                &source.replace("CurrentMixin", "UnlistedMixin"),
                Some(&uri),
                &provider,
            )
            .unwrap();
            assert_eq!(selected.as_ref(), context.mixin_configs.first());
            assert!(provider.config_reads.lock().unwrap().is_empty());
            assert!(provider.unit_reads.lock().unwrap().is_empty());
            assert!(provider.file_reads.lock().unwrap().is_empty());
            // The host supplies the original source-set file inventory. The
            // MC boundary merges it with the authoritative unsaved buffer.
            let entries = args["files"]
                .as_object()
                .unwrap()
                .iter()
                .filter_map(|(path, _)| {
                    let path = remap(std::path::Path::new(path));
                    matches!(
                        path.extension().and_then(|ext| ext.to_str()),
                        Some("kt" | "kts")
                    )
                    .then(|| SourceScanEntry {
                        document_uri: mcdev_core::project::path_to_uri(&path),
                        text: std::fs::read_to_string(path).unwrap(),
                    })
                });
            let actual = HostMixinFacade::collect_source_entries(&uri, source, entries)
                .into_iter()
                .map(|entry| entry.text)
                .collect::<std::collections::BTreeSet<_>>();
            let expected = std::iter::once(source.to_string())
                .chain(
                    row["output"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| value.as_str().unwrap().into()),
                )
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(actual, expected);
        }
        let actual = HostMixinFacade::collect_registered_mixin_sources(
            &context,
            args["source"].as_str().unwrap(),
            &uri,
            &provider,
        )
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
        assert_eq!(
            output(actual),
            row["output"],
            "real live files {}",
            row["legacyCase"]
        );
    }
}

pub fn original_edit_tails() {
    let mut snippet_boundaries = 0;
    let mut workspace_applications = 0;
    for text in include_str!("core-calls.jsonl").lines() {
        let row: Value = serde_json::from_str(text).unwrap();
        let id = row["legacyCase"].as_str().unwrap();
        let snippet = id
            .ends_with("facadeNormalizesDeclarationSnippetIndentationAtBothInsertionBoundaries")
            && row["api"] == "CoreMixinServiceFacade.complete";
        let workspace = id.ends_with("codeActionFixesWrapOperationBadReturnThroughFacade")
            && row["api"] == "CoreMixinServiceFacade.codeActions";
        if !snippet && !workspace {
            continue;
        }
        let (actual, expected) = replay_facade(&row);
        assert_eq!(actual, expected);
        let request = request(&row["args"]["request"]);
        let source = &request.buffer_text;
        if snippet {
            let items = actual.as_array().unwrap();
            let insert = items
                .iter()
                .find(|item| item["metadata"]["source"] == "mixin.declaration")
                .unwrap()["insertText"]
                .as_str()
                .unwrap();
            let offset = position_to_offset(
                source,
                McTextPosition {
                    line: request.line as usize,
                    character: request.character as usize,
                },
            )
            .unwrap();
            let applied = format!("{}{}{}", &source[..offset], insert, &source[offset..]);
            if snippet_boundaries == 0 {
                assert!(insert.starts_with("\n    public"));
                assert!(applied.contains("\n    public"));
            } else {
                assert!(insert.starts_with("public"));
                assert!(applied.contains("    public"));
            }
            assert!(insert.contains("\n        $0\n        throw new AssertionError();\n    }"));
            snippet_boundaries += 1;
        } else {
            let fixes = actual.as_array().unwrap();
            let fix = fixes
                .iter()
                .find(|fix| fix["title"] == "Fix WrapOperation handler signature")
                .unwrap();
            let edits: Vec<McTextEdit> = serde_json::from_value(fix["edits"].clone()).unwrap();
            let edit = edits
                .iter()
                .find(|edit| edit.new_text.contains("mcdevWrapLength"))
                .unwrap();
            assert!(!edit.new_text.contains("return original.call(instance);"));
            let applied = apply_text_edits(source, &edits).unwrap();
            assert!(applied.contains(
                "private int mcdevWrapLength(String instance, Operation<Integer> original)"
            ));
            assert!(applied.contains("original.call(instance);"));
            workspace_applications += 1;
        }
    }
    assert_eq!(snippet_boundaries, 2);
    assert_eq!(workspace_applications, 1);
}
struct NeverBinary;
impl ClassIndex for NeverBinary {
    fn find_classes(&self, _p: &str, _l: usize) -> Vec<ClassIndexEntry> {
        panic!("binary class catalog touched")
    }
    fn find_class(&self, _p: &str) -> Option<ClassIndexEntry> {
        panic!("binary class lookup touched")
    }
    fn find_class_by_fqn(&self, _p: &str) -> Option<ClassIndexEntry> {
        panic!("binary FQN lookup touched")
    }
    fn get_methods(&self, _p: &str) -> Vec<MethodIndexEntry> {
        panic!("binary methods touched")
    }
    fn get_fields(&self, _p: &str) -> Vec<FieldIndexEntry> {
        panic!("binary fields touched")
    }
}
impl BytecodeIndex for NeverBinary {
    fn get_at_target_candidates(
        &self,
        _o: &str,
        _n: &str,
        _d: Option<&str>,
        _v: &str,
    ) -> Vec<AtTargetCandidate> {
        panic!("binary candidates touched")
    }
    fn get_return_ordinal_count(&self, _o: &str, _n: &str, _d: Option<&str>) -> usize {
        panic!("binary ordinals touched")
    }
    fn get_class_bytes(&self, _o: &str) -> Option<Vec<u8>> {
        panic!("binary bytes touched")
    }
}
struct CompleteSource {
    classes: std::sync::atomic::AtomicUsize,
    methods: std::sync::atomic::AtomicUsize,
}
impl CompleteSource {
    fn entry() -> ClassIndexEntry {
        ClassIndexEntry {
            simple_name: "TargetTarget".into(),
            package_name: "demo".into(),
            internal_name: "demo/TargetTarget".into(),
        }
    }
    fn answer<T>(v: T) -> Result<SourceQueryResult<T>, HostSourceError> {
        Ok(SourceQueryResult::new(v, SourceAuthority::Complete))
    }
}
impl ProjectSourceQuery for CompleteSource {
    fn covers_project_dependencies(&self) -> bool {
        true
    }
    fn find_classes(
        &self,
        prefix: &str,
        _limit: usize,
    ) -> Result<SourceQueryResult<Vec<ClassIndexEntry>>, HostSourceError> {
        self.classes
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self::answer(if "TargetTarget".starts_with(prefix) {
            vec![Self::entry()]
        } else {
            vec![]
        })
    }
    fn find_class(
        &self,
        name: &str,
    ) -> Result<SourceQueryResult<Option<ClassIndexEntry>>, HostSourceError> {
        Self::answer((name == "demo/TargetTarget").then(Self::entry))
    }
    fn find_class_by_fqn(
        &self,
        name: &str,
    ) -> Result<SourceQueryResult<Option<ClassIndexEntry>>, HostSourceError> {
        Self::answer((name == "demo.TargetTarget").then(Self::entry))
    }
    fn get_methods(
        &self,
        owner: &str,
    ) -> Result<SourceQueryResult<Vec<MethodIndexEntry>>, HostSourceError> {
        self.methods
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self::answer(if owner == "demo/TargetTarget" {
            vec![MethodIndexEntry {
                name: "tick".into(),
                descriptor: "()V".into(),
                is_static: false,
                readable_signature: "tick(): void".into(),
            }]
        } else {
            vec![]
        })
    }
    fn get_fields(
        &self,
        _owner: &str,
    ) -> Result<SourceQueryResult<Vec<FieldIndexEntry>>, HostSourceError> {
        Self::answer(vec![])
    }
}
pub fn source_authority() {
    let source = std::sync::Arc::new(CompleteSource {
        classes: Default::default(),
        methods: Default::default(),
    });
    let index = SourceBackedClassIndex::new(std::sync::Arc::new(NeverBinary), source.clone());
    let reads = Reads {
        rows: &[],
        calls: Default::default(),
    };
    let provider = HostProvider { reads: &reads };
    let cache = SemanticModelCache::default();
    let facade = HostMixinServiceFacade::new(&index, &NeverBinary, &provider, &cache);
    let mut count = 0;
    for row in include_str!("host-calls.jsonl")
        .lines()
        .map(|t| serde_json::from_str::<Value>(t).unwrap())
        .filter(|r| {
            r["legacyCase"].as_str().unwrap().ends_with(
                "mixinClassAndInjectMethodCompletionUseAuthoritativeSourceQueryWithoutClasspath",
            ) && r["api"] == "HostMixinServiceFacade.complete"
        })
    {
        let a = &row["args"];
        let request = MixinFacadeRequest {
            buffer_text: a["source"].as_str().unwrap().into(),
            line: a["line"].as_i64().unwrap() as i32,
            character: a["character"].as_i64().unwrap() as i32,
            document_uri: a["documentUri"].as_str().unwrap().into(),
            semantic_model: Some(source_replay::deserialize(&a["semanticModel"])),
            ..Default::default()
        };
        let items = facade.complete(&request, &options(&a["options"])).unwrap();
        assert_eq!(
            canonical(output(items), ""),
            canonical(row["output"].clone(), "")
        );
        count += 1;
    }
    assert_eq!(count, 5);
    assert!(source.classes.load(std::sync::atomic::Ordering::SeqCst) >= 3);
    assert!(source.methods.load(std::sync::atomic::Ordering::SeqCst) >= 2);
}
pub fn utf16_oracle() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("utf16-cases.json")).unwrap();
    assert!(cases.iter().all(|c| c["status"] == "passed"));
    let mut count = 0;
    for text in include_str!("utf16-calls.jsonl").lines() {
        let row: Value = serde_json::from_str(text).unwrap();
        let args = &row["args"];
        let source = args["source"].as_str().unwrap();
        let reads = Reads {
            rows: row["reads"].as_array().unwrap(),
            calls: Default::default(),
        };
        let actual = match row["api"].as_str().unwrap() {
            "Utf16Facade.context" => {
                let site = HandlerSignatureService::find_annotation_sites(source)
                    .into_iter()
                    .next()
                    .unwrap();
                output(mcdev_core::mixinextras_expression::ExpressionContextResolver::resolve_expression_context(source,&site).values_for_at_id_utf16(None))
            }
            "Utf16Facade.prefix" => {
                let offset = source_replay::byte(source, args["cursor"].as_i64().unwrap()).unwrap();
                let context =
                    mcdev_core::mixin_source::AnnotationContextExtractor::extract_at_offset(
                        source, offset,
                    )
                    .unwrap();
                assert_eq!(
                    context.expression_completion_position,
                    Some(mcdev_core::mixin_source::ExpressionCompletionPosition::AfterDot)
                );
                let (units, cursor) = if let Some(prefix) = &context.decoded_expression_prefix_utf16
                {
                    (prefix.prefix.clone(), prefix.cursor)
                } else {
                    let prefix = context.decoded_expression_prefix.as_ref().unwrap();
                    (
                        prefix.prefix.encode_utf16().collect(),
                        prefix.prefix[..prefix.cursor].encode_utf16().count(),
                    )
                };
                json!({"units":units,"cursor":cursor,"startOffset":context.value_start_offset,"endOffset":context.value_end_offset,"position":"AFTER_DOT"})
            }
            "Utf16Facade.complete" => {
                let request = MixinFacadeRequest {
                    buffer_text: source.into(),
                    line: args["line"].as_i64().unwrap() as i32,
                    character: args["character"].as_i64().unwrap() as i32,
                    document_uri: args["documentUri"].as_str().unwrap().into(),
                    ..Default::default()
                };
                let result = MixinServiceFacade::new(&reads, &reads)
                    .with_reference_types(&reads)
                    .complete_with_debug(
                        &request,
                        &MixinCompletionOptions::default(),
                        "mcdev.completion",
                        if args["kotlin"] == true {
                            "kotlin"
                        } else {
                            "java"
                        },
                    )
                    .unwrap();
                let context = mcdev_core::mixin_source::AnnotationContextExtractor::extract(
                    source,
                    request.line as usize,
                    request.character as usize,
                )
                .unwrap();
                let trim = result
                    .items
                    .iter()
                    .find(|i| i.metadata.name.as_deref() == Some("trim"))
                    .expect("official matching candidate");
                let mut edits = trim.additional_edits.clone();
                edits.push(McTextEdit {
                    start_offset: context.value_start_offset,
                    end_offset: context.value_end_offset,
                    new_text: trim.insert_text.clone(),
                });
                assert!(apply_text_edits(source, &edits).unwrap().contains("trim"));
                output(result)
            }
            api => panic!("UTF16 API {api}"),
        };
        reads.assert_trace();
        let mut expected = row["output"].clone();
        source_replay::normalize_expected_offsets(&mut expected, source, "");
        assert_eq!(
            canonical(actual, ""),
            canonical(expected, ""),
            "official UTF16 {args}"
        );
        count += 1;
    }
    assert_eq!(count, 30);
}
