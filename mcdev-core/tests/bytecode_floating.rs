use mcdev_core::bytecode::{constant_label, ConstantValue};
#[path = "fixtures/expression_engine_support.rs"]
mod support;
use mcdev_core::expression_engine::*;
#[test]
fn java21_float_and_double_labels_keep_ieee_bits_and_shortest_decimal_conventions() {
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/floating_java21.json")).unwrap();
    let mut failures = Vec::new();
    for row in rows.as_array().unwrap() {
        let bits = u64::from_str_radix(row["bits"].as_str().unwrap(), 16).unwrap();
        let value = if row["kind"] == "Float" {
            ConstantValue::FloatValue(f32::from_bits(bits as u32))
        } else {
            ConstantValue::DoubleValue(f64::from_bits(bits))
        };
        let actual = constant_label(Some(&value));
        if actual != row["label"].as_str().unwrap() {
            failures.push(format!(
                "{} {}: {actual}, expected {}",
                row["kind"], row["bits"], row["label"]
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} floating presentation mismatches of {}:\n{}",
        failures.len(),
        rows.as_array().unwrap().len(),
        failures
            .iter()
            .take(24)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
#[test]
fn decimal_matching_uses_java21_minimum_subnormal_strings() {
    let mut class = support::ClassFile::default();
    let value = class.item([vec![4], 1u32.to_be_bytes().to_vec()].concat(), false);
    class.method(
        "run",
        "()F",
        9,
        0,
        [support::op(0x13, value), vec![0xae]].concat(),
    );
    let bytes = class.finish("MinFloat");
    let pool = IdentifierPool::default();
    let resolve = |_: &str, _: &str| None;
    let context = MatchContext {
        context_type: MatchContextType::ModifyExpressionValue,
        identifier_pool: &pool,
        allow_incomplete_list_inputs: false,
        common_super_class: &resolve,
        cancellation: &NO_CANCELLATION,
    };
    let result = match_expressions(
        &bytes,
        "run",
        "()F",
        &[
            format!("0.{}14", "0".repeat(44)),
            format!("0.{}1", "0".repeat(44)),
        ],
        &context,
    )
    .unwrap();
    let MatchResult::Available(values) = result else {
        panic!("float matching unavailable")
    };
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].expression_index, 0);
    assert_eq!(values[0].captured_type, TypeConstraint::exact("F"));
}
