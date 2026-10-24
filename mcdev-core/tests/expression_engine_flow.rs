#[path = "fixtures/expression_engine_support.rs"]
mod support;
use mcdev_core::expression_engine::*;
use support::*;
fn resolver(left: &str, right: &str) -> Option<String> {
    if left == right {
        Some(left.into())
    } else if left.starts_with('L') && right.starts_with('L') {
        Some("Ljava/lang/Object;".into())
    } else {
        None
    }
}
fn matched(
    bytes: &[u8],
    name: &str,
    descriptor: &str,
    expressions: &[&str],
    pool: &IdentifierPool,
    kind: MatchContextType,
) -> Vec<ExpressionMatch> {
    let context = MatchContext {
        context_type: kind,
        identifier_pool: pool,
        allow_incomplete_list_inputs: false,
        common_super_class: &resolver,
        cancellation: &NO_CANCELLATION,
    };
    match match_expressions(
        bytes,
        name,
        descriptor,
        &expressions
            .iter()
            .map(|value| (*value).into())
            .collect::<Vec<_>>(),
        &context,
    )
    .unwrap()
    {
        MatchResult::Available(values) => values,
        MatchResult::Unavailable { reason, cause } => panic!("{reason}: {cause:?}"),
    }
}
#[test]
fn baseline_arithmetic_and_bytecode_order() {
    let bytes = sample();
    let values = matched(
        &bytes,
        "add",
        "(II)I",
        &["@(?+?)"],
        &IdentifierPool::default(),
        MatchContextType::ModifyExpressionValue,
    );
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].original_instruction_opcode, 0x60);
    assert_eq!(values[0].original_instruction_index, 2);
    assert_eq!(values[0].expression_start_offset, 2);
    assert_eq!(values[0].captured_type, TypeConstraint::exact("I"));
    assert_eq!(
        values[0].decorations[&DecorationKey::SimpleExpressionType],
        DecorationValue::TypeConstraint(TypeConstraint::exact("I"))
    );
    let values = matched(
        &bytes,
        "addAndMultiply",
        "(III)I",
        &["@(?+?)", "@(?*?)"],
        &IdentifierPool::default(),
        MatchContextType::ModifyExpressionValue,
    );
    assert_eq!(
        values
            .iter()
            .map(|v| v.expression_index)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(
        values
            .iter()
            .map(|v| v.original_instruction_opcode)
            .collect::<Vec<_>>(),
        [0x60, 0x68]
    );
}
#[test]
fn baseline_array_load_and_store_have_exact_operation_decorations() {
    let bytes = sample();
    for (method, descriptor, source, opcode, args, returned, names) in [
        (
            "arrayAccess",
            "([II)V",
            "@(?[?])",
            0x2e,
            vec![TypeConstraint::exact("[I"), TypeConstraint::exact("I")],
            "I",
            vec!["array", "index"],
        ),
        (
            "arrayStore",
            "([III)V",
            "?[?]=?",
            0x4f,
            vec![
                TypeConstraint::exact("[I"),
                TypeConstraint::exact("I"),
                TypeConstraint::exact("I"),
            ],
            "V",
            vec!["array", "index", "value"],
        ),
    ] {
        let values = matched(
            &bytes,
            method,
            descriptor,
            &[source],
            &IdentifierPool::default(),
            MatchContextType::WrapOperation,
        );
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].original_instruction_opcode, opcode);
        assert_eq!(
            values[0].decorations[&DecorationKey::SimpleOperationArgs],
            DecorationValue::TypeConstraints(args)
        );
        assert_eq!(
            values[0].decorations[&DecorationKey::SimpleOperationReturnType],
            DecorationValue::TypeConstraint(TypeConstraint::exact(returned))
        );
        assert_eq!(
            values[0].decorations[&DecorationKey::SimpleOperationParamNames],
            DecorationValue::ParamNames(names.iter().map(|name| (*name).into()).collect())
        );
    }
}
#[test]
fn baseline_literal_field_new_cast_instanceof_and_invocation_metadata() {
    let bytes = sample();
    for (method, descriptor, source, opcode) in [
        ("assignString", "()V", "@('hello')", 0x12),
        ("readSampleField", "()I", "@(?.?)", 0xb4),
        ("writeSampleField", "(I)V", "?.?=?", 0xb5),
        ("newString", "()Ljava/lang/String;", "@(new ?())", 0xbb),
        (
            "castString",
            "(Ljava/lang/Object;)Ljava/lang/String;",
            "@((?)?)",
            0xc0,
        ),
        (
            "isString",
            "(Ljava/lang/Object;)Z",
            "@(? instanceof ?)",
            0xc1,
        ),
        (
            "trim",
            "(Ljava/lang/String;)Ljava/lang/String;",
            "@(?.?())",
            0xb6,
        ),
    ] {
        let values = matched(
            &bytes,
            method,
            descriptor,
            &[source],
            &IdentifierPool::default(),
            MatchContextType::ModifyExpressionValue,
        );
        assert_eq!(values.len(), 1, "{method}");
        assert_eq!(values[0].original_instruction_opcode, opcode);
        match opcode {
            0xb4 | 0xb5 => assert!(matches!(
                values[0].instruction_metadata,
                Some(InstructionMetadata::FieldAccess { .. })
            )),
            0xbb | 0xc0 | 0xc1 => assert!(matches!(
                values[0].instruction_metadata,
                Some(InstructionMetadata::TypeOperation { .. })
            )),
            0xb6 => assert!(matches!(
                values[0].instruction_metadata,
                Some(InstructionMetadata::MethodInvocation { .. })
            )),
            _ => {}
        }
    }
}
#[test]
fn baseline_unary_comparison_expansion_preserves_physical_index_and_operation_types() {
    let bytes = sample();
    let values = matched(
        &bytes,
        "intEqualsZero",
        "(I)Z",
        &["? == 0"],
        &IdentifierPool::default(),
        MatchContextType::ModifyExpressionValue,
    );
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].original_instruction_opcode, 0x9a);
    assert_eq!(values[0].original_instruction_index, 1);
    assert_eq!(
        values[0].decorations[&DecorationKey::SimpleExpressionType],
        DecorationValue::TypeConstraint(TypeConstraint::exact("Z"))
    );
    let values = matched(
        &bytes,
        "intComparison",
        "(II)Z",
        &["? == ?"],
        &IdentifierPool::default(),
        MatchContextType::WrapOperation,
    );
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].decorations[&DecorationKey::SimpleOperationArgs],
        DecorationValue::TypeConstraints(vec![
            TypeConstraint::exact("I"),
            TypeConstraint::exact("I")
        ])
    );
}
