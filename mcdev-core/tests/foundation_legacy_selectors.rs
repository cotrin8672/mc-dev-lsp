//! Exact legacy selector cases; each test retains the original case name, all inputs and assertions.
use mcdev_core::descriptor::*;
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesInternalOwnerFieldWithDescriptor"]
fn field_parses_internal_owner_field_with_descriptor() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesInternalOwnerFieldWithDescriptor";
    let parsed = parse_field_selector("Lcom/example/Foo;count:I")
        .unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(
        Pattern::Exact("com/example/Foo".into()),
        parsed.owner,
        "{legacy_id}"
    );
    assert_eq!(Pattern::Exact("count".into()), parsed.name, "{legacy_id}");
    assert_eq!(
        Pattern::Exact(JvmType::IntType),
        parsed.descriptor,
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesLastDotQualifiedOwnerWithSlashNormalization"]
fn field_parses_last_dot_qualified_owner_with_slash_normalization() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesLastDotQualifiedOwnerWithSlashNormalization";
    let parsed = parse_field_selector("com.example.Foo.value:Ljava/lang/String;")
        .unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(
        Pattern::Exact("com/example/Foo".into()),
        parsed.owner,
        "{legacy_id}"
    );
    assert_eq!(Pattern::Exact("value".into()), parsed.name, "{legacy_id}");
    assert_eq!(
        Pattern::Exact(JvmType::ObjectType("java/lang/String".into())),
        parsed.descriptor,
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesSlashQualifiedOwnerWithoutNormalizingSlashes"]
fn field_parses_slash_qualified_owner_without_normalizing_slashes() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesSlashQualifiedOwnerWithoutNormalizingSlashes";
    let parsed = parse_field_selector("com/example/Foo.value:I")
        .unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(
        Pattern::Exact("com/example/Foo".into()),
        parsed.owner,
        "{legacy_id}"
    );
    assert_eq!(Pattern::Exact("value".into()), parsed.name, "{legacy_id}");
    assert_eq!(
        Pattern::Exact(JvmType::IntType),
        parsed.descriptor,
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesOmittedOwnerAndDescriptor"]
fn field_parses_omitted_owner_and_descriptor() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesOmittedOwnerAndDescriptor";
    let parsed = parse_field_selector("count").unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(Pattern::Any, parsed.owner, "{legacy_id}");
    assert_eq!(Pattern::Exact("count".into()), parsed.name, "{legacy_id}");
    assert_eq!(Pattern::Any, parsed.descriptor, "{legacy_id}");
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesBareFieldNameStartingWithCapitalL"]
fn field_parses_bare_field_name_starting_with_capital_l() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesBareFieldNameStartingWithCapitalL";
    let parsed = parse_field_selector("Length").unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(Pattern::Any, parsed.owner, "{legacy_id}");
    assert_eq!(Pattern::Exact("Length".into()), parsed.name, "{legacy_id}");
    assert_eq!(Pattern::Any, parsed.descriptor, "{legacy_id}");
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesBareFieldNameStartingWithCapitalLWithDescriptor"]
fn field_parses_bare_field_name_starting_with_capital_l_with_descriptor() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesBareFieldNameStartingWithCapitalLWithDescriptor";
    let parsed = parse_field_selector("Length:I").unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(Pattern::Any, parsed.owner, "{legacy_id}");
    assert_eq!(Pattern::Exact("Length".into()), parsed.name, "{legacy_id}");
    assert_eq!(
        Pattern::Exact(JvmType::IntType),
        parsed.descriptor,
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesQualifiedFieldNameStartingWithCapitalL"]
fn field_parses_qualified_field_name_starting_with_capital_l() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesQualifiedFieldNameStartingWithCapitalL";
    let parsed =
        parse_field_selector("Logger.value:I").unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(Pattern::Exact("Logger".into()), parsed.owner, "{legacy_id}");
    assert_eq!(Pattern::Exact("value".into()), parsed.name, "{legacy_id}");
    assert_eq!(
        Pattern::Exact(JvmType::IntType),
        parsed.descriptor,
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesOwnerOnlySelector"]
fn field_parses_owner_only_selector() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesOwnerOnlySelector";
    let parsed =
        parse_field_selector("Lcom/example/Foo;").unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(
        Pattern::Exact("com/example/Foo".into()),
        parsed.owner,
        "{legacy_id}"
    );
    assert_eq!(Pattern::Any, parsed.name, "{legacy_id}");
    assert_eq!(Pattern::Any, parsed.descriptor, "{legacy_id}");
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesExactStarNameAsAnyNamePattern"]
fn field_parses_exact_star_name_as_any_name_pattern() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesExactStarNameAsAnyNamePattern";
    let parsed =
        parse_field_selector("Lcom/example/Foo;*").unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(
        Pattern::Exact("com/example/Foo".into()),
        parsed.owner,
        "{legacy_id}"
    );
    assert_eq!(Pattern::Any, parsed.name, "{legacy_id}");
    assert_eq!(Pattern::Any, parsed.descriptor, "{legacy_id}");
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesTerminalNameStarAsExactPrefixWithAnyDescriptor"]
fn field_parses_terminal_name_star_as_exact_prefix_with_any_descriptor() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesTerminalNameStarAsExactPrefixWithAnyDescriptor";
    let parsed = parse_field_selector("count*").unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(Pattern::Any, parsed.owner, "{legacy_id}");
    assert_eq!(Pattern::Exact("count".into()), parsed.name, "{legacy_id}");
    assert_eq!(Pattern::Any, parsed.descriptor, "{legacy_id}");
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesStarNameWithExactDescriptor"]
fn field_parses_star_name_with_exact_descriptor() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesStarNameWithExactDescriptor";
    let parsed = parse_field_selector("*:I").unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(Pattern::Any, parsed.owner, "{legacy_id}");
    assert_eq!(Pattern::Any, parsed.name, "{legacy_id}");
    assert_eq!(
        Pattern::Exact(JvmType::IntType),
        parsed.descriptor,
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesGlobalAnySelector"]
fn field_parses_global_any_selector() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesGlobalAnySelector";
    let parsed = parse_field_selector("*").unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(Pattern::Any, parsed.owner, "{legacy_id}");
    assert_eq!(Pattern::Any, parsed.name, "{legacy_id}");
    assert_eq!(Pattern::Any, parsed.descriptor, "{legacy_id}");
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesNestedDollarOwnerSegments"]
fn field_parses_nested_dollar_owner_segments() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesNestedDollarOwnerSegments";
    let parsed = parse_field_selector("Lcom/example/Outer$Inner$1;value:I")
        .unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(
        Pattern::Exact("com/example/Outer$Inner$1".into()),
        parsed.owner,
        "{legacy_id}"
    );
    assert_eq!(Pattern::Exact("value".into()), parsed.name, "{legacy_id}");
    assert_eq!(
        Pattern::Exact(JvmType::IntType),
        parsed.descriptor,
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesFieldDescriptorWithArrayTypes"]
fn field_parses_field_descriptor_with_array_types() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesFieldDescriptorWithArrayTypes";
    let parsed = parse_field_selector("Lcom/example/Foo;data:[I")
        .unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(
        Pattern::Exact(JvmType::ArrayType(Box::new(JvmType::IntType))),
        parsed.descriptor,
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesNestedArrayFieldDescriptor"]
fn field_parses_nested_array_field_descriptor() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::parsesNestedArrayFieldDescriptor";
    let parsed = parse_field_selector("Lcom/example/Foo;matrix:[[Ljava/lang/String;")
        .unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(
        Pattern::Exact(JvmType::ArrayType(Box::new(JvmType::ArrayType(Box::new(
            JvmType::ObjectType("java/lang/String".into())
        ))))),
        parsed.descriptor,
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsFieldNameStartingWithDigit"]
fn field_rejects_field_name_starting_with_digit() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsFieldNameStartingWithDigit";
    assert!(parse_field_selector("1count").is_err(), "{legacy_id}");
    assert!(parse_field_selector("1count:I").is_err(), "{legacy_id}");
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsMixedOwnerSeparators"]
fn field_rejects_mixed_owner_separators() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsMixedOwnerSeparators";
    assert!(
        parse_field_selector("com.example/Foo.value:I").is_err(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsEmptyInput"]
fn field_rejects_empty_input() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsEmptyInput";
    assert!(parse_field_selector("").is_err(), "{legacy_id}");
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::normalizesAsciiSpacesBeforeParsing"]
fn field_normalizes_ascii_spaces_before_parsing() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::normalizesAsciiSpacesBeforeParsing";
    let input: String = "Lcom/example/Foo;count:I".into();
    assert_eq!(
        parse_field_selector(&input).unwrap(),
        parse_field_selector(&input.replace(' ', "")).unwrap(),
        "{legacy_id}"
    );
    let input: String = " com.example.Foo.value:Ljava/lang/String; ".into();
    assert_eq!(
        parse_field_selector(&input).unwrap(),
        parse_field_selector(&input.replace(' ', "")).unwrap(),
        "{legacy_id}"
    );
    let input: String = "com . example . Foo . value : Ljava/lang/String;".into();
    assert_eq!(
        parse_field_selector(&input).unwrap(),
        parse_field_selector(&input.replace(' ', "")).unwrap(),
        "{legacy_id}"
    );
    let input: String = " count ".into();
    assert_eq!(
        parse_field_selector(&input).unwrap(),
        parse_field_selector(&input.replace(' ', "")).unwrap(),
        "{legacy_id}"
    );
    let input: String = " * ".into();
    assert_eq!(
        parse_field_selector(&input).unwrap(),
        parse_field_selector(&input.replace(' ', "")).unwrap(),
        "{legacy_id}"
    );
    let input: String = " count* ".into();
    assert_eq!(
        parse_field_selector(&input).unwrap(),
        parse_field_selector(&input.replace(' ', "")).unwrap(),
        "{legacy_id}"
    );
    let input: String = " * : I ".into();
    assert_eq!(
        parse_field_selector(&input).unwrap(),
        parse_field_selector(&input.replace(' ', "")).unwrap(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsWhitespaceOnlyInputAfterAsciiSpaceNormalization"]
fn field_rejects_whitespace_only_input_after_ascii_space_normalization() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsWhitespaceOnlyInputAfterAsciiSpaceNormalization";
    assert!(parse_field_selector("   ").is_err(), "{legacy_id}");
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsNonAsciiWhitespaceWithoutNormalizing"]
fn field_rejects_non_ascii_whitespace_without_normalizing() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsNonAsciiWhitespaceWithoutNormalizing";
    assert!(parse_field_selector("\tcount").is_err(), "{legacy_id}");
    assert!(parse_field_selector("count\n").is_err(), "{legacy_id}");
    assert!(parse_field_selector("count ").is_err(), "{legacy_id}");
    assert!(parse_field_selector("\t*").is_err(), "{legacy_id}");
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsMethodParenthesisSyntax"]
fn field_rejects_method_parenthesis_syntax() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsMethodParenthesisSyntax";
    assert!(
        parse_field_selector("Lcom/example/Foo;run()V").is_err(),
        "{legacy_id}"
    );
    assert!(parse_field_selector("run()V").is_err(), "{legacy_id}");
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsInitAndClinitFieldNames"]
fn field_rejects_init_and_clinit_field_names() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsInitAndClinitFieldNames";
    assert!(
        parse_field_selector("Lcom/example/Foo;<init>:I").is_err(),
        "{legacy_id}"
    );
    assert!(
        parse_field_selector("Lcom/example/Foo;<clinit>:I").is_err(),
        "{legacy_id}"
    );
    assert!(parse_field_selector("<init>:I").is_err(), "{legacy_id}");
    assert!(parse_field_selector("<clinit>:I").is_err(), "{legacy_id}");
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsDottedOwnerInsideInternalForm"]
fn field_rejects_dotted_owner_inside_internal_form() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsDottedOwnerInsideInternalForm";
    assert!(
        parse_field_selector("Lcom.example.Foo;count:I").is_err(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsTrailingJunkAfterDescriptor"]
fn field_rejects_trailing_junk_after_descriptor() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsTrailingJunkAfterDescriptor";
    assert!(
        parse_field_selector("Lcom/example/Foo;count:Iextra").is_err(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsInvalidFieldDescriptor"]
fn field_rejects_invalid_field_descriptor() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsInvalidFieldDescriptor";
    assert!(
        parse_field_selector("Lcom/example/Foo;count:Ljava/lang/String").is_err(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsVoidFieldType"]
fn field_rejects_void_field_type() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsVoidFieldType";
    assert!(
        parse_field_selector("Lcom/example/Foo;count:V").is_err(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsDottedObjectDescriptor"]
fn field_rejects_dotted_object_descriptor() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsDottedObjectDescriptor";
    assert!(
        parse_field_selector("Lcom/example/Foo;value:Ljava.lang.String;").is_err(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsFieldDescriptorWithBracketInInternalName"]
fn field_rejects_field_descriptor_with_bracket_in_internal_name() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsFieldDescriptorWithBracketInInternalName";
    assert!(
        parse_field_selector("value:Lfoo[Bar;").is_err(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsMalformedWildcards"]
fn field_rejects_malformed_wildcards() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsMalformedWildcards";
    assert!(parse_field_selector("count**").is_err(), "{legacy_id}");
    assert!(parse_field_selector("*count").is_err(), "{legacy_id}");
    assert!(
        parse_field_selector("Lcom/example/Foo;**").is_err(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsTerminalNameStarWithExplicitDescriptor"]
fn field_rejects_terminal_name_star_with_explicit_descriptor() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsTerminalNameStarWithExplicitDescriptor";
    assert!(parse_field_selector("count*:I").is_err(), "{legacy_id}");
    assert!(
        parse_field_selector("Lcom/example/Foo;count*:I").is_err(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::acceptsExactStarNameWithExplicitDescriptor"]
fn field_accepts_exact_star_name_with_explicit_descriptor() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::acceptsExactStarNameWithExplicitDescriptor";
    let parsed = parse_field_selector("*:I").unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(Pattern::Any, parsed.name, "{legacy_id}");
    assert_eq!(
        Pattern::Exact(JvmType::IntType),
        parsed.descriptor,
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsMissingOwnerSemicolonForm"]
fn field_rejects_missing_owner_semicolon_form() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsMissingOwnerSemicolonForm";
    assert!(
        parse_field_selector("Lcom/example/Foo").is_err(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsEmptyOwnerSegment"]
fn field_rejects_empty_owner_segment() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsEmptyOwnerSegment";
    assert!(
        parse_field_selector("Lcom//example/Foo;count:I").is_err(),
        "{legacy_id}"
    );
    assert!(
        parse_field_selector("com..example.Foo.count:I").is_err(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsWildcardInOwner"]
fn field_rejects_wildcard_in_owner() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsWildcardInOwner";
    assert!(
        parse_field_selector("Lcom/example/*;count:I").is_err(),
        "{legacy_id}"
    );
    assert!(
        parse_field_selector("com.example.*.count:I").is_err(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsMissingFieldName"]
fn field_rejects_missing_field_name() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsMissingFieldName";
    assert!(parse_field_selector(":I").is_err(), "{legacy_id}");
    assert!(
        parse_field_selector("Lcom/example/Foo;:I").is_err(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsMissingFieldDescriptor"]
fn field_rejects_missing_field_descriptor() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/FieldSelectorParserTest.kt::rejectsMissingFieldDescriptor";
    assert!(
        parse_field_selector("Lcom/example/Foo;count:").is_err(),
        "{legacy_id}"
    );
    assert!(parse_field_selector("count:").is_err(), "{legacy_id}");
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesInternalOwnerMethodWithDescriptor"]
fn method_parses_internal_owner_method_with_descriptor() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesInternalOwnerMethodWithDescriptor";
    let parsed = parse_method_selector("Lcom/example/Foo;run()V")
        .unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(
        Pattern::Exact("com/example/Foo".into()),
        parsed.owner,
        "{legacy_id}"
    );
    assert_eq!(Pattern::Exact("run".into()), parsed.name, "{legacy_id}");
    assert_eq!(
        Pattern::Exact(MethodDescriptor {
            parameters: vec![],
            return_type: JvmType::VoidType
        }),
        parsed.descriptor,
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesLastDotQualifiedOwnerWithSlashNormalization"]
fn method_parses_last_dot_qualified_owner_with_slash_normalization() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesLastDotQualifiedOwnerWithSlashNormalization";
    let parsed = parse_method_selector("com.example.Foo.run()V")
        .unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(
        Pattern::Exact("com/example/Foo".into()),
        parsed.owner,
        "{legacy_id}"
    );
    assert_eq!(Pattern::Exact("run".into()), parsed.name, "{legacy_id}");
    assert_eq!(
        Pattern::Exact(MethodDescriptor {
            parameters: vec![],
            return_type: JvmType::VoidType
        }),
        parsed.descriptor,
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesSlashQualifiedOwnerWithoutNormalizingSlashes"]
fn method_parses_slash_qualified_owner_without_normalizing_slashes() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesSlashQualifiedOwnerWithoutNormalizingSlashes";
    let parsed = parse_method_selector("com/example/Foo.run()V")
        .unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(
        Pattern::Exact("com/example/Foo".into()),
        parsed.owner,
        "{legacy_id}"
    );
    assert_eq!(Pattern::Exact("run".into()), parsed.name, "{legacy_id}");
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesOmittedOwnerAndDescriptor"]
fn method_parses_omitted_owner_and_descriptor() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesOmittedOwnerAndDescriptor";
    let parsed = parse_method_selector("run").unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(Pattern::Any, parsed.owner, "{legacy_id}");
    assert_eq!(Pattern::Exact("run".into()), parsed.name, "{legacy_id}");
    assert_eq!(Pattern::Any, parsed.descriptor, "{legacy_id}");
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesBareNameStartingWithCapitalL"]
fn method_parses_bare_name_starting_with_capital_l() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesBareNameStartingWithCapitalL";
    let parsed = parse_method_selector("Load").unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(Pattern::Any, parsed.owner, "{legacy_id}");
    assert_eq!(Pattern::Exact("Load".into()), parsed.name, "{legacy_id}");
    assert_eq!(Pattern::Any, parsed.descriptor, "{legacy_id}");
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesBareNameStartingWithCapitalLWithDescriptor"]
fn method_parses_bare_name_starting_with_capital_l_with_descriptor() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesBareNameStartingWithCapitalLWithDescriptor";
    let parsed = parse_method_selector("Load()V").unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(Pattern::Any, parsed.owner, "{legacy_id}");
    assert_eq!(Pattern::Exact("Load".into()), parsed.name, "{legacy_id}");
    assert_eq!(
        Pattern::Exact(MethodDescriptor {
            parameters: vec![],
            return_type: JvmType::VoidType
        }),
        parsed.descriptor,
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesQualifiedNameStartingWithCapitalL"]
fn method_parses_qualified_name_starting_with_capital_l() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesQualifiedNameStartingWithCapitalL";
    let parsed =
        parse_method_selector("Logger.info()V").unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(Pattern::Exact("Logger".into()), parsed.owner, "{legacy_id}");
    assert_eq!(Pattern::Exact("info".into()), parsed.name, "{legacy_id}");
    assert_eq!(
        Pattern::Exact(MethodDescriptor {
            parameters: vec![],
            return_type: JvmType::VoidType
        }),
        parsed.descriptor,
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesOwnerOnlySelector"]
fn method_parses_owner_only_selector() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesOwnerOnlySelector";
    let parsed =
        parse_method_selector("Lcom/example/Foo;").unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(
        Pattern::Exact("com/example/Foo".into()),
        parsed.owner,
        "{legacy_id}"
    );
    assert_eq!(Pattern::Any, parsed.name, "{legacy_id}");
    assert_eq!(Pattern::Any, parsed.descriptor, "{legacy_id}");
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesExactStarNameAsAnyNamePattern"]
fn method_parses_exact_star_name_as_any_name_pattern() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesExactStarNameAsAnyNamePattern";
    let parsed =
        parse_method_selector("Lcom/example/Foo;*").unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(
        Pattern::Exact("com/example/Foo".into()),
        parsed.owner,
        "{legacy_id}"
    );
    assert_eq!(Pattern::Any, parsed.name, "{legacy_id}");
    assert_eq!(Pattern::Any, parsed.descriptor, "{legacy_id}");
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesTerminalNameStarAsExactNameWithAnyDescriptor"]
fn method_parses_terminal_name_star_as_exact_name_with_any_descriptor() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesTerminalNameStarAsExactNameWithAnyDescriptor";
    let parsed = parse_method_selector("run*").unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(Pattern::Any, parsed.owner, "{legacy_id}");
    assert_eq!(Pattern::Exact("run".into()), parsed.name, "{legacy_id}");
    assert_eq!(Pattern::Any, parsed.descriptor, "{legacy_id}");
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesSpecialMethodTerminalStarAsExactNameWithAnyDescriptor"]
fn method_parses_special_method_terminal_star_as_exact_name_with_any_descriptor() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesSpecialMethodTerminalStarAsExactNameWithAnyDescriptor";
    let bareInit = parse_method_selector("<init>*").unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(Pattern::Any, bareInit.owner, "{legacy_id}");
    assert_eq!(
        Pattern::Exact("<init>".into()),
        bareInit.name,
        "{legacy_id}"
    );
    assert_eq!(Pattern::Any, bareInit.descriptor, "{legacy_id}");
    let bareClinit =
        parse_method_selector("<clinit>*").unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(Pattern::Any, bareClinit.owner, "{legacy_id}");
    assert_eq!(
        Pattern::Exact("<clinit>".into()),
        bareClinit.name,
        "{legacy_id}"
    );
    assert_eq!(Pattern::Any, bareClinit.descriptor, "{legacy_id}");
    let qualifiedInit = parse_method_selector("Lcom/example/Foo;<init>*")
        .unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(
        Pattern::Exact("com/example/Foo".into()),
        qualifiedInit.owner,
        "{legacy_id}"
    );
    assert_eq!(
        Pattern::Exact("<init>".into()),
        qualifiedInit.name,
        "{legacy_id}"
    );
    assert_eq!(Pattern::Any, qualifiedInit.descriptor, "{legacy_id}");
    let dotQualifiedClinit = parse_method_selector("com.example.Foo.<clinit>*")
        .unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(
        Pattern::Exact("com/example/Foo".into()),
        dotQualifiedClinit.owner,
        "{legacy_id}"
    );
    assert_eq!(
        Pattern::Exact("<clinit>".into()),
        dotQualifiedClinit.name,
        "{legacy_id}"
    );
    assert_eq!(Pattern::Any, dotQualifiedClinit.descriptor, "{legacy_id}");
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesStarNameWithExactDescriptor"]
fn method_parses_star_name_with_exact_descriptor() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesStarNameWithExactDescriptor";
    let parsed = parse_method_selector("*()V").unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(Pattern::Any, parsed.owner, "{legacy_id}");
    assert_eq!(Pattern::Any, parsed.name, "{legacy_id}");
    assert_eq!(
        Pattern::Exact(MethodDescriptor {
            parameters: vec![],
            return_type: JvmType::VoidType
        }),
        parsed.descriptor,
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsMethodNameStartingWithDigit"]
fn method_rejects_method_name_starting_with_digit() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsMethodNameStartingWithDigit";
    assert!(parse_method_selector("1run").is_err(), "{legacy_id}");
    assert!(parse_method_selector("1run()V").is_err(), "{legacy_id}");
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsMixedOwnerSeparators"]
fn method_rejects_mixed_owner_separators() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsMixedOwnerSeparators";
    assert!(
        parse_method_selector("com.example/Foo.run()V").is_err(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesGlobalAnySelector"]
fn method_parses_global_any_selector() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesGlobalAnySelector";
    let parsed = parse_method_selector("*").unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(Pattern::Any, parsed.owner, "{legacy_id}");
    assert_eq!(Pattern::Any, parsed.name, "{legacy_id}");
    assert_eq!(Pattern::Any, parsed.descriptor, "{legacy_id}");
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesConstructorAndClassInitializer"]
fn method_parses_constructor_and_class_initializer() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesConstructorAndClassInitializer";
    let constructor = parse_method_selector("Lcom/example/Foo;<init>()V")
        .unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(
        Pattern::Exact("<init>".into()),
        constructor.name,
        "{legacy_id}"
    );
    assert_eq!(
        Pattern::Exact(MethodDescriptor {
            parameters: vec![],
            return_type: JvmType::VoidType
        }),
        constructor.descriptor,
        "{legacy_id}"
    );
    let clinit = parse_method_selector("Lcom/example/Foo;<clinit>()V")
        .unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(
        Pattern::Exact("<clinit>".into()),
        clinit.name,
        "{legacy_id}"
    );
    assert_eq!(
        Pattern::Exact(MethodDescriptor {
            parameters: vec![],
            return_type: JvmType::VoidType
        }),
        clinit.descriptor,
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesNestedDollarOwnerSegments"]
fn method_parses_nested_dollar_owner_segments() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesNestedDollarOwnerSegments";
    let parsed = parse_method_selector("Lcom/example/Outer$Inner$1;run(I)V")
        .unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(
        Pattern::Exact("com/example/Outer$Inner$1".into()),
        parsed.owner,
        "{legacy_id}"
    );
    assert_eq!(
        Pattern::Exact(MethodDescriptor {
            parameters: vec![JvmType::IntType],
            return_type: JvmType::VoidType
        }),
        parsed.descriptor,
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesMethodDescriptorWithArrayTypes"]
fn method_parses_method_descriptor_with_array_types() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::parsesMethodDescriptorWithArrayTypes";
    let parsed = parse_method_selector("Lcom/example/Foo;process([Ljava/lang/String;)V")
        .unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(
        Pattern::Exact(MethodDescriptor {
            parameters: vec![JvmType::ArrayType(Box::new(JvmType::ObjectType(
                "java/lang/String".into()
            )))],
            return_type: JvmType::VoidType
        }),
        parsed.descriptor,
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsEmptyInput"]
fn method_rejects_empty_input() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsEmptyInput";
    assert!(parse_method_selector("").is_err(), "{legacy_id}");
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::normalizesAsciiSpacesBeforeParsing"]
fn method_normalizes_ascii_spaces_before_parsing() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::normalizesAsciiSpacesBeforeParsing";
    let input: String = "Lcom/example/Foo;run()V".into();
    assert_eq!(
        parse_method_selector(&input).unwrap(),
        parse_method_selector(&input.replace(' ', "")).unwrap(),
        "{legacy_id}"
    );
    let input: String = " com.example.Foo.run()V ".into();
    assert_eq!(
        parse_method_selector(&input).unwrap(),
        parse_method_selector(&input.replace(' ', "")).unwrap(),
        "{legacy_id}"
    );
    let input: String = "com . example . Foo . run ( ) V".into();
    assert_eq!(
        parse_method_selector(&input).unwrap(),
        parse_method_selector(&input.replace(' ', "")).unwrap(),
        "{legacy_id}"
    );
    let input: String = " run ".into();
    assert_eq!(
        parse_method_selector(&input).unwrap(),
        parse_method_selector(&input.replace(' ', "")).unwrap(),
        "{legacy_id}"
    );
    let input: String = " * ".into();
    assert_eq!(
        parse_method_selector(&input).unwrap(),
        parse_method_selector(&input.replace(' ', "")).unwrap(),
        "{legacy_id}"
    );
    let input: String = " run* ".into();
    assert_eq!(
        parse_method_selector(&input).unwrap(),
        parse_method_selector(&input.replace(' ', "")).unwrap(),
        "{legacy_id}"
    );
    let input: String = " * ( ) V ".into();
    assert_eq!(
        parse_method_selector(&input).unwrap(),
        parse_method_selector(&input.replace(' ', "")).unwrap(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsWhitespaceOnlyInputAfterAsciiSpaceNormalization"]
fn method_rejects_whitespace_only_input_after_ascii_space_normalization() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsWhitespaceOnlyInputAfterAsciiSpaceNormalization";
    assert!(parse_method_selector("   ").is_err(), "{legacy_id}");
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsNonAsciiWhitespaceWithoutNormalizing"]
fn method_rejects_non_ascii_whitespace_without_normalizing() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsNonAsciiWhitespaceWithoutNormalizing";
    assert!(parse_method_selector("\trun").is_err(), "{legacy_id}");
    assert!(parse_method_selector("run\n").is_err(), "{legacy_id}");
    assert!(parse_method_selector("run ").is_err(), "{legacy_id}");
    assert!(parse_method_selector("\t*").is_err(), "{legacy_id}");
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsFieldColonSyntax"]
fn method_rejects_field_colon_syntax() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsFieldColonSyntax";
    assert!(
        parse_method_selector("Lcom/example/Foo;count:I").is_err(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsDottedOwnerInsideInternalForm"]
fn method_rejects_dotted_owner_inside_internal_form() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsDottedOwnerInsideInternalForm";
    assert!(
        parse_method_selector("Lcom.example.Foo;run()V").is_err(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsTrailingJunkAfterDescriptor"]
fn method_rejects_trailing_junk_after_descriptor() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsTrailingJunkAfterDescriptor";
    assert!(
        parse_method_selector("Lcom/example/Foo;run()Vextra").is_err(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsInvalidMethodDescriptor"]
fn method_rejects_invalid_method_descriptor() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsInvalidMethodDescriptor";
    assert!(
        parse_method_selector("Lcom/example/Foo;run(I)").is_err(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsMethodDescriptorWithBracketInInternalName"]
fn method_rejects_method_descriptor_with_bracket_in_internal_name() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsMethodDescriptorWithBracketInInternalName";
    assert!(
        parse_method_selector("run(Lfoo[Bar;)V").is_err(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsConstructorWithNonVoidReturn"]
fn method_rejects_constructor_with_non_void_return() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsConstructorWithNonVoidReturn";
    assert!(
        parse_method_selector("Lcom/example/Foo;<init>(I)I").is_err(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsClassInitializerWithNonEmptyDescriptor"]
fn method_rejects_class_initializer_with_non_empty_descriptor() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsClassInitializerWithNonEmptyDescriptor";
    assert!(
        parse_method_selector("Lcom/example/Foo;<clinit>(I)V").is_err(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsMalformedWildcards"]
fn method_rejects_malformed_wildcards() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsMalformedWildcards";
    assert!(parse_method_selector("run**").is_err(), "{legacy_id}");
    assert!(parse_method_selector("*run").is_err(), "{legacy_id}");
    assert!(
        parse_method_selector("Lcom/example/Foo;**").is_err(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsTerminalNameStarWithExplicitDescriptor"]
fn method_rejects_terminal_name_star_with_explicit_descriptor() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsTerminalNameStarWithExplicitDescriptor";
    assert!(parse_method_selector("run*()V").is_err(), "{legacy_id}");
    assert!(
        parse_method_selector("Lcom/example/Foo;run*()V").is_err(),
        "{legacy_id}"
    );
    assert!(parse_method_selector("<init>*()V").is_err(), "{legacy_id}");
    assert!(
        parse_method_selector("Lcom/example/Foo;<clinit>*()V").is_err(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::acceptsExactStarNameWithExplicitDescriptor"]
fn method_accepts_exact_star_name_with_explicit_descriptor() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::acceptsExactStarNameWithExplicitDescriptor";
    let parsed = parse_method_selector("*()V").unwrap_or_else(|e| panic!("{legacy_id}: {e}"));
    assert_eq!(Pattern::Any, parsed.name, "{legacy_id}");
    assert_eq!(
        Pattern::Exact(MethodDescriptor {
            parameters: vec![],
            return_type: JvmType::VoidType
        }),
        parsed.descriptor,
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsMissingOwnerSemicolonForm"]
fn method_rejects_missing_owner_semicolon_form() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsMissingOwnerSemicolonForm";
    assert!(
        parse_method_selector("Lcom/example/Foo").is_err(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsEmptyOwnerSegment"]
fn method_rejects_empty_owner_segment() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsEmptyOwnerSegment";
    assert!(
        parse_method_selector("Lcom//example/Foo;run()V").is_err(),
        "{legacy_id}"
    );
    assert!(
        parse_method_selector("com..example.Foo.run()V").is_err(),
        "{legacy_id}"
    );
}
#[test]
#[doc = "mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsWildcardInOwner"]
fn method_rejects_wildcard_in_owner() {
    let legacy_id="mcdev-core/src/test/kotlin/io/github/mcdev/core/descriptor/MethodSelectorParserTest.kt::rejectsWildcardInOwner";
    assert!(
        parse_method_selector("Lcom/example/*;run()V").is_err(),
        "{legacy_id}"
    );
    assert!(
        parse_method_selector("com.example.*.run()V").is_err(),
        "{legacy_id}"
    );
}
