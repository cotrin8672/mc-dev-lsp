#[path = "fixtures/commands/replay.rs"]
mod replay;

#[test]
fn mcdev_command_dispatcher_test_info_returns_structured_lines() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/command/McdevCommandDispatcherTest.kt::infoReturnsStructuredLines");
}

#[test]
fn mcdev_command_dispatcher_test_completion_returns_non_empty_mixin_items() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/command/McdevCommandDispatcherTest.kt::completionReturnsNonEmptyMixinItems");
}

#[test]
fn mcdev_command_dispatcher_test_diagnostics_returns_diagnostics_envelope() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/command/McdevCommandDispatcherTest.kt::diagnosticsReturnsDiagnosticsEnvelope");
}

#[test]
fn mcdev_command_dispatcher_test_context_alias_returns_diagnostics_envelope() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/command/McdevCommandDispatcherTest.kt::contextAliasReturnsDiagnosticsEnvelope");
}

#[test]
fn mcdev_command_dispatcher_test_reindex_returns_structured_status() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/command/McdevCommandDispatcherTest.kt::reindexReturnsStructuredStatus");
}

#[test]
fn mcdev_command_dispatcher_test_production_dispatcher_reindex_invalidates_the_shared_project_context(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/command/McdevCommandDispatcherTest.kt::productionDispatcherReindexInvalidatesTheSharedProjectContext");
}

#[test]
fn mcdev_command_dispatcher_test_reload_project_context_returns_structured_status() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/command/McdevCommandDispatcherTest.kt::reloadProjectContextReturnsStructuredStatus");
}

#[test]
fn mcdev_command_dispatcher_test_dump_context_returns_structured_project_details() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/command/McdevCommandDispatcherTest.kt::dumpContextReturnsStructuredProjectDetails");
}

#[test]
fn mcdev_command_dispatcher_test_unknown_command_returns_structured_error() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/command/McdevCommandDispatcherTest.kt::unknownCommandReturnsStructuredError");
}

#[test]
fn mcdev_command_dispatcher_test_definition_returns_mixin_class_target() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/command/McdevCommandDispatcherTest.kt::definitionReturnsMixinClassTarget");
}

#[test]
fn mcdev_command_dispatcher_test_code_action_returns_mixin_config_fix() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/command/McdevCommandDispatcherTest.kt::codeActionReturnsMixinConfigFix");
}

#[test]
fn mcdev_command_dispatcher_test_references_returns_mixin_usages() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/command/McdevCommandDispatcherTest.kt::referencesReturnsMixinUsages");
}

#[test]
fn mcdev_command_dispatcher_test_hover_returns_mixin_class_details() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/command/McdevCommandDispatcherTest.kt::hoverReturnsMixinClassDetails");
}

#[test]
fn mcdev_command_dispatcher_test_registry_lists_implemented_commands() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/command/McdevCommandDispatcherTest.kt::registryListsImplementedCommands");
}

#[test]
fn mcdev_command_dispatcher_test_completion_with_missing_workspace_returns_error() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/command/McdevCommandDispatcherTest.kt::completionWithMissingWorkspaceReturnsError");
}

#[test]
fn mcdev_command_dispatcher_test_end_to_end_decode_and_execute_roundtrip() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/command/McdevCommandDispatcherTest.kt::endToEndDecodeAndExecuteRoundtrip");
}

#[test]
fn mcdev_delegate_command_handler_test_execute_command_returns_encoded_envelope() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/McdevDelegateCommandHandlerTest.kt::executeCommandReturnsEncodedEnvelope");
}

#[test]
fn mcdev_delegate_command_handler_test_unknown_command_throws_unsupported_operation_exception() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/McdevDelegateCommandHandlerTest.kt::unknownCommandThrowsUnsupportedOperationException");
}

#[test]
fn mcdev_delegate_command_handler_test_expression_cancellation_propagates_and_next_request_succeeds(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/McdevDelegateCommandHandlerTest.kt::expressionCancellationPropagatesAndNextRequestSucceeds");
}

#[test]
fn kotlin_mixin_request_handler_test_kotlin_requests_use_workspace_classpath_and_unsaved_source_without_compilation_unit(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/KotlinMixinRequestHandlerTest.kt::kotlinRequestsUseWorkspaceClasspathAndUnsavedSourceWithoutCompilationUnit");
}

#[test]
fn kotlin_mixin_request_handler_test_kotlin_project_readiness_uses_workspace_and_does_not_cache_an_importing_session(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/KotlinMixinRequestHandlerTest.kt::kotlinProjectReadinessUsesWorkspaceAndDoesNotCacheAnImportingSession");
}

#[test]
fn kotlin_mixin_request_handler_test_java_info_keeps_loading_session_without_kotlin_readiness_checks(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/KotlinMixinRequestHandlerTest.kt::javaInfoKeepsLoadingSessionWithoutKotlinReadinessChecks");
}

#[test]
fn mcdev_aw_at_code_action_handler_test_returns_remove_duplicate_fix_for_access_widener_buffer() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevAwAtCodeActionHandlerTest.kt::returnsRemoveDuplicateFixForAccessWidenerBuffer");
}

#[test]
fn mcdev_aw_at_code_action_handler_test_returns_add_descriptor_fix_for_access_transformer_buffer() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevAwAtCodeActionHandlerTest.kt::returnsAddDescriptorFixForAccessTransformerBuffer");
}

#[test]
fn mcdev_aw_at_completion_handler_test_returns_access_widener_directive_completions_for_language_id(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevAwAtCompletionHandlerTest.kt::returnsAccessWidenerDirectiveCompletionsForLanguageId");
}

#[test]
fn mcdev_aw_at_completion_handler_test_returns_access_widener_class_completions_by_file_extension()
{
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevAwAtCompletionHandlerTest.kt::returnsAccessWidenerClassCompletionsByFileExtension");
}

#[test]
fn mcdev_aw_at_completion_handler_test_returns_access_transformer_modifier_completions_for_at_cfg_buffer(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevAwAtCompletionHandlerTest.kt::returnsAccessTransformerModifierCompletionsForAtCfgBuffer");
}

#[test]
fn mcdev_aw_at_completion_handler_test_returns_access_widener_descriptor_completions_at_descriptor_slot(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevAwAtCompletionHandlerTest.kt::returnsAccessWidenerDescriptorCompletionsAtDescriptorSlot");
}

#[test]
fn mcdev_aw_at_completion_handler_test_returns_access_transformer_field_completions_with_intermediary_insert_text(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevAwAtCompletionHandlerTest.kt::returnsAccessTransformerFieldCompletionsWithIntermediaryInsertText");
}

#[test]
fn mcdev_aw_at_completion_handler_test_returns_access_transformer_member_completions_with_mapped_insert_text(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevAwAtCompletionHandlerTest.kt::returnsAccessTransformerMemberCompletionsWithMappedInsertText");
}

#[test]
fn mcdev_aw_at_diagnostics_handler_test_reports_unresolved_access_widener_class_for_broken_buffer()
{
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevAwAtDiagnosticsHandlerTest.kt::reportsUnresolvedAccessWidenerClassForBrokenBuffer");
}

#[test]
fn mcdev_aw_at_diagnostics_handler_test_clean_access_widener_fixture_produces_no_unresolved_targets(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevAwAtDiagnosticsHandlerTest.kt::cleanAccessWidenerFixtureProducesNoUnresolvedTargets");
}

#[test]
fn mcdev_aw_at_diagnostics_handler_test_clean_access_transformer_fixture_produces_no_unresolved_targets(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevAwAtDiagnosticsHandlerTest.kt::cleanAccessTransformerFixtureProducesNoUnresolvedTargets");
}

#[test]
fn mcdev_aw_at_diagnostics_handler_test_reports_namespace_mismatch_for_intermediary_owner_in_named_aw(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevAwAtDiagnosticsHandlerTest.kt::reportsNamespaceMismatchForIntermediaryOwnerInNamedAw");
}

#[test]
fn mcdev_aw_at_diagnostics_handler_test_reports_missing_method_descriptor_for_access_transformer_buffer(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevAwAtDiagnosticsHandlerTest.kt::reportsMissingMethodDescriptorForAccessTransformerBuffer");
}

#[test]
fn mcdev_aw_at_diagnostics_handler_test_detects_access_transformer_buffer_by_file_extension() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevAwAtDiagnosticsHandlerTest.kt::detectsAccessTransformerBufferByFileExtension");
}

#[test]
fn mcdev_code_action_handler_test_returns_mixin_config_add_fix() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCodeActionHandlerTest.kt::returnsMixinConfigAddFix");
}

#[test]
fn mcdev_code_action_handler_test_missing_workspace_root_returns_incomplete_context_error() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCodeActionHandlerTest.kt::missingWorkspaceRootReturnsIncompleteContextError");
}

#[test]
fn mcdev_code_action_handler_test_protocol_mismatch_returns_structured_error() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCodeActionHandlerTest.kt::protocolMismatchReturnsStructuredError");
}

#[test]
fn mcdev_code_action_handler_test_returns_ambiguous_inject_method_descriptor_fix() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCodeActionHandlerTest.kt::returnsAmbiguousInjectMethodDescriptorFix");
}

#[test]
fn mcdev_code_action_handler_test_returns_mixin_extras_fix_handler_signature_through_handler() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCodeActionHandlerTest.kt::returnsMixinExtrasFixHandlerSignatureThroughHandler");
}

#[test]
fn mcdev_code_action_handler_test_keeps_distinct_mixin_extras_signature_fixes_with_the_same_title()
{
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCodeActionHandlerTest.kt::keepsDistinctMixinExtrasSignatureFixesWithTheSameTitle");
}

#[test]
fn mcdev_code_action_handler_test_keeps_generation_at_cursor_when_diagnostic_filter_is_present() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCodeActionHandlerTest.kt::keepsGenerationAtCursorWhenDiagnosticFilterIsPresent");
}

#[test]
fn mcdev_code_action_handler_test_code_action_edits_include_workspace_document_uri() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCodeActionHandlerTest.kt::codeActionEditsIncludeWorkspaceDocumentUri");
}

#[test]
fn mcdev_code_action_handler_test_code_action_mixin_config_edit_document_uri_is_file_uri() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCodeActionHandlerTest.kt::codeActionMixinConfigEditDocumentUriIsFileUri");
}

#[test]
fn mcdev_code_action_handler_test_code_action_targets_selected_mixin_config_when_earlier_config_is_wrong(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCodeActionHandlerTest.kt::codeActionTargetsSelectedMixinConfigWhenEarlierConfigIsWrong");
}

#[test]
fn mcdev_code_action_handler_test_code_action_preserves_json5_comment_and_trailing_comma_in_selected_mixin_config(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCodeActionHandlerTest.kt::codeActionPreservesJson5CommentAndTrailingCommaInSelectedMixinConfig");
}

#[test]
fn mcdev_code_action_handler_test_code_action_scopes_mixin_config_to_fabric_source_set_when_editing_fabric_java(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCodeActionHandlerTest.kt::codeActionScopesMixinConfigToFabricSourceSetWhenEditingFabricJava");
}

#[test]
fn mcdev_code_action_handler_test_code_action_uses_config_that_lists_mixin_class() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCodeActionHandlerTest.kt::codeActionUsesConfigThatListsMixinClass");
}

#[test]
fn mcdev_completion_handler_test_returns_mixin_class_completions_for_fixture() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::returnsMixinClassCompletionsForFixture");
}

#[test]
fn mcdev_completion_handler_test_mixin_class_completion_never_invokes_source_class_scanner_on_production_path(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::mixinClassCompletionNeverInvokesSourceClassScannerOnProductionPath");
}

#[test]
fn mcdev_completion_handler_test_mixin_class_completion_returns_buffer_declared_class_when_jdt_unavailable(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::mixinClassCompletionReturnsBufferDeclaredClassWhenJdtUnavailable");
}

#[test]
fn mcdev_completion_handler_test_returns_empty_items_when_cursor_is_outside_annotation() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::returnsEmptyItemsWhenCursorIsOutsideAnnotation");
}

#[test]
fn mcdev_completion_handler_test_reuses_semantic_model_for_same_document_version() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::reusesSemanticModelForSameDocumentVersion");
}

#[test]
fn mcdev_completion_handler_test_reuses_negative_completion_for_same_zero_item_request() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::reusesNegativeCompletionForSameZeroItemRequest");
}

#[test]
fn mcdev_completion_handler_test_reuses_negative_completion_before_ttl_expires() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::reusesNegativeCompletionBeforeTtlExpires");
}

#[test]
fn mcdev_completion_handler_test_expires_negative_completion_after_ttl_and_recomputes() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::expiresNegativeCompletionAfterTtlAndRecomputes");
}

#[test]
fn mcdev_completion_handler_test_caps_negative_cache_at256_distinct_requests() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::capsNegativeCacheAt256DistinctRequests");
}

#[test]
fn mcdev_completion_handler_test_reuses_candidate_cache_for_same_inject_method_completion() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::reusesCandidateCacheForSameInjectMethodCompletion");
}

#[test]
fn mcdev_completion_handler_test_missing_workspace_root_returns_incomplete_context_error() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::missingWorkspaceRootReturnsIncompleteContextError");
}

#[test]
fn mcdev_completion_handler_test_protocol_mismatch_returns_structured_error() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::protocolMismatchReturnsStructuredError");
}

#[test]
fn mcdev_completion_handler_test_completion_items_include_metadata_and_kind() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::completionItemsIncludeMetadataAndKind");
}

#[test]
fn mcdev_completion_handler_test_import_mode_mixin_class_completion_includes_import_additional_edit(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::importModeMixinClassCompletionIncludesImportAdditionalEdit");
}

#[test]
fn mcdev_completion_handler_test_at_target_completion_remaps_insert_text_with_fixture_mappings() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::atTargetCompletionRemapsInsertTextWithFixtureMappings");
}

#[test]
fn mcdev_completion_handler_test_returns_mixin_extras_method_completions_through_facade() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::returnsMixinExtrasMethodCompletionsThroughFacade");
}

#[test]
fn mcdev_completion_handler_test_inject_method_completion_honors_always_descriptor_option() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::injectMethodCompletionHonorsAlwaysDescriptorOption");
}

#[test]
fn mcdev_completion_handler_test_returns_inject_method_completions_at_empty_open_quote() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::returnsInjectMethodCompletionsAtEmptyOpenQuote");
}

#[test]
fn mcdev_completion_handler_test_inject_method_edit_preserves_quotes_and_replaces_the_whole_string_value(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::injectMethodEditPreservesQuotesAndReplacesTheWholeStringValue");
}

#[test]
fn mcdev_completion_handler_test_inject_method_array_edit_only_replaces_the_active_element() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::injectMethodArrayEditOnlyReplacesTheActiveElement");
}

#[test]
fn mcdev_completion_handler_test_incomplete_method_attribute_offers_quoted_snippet() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::incompleteMethodAttributeOffersQuotedSnippet");
}

#[test]
fn mcdev_completion_handler_test_mixin_member_completion_does_not_scan_source_only_sibling_target()
{
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::mixinMemberCompletionDoesNotScanSourceOnlySiblingTarget");
}

#[test]
fn mcdev_completion_handler_test_at_target_completion_end_to_end_without_facade_override() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::atTargetCompletionEndToEndWithoutFacadeOverride");
}

#[test]
fn mcdev_completion_handler_test_fqn_insert_mode_returns_qualified_class_name() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::fqnInsertModeReturnsQualifiedClassName");
}

#[test]
fn mcdev_completion_handler_test_buffer_only_inject_attribute_completion_skips_project_session_load(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::bufferOnlyInjectAttributeCompletionSkipsProjectSessionLoad");
}

#[test]
fn mcdev_completion_handler_test_project_aware_completion_does_not_cache_session_before_jdt_project_exists(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::projectAwareCompletionDoesNotCacheSessionBeforeJdtProjectExists");
}

#[test]
fn mcdev_completion_handler_test_project_aware_completion_bypasses_buffer_only_shortcut() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::projectAwareCompletionBypassesBufferOnlyShortcut");
}

#[test]
fn mcdev_completion_handler_test_buffer_only_at_head_value_completion_skips_project_session_load() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::bufferOnlyAtHeadValueCompletionSkipsProjectSessionLoad");
}

#[test]
fn mcdev_completion_handler_test_buffer_only_at_head_value_completion_stays_stable_across_sequential_typing(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::bufferOnlyAtHeadValueCompletionStaysStableAcrossSequentialTyping");
}

#[test]
fn mcdev_completion_handler_test_buffer_only_definition_id_completion_skips_project_session_load() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::bufferOnlyDefinitionIdCompletionSkipsProjectSessionLoad");
}

#[test]
fn mcdev_completion_handler_test_buffer_only_definition_id_completion_does_not_leak_adjacent_handler_definitions(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::bufferOnlyDefinitionIdCompletionDoesNotLeakAdjacentHandlerDefinitions");
}

#[test]
fn mcdev_completion_handler_test_buffer_only_expression_return_keyword_completion_skips_project_session_load(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::bufferOnlyExpressionReturnKeywordCompletionSkipsProjectSessionLoad");
}

#[test]
fn mcdev_completion_handler_test_at_target_completion_loads_project_session() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::atTargetCompletionLoadsProjectSession");
}

#[test]
fn mcdev_completion_handler_test_access_widener_completion_loads_project_session() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::accessWidenerCompletionLoadsProjectSession");
}

#[test]
fn mcdev_completion_handler_test_access_transformer_completion_loads_project_session() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevCompletionHandlerTest.kt::accessTransformerCompletionLoadsProjectSession");
}

#[test]
fn mcdev_definition_handler_test_resolves_mixin_class_target() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevDefinitionHandlerTest.kt::resolvesMixinClassTarget");
}

#[test]
fn mcdev_definition_handler_test_resolves_shadow_field_target() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevDefinitionHandlerTest.kt::resolvesShadowFieldTarget");
}

#[test]
fn mcdev_definition_handler_test_resolves_access_widener_owner_to_class_definition() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevDefinitionHandlerTest.kt::resolvesAccessWidenerOwnerToClassDefinition");
}

#[test]
fn mcdev_definition_handler_test_resolves_access_transformer_member_to_field_definition() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevDefinitionHandlerTest.kt::resolvesAccessTransformerMemberToFieldDefinition");
}

#[test]
fn mcdev_definition_handler_test_resolves_loom_mixin_target_from_mapped_sources() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevDefinitionHandlerTest.kt::resolvesLoomMixinTargetFromMappedSources");
}

#[test]
fn mcdev_definition_handler_test_returns_empty_locations_outside_mixin_context() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevDefinitionHandlerTest.kt::returnsEmptyLocationsOutsideMixinContext");
}

#[test]
fn mcdev_definition_handler_test_missing_workspace_root_returns_incomplete_context_error() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevDefinitionHandlerTest.kt::missingWorkspaceRootReturnsIncompleteContextError");
}

#[test]
fn mcdev_definition_handler_test_definition_response_includes_source_range_metadata() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevDefinitionHandlerTest.kt::definitionResponseIncludesSourceRangeMetadata");
}

#[test]
fn mcdev_definition_handler_test_resolves_expression_definition_id_to_direct_source_location() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevDefinitionHandlerTest.kt::resolvesExpressionDefinitionIdToDirectSourceLocation");
}

#[test]
fn mcdev_definition_handler_test_preserves_definition_exact_method_navigation() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevDefinitionHandlerTest.kt::preservesDefinitionExactMethodNavigation");
}

#[test]
fn mcdev_diagnostics_handler_test_reports_unresolved_inject_method_for_broken_fixture() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevDiagnosticsHandlerTest.kt::reportsUnresolvedInjectMethodForBrokenFixture");
}

#[test]
fn mcdev_diagnostics_handler_test_reports_unresolved_at_target_for_broken_fixture() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevDiagnosticsHandlerTest.kt::reportsUnresolvedAtTargetForBrokenFixture");
}

#[test]
fn mcdev_diagnostics_handler_test_clean_fixture_produces_no_unresolved_mixin_target() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevDiagnosticsHandlerTest.kt::cleanFixtureProducesNoUnresolvedMixinTarget");
}

#[test]
fn mcdev_diagnostics_handler_test_reports_missing_mixin_config_entry_through_handler() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevDiagnosticsHandlerTest.kt::reportsMissingMixinConfigEntryThroughHandler");
}

#[test]
fn mcdev_diagnostics_handler_test_reports_shadow_static_mismatch_through_handler() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevDiagnosticsHandlerTest.kt::reportsShadowStaticMismatchThroughHandler");
}

#[test]
fn mcdev_diagnostics_handler_test_reports_mixin_extras_wrong_return_type_through_handler() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevDiagnosticsHandlerTest.kt::reportsMixinExtrasWrongReturnTypeThroughHandler");
}

#[test]
fn mcdev_diagnostics_handler_test_reports_ambiguous_inject_method_through_handler() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevDiagnosticsHandlerTest.kt::reportsAmbiguousInjectMethodThroughHandler");
}

#[test]
fn mcdev_diagnostics_handler_test_diagnostics_include_severity_and_message() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevDiagnosticsHandlerTest.kt::diagnosticsIncludeSeverityAndMessage");
}

#[test]
fn mcdev_diagnostics_handler_test_missing_workspace_root_returns_incomplete_context_error() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevDiagnosticsHandlerTest.kt::missingWorkspaceRootReturnsIncompleteContextError");
}

#[test]
fn mcdev_info_handler_test_returns_real_fabric_project_state() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevInfoHandlerTest.kt::returnsRealFabricProjectState");
}

#[test]
fn mcdev_info_handler_test_includes_protocol_and_extension_versions() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevInfoHandlerTest.kt::includesProtocolAndExtensionVersions");
}

#[test]
fn mcdev_info_handler_test_reports_access_widener_discovery_for_aw_at_fixture() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevInfoHandlerTest.kt::reportsAccessWidenerDiscoveryForAwAtFixture");
}

#[test]
fn mcdev_info_handler_test_reports_classpath_entry_count() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevInfoHandlerTest.kt::reportsClasspathEntryCount");
}

#[test]
fn mcdev_info_handler_test_missing_workspace_root_returns_incomplete_context_error() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevInfoHandlerTest.kt::missingWorkspaceRootReturnsIncompleteContextError");
}

#[test]
fn mcdev_project_context_handler_test_reload_clears_and_rebuilds_cached_project_context() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevProjectContextHandlerTest.kt::reloadClearsAndRebuildsCachedProjectContext");
}

#[test]
fn mcdev_project_context_handler_test_dump_returns_structured_project_context_details() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevProjectContextHandlerTest.kt::dumpReturnsStructuredProjectContextDetails");
}

#[test]
fn mcdev_project_context_handler_test_dump_with_missing_workspace_root_returns_incomplete_context_error(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevProjectContextHandlerTest.kt::dumpWithMissingWorkspaceRootReturnsIncompleteContextError");
}

#[test]
fn mcdev_references_handler_test_finds_mixin_class_references_in_workspace() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevReferencesHandlerTest.kt::findsMixinClassReferencesInWorkspace");
}

#[test]
fn mcdev_references_handler_test_finds_shadow_field_references_in_buffer() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevReferencesHandlerTest.kt::findsShadowFieldReferencesInBuffer");
}

#[test]
fn mcdev_references_handler_test_finds_access_widener_references_from_java_field() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevReferencesHandlerTest.kt::findsAccessWidenerReferencesFromJavaField");
}

#[test]
fn mcdev_references_handler_test_finds_access_widener_references_from_access_widener_buffer() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevReferencesHandlerTest.kt::findsAccessWidenerReferencesFromAccessWidenerBuffer");
}

#[test]
fn mcdev_references_handler_test_returns_empty_when_definition_target_missing() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevReferencesHandlerTest.kt::returnsEmptyWhenDefinitionTargetMissing");
}

#[test]
fn mcdev_reindex_handler_test_reindex_returns_ready_state() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevReindexHandlerTest.kt::reindexReturnsReadyState");
}

#[test]
fn mcdev_reindex_handler_test_reindex_updates_cached_session() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevReindexHandlerTest.kt::reindexUpdatesCachedSession");
}

#[test]
fn mcdev_reindex_handler_test_missing_workspace_root_returns_incomplete_context_error() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/handler/McdevReindexHandlerTest.kt::missingWorkspaceRootReturnsIncompleteContextError");
}

#[test]
fn completion_truncation_test_only_capped_class_sources_are_incomplete() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/CompletionTruncationTest.kt::onlyCappedClassSourcesAreIncomplete");
}

#[test]
fn completion_truncation_test_wire_response_includes_is_incomplete() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/convert/CompletionTruncationTest.kt::wireResponseIncludesIsIncomplete");
}

#[test]
fn mcdev_completion_transport_test_authenticated_requests_are_handled_concurrently_and_return_json()
{
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/transport/McdevCompletionTransportTest.kt::authenticatedRequestsAreHandledConcurrentlyAndReturnJson");
}

#[test]
fn mcdev_completion_transport_test_request_deadline_produces_bounded_error() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/transport/McdevCompletionTransportTest.kt::requestDeadlineProducesBoundedError");
}

#[test]
fn mcdev_completion_transport_test_rejects_unauthenticated_request_before_dispatch() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/transport/McdevCompletionTransportTest.kt::rejectsUnauthenticatedRequestBeforeDispatch");
}

#[test]
fn mcdev_completion_transport_test_disconnect_cancels_in_flight_computation() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/transport/McdevCompletionTransportTest.kt::disconnectCancelsInFlightComputation");
}

#[test]
fn mcdev_completion_transport_test_completed_request_does_not_report_server_teardown_as_failure() {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/transport/McdevCompletionTransportTest.kt::completedRequestDoesNotReportServerTeardownAsFailure");
}

#[test]
fn mcdev_stdio_main_test_shaded_jar_keeps_line_protocol_alive_across_provisional_and_fallback_requests(
) {
    replay::replay_case("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/stdio/McdevStdioMainTest.kt::shadedJarKeepsLineProtocolAliveAcrossProvisionalAndFallbackRequests");
}
