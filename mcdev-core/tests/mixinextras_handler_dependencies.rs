//! Original assertions ran during one-time capture. Native replay verifies every argument and full result below.
use mcdev_core::{
    bytecode::*, extras_model::*, mixinextras_handler::*, mixinextras_occurrence::*,
    mixinextras_operation::*, *,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

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
struct Reads<'a>(&'a [Value]);
fn handler_service<'a>(reads: &'a Reads<'a>, args: &Value) -> HandlerSignatureService<'a> {
    let has_bytecode = args["hasBytecodeIndex"]
        .as_bool()
        .expect("original constructor capability required");
    HandlerSignatureService::new(reads, has_bytecode.then_some(reads as &dyn BytecodeIndex))
        .with_reference_types(reads)
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
            "fixtures/mixinextras_handler/host-reference-facts.json"
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
        &self
            .0
            .iter()
            .find(|r| r["api"] == api && r["args"] == args)
            .unwrap_or_else(|| panic!("uncaptured read {api} {args}"))["output"]
    }
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
fn supported(api: &str) -> bool {
    api.starts_with("ConstantAtArgsParser.")
        || api.starts_with("OperationSignatureRenderer.")
        || api.starts_with("InjectionPointOccurrenceResolver.")
        || api.starts_with("OperationCallValidator.")
        || api.starts_with("HandlerSignatureService.")
}
fn replay(row: &Value) -> (Value, Value) {
    let api = row["api"].as_str().unwrap();
    let args = &row["args"];
    let reads = Reads(row["reads"].as_array().unwrap());
    match api {
        "HandlerSignatureService.validateHandler" => {
            let site = decode(&args["site"]);
            let targets = decode::<Vec<String>>(&args["mixinTargets"]);
            let handler = decode(&args["handler"]);
            let context: Option<ExpressionContext> = decode(&args["resolvedContext"]);
            let actual = handler_service(&reads, args).validate_handler(
                args["source"].as_str().unwrap(),
                &site,
                &targets,
                &handler,
                context.as_ref(),
            );
            let expected: Vec<HandlerValidationIssue> = decode(&row["output"]);
            (output(actual), output(expected))
        }
        "HandlerSignatureService.generateHandlerStub" => {
            let site = decode(&args["site"]);
            let targets = decode::<Vec<String>>(&args["mixinTargets"]);
            let context: Option<ExpressionContext> = decode(&args["resolvedContext"]);
            let spec: Option<HandlerSignatureSpec> = decode(&args["signatureSpec"]);
            let actual = handler_service(&reads, args).generate_handler_stub(
                args["source"].as_str().unwrap(),
                &site,
                &targets,
                args["methodName"].as_str().unwrap(),
                args["indent"].as_str().unwrap(),
                context.as_ref(),
                spec.as_ref(),
            );
            (output(actual), row["output"].clone())
        }
        "HandlerSignatureService.expectedSignature" => {
            let site = decode(&args["site"]);
            let targets = decode::<Vec<String>>(&args["mixinTargets"]);
            let context: Option<ExpressionContext> = decode(&args["resolvedContext"]);
            let actual = handler_service(&reads, args).expected_signature(
                args["source"].as_str().unwrap(),
                &site,
                &targets,
                context.as_ref(),
            );
            let expected: Option<HandlerSignatureSpec> = decode(&row["output"]);
            (output(actual), output(expected))
        }
        "HandlerSignatureService.inferExpressionValueType" => {
            let site = decode(&args["site"]);
            let targets = decode::<Vec<String>>(&args["mixinTargets"]);
            let method = serde_json::from_value(args["targetMethod"].clone()).unwrap();
            let context: Option<ExpressionContext> = decode(&args["resolvedContext"]);
            (
                output(handler_service(&reads, args).infer_expression_value_type(
                    args["source"].as_str().unwrap(),
                    &site,
                    &method,
                    &targets,
                    context.as_ref(),
                )),
                row["output"].clone(),
            )
        }
        "HandlerSignatureService.wrapWithConditionTargetStatus" => {
            let site = decode(&args["site"]);
            let targets = decode::<Vec<String>>(&args["mixinTargets"]);
            let context: Option<ExpressionContext> = decode(&args["resolvedContext"]);
            let actual = handler_service(&reads, args).wrap_with_condition_target_status(
                &site,
                &targets,
                args["source"].as_str(),
                context.as_ref(),
            );
            (
                output(actual),
                json!(pascal(row["output"].as_str().unwrap())),
            )
        }
        "HandlerSignatureService.returnTypeMatches" => {
            let expected = decode(&args["expected"]);
            let handler = decode(&args["handler"]);
            (
                output(handler_service(&reads, args).return_type_matches(&expected, &handler)),
                row["output"].clone(),
            )
        }
        "HandlerSignatureService.parameterTypeMatches" => {
            let expected = decode(&args["expected"]);
            let actual = decode(&args["actual"]);
            (
                output(handler_service(&reads, args).parameter_type_matches(&expected, &actual)),
                row["output"].clone(),
            )
        }
        "HandlerSignatureService.specializeIntLikeSignature" => {
            let expected = decode(&args["expected"]);
            let handler = decode(&args["handler"]);
            let actual =
                handler_service(&reads, args).specialize_int_like_signature(&expected, &handler);
            let expected: HandlerSignatureSpec = decode(&row["output"]);
            (output(actual), output(expected))
        }
        "HandlerSignatureService.validateCommonSugarConstraints" => {
            let handler = decode(&args["handler"]);
            let target: Option<MethodIndexEntry> =
                serde_json::from_value(args["targetMethod"].clone()).unwrap();
            let annotation: Option<MixinExtrasAnnotation> = args["injectorAnnotation"]
                .as_str()
                .map(|s| serde_json::from_value(json!(pascal(s))).unwrap());
            let actual = handler_service(&reads, args).validate_common_sugar_constraints(
                &handler,
                target.as_ref(),
                annotation,
            );
            let expected: Vec<HandlerValidationIssue> = decode(&row["output"]);
            (output(actual), output(expected))
        }
        "HandlerSignatureService.resolveTargetMethod" => {
            let targets = decode::<Vec<String>>(&args["mixinTargets"]);
            let actual = handler_service(&reads, args)
                .resolve_target_method(&targets, args["methodAttribute"].as_str().unwrap());
            let expected: Option<MethodIndexEntry> =
                serde_json::from_value(row["output"].clone()).unwrap();
            (output(actual), output(expected))
        }
        "HandlerSignatureService.findAnnotationSites"
        | "HandlerSignatureService.findSugarHandlerAnnotationSites" => {
            let source = args["source"].as_str().unwrap();
            let actual = if api.ends_with("findAnnotationSites") {
                HandlerSignatureService::find_annotation_sites(source)
            } else {
                HandlerSignatureService::find_sugar_handler_annotation_sites(source)
            };
            let expected: Vec<MixinExtrasAnnotationSite> = decode(&row["output"]);
            (output(actual), output(expected))
        }
        "HandlerSignatureService.parseHandlerMethod" => {
            let source = args["source"].as_str().unwrap();
            let utf16 = args["startOffset"].as_u64().unwrap() as usize;
            let start = utf16_offset(source, utf16);
            let imports: Option<mcdev_core::mixin_source::JavaSourceImports> =
                decode(&args["imports"]);
            let actual = start.and_then(|start| {
                HandlerSignatureService::parse_handler_method(source, start, imports.as_ref())
            });
            let expected: Option<HandlerMethodDeclaration> = decode(&row["output"]);
            (output(actual), output(expected))
        }
        "HandlerSignatureService.isKotlinCompanionMember" => {
            let source = args["source"].as_str().unwrap();
            let offset = utf16_offset(source, args["offset"].as_u64().unwrap() as usize);
            (
                output(offset.is_some_and(|offset| {
                    HandlerSignatureService::is_kotlin_companion_member(source, offset)
                })),
                row["output"].clone(),
            )
        }
        "HandlerSignatureService.enrichHandlerTypes" => {
            let handler = decode(&args["handler"]);
            let actual = HandlerSignatureService::enrich_handler_types_with_reference_types(
                &handler,
                &reads,
                args["source"].as_str(),
                Some(&reads),
            );
            let expected: HandlerMethodDeclaration = decode(&row["output"]);
            (output(actual), output(expected))
        }
        "HandlerSignatureService.shareRefValueDescriptor" => (
            output(HandlerSignatureService::share_ref_value_descriptor(
                args["descriptor"].as_str().unwrap(),
            )),
            row["output"].clone(),
        ),
        "ConstantAtArgsParser.parse" => (
            output(ConstantAtArgsParser::parse(&decode::<Vec<String>>(
                &args["atArgs"],
            ))),
            row["output"].clone(),
        ),
        "OperationSignatureRenderer.readableType" => (
            output(OperationSignatureRenderer::readable_type(
                args["descriptor"].as_str().unwrap(),
            )),
            row["output"].clone(),
        ),
        "OperationSignatureRenderer.renderOperationType" => (
            output(OperationSignatureRenderer::render_operation_type(
                args["returnTypeDescriptor"].as_str().unwrap(),
            )),
            row["output"].clone(),
        ),
        "OperationSignatureRenderer.descriptorFromTypeName" => (
            output(OperationSignatureRenderer::descriptor_from_type_name(
                args["typeName"].as_str().unwrap(),
                &reads,
            )),
            row["output"].clone(),
        ),
        "InjectionPointOccurrenceResolver.resolve" => {
            let site = decode(&args["site"]);
            let target = serde_json::from_value(args["targetMethod"].clone()).unwrap();
            let indices: Option<BTreeSet<i32>> = decode(&args["expressionInstructionIndices"]);
            let value = InjectionPointOccurrenceResolver::new(&reads).resolve(
                args["owner"].as_str().unwrap(),
                &target,
                &site,
                indices.as_ref(),
            );
            let expected: Option<BTreeSet<i32>> = decode(&row["output"]);
            (output(value), output(expected))
        }
        "OperationCallValidator.resolveOperationCallLayout" => {
            let site = decode(&args["site"]);
            let handler = decode(&args["handler"]);
            let expected: Option<HandlerSignatureSpec> = decode(&args["expected"]);
            let actual = OperationCallValidator::resolve_operation_call_layout(
                &site,
                &handler,
                expected.as_ref(),
            );
            let expected: Option<OperationCallValidationLayout> = decode(&row["output"]);
            (output(actual), output(expected))
        }
        "OperationCallValidator.validate" => {
            let site = decode(&args["site"]);
            let layout: Option<OperationCallValidationLayout> = decode(&args["layout"]);
            let typing = args["argumentTypeResolver"]["hasResolver"]
                .as_bool()
                .unwrap_or(false)
                .then_some(&reads as &dyn OperationCallArgumentTypeResolver);
            let actual = OperationCallValidator::validate(
                args["source"].as_str().unwrap(),
                &site,
                layout.as_ref(),
                typing,
            );
            let expected: Vec<OperationCallIssue> = decode(&row["output"]);
            (output(actual), output(expected))
        }
        other => panic!("unimplemented fixture API {other}"),
    }
}
#[test]
fn every_handler_call_replays_exact_inputs_and_results() {
    let fixtures =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mixinextras_handler");
    let calls = std::fs::read_to_string(fixtures.join("handler-calls.jsonl")).unwrap();
    let manifest: Vec<Value> = serde_json::from_str(
        &std::fs::read_to_string(fixtures.join("handler-cases.json")).unwrap(),
    )
    .unwrap();
    let mut seen = BTreeMap::<String, usize>::new();
    let mut failures = vec![];
    let mut count = 0;
    for (line, text) in calls.lines().enumerate() {
        let row: Value = serde_json::from_str(text).unwrap();
        if !supported(row["api"].as_str().unwrap()) {
            continue;
        }
        let id = row["legacyCase"].as_str().unwrap().to_string();
        *seen.entry(id.clone()).or_default() += 1;
        count += 1;
        match std::panic::catch_unwind(||replay(&row)){Ok((actual,expected))if actual==expected=>{},Ok((actual,expected))=>failures.push(json!({"line":line+1,"legacyCase":id,"api":row["api"],"expected":expected,"actual":actual})),Err(error)=>failures.push(json!({"line":line+1,"legacyCase":id,"api":row["api"],"panic":error.downcast_ref::<String>().cloned().or_else(||error.downcast_ref::<&str>().map(|s|s.to_string())).unwrap_or_default()}))}
    }
    let direct = [
        "ConstantAtArgsParserTest",
        "InjectionPointOccurrenceResolverTest",
        "OperationCallValidatorTest",
        "HandlerSignatureServiceTest",
        "CoreHandlerSignatureCompletionTest",
        "KotlinHandlerSignatureTest",
    ];
    let deferred = [
        "facadeNormalizesDeclarationSnippetIndentationAtBothInsertionBoundaries",
        "facadeRoutesAccessorInvokerAndOverwriteToDeclarationSnippets",
    ];
    let mut direct_count = 0;
    for case in manifest {
        let id = case["legacyCase"].as_str().unwrap();
        if direct.iter().any(|name| id.contains(name)) {
            assert_eq!(case["status"], "passed");
            if deferred.iter().any(|name| id.ends_with(name)) {
                continue;
            }
            assert!(
                seen.contains_key(id) || id.ends_with("mixinExtrasAnnotationMapsFromSimpleName"),
                "missing exact case {id}"
            );
            direct_count += 1;
        }
    }
    assert_eq!(direct_count, 392);
    assert_eq!(count, 4772);
    assert_eq!(seen.len(), 391);
    let log = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../build/rust-test-logs/mixinextras-handler-dependency-failures.json");
    std::fs::create_dir_all(log.parent().unwrap()).unwrap();
    std::fs::write(&log, serde_json::to_string_pretty(&failures).unwrap()).unwrap();
    assert!(failures.is_empty(),"{} full-output failures across {count} captured calls/{direct_count} direct legacy cases; bounded report {}",failures.len(),log.display());
}

/// HandlerSignatureServiceTest.kt::mixinExtrasAnnotationMapsFromSimpleName
#[test]
fn mixin_extras_annotation_maps_from_simple_name() {
    assert_eq!(
        MixinExtrasAnnotation::from_simple_name("WrapOperation"),
        Some(MixinExtrasAnnotation::WrapOperation)
    );
    assert_eq!(
        MixinExtrasAnnotation::from_simple_name("Expression"),
        Some(MixinExtrasAnnotation::Expression)
    );
}
