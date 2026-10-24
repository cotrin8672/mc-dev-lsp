use mcdev_core::expression_engine::*;
fn inner(expression: &Expression) -> &Expression {
    match &expression.kind {
        ExprKind::Capture(value) => value,
        _ => expression,
    }
}
#[test]
fn all_official_baseline_success_alternatives_parse() {
    for source in [
        "this.foo=x",
        "arr[i]=value",
        "name=value",
        "return x",
        "throw x",
        "@(this.foo)",
        "?",
        "this",
        "-23",
        "-1.5",
        "true",
        "null",
        "'hello'",
        "foo",
        "Type.class",
        "arr[i]",
        "this.foo",
        "super.foo(x)",
        "this.foo(x)",
        "foo(x)",
        "this::foo",
        "::foo",
        "Type::new",
        "~x",
        "new Type(x)",
        "new Type[]{x,y}",
        "new Type[x][]",
        "(Type)x",
        "x*y",
        "x+y",
        "x<<y",
        "x<y",
        "x instanceof Type",
        "x==y",
        "x&y",
        "x^y",
        "x|y",
    ] {
        assert!(
            parse_expression(source).is_ok(),
            "{source}: {:?}",
            parse_expression(source)
        );
    }
}
#[test]
fn baseline_and_grammar_malformed_inputs_are_syntax_failures() {
    for source in [
        "",
        "foo(",
        "1 + )",
        "a +",
        "++",
        "@)",
        "return",
        "new Type[]{}",
        "new Type[][1]",
        "a+=1",
        "this = 2",
        "a && b",
        "!a",
        "foo();",
        "\"foo\"",
        "'bad\\n'",
        "0x80000000",
        "9223372036854775808",
        "_",
        "a$",
        "日本語",
        "a\r+b",
        "a//b",
        "a/*b*/c",
    ] {
        let error = parse_expression(source).expect_err(source);
        assert!(!error.message.is_empty());
        assert!(error.line > 0);
    }
}
#[test]
fn implicit_capture_is_suppressed_by_any_explicit_capture() {
    assert!(matches!(
        parse_expression("a+b").unwrap().kind,
        ExprKind::Capture(_)
    ));
    let expression = parse_expression("a+@(b)").unwrap();
    assert!(matches!(expression.kind, ExprKind::Binary { .. }));
    let expression = parse_expression("@(a)+@(b)").unwrap();
    assert!(matches!(expression.kind, ExprKind::Binary { .. }));
}
#[test]
fn official_precedence_and_cast_ambiguities_are_retained() {
    let expression = parse_expression("a+b*c<<d==e&f^g|h").unwrap();
    let ExprKind::Binary {
        operator: BinaryOperator::BitOr,
        left,
        ..
    } = &inner(&expression).kind
    else {
        panic!("{expression:?}")
    };
    assert!(matches!(
        left.kind,
        ExprKind::Binary {
            operator: BinaryOperator::BitXor,
            ..
        }
    ));
    let expression = parse_expression("(Type)-23").unwrap();
    assert!(matches!(inner(&expression).kind, ExprKind::Cast { .. }));
    let expression = parse_expression("(value)+23").unwrap();
    assert!(matches!(
        inner(&expression).kind,
        ExprKind::Binary {
            operator: BinaryOperator::Add,
            ..
        }
    ));
    let expression = parse_expression("-x.y").unwrap();
    let ExprKind::Unary { value, .. } = &inner(&expression).kind else {
        panic!()
    };
    assert!(matches!(value.kind, ExprKind::MemberAccess { .. }));
}
#[test]
fn strings_unicode_offsets_and_numeric_boundaries() {
    let expression = parse_expression("'a\\'b\\\\c😀'").unwrap();
    assert_eq!(inner(&expression).kind, ExprKind::String("a'b\\c😀".into()));
    let expression = parse_expression("'😀' + @( 42 )").unwrap();
    let ExprKind::Binary { right, .. } = expression.kind else {
        panic!()
    };
    let ExprKind::Capture(value) = right.kind else {
        panic!()
    };
    assert_eq!(value.span.start, 9);
    let expression = parse_expression("-9223372036854775808").unwrap();
    assert_eq!(inner(&expression).kind, ExprKind::Int(i64::MIN));
    let expression = parse_expression("0x7fffffff").unwrap();
    assert_eq!(inner(&expression).kind, ExprKind::Int(i32::MAX as i64));
    assert!(parse_expression(".5").is_ok());
    assert!(parse_expression("1.").is_err());
}
#[test]
fn array_types_calls_and_references_preserve_typed_ast() {
    let expression = parse_expression("new Type[][]{x,y}").unwrap();
    assert!(
        matches!(&inner(&expression).kind,ExprKind::ArrayLiteral{element:TypeName{dimensions:1,..},values}if values.len()==2)
    );
    let expression = parse_expression("new Type[x][y][][]").unwrap();
    assert!(
        matches!(&inner(&expression).kind,ExprKind::NewArray{dimensions,blank_dimensions:2,..}if dimensions.len()==2)
    );
    assert!(matches!(
        inner(&parse_expression("Type[][].class").unwrap()).kind,
        ExprKind::ClassLiteral(TypeName { dimensions: 2, .. })
    ));
    assert!(matches!(
        inner(&parse_expression("?::new").unwrap()).kind,
        ExprKind::ConstructorReference(_)
    ));
    assert!(matches!(
        inner(&parse_expression("::?").unwrap()).kind,
        ExprKind::FreeMethodReference(Name::Wildcard)
    ));
    assert!(parse_expression("((?)?).?()").is_ok());
}
