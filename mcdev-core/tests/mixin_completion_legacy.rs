#[path = "fixtures/mixin_completion/replay.rs"]
mod legacy_replay;

#[test]
fn at_completion_service_test_additional_at_values_completes_custom_namespaced_prefix() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::additionalAtValuesCompletesCustomNamespacedPrefix");
}

#[test]
fn at_completion_service_test_additional_at_values_completes_fqn_prefix() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::additionalAtValuesCompletesFqnPrefix");
}

#[test]
fn at_completion_service_test_additional_at_values_deduplicates_case_insensitively() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::additionalAtValuesDeduplicatesCaseInsensitively");
}

#[test]
fn at_completion_service_test_additional_at_values_does_not_reintroduce_disallowed_built_in() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::additionalAtValuesDoesNotReintroduceDisallowedBuiltIn");
}

#[test]
fn at_completion_service_test_additional_at_values_specifier_completion_for_custom_code() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::additionalAtValuesSpecifierCompletionForCustomCode");
}

#[test]
fn at_completion_service_test_at_specifier_completion_filters_by_suffix_prefix() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::atSpecifierCompletionFiltersBySuffixPrefix");
}

#[test]
fn at_completion_service_test_at_specifier_completion_rejects_unknown_code() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::atSpecifierCompletionRejectsUnknownCode");
}

#[test]
fn at_completion_service_test_at_specifier_completion_uses_at_inside_slice() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::atSpecifierCompletionUsesAtInsideSlice");
}

#[test]
fn at_completion_service_test_at_specifier_completion_uses_full_slice_values_with_parent_injector()
{
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::atSpecifierCompletionUsesFullSliceValuesWithParentInjector");
}

#[test]
fn at_completion_service_test_at_specifier_completion_uses_injection_point_specifier_probe() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::atSpecifierCompletionUsesInjectionPointSpecifierProbe");
}

#[test]
fn at_completion_service_test_at_specifier_probe_called_once_outside_slice() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::atSpecifierProbeCalledOnceOutsideSlice");
}

#[test]
fn at_completion_service_test_at_specifier_probe_skipped_for_invalid_injector_code() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::atSpecifierProbeSkippedForInvalidInjectorCode");
}

#[test]
fn at_completion_service_test_at_specifier_probe_skipped_for_ordinary_partials() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::atSpecifierProbeSkippedForOrdinaryPartials");
}

#[test]
fn at_completion_service_test_at_specifier_probe_skipped_for_unknown_code() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::atSpecifierProbeSkippedForUnknownCode");
}

#[test]
fn at_completion_service_test_at_specifier_probe_skipped_inside_slice() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::atSpecifierProbeSkippedInsideSlice");
}

#[test]
fn at_completion_service_test_at_target_deduplicates_field_get_and_put_with_same_selector() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::atTargetDeduplicatesFieldGetAndPutWithSameSelector");
}

#[test]
fn at_completion_service_test_at_target_deduplicates_repeated_identical_invoke_occurrences() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::atTargetDeduplicatesRepeatedIdenticalInvokeOccurrences");
}

#[test]
fn at_completion_service_test_at_target_filters_by_partial() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::atTargetFiltersByPartial");
}

#[test]
fn at_completion_service_test_at_target_item_metadata_unchanged_for_first_duplicate() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::atTargetItemMetadataUnchangedForFirstDuplicate");
}

#[test]
fn at_completion_service_test_at_target_keeps_distinct_constant_values() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::atTargetKeepsDistinctConstantValues");
}

#[test]
fn at_completion_service_test_at_target_keeps_distinct_invoke_overloads() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::atTargetKeepsDistinctInvokeOverloads");
}

#[test]
fn at_completion_service_test_at_target_label_differs_from_insert_text() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::atTargetLabelDiffersFromInsertText");
}

#[test]
fn at_completion_service_test_at_target_prefix_filtering_unchanged_after_deduplication() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::atTargetPrefixFilteringUnchangedAfterDeduplication");
}

#[test]
fn at_completion_service_test_at_value_completion_filters_by_prefix() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::atValueCompletionFiltersByPrefix");
}

#[test]
fn at_completion_service_test_completes_all_at_values() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::completesAllAtValues");
}

#[test]
fn at_completion_service_test_completes_at_field_target() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::completesAtFieldTarget");
}

#[test]
fn at_completion_service_test_completes_at_invoke_target() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::completesAtInvokeTarget");
}

#[test]
fn at_completion_service_test_completes_at_specifiers_for_namespaced_code() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::completesAtSpecifiersForNamespacedCode");
}

#[test]
fn at_completion_service_test_completes_at_specifiers_inside_slice_with_empty_suffix() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::completesAtSpecifiersInsideSliceWithEmptySuffix");
}

#[test]
fn at_completion_service_test_completes_at_value_head() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::completesAtValueHead");
}

#[test]
fn at_completion_service_test_completes_modify_return_value_tail() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::completesModifyReturnValueTail");
}

#[test]
fn at_completion_service_test_format_target_for_constructor() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::formatTargetForConstructor");
}

#[test]
fn at_completion_service_test_format_target_for_new_class_only() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::formatTargetForNewClassOnly");
}

#[test]
fn at_completion_service_test_returns_empty_for_non_at_annotation() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::returnsEmptyForNonAtAnnotation");
}

#[test]
fn at_completion_service_test_wrap_operation_at_value_completion_uses_all_values_inside_slice() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/AtCompletionServiceTest.kt::wrapOperationAtValueCompletionUsesAllValuesInsideSlice");
}

#[test]
fn inject_method_completion_service_test_always_mode_inserts_descriptor_even_for_single_overload() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/InjectMethodCompletionServiceTest.kt::alwaysModeInsertsDescriptorEvenForSingleOverload");
}

#[test]
fn inject_method_completion_service_test_completes_from_multiple_mixin_targets() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/InjectMethodCompletionServiceTest.kt::completesFromMultipleMixinTargets");
}

#[test]
fn inject_method_completion_service_test_completes_inject_method_with_descriptor_for_overloads_in_auto_mode(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/InjectMethodCompletionServiceTest.kt::completesInjectMethodWithDescriptorForOverloadsInAutoMode");
}

#[test]
fn inject_method_completion_service_test_completes_inject_method_without_descriptor_for_single_overload(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/InjectMethodCompletionServiceTest.kt::completesInjectMethodWithoutDescriptorForSingleOverload");
}

#[test]
fn inject_method_completion_service_test_completion_label_shows_readable_signature() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/InjectMethodCompletionServiceTest.kt::completionLabelShowsReadableSignature");
}

#[test]
fn inject_method_completion_service_test_filters_by_method_prefix() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/InjectMethodCompletionServiceTest.kt::filtersByMethodPrefix");
}

#[test]
fn inject_method_completion_service_test_metadata_contains_descriptor() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/InjectMethodCompletionServiceTest.kt::metadataContainsDescriptor");
}

#[test]
fn inject_method_completion_service_test_never_mode_inserts_name_only_even_for_overloads() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/InjectMethodCompletionServiceTest.kt::neverModeInsertsNameOnlyEvenForOverloads");
}

#[test]
fn inject_method_completion_service_test_returns_empty_for_non_method_slot() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/InjectMethodCompletionServiceTest.kt::returnsEmptyForNonMethodSlot");
}

#[test]
fn inject_method_completion_service_test_returns_empty_without_mixin_targets() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/InjectMethodCompletionServiceTest.kt::returnsEmptyWithoutMixinTargets");
}

#[test]
fn mixin_attribute_completion_service_test_at_attribute_builds_nested_at_snippet() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::atAttributeBuildsNestedAtSnippet");
}

#[test]
fn mixin_attribute_completion_service_test_constant_suggests_exact_attribute_names_with_prefix_and_existing_filtering(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::constantSuggestsExactAttributeNamesWithPrefixAndExistingFiltering");
}

#[test]
fn mixin_attribute_completion_service_test_definition_attribute_prefix_filters_candidates() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::definitionAttributePrefixFiltersCandidates");
}

#[test]
fn mixin_attribute_completion_service_test_definition_excludes_existing_attributes() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::definitionExcludesExistingAttributes");
}

#[test]
fn mixin_attribute_completion_service_test_definition_suggests_only_valid_mixin_extras_attributes()
{
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::definitionSuggestsOnlyValidMixinExtrasAttributes");
}

#[test]
fn mixin_attribute_completion_service_test_definition_uses_expected_snippet_kinds() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::definitionUsesExpectedSnippetKinds");
}

#[test]
fn mixin_attribute_completion_service_test_definitions_and_expressions_use_nested_annotation_snippets(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::definitionsAndExpressionsUseNestedAnnotationSnippets");
}

#[test]
fn mixin_attribute_completion_service_test_definitions_suggests_only_value_attribute() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::definitionsSuggestsOnlyValueAttribute");
}

#[test]
fn mixin_attribute_completion_service_test_excludes_only_attributes_already_present_on_the_same_annotation(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::excludesOnlyAttributesAlreadyPresentOnTheSameAnnotation");
}

#[test]
fn mixin_attribute_completion_service_test_expression_attribute_prefix_filters_candidates() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::expressionAttributePrefixFiltersCandidates");
}

#[test]
fn mixin_attribute_completion_service_test_expression_excludes_existing_attributes() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::expressionExcludesExistingAttributes");
}

#[test]
fn mixin_attribute_completion_service_test_expression_suggests_only_value_and_id() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::expressionSuggestsOnlyValueAndId");
}

#[test]
fn mixin_attribute_completion_service_test_expressions_suggests_only_value_attribute() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::expressionsSuggestsOnlyValueAttribute");
}

#[test]
fn mixin_attribute_completion_service_test_local_attribute_prefix_filters_candidates() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::localAttributePrefixFiltersCandidates");
}

#[test]
fn mixin_attribute_completion_service_test_local_excludes_existing_attributes() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::localExcludesExistingAttributes");
}

#[test]
fn mixin_attribute_completion_service_test_local_suggests_only_valid_mixin_extras_attributes() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::localSuggestsOnlyValidMixinExtrasAttributes");
}

#[test]
fn mixin_attribute_completion_service_test_local_uses_expected_snippet_kinds() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::localUsesExpectedSnippetKinds");
}

#[test]
fn mixin_attribute_completion_service_test_method_attribute_inserts_quoted_snippet_with_cursor_inside(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::methodAttributeInsertsQuotedSnippetWithCursorInside");
}

#[test]
fn mixin_attribute_completion_service_test_mixin_extras_injector_does_not_suggest_constraints_attribute(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::mixinExtrasInjectorDoesNotSuggestConstraintsAttribute");
}

#[test]
fn mixin_attribute_completion_service_test_share_attribute_prefix_filters_candidates() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::shareAttributePrefixFiltersCandidates");
}

#[test]
fn mixin_attribute_completion_service_test_share_excludes_existing_attributes() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::shareExcludesExistingAttributes");
}

#[test]
fn mixin_attribute_completion_service_test_share_suggests_only_value_and_namespace() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::shareSuggestsOnlyValueAndNamespace");
}

#[test]
fn mixin_attribute_completion_service_test_slice_suggests_exact_attribute_names_with_prefix_and_existing_filtering(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::sliceSuggestsExactAttributeNamesWithPrefixAndExistingFiltering");
}

#[test]
fn mixin_attribute_completion_service_test_vanilla_inject_still_suggests_constraints_attribute() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::vanillaInjectStillSuggestsConstraintsAttribute");
}

#[test]
fn mixin_attribute_completion_service_test_wrap_method_does_not_suggest_constraints_attribute() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::wrapMethodDoesNotSuggestConstraintsAttribute");
}

#[test]
fn mixin_attribute_completion_service_test_wrap_operation_does_not_suggest_constraints_and_keeps_at_constant_exclusivity(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::wrapOperationDoesNotSuggestConstraintsAndKeepsAtConstantExclusivity");
}

#[test]
fn mixin_attribute_completion_service_test_wrap_with_condition_order_depends_on_resolved_official_fqn(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinAttributeCompletionServiceTest.kt::wrapWithConditionOrderDependsOnResolvedOfficialFqn");
}

#[test]
fn mixin_target_completion_service_test_class_completion_metadata_contains_owner() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinTargetCompletionServiceTest.kt::classCompletionMetadataContainsOwner");
}

#[test]
fn mixin_target_completion_service_test_class_completion_preserves_label_and_detail() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinTargetCompletionServiceTest.kt::classCompletionPreservesLabelAndDetail");
}

#[test]
fn mixin_target_completion_service_test_completes_mixin_class_with_fqn_mode() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinTargetCompletionServiceTest.kt::completesMixinClassWithFqnMode");
}

#[test]
fn mixin_target_completion_service_test_completes_mixin_class_with_import_mode() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinTargetCompletionServiceTest.kt::completesMixinClassWithImportMode");
}

#[test]
fn mixin_target_completion_service_test_completes_targets_string_with_fqn() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinTargetCompletionServiceTest.kt::completesTargetsStringWithFqn");
}

#[test]
fn mixin_target_completion_service_test_filters_classes_by_prefix() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinTargetCompletionServiceTest.kt::filtersClassesByPrefix");
}

#[test]
fn mixin_target_completion_service_test_narrower_prefix_finds_class_beyond_broad_completion_limit()
{
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinTargetCompletionServiceTest.kt::narrowerPrefixFindsClassBeyondBroadCompletionLimit");
}

#[test]
fn mixin_target_completion_service_test_returns_empty_for_non_mixin_annotation() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinTargetCompletionServiceTest.kt::returnsEmptyForNonMixinAnnotation");
}

#[test]
fn mixin_target_completion_service_test_returns_empty_for_wrong_slot() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinTargetCompletionServiceTest.kt::returnsEmptyForWrongSlot");
}

#[test]
fn mixin_target_completion_service_test_sort_key_is_stable() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinTargetCompletionServiceTest.kt::sortKeyIsStable");
}

#[test]
fn mixin_target_completion_service_test_targets_completion_label_is_simple_name() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinTargetCompletionServiceTest.kt::targetsCompletionLabelIsSimpleName");
}

#[test]
fn mixin_declaration_snippet_service_test_constructor_invoker_accepts_target_simple_name_alias() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDeclarationSnippetServiceTest.kt::constructorInvokerAcceptsTargetSimpleNameAlias");
}

#[test]
fn mixin_declaration_snippet_service_test_constructor_invoker_is_static_factory_returning_target_type(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDeclarationSnippetServiceTest.kt::constructorInvokerIsStaticFactoryReturningTargetType");
}

#[test]
fn mixin_declaration_snippet_service_test_existing_member_after_annotation_does_not_produce_duplicate_declaration(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDeclarationSnippetServiceTest.kt::existingMemberAfterAnnotationDoesNotProduceDuplicateDeclaration");
}

#[test]
fn mixin_declaration_snippet_service_test_explicit_accessor_offers_getter_and_setter_with_whole_empty_name_placeholder(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDeclarationSnippetServiceTest.kt::explicitAccessorOffersGetterAndSetterWithWholeEmptyNamePlaceholder");
}

#[test]
fn mixin_declaration_snippet_service_test_instance_accessor_makes_concrete_mixin_abstract() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDeclarationSnippetServiceTest.kt::instanceAccessorMakesConcreteMixinAbstract");
}

#[test]
fn mixin_declaration_snippet_service_test_instance_declarations_in_interface_need_no_class_edit() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDeclarationSnippetServiceTest.kt::instanceDeclarationsInInterfaceNeedNoClassEdit");
}

#[test]
fn mixin_declaration_snippet_service_test_invoker_accepts_jvm_descriptor_selector() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDeclarationSnippetServiceTest.kt::invokerAcceptsJvmDescriptorSelector");
}

#[test]
fn mixin_declaration_snippet_service_test_mismatched_multi_target_field_does_not_offer_a_one_target_declaration(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDeclarationSnippetServiceTest.kt::mismatchedMultiTargetFieldDoesNotOfferAOneTargetDeclaration");
}

#[test]
fn mixin_declaration_snippet_service_test_overloaded_invoker_returns_one_declaration_per_resolved_descriptor(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDeclarationSnippetServiceTest.kt::overloadedInvokerReturnsOneDeclarationPerResolvedDescriptor");
}

#[test]
fn mixin_declaration_snippet_service_test_overwrite_non_void_uses_throwing_body_stub_with_editable_stop_before_throw(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDeclarationSnippetServiceTest.kt::overwriteNonVoidUsesThrowingBodyStubWithEditableStopBeforeThrow");
}

#[test]
fn mixin_declaration_snippet_service_test_overwrite_requires_exactly_one_mixin_target() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDeclarationSnippetServiceTest.kt::overwriteRequiresExactlyOneMixinTarget");
}

#[test]
fn mixin_declaration_snippet_service_test_overwrite_without_target_offers_explicit_signature_choices(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDeclarationSnippetServiceTest.kt::overwriteWithoutTargetOffersExplicitSignatureChoices");
}

#[test]
fn mixin_declaration_snippet_service_test_static_accessor_uses_static_body_instead_of_invalid_abstract_method(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDeclarationSnippetServiceTest.kt::staticAccessorUsesStaticBodyInsteadOfInvalidAbstractMethod");
}

#[test]
fn mixin_declaration_snippet_service_test_static_invoker_uses_static_body() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDeclarationSnippetServiceTest.kt::staticInvokerUsesStaticBody");
}

#[test]
fn mixin_declaration_snippet_service_test_type_like_text_in_comment_cannot_receive_abstract_class_edit(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDeclarationSnippetServiceTest.kt::typeLikeTextInCommentCannotReceiveAbstractClassEdit");
}

#[test]
fn mixin_declaration_snippet_service_test_unresolved_targets_do_not_fabricate_declarations() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDeclarationSnippetServiceTest.kt::unresolvedTargetsDoNotFabricateDeclarations");
}

#[test]
fn mixin_import_edit_builder_test_generates_import_edit_after_existing_imports() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinEdgeCasesTest.kt::generatesImportEditAfterExistingImports");
}

#[test]
fn mixin_import_edit_builder_test_generates_import_edit_after_package_declaration() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinEdgeCasesTest.kt::generatesImportEditAfterPackageDeclaration");
}

#[test]
fn mixin_import_edit_builder_test_ignores_import_mentioned_in_comment_when_choosing_type_reference()
{
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinEdgeCasesTest.kt::ignoresImportMentionedInCommentWhenChoosingTypeReference");
}

#[test]
fn mixin_import_edit_builder_test_import_edit_pairs_with_import_mode_completion_insert_text() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinEdgeCasesTest.kt::importEditPairsWithImportModeCompletionInsertText");
}

#[test]
fn mixin_import_edit_builder_test_skips_import_for_same_package_target() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinEdgeCasesTest.kt::skipsImportForSamePackageTarget");
}

#[test]
fn mixin_import_edit_builder_test_skips_import_when_already_present() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinEdgeCasesTest.kt::skipsImportWhenAlreadyPresent");
}

#[test]
fn mixin_config_editor_test_add_entry_does_not_duplicate() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinConfigEditorTest.kt::addEntryDoesNotDuplicate");
}

#[test]
fn mixin_config_editor_test_add_entry_lossless_aborts_on_incomplete_unicode_escape() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinConfigEditorTest.kt::addEntryLosslessAbortsOnIncompleteUnicodeEscape");
}

#[test]
fn mixin_config_editor_test_add_entry_lossless_aborts_on_invalid_unicode_escape() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinConfigEditorTest.kt::addEntryLosslessAbortsOnInvalidUnicodeEscape");
}

#[test]
fn mixin_config_editor_test_add_entry_lossless_aborts_when_closing_bracket_missing() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinConfigEditorTest.kt::addEntryLosslessAbortsWhenClosingBracketMissing");
}

#[test]
fn mixin_config_editor_test_add_entry_lossless_appends_to_client_array() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinConfigEditorTest.kt::addEntryLosslessAppendsToClientArray");
}

#[test]
fn mixin_config_editor_test_add_entry_lossless_appends_to_comment_only_empty_array() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinConfigEditorTest.kt::addEntryLosslessAppendsToCommentOnlyEmptyArray");
}

#[test]
fn mixin_config_editor_test_add_entry_lossless_appends_to_empty_array() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinConfigEditorTest.kt::addEntryLosslessAppendsToEmptyArray");
}

#[test]
fn mixin_config_editor_test_add_entry_lossless_appends_to_server_array() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinConfigEditorTest.kt::addEntryLosslessAppendsToServerArray");
}

#[test]
fn mixin_config_editor_test_add_entry_lossless_duplicate_is_idempotent_with_escaped_entry() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinConfigEditorTest.kt::addEntryLosslessDuplicateIsIdempotentWithEscapedEntry");
}

#[test]
fn mixin_config_editor_test_add_entry_lossless_edit_delta() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinConfigEditorTest.kt::addEntryLosslessEditDelta");
}

#[test]
fn mixin_config_editor_test_add_entry_lossless_escapes_special_characters() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinConfigEditorTest.kt::addEntryLosslessEscapesSpecialCharacters");
}

#[test]
fn mixin_config_editor_test_add_entry_lossless_ignores_comment_text() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinConfigEditorTest.kt::addEntryLosslessIgnoresCommentText");
}

#[test]
fn mixin_config_editor_test_add_entry_lossless_ignores_nested_array_key() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinConfigEditorTest.kt::addEntryLosslessIgnoresNestedArrayKey");
}

#[test]
fn mixin_config_editor_test_add_entry_lossless_ignores_string_literal_key_text() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinConfigEditorTest.kt::addEntryLosslessIgnoresStringLiteralKeyText");
}

#[test]
fn mixin_config_editor_test_add_entry_lossless_inserts_mixins_array_when_missing() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinConfigEditorTest.kt::addEntryLosslessInsertsMixinsArrayWhenMissing");
}

#[test]
fn mixin_config_editor_test_add_entry_lossless_inserts_mixins_array_with_lf_line_ending_before_closing_brace(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinConfigEditorTest.kt::addEntryLosslessInsertsMixinsArrayWithLfLineEndingBeforeClosingBrace");
}

#[test]
fn mixin_config_editor_test_add_entry_lossless_preserves_crlf() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinConfigEditorTest.kt::addEntryLosslessPreservesCrlf");
}

#[test]
fn mixin_config_editor_test_add_entry_lossless_preserves_trailing_comma() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinConfigEditorTest.kt::addEntryLosslessPreservesTrailingComma");
}

#[test]
fn mixin_config_editor_test_add_entry_lossless_trailing_comma_before_comments_does_not_double_comma(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinConfigEditorTest.kt::addEntryLosslessTrailingCommaBeforeCommentsDoesNotDoubleComma");
}

#[test]
fn mixin_config_editor_test_add_entry_preserves_existing_order() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinConfigEditorTest.kt::addEntryPreservesExistingOrder");
}

#[test]
fn mixin_config_editor_test_contains_entry_checks_all_arrays() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinConfigEditorTest.kt::containsEntryChecksAllArrays");
}

#[test]
fn mixin_config_editor_test_list_mixin_classes_returns_sorted_distinct_entries() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinConfigEditorTest.kt::listMixinClassesReturnsSortedDistinctEntries");
}

#[test]
fn mixin_config_editor_test_parses_json5_comments_and_trailing_commas() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinConfigEditorTest.kt::parsesJson5CommentsAndTrailingCommas");
}

#[test]
fn mixin_config_editor_test_parses_mixin_config_object() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinConfigEditorTest.kt::parsesMixinConfigObject");
}

#[test]
fn mixin_config_editor_test_parses_root_array_config() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinConfigEditorTest.kt::parsesRootArrayConfig");
}

#[test]
fn mixin_side_resolver_test_allows_whitespace_before_annotation_paren() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinSideResolverTest.kt::allowsWhitespaceBeforeAnnotationParen");
}

#[test]
fn mixin_side_resolver_test_fabric_environment_client() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinSideResolverTest.kt::fabricEnvironmentClient");
}

#[test]
fn mixin_side_resolver_test_fabric_environment_server() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinSideResolverTest.kt::fabricEnvironmentServer");
}

#[test]
fn mixin_side_resolver_test_forge_only_in_client() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinSideResolverTest.kt::forgeOnlyInClient");
}

#[test]
fn mixin_side_resolver_test_forge_only_in_dedicated_server() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinSideResolverTest.kt::forgeOnlyInDedicatedServer");
}

#[test]
fn mixin_side_resolver_test_fully_qualified_annotation_and_enum_values() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinSideResolverTest.kt::fullyQualifiedAnnotationAndEnumValues");
}

#[test]
fn mixin_side_resolver_test_ignores_method_annotation_after_class_declaration() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinSideResolverTest.kt::ignoresMethodAnnotationAfterClassDeclaration");
}

#[test]
fn mixin_side_resolver_test_ignores_side_annotation_examples_in_comments() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinSideResolverTest.kt::ignoresSideAnnotationExamplesInComments");
}

#[test]
fn mixin_side_resolver_test_ignores_unqualified_side_annotation_and_finds_later_valid_one() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinSideResolverTest.kt::ignoresUnqualifiedSideAnnotationAndFindsLaterValidOne");
}

#[test]
fn mixin_side_resolver_test_legacy_side_only_client() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinSideResolverTest.kt::legacySideOnlyClient");
}

#[test]
fn mixin_side_resolver_test_legacy_side_only_server() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinSideResolverTest.kt::legacySideOnlyServer");
}

#[test]
fn mixin_side_resolver_test_neo_forge_fully_qualified_only_in() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinSideResolverTest.kt::neoForgeFullyQualifiedOnlyIn");
}

#[test]
fn mixin_side_resolver_test_net_minecraft_forge_fml_fully_qualified_side_only() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinSideResolverTest.kt::netMinecraftForgeFmlFullyQualifiedSideOnly");
}

#[test]
fn mixin_side_resolver_test_returns_null_for_conflicting_client_and_server_annotations() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinSideResolverTest.kt::returnsNullForConflictingClientAndServerAnnotations");
}

#[test]
fn mixin_side_resolver_test_returns_null_for_malformed_side_annotation() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinSideResolverTest.kt::returnsNullForMalformedSideAnnotation");
}

#[test]
fn mixin_side_resolver_test_returns_null_for_unqualified_ambiguous_enum_value() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinSideResolverTest.kt::returnsNullForUnqualifiedAmbiguousEnumValue");
}

#[test]
fn mixin_side_resolver_test_returns_null_when_no_side_annotation_present() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinSideResolverTest.kt::returnsNullWhenNoSideAnnotationPresent");
}

#[test]
fn kotlin_completion_editing_test_kotlin_class_completion_matches_alias_prefixes_and_keeps_java_search_unchanged(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/KotlinCompletionEditingTest.kt::kotlinClassCompletionMatchesAliasPrefixesAndKeepsJavaSearchUnchanged");
}

#[test]
fn kotlin_completion_editing_test_kotlin_class_completion_uses_class_reference_and_existing_alias()
{
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/KotlinCompletionEditingTest.kt::kotlinClassCompletionUsesClassReferenceAndExistingAlias");
}

#[test]
fn kotlin_completion_editing_test_kotlin_import_edit_preserves_cr_lf_and_existing_aliases() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/KotlinCompletionEditingTest.kt::kotlinImportEditPreservesCrLfAndExistingAliases");
}

#[test]
fn kotlin_completion_editing_test_kotlin_imports_follow_file_annotations_and_do_not_shadow_bound_names(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/KotlinCompletionEditingTest.kt::kotlinImportsFollowFileAnnotationsAndDoNotShadowBoundNames");
}

#[test]
fn kotlin_completion_editing_test_kotlin_imports_follow_the_whole_file_annotation_without_package_or_imports(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/KotlinCompletionEditingTest.kt::kotlinImportsFollowTheWholeFileAnnotationWithoutPackageOrImports");
}

#[test]
fn kotlin_completion_editing_test_kotlin_injector_attributes_use_arrays_and_annotation_constructors(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/KotlinCompletionEditingTest.kt::kotlinInjectorAttributesUseArraysAndAnnotationConstructors");
}

#[test]
fn kotlin_completion_editing_test_kotlin_snippet_types_reuse_existing_import_aliases() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/KotlinCompletionEditingTest.kt::kotlinSnippetTypesReuseExistingImportAliases");
}

#[test]
fn kotlin_completion_editing_test_lapis_attributes_match_published_api_rather_than_java_mixin() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/KotlinCompletionEditingTest.kt::lapisAttributesMatchPublishedApiRatherThanJavaMixin");
}

#[test]
fn kotlin_completion_editing_test_unimported_kotlin_snippet_types_use_fully_qualified_names() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/KotlinCompletionEditingTest.kt::unimportedKotlinSnippetTypesUseFullyQualifiedNames");
}

#[test]
fn at_target_mapping_test_leaves_return_targets_unchanged_when_remapping() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinEdgeCasesTest.kt::leavesReturnTargetsUnchangedWhenRemapping");
}

#[test]
fn at_target_mapping_test_preserves_named_descriptor_without_resolver() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinEdgeCasesTest.kt::preservesNamedDescriptorWithoutResolver");
}

#[test]
fn at_target_mapping_test_remaps_named_method_target_to_intermediary_using_fixture_mappings() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinEdgeCasesTest.kt::remapsNamedMethodTargetToIntermediaryUsingFixtureMappings");
}

#[test]
fn mixin_edge_cases_test_completes_class_inside_mixin_array_form_using_manual_context() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinEdgeCasesTest.kt::completesClassInsideMixinArrayFormUsingManualContext");
}

#[test]
fn mixin_edge_cases_test_constant_at_completion_includes_int_value_hint() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinEdgeCasesTest.kt::constantAtCompletionIncludesIntValueHint");
}

#[test]
fn mixin_edge_cases_test_constant_at_completion_includes_string_value_hint() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinEdgeCasesTest.kt::constantAtCompletionIncludesStringValueHint");
}
