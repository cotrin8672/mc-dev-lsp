//! Frozen original arguments and complete public results, including final facade outputs.
use mcdev_core::{
    at::*,
    aw::*,
    awat::*,
    mapping::{self, MappingLookupResult, MappingResolver, ProjectMappingContext},
    *,
};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    sync::{Arc, OnceLock},
};

fn parse<T: DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("fixture decode {e}: {v}"))
}
fn text<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or_else(|| panic!("missing {k}: {v}"))
}
fn number(v: &Value, k: &str) -> usize {
    v[k].as_u64().unwrap() as usize
}
fn camel(s: &str) -> String {
    let mut upper = false;
    s.chars()
        .filter_map(|c| {
            if c == '_' {
                upper = true;
                None
            } else {
                let ch = if upper { c.to_ascii_uppercase() } else { c };
                upper = false;
                Some(ch)
            }
        })
        .collect()
}
fn snake(s: &str) -> String {
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
        .map(|p| {
            let mut c = p.chars();
            c.next().map_or(String::new(), |c| {
                c.to_ascii_uppercase().to_string() + &p[1..].to_ascii_lowercase()
            })
        })
        .collect()
}
fn input(v: &Value, key: &str) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, v)| {
                    let k = snake(k);
                    let v = input(v, &k);
                    (k, v)
                })
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(|v| input(v, key)).collect()),
        Value::String(s) if matches!(key, "slot" | "directive" | "kind" | "modifier") => {
            json!(pascal(s))
        }
        _ => v.clone(),
    }
}
fn canonical(v: Value, key: &str) -> Value {
    if key == "metadata" {
        return v;
    }
    match v {
        Value::Object(mut m) => {
            if m.contains_key("title") && !m.contains_key("type") {
                let kind = m.get("kind").and_then(Value::as_str).unwrap();
                let ty = if m.contains_key("edits") {
                    "WorkspaceEditFix"
                } else {
                    match kind {
                        "quickfix.aw.generateEntry" => "GenerateAccessWidenerEntryFix",
                        "quickfix.aw.remapNamespace" => "RemapAccessWidenerEntryFix",
                        "quickfix.aw.addDescriptor" => "AddAccessWidenerDescriptorFix",
                        "quickfix.aw.fixDescriptor" => "FixAccessWidenerDescriptorFix",
                        "quickfix.aw.removeDuplicate" => "RemoveDuplicateAccessWidenerEntryFix",
                        "quickfix.at.generateEntry" => "AddAccessTransformerEntryFix",
                        "quickfix.at.remapNamespace" => "RemapAccessTransformerEntryFix",
                        "quickfix.at.addDescriptor" => "AddAtMethodDescriptorFix",
                        "quickfix.at.removeDuplicate" => "RemoveDuplicateAtEntryFix",
                        unknown => panic!("unknown legacy fix kind {unknown}"),
                    }
                };
                m.insert("type".into(), json!(ty));
            }
            Value::Object(
                m.into_iter()
                    .map(|(k, v)| {
                        let k = camel(&k);
                        let v = canonical(v, &k);
                        (k, v)
                    })
                    .collect(),
            )
        }
        Value::Array(a) => Value::Array(a.into_iter().map(|v| canonical(v, key)).collect()),
        Value::String(s)
            if matches!(key, "slot" | "directive" | "kind" | "modifier") && !s.contains('.') =>
        {
            json!(s.replace('_', "").to_ascii_lowercase())
        }
        other => other,
    }
}

struct RecordedIndex<'a>(&'a [Value]);
impl RecordedIndex<'_> {
    fn read(&self, api: &str, args: Value) -> Option<&Value> {
        self.0
            .iter()
            .find(|r| r["api"] == api && r["args"] == args)
            .map(|r| &r["output"])
    }
    fn classes(&self) -> Vec<ClassIndexEntry> {
        self.0
            .iter()
            .filter(|r| {
                matches!(
                    r["api"].as_str(),
                    Some(
                        "ClassIndex.findClass"
                            | "ClassIndex.findClassByFqn"
                            | "ClassIndex.findClasses"
                    )
                )
            })
            .flat_map(|r| {
                if r["output"].is_array() {
                    parse::<Vec<ClassIndexEntry>>(&r["output"])
                } else if r["output"].is_null() {
                    vec![]
                } else {
                    vec![parse(&r["output"])]
                }
            })
            .collect()
    }
}
impl ClassIndex for RecordedIndex<'_> {
    fn find_classes(&self, prefix: &str, limit: usize) -> Vec<ClassIndexEntry> {
        self.read("ClassIndex.findClasses", json!([prefix, limit]))
            .map(parse)
            .unwrap_or_else(|| panic!("unrecorded classes {prefix}/{limit}"))
    }
    fn find_class(&self, name: &str) -> Option<ClassIndexEntry> {
        self.read("ClassIndex.findClass", json!([name]))
            .map(parse)
            .unwrap_or_else(|| self.classes().into_iter().find(|c| c.internal_name == name))
    }
    fn find_class_by_fqn(&self, name: &str) -> Option<ClassIndexEntry> {
        self.read("ClassIndex.findClassByFqn", json!([name]))
            .map(parse)
            .unwrap_or_else(|| self.classes().into_iter().find(|c| c.fqn() == name))
    }
    fn get_methods(&self, owner: &str) -> Vec<MethodIndexEntry> {
        self.read("ClassIndex.getMethods", json!([owner]))
            .map(parse)
            .unwrap_or_else(|| panic!("unrecorded methods {owner}"))
    }
    fn get_fields(&self, owner: &str) -> Vec<FieldIndexEntry> {
        self.read("ClassIndex.getFields", json!([owner]))
            .map(parse)
            .unwrap_or_else(|| panic!("unrecorded fields {owner}"))
    }
}
struct RecordedMapping {
    reads: Vec<Value>,
    namespaces: BTreeSet<MappingNamespace>,
}
impl RecordedMapping {
    fn read<T: DeserializeOwned>(&self, api: &str, args: Value) -> MappingLookupResult<T> {
        let r = self
            .reads
            .iter()
            .find(|r| r["api"] == api && r["args"] == args)
            .unwrap_or_else(|| panic!("unrecorded mapping {api} {args}"));
        let v = &r["output"];
        match r["outputType"]
            .as_str()
            .unwrap()
            .rsplit('$')
            .next()
            .unwrap()
        {
            "Found" => MappingLookupResult::Found(parse(&v["value"])),
            "Missing" => MappingLookupResult::Missing {
                subject: parse(&v["subject"]),
                owner: parse(&v["owner"]),
                name: parse(&v["name"]),
                descriptor: parse(&v["descriptor"]),
                from: parse(&v["from"]),
                to: parse(&v["to"]),
            },
            "InvalidDescriptor" => MappingLookupResult::InvalidDescriptor {
                descriptor: text(v, "descriptor").into(),
                message: text(v, "message").into(),
                offset: number(v, "offset"),
            },
            unknown => panic!("mapping subtype {unknown}"),
        }
    }
}
impl MappingResolver for RecordedMapping {
    fn namespaces(&self) -> BTreeSet<MappingNamespace> {
        self.namespaces.clone()
    }
    fn remap_class(
        &self,
        r: &mapping::ClassRef,
        to: MappingNamespace,
    ) -> MappingLookupResult<mapping::ClassRef> {
        self.read("MappingResolver.remapClass", json!([r, to]))
    }
    fn remap_field(
        &self,
        r: &mapping::FieldRef,
        to: MappingNamespace,
    ) -> MappingLookupResult<mapping::FieldRef> {
        self.read("MappingResolver.remapField", json!([r, to]))
    }
    fn remap_method(
        &self,
        r: &mapping::MethodRef,
        to: MappingNamespace,
    ) -> MappingLookupResult<mapping::MethodRef> {
        self.read("MappingResolver.remapMethod", json!([r, to]))
    }
    fn remap_descriptor(
        &self,
        d: &str,
        from: MappingNamespace,
        to: MappingNamespace,
    ) -> MappingLookupResult<String> {
        self.read("MappingResolver.remapDescriptor", json!([d, from, to]))
    }
}
fn mapping_context(v: &Value, reads: &[Value]) -> Option<ProjectMappingContext> {
    if v.is_null() {
        return None;
    }
    let available_namespaces = parse(&v["availableNamespaces"]);
    Some(ProjectMappingContext {
        source_namespace: parse(&v["sourceNamespace"]),
        runtime_namespace: parse(&v["runtimeNamespace"]),
        aw_namespace: parse(&v["awNamespace"]),
        at_namespace: parse(&v["atNamespace"]),
        available_namespaces,
        resolver: Arc::new(RecordedMapping {
            reads: reads.to_vec(),
            namespaces: parse(&v["availableNamespaces"]),
        }),
    })
}
fn fix(v: &Value) -> McFix {
    let mut value = v["_value"].clone();
    value["type"] = v["_type"].clone();
    parse(&value)
}
fn fixes(v: Vec<McFix>) -> Value {
    json!(v
        .iter()
        .map(|f| {
            let mut v = json!(f);
            v["kind"] = json!(f.kind());
            v
        })
        .collect::<Vec<_>>())
}
fn file_type(v: &Value) -> AwAtFileType {
    match v.as_str().unwrap() {
        "ACCESS_WIDENER" => AwAtFileType::AccessWidener,
        "ACCESS_TRANSFORMER" => AwAtFileType::AccessTransformer,
        _ => panic!("file type {v}"),
    }
}
fn request(v: &Value) -> AwAtFacadeRequest<'_> {
    AwAtFacadeRequest {
        buffer_text: text(v, "bufferText"),
        document_uri: text(v, "documentUri"),
        line: number(v, "line"),
        character: number(v, "character"),
        file_type: file_type(&v["fileType"]),
    }
}
fn replay(row: &Value) -> Value {
    let a = &row["args"];
    let reads = row["reads"].as_array().unwrap();
    let index = RecordedIndex(reads);
    let mv = a
        .get("mappingContext")
        .filter(|v| !v.is_null())
        .or_else(|| a.get("_mappingContext"))
        .unwrap_or(&Value::Null);
    let m = mapping_context(mv, reads);
    let at = AccessTransformerCodeActionService::new(&index, m.as_ref());
    match row["api"].as_str().unwrap() {
        "AccessWidenerParser.parse" => match AccessWidenerParser::parse(text(a, "text")) {
            Ok(f) => json!({"file":f}),
            Err(e) => json!(e),
        },
        "AccessTransformerParser.parse" => match AccessTransformerParser::parse(text(a, "text")) {
            Ok(f) => json!({"file":f}),
            Err(e) => json!(e),
        },
        "AwContextExtractor.extract" => json!(AwContextExtractor::extract(
            text(a, "source"),
            number(a, "line"),
            number(a, "character")
        )),
        "AwContextExtractor.extractAtOffset" => json!(AwContextExtractor::extract_at_offset(
            text(a, "source"),
            number(a, "offset")
        )),
        "AwContextExtractor.parseFileNamespace" => {
            json!(AwContextExtractor::parse_file_namespace(text(a, "source")))
        }
        "AwContextExtractor.toOffset" => json!(AwContextExtractor::to_offset(
            text(a, "source"),
            number(a, "line"),
            number(a, "character")
        )),
        "AtContextExtractor.toOffset" => json!(AtTextPositions::to_offset(
            text(a, "source"),
            number(a, "line"),
            number(a, "character")
        )),
        "AwContextExtractor.offsetToPosition" => {
            offset_to_position(text(a, "source"), number(a, "offset")).map_or(
                Value::Null,
                |p| json!({"first":p.line,"second":p.character}),
            )
        }
        "AtContextExtractor.extract" => json!(AtContextExtractor::extract(
            text(a, "source"),
            number(a, "line"),
            number(a, "character")
        )),
        "AtContextExtractor.extractAtOffset" => json!(AtContextExtractor::extract_at_offset(
            text(a, "source"),
            number(a, "offset")
        )),
        "AtContextExtractor.parseLine" => json!(AtContextExtractor::parse_line(
            text(a, "source"),
            number(a, "lineNumber")
        )),
        "AccessWidenerCompletionService.complete" => {
            json!(AccessWidenerCompletionService::new(&index).complete(
                &parse(&input(&a["context"], "")),
                m.as_ref(),
                AccessWidenerCompletionOptions {
                    include_descriptors_for_members: a["options"]["includeDescriptorsForMembers"]
                        .as_bool()
                        .unwrap()
                }
            ))
        }
        "AccessTransformerCompletionService.complete" => {
            json!(AccessTransformerCompletionService::new(&index, m.as_ref())
                .complete(&parse(&input(&a["context"], ""))))
        }
        "AccessWidenerDiagnosticsService.analyze" => {
            let r = &a["request"];
            let rm = mapping_context(&r["mappingContext"], reads);
            json!(AccessWidenerDiagnosticsService::new(&index).analyze(
                text(r, "source"),
                text(r, "documentUri"),
                rm.as_ref()
            ))
        }
        "AccessTransformerDiagnosticsService.analyze" => {
            let r = &a["request"];
            let rm = mapping_context(
                r.get("mappingContext")
                    .filter(|v| !v.is_null())
                    .unwrap_or(mv),
                reads,
            );
            json!(
                AccessTransformerDiagnosticsService::new(&index, rm.as_ref())
                    .analyze(text(r, "source"), text(r, "documentUri"))
            )
        }
        "AccessWidenerDefinitionService.definitionsAt" => json!(
            AccessWidenerDefinitionService::new(&index, m.as_ref()).definitions_at(
                text(a, "source"),
                number(a, "line"),
                number(a, "character")
            )
        ),
        "AccessTransformerDefinitionService.definitionsAt" => json!(
            AccessTransformerDefinitionService::new(&index, m.as_ref()).definitions_at(
                text(a, "source"),
                number(a, "line"),
                number(a, "character")
            )
        ),
        "AccessWidenerReferenceService.findReferences"
        | "AccessTransformerReferenceService.findReferences" => {
            let target = parse(&a["target"]["_value"]);
            let sources = if let Some(s) = a.get("entry") {
                vec![parse(s)]
            } else {
                parse(&a["sources"])
            };
            if row["api"] == "AccessWidenerReferenceService.findReferences" {
                json!(AccessWidenerReferenceService::new(&index, m.as_ref())
                    .find_references(&target, &sources))
            } else {
                json!(AccessTransformerReferenceService::new(&index)
                    .find_references(&target, &sources))
            }
        }
        "AccessWidenerEditor.formatClassEntry" => json!(AccessWidenerEditor::format_class_entry(
            parse(&input(&a["directive"], "directive")),
            text(a, "owner")
        )),
        "AccessWidenerEditor.formatMethodEntry" => json!(AccessWidenerEditor::format_method_entry(
            parse(&input(&a["directive"], "directive")),
            text(a, "owner"),
            text(a, "name"),
            text(a, "descriptor")
        )),
        "AccessWidenerEditor.formatFieldEntry" => json!(AccessWidenerEditor::format_field_entry(
            parse(&input(&a["directive"], "directive")),
            text(a, "owner"),
            text(a, "name"),
            text(a, "descriptor")
        )),
        "AccessWidenerEditor.formatEntry" => json!(AccessWidenerEditor::format_entry(&parse(
            &input(&a["entry"], "")
        ))),
        "AccessWidenerEditor.entryKey" => json!(AccessWidenerEditor::entry_key(&parse(&input(
            &a["entry"],
            ""
        )))),
        "AccessWidenerEditor.insertEntry" => json!(AccessWidenerEditor::insert_entry(
            text(a, "content"),
            &parse(&input(&a["entry"], "")),
            parse(&a["afterLine"])
        )),
        "AccessWidenerEditor.removeLine" => json!(AccessWidenerEditor::remove_line(
            text(a, "content"),
            number(a, "lineNumber")
        )),
        "AccessWidenerEditor.replaceLine" => json!(AccessWidenerEditor::replace_line(
            text(a, "content"),
            number(a, "lineNumber"),
            text(a, "newLine")
        )),
        "AccessTransformerEditor.formatEntry" => json!(AccessTransformerEditor::format_entry(
            &parse(&input(&a["entry"], ""))
        )),
        "AccessTransformerEditor.entryKey" => json!(AccessTransformerEditor::entry_key(&parse(
            &input(&a["entry"], "")
        ))),
        "AccessTransformerEditor.appendEntry" => json!(AccessTransformerEditor::append_entry(
            text(a, "content"),
            &parse(&input(&a["entry"], ""))
        )),
        "AccessTransformerEditor.removeLine" => json!(AccessTransformerEditor::remove_line(
            text(a, "content"),
            number(a, "lineNumber")
        )),
        "AccessTransformerEditor.replaceLine" => json!(AccessTransformerEditor::replace_line(
            text(a, "content"),
            number(a, "lineNumber"),
            text(a, "newLine")
        )),
        "AtMemberInsertFormatter.formatOwnerInsert" => json!(
            AtMemberInsertFormatter::format_owner_insert(&parse(&a["classEntry"]))
        ),
        "AtMemberInsertFormatter.formatMethodInsert" => {
            json!(AtMemberInsertFormatter::format_method_insert(
                &parse(&a["method"]),
                text(a, "ownerInternalName"),
                m.as_ref()
            ))
        }
        "AtMemberInsertFormatter.formatFieldInsert" => {
            json!(AtMemberInsertFormatter::format_field_insert(
                &parse(&a["field"]),
                text(a, "ownerInternalName"),
                m.as_ref()
            ))
        }
        "AtMemberInsertFormatter.remapMethodForEntry" => {
            json!(AtMemberInsertFormatter::remap_method_for_entry(
                text(a, "ownerInternalName"),
                &parse(&a["method"]),
                m.as_ref().unwrap()
            ))
        }
        "AtMemberInsertFormatter.remapFieldForEntry" => {
            json!(AtMemberInsertFormatter::remap_field_for_entry(
                text(a, "ownerInternalName"),
                &parse(&a["field"]),
                m.as_ref().unwrap()
            ))
        }
        "AccessWidenerCodeActionService.fixesForDiagnostics" => {
            fixes(AccessWidenerCodeActionService::fixes_for_diagnostics(
                &parse::<Vec<McDiagnostic>>(&a["diagnostics"]),
                text(a, "documentUri"),
                text(a, "source"),
                if a["classIndex"].is_null() {
                    None
                } else {
                    Some(&index)
                },
                m.as_ref(),
            ))
        }
        "AccessTransformerCodeActionService.fixesForDiagnostics" => {
            fixes(at.fixes_for_diagnostics(
                &parse::<Vec<McDiagnostic>>(&a["diagnostics"]),
                text(a, "documentUri"),
                text(a, "source"),
            ))
        }
        "AccessWidenerCodeActionService.generateEntryFix" => {
            let f = AccessWidenerCodeActionService::generate_entry_fix(
                text(a, "documentUri"),
                &parse(&input(&a["entry"], "")),
                parse(&a["insertLine"]),
            );
            fixes(vec![f])[0].clone()
        }
        "AccessTransformerCodeActionService.generateEntryFix" => {
            AccessTransformerCodeActionService::generate_entry_fix(
                &parse(&input(&a["context"], "")),
                text(a, "documentUri"),
            )
            .map_or(Value::Null, |f| fixes(vec![f])[0].clone())
        }
        "AccessWidenerCodeActionService.applyDescriptorFix"
        | "AccessWidenerCodeActionService.applyGenerateEntryFix"
        | "AccessWidenerCodeActionService.applyRemoveDuplicateFix" => json!(
            AccessWidenerCodeActionService::apply_fix(&fix(&a["fix"]), text(a, "currentContent"))
        ),
        "AccessTransformerCodeActionService.applyMethodDescriptorFix"
        | "AccessTransformerCodeActionService.applyAddEntryFix"
        | "AccessTransformerCodeActionService.applyRemapFix"
        | "AccessTransformerCodeActionService.applyRemoveDuplicateFix" => {
            json!(at.apply_fix(&fix(&a["fix"]), text(a, "currentContent")))
        }
        "AwAtBufferSupport.detectFileType" => json!(AwAtBufferSupport::detect_file_type(
            text(a, "languageId"),
            text(a, "documentUri")
        )
        .map(|f| match f {
            AwAtFileType::AccessWidener => "ACCESS_WIDENER",
            AwAtFileType::AccessTransformer => "ACCESS_TRANSFORMER",
        })),
        "AwAtServiceFacade.complete" => {
            json!(AwAtServiceFacade::new(&index, m.as_ref()).complete(&request(&a["request"])))
        }
        "AwAtServiceFacade.diagnose" => {
            json!(AwAtServiceFacade::new(&index, m.as_ref()).diagnose(&request(&a["request"])))
        }
        "AwAtServiceFacade.codeActions" => fixes(
            AwAtServiceFacade::new(&index, m.as_ref())
                .code_actions(&request(&a["request"]), a["diagnosticCode"].as_str()),
        ),
        unknown => panic!("unmapped original public API {unknown}"),
    }
}
fn deltas() -> &'static Vec<Value> {
    static DELTAS: OnceLock<Vec<Value>> = OnceLock::new();
    DELTAS.get_or_init(|| {
        parse(&serde_json::from_str::<Value>(include_str!("intentional_deltas.json")).unwrap())
    })
}
fn apply_edits(source: &str, fix: &Value) -> String {
    let mut result = source.to_string();
    let mut edits = parse::<Vec<McTextEdit>>(&fix["edits"]);
    edits.sort_by_key(|e| std::cmp::Reverse(e.start_offset));
    let mut previous = source.len();
    for e in edits {
        assert!(
            e.start_offset <= e.end_offset && e.end_offset <= previous,
            "overlapping/outside edit"
        );
        assert!(source.is_char_boundary(e.start_offset) && source.is_char_boundary(e.end_offset));
        result.replace_range(e.start_offset..e.end_offset, &e.new_text);
        previous = e.start_offset;
    }
    result
}
fn expected_result(row: &Value, actual: &Value) -> Value {
    let matching = deltas()
        .iter()
        .filter(|d| {
            d["legacyCase"] == row["legacyCase"]
                && d["api"] == row["api"]
                && d["args"] == row["args"]
        })
        .collect::<Vec<_>>();
    assert!(matching.len() <= 1, "duplicate delta selector");
    let mut expected = row["output"].clone();
    let Some(d) = matching.first() else {
        return expected;
    };
    assert!(!text(d, "reason").is_empty());
    for change in d["changes"].as_array().unwrap() {
        let pointer = text(change, "field");
        assert_eq!(
            expected.pointer(pointer).unwrap(),
            &change["old"],
            "original oracle must stay intact"
        );
        *expected.pointer_mut(pointer).unwrap() = change["new"].clone();
    }
    let r = &d["regression"];
    let normalized = canonical(actual.clone(), "");
    match text(r, "kind") {
        "contextRange" => {
            let source = text(r, "source");
            let start = number(r, "start");
            let end = number(r, "end");
            assert_eq!(normalized["valueStartOffset"], json!(start));
            assert_eq!(normalized["valueEndOffset"], json!(end));
            assert_eq!(&source[start..end], text(r, "selectedText"));
        }
        "diagnosticRange" => {
            let range = parse::<McTextRange>(&normalized[0]["range"]);
            let source = text(r, "source");
            let start = position_to_offset(source, range.start).unwrap();
            let end = position_to_offset(source, range.end).unwrap();
            assert_eq!(&source[start..end], text(r, "selectedText"));
        }
        "document" => assert_eq!(
            normalized
                .as_str()
                .unwrap_or_else(|| text(&normalized, "content")),
            text(r, "document")
        ),
        "equivalentEdits" => {
            assert_eq!(
                apply_edits(text(r, "source"), &row["output"]),
                text(r, "document")
            );
            let document = apply_edits(text(r, "source"), &normalized);
            assert_eq!(document, text(r, "document"));
            assert!(AccessTransformerParser::parse(&document).is_ok());
        }
        "changedEdits" => {
            assert_eq!(
                apply_edits(text(r, "source"), &row["output"]),
                text(r, "oldDocument")
            );
            let document = apply_edits(text(r, "source"), &normalized);
            assert_eq!(document, text(r, "document"));
            if r.get("assertion").is_some() {
                if row["api"].as_str().unwrap().starts_with("AccessWidener") {
                    assert_eq!(
                        AccessWidenerParser::parse(&document).unwrap().entries.len(),
                        1,
                        "bug-corrected adaptation: count entries in applied document"
                    );
                } else {
                    assert_eq!(
                        AccessTransformerParser::parse(&document)
                            .unwrap()
                            .entries
                            .len(),
                        1,
                        "bug-corrected adaptation: count entries in applied document"
                    );
                }
            }
        }
        unknown => panic!("unhandled delta regression {unknown}"),
    }
    expected
}
pub fn validate_manifest() {
    let rows = include_str!("legacy_calls.jsonl")
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .collect::<Vec<_>>();
    let cases = parse::<Vec<Value>>(
        &serde_json::from_str::<Value>(include_str!("legacy_cases.json")).unwrap(),
    );
    assert_eq!(cases.len(), 155);
    assert_eq!(rows.len(), 549);
    assert_eq!(deltas().len(), 27);
    let source = include_str!("../../awat_legacy.rs");
    let mut native_cases = BTreeSet::new();
    let mut native_functions = BTreeSet::new();
    let mut native_count = 0;
    for (at, tail) in source.match_indices("replay::replay_case(") {
        let _ = tail;
        let start = source[at..].find('"').unwrap() + at + 1;
        let end = source[start..].find('"').unwrap() + start;
        assert!(
            native_cases.insert(source[start..end].to_string()),
            "duplicate native case"
        );
        let function = source[..at].rfind("fn ").unwrap() + 3;
        let finish = source[function..].find('(').unwrap() + function;
        assert!(
            native_functions.insert(source[function..finish].trim().to_string()),
            "duplicate native function"
        );
        native_count += 1;
    }
    let original_cases = cases
        .iter()
        .map(|c| text(c, "legacyCase").to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(native_count, 155);
    assert_eq!(
        native_cases, original_cases,
        "exact frozen IDs must each have an executable independent native entry point"
    );
    for case in &cases {
        assert_eq!(case["status"], "passed");
        let captured = rows
            .iter()
            .filter(|r| r["legacyCase"] == case["legacyCase"])
            .count();
        assert_eq!(json!(captured), case["calls"]);
        assert!(captured > 0);
    }
    for delta in deltas() {
        assert_eq!(
            rows.iter()
                .filter(|r| r["legacyCase"] == delta["legacyCase"]
                    && r["api"] == delta["api"]
                    && r["args"] == delta["args"])
                .count(),
            1
        );
    }
    assert_eq!(
        rows.iter()
            .filter(|r| r["api"].as_str().unwrap().starts_with("AwAtServiceFacade."))
            .count(),
        17,
        "final facade results, not only callees"
    );
}

#[test]
fn metadata_dictionary_keys_are_never_schema_normalized() {
    let value = json!({"metadata":{"owner_name":"a","ownerName":"b","UpperKey":"c"},"value_start_offset":3});
    let normalized = canonical(value.clone(), "");
    assert_eq!(normalized["metadata"], value["metadata"]);
    assert_eq!(normalized["valueStartOffset"], 3);
    assert_eq!(normalized["metadata"].as_object().unwrap().len(), 3);
}
pub fn replay_case(case: &str) {
    static ROWS: OnceLock<Vec<Value>> = OnceLock::new();
    let rows = ROWS.get_or_init(|| {
        include_str!("legacy_calls.jsonl")
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    });
    let mut count = 0;
    let mut differences = vec![];
    for row in rows.iter().filter(|r| r["legacyCase"] == case) {
        count += 1;
        let actual = replay(row);
        let expected = expected_result(row, &actual);
        if canonical(actual.clone(), "") != canonical(expected, "") {
            differences.push(json!({"legacyCase":case,"api":row["api"],"args":row["args"],"original":row["output"],"actual":actual}));
        }
    }
    assert!(count > 0, "uncaptured original case {case}");
    if !differences.is_empty() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../build/awat-replay-differences");
        std::fs::create_dir_all(&dir).unwrap();
        let name = case.rsplit('/').next().unwrap().replace("::", "-");
        let path = dir.join(name + ".json");
        std::fs::write(&path, serde_json::to_string_pretty(&differences).unwrap()).unwrap();
        panic!(
            "{case}: {} full-output differences; {}",
            differences.len(),
            path.display()
        );
    }
}
