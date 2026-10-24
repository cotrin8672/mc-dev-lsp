//! Exact requests/results captured while executing all original engine @Test assertions.
//! The fixture generator is an optional migration tool; cargo test never starts a JVM.
use mcdev_core::descriptor::{
    parse_field_descriptor, parse_method_descriptor, FieldSelector, MethodSelector, Pattern,
};
use mcdev_core::expression_engine::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;
pub(crate) fn rows(api: &str) -> Vec<Value> {
    include_str!("expression_engine/legacy_calls.jsonl")
        .lines()
        .chain(include_str!("expression_engine/additional_calls.jsonl").lines())
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|row| row["api"] == api)
        .collect()
}
pub(crate) fn s(value: &Value) -> String {
    value.as_str().unwrap().into()
}
pub(crate) fn source_units(value: &Value) -> Vec<u16> {
    if let Some(value) = value.as_str() {
        value.encode_utf16().collect()
    } else {
        value["utf16"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as u16)
            .collect()
    }
}
pub(crate) fn parse_source(value: &Value) -> Result<Expression, SyntaxFailure> {
    if let Some(value) = value.as_str() {
        parse_expression(value)
    } else {
        parse_expression_utf16(&source_units(value))
    }
}
pub(crate) fn strings(value: &Value) -> Vec<String> {
    value.as_array().unwrap().iter().map(s).collect()
}
pub(crate) fn span(value: &Value) -> Option<SourceSpan> {
    if value.is_null() {
        None
    } else {
        Some(SourceSpan {
            start: value["first"].as_u64().unwrap() as usize,
            end: (value["last"].as_i64().unwrap() + 1) as usize,
        })
    }
}
pub(crate) fn string_pattern(value: &Value) -> Pattern<String> {
    if value["kind"] == "Any" {
        Pattern::Any
    } else {
        Pattern::Exact(s(&value["fields"]["value"]))
    }
}
pub(crate) fn method_selector(value: &Value) -> MethodSelector {
    MethodSelector {
        owner: string_pattern(&value["owner"]),
        name: string_pattern(&value["name"]),
        descriptor: if value["descriptor"]["kind"] == "Any" {
            Pattern::Any
        } else {
            let desc = &value["descriptor"]["fields"]["value"];
            let descriptor = format!(
                "({}){}",
                strings(&desc["parameters"]).concat(),
                s(&desc["returnType"])
            );
            Pattern::Exact(parse_method_descriptor(&descriptor).unwrap())
        },
    }
}
pub(crate) fn member(value: &Value) -> MemberDefinition {
    let fields = &value["value"];
    match value["kind"].as_str().unwrap() {
        "ArrayLengthDef" => MemberDefinition::ArrayLength,
        "LocalIndexMemberDefinition" => {
            MemberDefinition::LocalIndex(fields["index"].as_i64().unwrap() as i32)
        }
        "MethodSelectorMemberDefinition" => {
            MemberDefinition::Method(method_selector(&fields["selector"]))
        }
        "FieldSelectorMemberDefinition" => {
            let selector = &fields["selector"];
            MemberDefinition::Field(FieldSelector {
                owner: string_pattern(&selector["owner"]),
                name: string_pattern(&selector["name"]),
                descriptor: if selector["descriptor"]["kind"] == "Any" {
                    Pattern::Any
                } else {
                    Pattern::Exact(
                        parse_field_descriptor(
                            selector["descriptor"]["fields"]["value"].as_str().unwrap(),
                        )
                        .unwrap(),
                    )
                },
            })
        }
        kind => panic!("unknown member kind {kind}"),
    }
}
pub(crate) fn local_spec(value: &Value) -> LocalSpec {
    LocalSpec {
        args_only: value["argsOnly"].as_bool().unwrap(),
        index: value["index"].as_i64().map(|v| v as i32),
        ordinal: value["ordinal"].as_i64().map(|v| v as i32),
        names: strings(&value["names"]).into_iter().collect(),
        print: value["print"].as_bool().unwrap(),
        type_class_name: value["typeClassName"].as_str().map(str::to_owned),
    }
}
pub(crate) fn pool(value: &Value) -> IdentifierPool {
    IdentifierPool {
        members: value["members"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(id, values)| {
                (
                    id.clone(),
                    values.as_array().unwrap().iter().map(member).collect(),
                )
            })
            .collect(),
        types: value["types"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(id, values)| {
                (
                    id.clone(),
                    values
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| s(&v["value"]["type"]))
                        .collect(),
                )
            })
            .collect(),
        locals: value["locals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| LocalDefinition {
                id: s(&v["id"]),
                spec: local_spec(&v["spec"]),
                descriptor: s(&v["descriptor"]),
            })
            .collect(),
    }
}
pub(crate) fn constraint(value: &Value) -> TypeConstraint {
    match value["kind"].as_str().unwrap() {
        "Exact" => TypeConstraint::exact(s(&value["fields"]["descriptor"])),
        "IntLike" => TypeConstraint::IntLike,
        kind => panic!("unknown constraint {kind}"),
    }
}
pub(crate) fn decorations(value: &Value) -> Decorations {
    value
        .as_object()
        .unwrap()
        .iter()
        .map(|(key, value)| match key.as_str() {
            "SIMPLE_EXPRESSION_TYPE" => (
                DecorationKey::SimpleExpressionType,
                DecorationValue::TypeConstraint(constraint(&value["constraint"])),
            ),
            "SIMPLE_OPERATION_ARGS" => (
                DecorationKey::SimpleOperationArgs,
                DecorationValue::TypeConstraints(
                    value["constraints"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(constraint)
                        .collect(),
                ),
            ),
            "SIMPLE_OPERATION_PARAM_NAMES" => (
                DecorationKey::SimpleOperationParamNames,
                DecorationValue::ParamNames(strings(&value["names"])),
            ),
            "SIMPLE_OPERATION_RETURN_TYPE" => (
                DecorationKey::SimpleOperationReturnType,
                DecorationValue::TypeConstraint(constraint(&value["constraint"])),
            ),
            "IS_STRING_CONCAT_EXPRESSION" => (
                DecorationKey::IsStringConcatExpression,
                DecorationValue::Flag(value["value"].as_bool().unwrap()),
            ),
            other => panic!("unknown decoration {other}"),
        })
        .collect()
}
pub(crate) fn metadata(value: &Value) -> Option<InstructionMetadata> {
    if value.is_null() {
        None
    } else if let Some(ty) = value["typeInternalName"].as_str() {
        Some(InstructionMetadata::TypeOperation {
            type_internal_name: ty.into(),
        })
    } else if let Some(interface) = value["isInterface"].as_bool() {
        Some(InstructionMetadata::MethodInvocation {
            owner_internal_name: s(&value["ownerInternalName"]),
            name: s(&value["name"]),
            descriptor: s(&value["descriptor"]),
            is_interface: interface,
        })
    } else {
        Some(InstructionMetadata::FieldAccess {
            owner_internal_name: s(&value["ownerInternalName"]),
            name: s(&value["name"]),
            descriptor: s(&value["descriptor"]),
        })
    }
}
pub(crate) fn expression_match(value: &Value) -> ExpressionMatch {
    ExpressionMatch {
        expression_index: value["expressionIndex"].as_u64().unwrap() as usize,
        expression_start_offset: value["expressionStartOffset"].as_u64().unwrap() as usize,
        original_instruction_opcode: value["originalInstructionOpcode"].as_u64().unwrap() as u8,
        original_instruction_index: value["originalInstructionIndex"].as_u64().unwrap() as usize,
        captured_type: constraint(&value["capturedType"]),
        decorations: decorations(&value["decorations"]),
        instruction_metadata: metadata(&value["instructionMetadata"]),
    }
}
pub(crate) fn context_kind(value: &Value) -> MatchContextType {
    match value.as_str().unwrap() {
        "CUSTOM" => MatchContextType::Custom,
        "INJECT" => MatchContextType::Inject,
        "MODIFY_ARG" => MatchContextType::ModifyArg,
        "MODIFY_ARGS" => MatchContextType::ModifyArgs,
        "MODIFY_CONSTANT" => MatchContextType::ModifyConstant,
        "MODIFY_EXPRESSION_VALUE" => MatchContextType::ModifyExpressionValue,
        "MODIFY_RECEIVER" => MatchContextType::ModifyReceiver,
        "MODIFY_RETURN_VALUE" => MatchContextType::ModifyReturnValue,
        "MODIFY_VARIABLE" => MatchContextType::ModifyVariable,
        "REDIRECT" => MatchContextType::Redirect,
        "SLICE" => MatchContextType::Slice,
        "WRAP_OPERATION" => MatchContextType::WrapOperation,
        "WRAP_WITH_CONDITION" => MatchContextType::WrapWithCondition,
        kind => panic!("unknown context {kind}"),
    }
}
pub(crate) fn resolver(row: &Value) -> BTreeMap<(String, String), Option<String>> {
    row["reads"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["api"] == "CommonSuperClassResolver.resolve")
        .map(|r| {
            (
                (s(&r["args"][0]), s(&r["args"][1])),
                r["output"].as_str().map(str::to_owned),
            )
        })
        .collect()
}
pub(crate) fn bytes(row: &Value) -> Vec<u8> {
    static CLASSES: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    let classes = CLASSES.get_or_init(|| {
        serde_json::from_str(include_str!("expression_engine/class_bytes.json")).unwrap()
    });
    let values = if let Some(bytes) = row["args"].get("classBytes") {
        bytes
    } else {
        &classes[row["args"]["classBytesKey"].as_str().unwrap()]
    };
    values
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap() as u8)
        .collect()
}
pub(crate) fn check_rows(api: &str, mut check: impl FnMut(&Value) -> Result<(), String>) {
    let mut failures = Vec::new();
    let rows = rows(api);
    assert!(!rows.is_empty());
    for (row_index, row) in rows.iter().enumerate() {
        if let Err(error) = check(row) {
            failures.push(format!(
                "{} [call {row_index}]: {error}",
                row["legacyCase"].as_str().unwrap()
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} {api} rows failed:\n{}",
        failures.len(),
        rows.len(),
        failures.join("\n")
    );
}
pub(crate) fn equal<T: std::fmt::Debug + PartialEq>(actual: T, expected: T) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!("actual {actual:?}; expected {expected:?}"))
    }
}
fn replay_jsr_fatal(row: &Value) -> Result<(), String> {
    let deltas: Value =
        serde_json::from_str(include_str!("expression_engine/jsr_fatal_deltas.json")).unwrap();
    let delta=deltas.as_array().unwrap().iter().find(|delta|delta["legacyCase"]==row["legacyCase"]&&delta["api"]==row["api"]&&delta["classSha256"]==row["args"]["classBytesKey"]&&delta["originalErrorClass"]==row["throwable"]&&delta["originalErrorMessage"]==row["message"]).ok_or_else(||"unexpected fatal oracle result; only the recorded JSR assertions have an approved adaptation".to_owned())?;
    let args = &row["args"];
    let pool = pool(&args["identifierPool"]);
    let resolve = |_: &str, _: &str| None;
    let context = MatchContext {
        context_type: context_kind(&args["contextType"]),
        identifier_pool: &pool,
        allow_incomplete_list_inputs: args["allowIncompleteListInputs"].as_bool().unwrap(),
        common_super_class: &resolve,
        cancellation: &NO_CANCELLATION,
    };
    let bytes = bytes(row);
    let decoded = decode_method(
        &bytes,
        args["methodName"].as_str().unwrap(),
        args["methodDescriptor"].as_str().unwrap(),
        &context,
    )
    .unwrap()
    .unwrap();
    equal(decoded.instructions[0].opcode, 0xa8)?;
    equal(decoded.instructions[0].byte_offset, 0)?;
    let actual = match_expressions(
        &bytes,
        args["methodName"].as_str().unwrap(),
        args["methodDescriptor"].as_str().unwrap(),
        &strings(&args["expressions"]),
        &context,
    )
    .unwrap();
    equal(
        actual,
        MatchResult::Unavailable {
            reason: s(&delta["nativeReason"]),
            cause: Some(s(&delta["nativeCause"])),
        },
    )
}
fn match_key(value: &ExpressionMatch) -> (usize, usize, usize, u8) {
    (
        value.expression_index,
        value.expression_start_offset,
        value.original_instruction_index,
        value.original_instruction_opcode,
    )
}
fn compare_matches(
    actual: MatchResult,
    expected: Vec<ExpressionMatch>,
    row: &Value,
) -> Result<(), String> {
    if row["api"] == "OfficialExpressionMatcher.matchComponents" {
        return match actual {
            MatchResult::Available(actual) => {
                let mut actual = actual
                    .iter()
                    .map(|v| serde_json::to_string(v).unwrap())
                    .collect::<Vec<_>>();
                let mut expected = expected
                    .iter()
                    .map(|v| serde_json::to_string(v).unwrap())
                    .collect::<Vec<_>>();
                actual.sort();
                expected.sort();
                equal(actual, expected)
            }
            other => Err(format!("expected Available components, got {other:?}")),
        };
    }
    let MatchResult::Available(values) = &actual else {
        return strict_matches(actual, expected);
    };
    if *values == expected {
        return Ok(());
    }
    // Only the additional oracle's explicitly recorded IdentityHashMap collisions
    // admit alternatives. Every legacy request remains a strict ordered comparison.
    if row["legacyClass"] == "io.github.mcdev.core.mixinextras.EngineAdditionalOracle"
        && values.len() == expected.len()
    {
        static AMBIGUITIES: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
        let ambiguities = AMBIGUITIES.get_or_init(|| {
            serde_json::from_str(include_str!("expression_engine/identity_ambiguities.json"))
                .unwrap()
        });
        let mut valid = true;
        for (actual, expected) in values.iter().zip(&expected) {
            if actual == expected {
                continue;
            }
            if match_key(actual) != match_key(expected) {
                valid = false;
                break;
            }
            let key = match_key(actual);
            let legal = ambiguities["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|entry| {
                    entry["case"] == row["legacyCase"]
                        && entry["args"] == row["args"]
                        && entry["key"] == json!([key.0, key.1, key.2, key.3])
                });
            if !legal.is_some_and(|entry| {
                entry["legalAlternatives"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|candidate| expression_match(candidate) == *actual)
            }) {
                valid = false;
                break;
            }
        }
        if valid {
            return Ok(());
        }
    }
    strict_matches(actual, expected)
}
fn strict_matches(actual: MatchResult, expected: Vec<ExpressionMatch>) -> Result<(), String> {
    match actual {
        MatchResult::Available(actual) => {
            if actual.len() != expected.len() {
                return Err(format!(
                    "match count actual{} expected{}; actual keys{:?}; expected keys{:?}",
                    actual.len(),
                    expected.len(),
                    actual
                        .iter()
                        .map(|m| (
                            m.original_instruction_index,
                            m.original_instruction_opcode,
                            &m.captured_type
                        ))
                        .collect::<Vec<_>>(),
                    expected
                        .iter()
                        .map(|m| (
                            m.original_instruction_index,
                            m.original_instruction_opcode,
                            &m.captured_type
                        ))
                        .collect::<Vec<_>>()
                ));
            }
            for (index, (actual, expected)) in actual.into_iter().zip(expected).enumerate() {
                if actual != expected {
                    return Err(format!(
                        "match{index}: actual{actual:?}; expected{expected:?}"
                    ));
                }
            }
            Ok(())
        }
        other => Err(format!("expected Available, got{other:?}")),
    }
}
pub(crate) fn unavailable(
    reason: &str,
    cause: Option<String>,
    expected: &Value,
) -> Result<(), String> {
    let expected_reason = expected["reason"].as_str().unwrap();
    if expected_reason.starts_with("Failed to parse expression:") {
        if !reason.starts_with("Failed to parse expression:") {
            return Err(format!("expected parse failure, got {reason}"));
        }
    } else if reason != expected_reason {
        return Err(format!("reason {reason:?}, expected {expected_reason:?}"));
    }
    equal(cause.is_some(), !expected["cause"].is_null())
}
#[derive(Debug)]
struct CancelMarker {
    kind: String,
    message: String,
}
impl std::fmt::Display for CancelMarker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}
impl std::error::Error for CancelMarker {}
struct PhaseCancellation {
    phase: CancellationPhase,
    marker: std::sync::Arc<dyn std::error::Error + Send + Sync>,
}
impl CancellationChecker for PhaseCancellation {
    fn check_cancelled(&self) -> Result<(), Cancellation> {
        Ok(())
    }
    fn check_phase(&self, phase: CancellationPhase) -> Result<(), Cancellation> {
        if phase == self.phase {
            Err(Cancellation(self.marker.clone()))
        } else {
            Ok(())
        }
    }
}
pub(crate) fn original_cancellation(row: &Value) -> Result<(), String> {
    if row["throwable"] == "java.lang.AssertionError" {
        return replay_jsr_fatal(row);
    }
    let args = &row["args"];
    let pool = pool(&args["identifierPool"]);
    let resolved = resolver(row);
    let resolve = |left: &str, right: &str| {
        resolved
            .get(&(left.into(), right.into()))
            .or_else(|| resolved.get(&(right.into(), left.into())))
            .cloned()
            .flatten()
            .or_else(|| {
                if row["legacyClass"] == "io.github.mcdev.core.mixinextras.EngineAdditionalOracle" {
                    Some(if left == right {
                        left.into()
                    } else {
                        "Ljava/lang/Object;".into()
                    })
                } else {
                    None
                }
            })
    };
    let phase = if row["legacyCase"]
        .as_str()
        .unwrap()
        .contains("analyzerCancellation")
    {
        CancellationPhase::LocalAnalysis
    } else if row["legacyCase"]
        .as_str()
        .unwrap()
        .ends_with("FromFlowInterpreter")
    {
        CancellationPhase::FlowInterpretation
    } else if row["legacyCase"]
        .as_str()
        .unwrap()
        .ends_with("FromMatchFailureRecheck")
    {
        CancellationPhase::MatchFailureRecheck
    } else {
        CancellationPhase::Matching
    };
    let marker: std::sync::Arc<dyn std::error::Error + Send + Sync> =
        std::sync::Arc::new(CancelMarker {
            kind: s(&row["throwable"]),
            message: row["message"].as_str().unwrap_or_default().into(),
        });
    let cancellation = PhaseCancellation {
        phase,
        marker: marker.clone(),
    };
    let context = MatchContext {
        context_type: context_kind(&args["contextType"]),
        identifier_pool: &pool,
        allow_incomplete_list_inputs: args["allowIncompleteListInputs"].as_bool().unwrap(),
        common_super_class: &resolve,
        cancellation: &cancellation,
    };
    let result = if row["api"] == "OfficialExpressionMatcher.match" {
        match_expressions(
            &bytes(row),
            args["methodName"].as_str().unwrap(),
            args["methodDescriptor"].as_str().unwrap(),
            &strings(&args["expressions"]),
            &context,
        )
        .map(|_| ())
    } else {
        complete_receiver_members(
            &bytes(row),
            args["methodName"].as_str().unwrap(),
            args["methodDescriptor"].as_str().unwrap(),
            args["receiverExpression"].as_str().unwrap(),
            args["memberPrefix"].as_str().unwrap(),
            &context,
        )
        .map(|_| ())
    };
    match result {
        Err(error) if std::sync::Arc::ptr_eq(&error.0, &marker) => Ok(()),
        other => Err(format!(
            "expected original cancellation marker from {phase:?}, got {other:?}"
        )),
    }
}
pub(crate) fn ast_name(name: &Name, ty: bool) -> Value {
    match name {
        Name::Wildcard => json!({"kind":"WildcardIdentifier","fields":{}}),
        Name::Defined(name) => {
            json!({"kind":if ty{"DefinedTypeIdentifier"}else{"DefinedMemberIdentifier"},"fields":{"name":name}})
        }
    }
}
pub(crate) fn ast_type(ty: &TypeName) -> Value {
    let name = ast_name(&ty.name, true);
    if ty.dimensions == 0 {
        name
    } else {
        json!({"kind":"ArrayTypeIdentifier","fields":{"dims":ty.dimensions,"elementType":name}})
    }
}
pub(crate) fn ast_list(values: &[Expression], source: &str) -> Vec<Value> {
    values.iter().map(|v| ast(v, source)).collect()
}
pub(crate) fn ast(value: &Expression, source: &str) -> Value {
    let (kind, fields) = match &value.kind {
        ExprKind::Capture(inner) => (
            "CapturingExpression",
            json!({"expression":ast(inner,source)}),
        ),
        ExprKind::Wildcard => ("WildcardExpression", json!({})),
        ExprKind::This => ("ThisExpression", json!({})),
        ExprKind::Int(value) => ("IntLiteralExpression", json!({"value":value})),
        ExprKind::Decimal(value) => ("DecimalLiteralExpression", json!({"value":value})),
        ExprKind::Boolean(value) => ("BooleanLiteralExpression", json!({"value":value})),
        ExprKind::Null => ("NullLiteralExpression", json!({})),
        ExprKind::String(value) => ("StringLiteralExpression", json!({"value":value})),
        ExprKind::StringUtf16(value) => {
            ("StringLiteralExpression", json!({"value":{"utf16":value}}))
        }
        ExprKind::Identifier(value) => ("IdentifierExpression", json!({"identifier":value})),
        ExprKind::ClassLiteral(ty) => ("ClassConstantExpression", json!({"type":ast_type(ty)})),
        ExprKind::ArrayAccess { array, index } => (
            "ArrayAccessExpression",
            json!({"arr":ast(array,source),"index":ast(index,source)}),
        ),
        ExprKind::MemberAccess { receiver, name } => (
            "MemberAccessExpression",
            json!({"receiver":ast(receiver,source),"name":ast_name(name,false)}),
        ),
        ExprKind::MethodCall {
            receiver,
            name,
            arguments,
        } => (
            "MethodCallExpression",
            json!({"receiver":ast(receiver,source),"name":ast_name(name,false),"arguments":ast_list(arguments,source)}),
        ),
        ExprKind::StaticCall { name, arguments } => (
            "StaticMethodCallExpression",
            json!({"name":ast_name(name,false),"arguments":ast_list(arguments,source)}),
        ),
        ExprKind::SuperCall { name, arguments } => (
            "SuperCallExpression",
            json!({"name":ast_name(name,false),"arguments":ast_list(arguments,source)}),
        ),
        ExprKind::BoundMethodReference { receiver, name } => (
            "BoundMethodReferenceExpression",
            json!({"receiver":ast(receiver,source),"name":ast_name(name,false)}),
        ),
        ExprKind::FreeMethodReference(name) => (
            "FreeMethodReferenceExpression",
            json!({"name":ast_name(name,false)}),
        ),
        ExprKind::ConstructorReference(ty) => (
            "ConstructorReferenceExpression",
            json!({"type":ast_type(ty)}),
        ),
        ExprKind::Unary { operator, value } => (
            "UnaryExpression",
            json!({"operator":match operator{UnaryOperator::Negate=>"MINUS",UnaryOperator::BitNot=>"BITWISE_NOT"},"expression":ast(value,source)}),
        ),
        ExprKind::New { target, arguments } => (
            "InstantiationExpression",
            json!({"type":ast_type(target),"arguments":ast_list(arguments,source)}),
        ),
        ExprKind::ArrayLiteral { element, values } => (
            "ArrayLiteralExpression",
            json!({"elementType":ast_type(element),"values":ast_list(values,source)}),
        ),
        ExprKind::NewArray {
            element,
            dimensions,
            blank_dimensions,
        } => (
            "NewArrayExpression",
            json!({"innerType":ast_type(element),"dims":ast_list(dimensions,source),"blankDims":blank_dimensions}),
        ),
        ExprKind::Cast { target, value } => (
            "CastExpression",
            json!({"type":ast_type(target),"expression":ast(value,source)}),
        ),
        ExprKind::InstanceOf { value, target } => (
            "InstanceofExpression",
            json!({"type":ast_type(target),"expression":ast(value,source)}),
        ),
        ExprKind::AssignIdentifier { name, value } => (
            "IdentifierAssignmentExpression",
            json!({"identifier":ast_name(name,false),"value":ast(value,source)}),
        ),
        ExprKind::AssignMember {
            receiver,
            name,
            value,
        } => (
            "MemberAssignmentExpression",
            json!({"receiver":ast(receiver,source),"name":ast_name(name,false),"value":ast(value,source)}),
        ),
        ExprKind::ArrayStore {
            array,
            index,
            value,
        } => (
            "ArrayStoreExpression",
            json!({"arr":ast(array,source),"index":ast(index,source),"value":ast(value,source)}),
        ),
        ExprKind::Return(value) => ("ReturnExpression", json!({"value":ast(value,source)})),
        ExprKind::Throw(value) => ("ThrowExpression", json!({"value":ast(value,source)})),
        ExprKind::Binary {
            left,
            operator,
            right,
        } => {
            let operator = match operator {
                BinaryOperator::Multiply => "MULT",
                BinaryOperator::Divide => "DIV",
                BinaryOperator::Modulo => "MOD",
                BinaryOperator::Add => "PLUS",
                BinaryOperator::Subtract => "MINUS",
                BinaryOperator::ShiftLeft => "SHL",
                BinaryOperator::ShiftRight => "SHR",
                BinaryOperator::UnsignedShiftRight => "USHR",
                BinaryOperator::Less => "LT",
                BinaryOperator::LessEqual => "LE",
                BinaryOperator::Greater => "GT",
                BinaryOperator::GreaterEqual => "GE",
                BinaryOperator::Equal => "EQ",
                BinaryOperator::NotEqual => "NE",
                BinaryOperator::BitAnd => "BITWISE_AND",
                BinaryOperator::BitXor => "BITWISE_XOR",
                BinaryOperator::BitOr => "BITWISE_OR",
            };
            (
                if matches!(operator, "LT" | "LE" | "GT" | "GE" | "EQ" | "NE") {
                    "ComparisonExpression"
                } else {
                    "BinaryExpression"
                },
                json!({"left":ast(left,source),"operator":operator,"right":ast(right,source)}),
            )
        }
    };
    json!({"kind":kind,"source":{"expression":source,"startIndex":value.span.start,"endIndex":value.span.end-1},"fields":fields})
}
pub(crate) fn ast_source(value: &Expression, source: &Value) -> Value {
    fn patch(value: &mut Value, source: &Value) {
        match value {
            Value::Object(map) => {
                if let Some(location) = map.get_mut("source") {
                    location["expression"] = source.clone();
                }
                for value in map.values_mut() {
                    patch(value, source);
                }
            }
            Value::Array(values) => {
                for value in values {
                    patch(value, source);
                }
            }
            _ => {}
        }
    }
    let mut value = ast(value, source.as_str().unwrap_or_default());
    patch(&mut value, source);
    value
}

pub fn replay_match_request(row: &Value) -> Result<(), String> {
    if row.get("throwable").is_some() {
        return original_cancellation(row);
    }
    let args = &row["args"];
    let pool = pool(&args["identifierPool"]);
    let resolved = resolver(row);
    let resolve = |left: &str, right: &str| {
        resolved
            .get(&(left.into(), right.into()))
            .or_else(|| resolved.get(&(right.into(), left.into())))
            .cloned()
            .flatten()
            .or_else(|| {
                if row["legacyClass"] == "io.github.mcdev.core.mixinextras.EngineAdditionalOracle" {
                    Some(if left == right {
                        left.into()
                    } else {
                        "Ljava/lang/Object;".into()
                    })
                } else {
                    None
                }
            })
    };
    let context = MatchContext {
        context_type: context_kind(&args["contextType"]),
        identifier_pool: &pool,
        allow_incomplete_list_inputs: args["allowIncompleteListInputs"].as_bool().unwrap(),
        common_super_class: &resolve,
        cancellation: &NO_CANCELLATION,
    };
    let matcher = if row["api"] == "OfficialExpressionMatcher.matchComponents" {
        match_expression_components
    } else {
        match_expressions
    };
    let actual = if args["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .all(Value::is_string)
    {
        matcher(
            &bytes(row),
            args["methodName"].as_str().unwrap(),
            args["methodDescriptor"].as_str().unwrap(),
            &strings(&args["expressions"]),
            &context,
        )
    } else {
        match_expressions_utf16(
            &bytes(row),
            args["methodName"].as_str().unwrap(),
            args["methodDescriptor"].as_str().unwrap(),
            &args["expressions"]
                .as_array()
                .unwrap()
                .iter()
                .map(source_units)
                .collect::<Vec<_>>(),
            &context,
        )
    }
    .unwrap();
    let expected = &row["output"];
    if row["outputType"].as_str().unwrap().ends_with("$Available") {
        let expected = expected["matches"]
            .as_array()
            .unwrap()
            .iter()
            .map(expression_match)
            .collect();
        compare_matches(actual, expected, row)
    } else {
        match actual {
            MatchResult::Unavailable { reason, cause } => unavailable(&reason, cause, expected),
            other => Err(format!(
                "expected Unavailable {}, actual {other:?}",
                expected["reason"]
            )),
        }
    }
}

pub fn replay_build_request(row: &Value) -> Result<(), String> {
    let definitions = row["args"]["index"]["definitions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| DefinitionInput {
            id: v["id"].as_str().map(str::to_owned),
            raw_method_references: strings(&v["rawMethodReferences"]),
            raw_field_references: strings(&v["rawFieldReferences"]),
            class_literal_type_names: strings(&v["classLiteralTypeNames"]),
            local_specs: v["localSpecs"]
                .as_array()
                .unwrap()
                .iter()
                .map(local_spec)
                .collect(),
            parse_issues: v["parseIssues"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| DefinitionParseIssue {
                    attribute: s(&i["attribute"]),
                    raw_value: s(&i["rawValue"]),
                    message: s(&i["message"]),
                    body_range: span(&i["bodyRange"]).unwrap(),
                })
                .collect(),
            source_range: span(&v["sourceRange"]),
        })
        .collect::<Vec<_>>();
    let resolved = row["reads"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v["api"] == "ClassLiteralTypeNameResolver.resolve")
        .map(|v| {
            (
                s(&v["args"][0]),
                v["output"]["descriptor"]
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| v["output"].as_str().map(str::to_owned)),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let built = build_identifier_pool(&definitions, &|name| resolved.get(name).cloned().flatten());
    equal(built.pool, pool(&row["output"]["pool"]))?;
    let expected = row["output"]["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| IdentifierPoolBuildIssue {
            definition_id: v["definitionId"].as_str().map(str::to_owned),
            attribute: s(&v["attribute"]),
            raw_value: s(&v["rawValue"]),
            message: s(&v["message"]),
            source_range: span(&v["sourceRange"]),
        })
        .collect();
    equal(built.issues, expected)
}
