#[path = "fixtures/expression_engine_support.rs"]
mod support;
use mcdev_core::expression_engine::*;
use mcdev_core::mixin_source::JavaStringContentDecoder;
use support::{op, ClassFile};
fn literal_class(units: &[u16], member: bool) -> Vec<u8> {
    let mut class = ClassFile::default();
    let utf = class.utf16(units);
    let constant = class.item([vec![8], utf.to_be_bytes().to_vec()].concat(), false);
    let mut code = op(0x13, constant);
    if member {
        let call = class.member(
            10,
            "Helper",
            "someMethod",
            "(Ljava/lang/String;)Ljava/lang/String;",
        );
        let trim = class.member(10, "java/lang/String", "trim", "()Ljava/lang/String;");
        code.extend(op(0xb8, call));
        code.extend(op(0xb6, trim));
    }
    code.push(0xb0);
    class.method("run", "()Ljava/lang/String;", 9, 0, code);
    class.finish("Utf16Expressions")
}
fn decoded(raw: &str) -> Vec<u16> {
    JavaStringContentDecoder::decode_expression_content_utf16(raw, 0, raw.len(), false, false)
        .unwrap()
        .value
}
fn context(pool: &IdentifierPool) -> MatchContext<'_> {
    static RESOLVE: fn(&str, &str) -> Option<String> = |_, _| None;
    MatchContext {
        context_type: MatchContextType::ModifyExpressionValue,
        identifier_pool: pool,
        allow_incomplete_list_inputs: false,
        common_super_class: &RESOLVE,
        cancellation: &NO_CANCELLATION,
    }
}
#[test]
fn java_unicode_escape_to_official_literal_keeps_each_jvm_code_unit() {
    for (raw, payload) in [
        (r"'\uD800'", vec![0xd800]),
        (r"'\uDC00'", vec![0xdc00]),
        (r"'\uD800\uDC00'", vec![0xd800, 0xdc00]),
        (r"'\uD83D\uDE42'", vec![0xd83d, 0xde42]),
        (r"'a\uD800\u0000\uDC00'", vec![0x61, 0xd800, 0, 0xdc00]),
        ("'�'", vec![0xfffd]),
    ] {
        let expression = decoded(raw);
        let parsed = parse_expression_utf16(&expression).unwrap();
        let ExprKind::Capture(inner) = &parsed.kind else {
            panic!("implicit capture missing")
        };
        let units = match &inner.kind {
            ExprKind::String(value) => value.encode_utf16().collect::<Vec<_>>(),
            ExprKind::StringUtf16(value) => value.clone(),
            other => panic!("unexpected literal {other:?}"),
        };
        assert_eq!(units, payload, "{raw}");
        let pool = IdentifierPool::default();
        let context = context(&pool);
        let bytes = literal_class(&payload, false);
        let result = match_expressions_utf16(
            &bytes,
            "run",
            "()Ljava/lang/String;",
            std::slice::from_ref(&expression),
            &context,
        )
        .unwrap();
        let MatchResult::Available(values) = result else {
            panic!("literal unavailable")
        };
        assert_eq!(values.len(), 1, "{raw}");
        assert_eq!(values[0].original_instruction_index, 0);
        assert_eq!(
            values[0].captured_type,
            TypeConstraint::exact("Ljava/lang/String;")
        );
        if String::from_utf16(&payload).is_err() {
            assert_eq!(
                match_expressions_utf16(
                    &bytes,
                    "run",
                    "()Ljava/lang/String;",
                    &["'�'".encode_utf16().collect()],
                    &context
                )
                .unwrap(),
                MatchResult::Available(vec![])
            );
        }
    }
}
#[test]
fn raw_escape_offsets_and_codepoint_spans_survive_lone_and_paired_units() {
    let raw = r"'\uD800'";
    let source =
        JavaStringContentDecoder::decode_expression_content_utf16(raw, 0, raw.len(), false, false)
            .unwrap();
    assert_eq!(source.source_offsets, [0, 1, 7, 8]);
    let parsed = parse_expression_utf16(&source.value).unwrap();
    assert_eq!(parsed.span, SourceSpan { start: 0, end: 3 });
    let ExprKind::Capture(inner) = parsed.kind else {
        panic!()
    };
    assert_eq!(inner.span, SourceSpan { start: 0, end: 3 });
    let raw = r"'\uD83D\uDE42'";
    let source =
        JavaStringContentDecoder::decode_expression_content_utf16(raw, 0, raw.len(), false, false)
            .unwrap();
    assert_eq!(source.source_offsets, [0, 1, 7, 13, 14]);
    let parsed = parse_expression_utf16(&source.value).unwrap();
    assert_eq!(parsed.span, SourceSpan { start: 0, end: 4 });
    let ExprKind::Capture(inner) = parsed.kind else {
        panic!()
    };
    assert_eq!(inner.span, SourceSpan { start: 0, end: 3 });
    assert!(parse_expression_utf16(&[0xd800]).is_err());
    assert!(parse_expression("'\\uD800'").is_err(),"official grammar does not accept Java Unicode escapes; only the shared source decoder processes those");
    assert_eq!(
        codepoint_offset_to_utf16(&[0x27, 0xd83d, 0xde42, 0x27], 2),
        Some(3)
    );
    assert_eq!(codepoint_offset_to_utf16(&[0x27, 0xd800, 0x27], 2), Some(2));
    assert_eq!(codepoint_offset_to_utf16(&[0xd800], 2), None);
}
#[test]
fn receiver_probes_and_completion_preserve_unpaired_literal_arguments() {
    let payload = vec![0xd800];
    let receiver = decoded(r"someMethod('\uD800')");
    let pool = IdentifierPool::default();
    let probes = receiver_probes_utf16(&receiver, &pool).unwrap();
    assert_eq!(probes.len(), 2);
    assert!(probes
        .iter()
        .all(|(_, value)| value.contains(&0xd800) && !value.contains(&0xfffd)));
    let bytes = literal_class(&payload, true);
    let context = context(&pool);
    let result = complete_receiver_members_utf16(
        &bytes,
        "run",
        "()Ljava/lang/String;",
        &receiver,
        "tr",
        &context,
    )
    .unwrap();
    let MemberCompletionResult::Available(values) = result else {
        panic!("completion unavailable")
    };
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].kind, MemberKind::Method);
    assert_eq!(values[0].owner_internal_name, "java/lang/String");
    assert_eq!(values[0].name, "trim");
    assert_eq!(values[0].original_instruction_index, 2);
    assert_eq!(
        complete_receiver_members_utf16(
            &bytes,
            "run",
            "()Ljava/lang/String;",
            &"someMethod('�')".encode_utf16().collect::<Vec<_>>(),
            "",
            &context
        )
        .unwrap(),
        MemberCompletionResult::Available(vec![])
    );
}
