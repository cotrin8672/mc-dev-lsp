#[path = "fixtures/mixin_navigation/replay.rs"]
mod replay;
#[test]
fn mixin_definition_service_test_at_target_parser_parses_field_descriptor() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::atTargetParserParsesFieldDescriptor");
}
#[test]
fn mixin_definition_service_test_at_target_parser_parses_method_descriptor() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::atTargetParserParsesMethodDescriptor");
}
#[test]
fn mixin_definition_service_test_definition_cursor_outside_value_returns_empty() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::definitionCursorOutsideValueReturnsEmpty");
}
#[test]
fn mixin_definition_service_test_definition_does_not_match_comment_string() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::definitionDoesNotMatchCommentString");
}
#[test]
fn mixin_definition_service_test_definition_does_not_match_unrelated_handler_method_string() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::definitionDoesNotMatchUnrelatedHandlerMethodString");
}
#[test]
fn mixin_definition_service_test_definition_exact_method_does_not_set_direct_source_document_uri() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::definitionExactMethodDoesNotSetDirectSourceDocumentUri");
}
#[test]
fn mixin_definition_service_test_definition_local_type_ambiguous_wildcard_class_literal_returns_empty(
) {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::definitionLocalTypeAmbiguousWildcardClassLiteralReturnsEmpty");
}
#[test]
fn mixin_definition_service_test_definition_local_type_unknown_class_literal_returns_empty() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::definitionLocalTypeUnknownClassLiteralReturnsEmpty");
}
#[test]
fn mixin_definition_service_test_definition_malformed_selector_returns_empty() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::definitionMalformedSelectorReturnsEmpty");
}
#[test]
fn mixin_definition_service_test_definition_omitted_descriptor_returns_empty() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::definitionOmittedDescriptorReturnsEmpty");
}
#[test]
fn mixin_definition_service_test_definition_wildcard_selector_returns_empty() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::definitionWildcardSelectorReturnsEmpty");
}
#[test]
fn mixin_definition_service_test_expression_definition_id_sets_direct_source_document_uri_when_provided(
) {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::expressionDefinitionIdSetsDirectSourceDocumentUriWhenProvided");
}
#[test]
fn mixin_definition_service_test_invoker_definition_does_not_fallback_to_wrong_overload() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::invokerDefinitionDoesNotFallbackToWrongOverload");
}
#[test]
fn mixin_definition_service_test_invoker_definition_does_not_resolve_when_descriptor_contains_unknown_type(
) {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::invokerDefinitionDoesNotResolveWhenDescriptorContainsUnknownType");
}
#[test]
fn mixin_definition_service_test_isolates_expression_definition_id_by_handler() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::isolatesExpressionDefinitionIdByHandler");
}
#[test]
fn mixin_definition_service_test_preserves_definition_exact_method_navigation() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::preservesDefinitionExactMethodNavigation");
}
#[test]
fn mixin_definition_service_test_preserves_raw_expression_token_offsets_for_octal_escaped_identifier(
) {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::preservesRawExpressionTokenOffsetsForOctalEscapedIdentifier");
}
#[test]
fn mixin_definition_service_test_rejects_ambiguous_injector_method_and_non_method_attribute() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::rejectsAmbiguousInjectorMethodAndNonMethodAttribute");
}
#[test]
fn mixin_definition_service_test_rejects_expression_id_attribute_selection() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::rejectsExpressionIdAttributeSelection");
}
#[test]
fn mixin_definition_service_test_rejects_expression_identifier_after_dot_member_name() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::rejectsExpressionIdentifierAfterDotMemberName");
}
#[test]
fn mixin_definition_service_test_rejects_expression_identifier_in_comment_string_fake() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::rejectsExpressionIdentifierInCommentStringFake");
}
#[test]
fn mixin_definition_service_test_rejects_expression_identifier_with_leading_backslash_after_java_decode(
) {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::rejectsExpressionIdentifierWithLeadingBackslashAfterJavaDecode");
}
#[test]
fn mixin_definition_service_test_rejects_expression_keyword_identifier() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::rejectsExpressionKeywordIdentifier");
}
#[test]
fn mixin_definition_service_test_rejects_missing_expression_definition_id() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::rejectsMissingExpressionDefinitionId");
}
#[test]
fn mixin_definition_service_test_rejects_partial_expression_definition_identifier() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::rejectsPartialExpressionDefinitionIdentifier");
}
#[test]
fn mixin_definition_service_test_resolves_accessor_field() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesAccessorField");
}
#[test]
fn mixin_definition_service_test_resolves_at_target_invoke_member() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesAtTargetInvokeMember");
}
#[test]
fn mixin_definition_service_test_resolves_complete_mixin_target_before_ambiguous_partial_prefix() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesCompleteMixinTargetBeforeAmbiguousPartialPrefix");
}
#[test]
fn mixin_definition_service_test_resolves_definition_exact_field() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesDefinitionExactField");
}
#[test]
fn mixin_definition_service_test_resolves_definition_exact_method() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesDefinitionExactMethod");
}
#[test]
fn mixin_definition_service_test_resolves_definition_local_type_class_literal() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesDefinitionLocalTypeClassLiteral");
}
#[test]
fn mixin_definition_service_test_resolves_definition_local_type_implicit_java_lang_class_literal() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesDefinitionLocalTypeImplicitJavaLangClassLiteral");
}
#[test]
fn mixin_definition_service_test_resolves_definition_local_type_nested_class_through_import() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesDefinitionLocalTypeNestedClassThroughImport");
}
#[test]
fn mixin_definition_service_test_resolves_definition_local_type_through_explicit_import() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesDefinitionLocalTypeThroughExplicitImport");
}
#[test]
fn mixin_definition_service_test_resolves_definition_type_class_literal() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesDefinitionTypeClassLiteral");
}
#[test]
fn mixin_definition_service_test_resolves_each_local_type_inside_definition_array() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesEachLocalTypeInsideDefinitionArray");
}
#[test]
fn mixin_definition_service_test_resolves_expression_identifier_from_nested_definitions() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesExpressionIdentifierFromNestedDefinitions");
}
#[test]
fn mixin_definition_service_test_resolves_expression_identifier_to_unique_definition_id() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesExpressionIdentifierToUniqueDefinitionId");
}
#[test]
fn mixin_definition_service_test_resolves_expression_identifier_with_definitions_after_injector() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesExpressionIdentifierWithDefinitionsAfterInjector");
}
#[test]
fn mixin_definition_service_test_resolves_expression_identifier_with_fqn_annotations() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesExpressionIdentifierWithFqnAnnotations");
}
#[test]
fn mixin_definition_service_test_resolves_expression_member_and_method_reference_definition_ids() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesExpressionMemberAndMethodReferenceDefinitionIds");
}
#[test]
fn mixin_definition_service_test_resolves_expressions_container_identifier_to_definition_id() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesExpressionsContainerIdentifierToDefinitionId");
}
#[test]
fn mixin_definition_service_test_resolves_fully_qualified_definition_exact_method() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesFullyQualifiedDefinitionExactMethod");
}
#[test]
fn mixin_definition_service_test_resolves_injector_method_for_inject_and_mixin_extras() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesInjectorMethodForInjectAndMixinExtras");
}
#[test]
fn mixin_definition_service_test_resolves_invoker_method() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesInvokerMethod");
}
#[test]
fn mixin_definition_service_test_resolves_kotlin_definition_and_local_class_literals_with_original_ranges(
) {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesKotlinDefinitionAndLocalClassLiteralsWithOriginalRanges");
}
#[test]
fn mixin_definition_service_test_resolves_mixin_class_target() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesMixinClassTarget");
}
#[test]
fn mixin_definition_service_test_resolves_mixin_targets_string() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesMixinTargetsString");
}
#[test]
fn mixin_definition_service_test_resolves_nested_definition_field_in_definitions_array() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesNestedDefinitionFieldInDefinitionsArray");
}
#[test]
fn mixin_definition_service_test_resolves_octal_escaped_expression_token_to_plain_definition_id() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesOctalEscapedExpressionTokenToPlainDefinitionId");
}
#[test]
fn mixin_definition_service_test_resolves_plain_expression_token_to_octal_escaped_definition_id() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesPlainExpressionTokenToOctalEscapedDefinitionId");
}
#[test]
fn mixin_definition_service_test_resolves_shadow_field() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesShadowField");
}
#[test]
fn mixin_definition_service_test_resolves_shadow_members_when_the_caret_is_at_the_name_end() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesShadowMembersWhenTheCaretIsAtTheNameEnd");
}
#[test]
fn mixin_definition_service_test_resolves_shadow_method() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesShadowMethod");
}
#[test]
fn mixin_definition_service_test_resolves_simple_target_mixin_class_from_bytecode_index() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::resolvesSimpleTargetMixinClassFromBytecodeIndex");
}
#[test]
fn mixin_definition_service_test_returns_empty_outside_mixin_context() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::returnsEmptyOutsideMixinContext");
}
#[test]
fn mixin_definition_service_test_returns_equivalent_plain_and_escaped_definition_ids() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::returnsEquivalentPlainAndEscapedDefinitionIds");
}
#[test]
fn mixin_definition_service_test_returns_every_definition_of_repeated_identifier() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::returnsEveryDefinitionOfRepeatedIdentifier");
}
#[test]
fn mixin_definition_service_test_semantic_member_definitions_only_resolve_the_exact_name_token() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt::semanticMemberDefinitionsOnlyResolveTheExactNameToken");
}
#[test]
fn mixin_reference_service_test_does_not_match_inject_method_name_by_prefix() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinReferenceServiceTest.kt::doesNotMatchInjectMethodNameByPrefix");
}
#[test]
fn mixin_reference_service_test_finds_access_transformer_field_references() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinReferenceServiceTest.kt::findsAccessTransformerFieldReferences");
}
#[test]
fn mixin_reference_service_test_finds_access_widener_method_references() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinReferenceServiceTest.kt::findsAccessWidenerMethodReferences");
}
#[test]
fn mixin_reference_service_test_finds_accessor_field_references() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinReferenceServiceTest.kt::findsAccessorFieldReferences");
}
#[test]
fn mixin_reference_service_test_finds_invoker_references() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinReferenceServiceTest.kt::findsInvokerReferences");
}
#[test]
fn mixin_reference_service_test_finds_mixin_class_references() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinReferenceServiceTest.kt::findsMixinClassReferences");
}
#[test]
fn mixin_reference_service_test_finds_references_across_multiple_sources() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinReferenceServiceTest.kt::findsReferencesAcrossMultipleSources");
}
#[test]
fn mixin_reference_service_test_finds_shadow_field_references() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinReferenceServiceTest.kt::findsShadowFieldReferences");
}
#[test]
fn mixin_reference_service_test_matches_inject_method_descriptor_exactly() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinReferenceServiceTest.kt::matchesInjectMethodDescriptorExactly");
}
#[test]
fn mixin_reference_service_test_returns_empty_when_no_matches() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinReferenceServiceTest.kt::returnsEmptyWhenNoMatches");
}
