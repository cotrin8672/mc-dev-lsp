#[path = "fixtures/awat/replay.rs"]
mod replay;

#[test]
fn original_manifest_and_explicit_adaptations_are_complete() {
    replay::validate_manifest();
}

#[test]
fn access_widener_code_action_service_test_apply_descriptor_fix_replaces_wrong_method_descriptor() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCodeActionServiceTest.kt::applyDescriptorFixReplacesWrongMethodDescriptor");
}

#[test]
fn access_widener_code_action_service_test_apply_generate_entry_fix_inserts_line() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCodeActionServiceTest.kt::applyGenerateEntryFixInsertsLine");
}

#[test]
fn access_widener_code_action_service_test_apply_remove_duplicate_fix_removes_line() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCodeActionServiceTest.kt::applyRemoveDuplicateFixRemovesLine");
}

#[test]
fn access_widener_code_action_service_test_generate_entry_fix_uses_editor_formatting() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCodeActionServiceTest.kt::generateEntryFixUsesEditorFormatting");
}

#[test]
fn access_widener_code_action_service_test_offers_fix_descriptor_for_mismatch() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCodeActionServiceTest.kt::offersFixDescriptorForMismatch");
}

#[test]
fn access_widener_code_action_service_test_offers_remap_namespace_fix() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCodeActionServiceTest.kt::offersRemapNamespaceFix");
}

#[test]
fn access_widener_code_action_service_test_offers_remove_duplicate_fix() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCodeActionServiceTest.kt::offersRemoveDuplicateFix");
}

#[test]
fn access_widener_completion_service_test_class_completion_shows_internal_name_detail() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::classCompletionShowsInternalNameDetail");
}

#[test]
fn access_widener_completion_service_test_completes_all_directives() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::completesAllDirectives");
}

#[test]
fn access_widener_completion_service_test_completes_classes() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::completesClasses");
}

#[test]
fn access_widener_completion_service_test_completes_descriptors_for_methods() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::completesDescriptorsForMethods");
}

#[test]
fn access_widener_completion_service_test_completes_directives() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::completesDirectives");
}

#[test]
fn access_widener_completion_service_test_completes_fields() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::completesFields");
}

#[test]
fn access_widener_completion_service_test_completes_header_namespaces() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::completesHeaderNamespaces");
}

#[test]
fn access_widener_completion_service_test_completes_kinds() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::completesKinds");
}

#[test]
fn access_widener_completion_service_test_completes_methods() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::completesMethods");
}

#[test]
fn access_widener_completion_service_test_completes_owner_from_partial_internal_class_path() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::completesOwnerFromPartialInternalClassPath");
}

#[test]
fn access_widener_completion_service_test_descriptor_completion_uses_value_kind() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::descriptorCompletionUsesValueKind");
}

#[test]
fn access_widener_completion_service_test_extendable_kind_completion_only_offers_class() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::extendableKindCompletionOnlyOffersClass");
}

#[test]
fn access_widener_completion_service_test_mapping_aware_owner_insertion_uses_target_namespace() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::mappingAwareOwnerInsertionUsesTargetNamespace");
}

#[test]
fn access_widener_completion_service_test_method_completion_includes_descriptor_at_descriptor_slot()
{
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::methodCompletionIncludesDescriptorAtDescriptorSlot");
}

#[test]
fn access_widener_completion_service_test_method_completion_inserts_name_only_at_name_slot() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::methodCompletionInsertsNameOnlyAtNameSlot");
}

#[test]
fn access_widener_completion_service_test_mutable_kind_completion_only_offers_field() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::mutableKindCompletionOnlyOffersField");
}

#[test]
fn access_widener_completion_service_test_returns_empty_for_unknown_owner_members() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::returnsEmptyForUnknownOwnerMembers");
}

#[test]
fn access_widener_definition_service_test_method_definition_does_not_fallback_when_descriptor_mismatches(
) {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDefinitionServiceTest.kt::methodDefinitionDoesNotFallbackWhenDescriptorMismatches");
}

#[test]
fn access_widener_definition_service_test_resolves_field_name_to_field_definition() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDefinitionServiceTest.kt::resolvesFieldNameToFieldDefinition");
}

#[test]
fn access_widener_definition_service_test_resolves_method_name_to_method_definition() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDefinitionServiceTest.kt::resolvesMethodNameToMethodDefinition");
}

#[test]
fn access_widener_definition_service_test_resolves_owner_to_class_definition() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDefinitionServiceTest.kt::resolvesOwnerToClassDefinition");
}

#[test]
fn access_widener_definition_service_test_returns_empty_on_directive_slot() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDefinitionServiceTest.kt::returnsEmptyOnDirectiveSlot");
}

#[test]
fn access_widener_diagnostics_service_test_detects_descriptor_mismatch() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::detectsDescriptorMismatch");
}

#[test]
fn access_widener_diagnostics_service_test_detects_duplicate_entry() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::detectsDuplicateEntry");
}

#[test]
fn access_widener_diagnostics_service_test_detects_extendable_on_non_class() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::detectsExtendableOnNonClass");
}

#[test]
fn access_widener_diagnostics_service_test_detects_invalid_descriptor() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::detectsInvalidDescriptor");
}

#[test]
fn access_widener_diagnostics_service_test_detects_invalid_directive() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::detectsInvalidDirective");
}

#[test]
fn access_widener_diagnostics_service_test_detects_invalid_kind() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::detectsInvalidKind");
}

#[test]
fn access_widener_diagnostics_service_test_detects_mutable_on_non_field() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::detectsMutableOnNonField");
}

#[test]
fn access_widener_diagnostics_service_test_detects_namespace_mismatch() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::detectsNamespaceMismatch");
}

#[test]
fn access_widener_diagnostics_service_test_detects_unresolved_class() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::detectsUnresolvedClass");
}

#[test]
fn access_widener_diagnostics_service_test_detects_unresolved_member() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::detectsUnresolvedMember");
}

#[test]
fn access_widener_diagnostics_service_test_duplicate_diagnostics_include_line_metadata() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::duplicateDiagnosticsIncludeLineMetadata");
}

#[test]
fn access_widener_diagnostics_service_test_parse_failure_uses_invalid_directive_code_for_bad_directive_line(
) {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::parseFailureUsesInvalidDirectiveCodeForBadDirectiveLine");
}

#[test]
fn access_widener_diagnostics_service_test_valid_file_produces_no_errors() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::validFileProducesNoErrors");
}

#[test]
fn access_widener_editor_test_entry_key_ignores_class_member_parts() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerEditorTest.kt::entryKeyIgnoresClassMemberParts");
}

#[test]
fn access_widener_editor_test_entry_key_includes_member_parts() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerEditorTest.kt::entryKeyIncludesMemberParts");
}

#[test]
fn access_widener_editor_test_formats_class_entry_deterministically() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerEditorTest.kt::formatsClassEntryDeterministically");
}

#[test]
fn access_widener_editor_test_formats_field_entry_deterministically() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerEditorTest.kt::formatsFieldEntryDeterministically");
}

#[test]
fn access_widener_editor_test_formats_method_entry_deterministically() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerEditorTest.kt::formatsMethodEntryDeterministically");
}

#[test]
fn access_widener_editor_test_insert_entry_appends_formatted_line() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerEditorTest.kt::insertEntryAppendsFormattedLine");
}

#[test]
fn access_widener_editor_test_remove_line_deletes_target_entry() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerEditorTest.kt::removeLineDeletesTargetEntry");
}

#[test]
fn access_widener_parser_test_parses_header_and_entries() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerParserTest.kt::parsesHeaderAndEntries");
}

#[test]
fn access_widener_parser_test_rejects_invalid_descriptor() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerParserTest.kt::rejectsInvalidDescriptor");
}

#[test]
fn access_widener_reference_service_test_finds_class_references_in_access_widener_file() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerReferenceServiceTest.kt::findsClassReferencesInAccessWidenerFile");
}

#[test]
fn access_widener_reference_service_test_finds_field_references_in_access_widener_file() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerReferenceServiceTest.kt::findsFieldReferencesInAccessWidenerFile");
}

#[test]
fn access_widener_reference_service_test_finds_method_references_in_access_widener_file() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerReferenceServiceTest.kt::findsMethodReferencesInAccessWidenerFile");
}

#[test]
fn access_widener_reference_service_test_returns_empty_when_no_matches() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerReferenceServiceTest.kt::returnsEmptyWhenNoMatches");
}

#[test]
fn aw_context_extractor_test_blank_entry_line_completes_directive() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AwContextExtractorTest.kt::blankEntryLineCompletesDirective");
}

#[test]
fn aw_context_extractor_test_extract_uses_line_character() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AwContextExtractorTest.kt::extractUsesLineCharacter");
}

#[test]
fn aw_context_extractor_test_extracts_descriptor_slot() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AwContextExtractorTest.kt::extractsDescriptorSlot");
}

#[test]
fn aw_context_extractor_test_extracts_directive_slot() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AwContextExtractorTest.kt::extractsDirectiveSlot");
}

#[test]
fn aw_context_extractor_test_extracts_header_namespace_slot() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AwContextExtractorTest.kt::extractsHeaderNamespaceSlot");
}

#[test]
fn aw_context_extractor_test_extracts_kind_slot() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AwContextExtractorTest.kt::extractsKindSlot");
}

#[test]
fn aw_context_extractor_test_extracts_member_name_slot() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AwContextExtractorTest.kt::extractsMemberNameSlot");
}

#[test]
fn aw_context_extractor_test_extracts_owner_slot() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AwContextExtractorTest.kt::extractsOwnerSlot");
}

#[test]
fn aw_context_extractor_test_parse_file_namespace_reads_header() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AwContextExtractorTest.kt::parseFileNamespaceReadsHeader");
}

#[test]
fn aw_context_extractor_test_to_offset_round_trips_line_character() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AwContextExtractorTest.kt::toOffsetRoundTripsLineCharacter");
}

#[test]
fn aw_context_extractor_test_trailing_whitespace_advances_to_the_next_slot() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AwContextExtractorTest.kt::trailingWhitespaceAdvancesToTheNextSlot");
}

#[test]
fn access_transformer_code_action_service_test_applies_add_descriptor_fix() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCodeActionServiceTest.kt::appliesAddDescriptorFix");
}

#[test]
fn access_transformer_code_action_service_test_applies_generate_entry_fix() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCodeActionServiceTest.kt::appliesGenerateEntryFix");
}

#[test]
fn access_transformer_code_action_service_test_applies_remap_fix_to_srg_names() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCodeActionServiceTest.kt::appliesRemapFixToSrgNames");
}

#[test]
fn access_transformer_code_action_service_test_applies_remove_duplicate_fix() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCodeActionServiceTest.kt::appliesRemoveDuplicateFix");
}

#[test]
fn access_transformer_code_action_service_test_generates_entry_fix_from_context() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCodeActionServiceTest.kt::generatesEntryFixFromContext");
}

#[test]
fn access_transformer_code_action_service_test_offers_add_descriptor_fix_for_missing_descriptor() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCodeActionServiceTest.kt::offersAddDescriptorFixForMissingDescriptor");
}

#[test]
fn access_transformer_code_action_service_test_offers_remap_fix_for_wrong_namespace() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCodeActionServiceTest.kt::offersRemapFixForWrongNamespace");
}

#[test]
fn access_transformer_code_action_service_test_offers_remove_duplicate_fix() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCodeActionServiceTest.kt::offersRemoveDuplicateFix");
}

#[test]
fn access_transformer_code_action_service_test_remap_action_does_not_use_first_overload_when_descriptor_mismatches(
) {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCodeActionServiceTest.kt::remapActionDoesNotUseFirstOverloadWhenDescriptorMismatches");
}

#[test]
fn access_transformer_code_action_service_test_remap_action_is_not_offered_for_overloaded_method_without_descriptor(
) {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCodeActionServiceTest.kt::remapActionIsNotOfferedForOverloadedMethodWithoutDescriptor");
}

#[test]
fn access_transformer_code_action_service_test_remap_action_uses_exact_descriptor_when_descriptor_is_present(
) {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCodeActionServiceTest.kt::remapActionUsesExactDescriptorWhenDescriptorIsPresent");
}

#[test]
fn access_transformer_code_action_service_test_remap_action_uses_single_candidate_only_when_descriptor_is_omitted(
) {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCodeActionServiceTest.kt::remapActionUsesSingleCandidateOnlyWhenDescriptorIsOmitted");
}

#[test]
fn access_transformer_completion_service_test_completes_all_modifiers() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCompletionServiceTest.kt::completesAllModifiers");
}

#[test]
fn access_transformer_completion_service_test_completes_classes_with_named_fqn_insert() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCompletionServiceTest.kt::completesClassesWithNamedFqnInsert");
}

#[test]
fn access_transformer_completion_service_test_completes_fields_with_named_insert_on_fabric() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCompletionServiceTest.kt::completesFieldsWithNamedInsertOnFabric");
}

#[test]
fn access_transformer_completion_service_test_completes_fields_with_srg_insert_on_forge() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCompletionServiceTest.kt::completesFieldsWithSrgInsertOnForge");
}

#[test]
fn access_transformer_completion_service_test_completes_methods_with_named_insert_on_fabric() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCompletionServiceTest.kt::completesMethodsWithNamedInsertOnFabric");
}

#[test]
fn access_transformer_completion_service_test_completes_methods_with_srg_insert_on_forge() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCompletionServiceTest.kt::completesMethodsWithSrgInsertOnForge");
}

#[test]
fn access_transformer_completion_service_test_filters_modifiers_by_prefix() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCompletionServiceTest.kt::filtersModifiersByPrefix");
}

#[test]
fn access_transformer_completion_service_test_returns_empty_for_descriptor_slot() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCompletionServiceTest.kt::returnsEmptyForDescriptorSlot");
}

#[test]
fn access_transformer_completion_service_test_returns_empty_when_owner_missing_for_member_completion(
) {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCompletionServiceTest.kt::returnsEmptyWhenOwnerMissingForMemberCompletion");
}

#[test]
fn access_transformer_completion_service_test_separates_display_label_from_insert_text() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCompletionServiceTest.kt::separatesDisplayLabelFromInsertText");
}

#[test]
fn access_transformer_definition_service_test_method_definition_does_not_fallback_when_descriptor_mismatches(
) {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDefinitionServiceTest.kt::methodDefinitionDoesNotFallbackWhenDescriptorMismatches");
}

#[test]
fn access_transformer_definition_service_test_resolves_field_member_to_field_definition() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDefinitionServiceTest.kt::resolvesFieldMemberToFieldDefinition");
}

#[test]
fn access_transformer_definition_service_test_resolves_method_member_to_method_definition() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDefinitionServiceTest.kt::resolvesMethodMemberToMethodDefinition");
}

#[test]
fn access_transformer_definition_service_test_resolves_owner_to_class_definition() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDefinitionServiceTest.kt::resolvesOwnerToClassDefinition");
}

#[test]
fn access_transformer_definition_service_test_returns_empty_on_modifier_slot() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDefinitionServiceTest.kt::returnsEmptyOnModifierSlot");
}

#[test]
fn access_transformer_diagnostics_service_test_accepts_descriptor_matching_later_overload() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::acceptsDescriptorMatchingLaterOverload");
}

#[test]
fn access_transformer_diagnostics_service_test_accepts_named_member_on_fabric() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::acceptsNamedMemberOnFabric");
}

#[test]
fn access_transformer_diagnostics_service_test_accepts_srg_member_on_forge() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::acceptsSrgMemberOnForge");
}

#[test]
fn access_transformer_diagnostics_service_test_no_missing_descriptor_for_field_entry() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::noMissingDescriptorForFieldEntry");
}

#[test]
fn access_transformer_diagnostics_service_test_no_unresolved_for_valid_class_only_entry() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::noUnresolvedForValidClassOnlyEntry");
}

#[test]
fn access_transformer_diagnostics_service_test_reports_duplicate_entry() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::reportsDuplicateEntry");
}

#[test]
fn access_transformer_diagnostics_service_test_reports_invalid_descriptor() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::reportsInvalidDescriptor");
}

#[test]
fn access_transformer_diagnostics_service_test_reports_invalid_descriptor_mismatch() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::reportsInvalidDescriptorMismatch");
}

#[test]
fn access_transformer_diagnostics_service_test_reports_invalid_descriptor_when_overloaded_methods_exist_but_descriptor_matches_none(
) {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::reportsInvalidDescriptorWhenOverloadedMethodsExistButDescriptorMatchesNone");
}

#[test]
fn access_transformer_diagnostics_service_test_reports_invalid_modifier() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::reportsInvalidModifier");
}

#[test]
fn access_transformer_diagnostics_service_test_reports_missing_method_descriptor() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::reportsMissingMethodDescriptor");
}

#[test]
fn access_transformer_diagnostics_service_test_reports_missing_method_descriptor_when_descriptor_is_omitted_and_overloads_exist(
) {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::reportsMissingMethodDescriptorWhenDescriptorIsOmittedAndOverloadsExist");
}

#[test]
fn access_transformer_diagnostics_service_test_reports_srg_mapping_not_found_when_method_mapping_missing(
) {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::reportsSrgMappingNotFoundWhenMethodMappingMissing");
}

#[test]
fn access_transformer_diagnostics_service_test_reports_unknown_method_only_when_method_name_itself_does_not_exist(
) {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::reportsUnknownMethodOnlyWhenMethodNameItselfDoesNotExist");
}

#[test]
fn access_transformer_diagnostics_service_test_reports_unresolved_class() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::reportsUnresolvedClass");
}

#[test]
fn access_transformer_diagnostics_service_test_reports_unresolved_member() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::reportsUnresolvedMember");
}

#[test]
fn access_transformer_diagnostics_service_test_reports_wrong_namespace_for_named_member_on_forge() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::reportsWrongNamespaceForNamedMemberOnForge");
}

#[test]
fn access_transformer_editor_test_append_entry_is_stable() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerEditorTest.kt::appendEntryIsStable");
}

#[test]
fn access_transformer_editor_test_entry_key_includes_descriptor() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerEditorTest.kt::entryKeyIncludesDescriptor");
}

#[test]
fn access_transformer_editor_test_formats_class_entry() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerEditorTest.kt::formatsClassEntry");
}

#[test]
fn access_transformer_editor_test_formats_field_entry() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerEditorTest.kt::formatsFieldEntry");
}

#[test]
fn access_transformer_editor_test_formats_method_entry_with_descriptor() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerEditorTest.kt::formatsMethodEntryWithDescriptor");
}

#[test]
fn access_transformer_editor_test_replace_line_no_op_when_unchanged() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerEditorTest.kt::replaceLineNoOpWhenUnchanged");
}

#[test]
fn access_transformer_parser_test_parses_class_field_and_method_entries() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerParserTest.kt::parsesClassFieldAndMethodEntries");
}

#[test]
fn access_transformer_parser_test_rejects_invalid_modifier() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerParserTest.kt::rejectsInvalidModifier");
}

#[test]
fn access_transformer_reference_service_test_finds_class_only_references() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerReferenceServiceTest.kt::findsClassOnlyReferences");
}

#[test]
fn access_transformer_reference_service_test_finds_field_references() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerReferenceServiceTest.kt::findsFieldReferences");
}

#[test]
fn access_transformer_reference_service_test_finds_method_references() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerReferenceServiceTest.kt::findsMethodReferences");
}

#[test]
fn access_transformer_reference_service_test_returns_empty_when_no_matches() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerReferenceServiceTest.kt::returnsEmptyWhenNoMatches");
}

#[test]
fn at_context_extractor_test_detects_member_descriptor_slot() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtContextExtractorTest.kt::detectsMemberDescriptorSlot");
}

#[test]
fn at_context_extractor_test_detects_member_name_slot() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtContextExtractorTest.kt::detectsMemberNameSlot");
}

#[test]
fn at_context_extractor_test_detects_member_slot_after_owner_on_trailing_cursor() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtContextExtractorTest.kt::detectsMemberSlotAfterOwnerOnTrailingCursor");
}

#[test]
fn at_context_extractor_test_detects_modifier_slot() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtContextExtractorTest.kt::detectsModifierSlot");
}

#[test]
fn at_context_extractor_test_detects_owner_slot() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtContextExtractorTest.kt::detectsOwnerSlot");
}

#[test]
fn at_context_extractor_test_detects_owner_slot_after_modifier_on_trailing_cursor() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtContextExtractorTest.kt::detectsOwnerSlotAfterModifierOnTrailingCursor");
}

#[test]
fn at_context_extractor_test_ignores_comment_suffix() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtContextExtractorTest.kt::ignoresCommentSuffix");
}

#[test]
fn at_context_extractor_test_parses_line_tokens() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtContextExtractorTest.kt::parsesLineTokens");
}

#[test]
fn at_context_extractor_test_returns_null_outside_source() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtContextExtractorTest.kt::returnsNullOutsideSource");
}

#[test]
fn at_member_insert_formatter_test_formats_owner_as_fqn() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtMemberInsertFormatterTest.kt::formatsOwnerAsFqn");
}

#[test]
fn at_member_insert_formatter_test_inserts_named_method_on_fabric() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtMemberInsertFormatterTest.kt::insertsNamedMethodOnFabric");
}

#[test]
fn at_member_insert_formatter_test_inserts_srg_method_on_forge() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtMemberInsertFormatterTest.kt::insertsSrgMethodOnForge");
}

#[test]
fn at_member_insert_formatter_test_reports_missing_mapping_for_unknown_method() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtMemberInsertFormatterTest.kt::reportsMissingMappingForUnknownMethod");
}

#[test]
fn aw_at_buffer_support_test_detects_access_transformer_by_extension() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtBufferSupportTest.kt::detectsAccessTransformerByExtension");
}

#[test]
fn aw_at_buffer_support_test_detects_access_transformer_by_language_id() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtBufferSupportTest.kt::detectsAccessTransformerByLanguageId");
}

#[test]
fn aw_at_buffer_support_test_detects_access_widener_by_extension() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtBufferSupportTest.kt::detectsAccessWidenerByExtension");
}

#[test]
fn aw_at_buffer_support_test_detects_access_widener_by_language_id() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtBufferSupportTest.kt::detectsAccessWidenerByLanguageId");
}

#[test]
fn aw_at_buffer_support_test_returns_null_for_java_buffers() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtBufferSupportTest.kt::returnsNullForJavaBuffers");
}

#[test]
fn aw_at_service_facade_e2_e_test_code_actions_offers_add_descriptor_fix_for_missing_method_descriptor(
) {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::codeActionsOffersAddDescriptorFixForMissingMethodDescriptor");
}

#[test]
fn aw_at_service_facade_e2_e_test_code_actions_offers_remove_duplicate_fix_for_access_widener() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::codeActionsOffersRemoveDuplicateFixForAccessWidener");
}

#[test]
fn aw_at_service_facade_e2_e_test_completes_access_transformer_classes_from_fixture() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::completesAccessTransformerClassesFromFixture");
}

#[test]
fn aw_at_service_facade_e2_e_test_completes_access_transformer_field_with_intermediary_insert_text()
{
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::completesAccessTransformerFieldWithIntermediaryInsertText");
}

#[test]
fn aw_at_service_facade_e2_e_test_completes_access_transformer_members_with_intermediary_names() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::completesAccessTransformerMembersWithIntermediaryNames");
}

#[test]
fn aw_at_service_facade_e2_e_test_completes_access_transformer_modifiers() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::completesAccessTransformerModifiers");
}

#[test]
fn aw_at_service_facade_e2_e_test_completes_access_widener_classes_from_fixture() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::completesAccessWidenerClassesFromFixture");
}

#[test]
fn aw_at_service_facade_e2_e_test_completes_access_widener_directives() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::completesAccessWidenerDirectives");
}

#[test]
fn aw_at_service_facade_e2_e_test_completes_access_widener_fields_from_fixture() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::completesAccessWidenerFieldsFromFixture");
}

#[test]
fn aw_at_service_facade_e2_e_test_completes_access_widener_kinds() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::completesAccessWidenerKinds");
}

#[test]
fn aw_at_service_facade_e2_e_test_completes_access_widener_method_descriptor_slot_from_fixture() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::completesAccessWidenerMethodDescriptorSlotFromFixture");
}

#[test]
fn aw_at_service_facade_e2_e_test_completes_access_widener_methods_from_fixture() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::completesAccessWidenerMethodsFromFixture");
}

#[test]
fn aw_at_service_facade_e2_e_test_diagnoses_unresolved_access_widener_class() {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::diagnosesUnresolvedAccessWidenerClass");
}

#[test]
fn aw_at_service_facade_e2_e_test_diagnoses_valid_fixture_access_transformer_without_unresolved_targets(
) {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::diagnosesValidFixtureAccessTransformerWithoutUnresolvedTargets");
}

#[test]
fn aw_at_service_facade_e2_e_test_diagnoses_valid_fixture_access_widener_without_unresolved_targets(
) {
    replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::diagnosesValidFixtureAccessWidenerWithoutUnresolvedTargets");
}
