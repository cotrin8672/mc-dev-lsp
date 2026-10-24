use mcdev_companion::commands::*;
use mcdev_core::{
    expression_engine::CancellationChecker,
    mapping::{MappingSet, MappingSetResolver},
    mixin_completion::MixinCompletionOptions,
    mixin_source::{AnnotationContext, MixinClassModel},
    project::{CachedProjectSession, ProjectContext},
    *,
};
use mcdev_protocol::{convert::ResolvedDefinition, *};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

pub fn parse<T: DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("fixture decode {e}: {v}"))
}
pub fn snake(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            if c.is_ascii_uppercase() {
                vec!['_', c.to_ascii_lowercase()]
            } else {
                vec![c]
            }
        })
        .collect()
}
fn pascal(s: &str) -> String {
    s.split('_')
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or(String::new(), |c| {
                c.to_ascii_uppercase().to_string() + &chars.as_str().to_ascii_lowercase()
            })
        })
        .collect()
}
fn source_input(v: &Value, key: &str) -> Value {
    if matches!(key, "metadata" | "explicit" | "ambiguousExplicit") {
        return v.clone();
    }
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, v)| (snake(k), source_input(v, k)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(|v| source_input(v, key)).collect()),
        Value::String(s)
            if matches!(
                key,
                "annotation"
                    | "slot"
                    | "parseSource"
                    | "confidence"
                    | "typeKind"
                    | "annotationKind"
                    | "parentInjectorAnnotation"
                    | "expressionCompletionPosition"
                    | "modifiers"
            ) =>
        {
            json!(pascal(s))
        }
        _ => v.clone(),
    }
}
pub fn parse_source<T: DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone())
        .or_else(|_| serde_json::from_value(source_input(v, "")))
        .unwrap_or_else(|e| panic!("source fixture decode {e}: {v}"))
}
fn bytes(source: &str, units: usize) -> usize {
    let mut count = 0;
    for (at, c) in source.char_indices() {
        if count == units {
            return at;
        }
        count += c.len_utf16();
    }
    // Unclosed legacy annotation extents may carry one virtual EOF delimiter.
    // Preserve it; edited ranges still pass the real converter's bounds checks.
    source.len() + units.checked_sub(count).expect("offset split a surrogate")
}
fn offsets(v: &mut Value, source: &str, key: &str) {
    match v {
        Value::Object(m) => {
            for (k, v) in m {
                if k != "metadata" {
                    offsets(v, source, k);
                }
            }
        }
        Value::Array(a) => {
            for v in a {
                offsets(v, source, key)
            }
        }
        Value::Number(n) if key.ends_with("Offset") => {
            if let Some(n) = n.as_u64() {
                *v = json!(bytes(source, n as usize));
            }
        }
        _ => (),
    }
}
fn text<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap()
}
fn project(v: &Value) -> Arc<ProjectContext> {
    let mut value = v.clone();
    value
        .as_object_mut()
        .unwrap()
        .entry("resourceIssues")
        .or_insert(json!([]));
    for key in ["accessTransformers", "accessWideners"] {
        if let Some(files) = value[key].as_array_mut() {
            for file in files {
                if let Some(entries) = file["parsed"]["entries"].as_array_mut() {
                    for entry in entries {
                        for field in ["modifier", "directive", "kind"] {
                            if let Some(name) = entry[field].as_str() {
                                entry[field] = json!(pascal(name));
                            }
                        }
                    }
                }
            }
        }
    }
    let mut project: ProjectContext = parse(&value);
    if let Some(mappings) = value["mappings"]["resolver"].get("mappings") {
        project.mappings.resolver =
            Arc::new(MappingSetResolver::new(parse::<MappingSet>(mappings)));
    }
    Arc::new(project)
}
fn completion(value: &Value, source: &str, context: Option<&Value>) -> RuntimeCompletion {
    let mut replacement = value["replacementRange"].clone();
    offsets(&mut replacement, source, "");
    let annotation_context = context.filter(|v| !v.is_null()).map(|v| {
        let mut v = v.clone();
        offsets(&mut v, source, "");
        parse_source::<AnnotationContext>(&v)
    });
    RuntimeCompletion {
        items: parse(&value["items"]),
        replacement_range: parse(&replacement),
        annotation_context,
        debug: parse(&value["debug"]),
    }
}
pub struct RecordedHost {
    row: Mutex<Value>,
    pub calls: Mutex<Vec<String>>,
    pub buffer_delay_millis: AtomicU64,
    pub session_failure: Mutex<Option<CommandFailure>>,
}
impl RecordedHost {
    pub fn new() -> Self {
        Self {
            row: Mutex::new(Value::Null),
            calls: Mutex::new(vec![]),
            buffer_delay_millis: AtomicU64::new(0),
            session_failure: Mutex::new(None),
        }
    }
    pub fn set(&self, row: &Value) {
        *self.row.lock().unwrap() = row.clone();
        self.calls.lock().unwrap().clear();
    }
    pub fn assert_host_calls(&self) {
        const HOST_APIS: &[&str] = &[
            "McdevBuildInfo.load",
            "mixinFacade.hasJavaProject",
            "projectService.loadSession",
            "projectService.loadCachedSession",
            "projectService.reindex",
            "projectService.clearCache",
            "semanticCache.get",
            "mixinFacade.tryCompleteBufferOnly",
            "mixinFacade.completeWithDebug",
            "mixinFacade.candidateCacheDebug",
            "awAtFacade.complete",
            "mixinFacade.analyzeDiagnostics",
            "awAtFacade.analyzeDiagnostics",
            "mixinFacade.definitions",
            "awAtFacade.definitions",
            "definitionResolver.resolveAll",
            "mixinFacade.collectSourceEntries",
            "awAtFacade.collectAwAtEntries",
            "mixinFacade.references",
            "awAtFacade.references",
            "mixinFacade.codeActions",
            "awAtFacade.codeActions",
            "mixinFacade.selectedMixinConfigContent",
        ];
        let row = self.row.lock().unwrap();
        let expected = row["reads"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["api"].as_str().unwrap())
            .filter(|a| HOST_APIS.contains(a))
            .collect::<Vec<_>>();
        let calls = self.calls.lock().unwrap();
        assert_eq!(
            calls.iter().map(String::as_str).collect::<Vec<_>>(),
            expected,
            "host callback sequence in {} {}",
            row["legacyCase"],
            row["api"]
        );
    }
    fn read(&self, api: &str) -> Value {
        self.read_where(api, |_| true)
    }
    fn read_where(&self, api: &str, predicate: impl Fn(&Value) -> bool) -> Value {
        self.calls.lock().unwrap().push(api.into());
        let row = self.row.lock().unwrap();
        row["reads"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["api"] == api && predicate(&r["args"]))
            .unwrap_or_else(|| {
                panic!(
                    "unrecorded command host operation {api} in {}",
                    row["legacyCase"]
                )
            })
            .clone()
    }
}
fn validate_source(record: &Value, context: &McdevRequestContext) {
    let args = &record["args"];
    if let Some(source) = args.get("source") {
        assert_eq!(source, &context.buffer_text);
    }
    if let Some(uri) = args.get("documentUri") {
        assert_eq!(uri, &context.document_uri);
    }
    if let Some(line) = args.get("line") {
        assert_eq!(line, &json!(context.position.line));
    }
    if let Some(character) = args.get("character") {
        assert_eq!(character, &json!(context.position.character));
    }
}
impl CommandHost for RecordedHost {
    fn now_millis(&self) -> i64 {
        self.row.lock().unwrap()["reads"]
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .find(|r| r["api"] == "clock.now")
            .and_then(|r| r["output"].as_i64())
            .unwrap_or(0)
    }
    fn build_identity(&self) -> Result<BuildIdentity, CommandFailure> {
        let r = self.read("McdevBuildInfo.load");
        let v = &r["output"];
        Ok(BuildIdentity {
            commit: text(v, "commit").into(),
            build_time: text(v, "buildTime").into(),
            version: text(v, "version").into(),
            artifact_location: text(v, "jarLocation").into(),
        })
    }
    fn has_project_facts(&self, c: &McdevRequestContext) -> Result<bool, CommandFailure> {
        let r = self.read("mixinFacade.hasJavaProject");
        assert_eq!(r["args"]["arg0"], c.document_uri);
        assert_eq!(r["args"]["arg1"], c.workspace_root);
        Ok(parse(&r["output"]))
    }
    fn load_session(&self, root: &str) -> Result<Arc<ProjectContext>, CommandFailure> {
        if let Some(error) = self.session_failure.lock().unwrap().clone() {
            return Err(error);
        }
        let r = self.read("projectService.loadSession");
        assert_eq!(r["args"]["arg0"], root);
        Ok(project(&r["output"]["context"]))
    }
    fn load_cached_session(&self, root: &str) -> Result<CachedProjectSession, CommandFailure> {
        let r = self.read("projectService.loadCachedSession");
        assert_eq!(r["args"]["arg0"], root);
        let v = &r["output"];
        Ok(CachedProjectSession {
            context: project(&v["session"]["context"]),
            version: parse(&v["version"]),
            cache_hit: parse(&v["cacheHit"]),
            fingerprint: String::new(),
        })
    }
    fn reindex(&self, root: &str) -> Result<Arc<ProjectContext>, CommandFailure> {
        let r = self.read("projectService.reindex");
        assert_eq!(r["args"]["arg0"], root);
        Ok(project(&r["output"]["context"]))
    }
    fn clear_project_cache(&self) -> Result<(), CommandFailure> {
        self.read("projectService.clearCache");
        Ok(())
    }
    fn semantic_model(
        &self,
        c: &McdevRequestContext,
        version: i64,
        _: &ProjectContext,
        _: &dyn CancellationChecker,
    ) -> Result<SemanticSnapshot, CommandFailure> {
        let r = self.read("semanticCache.get");
        assert_eq!(r["args"]["source"], c.buffer_text);
        assert_eq!(r["args"]["documentUri"], c.document_uri);
        assert_eq!(r["args"]["documentVersion"], version);
        let v = &r["output"];
        Ok(SemanticSnapshot {
            model: parse_source(&v["model"]),
            cache_hit: parse(&v["cacheHit"]),
            parse_ms: parse(&v["astParseMs"]),
        })
    }
    fn buffer_only_completion(
        &self,
        c: &McdevRequestContext,
        options: &MixinCompletionOptions,
        _: &dyn CancellationChecker,
    ) -> Result<Option<RuntimeCompletion>, CommandFailure> {
        let r = self.read("mixinFacade.tryCompleteBufferOnly");
        std::thread::sleep(std::time::Duration::from_millis(
            self.buffer_delay_millis.load(Ordering::Relaxed),
        ));
        validate_source(&r, c);
        assert_eq!(r["args"]["options"], json!(options));
        let v = &r["output"];
        Ok(if v.is_null() {
            None
        } else {
            Some(completion(
                &v["result"],
                &c.buffer_text,
                Some(&v["context"]),
            ))
        })
    }
    fn complete(
        &self,
        _: &ProjectContext,
        c: &McdevRequestContext,
        options: &MixinCompletionOptions,
        _: &MixinClassModel,
        version: u64,
        _: &dyn CancellationChecker,
    ) -> Result<RuntimeCompletion, CommandFailure> {
        let r = self.read("mixinFacade.completeWithDebug");
        validate_source(&r, c);
        assert_eq!(r["args"]["options"], json!(options));
        assert_eq!(r["args"]["projectSessionVersion"], version);
        Ok(completion(&r["output"], &c.buffer_text, None))
    }
    fn candidate_cache_metrics(&self) -> Result<(Option<bool>, Option<i64>), CommandFailure> {
        let r = self.read("mixinFacade.candidateCacheDebug");
        Ok((parse(&r["output"]["hit"]), parse(&r["output"]["buildMs"])))
    }
    fn awat_complete(
        &self,
        _: &ProjectContext,
        c: &McdevRequestContext,
        _: &dyn CancellationChecker,
    ) -> Result<Vec<McCompletionItem>, CommandFailure> {
        let r = self.read("awAtFacade.complete");
        validate_source(&r, c);
        Ok(parse(&r["output"]))
    }
    fn diagnostics(
        &self,
        _: &ProjectContext,
        c: &McdevRequestContext,
        family: SourceFamily,
        cancellation: &dyn CancellationChecker,
    ) -> Result<Vec<McDiagnostic>, CommandFailure> {
        let r = self.read(if family == SourceFamily::AwAt {
            "awAtFacade.analyzeDiagnostics"
        } else {
            "mixinFacade.analyzeDiagnostics"
        });
        validate_source(&r, c);
        if r["output"].get("throwable").is_some() {
            cancellation.check_phase(mcdev_core::expression_engine::CancellationPhase::Matching)?;
            return Err(CommandFailure::Runtime(
                "expected matching cancellation was not observed".into(),
            ));
        }
        Ok(parse(&r["output"]))
    }
    fn definitions(
        &self,
        _: &ProjectContext,
        c: &McdevRequestContext,
        family: SourceFamily,
        _: &dyn CancellationChecker,
    ) -> Result<Vec<McDefinitionTarget>, CommandFailure> {
        let r = self.read(if family == SourceFamily::AwAt {
            "awAtFacade.definitions"
        } else {
            "mixinFacade.definitions"
        });
        validate_source(&r, c);
        Ok(parse(&r["output"]))
    }
    fn resolve_definitions(
        &self,
        _: &ProjectContext,
        root: &str,
        targets: &[McDefinitionTarget],
    ) -> Result<Vec<ResolvedDefinition>, CommandFailure> {
        let r = self.read("definitionResolver.resolveAll");
        assert_eq!(r["args"]["workspaceRootUri"], root);
        assert_eq!(
            parse::<Vec<McDefinitionTarget>>(&r["args"]["targets"]),
            targets
        );
        Ok(r["output"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| ResolvedDefinition {
                target: parse(&v["target"]),
                document_uri: text(v, "documentUri").into(),
                range: parse(&v["range"]),
                resolution: parse(&v["resolution"]),
                resolution_message: parse(&v["resolutionMessage"]),
            })
            .collect())
    }
    fn source_entries(
        &self,
        _: &ProjectContext,
        c: &McdevRequestContext,
        family: SourceFamily,
    ) -> Result<Vec<SourceScanEntry>, CommandFailure> {
        let r = self.read(if family == SourceFamily::AwAt {
            "awAtFacade.collectAwAtEntries"
        } else {
            "mixinFacade.collectSourceEntries"
        });
        assert_eq!(r["args"]["currentDocumentUri"], c.document_uri);
        assert_eq!(r["args"]["currentBufferText"], c.buffer_text);
        Ok(parse(&r["output"]))
    }
    fn references(
        &self,
        _: &ProjectContext,
        target: &McDefinitionTarget,
        sources: &[SourceScanEntry],
        family: SourceFamily,
        _: &dyn CancellationChecker,
    ) -> Result<Vec<McReferenceLocation>, CommandFailure> {
        let api = if family == SourceFamily::AwAt {
            "awAtFacade.references"
        } else {
            "mixinFacade.references"
        };
        let r = self.read_where(api, |a| parse::<McDefinitionTarget>(&a["arg1"]) == *target);
        assert_eq!(parse::<Vec<SourceScanEntry>>(&r["args"]["arg2"]), sources);
        Ok(parse(&r["output"]))
    }
    fn code_actions(
        &self,
        _: &ProjectContext,
        c: &McdevRequestContext,
        range: &McdevRange,
        code: Option<&str>,
        family: SourceFamily,
        _: &dyn CancellationChecker,
    ) -> Result<Vec<McFix>, CommandFailure> {
        let r = self.read_where(
            if family == SourceFamily::AwAt {
                "awAtFacade.codeActions"
            } else {
                "mixinFacade.codeActions"
            },
            |a| a["diagnosticCode"].as_str() == code,
        );
        assert_eq!(r["args"]["source"], c.buffer_text);
        assert_eq!(r["args"]["documentUri"], c.document_uri);
        if family == SourceFamily::JavaKotlin {
            assert_eq!(r["args"]["line"], range.start.line);
            assert_eq!(r["args"]["character"], range.start.character);
        }
        Ok(r["output"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| fix(v, &c.buffer_text))
            .collect())
    }
    fn selected_mixin_config_content(
        &self,
        _: &ProjectContext,
        c: &McdevRequestContext,
    ) -> Result<Option<String>, CommandFailure> {
        let r = self.read("mixinFacade.selectedMixinConfigContent");
        assert_eq!(r["args"]["source"], c.buffer_text);
        Ok(parse(&r["output"]))
    }
    fn resolve_fix(
        &self,
        project: &ProjectContext,
        c: &McdevRequestContext,
        fix: &McFix,
        config: Option<&str>,
    ) -> Result<Option<(String, McFix)>, CommandFailure> {
        let row = self.row.lock().unwrap();
        let index = RecordedIndex {
            reads: row["reads"].as_array().unwrap(),
        };
        let source = if matches!(fix, McFix::AddMixinConfigEntryFix { .. }) {
            let Some(config) = config else {
                return Ok(None);
            };
            config
        } else {
            c.buffer_text.as_str()
        };
        let result = match fix {
            McFix::AddMixinConfigEntryFix { .. } | McFix::AddMethodDescriptorFix { .. } => {
                mcdev_core::mixin_actions::MixinCodeActionService::apply_fix(fix, source)
            }
            McFix::AddAccessTransformerEntryFix { .. }
            | McFix::RemapAccessTransformerEntryFix { .. }
            | McFix::AddAtMethodDescriptorFix { .. }
            | McFix::RemoveDuplicateAtEntryFix { .. } => {
                mcdev_core::at::AccessTransformerCodeActionService::new(
                    &index,
                    Some(&project.mappings),
                )
                .apply_fix(fix, source)
            }
            _ => mcdev_core::aw::AccessWidenerCodeActionService::apply_fix(fix, source),
        };
        Ok(result.map(|fix| (source.into(), fix)))
    }
}
fn fix(v: &Value, source: &str) -> McFix {
    let mut value = v.clone();
    let kind = text(v, "kind");
    let ty = if v.get("edits").is_some() {
        "WorkspaceEditFix"
    } else {
        match kind {
            "quickfix.mixin.config" => "AddMixinConfigEntryFix",
            "quickfix.mixin.methodDescriptor" => "AddMethodDescriptorFix",
            "quickfix.mixin.generateAccessor" => "GenerateAccessorMethodFix",
            "quickfix.mixin.generateInvoker" => "GenerateInvokerMethodFix",
            "quickfix.aw.generateEntry" => "GenerateAccessWidenerEntryFix",
            "quickfix.aw.remapNamespace" => "RemapAccessWidenerEntryFix",
            "quickfix.aw.addDescriptor" => "AddAccessWidenerDescriptorFix",
            "quickfix.aw.fixDescriptor" => "FixAccessWidenerDescriptorFix",
            "quickfix.aw.removeDuplicate" => "RemoveDuplicateAccessWidenerEntryFix",
            "quickfix.at.generateEntry" => "AddAccessTransformerEntryFix",
            "quickfix.at.remapNamespace" => "RemapAccessTransformerEntryFix",
            "quickfix.at.addDescriptor" => "AddAtMethodDescriptorFix",
            "quickfix.at.removeDuplicate" => "RemoveDuplicateAtEntryFix",
            other => panic!("unknown legacy fix kind {other}"),
        }
    };
    value["type"] = json!(ty);
    offsets(&mut value, source, "");
    parse(&value)
}
struct RecordedIndex<'a> {
    reads: &'a [Value],
}
impl RecordedIndex<'_> {
    fn read(&self, api: &str, args: Value) -> Option<&Value> {
        self.reads
            .iter()
            .find(|r| r["api"] == api && r["args"] == args)
            .map(|r| &r["output"])
    }
    fn classes(&self) -> Vec<ClassIndexEntry> {
        let mut classes = BTreeMap::new();
        for r in self.reads {
            let values = if let Some(a) = r["output"].as_array() {
                a.iter().collect::<Vec<_>>()
            } else {
                vec![&r["output"]]
            };
            for v in values {
                if v.get("simpleName").is_some() {
                    let entry: ClassIndexEntry = parse(v);
                    classes.insert(entry.internal_name.clone(), entry);
                }
            }
        }
        classes.into_values().collect()
    }
}
impl ClassIndex for RecordedIndex<'_> {
    fn find_classes(&self, p: &str, l: usize) -> Vec<ClassIndexEntry> {
        self.read("ClassIndex.findClasses", json!([p, l]))
            .map(parse)
            .unwrap_or_else(|| panic!("unrecorded classes {p} {l}"))
    }
    fn find_class(&self, n: &str) -> Option<ClassIndexEntry> {
        self.read("ClassIndex.findClass", json!([n]))
            .map(parse)
            .unwrap_or_else(|| self.classes().into_iter().find(|e| e.internal_name == n))
    }
    fn find_class_by_fqn(&self, n: &str) -> Option<ClassIndexEntry> {
        self.read("ClassIndex.findClassByFqn", json!([n]))
            .map(parse)
            .unwrap_or_else(|| self.classes().into_iter().find(|e| e.fqn() == n))
    }
    fn get_methods(&self, o: &str) -> Vec<MethodIndexEntry> {
        self.read("ClassIndex.getMethods", json!([o]))
            .map(parse)
            .unwrap_or_else(|| panic!("unrecorded methods {o}"))
    }
    fn get_fields(&self, o: &str) -> Vec<FieldIndexEntry> {
        self.read("ClassIndex.getFields", json!([o]))
            .map(parse)
            .unwrap_or_else(|| panic!("unrecorded fields {o}"))
    }
}
