#[path = "fixtures/mixinextras_locals/replay.rs"]
mod legacy_replay;
#[test]
fn local_capture_extractor_test_analysis_failure_fails_closed() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureExtractorTest.kt::analysisFailureFailsClosed");
}
#[test]
fn local_capture_extractor_test_body_local_is_not_marked_as_argument() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureExtractorTest.kt::bodyLocalIsNotMarkedAsArgument");
}
#[test]
fn local_capture_extractor_test_corrupt_class_bytes_fail_closed() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureExtractorTest.kt::corruptClassBytesFailClosed");
}
#[test]
fn local_capture_extractor_test_empty_class_bytes_fail_closed() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureExtractorTest.kt::emptyClassBytesFailClosed");
}
#[test]
fn local_capture_extractor_test_instance_method_excludes_receiver_and_marks_arguments() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureExtractorTest.kt::instanceMethodExcludesReceiverAndMarksArguments");
}
#[test]
fn local_capture_extractor_test_invalid_instruction_index_fails_closed() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureExtractorTest.kt::invalidInstructionIndexFailsClosed");
}
#[test]
fn local_capture_extractor_test_label_line_and_frame_nodes_do_not_shift_requested_occurrence() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureExtractorTest.kt::labelLineAndFrameNodesDoNotShiftRequestedOccurrence");
}
#[test]
fn local_capture_extractor_test_local_variable_table_name_attached_when_live() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureExtractorTest.kt::localVariableTableNameAttachedWhenLive");
}
#[test]
fn local_capture_extractor_test_local_variable_table_name_null_when_debug_info_absent() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureExtractorTest.kt::localVariableTableNameNullWhenDebugInfoAbsent");
}
#[test]
fn local_capture_extractor_test_missing_class_bytes_fail_closed() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureExtractorTest.kt::missingClassBytesFailClosed");
}
#[test]
fn local_capture_extractor_test_missing_method_fails_closed() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureExtractorTest.kt::missingMethodFailsClosed");
}
#[test]
fn local_capture_extractor_test_multiple_instruction_indices_share_single_analysis() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureExtractorTest.kt::multipleInstructionIndicesShareSingleAnalysis");
}
#[test]
fn local_capture_extractor_test_static_method_covers_wide_argument_slots() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureExtractorTest.kt::staticMethodCoversWideArgumentSlots");
}
#[test]
fn local_capture_type_resolver_test_resolve_definition_resolves_non_void_type_class_name() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureTypeResolverTest.kt::resolveDefinitionResolvesNonVoidTypeClassName");
}
#[test]
fn local_capture_type_resolver_test_resolve_definition_returns_null_for_missing_or_void_type_class_name(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureTypeResolverTest.kt::resolveDefinitionReturnsNullForMissingOrVoidTypeClassName");
}
#[test]
fn local_capture_type_resolver_test_resolve_definition_returns_null_for_unresolved_type_class_name()
{
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureTypeResolverTest.kt::resolveDefinitionReturnsNullForUnresolvedTypeClassName");
}
#[test]
fn local_capture_type_resolver_test_resolve_parameter_ignores_local_definition_only_type_attribute()
{
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureTypeResolverTest.kt::resolveParameterIgnoresLocalDefinitionOnlyTypeAttribute");
}
#[test]
fn local_capture_type_resolver_test_resolve_parameter_leaves_other_official_sugar_ref_types_unchanged(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureTypeResolverTest.kt::resolveParameterLeavesOtherOfficialSugarRefTypesUnchanged");
}
#[test]
fn local_capture_type_resolver_test_resolve_parameter_leaves_wrong_package_primitive_ref_descriptor_unwrapped(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureTypeResolverTest.kt::resolveParameterLeavesWrongPackagePrimitiveRefDescriptorUnwrapped");
}
#[test]
fn local_capture_type_resolver_test_resolve_parameter_resolves_plain_primitive_and_reference_types()
{
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureTypeResolverTest.kt::resolveParameterResolvesPlainPrimitiveAndReferenceTypes");
}
#[test]
fn local_capture_type_resolver_test_resolve_parameter_returns_null_for_raw_wildcard_multiple_or_unresolved_local_ref_generics(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureTypeResolverTest.kt::resolveParameterReturnsNullForRawWildcardMultipleOrUnresolvedLocalRefGenerics");
}
#[test]
fn local_capture_type_resolver_test_resolve_parameter_returns_null_for_unresolved_type() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureTypeResolverTest.kt::resolveParameterReturnsNullForUnresolvedType");
}
#[test]
fn local_capture_type_resolver_test_resolve_parameter_returns_null_for_wrong_package_local_ref_lookalike(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureTypeResolverTest.kt::resolveParameterReturnsNullForWrongPackageLocalRefLookalike");
}
#[test]
fn local_capture_type_resolver_test_resolve_parameter_unwraps_official_local_ref_erased_generic_type(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureTypeResolverTest.kt::resolveParameterUnwrapsOfficialLocalRefErasedGenericType");
}
#[test]
fn local_capture_type_resolver_test_resolve_parameter_unwraps_official_local_ref_reference_array_generic(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureTypeResolverTest.kt::resolveParameterUnwrapsOfficialLocalRefReferenceArrayGeneric");
}
#[test]
fn local_capture_type_resolver_test_resolve_parameter_unwraps_official_local_ref_with_concrete_generic(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureTypeResolverTest.kt::resolveParameterUnwrapsOfficialLocalRefWithConcreteGeneric");
}
#[test]
fn local_capture_type_resolver_test_resolve_parameter_unwraps_official_primitive_ref_types() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureTypeResolverTest.kt::resolveParameterUnwrapsOfficialPrimitiveRefTypes");
}
#[test]
fn local_capture_validation_service_test_ambiguous_site_does_not_discard_resolved_site() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureValidationServiceTest.kt::ambiguousSiteDoesNotDiscardResolvedSite");
}
#[test]
fn local_capture_validation_service_test_missing_class_bytes_with_empty_invoke_candidates_is_unavailable_not_not_found(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureValidationServiceTest.kt::missingClassBytesWithEmptyInvokeCandidatesIsUnavailableNotNotFound");
}
#[test]
fn local_capture_validation_service_test_resolved_capture_retains_raw_snapshot() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureValidationServiceTest.kt::resolvedCaptureRetainsRawSnapshot");
}
#[test]
fn local_capture_validation_service_test_unresolved_type_occurrence_and_extraction_are_unavailable_not_not_found(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureValidationServiceTest.kt::unresolvedTypeOccurrenceAndExtractionAreUnavailableNotNotFound");
}
#[test]
fn local_discriminator_resolver_test_args_only_filters_out_non_argument_locals() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalDiscriminatorResolverTest.kt::argsOnlyFiltersOutNonArgumentLocals");
}
#[test]
fn local_discriminator_resolver_test_args_only_implicit_ambiguous_uses_only_arguments() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalDiscriminatorResolverTest.kt::argsOnlyImplicitAmbiguousUsesOnlyArguments");
}
#[test]
fn local_discriminator_resolver_test_empty_names_fall_through_to_implicit() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalDiscriminatorResolverTest.kt::emptyNamesFallThroughToImplicit");
}
#[test]
fn local_discriminator_resolver_test_implicit_ambiguous_when_multiple_matching_candidates() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalDiscriminatorResolverTest.kt::implicitAmbiguousWhenMultipleMatchingCandidates");
}
#[test]
fn local_discriminator_resolver_test_implicit_not_found_when_no_descriptor_match() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalDiscriminatorResolverTest.kt::implicitNotFoundWhenNoDescriptorMatch");
}
#[test]
fn local_discriminator_resolver_test_implicit_resolves_single_matching_candidate() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalDiscriminatorResolverTest.kt::implicitResolvesSingleMatchingCandidate");
}
#[test]
fn local_discriminator_resolver_test_index_matches_exact_jvm_slot() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalDiscriminatorResolverTest.kt::indexMatchesExactJvmSlot");
}
#[test]
fn local_discriminator_resolver_test_index_not_found_when_slot_has_different_descriptor() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalDiscriminatorResolverTest.kt::indexNotFoundWhenSlotHasDifferentDescriptor");
}
#[test]
fn local_discriminator_resolver_test_index_precedence_over_names_when_ordinal_unspecified() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalDiscriminatorResolverTest.kt::indexPrecedenceOverNamesWhenOrdinalUnspecified");
}
#[test]
fn local_discriminator_resolver_test_names_not_found_when_no_name_matches() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalDiscriminatorResolverTest.kt::namesNotFoundWhenNoNameMatches");
}
#[test]
fn local_discriminator_resolver_test_names_resolve_single_match() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalDiscriminatorResolverTest.kt::namesResolveSingleMatch");
}
#[test]
fn local_discriminator_resolver_test_names_select_first_matching_slot_when_multiple_candidates_match(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalDiscriminatorResolverTest.kt::namesSelectFirstMatchingSlotWhenMultipleCandidatesMatch");
}
#[test]
fn local_discriminator_resolver_test_negative_index_falls_through_to_implicit() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalDiscriminatorResolverTest.kt::negativeIndexFallsThroughToImplicit");
}
#[test]
fn local_discriminator_resolver_test_negative_ordinal_falls_through_to_implicit() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalDiscriminatorResolverTest.kt::negativeOrdinalFallsThroughToImplicit");
}
#[test]
fn local_discriminator_resolver_test_ordinal_miss_returns_not_found() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalDiscriminatorResolverTest.kt::ordinalMissReturnsNotFound");
}
#[test]
fn local_discriminator_resolver_test_ordinal_precedence_over_index_and_names() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalDiscriminatorResolverTest.kt::ordinalPrecedenceOverIndexAndNames");
}
#[test]
fn local_discriminator_resolver_test_ordinal_selects_zero_based_candidate_in_stable_slot_order() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalDiscriminatorResolverTest.kt::ordinalSelectsZeroBasedCandidateInStableSlotOrder");
}
#[test]
fn local_discriminator_resolver_test_ordinal_uses_receiver_count_but_cannot_select_receiver() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalDiscriminatorResolverTest.kt::ordinalUsesReceiverCountButCannotSelectReceiver");
}
#[test]
fn local_discriminator_resolver_test_print_does_not_change_selection() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalDiscriminatorResolverTest.kt::printDoesNotChangeSelection");
}
#[test]
fn local_discriminator_resolver_test_stable_ordering_does_not_depend_on_input_candidate_order() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalDiscriminatorResolverTest.kt::stableOrderingDoesNotDependOnInputCandidateOrder");
}
#[test]
fn local_completion_service_test_args_only_filters_derived_candidates() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCompletionServiceTest.kt::argsOnlyFiltersDerivedCandidates");
}
#[test]
fn local_completion_service_test_completes_first_matching_name_across_snapshots() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCompletionServiceTest.kt::completesFirstMatchingNameAcrossSnapshots");
}
#[test]
fn local_completion_service_test_completes_index_values_present_in_every_snapshot() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCompletionServiceTest.kt::completesIndexValuesPresentInEverySnapshot");
}
#[test]
fn local_completion_service_test_completes_names_lexically_sorted() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCompletionServiceTest.kt::completesNamesLexicallySorted");
}
#[test]
fn local_completion_service_test_completes_ordinal_values_stable_across_snapshots() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCompletionServiceTest.kt::completesOrdinalValuesStableAcrossSnapshots");
}
#[test]
fn local_completion_service_test_respects_partial_prefix_for_names() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCompletionServiceTest.kt::respectsPartialPrefixForNames");
}
#[test]
fn local_completion_service_test_respects_partial_prefix_for_numeric_values() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCompletionServiceTest.kt::respectsPartialPrefixForNumericValues");
}
#[test]
fn local_completion_service_test_returns_empty_for_unsupported_attribute() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCompletionServiceTest.kt::returnsEmptyForUnsupportedAttribute");
}
#[test]
fn local_completion_service_test_returns_empty_when_any_result_unavailable() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCompletionServiceTest.kt::returnsEmptyWhenAnyResultUnavailable");
}
#[test]
fn local_completion_service_test_returns_empty_when_any_snapshots_empty() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCompletionServiceTest.kt::returnsEmptyWhenAnySnapshotsEmpty");
}
#[test]
fn local_completion_service_test_returns_empty_when_results_empty() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCompletionServiceTest.kt::returnsEmptyWhenResultsEmpty");
}
#[test]
fn local_completion_service_test_returns_empty_when_target_descriptor_unresolved() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCompletionServiceTest.kt::returnsEmptyWhenTargetDescriptorUnresolved");
}
#[test]
fn mixin_extras_completion_service_test_complete_at_target_for_wrap_operation_uses_bytecode_index()
{
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/MixinExtrasCompletionServiceTest.kt::completeAtTargetForWrapOperationUsesBytecodeIndex");
}
#[test]
fn mixin_extras_completion_service_test_completes_modify_expression_value_method() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/MixinExtrasCompletionServiceTest.kt::completesModifyExpressionValueMethod");
}
#[test]
fn mixin_extras_completion_service_test_completes_modify_receiver_method() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/MixinExtrasCompletionServiceTest.kt::completesModifyReceiverMethod");
}
#[test]
fn mixin_extras_completion_service_test_completes_modify_return_value_method() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/MixinExtrasCompletionServiceTest.kt::completesModifyReturnValueMethod");
}
#[test]
fn mixin_extras_completion_service_test_completes_wrap_method_target_method() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/MixinExtrasCompletionServiceTest.kt::completesWrapMethodTargetMethod");
}
#[test]
fn mixin_extras_completion_service_test_completes_wrap_operation_at_invoke_target() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/MixinExtrasCompletionServiceTest.kt::completesWrapOperationAtInvokeTarget");
}
#[test]
fn mixin_extras_completion_service_test_completes_wrap_operation_method() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/MixinExtrasCompletionServiceTest.kt::completesWrapOperationMethod");
}
#[test]
fn mixin_extras_completion_service_test_completes_wrap_with_condition_method() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/MixinExtrasCompletionServiceTest.kt::completesWrapWithConditionMethod");
}
#[test]
fn mixin_extras_completion_service_test_returns_empty_at_target_without_inject_method_name() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/MixinExtrasCompletionServiceTest.kt::returnsEmptyAtTargetWithoutInjectMethodName");
}
#[test]
fn mixin_extras_completion_service_test_returns_empty_for_non_method_slot() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/MixinExtrasCompletionServiceTest.kt::returnsEmptyForNonMethodSlot");
}
#[test]
fn mixin_extras_completion_service_test_wrap_operation_at_target_filters_by_partial() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/MixinExtrasCompletionServiceTest.kt::wrapOperationAtTargetFiltersByPartial");
}
#[test]
fn local_capture_runtime_semantics_test_analyzer_cancellation_escapes_and_the_next_request_still_succeeds(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureRuntimeSemanticsTest.kt::analyzerCancellationEscapesAndTheNextRequestStillSucceeds");
}
#[test]
fn local_capture_runtime_semantics_test_instance_local_ordinal_includes_receiver_but_does_not_select_it(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureRuntimeSemanticsTest.kt::instanceLocalOrdinalIncludesReceiverButDoesNotSelectIt");
}
#[test]
fn local_capture_runtime_semantics_test_preserves_boolean_byte_char_and_short_descriptors_for_loads_and_stores(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureRuntimeSemanticsTest.kt::preservesBooleanByteCharAndShortDescriptorsForLoadsAndStores");
}
#[test]
fn local_capture_runtime_semantics_test_preserves_int_like_descriptors_from_the_declared_method_signature_without_lvt(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureRuntimeSemanticsTest.kt::preservesIntLikeDescriptorsFromTheDeclaredMethodSignatureWithoutLvt");
}
#[test]
fn local_capture_runtime_semantics_test_typed_primitive_definitions_match_the_real_store_and_load_instructions(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/LocalCaptureRuntimeSemanticsTest.kt::typedPrimitiveDefinitionsMatchTheRealStoreAndLoadInstructions");
}
