use mcdev_core::host_source::*;
use serde_json::{json, Value};
#[allow(dead_code)]
#[path = "fixtures/mixin_source/replay.rs"]
mod shared;
fn kind(v: &Value) -> &str {
    v["_class"]
        .as_str()
        .unwrap_or("")
        .rsplit('$')
        .next()
        .unwrap_or("")
}
fn validity(v: &Value) -> HostNodeValidity {
    let flags = v["flags"].as_i64().unwrap_or(0);
    HostNodeValidity {
        recovered: flags & 2 != 0 || v["recovered"] == true,
        malformed: flags & 4 != 0 || v["malformed"] == true,
    }
}
fn ty(v: &Value) -> HostTypeFact {
    if v.is_null() || kind(v) == "RecoveredBinding" || v["recovered"] == true {
        return HostTypeFact::Recovered;
    }
    if let Some(name) = v["name"].as_str() {
        let primitive = match name {
            "void" => HostPrimitive::Void,
            "boolean" => HostPrimitive::Boolean,
            "byte" => HostPrimitive::Byte,
            "char" => HostPrimitive::Char,
            "short" => HostPrimitive::Short,
            "int" => HostPrimitive::Int,
            "long" => HostPrimitive::Long,
            "float" => HostPrimitive::Float,
            "double" => HostPrimitive::Double,
            _ => return HostTypeFact::Unresolved,
        };
        return HostTypeFact::Primitive(primitive);
    }
    if let Some(name) = v["binaryName"].as_str() {
        return HostTypeFact::Class {
            internal_name: name.replace('.', "/"),
        };
    }
    if let Some(element) = v.get("element") {
        return HostTypeFact::Array {
            element: Box::new(ty(element)),
            dimensions: v["totalDims"].as_u64().unwrap_or(1) as usize,
        };
    }
    if let Some(component) = v.get("component") {
        return HostTypeFact::Array {
            element: Box::new(ty(component)),
            dimensions: 1,
        };
    }
    HostTypeFact::Unresolved
}
fn range(v: &Value, source: Option<&str>) -> Option<std::ops::Range<usize>> {
    let start = v["startPosition"].as_i64()?;
    let len = v["nodeLength"].as_i64()?;
    if start < 0 || len <= 0 {
        return None;
    }
    let source = source?;
    Some(shared::byte(source, start)?..shared::byte(source, start + len)?)
}
fn annotation(v: &Value, source: Option<&str>) -> HostAnnotationFact {
    let mut members = vec![];
    if let Some(values) = v["members"].as_object() {
        for (key, value) in values {
            members.push((key.clone(), node(value, source)));
        }
    }
    if let Some(value) = v.get("value") {
        members.push(("value".into(), node(value, source)));
    }
    if let Some(duplicate) = v["duplicateMember"].as_str() {
        if let Some((_, value)) = members.iter().find(|(n, _)| n == duplicate) {
            members.push((duplicate.into(), value.clone()));
        }
    }
    let source_range = range(v, source);
    let body_range = source_range.as_ref().and_then(|r| {
        let source = source?;
        let open = source[r.clone()].find('(')? + r.start;
        let end = if source.as_bytes().get(r.end.checked_sub(1)?) == Some(&b')') {
            r.end - 1
        } else {
            r.end
        };
        Some((open + 1).min(end)..end)
    });
    HostAnnotationFact {
        source_name: v["typeName"]
            .as_str()
            .or_else(|| v["label"].as_str())
            .unwrap_or("")
            .into(),
        qualified_name: v["fqn"].as_str().map(Into::into),
        validity: validity(v),
        exists: true,
        members,
        members_available: v["unreadablePair"] != true,
        source_range,
        body_range,
    }
}
fn node(v: &Value, source: Option<&str>) -> HostExpressionFact {
    let k = kind(v);
    let expression = if k.ends_with("Annotation") {
        HostExpressionKind::Annotation(Box::new(annotation(v, source)))
    } else if k == "ArrayInitializer" {
        HostExpressionKind::Array(
            v["values"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| {
                    if v.is_null() {
                        None
                    } else {
                        Some(node(v, source))
                    }
                })
                .collect(),
        )
    } else if k == "TypeLiteral" {
        HostExpressionKind::TypeLiteral(ty(&v["binding"]))
    } else if let Some(value) = v.get("constant") {
        HostExpressionKind::Constant(if value.is_null() {
            HostConstant::Null
        } else if let Some(s) = value.as_str() {
            HostConstant::String(s.encode_utf16().collect())
        } else if let Some(b) = value.as_bool() {
            HostConstant::Boolean(b)
        } else if k == "LongLiteral" {
            HostConstant::Long(value.as_i64().unwrap())
        } else {
            HostConstant::Int(value.as_i64().unwrap() as i32)
        })
    } else {
        HostExpressionKind::Unresolved
    };
    HostExpressionFact {
        validity: validity(v),
        kind: expression,
        source_text: v["_sourceText"].as_str().unwrap_or("").into(),
        source_range: range(v, source),
    }
}
fn clean(v: &Value) -> Value {
    match v {
        Value::Object(map) => {
            if v["_class"].as_str() == Some("kotlin.ranges.IntRange") {
                let start = v["first"].as_i64().unwrap();
                let last = v["last"].as_i64().unwrap();
                return json!({"start":start,"end":(last+1).max(start)});
            }
            Value::Object(
                map.iter()
                    .filter(|(k, _)| !k.starts_with('_'))
                    .map(|(k, v)| (k.clone(), clean(v)))
                    .collect(),
            )
        }
        Value::Array(a) => Value::Array(a.iter().map(clean).collect()),
        _ => v.clone(),
    }
}
fn normalized(value: Value) -> Value {
    shared::canonical(value, "")
}
#[test]
fn all_legacy_constant_binding_and_resolved_context_outputs_use_normalized_host_facts() {
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    let mut comparisons = 0;
    for (i, line) in include_str!("fixtures/host_source/adapter-calls.jsonl")
        .lines()
        .enumerate()
    {
        let row: Value = serde_json::from_str(line).unwrap();
        let api = row["api"].as_str().unwrap();
        if api.starts_with("JdtTypeSignatureConverter.") {
            continue;
        } // Exact14-input table in host_source_types.
        if row["legacyCase"]
            .as_str()
            .unwrap()
            .ends_with("::cancelledConstantResolutionPropagatesAndCanRecover")
        {
            continue;
        } // Stateful provider test in host_source_cache.
        assert!(
            row.get("throwable").is_none(),
            "unexpected throwable row {}",
            i + 1
        );
        let args = &row["args"];
        let source = args["source"].as_str();
        let (actual, expected) = if api == "JdtBindingDescriptorConverter.descriptorFromBinding" {
            (
                json!(ty(&args["binding"]).descriptor()),
                row["output"].clone(),
            )
        } else if api == "JdtMixinExtrasResolvedContextExtractor.extract" {
            let mut annotations: Vec<_> = args["methodDeclaration"]["modifierList"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| annotation(a, source))
                .collect();
            let corrected_slice = if row["legacyCase"]
                .as_str()
                .unwrap()
                .ends_with("::mapsJdtDefinitionParseIssueToOffendingValueRange")
            {
                let a = annotations
                    .iter_mut()
                    .find(|a| a.source_name == "Definition")
                    .unwrap();
                let node = &mut a
                    .members
                    .iter_mut()
                    .find(|(name, _)| name == "method")
                    .unwrap()
                    .1;
                let source = source.unwrap();
                let range = node.source_range.clone().unwrap();
                let slice = source[range].to_string();
                node.source_text = slice.clone();
                Some(slice)
            } else {
                None
            };
            let range = shared::deserialize(&clean(&args["handlerRange"]));
            let expected_kind = row["outputType"]
                .as_str()
                .unwrap()
                .rsplit('$')
                .next()
                .unwrap();
            let actual = match extract_resolved_extras_context(&annotations, range) {
                HostResolvedContextResult::NotHandler => {
                    assert_eq!(expected_kind, "NotMixinExtrasHandler");
                    json!({})
                }
                HostResolvedContextResult::Unavailable { reason } => {
                    assert_eq!(expected_kind, "Unavailable");
                    json!({"reason":reason})
                }
                HostResolvedContextResult::Resolved(context) => {
                    assert_eq!(expected_kind, "Resolved");
                    json!({"context":context})
                }
            };
            let mut expected = clean(&row["output"]);
            if let Some(slice) = corrected_slice {
                expected["context"]["context"]["definitionIndex"]["definitions"][0]
                    ["parseIssues"][0]["rawValue"] = json!(slice);
            }
            (actual, expected)
        } else {
            let fact = node(&args["node"], source);
            match api.rsplit('.').next().unwrap() {
                "constantString" => (json!(fact.constant_string()), row["output"].clone()),
                "constantBoolean" => (json!(fact.constant_boolean()), row["output"].clone()),
                "constantInt" => (json!(fact.constant_int()), row["output"].clone()),
                "stringArray" => (json!(fact.string_array()), row["output"].clone()),
                "typeLiteralDescriptor" => {
                    (json!(fact.type_literal_descriptor()), row["output"].clone())
                }
                "typeLiteralSourceName" => (
                    json!(fact.type_literal_source_name()),
                    row["output"].clone(),
                ),
                "typeArray" => (json!(fact.type_array()), row["output"].clone()),
                "typeArraySourceNames" => {
                    (json!(fact.type_array_source_names()), row["output"].clone())
                }
                "nestedAnnotationArray" => {
                    let expected = row["output"]
                        .as_array()
                        .map(|a| a.iter().map(|a| annotation(a, source)).collect::<Vec<_>>());
                    (json!(fact.nested_annotations()), json!(expected))
                }
                _ => panic!("unhandled {api}"),
            }
        };
        assert_eq!(
            normalized(actual),
            normalized(expected),
            "row {} {api} {}",
            i + 1,
            row["legacyCase"]
        );
        comparisons += 1;
        *counts
            .entry(row["legacyCase"].as_str().unwrap().into())
            .or_default() += 1;
    }
    assert_eq!(comparisons, 324);
    assert_eq!(counts.len(), 44);
}
#[test]
fn malformed_definition_issue_retains_actual_host_snapshot_value_text() {
    let source = "@Definition(id = \"broken\", method = 123)";
    let start = source.find("123").unwrap();
    let mut definition = annotation(
        &json!({"typeName":"Definition","fqn":"com.llamalad7.mixinextras.expression.Definition","members":{},"startPosition":0,"nodeLength":source.len()}),
        Some(source),
    );
    definition.members.push((
        "method".into(),
        HostExpressionFact {
            validity: Default::default(),
            kind: HostExpressionKind::Constant(HostConstant::Int(123)),
            source_text: source[start..start + 3].into(),
            source_range: Some(start..start + 3),
        },
    ));
    let handler = annotation(
        &json!({"typeName":"ModifyExpressionValue","fqn":"com.llamalad7.mixinextras.injector.ModifyExpressionValue","members":{}}),
        None,
    );
    let HostResolvedContextResult::Resolved(result) =
        extract_resolved_extras_context(&[handler, definition], Default::default())
    else {
        panic!("resolved handler expected")
    };
    let issue = &result.context.definition_index.definitions[0].parse_issues[0];
    assert_eq!(issue.raw_value, &source[start..start + 3]);
    assert_eq!(issue.raw_value, "123");
    assert_eq!(issue.message, "expected a string or string array");
    assert_eq!(issue.attribute, "method");
}
