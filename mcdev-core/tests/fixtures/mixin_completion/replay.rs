use mcdev_core::{
    bytecode::{
        AtTargetCandidate, AtTargetOperationKind, ConstantValue, OccurrenceResultClassification,
    },
    mapping::{MappingResolver, MappingSet, MappingSetResolver},
    mixin_completion::*,
    mixin_edits::*,
    mixin_source::{AnnotationContext, MixinAnnotation},
    *,
};
#[path = "../mixin_source/replay.rs"]
mod source_replay;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::{cell::Cell, collections::BTreeSet};

fn parse<T: DeserializeOwned>(value: &Value) -> T {
    serde_json::from_value(value.clone()).unwrap_or_else(|e| panic!("fixture decode {e}: {value}"))
}
fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap()
}
fn pascal(value: &str) -> String {
    value
        .split('_')
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|c| c.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase())
                .unwrap_or_default()
        })
        .collect()
}
fn snake(value: &str) -> String {
    let mut output = String::new();
    for c in value.chars() {
        if c.is_uppercase() {
            output.push('_');
            output.extend(c.to_lowercase());
        } else {
            output.push(c);
        }
    }
    output
}
fn source_json(value: &Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.iter().map(source_json).collect()),
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| {
                    let key = snake(key);
                    let value = if matches!(
                        key.as_str(),
                        "annotation"
                            | "slot"
                            | "parent_injector_annotation"
                            | "expression_completion_position"
                    ) {
                        value
                            .as_str()
                            .map(|v| json!(pascal(v)))
                            .unwrap_or_else(|| value.clone())
                    } else {
                        source_json(value)
                    };
                    (key, value)
                })
                .collect(),
        ),
        _ => value.clone(),
    }
}
pub fn context(value: &Value) -> AnnotationContext {
    parse(&source_json(value))
}
pub fn candidate(value: &Value) -> AtTargetCandidate {
    let mut candidate = AtTargetCandidate::new(
        parse(&json!(pascal(text(value, "kind")))),
        text(value, "owner"),
        text(value, "name"),
        text(value, "descriptor"),
    );
    candidate.display_label = text(value, "displayLabel").into();
    candidate.detail = text(value, "detail").into();
    candidate.ordinal = parse(&value["ordinal"]);
    candidate.namespace = parse(&value["namespace"]);
    candidate.operation_kind = value["operationKind"]
        .as_str()
        .map(|v| parse::<AtTargetOperationKind>(&json!(pascal(v))));
    candidate.instruction_occurrence_index =
        value["instructionOccurrenceIndex"].as_i64().unwrap() as i32;
    candidate.occurrence_result_classification = parse::<OccurrenceResultClassification>(&json!(
        pascal(text(value, "occurrenceResultClassification"))
    ));
    candidate.condition_opcode = parse(&value["conditionOpcode"]);
    let constant = &value["constantValue"];
    if !constant.is_null() {
        candidate.constant_value = Some(if let Some(value) = constant.get("internalName") {
            ConstantValue::ClassLiteral(value.as_str().unwrap().into())
        } else if let Some(value) = constant.get("value") {
            if let Some(value) = value.as_str() {
                ConstantValue::StringValue(value.into())
            } else {
                ConstantValue::IntValue(value.as_i64().unwrap() as i32)
            }
        } else {
            ConstantValue::NullValue
        });
    }
    candidate
}
pub struct RecordedIndex<'a> {
    pub reads: &'a [Value],
}
impl RecordedIndex<'_> {
    fn read(&self, api: &str, args: Value) -> Option<&Value> {
        self.reads
            .iter()
            .find(|r| r["api"] == api && r["args"] == args)
            .map(|r| &r["output"])
    }
    fn known_classes(&self) -> Vec<ClassIndexEntry> {
        self.reads
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
            .unwrap_or_else(|| panic!("unrecorded classes query {prefix} {limit}"))
    }
    fn find_class(&self, name: &str) -> Option<ClassIndexEntry> {
        self.read("ClassIndex.findClass", json!([name]))
            .map(parse)
            .unwrap_or_else(|| {
                self.known_classes()
                    .into_iter()
                    .find(|c| c.internal_name == name)
            })
    }
    fn find_class_by_fqn(&self, name: &str) -> Option<ClassIndexEntry> {
        self.read("ClassIndex.findClassByFqn", json!([name]))
            .map(parse)
            .unwrap_or_else(|| self.known_classes().into_iter().find(|c| c.fqn() == name))
    }
    fn get_methods(&self, owner: &str) -> Vec<MethodIndexEntry> {
        self.read("ClassIndex.getMethods", json!([owner]))
            .map(parse)
            .unwrap_or_else(|| panic!("unrecorded methods query {owner}"))
    }
    fn get_fields(&self, owner: &str) -> Vec<FieldIndexEntry> {
        self.read("ClassIndex.getFields", json!([owner]))
            .map(parse)
            .unwrap_or_else(|| panic!("unrecorded fields query {owner}"))
    }
}
fn replay(row: &Value) -> Result<Value, String> {
    let args = &row["args"];
    let reads = row["reads"].as_array().unwrap();
    let index = RecordedIndex { reads };
    Ok(match row["api"].as_str().unwrap() {
        "AtValueCompletionService.complete" => {
            let callbacks = reads
                .iter()
                .filter(|r| {
                    r["api"]
                        .as_str()
                        .is_some_and(|a| a.starts_with("callback."))
                })
                .collect::<Vec<_>>();
            let at = Cell::new(0);
            let probe = || {
                let read = callbacks
                    .get(at.get())
                    .expect("unexpected specifier callback");
                assert_eq!(read["api"], "callback.specifier");
                at.set(at.get() + 1);
                read["output"].as_bool().unwrap()
            };
            let additional = || {
                let read = callbacks
                    .get(at.get())
                    .expect("unexpected additional callback");
                assert_eq!(read["api"], "callback.additional");
                at.set(at.get() + 1);
                parse::<Vec<String>>(&read["output"])
            };
            let result = AtValueCompletionService::new(probe, additional)
                .complete(&context(&args["context"]));
            assert_eq!(at.get(), callbacks.len(), "callback invocation count");
            json!(result)
        }
        "AtTargetCompletionService.formatTarget" => json!(
            AtTargetCompletionService::format_target(&candidate(&args["candidate"]))
        ),
        "ConstantAtHintFormatter.hintFor" => json!(ConstantAtHintFormatter::hint_for(&candidate(
            &args["candidate"]
        ))),
        "AtTargetCompletionService.complete" => json!(AtTargetCompletionService::complete(
            &context(&args["context"]),
            &args["candidates"]
                .as_array()
                .unwrap()
                .iter()
                .map(candidate)
                .collect::<Vec<_>>()
        )),
        "AtTargetInsertFormatter.formatInsert" => {
            let resolver = args["resolver"]
                .get("mappings")
                .map(|v| MappingSetResolver::new(parse::<MappingSet>(v)));
            json!(AtTargetInsertFormatter::format_insert(
                &candidate(&args["candidate"]),
                resolver.as_ref().map(|r| r as &dyn MappingResolver),
                parse(&args["from"]),
                parse(&args["to"])
            ))
        }
        "InjectMethodCompletionService.complete" => {
            json!(InjectMethodCompletionService::new(&index)
                .complete(&context(&args["context"]), &parse(&args["options"])))
        }
        "MixinTargetCompletionService.complete" => json!(MixinTargetCompletionService::new(&index)
            .complete(
                &context(&args["context"]),
                &parse(&args["options"]),
                text(args, "source")
            )),
        "MixinAttributeCompletionService.complete" => {
            json!(MixinAttributeCompletionService::complete(
                &context(&args["context"]),
                text(args, "source")
            ))
        }
        "MixinDeclarationSnippetService.complete" => {
            let service = MixinDeclarationSnippetService::new(&index);
            if args.get("context").is_some() {
                json!(service.complete(text(args, "source"), &context(&args["context"])))
            } else {
                json!(service.complete_at(
                    text(args, "source"),
                    parse::<MixinAnnotation>(&json!(pascal(text(args, "annotation")))),
                    args["annotationStartOffset"].as_u64().unwrap() as usize,
                    &parse::<Vec<String>>(&args["mixinTargets"]),
                    text(args, "targetPrefix")
                ))
            }
        }
        "MixinImportEditBuilder.referenceForInternalName" => {
            json!(MixinImportEditBuilder::reference_for_internal_name(
                text(args, "source"),
                text(args, "internalName"),
                &parse::<BTreeSet<String>>(&args["additionalInternalNames"])
            ))
        }
        "MixinImportEditBuilder.buildImportEdit" => json!(
            MixinImportEditBuilder::build_import_edit(text(args, "source"), text(args, "fqn"))
        ),
        "MixinImportEditBuilder.needsImport" => json!(MixinImportEditBuilder::needs_import(
            text(args, "source"),
            text(args, "fqn")
        )),
        "MixinConfigEditor.parse" => json!(MixinConfigEditor::parse(
            text(args, "content"),
            text(args, "path")
        )?),
        "MixinConfigEditor.containsEntry" => json!(MixinConfigEditor::contains_entry(
            text(args, "content"),
            text(args, "mixinClassName")
        )?),
        "MixinConfigEditor.addEntry" => json!(MixinConfigEditor::add_entry(
            text(args, "content"),
            text(args, "mixinClassName"),
            text(args, "arrayName")
        )?),
        "MixinConfigEditor.listMixinClasses" => json!(MixinConfigEditor::list_mixin_classes(
            text(args, "content")
        )?),
        "MixinSideResolver.explicitSideFromSource" => json!(
            MixinSideResolver::explicit_side_from_source(text(args, "source")).map(|s| match s {
                MixinSide::Client => "CLIENT",
                MixinSide::Server => "SERVER",
            })
        ),
        unknown => {
            let module = unknown.split('.').next().unwrap();
            assert!(
                [
                    "AnnotationContextExtractor",
                    "JavaTypeDescriptorResolver",
                    "MixinMemberDeclarationParser",
                    "MixinSemanticModelParser",
                    "MixinTargetResolver",
                    "SemanticCompletionContextExtractor",
                    "JavaStringContentDecoder"
                ]
                .contains(&module),
                "unmapped legacy API {unknown}"
            );
            source_replay::replay(row)
        }
    })
}
pub fn replay_case(case: &str) {
    let rows = include_str!("legacy_calls.jsonl")
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|row| row["legacyCase"] == case)
        .collect::<Vec<_>>();
    assert!(!rows.is_empty(), "uncaptured {case}");
    for (row_number, row) in rows.iter().enumerate() {
        let result = replay(row);
        if row.get("throwable").is_some() {
            assert!(
                result.is_err(),
                "{case}#{row_number} {} expected original exception",
                row["api"]
            );
        } else {
            let module = row["api"].as_str().unwrap().split('.').next().unwrap();
            let source = [
                "AnnotationContextExtractor",
                "JavaTypeDescriptorResolver",
                "MixinMemberDeclarationParser",
                "MixinSemanticModelParser",
                "MixinTargetResolver",
                "SemanticCompletionContextExtractor",
                "JavaStringContentDecoder",
            ]
            .contains(&module);
            let expected = if source {
                source_replay::expected_output(row)
            } else {
                row["output"].clone()
            };
            let actual =
                result.unwrap_or_else(|e| panic!("{case}#{row_number} {} failed: {e}", row["api"]));
            if source {
                assert!(
                    source_replay::kinds_match(row, &actual),
                    "{case}#{row_number} {} result variant",
                    row["api"]
                );
            }
            assert_eq!(
                if source {
                    source_replay::canonical(actual, "")
                } else {
                    actual
                },
                if source {
                    source_replay::canonical(expected, "")
                } else {
                    expected
                },
                "{case}#{row_number} {}",
                row["api"]
            );
        }
    }
}
