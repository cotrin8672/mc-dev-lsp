//! Direct ports of the zero-call constructor/index/resolver and annotation-context cases.
#[test]
fn maps_parse_issue_to_source_and_keeps_valid_identifier_binding() {
    let source = "@Definition(id = \"good\", method = 123, field = \"Lcom/example/Foo;value:I\")";
    let index = DefinitionsAnnotationParser::parse(source, 0, None);
    let built = OfficialExpressionIdentifierPoolBuilder::build(
        &index,
        &ClassLiteralTypeNameResolver::FAIL_CLOSED,
    );
    assert_eq!(
        built
            .issues
            .iter()
            .map(|i| i.attribute.as_str())
            .collect::<Vec<_>>(),
        vec!["method"]
    );
    assert_eq!(built.issues[0].raw_value, "123");
    let start = source.find("123").unwrap();
    assert_eq!(
        built.issues[0].source_range,
        Some(mcdev_core::expression_engine::SourceSpan {
            start,
            end: start + 3
        })
    );
    assert!(built.pool.identifier_exists("good"));
}
#[test]
fn malformed_id_produces_one_parse_issue_instead_of_duplicate_missing_id_issues() {
    let index = DefinitionsAnnotationParser::parse("@Definition(id = notAString)", 0, None);
    let built = OfficialExpressionIdentifierPoolBuilder::build(
        &index,
        &ClassLiteralTypeNameResolver::FAIL_CLOSED,
    );
    assert_eq!(
        built
            .issues
            .iter()
            .map(|i| i.attribute.as_str())
            .collect::<Vec<_>>(),
        vec!["id"]
    );
    assert_eq!(built.issues[0].raw_value, "notAString");
}
use mcdev_core::{extras_model::*, mixin_source::*, mixinextras_expression::*, *};
struct Index(Vec<ClassIndexEntry>);
impl ClassIndex for Index {
    fn find_classes(&self, p: &str, l: usize) -> Vec<ClassIndexEntry> {
        self.0
            .iter()
            .filter(|c| c.simple_name.starts_with(p))
            .take(l)
            .cloned()
            .collect()
    }
    fn find_class(&self, n: &str) -> Option<ClassIndexEntry> {
        self.0.iter().find(|c| c.internal_name == n).cloned()
    }
    fn find_class_by_fqn(&self, n: &str) -> Option<ClassIndexEntry> {
        self.0.iter().find(|c| c.fqn() == n).cloned()
    }
    fn get_methods(&self, _: &str) -> Vec<MethodIndexEntry> {
        vec![]
    }
    fn get_fields(&self, _: &str) -> Vec<FieldIndexEntry> {
        vec![]
    }
}
fn index(names: &[&str]) -> Index {
    Index(
        names
            .iter()
            .map(|n| {
                let (package, simple) = n.rsplit_once('/').unwrap_or(("", n));
                ClassIndexEntry {
                    simple_name: simple.into(),
                    package_name: package.replace('/', "."),
                    internal_name: (*n).into(),
                }
            })
            .collect(),
    )
}
fn resolve(source: &str, classes: &[&str], name: &str) -> Option<String> {
    ClassLiteralTypeNameResolver::for_source(source, &index(classes)).resolve(name)
}
#[test]
fn kotlin_builtins_and_explicit_aliases_resolve_to_jvm_types_without_changing_java_names() {
    let source="import com.example.Foo as Imported\nimport kotlin.Int as NumberValue\nimport game.outer as Box";
    let classes = ["com/example/Foo", "game/outer$Inner"];
    for (name, expected) in [
        ("Int", "I"),
        ("NumberValue", "I"),
        ("String", "Ljava/lang/String;"),
        ("IntArray", "[I"),
        ("Imported", "Lcom/example/Foo;"),
        ("Box.Inner", "Lgame/outer$Inner;"),
    ] {
        assert_eq!(
            resolve(source, &classes, name).as_deref(),
            Some(expected),
            "{name}"
        );
    }
    for name in ["Missing", "()V"] {
        assert_eq!(resolve(source, &classes, name), None);
    }
    assert_eq!(resolve(source, &classes, "[I").as_deref(), Some("[I"));
    assert_eq!(
        resolve(
            "package patch\nimport one.*\nimport two.*",
            &["one/Foo", "two/Foo", "Foo"],
            "Foo"
        ),
        None
    );
    for (name, expected) in [
        ("int", Some("I")),
        ("int[]", Some("[I")),
        ("[Ljava/lang/String;", Some("[Ljava/lang/String;")),
        ("void[]", None),
        ("Int", None),
        ("IntArray", None),
        ("String", None),
    ] {
        assert_eq!(
            resolve("import com.example.Foo;", &[], name).as_deref(),
            expected
        );
    }
}
#[test]
fn conflicting_explicit_imports_remain_ambiguous_while_identical_imports_are_allowed() {
    let source = "import one.Foo;\nimport two.Foo;\nimport one.Foo;";
    for name in ["Foo", "Foo.Inner"] {
        assert_eq!(resolve(source, &["one/Foo", "two/Foo"], name), None);
    }
    let imports = JavaTypeDescriptorResolver::imports_for(source);
    assert!(matches!(
        JavaTypeDescriptorResolver::descriptor_or_diagnostic(
            "Foo",
            &JavaTypeResolutionContext {
                imports: &imports,
                lookup: None
            }
        ),
        TypeDescriptorResult::Ambiguous { .. }
    ));
    assert_eq!(
        resolve(
            "import one.Foo;\nimport one.Foo;",
            &["one/Foo", "two/Foo"],
            "Foo"
        )
        .as_deref(),
        Some("Lone/Foo;")
    );
}
#[test]
fn single_letter_java_class_names_are_not_primitive_descriptors() {
    for (name, expected) in [
        ("B", Some("Lsample/B;")),
        ("B[]", Some("[Lsample/B;")),
        ("int", Some("I")),
        ("I", None),
    ] {
        assert_eq!(
            resolve("import sample.B;", &["sample/B"], name).as_deref(),
            expected
        );
    }
}
#[test]
fn resolves_indexed_implicit_java_lang_type() {
    assert_eq!(
        resolve(
            "@Definition(local = @Local(type = String.class))",
            &["java/lang/String"],
            "String"
        )
        .as_deref(),
        Some("Ljava/lang/String;")
    );
}
#[test]
fn does_not_guess_unindexed_implicit_java_lang_type() {
    assert_eq!(
        resolve(
            "@Definition(local = @Local(type = String.class))",
            &[],
            "String"
        ),
        None
    );
}
#[test]
fn resolves_explicit_import() {
    assert_eq!(
        resolve(
            "import com.example.Foo;\n@Definition(local = @Local(type = Foo.class))",
            &["com/example/Foo"],
            "Foo"
        )
        .as_deref(),
        Some("Lcom/example/Foo;")
    );
}
#[test]
fn resolves_same_package_type() {
    assert_eq!(
        resolve(
            "package com.example;\n@Definition(local = @Local(type = Foo.class))",
            &["com/example/Foo"],
            "Foo"
        )
        .as_deref(),
        Some("Lcom/example/Foo;")
    );
}
#[test]
fn resolves_nested_type_through_imported_outer_type() {
    assert_eq!(
        resolve(
            "import com.example.Outer;\n@Definition(local = @Local(type = Outer.Inner.class))",
            &["com/example/Outer$Inner"],
            "Outer.Inner"
        )
        .as_deref(),
        Some("Lcom/example/Outer$Inner;")
    );
}
#[test]
fn resolves_nested_type_through_same_package() {
    assert_eq!(
        resolve(
            "package com.example;\n@Definition(local = @Local(type = Outer.Inner.class))",
            &["com/example/Outer$Inner"],
            "Outer.Inner"
        )
        .as_deref(),
        Some("Lcom/example/Outer$Inner;")
    );
}
#[test]
fn rejects_ambiguous_wildcard_and_implicit_java_lang_type() {
    assert_eq!(
        resolve(
            "import one.*;\nimport two.*;\n@Definition(local = @Local(type = Foo.class))",
            &["one/Foo", "two/Foo", "java/lang/Foo"],
            "Foo"
        ),
        None
    );
}
#[test]
fn definition_empty_index_has_no_definitions() {
    let index = MixinExtrasDefinitionIndex::default();
    assert!(index.definitions.is_empty());
    assert!(index.definitions_with_id("main").is_empty());
}
#[test]
fn definition_preserves_document_order() {
    let first = MixinExtrasDefinition {
        id: Some("a".into()),
        ..Default::default()
    };
    let second = MixinExtrasDefinition {
        id: Some("b".into()),
        raw_method_references: vec!["Lcom/example/Foo;bar()V".into()],
        ..Default::default()
    };
    let third = MixinExtrasDefinition {
        id: Some("c".into()),
        class_literal_type_names: vec!["java.lang.String".into()],
        local_specs: vec![LocalSugarSpec {
            ordinal: Some(0),
            ..Default::default()
        }],
        ..Default::default()
    };
    let values = vec![first, second, third];
    assert_eq!(
        MixinExtrasDefinitionIndex {
            definitions: values.clone()
        }
        .definitions,
        values
    );
}
#[test]
fn definitions_with_id_returns_all_matches_in_document_order() {
    let first = MixinExtrasDefinition {
        id: Some("main".into()),
        raw_field_references: vec!["Lcom/example/Foo;field:I".into()],
        ..Default::default()
    };
    let other = MixinExtrasDefinition {
        id: Some("other".into()),
        ..Default::default()
    };
    let second = MixinExtrasDefinition {
        id: Some("main".into()),
        raw_method_references: vec!["Lcom/example/Foo;run()V".into()],
        ..Default::default()
    };
    let index = MixinExtrasDefinitionIndex {
        definitions: vec![first.clone(), other.clone(), second.clone()],
    };
    assert_eq!(index.definitions_with_id("main"), vec![&first, &second]);
    assert_eq!(index.definitions_with_id("other"), vec![&other]);
    assert!(index.definitions_with_id("missing").is_empty());
}
fn expr(id: &str, values: &[&str]) -> MixinExtrasExpression {
    MixinExtrasExpression {
        values_utf16: None,
        id: id.into(),
        values: values.iter().map(|s| (*s).into()).collect(),
    }
}
#[test]
fn expression_empty_index_has_no_expressions() {
    let index = MixinExtrasExpressionIndex::default();
    assert!(index.expressions.is_empty());
    assert!(index.values_for_id("main").is_empty());
    assert!(index.values_for_id("").is_empty());
}
#[test]
fn expression_preserves_document_order() {
    let values = vec![
        expr("a", &["(this.one)"]),
        expr("", &["(this.two)"]),
        expr("c", &["(this.three)", "(this.four)"]),
    ];
    assert_eq!(
        MixinExtrasExpressionIndex {
            expressions: values.clone()
        }
        .expressions,
        values
    );
}
#[test]
fn values_for_id_returns_all_matches_flat_mapped_in_document_order() {
    let index = MixinExtrasExpressionIndex {
        expressions: vec![
            expr("main", &["(this.a)", "(this.b)"]),
            expr("other", &["(this.other)"]),
            expr("main", &["(this.c)"]),
        ],
    };
    assert_eq!(
        index.values_for_id("main"),
        vec!["(this.a)", "(this.b)", "(this.c)"]
    );
    assert_eq!(index.values_for_id("other"), vec!["(this.other)"]);
    assert!(index.values_for_id("missing").is_empty());
}
#[test]
fn values_for_id_is_case_sensitive() {
    let index = MixinExtrasExpressionIndex {
        expressions: vec![
            expr("main", &["(this.lower)"]),
            expr("Main", &["(this.upper)"]),
        ],
    };
    assert_eq!(index.values_for_id("main"), vec!["(this.lower)"]);
    assert_eq!(index.values_for_id("Main"), vec!["(this.upper)"]);
    assert!(index.values_for_id("MAIN").is_empty());
}
#[test]
fn values_for_id_matches_empty_id_exactly() {
    let index = MixinExtrasExpressionIndex {
        expressions: vec![expr("", &["(this.empty)"]), expr("main", &["(this.main)"])],
    };
    assert_eq!(index.values_for_id(""), vec!["(this.empty)"]);
    assert_eq!(index.values_for_id("main"), vec!["(this.main)"]);
}
#[test]
fn integration_with_expressions_annotation_parser_preserves_same_id_order() {
    let body="value = {\n    @Expression(id = \"main\", value = \"(this.first)\"),\n    @Expression(id = \"other\", value = \"(this.other)\"),\n    @Expression(id = \"main\", value = { \"(this.second)\", \"(this.third)\" })\n}";
    let index = ExpressionsAnnotationParser::parse(body, None);
    assert_eq!(
        index.values_for_id("main"),
        vec!["(this.first)", "(this.second)", "(this.third)"]
    );
    assert_eq!(index.values_for_id("other"), vec!["(this.other)"]);
}
fn annotation_context(annotation: &str, arguments: &str, needle: &str) -> AnnotationContext {
    let source=format!("@Mixin(com.example.target.SimpleTarget.class)\nclass ExampleMixin {{\n    @{annotation}({arguments})\n    private void handler() {{}}\n}}");
    let offset = source.find(needle).unwrap() + needle.len();
    AnnotationContextExtractor::extract_at_offset(&source, offset).unwrap()
}
#[test]
fn extracts_modify_expression_value_method_slot() {
    let c = annotation_context(
        "ModifyExpressionValue",
        "method = \"dra\", at = @At(\"HEAD\")",
        "dra",
    );
    assert_eq!(c.annotation, MixinAnnotation::ModifyExpressionValue);
    assert_eq!(c.slot, AnnotationSlot::Method);
}
#[test]
fn extracts_modify_return_value_method_slot() {
    assert_eq!(
        annotation_context(
            "ModifyReturnValue",
            "method = \"draw\", at = @At(\"RETURN\")",
            "draw"
        )
        .annotation,
        MixinAnnotation::ModifyReturnValue
    );
}
#[test]
fn extracts_wrap_operation_method_slot() {
    assert_eq!(
        annotation_context(
            "WrapOperation",
            "method = \"draw\", at = @At(value = \"INVOKE\", target = \"\")",
            "draw"
        )
        .annotation,
        MixinAnnotation::WrapOperation
    );
}
#[test]
fn extracts_at_target_context_with_parent_mixin_extras_method() {
    let c=annotation_context("WrapOperation","method = \"draw(Ljava/lang/String;FF)V\", at = @At(value = \"INVOKE\", target = \"length\")","length");
    assert_eq!(c.annotation, MixinAnnotation::At);
    assert_eq!(c.slot, AnnotationSlot::Target);
    assert_eq!(
        c.inject_method_name.as_deref(),
        Some("draw(Ljava/lang/String;FF)V")
    );
    assert_eq!(c.at_value.as_deref(), Some("INVOKE"));
}
#[test]
fn extracts_wrap_with_condition_method_slot() {
    assert_eq!(
        annotation_context(
            "WrapWithCondition",
            "method = \"draw\", at = @At(\"HEAD\")",
            "draw"
        )
        .annotation,
        MixinAnnotation::WrapWithCondition
    );
}
#[test]
fn extracts_wrap_method_method_slot() {
    assert_eq!(
        annotation_context("WrapMethod", "method = \"draw\"", "draw").annotation,
        MixinAnnotation::WrapMethod
    );
}
