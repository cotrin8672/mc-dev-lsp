#[path = "fixtures/expression_engine_support.rs"]
mod support;
use mcdev_core::bytecode::*;
use mcdev_core::expression_engine::*;
use support::{op, ClassFile};

fn fixture(units: &[u16]) -> Vec<u8> {
    let mut class = ClassFile::default();
    let utf = class.utf16(units);
    let constant = class.item([vec![8], utf.to_be_bytes().to_vec()].concat(), false);
    class.method(
        "run",
        "()Ljava/lang/String;",
        9,
        0,
        [op(0x13, constant), vec![0xb0]].concat(),
    );
    class.finish("Utf16Constant")
}
#[test]
fn every_jvm_string_code_unit_survives_the_class_reader_and_serialization() {
    for (name, units) in [
        ("empty", vec![]),
        ("NUL", vec![0]),
        ("BMP", vec![0x41, 0x65e5]),
        ("supplementary", vec![0xd83d, 0xde42]),
        ("isolated_high", vec![0xd800]),
        ("isolated_low", vec![0xdc00]),
        ("paired_surrogates", vec![0xd800, 0xdc00]),
        ("reversed_surrogates", vec![0xdc00, 0xd800]),
        ("consecutive_high", vec![0xd800, 0xdbff]),
        (
            "mixed",
            vec![0x61, 0xd800, 0, 0xd83d, 0xde42, 0xdc00, 0x22, 0x5c, 0xa],
        ),
    ] {
        let bytes = fixture(&units);
        let targets = extract_instructions(&bytes, "run", "()Ljava/lang/String;").unwrap();
        let constant = targets
            .iter()
            .find_map(|candidate| candidate.constant_value.as_ref())
            .unwrap();
        assert_eq!(constant.string_utf16(), Some(units.clone()), "{name}");
        let json = serde_json::to_string(constant).unwrap();
        let decoded: ConstantValue = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, *constant, "{name}");
        assert_eq!(decoded.string_utf16(), Some(units.clone()), "{name}");
        assert_eq!(
            matches!(constant, ConstantValue::StringValue(_)),
            String::from_utf16(&units).is_ok(),
            "{name}"
        );
        let pool = IdentifierPool::default();
        let resolve = |_: &str, _: &str| None;
        let context = MatchContext {
            context_type: MatchContextType::ModifyExpressionValue,
            identifier_pool: &pool,
            allow_incomplete_list_inputs: false,
            common_super_class: &resolve,
            cancellation: &NO_CANCELLATION,
        };
        let flow = analyze_method(&bytes, "run", "()Ljava/lang/String;", &context)
            .unwrap()
            .unwrap();
        assert!(flow.instructions.iter().any(|instruction|matches!(&instruction.operand,InstructionOperand::Constant(JvmConstant::String(actual))if *actual==units)),"{name}");
    }
}
#[test]
fn isolated_surrogates_keep_distinct_constant_identity_and_safe_java_literals() {
    let high = ConstantValue::from_utf16(vec![0xd800]);
    let low = ConstantValue::from_utf16(vec![0xdc00]);
    assert_ne!(high, low);
    assert_ne!(high, ConstantValue::StringValue("�".into()));
    assert_ne!(high, ConstantValue::StringValue("\\uD800".into()));
    assert_eq!(constant_label(Some(&high)), "\"\\uD800\"");
    assert_eq!(constant_label(Some(&low)), "\"\\uDC00\"");
    assert_eq!(high.java_string_literal().as_deref(), Some("\"\\uD800\""));
    let mixed = ConstantValue::from_utf16(vec![0x22, 0x5c, 0xa, 0xd800, 0, 0xd83d, 0xde42]);
    assert_eq!(
        mixed.java_string_literal().as_deref(),
        Some("\"\\\"\\\\\\n\\uD800\\u0000🙂\"")
    );
    assert_eq!(
        ConstantValue::StringValue("a🙂".into()),
        ConstantValue::StringUtf16Value("a🙂".encode_utf16().collect())
    );
}
#[test]
fn string_representations_share_ordinals_by_utf16_value_and_never_by_rendered_label() {
    let values = [
        ConstantValue::StringValue("a🙂".into()),
        ConstantValue::StringUtf16Value("a🙂".encode_utf16().collect()),
        ConstantValue::StringValue("a🙂".into()),
        ConstantValue::StringUtf16Value(vec![0xd800]),
        ConstantValue::StringValue("\\uD800".into()),
        ConstantValue::StringUtf16Value(vec![0xd800]),
    ];
    let mut candidates = values
        .into_iter()
        .map(|value| {
            let mut candidate = AtTargetCandidate::new(AtTargetKind::Constant, "", "", "");
            candidate.constant_value = Some(value);
            candidate
        })
        .collect::<Vec<_>>();
    assign_ordinals(&mut candidates);
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.ordinal)
            .collect::<Vec<_>>(),
        [Some(0), Some(1), Some(2), Some(0), Some(0), Some(1)]
    );
}
